use log::{debug, error, info, warn};

use crate::crypto::hash::{sha256, Hash};
use crate::storage::poh_store::{PoHEntry, PoHStore};

/// Verifier for Proof of History sequence
pub struct PoHVerifier {
    /// PoH store for retrieving entries
    poh_store: Option<PoHStore<'static>>,
}

impl PoHVerifier {
    /// Create a new PoH verifier
    pub fn new() -> Self {
        Self {
            poh_store: None,
        }
    }
    
    /// Set the PoH store
    pub fn with_store(mut self, store: PoHStore<'static>) -> Self {
        self.poh_store = Some(store);
        self
    }
    
    /// Verify a single PoH entry
    pub fn verify_entry(&self, entry: &PoHEntry, prev_hash: &[u8; 32]) -> bool {
        // For a valid PoH entry, entry.hash should be the SHA-256 hash of prev_hash
        let expected_hash = sha256(prev_hash);
        
        if entry.hash != expected_hash {
            warn!("Invalid PoH entry: hash mismatch at sequence {}", entry.sequence);
            return false;
        }
        
        true
    }
    
    /// Verify a range of PoH entries
    pub fn verify_range(&self, start_seq: u64, end_seq: u64) -> bool {
        // We need a store to verify a range
        let store = match &self.poh_store {
            Some(store) => store,
            None => {
                error!("Cannot verify PoH range without a store");
                return false;
            }
        };
        
        // Get the starting entry
        let start_entry = match store.get_entry(start_seq) {
            Some(entry) => entry,
            None => {
                error!("Cannot find PoH entry at sequence {}", start_seq);
                return false;
            }
        };
        
        // Verify each entry in the range
        let mut prev_hash = start_entry.hash;
        
        for seq in (start_seq + 1)..=end_seq {
            let entry = match store.get_entry(seq) {
                Some(entry) => entry,
                None => {
                    error!("Missing PoH entry at sequence {}", seq);
                    return false;
                }
            };
            
            if !self.verify_entry(&entry, &prev_hash) {
                return false;
            }
            
            prev_hash = entry.hash;
        }
        
        true
    }
    
    /// Verify a PoH entry with an embedded event
    pub fn verify_event(&self, entry: &PoHEntry, prev_hash: &[u8; 32], event_data: &[u8]) -> bool {
        // For a valid PoH entry with an event, entry.hash should be the SHA-256 hash of
        // the concatenation of prev_hash and event_data
        let combined = [prev_hash, event_data].concat();
        let expected_hash = sha256(&combined);
        
        if entry.hash != expected_hash {
            warn!("Invalid PoH event entry: hash mismatch at sequence {}", entry.sequence);
            return false;
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::poh::generator::PoHGenerator;
    use crate::consensus::config::ConsensusConfig;
    
    #[test]
    fn test_verify_entry() {
        // Create a verifier
        let verifier = PoHVerifier::new();
        
        // Create a valid entry
        let prev_hash = [0u8; 32];
        let expected_hash = sha256(&prev_hash);
        
        let valid_entry = PoHEntry {
            hash: expected_hash,
            sequence: 1,
            timestamp: 12345,
        };
        
        // Verify the valid entry
        assert!(verifier.verify_entry(&valid_entry, &prev_hash));
        
        // Create an invalid entry
        let invalid_entry = PoHEntry {
            hash: [1u8; 32], // Wrong hash
            sequence: 1,
            timestamp: 12345,
        };
        
        // Verify the invalid entry
        assert!(!verifier.verify_entry(&invalid_entry, &prev_hash));
    }
    
    #[test]
    fn test_verify_event() {
        // Create a verifier
        let verifier = PoHVerifier::new();
        
        // Create a valid event entry
        let prev_hash = [0u8; 32];
        let event_data = b"test event";
        
        let combined = [&prev_hash[..], &event_data[..]].concat();
        let expected_hash = sha256(&combined);
        
        let valid_entry = PoHEntry {
            hash: expected_hash,
            sequence: 1,
            timestamp: 12345,
        };
        
        // Verify the valid entry
        assert!(verifier.verify_event(&valid_entry, &prev_hash, event_data));
        
        // Create an invalid entry
        let invalid_entry = PoHEntry {
            hash: [1u8; 32], // Wrong hash
            sequence: 1,
            timestamp: 12345,
        };
        
        // Verify the invalid entry
        assert!(!verifier.verify_event(&invalid_entry, &prev_hash, event_data));
    }
    
    #[test]
    fn test_generator_and_verifier() {
        // Create a config
        let config = ConsensusConfig::default();
        
        // Create a generator
        let mut generator = PoHGenerator::new(&config);
        
        // Generate some entries
        let entry1 = generator.record_event(b"event1");
        let hash1 = generator.current_hash();
        
        let entry2 = generator.record_event(b"event2");
        
        // Create a verifier
        let verifier = PoHVerifier::new();
        
        // Verify the entries
        assert!(verifier.verify_event(&entry1, &[0u8; 32], b"event1"));
        assert!(verifier.verify_event(&entry2, &hash1, b"event2"));
    }
}
