use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use log::{error, info, trace};

use crate::crypto::hash::sha256;
use crate::storage::poh_store::{PoHEntry, PoHStore};
use crate::consensus::config::ConsensusConfig;

/// Generator for Proof of History sequence
pub struct PoHGenerator {
    /// Current PoH state
    current_hash: [u8; 32],

    /// Current sequence number
    sequence: u64,

    /// PoH tick rate (hashes per second)
    tick_rate: u64,

    /// PoH store for persistence
    poh_store: Option<Arc<PoHStore<'static>>>,

    /// Channel for new PoH entries
    entry_tx: Option<mpsc::Sender<PoHEntry>>,

    /// Whether the generator is running
    running: Arc<Mutex<bool>>,
}

impl PoHGenerator {
    /// Create a new PoH generator
    pub fn new(config: &ConsensusConfig) -> Self {
        // Start with a zero hash
        let current_hash = [0u8; 32];

        Self {
            current_hash,
            sequence: 0,
            tick_rate: config.poh_tick_rate,
            poh_store: None,
            entry_tx: None,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Set the PoH store for persistence
    pub fn with_store(mut self, store: Arc<PoHStore<'static>>) -> Self {
        self.poh_store = Some(store);
        self
    }

    /// Set the channel for new PoH entries
    pub fn with_channel(mut self, tx: mpsc::Sender<PoHEntry>) -> Self {
        self.entry_tx = Some(tx);
        self
    }

    /// Start the PoH generator
    pub async fn start(&mut self) {
        let mut running = self.running.lock().unwrap();
        if *running {
            return;
        }

        *running = true;
        drop(running);

        let running_clone = self.running.clone();
        let tick_rate = self.tick_rate;
        let mut current_hash = self.current_hash;
        let mut sequence = self.sequence;
        let poh_store = self.poh_store.clone();
        let entry_tx = self.entry_tx.clone();

        // Spawn a task to generate PoH entries
        tokio::spawn(async move {
            let tick_interval = Duration::from_secs(1) / tick_rate as u32;
            let mut interval = time::interval(tick_interval);

            info!("Starting PoH generator with tick rate {} Hz", tick_rate);

            loop {
                // Check if we should stop
                if !*running_clone.lock().unwrap() {
                    break;
                }

                // Wait for the next tick
                interval.tick().await;

                // Generate the next hash
                current_hash = sha256(&current_hash);
                sequence += 1;

                // Create a PoH entry
                let entry = PoHEntry {
                    hash: current_hash,
                    sequence,
                    timestamp: chrono::Utc::now().timestamp() as u64,
                };

                // Store the entry if we have a store
                if let Some(store) = &poh_store {
                    store.append_entry(&entry);
                }

                // Send the entry if we have a channel
                if let Some(tx) = &entry_tx {
                    if let Err(e) = tx.try_send(entry.clone()) {
                        if matches!(e, tokio::sync::mpsc::error::TrySendError::Full(_)) {
                            // Channel is full, this is not critical
                            trace!("PoH entry channel is full, dropping entry");
                        } else {
                            // Channel is closed, this is more serious
                            error!("Failed to send PoH entry: {}", e);
                            break;
                        }
                    }
                }

                trace!("PoH sequence {}: {:?}", sequence, current_hash);
            }

            info!("PoH generator stopped");
        });
    }

    /// Stop the PoH generator
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    /// Get the current PoH hash
    pub fn current_hash(&self) -> [u8; 32] {
        self.current_hash
    }

    /// Get the current sequence number
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Verify a PoH sequence
    pub fn verify_sequence(start_hash: &[u8; 32], end_hash: &[u8; 32], steps: u64) -> bool {
        let mut hash = *start_hash;

        // Perform the specified number of hash operations
        for _ in 0..steps {
            hash = sha256(&hash);
        }

        // Check if the resulting hash matches the expected end hash
        &hash == end_hash
    }

    /// Record an event in the PoH sequence
    pub fn record_event(&mut self, data: &[u8]) -> PoHEntry {
        // Hash the data with the current hash
        let combined = [&self.current_hash[..], data].concat();
        self.current_hash = sha256(&combined);
        self.sequence += 1;

        // Create a PoH entry
        let entry = PoHEntry {
            hash: self.current_hash,
            sequence: self.sequence,
            timestamp: chrono::Utc::now().timestamp() as u64,
        };

        // Store the entry if we have a store
        if let Some(store) = &self.poh_store {
            store.append_entry(&entry);
        }

        // Send the entry if we have a channel
        if let Some(tx) = &self.entry_tx {
            if let Err(e) = tx.try_send(entry.clone()) {
                match e {
                    tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    // Channel is full, this is not critical
                    trace!("PoH entry channel is full, dropping entry");
                    },
                    tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    // Channel is closed, this is more serious
                    error!("Failed to send PoH entry: {}", e);
                    }
                }
            }
        }

        entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_poh_generator() {
        // Create a config with a low tick rate for testing
        let config = ConsensusConfig::default()
            .with_poh_tick_rate(10); // 10 Hz

        // Create a channel for PoH entries
        let (tx, mut rx) = mpsc::channel(100);

        // Create a PoH generator
        let mut generator = PoHGenerator::new(&config)
            .with_channel(tx);

        // Start the generator
        generator.start().await;

        // Wait for some entries
        let mut entries = Vec::new();
        for _ in 0..5 {
            if let Some(entry) = rx.recv().await {
                entries.push(entry);
            }
        }

        // Stop the generator
        generator.stop();

        // Check that we got some entries
        assert!(!entries.is_empty());

        // Check that the sequence numbers are increasing
        for i in 1..entries.len() {
            assert!(entries[i].sequence > entries[i-1].sequence);
        }
    }

    #[test]
    fn test_record_event() {
        // Create a config
        let config = ConsensusConfig::default();

        // Create a PoH generator
        let mut generator = PoHGenerator::new(&config);

        // Record some events
        let event1 = b"event1";
        let event2 = b"event2";

        let entry1 = generator.record_event(event1);
        let entry2 = generator.record_event(event2);

        // Check that the sequence numbers are increasing
        assert_eq!(entry1.sequence, 1);
        assert_eq!(entry2.sequence, 2);

        // Check that the hashes are different
        assert_ne!(entry1.hash, entry2.hash);
    }
}
