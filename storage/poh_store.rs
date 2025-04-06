use serde::{Serialize, Deserialize};
use crate::storage::kv_store::KVStore;

/// Proof of History entry structure
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PoHEntry {
    pub hash: [u8; 32],
    pub sequence: u64,
    pub timestamp: u64,
}

/// Store for Proof of History entries
pub struct PoHStore<'a> {
    store: &'a dyn KVStore,
}

impl<'a> PoHStore<'a> {
    /// Create a new PoHStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }

    /// Append a new PoH entry
    pub fn append_entry(&self, entry: &PoHEntry) {
        let key = format!("poh:seq:{}", entry.sequence);
        let value = bincode::serialize(entry).unwrap();
        self.store.put(key.as_bytes(), &value);
        
        // Also index by hash for verification
        let hash_key = format!("poh:hash:{:x?}", entry.hash);
        self.store.put(hash_key.as_bytes(), &value);
    }

    /// Get a PoH entry by sequence number
    pub fn get_entry(&self, sequence: u64) -> Option<PoHEntry> {
        let key = format!("poh:seq:{}", sequence);
        self.store.get(key.as_bytes())
            .and_then(|bytes| bincode::deserialize(&bytes).ok())
    }
    
    /// Get a PoH entry by hash
    pub fn get_entry_by_hash(&self, hash: &[u8]) -> Option<PoHEntry> {
        let key = format!("poh:hash:{:x?}", hash);
        self.store.get(key.as_bytes())
            .and_then(|bytes| bincode::deserialize(&bytes).ok())
    }
    
    /// Get the latest sequence number
    pub fn get_latest_sequence(&self) -> Option<u64> {
        let prefix = b"poh:seq:";
        let entries = self.store.scan_prefix(prefix);
        
        entries.iter()
            .map(|(key, _)| {
                let key_str = std::str::from_utf8(key).unwrap();
                let seq_str = key_str.strip_prefix("poh:seq:").unwrap();
                seq_str.parse::<u64>().unwrap()
            })
            .max()
    }
    
    /// Get a range of PoH entries
    pub fn get_entry_range(&self, start_seq: u64, end_seq: u64) -> Vec<PoHEntry> {
        let mut entries = Vec::new();
        for seq in start_seq..=end_seq {
            if let Some(entry) = self.get_entry(seq) {
                entries.push(entry);
            }
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[test]
    fn test_poh_store() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let poh_store = PoHStore::new(&kv_store);
        
        // Create test entries
        let entry1 = PoHEntry {
            hash: [1; 32],
            sequence: 1,
            timestamp: 12345,
        };
        
        let entry2 = PoHEntry {
            hash: [2; 32],
            sequence: 2,
            timestamp: 12346,
        };
        
        // Store the entries
        poh_store.append_entry(&entry1);
        poh_store.append_entry(&entry2);
        
        // Retrieve by sequence
        let retrieved = poh_store.get_entry(1).unwrap();
        assert_eq!(retrieved, entry1);
        
        // Retrieve by hash
        let retrieved = poh_store.get_entry_by_hash(&[2; 32]).unwrap();
        assert_eq!(retrieved, entry2);
        
        // Test latest sequence
        assert_eq!(poh_store.get_latest_sequence(), Some(2));
        
        // Test range retrieval
        let range = poh_store.get_entry_range(1, 2);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0], entry1);
        assert_eq!(range[1], entry2);
    }
}
