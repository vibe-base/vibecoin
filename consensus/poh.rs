use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::thread;
use log::{debug, info, warn, error};
use sha2::{Sha256, Digest};
use tokio::sync::mpsc;

use crate::storage::block_store::Hash;
use crate::consensus::types::ConsensusConfig;

/// PoH entry
#[derive(Debug, Clone)]
pub struct PoHEntry {
    /// Sequence number
    pub seq: u64,
    
    /// Hash
    pub hash: Hash,
    
    /// Timestamp
    pub timestamp: u64,
}

/// PoH generator
pub struct PoHGenerator {
    /// Current PoH state
    state: Arc<RwLock<PoHState>>,
    
    /// Configuration
    config: ConsensusConfig,
    
    /// Running flag
    running: Arc<Mutex<bool>>,
    
    /// PoH entry channel
    entry_tx: mpsc::Sender<PoHEntry>,
}

/// PoH state
#[derive(Debug, Clone)]
struct PoHState {
    /// Current sequence number
    seq: u64,
    
    /// Current hash
    hash: Hash,
    
    /// Last update timestamp
    timestamp: u64,
}

impl PoHGenerator {
    /// Create a new PoH generator
    pub fn new(config: ConsensusConfig, entry_tx: mpsc::Sender<PoHEntry>) -> Self {
        // Initialize with a zero hash
        let initial_hash = [0u8; 32];
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let state = Arc::new(RwLock::new(PoHState {
            seq: 0,
            hash: initial_hash,
            timestamp,
        }));
        
        Self {
            state,
            config,
            running: Arc::new(Mutex::new(false)),
            entry_tx,
        }
    }
    
    /// Start the PoH generator
    pub fn start(&self) -> Result<(), String> {
        let mut running = self.running.lock().unwrap();
        if *running {
            return Err("PoH generator already running".to_string());
        }
        
        *running = true;
        
        let state = self.state.clone();
        let running_flag = self.running.clone();
        let tick_interval = self.config.poh_tick_interval;
        let entry_tx = self.entry_tx.clone();
        
        // Spawn a thread to generate PoH entries
        thread::spawn(move || {
            info!("PoH generator started");
            
            let tick_duration = Duration::from_millis(tick_interval);
            let mut last_tick = Instant::now();
            
            while *running_flag.lock().unwrap() {
                // Sleep until the next tick
                let now = Instant::now();
                let elapsed = now.duration_since(last_tick);
                
                if elapsed < tick_duration {
                    thread::sleep(tick_duration - elapsed);
                }
                
                last_tick = Instant::now();
                
                // Generate the next PoH entry
                let entry = {
                    let mut state = state.write().unwrap();
                    
                    // Update the hash
                    let mut hasher = Sha256::new();
                    hasher.update(&state.hash);
                    let result = hasher.finalize();
                    
                    let mut new_hash = [0u8; 32];
                    new_hash.copy_from_slice(&result);
                    
                    // Update the state
                    state.seq += 1;
                    state.hash = new_hash;
                    state.timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    
                    // Create the entry
                    PoHEntry {
                        seq: state.seq,
                        hash: state.hash,
                        timestamp: state.timestamp,
                    }
                };
                
                // Send the entry
                if let Err(e) = entry_tx.try_send(entry) {
                    if e.is_full() {
                        warn!("PoH entry channel is full, dropping entry");
                    } else if e.is_closed() {
                        error!("PoH entry channel is closed, stopping generator");
                        break;
                    }
                }
            }
            
            info!("PoH generator stopped");
        });
        
        Ok(())
    }
    
    /// Stop the PoH generator
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }
    
    /// Get the current PoH state
    pub fn get_current_state(&self) -> (u64, Hash) {
        let state = self.state.read().unwrap();
        (state.seq, state.hash)
    }
    
    /// Generate the next PoH hash
    pub fn generate_next_hash(prev_hash: &Hash) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(prev_hash);
        let result = hasher.finalize();
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Generate a sequence of PoH hashes
    pub fn generate_hash_sequence(prev_hash: &Hash, steps: u64) -> Hash {
        let mut hash = *prev_hash;
        
        for _ in 0..steps {
            hash = Self::generate_next_hash(&hash);
        }
        
        hash
    }
    
    /// Verify a PoH sequence
    pub fn verify_sequence(start_hash: &Hash, end_hash: &Hash, steps: u64) -> bool {
        let computed_hash = Self::generate_hash_sequence(start_hash, steps);
        computed_hash == *end_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    
    #[test]
    fn test_poh_hash_generation() {
        let initial_hash = [0u8; 32];
        
        // Generate the next hash
        let next_hash = PoHGenerator::generate_next_hash(&initial_hash);
        
        // Hash should not be all zeros
        assert_ne!(next_hash, [0u8; 32]);
        
        // Generate a sequence of hashes
        let seq_hash = PoHGenerator::generate_hash_sequence(&initial_hash, 10);
        
        // Verify the sequence
        assert!(PoHGenerator::verify_sequence(&initial_hash, &seq_hash, 10));
        
        // Verify with wrong steps should fail
        assert!(!PoHGenerator::verify_sequence(&initial_hash, &seq_hash, 9));
        assert!(!PoHGenerator::verify_sequence(&initial_hash, &seq_hash, 11));
    }
    
    #[tokio::test]
    async fn test_poh_generator() {
        let (tx, mut rx) = mpsc::channel(100);
        let config = ConsensusConfig::default();
        
        let generator = PoHGenerator::new(config, tx);
        
        // Start the generator
        generator.start().unwrap();
        
        // Wait for a few entries
        let mut entries = Vec::new();
        for _ in 0..5 {
            if let Some(entry) = rx.recv().await {
                entries.push(entry);
            }
        }
        
        // Stop the generator
        generator.stop();
        
        // Check that we received some entries
        assert!(!entries.is_empty());
        
        // Check that the sequence numbers are increasing
        for i in 1..entries.len() {
            assert!(entries[i].seq > entries[i-1].seq);
        }
    }
}
