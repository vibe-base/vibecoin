use rocksdb::{DB, Options};
use std::path::Path;

/// Key-Value Store trait defining the interface for storage operations
pub trait KVStore {
    /// Store a key-value pair
    fn put(&self, key: &[u8], value: &[u8]);
    
    /// Retrieve a value by key
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    
    /// Delete a key-value pair
    fn delete(&self, key: &[u8]);
    
    /// Scan all key-value pairs with a given prefix
    fn scan_prefix(&self, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)>;
}

/// RocksDB implementation of the KVStore trait
pub struct RocksDBStore {
    db: DB,
}

impl RocksDBStore {
    /// Create a new RocksDBStore at the specified path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path).unwrap();
        Self { db }
    }
}

impl KVStore for RocksDBStore {
    fn put(&self, key: &[u8], value: &[u8]) {
        self.db.put(key, value).unwrap();
    }

    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.db.get(key).unwrap()
    }

    fn delete(&self, key: &[u8]) {
        self.db.delete(key).unwrap();
    }

    fn scan_prefix(&self, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        self.db.prefix_iterator(prefix)
            .map(|(k, v)| (k.to_vec(), v.to_vec()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_rocksdb_store() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path());
        
        // Test put and get
        let key = b"test_key";
        let value = b"test_value";
        store.put(key, value);
        
        let retrieved = store.get(key).unwrap();
        assert_eq!(retrieved, value);
        
        // Test delete
        store.delete(key);
        assert!(store.get(key).is_none());
        
        // Test scan_prefix
        store.put(b"prefix1:a", b"value1");
        store.put(b"prefix1:b", b"value2");
        store.put(b"prefix2:c", b"value3");
        
        let results = store.scan_prefix(b"prefix1:");
        assert_eq!(results.len(), 2);
    }
}
