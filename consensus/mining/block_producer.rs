use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::storage::tx_store::TxStore;
use crate::storage::state_store::StateStore;
use crate::consensus::types::{ChainState, BlockTemplate, Target};
use crate::consensus::config::ConsensusConfig;
use crate::consensus::pow::miner::{PoWMiner, MiningResult};
use crate::consensus::mining::mempool::Mempool;
use crate::consensus::poh::generator::PoHGenerator;
use crate::network::types::message::NetMessage;

/// Block producer for creating new blocks
pub struct BlockProducer<'a> {
    /// Chain state
    chain_state: ChainState,

    /// Block store
    block_store: Arc<BlockStore<'a>>,

    /// Transaction store
    tx_store: Arc<TxStore<'a>>,

    /// State store
    state_store: Arc<StateStore<'a>>,

    /// Mempool
    mempool: Arc<Mempool>,

    /// PoW miner
    miner: PoWMiner,

    /// PoH generator
    poh_generator: Arc<Mutex<PoHGenerator>>,

    /// Network sender
    network_tx: mpsc::Sender<NetMessage>,

    /// Configuration
    config: ConsensusConfig,
}

impl<'a> BlockProducer<'a> {
    /// Create a new block producer
    pub fn new(
        chain_state: ChainState,
        block_store: Arc<BlockStore<'a>>,
        tx_store: Arc<TxStore<'a>>,
        state_store: Arc<StateStore<'a>>,
        mempool: Arc<Mempool>,
        poh_generator: Arc<Mutex<PoHGenerator>>,
        network_tx: mpsc::Sender<NetMessage>,
        config: ConsensusConfig,
    ) -> Self {
        // Create the miner
        let miner = PoWMiner::new(config.clone());

        Self {
            chain_state,
            block_store,
            tx_store,
            state_store,
            mempool,
            miner,
            poh_generator,
            network_tx,
            config,
        }
    }

    /// Create a block template
    pub fn create_block_template(&self) -> BlockTemplate {
        // Get pending transactions from the mempool
        let transactions = self.mempool.get_pending_transactions(
            self.config.max_transactions_per_block
        );

        // Keep the full transaction records
        let selected_transactions = transactions.clone();

        // Calculate the state root
        let state_root = match self.state_store.calculate_state_root(
            self.chain_state.height + 1,
            chrono::Utc::now().timestamp() as u64
        ) {
            Ok(root) => root.root_hash,
            Err(e) => {
                error!("Failed to calculate state root: {}", e);
                [0u8; 32] // Fallback to zeros
            }
        };

        // Create the block template
        BlockTemplate {
            height: self.chain_state.height + 1,
            prev_hash: self.chain_state.tip_hash,
            timestamp: chrono::Utc::now().timestamp() as u64,
            transactions: selected_transactions,
            state_root,
            tx_root: [0u8; 32], // Will be calculated when needed
            poh_seq: 0, // Will be set when we have access to the PoH generator
            poh_hash: [0u8; 32], // Will be calculated during mining
            target: Target::from_difficulty(self.chain_state.total_difficulty),
            total_difficulty: self.chain_state.total_difficulty as u128,
            miner: [0u8; 32], // Will be set by the miner
        }
    }

    /// Mine a new block
    pub async fn mine_block(&self) -> Option<Block> {
        // Create a block template
        let template = self.create_block_template();

        info!("Mining block at height {}", template.height);

        // Mine the block
        let result = self.miner.mine_block(template).await;

        match result {
            Some(mining_result) => {
                let block = mining_result.block;

                // Mark transactions as included
                for tx_id in &block.transactions {
                    self.mempool.mark_included(tx_id);
                }

                // Broadcast the block
                self.broadcast_block(&block).await;

                Some(block)
            }
            None => {
                warn!("Mining failed");
                None
            }
        }
    }

    /// Broadcast a block to the network
    async fn broadcast_block(&self, block: &Block) {
        // Create a network message
        let message = NetMessage::NewBlock(block.clone());

        // Send the message
        if let Err(e) = self.network_tx.send(message).await {
            error!("Failed to broadcast block: {}", e);
        }
    }

    /// Update the chain state
    pub fn update_chain_state(&mut self, new_state: ChainState) {
        self.chain_state = new_state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_block_template_creation() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());

        // Create the stores
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let tx_store = Arc::new(TxStore::new(&kv_store));
        let state_store = Arc::new(StateStore::new(&kv_store));

        // Create a mempool
        let mempool = Arc::new(Mempool::new(100));

        // Create a config
        let config = ConsensusConfig::default();

        // Create a PoH generator
        let poh_generator = Arc::new(PoHGenerator::new(&config));

        // Create a network channel
        let (network_tx, _network_rx) = mpsc::channel(100);

        // Create a chain state
        let chain_state = ChainState {
            height: 10,
            current_target: Target::from_difficulty(100),
            latest_hash: [10u8; 32],
            latest_timestamp: 100,
            latest_poh_sequence: 1000,
        };

        // Create a block producer
        let producer = BlockProducer::new(
            chain_state,
            block_store,
            tx_store,
            state_store,
            mempool.clone(),
            poh_generator,
            network_tx,
            config,
        );

        // Add some transactions to the mempool
        let tx1 = TransactionRecord {
            tx_id: [1u8; 32],
            sender: [2u8; 32],
            recipient: [3u8; 32],
            value: 100,
            gas_used: 10,
            block_height: 0,
        };

        let tx2 = TransactionRecord {
            tx_id: [2u8; 32],
            sender: [2u8; 32],
            recipient: [3u8; 32],
            value: 200,
            gas_used: 10,
            block_height: 0,
        };

        mempool.add_transaction(tx1);
        mempool.add_transaction(tx2);

        // Create a block template
        let template = producer.create_block_template();

        // Check the template
        assert_eq!(template.height, 11);
        assert_eq!(template.prev_hash, [10u8; 32]);
        assert_eq!(template.transactions.len(), 2);
        assert!(template.tx_root.is_none()); // tx_root is calculated when needed
    }
}
