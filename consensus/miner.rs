use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use log::{debug, info, warn, error};
use sha2::{Sha256, Digest};
use rand::Rng;
use rayon::prelude::*;

use crate::storage::block_store::{Block, BlockStore, Hash};
use crate::storage::state_store::{StateStore, StateRoot};
use crate::storage::tx_store::TxStore;
use crate::mempool::Mempool;
use crate::consensus::types::{BlockHeader, Target, ConsensusConfig};
use crate::consensus::poh::PoHGenerator;

/// Mining result
#[derive(Debug, Clone)]
pub struct MiningResult {
    /// Block header
    pub header: BlockHeader,
    
    /// Transactions
    pub transactions: Vec<Hash>,
    
    /// Nonce
    pub nonce: u64,
    
    /// Block hash
    pub hash: Hash,
}

/// Block miner
pub struct BlockMiner<'a> {
    /// Block store
    block_store: &'a BlockStore<'a>,
    
    /// State store
    state_store: &'a StateStore<'a>,
    
    /// Transaction store
    tx_store: &'a TxStore<'a>,
    
    /// Mempool
    mempool: &'a Mempool,
    
    /// Configuration
    config: ConsensusConfig,
    
    /// Running flag
    running: Arc<Mutex<bool>>,
    
    /// Current mining task
    current_task: Arc<RwLock<Option<MiningTask>>>,
}

/// Mining task
#[derive(Debug, Clone)]
struct MiningTask {
    /// Block header
    header: BlockHeader,
    
    /// Transactions
    transactions: Vec<Hash>,
    
    /// Target
    target: Target,
    
    /// Start nonce
    start_nonce: u64,
    
    /// End nonce
    end_nonce: u64,
}

impl<'a> BlockMiner<'a> {
    /// Create a new block miner
    pub fn new(
        block_store: &'a BlockStore<'a>,
        state_store: &'a StateStore<'a>,
        tx_store: &'a TxStore<'a>,
        mempool: &'a Mempool,
        config: ConsensusConfig,
    ) -> Self {
        Self {
            block_store,
            state_store,
            tx_store,
            mempool,
            config,
            running: Arc::new(Mutex::new(false)),
            current_task: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Start mining
    pub fn start_mining(&self) -> Result<(), String> {
        let mut running = self.running.lock().unwrap();
        if *running {
            return Err("Mining already running".to_string());
        }
        
        *running = true;
        
        // Determine the number of mining threads
        let thread_count = if self.config.mining_threads == 0 {
            num_cpus::get()
        } else {
            self.config.mining_threads
        };
        
        info!("Starting mining with {} threads", thread_count);
        
        // Spawn mining threads
        for thread_id in 0..thread_count {
            let running = self.running.clone();
            let current_task = self.current_task.clone();
            
            thread::spawn(move || {
                info!("Mining thread {} started", thread_id);
                
                let mut rng = rand::thread_rng();
                
                while *running.lock().unwrap() {
                    // Get the current task
                    let task = {
                        let task_guard = current_task.read().unwrap();
                        match &*task_guard {
                            Some(task) => task.clone(),
                            None => {
                                // No task, sleep and try again
                                thread::sleep(Duration::from_millis(100));
                                continue;
                            }
                        }
                    };
                    
                    // Mine the block
                    let result = Self::mine_block_task(&task, thread_id);
                    
                    // Check if we found a solution
                    if let Some(result) = result {
                        info!("Thread {} found a solution with nonce {}", thread_id, result.nonce);
                        
                        // Clear the current task
                        let mut task_guard = current_task.write().unwrap();
                        *task_guard = None;
                        
                        // TODO: Submit the solution to the consensus engine
                    }
                    
                    // Sleep a bit to avoid busy-waiting
                    thread::sleep(Duration::from_millis(10));
                }
                
                info!("Mining thread {} stopped", thread_id);
            });
        }
        
        Ok(())
    }
    
    /// Stop mining
    pub fn stop_mining(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }
    
    /// Set the current mining task
    pub fn set_mining_task(
        &self,
        header: BlockHeader,
        transactions: Vec<Hash>,
        target: Target,
    ) {
        let mut task_guard = self.current_task.write().unwrap();
        
        // Create a new task
        let task = MiningTask {
            header,
            transactions,
            target,
            start_nonce: 0,
            end_nonce: u64::MAX,
        };
        
        *task_guard = Some(task);
    }
    
    /// Mine a block task
    fn mine_block_task(task: &MiningTask, thread_id: usize) -> Option<MiningResult> {
        let mut header = task.header.clone();
        let mut rng = rand::thread_rng();
        
        // Start with a random nonce in our range
        let mut nonce = rng.gen_range(task.start_nonce..task.end_nonce);
        
        // Try a batch of nonces
        let batch_size = 1000;
        let mut nonces = Vec::with_capacity(batch_size);
        
        for _ in 0..batch_size {
            nonces.push(nonce);
            nonce = nonce.wrapping_add(1);
            if nonce >= task.end_nonce {
                nonce = task.start_nonce;
            }
        }
        
        // Try each nonce in parallel
        let result = nonces.par_iter().find_map_first(|&n| {
            let mut header_copy = header.clone();
            header_copy.nonce = n;
            
            let hash = header_copy.hash();
            
            if task.target.is_met_by(&hash) {
                Some(MiningResult {
                    header: header_copy,
                    transactions: task.transactions.clone(),
                    nonce: n,
                    hash,
                })
            } else {
                None
            }
        });
        
        result
    }
    
    /// Create a block template
    pub fn create_block_template(
        &self,
        prev_block: &Block,
        poh_hash: Hash,
        poh_seq: u64,
        difficulty: u64,
    ) -> Result<(BlockHeader, Vec<Hash>), String> {
        // Get transactions from the mempool
        let tx_hashes = self.mempool.get_transactions(self.config.max_transactions_per_block);
        
        // Calculate the transaction root
        let tx_root = self.calculate_tx_root(&tx_hashes);
        
        // Calculate the state root
        let state_root = match self.state_store.calculate_state_root(
            prev_block.height + 1,
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        ) {
            Ok(root) => root.root_hash,
            Err(e) => return Err(format!("Failed to calculate state root: {}", e)),
        };
        
        // Create the block header
        let header = BlockHeader {
            height: prev_block.height + 1,
            prev_hash: prev_block.hash,
            state_root,
            tx_root,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            poh_seq,
            poh_hash,
            nonce: 0, // Will be set during mining
            difficulty,
            total_difficulty: prev_block.total_difficulty + difficulty as u128,
        };
        
        Ok((header, tx_hashes))
    }
    
    /// Calculate the transaction root
    fn calculate_tx_root(&self, tx_hashes: &[Hash]) -> Hash {
        // Simple Merkle root calculation
        if tx_hashes.is_empty() {
            return [0u8; 32];
        }
        
        let mut hashes = tx_hashes.to_vec();
        
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in hashes.chunks(2) {
                let mut hasher = Sha256::new();
                
                hasher.update(&chunk[0]);
                
                if chunk.len() > 1 {
                    hasher.update(&chunk[1]);
                } else {
                    // If odd number of hashes, duplicate the last one
                    hasher.update(&chunk[0]);
                }
                
                let result = hasher.finalize();
                
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&result);
                
                next_level.push(hash);
            }
            
            hashes = next_level;
        }
        
        hashes[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    
    #[test]
    fn test_tx_root_calculation() {
        // Create a miner
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);
        let state_store = StateStore::new(&kv_store);
        let tx_store = TxStore::new(&kv_store);
        let mempool = Mempool::new();
        let config = ConsensusConfig::default();
        
        let miner = BlockMiner::new(&block_store, &state_store, &tx_store, &mempool, config);
        
        // Test with empty transactions
        let empty_root = miner.calculate_tx_root(&[]);
        assert_eq!(empty_root, [0u8; 32]);
        
        // Test with one transaction
        let tx1 = [1u8; 32];
        let root1 = miner.calculate_tx_root(&[tx1]);
        
        // Test with two transactions
        let tx2 = [2u8; 32];
        let root2 = miner.calculate_tx_root(&[tx1, tx2]);
        
        // The roots should be different
        assert_ne!(root1, empty_root);
        assert_ne!(root2, root1);
    }
}
