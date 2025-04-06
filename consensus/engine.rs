use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::storage::state_store::StateStore;
use crate::storage::poh_store::PoHStore;
use crate::network::types::message::NetMessage;
use crate::consensus::config::ConsensusConfig;
use crate::consensus::types::{ChainState, Target, ForkChoice};
use crate::consensus::pow::difficulty::calculate_next_target;
use crate::consensus::pow::miner::PoWMiner;
use crate::consensus::poh::generator::PoHGenerator;
use crate::consensus::poh::verifier::PoHVerifier;
use crate::consensus::validation::block_validator::{BlockValidator, BlockValidationResult};
use crate::consensus::validation::transaction_validator::{TransactionValidator, TransactionValidationResult};
use crate::consensus::validation::fork_choice::{choose_fork, resolve_fork};
use crate::consensus::mining::mempool::Mempool;
use crate::consensus::mining::block_producer::BlockProducer;

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
    poh_generator: Arc<PoHGenerator>,
    
    /// Block producer
    block_producer: Arc<Mutex<BlockProducer<'static>>>,
    
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
        block_store: Arc<BlockStore<'static>>,
        tx_store: Arc<TxStore<'static>>,
        state_store: Arc<StateStore<'static>>,
        network_tx: mpsc::Sender<NetMessage>,
    ) -> Self {
        // Create channels
        let (block_tx, block_rx) = mpsc::channel(100);
        let (tx_tx, tx_rx) = mpsc::channel(1000);
        
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
        let poh_generator = Arc::new(PoHGenerator::new(&config));
        
        // Get the latest block
        let latest_block = match block_store.get_latest_height() {
            Some(height) => block_store.get_block_by_height(height).unwrap(),
            None => {
                // Create a genesis block
                let genesis = Block {
                    height: 0,
                    hash: [0u8; 32],
                    prev_hash: [0u8; 32],
                    timestamp: 0,
                    transactions: vec![],
                    state_root: [0u8; 32],
                };
                
                // Store the genesis block
                block_store.put_block(&genesis);
                
                genesis
            }
        };
        
        // Create the chain state
        let chain_state = Arc::new(Mutex::new(ChainState {
            height: latest_block.height,
            current_target: Target::from_difficulty(config.initial_difficulty),
            latest_hash: latest_block.hash,
            latest_timestamp: latest_block.timestamp,
            latest_poh_sequence: 0,
        }));
        
        // Create the block producer
        let block_producer = Arc::new(Mutex::new(BlockProducer::new(
            chain_state.lock().unwrap().clone(),
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
            chain_state,
            network_tx,
            block_rx,
            tx_rx,
        }
    }
    
    /// Run the consensus engine
    pub async fn run(&self) {
        // Start the PoH generator
        self.poh_generator.start().await;
        
        // Register network handlers
        self.register_network_handlers().await;
        
        // Start the main loop
        self.main_loop().await;
    }
    
    /// Register handlers for network messages
    async fn register_network_handlers(&self) {
        // TODO: Register handlers for network messages
    }
    
    /// Main consensus loop
    async fn main_loop(&self) {
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
        
        // Validate the block
        let validation_result = self.block_validator.validate_block(
            &block,
            &chain_state.current_target,
        );
        
        match validation_result {
            BlockValidationResult::Valid => {
                // Decide whether to accept the block
                let fork_choice = choose_fork(
                    &self.block_store,
                    &chain_state,
                    &block,
                );
                
                match fork_choice {
                    ForkChoice::Accept => {
                        // Accept the block
                        self.accept_block(block).await;
                    }
                    ForkChoice::Reject => {
                        // Reject the block
                        warn!("Rejecting block at height {}", block.height);
                    }
                    ForkChoice::Fork => {
                        // We have a fork, need to resolve it
                        warn!("Fork detected at height {}", block.height);
                        
                        // Get the current tip
                        let current_tip = self.block_store.get_block_by_hash(
                            &chain_state.latest_hash
                        ).unwrap();
                        
                        // Resolve the fork
                        let canonical_tip = resolve_fork(
                            &self.block_store,
                            &current_tip,
                            &block,
                        );
                        
                        // If the new block is the canonical tip, accept it
                        if canonical_tip.hash == block.hash {
                            self.accept_block(block).await;
                        }
                    }
                }
            }
            BlockValidationResult::Invalid(reason) => {
                warn!("Invalid block at height {}: {}", block.height, reason);
            }
            BlockValidationResult::AlreadyKnown => {
                debug!("Block already known at height {}", block.height);
            }
            BlockValidationResult::UnknownParent => {
                warn!("Block with unknown parent at height {}", block.height);
                
                // TODO: Request the parent block from the network
            }
        }
    }
    
    /// Accept a block
    async fn accept_block(&self, block: Block) {
        info!("Accepting block at height {}", block.height);
        
        // Store the block
        self.block_store.put_block(&block);
        
        // Update the chain state
        let mut chain_state = self.chain_state.lock().await;
        chain_state.height = block.height;
        chain_state.latest_hash = block.hash;
        chain_state.latest_timestamp = block.timestamp;
        
        // Update the block producer
        let mut block_producer = self.block_producer.lock().await;
        block_producer.update_chain_state(chain_state.clone());
        
        // Mark transactions as included
        for tx_id in &block.transactions {
            self.mempool.mark_included(tx_id);
        }
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
