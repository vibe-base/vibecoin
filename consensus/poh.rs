use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use std::thread;
use log::{debug, info, warn, error};
use sha2::{Sha256, Digest};
use tokio::sync::mpsc;
use tokio::time;

use crate::storage::block_store::Hash;
use crate::storage::poh_store::PoHStore;
use crate::consensus::types::ConsensusConfig;
use crate::crypto::hash::sha256;

/// PoH entry
#[derive(Debug, Clone)]
pub struct PoHEntry {
    /// Sequence number
    pub seq: u64,
    
    /// Hash
    pub hash: Hash,
    
    /// Timestamp
    pub timestamp: u64,
    
    /// Optional event data hash
    pub event_hash: Option<Hash>,
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
    
    /// PoH store for persistence
    poh_store: Option<Arc<PoHStore<'static>>>,
    
    /// Tick interval in milliseconds
    tick_interval: u64,
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
    
    /// Ticks since last event
    ticks_since_event: u64,
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
            ticks_since_event: 0,
        }));
        
        Self {
            state,
            config,
            running: Arc::new(Mutex::new(false)),
            entry_tx,
            poh_store: None,
            tick_interval: config.poh_tick_interval,
        }
    }
    
    /// Set the PoH store
    pub fn with_store(mut self, store: Arc<PoHStore<'static>>) -> Self {
        self.poh_store = Some(store);
        self
    }
    
    /// Start the PoH generator
    pub async fn start(&self) {
        // Set the running flag
        {
            let mut running = self.running.lock().unwrap();
            if *running {
                warn!("PoH generator is already running");
                return;
            }
            *running = true;
        }
        
        info!("Starting PoH generator with tick interval {} ms", self.tick_interval);
        
        // Clone the necessary state for the async task
        let state = self.state.clone();
        let running = self.running.clone();
        let entry_tx = self.entry_tx.clone();
        let poh_store = self.poh_store.clone();
        let tick_interval = self.tick_interval;
        
        // Spawn a task to generate PoH ticks
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(tick_interval));
            
            loop {
                // Check if we should stop
                if !*running.lock().unwrap() {
                    debug!("PoH generator stopping");
                    break;
                }
                
                // Wait for the next tick
                interval.tick().await;
                
                // Generate the next PoH hash
                let entry = {
                    let mut state_guard = state.write().unwrap();
                    
                    // Update the hash
                    state_guard.hash = Self::generate_next_hash(&state_guard.hash);
                    state_guard.seq += 1;
                    state_guard.timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    state_guard.ticks_since_event += 1;
                    
                    // Create a PoH entry
                    PoHEntry {
                        seq: state_guard.seq,
                        hash: state_guard.hash,
                        timestamp: state_guard.timestamp,
                        event_hash: None,
                    }
                };
                
                // Store the entry if we have a store
                if let Some(store) = &poh_store {
                    store.append_entry(&entry);
                }
                
                // Send the entry to the channel
                if let Err(e) = entry_tx.try_send(entry.clone()) {
                    if e.is_full() {
                        // Channel is full, this is not critical
                        debug!("PoH entry channel is full, dropping entry {}", entry.seq);
                    } else {
                        // Channel is closed, this is critical
                        error!("PoH entry channel is closed: {}", e);
                        break;
                    }
                }
            }
            
            info!("PoH generator stopped");
        });
    }
    
    /// Stop the PoH generator
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
        info!("Stopping PoH generator");
    }
    
    /// Record an event in the PoH sequence
    pub fn record_event(&self, data: &[u8]) -> PoHEntry {
        let mut state_guard = self.state.write().unwrap();
        
        // Hash the data
        let data_hash = sha256(data);
        
        // Hash the data with the current hash
        let combined = [&state_guard.hash[..], &data_hash[..]].concat();
        state_guard.hash = sha256(&combined);
        state_guard.seq += 1;
        state_guard.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        state_guard.ticks_since_event = 0;
        
        // Create a PoH entry
        let entry = PoHEntry {
            seq: state_guard.seq,
            hash: state_guard.hash,
            timestamp: state_guard.timestamp,
            event_hash: Some(data_hash),
        };
        
        // Store the entry if we have a store
        if let Some(store) = &self.poh_store {
            store.append_entry(&entry);
        }
        
        // Send the entry to the channel
        if let Err(e) = self.entry_tx.try_send(entry.clone()) {
            if e.is_full() {
                // Channel is full, this is not critical
                debug!("PoH entry channel is full, dropping entry {}", entry.seq);
            } else {
                // Channel is closed, this is critical
                error!("PoH entry channel is closed: {}", e);
            }
        }
        
        entry
    }
    
    /// Get the current PoH state
    pub fn get_state(&self) -> (u64, Hash, u64) {
        let state = self.state.read().unwrap();
        (state.seq, state.hash, state.timestamp)
    }
    
    /// Get the current sequence number
    pub fn current_seq(&self) -> u64 {
        self.state.read().unwrap().seq
    }
    
    /// Get the current hash
    pub fn current_hash(&self) -> Hash {
        self.state.read().unwrap().hash
    }
    
    /// Get the current timestamp
    pub fn current_timestamp(&self) -> u64 {
        self.state.read().unwrap().timestamp
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
