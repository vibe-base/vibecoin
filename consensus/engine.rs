use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::storage::state_store::StateStore;
use crate::storage::poh_store::PoHStore;
use crate::storage::{BatchOperationManager, BatchOperationError, KVStore};
use crate::network::types::message::NetMessage;
use crate::consensus::config::ConsensusConfig;

/// Create a genesis block
fn create_genesis_block(config: &ConsensusConfig) -> Block {
    Block {
        height: 0,
        hash: [0u8; 32],
        prev_hash: [0u8; 32],
        timestamp: 0,
        transactions: vec![],
        state_root: [0u8; 32],
        tx_root: [0u8; 32],
        nonce: 0,
        poh_seq: 0,
        poh_hash: [0u8; 32],
        difficulty: config.initial_difficulty,
        total_difficulty: config.initial_difficulty as u128,
    }
}
use crate::consensus::types::{ChainState, Target};
use crate::consensus::validation::ForkChoice;
use crate::consensus::pow::difficulty::calculate_next_target;
use crate::consensus::pow::miner::PoWMiner;
use crate::consensus::poh::generator::PoHGenerator;
use crate::consensus::poh::verifier::PoHVerifier;
use crate::consensus::validation::block_validator::{BlockValidator, BlockValidationResult};
use crate::consensus::validation::transaction_validator::{TransactionValidator, TransactionValidationResult};
use crate::consensus::validation::fork_choice::{choose_fork, resolve_fork};
use crate::consensus::mining::mempool::Mempool;
use crate::consensus::mining::block_producer::BlockProducer;
use crate::consensus::block_processor::{BlockProcessor, BlockProcessingResult};

/// Main consensus engine
pub struct ConsensusEngine {
    /// Consensus configuration
    config: ConsensusConfig,

    /// Block store
    block_store: Arc<BlockStore<'static>>,

    /// Transaction store
    tx_store: Arc<TxStore<'static>>,

    /// State store
    state_store: Arc<StateStore<'static>>,

    /// Mempool
    mempool: Arc<Mempool>,

    /// Block validator
    block_validator: Arc<BlockValidator<'static>>,

    /// Transaction validator
    tx_validator: Arc<TransactionValidator<'static>>,

    /// PoH generator
    poh_generator: Arc<Mutex<PoHGenerator>>,

    /// Block producer
    block_producer: Arc<Mutex<BlockProducer<'static>>>,

    /// Block processor
    block_processor: Arc<BlockProcessor<'static>>,

    /// Batch operation manager
    batch_manager: Arc<BatchOperationManager<'static>>,

    /// Chain state
    chain_state: Arc<Mutex<ChainState>>,

    /// Network sender
    network_tx: mpsc::Sender<NetMessage>,

    /// Channel for new blocks
    block_rx: mpsc::Receiver<Block>,

    /// Channel for new transactions
    tx_rx: mpsc::Receiver<TransactionRecord>,
}

impl ConsensusEngine {
    /// Create a new consensus engine
    pub fn new(
        config: ConsensusConfig,
        kv_store: Arc<dyn KVStore + 'static>,
        block_store: Arc<BlockStore<'static>>,
        tx_store: Arc<TxStore<'static>>,
        state_store: Arc<StateStore<'static>>,
        network_tx: mpsc::Sender<NetMessage>,
    ) -> Self {
        // Create channels
        let (_block_tx, block_rx) = mpsc::channel(100);
        let (_tx_tx, tx_rx) = mpsc::channel(1000);

        // Create the mempool
        let mempool = Arc::new(Mempool::new(10000));

        // Create validators
        let block_validator = Arc::new(BlockValidator::new(
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
        ));

        let tx_validator = Arc::new(TransactionValidator::new(
            tx_store.clone(),
            state_store.clone(),
        ));

        // Create the PoH generator
        let poh_generator = Arc::new(Mutex::new(PoHGenerator::new(&config)));

        // Get the latest block
        let latest_block = match block_store.get_latest_height() {
            Some(height) => match block_store.get_block_by_height(height) {
                Ok(Some(block)) => block,
                _ => {
                    // Create a genesis block if we can't get the latest block
                    let genesis = create_genesis_block(&config);
                    block_store.put_block(&genesis).unwrap();
                    genesis
                }
            },
            None => {
                // Create a genesis block
                let genesis = create_genesis_block(&config);

                // Store the genesis block
                block_store.put_block(&genesis);

                genesis
            }
        };

        // Create the chain state
        let chain_state = Arc::new(Mutex::new(ChainState::new(
            latest_block.height,
            latest_block.hash,
            crate::storage::state::StateRoot::new(
                latest_block.state_root,
                latest_block.height,
                latest_block.timestamp
            ),
            latest_block.total_difficulty as u64,
            0, // finalized_height
            [0u8; 32], // finalized_hash
        )));

        // Create the batch operation manager
        let batch_manager = Arc::new(BatchOperationManager::new(
            kv_store.clone(),
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
        ));

        // Create the block processor
        let block_processor = Arc::new(BlockProcessor::new(
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
            batch_manager.clone(),
            block_validator.clone(),
            Some(mempool.clone()),
        ));

        // Create the block producer
        let block_producer = Arc::new(Mutex::new(BlockProducer::new(
            // Get a clone of the chain state
            chain_state.try_lock().expect("Failed to lock chain state").clone(),
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
            mempool.clone(),
            poh_generator.clone(),
            network_tx.clone(),
            config.clone(),
        )));

        Self {
            config,
            block_store,
            tx_store,
            state_store,
            mempool,
            block_validator,
            tx_validator,
            poh_generator,
            block_producer,
            block_processor,
            batch_manager,
            chain_state,
            network_tx,
            block_rx,
            tx_rx,
        }
    }

    /// Run the consensus engine
    pub async fn run(&mut self) {
        // Start the PoH generator
        {
            let mut poh_gen = self.poh_generator.lock().await;
            poh_gen.start().await;
        } // Release the lock before proceeding

        // Register network handlers
        self.register_network_handlers().await;

        // Start the main loop
        self.main_loop().await;
    }

    /// Get a channel for sending blocks to the consensus engine
    pub fn block_channel(self: &Arc<Self>) -> mpsc::Sender<Block> {
        // Create a new channel
        let (tx, rx) = mpsc::channel::<Block>(100);

        // Clone self as Arc to extend lifetime to 'static
        let engine = self.clone();

        // Spawn a task with 'static lifetime
        tokio::spawn(async move {
            let mut block_rx = rx;
            while let Some(block) = block_rx.recv().await {
                debug!("Received block from network: height={}", block.height);

                // Get the chain state
                let chain_state_guard = engine.chain_state.lock().await;

                // Process the block
                let result = engine.block_processor.process_block(&block, &chain_state_guard.current_target, &chain_state_guard).await;
                match result {
                    BlockProcessingResult::Success => {
                        debug!("Block processed: Success");
                    },
                    BlockProcessingResult::AlreadyKnown => {
                        debug!("Block processed: Already Known");
                    },
                    BlockProcessingResult::UnknownParent => {
                        debug!("Block processed: Unknown Parent");
                    },
                    BlockProcessingResult::Invalid(reason) => {
                        debug!("Block processed: Invalid - {}", reason);
                    },
                    BlockProcessingResult::Error(error) => {
                        debug!("Block processed: Error - {}", error);
                    },
                }
            }
        });

        tx
    }

    /// Register handlers for network messages
    async fn register_network_handlers(&mut self) {
        // TODO: Register handlers for network messages
    }

    /// Main consensus loop
    async fn main_loop(&mut self) {
        // Mining interval
        let mining_interval = self.config.target_block_time_duration();
        let mut mining_timer = time::interval(mining_interval);

        // Difficulty adjustment interval
        let difficulty_interval = Duration::from_secs(60);
        let mut difficulty_timer = time::interval(difficulty_interval);

        loop {
            tokio::select! {
                // Handle new blocks
                Some(block) = self.block_rx.recv() => {
                    self.handle_new_block(block).await;
                }

                // Handle new transactions
                Some(tx) = self.tx_rx.recv() => {
                    self.handle_new_transaction(tx).await;
                }

                // Mining timer
                _ = mining_timer.tick() => {
                    if self.config.enable_mining {
                        self.mine_block().await;
                    }
                }

                // Difficulty adjustment timer
                _ = difficulty_timer.tick() => {
                    self.adjust_difficulty().await;
                }
            }
        }
    }

    /// Handle a new block
    async fn handle_new_block(&self, block: Block) {
        info!("Received new block at height {}", block.height);

        // Get the current chain state
        let chain_state = self.chain_state.lock().await.clone();

        // Process the block using the block processor
        let result = self.block_processor.process_block(&block, &chain_state.current_target, &chain_state).await;

        match result {
            BlockProcessingResult::Success => {
                // Update the chain state
                self.update_chain_state(&block).await;

                info!("Block processed successfully at height {}", block.height);
            },
            BlockProcessingResult::AlreadyKnown => {
                debug!("Block already known at height {}", block.height);
            },
            BlockProcessingResult::UnknownParent => {
                warn!("Block with unknown parent at height {}", block.height);

                // TODO: Request the parent block from the network
            },
            BlockProcessingResult::Invalid(reason) => {
                warn!("Invalid block at height {}: {}", block.height, reason);
            },
            BlockProcessingResult::Error(error) => {
                error!("Error processing block at height {}: {}", block.height, error);
            }
        }
    }

    /// Update the chain state with a new block
    async fn update_chain_state(&self, block: &Block) {
        // Update the chain state
        let mut chain_state = self.chain_state.lock().await;
        chain_state.height = block.height;
        chain_state.tip_hash = block.hash;
        chain_state.latest_hash = block.hash;
        chain_state.latest_timestamp = block.timestamp;

        // Update the block producer
        let mut block_producer = self.block_producer.lock().await;
        block_producer.update_chain_state(chain_state.clone());
    }

    /// Handle a new transaction
    async fn handle_new_transaction(&self, tx: TransactionRecord) {
        debug!("Received new transaction: {:?}", tx.tx_id);

        // Add the transaction to the mempool
        if self.mempool.add_transaction(tx.clone()) {
            debug!("Added transaction to mempool: {:?}", tx.tx_id);
        } else {
            debug!("Transaction already in mempool: {:?}", tx.tx_id);
        }
    }

    /// Mine a new block
    async fn mine_block(&self) {
        // Get the block producer
        let block_producer = self.block_producer.lock().await;

        // Mine a block
        if let Some(block) = block_producer.mine_block().await {
            info!("Mined new block at height {}", block.height);

            // Handle the new block
            self.handle_new_block(block).await;
        }
    }

    /// Adjust the difficulty
    async fn adjust_difficulty(&self) {
        // Get the current chain state
        let mut chain_state = self.chain_state.lock().await;

        // Calculate the next target
        let next_target = calculate_next_target(
            &self.config,
            &self.block_store,
            chain_state.height,
        );

        // Update the chain state
        chain_state.current_target = next_target;

        // Update the block producer
        let mut block_producer = self.block_producer.lock().await;
        block_producer.update_chain_state(chain_state.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_consensus_engine_creation() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());

        // Create the stores
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let tx_store = Arc::new(TxStore::new(&kv_store));
        let state_store = Arc::new(StateStore::new(&kv_store));

        // Create a network channel
        let (network_tx, _network_rx) = mpsc::channel(100);

        // Create a config
        let config = ConsensusConfig::default();

        // Create the consensus engine
        let engine = ConsensusEngine::new(
            config,
            block_store,
            tx_store,
            state_store,
            network_tx,
        );

        // Check that the engine was created successfully
        assert_eq!(engine.config.target_block_time, 15);
    }
}
