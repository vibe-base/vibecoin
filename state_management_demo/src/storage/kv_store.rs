use rocksdb::{DB, Options, WriteBatch, WriteOptions, ColumnFamily, IteratorMode};
use std::path::Path;
use std::sync::Arc;
use std::io::{Error, ErrorKind};
use std::fmt;
use log::{debug, error, info, warn};

/// Custom error type for KVStore operations
#[derive(Debug)]
pub enum KVStoreError {
    /// IO error
    IoError(Error),
    /// RocksDB error
    RocksDBError(String),
    /// Serialization error
    SerializationError(String),
    /// Deserialization error
    DeserializationError(String),
    /// Key not found
    KeyNotFound(String),
    /// Invalid data format
    InvalidDataFormat(String),
    /// Batch operation failed
    BatchOperationFailed(String),
    /// Database already exists
    DatabaseAlreadyExists(String),
    /// Database not found
    DatabaseNotFound(String),
    /// Column family not found
    ColumnFamilyNotFound(String),
    /// Other error
    Other(String),
}

impl fmt::Display for KVStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KVStoreError::IoError(err) => write!(f, "IO error: {}", err),
            KVStoreError::RocksDBError(err) => write!(f, "RocksDB error: {}", err),
            KVStoreError::SerializationError(err) => write!(f, "Serialization error: {}", err),
            KVStoreError::DeserializationError(err) => write!(f, "Deserialization error: {}", err),
            KVStoreError::KeyNotFound(key) => write!(f, "Key not found: {}", key),
            KVStoreError::InvalidDataFormat(msg) => write!(f, "Invalid data format: {}", msg),
            KVStoreError::BatchOperationFailed(msg) => write!(f, "Batch operation failed: {}", msg),
            KVStoreError::DatabaseAlreadyExists(path) => write!(f, "Database already exists: {}", path),
            KVStoreError::DatabaseNotFound(path) => write!(f, "Database not found: {}", path),
            KVStoreError::ColumnFamilyNotFound(name) => write!(f, "Column family not found: {}", name),
            KVStoreError::Other(err) => write!(f, "Other error: {}", err),
        }
    }
}

impl std::error::Error for KVStoreError {}

impl From<Error> for KVStoreError {
    fn from(err: Error) -> Self {
        KVStoreError::IoError(err)
    }
}

impl From<rocksdb::Error> for KVStoreError {
    fn from(err: rocksdb::Error) -> Self {
        KVStoreError::RocksDBError(err.to_string())
    }
}

impl From<bincode::Error> for KVStoreError {
    fn from(err: bincode::Error) -> Self {
        KVStoreError::SerializationError(err.to_string())
    }
}

/// Write batch operation for atomic updates
#[derive(Debug, Clone)]
pub enum WriteBatchOperation {
    /// Put operation
    Put { key: Vec<u8>, value: Vec<u8> },
    /// Delete operation
    Delete { key: Vec<u8> },
}

/// Key-value store trait
pub trait KVStore: Send + Sync {
    /// Put a key-value pair
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError>;

    /// Get a value by key
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError>;

    /// Delete a key-value pair
    fn delete(&self, key: &[u8]) -> Result<(), KVStoreError>;

    /// Check if a key exists
    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError>;

    /// Write a batch of operations atomically
    fn write_batch(&self, operations: Vec<WriteBatchOperation>) -> Result<(), KVStoreError>;

    /// Iterate over key-value pairs with a prefix
    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError>;

    /// Flush any pending writes to disk
    fn flush(&self) -> Result<(), KVStoreError>;
}

/// RocksDB implementation of KVStore
pub struct RocksDBStore {
    /// RocksDB instance
    db: DB,
}

impl RocksDBStore {
    /// Create a new RocksDBStore
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, KVStoreError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_keep_log_file_num(10);
        opts.set_max_total_wal_size(64 * 1024 * 1024); // 64MB
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        opts.set_max_write_buffer_number(3);
        opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB
        opts.set_level_zero_file_num_compaction_trigger(8);
        opts.set_level_zero_slowdown_writes_trigger(17);
        opts.set_level_zero_stop_writes_trigger(24);
        opts.set_num_levels(7);
        opts.set_max_bytes_for_level_base(512 * 1024 * 1024); // 512MB
        opts.set_max_bytes_for_level_multiplier(10.0);

        let db = DB::open(&opts, path)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to open database: {}", e)))?;

        Ok(Self { db })
    }

    /// Create a new RocksDBStore with custom options
    pub fn with_options<P: AsRef<Path>>(path: P, options: Options) -> Result<Self, KVStoreError> {
        let db = DB::open(&options, path)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to open database: {}", e)))?;

        Ok(Self { db })
    }

    /// Get the underlying RocksDB instance
    pub fn get_db(&self) -> &DB {
        &self.db
    }
}

impl KVStore for RocksDBStore {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        self.db.put(key, value)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to put key: {}", e)))
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        self.db.get(key)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to get key: {}", e)))
    }

    fn delete(&self, key: &[u8]) -> Result<(), KVStoreError> {
        self.db.delete(key)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to delete key: {}", e)))
    }

    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError> {
        match self.db.get(key)? {
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    fn write_batch(&self, operations: Vec<WriteBatchOperation>) -> Result<(), KVStoreError> {
        let mut batch = WriteBatch::default();

        for op in operations {
            match op {
                WriteBatchOperation::Put { key, value } => {
                    batch.put(&key, &value);
                },
                WriteBatchOperation::Delete { key } => {
                    batch.delete(&key);
                },
            }
        }

        self.db.write(batch)
            .map_err(|e| KVStoreError::BatchOperationFailed(format!("Failed to write batch: {}", e)))
    }

    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError> {
        let mut results = Vec::new();
        let iterator = self.db.iterator(IteratorMode::From(prefix, rocksdb::Direction::Forward));

        for item in iterator {
            let (key, value) = item
                .map_err(|e| KVStoreError::RocksDBError(format!("Failed to iterate: {}", e)))?;

            // Check if the key starts with the prefix
            if key.starts_with(prefix) {
                results.push((key.to_vec(), value.to_vec()));
            } else {
                // We've moved past the prefix, stop iterating
                break;
            }
        }

        Ok(results)
    }

    fn flush(&self) -> Result<(), KVStoreError> {
        let mut options = WriteOptions::default();
        options.set_sync(true);
        
        // Create an empty batch to force a sync
        let batch = WriteBatch::default();
        self.db.write_opt(batch, &options)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to flush: {}", e)))
    }
}

/// In-memory implementation of KVStore for testing
#[cfg(test)]
pub struct MemoryStore {
    /// In-memory storage
    data: std::sync::RwLock<std::collections::HashMap<Vec<u8>, Vec<u8>>>,
}

#[cfg(test)]
impl MemoryStore {
    /// Create a new MemoryStore
    pub fn new() -> Self {
        Self {
            data: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

#[cfg(test)]
impl KVStore for MemoryStore {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        let data = self.data.read().unwrap();
        Ok(data.get(key).cloned())
    }

    fn delete(&self, key: &[u8]) -> Result<(), KVStoreError> {
        let mut data = self.data.write().unwrap();
        data.remove(key);
        Ok(())
    }

    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(key))
    }

    fn write_batch(&self, operations: Vec<WriteBatchOperation>) -> Result<(), KVStoreError> {
        let mut data = self.data.write().unwrap();
        
        for op in operations {
            match op {
                WriteBatchOperation::Put { key, value } => {
                    data.insert(key, value);
                },
                WriteBatchOperation::Delete { key } => {
                    data.remove(&key);
                },
            }
        }
        
        Ok(())
    }

    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError> {
        let data = self.data.read().unwrap();
        let mut results = Vec::new();
        
        for (key, value) in data.iter() {
            if key.starts_with(prefix) {
                results.push((key.clone(), value.clone()));
            }
        }
        
        Ok(results)
    }

    fn flush(&self) -> Result<(), KVStoreError> {
        // No-op for in-memory store
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_rocksdb_store() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path()).unwrap();

        // Test put and get
        store.put(b"key1", b"value1").unwrap();
        let value = store.get(b"key1").unwrap().unwrap();
        assert_eq!(value, b"value1");

        // Test exists
        assert!(store.exists(b"key1").unwrap());
        assert!(!store.exists(b"key2").unwrap());

        // Test delete
        store.delete(b"key1").unwrap();
        assert!(!store.exists(b"key1").unwrap());

        // Test batch operations
        let operations = vec![
            WriteBatchOperation::Put { key: b"key1".to_vec(), value: b"value1".to_vec() },
            WriteBatchOperation::Put { key: b"key2".to_vec(), value: b"value2".to_vec() },
            WriteBatchOperation::Delete { key: b"key1".to_vec() },
        ];
        store.write_batch(operations).unwrap();

        assert!(!store.exists(b"key1").unwrap());
        assert!(store.exists(b"key2").unwrap());

        // Test prefix scan
        store.put(b"prefix:key1", b"value1").unwrap();
        store.put(b"prefix:key2", b"value2").unwrap();
        store.put(b"other:key3", b"value3").unwrap();

        let results = store.scan_prefix(b"prefix:").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|(k, v)| k == b"prefix:key1" && v == b"value1"));
        assert!(results.iter().any(|(k, v)| k == b"prefix:key2" && v == b"value2"));
    }

    #[test]
    fn test_memory_store() {
        let store = MemoryStore::new();

        // Test put and get
        store.put(b"key1", b"value1").unwrap();
        let value = store.get(b"key1").unwrap().unwrap();
        assert_eq!(value, b"value1");

        // Test exists
        assert!(store.exists(b"key1").unwrap());
        assert!(!store.exists(b"key2").unwrap());

        // Test delete
        store.delete(b"key1").unwrap();
        assert!(!store.exists(b"key1").unwrap());

        // Test batch operations
        let operations = vec![
            WriteBatchOperation::Put { key: b"key1".to_vec(), value: b"value1".to_vec() },
            WriteBatchOperation::Put { key: b"key2".to_vec(), value: b"value2".to_vec() },
            WriteBatchOperation::Delete { key: b"key1".to_vec() },
        ];
        store.write_batch(operations).unwrap();

        assert!(!store.exists(b"key1").unwrap());
        assert!(store.exists(b"key2").unwrap());

        // Test prefix scan
        store.put(b"prefix:key1", b"value1").unwrap();
        store.put(b"prefix:key2", b"value2").unwrap();
        store.put(b"other:key3", b"value3").unwrap();

        let results = store.scan_prefix(b"prefix:").unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|(k, v)| k == b"prefix:key1" && v == b"value1"));
        assert!(results.iter().any(|(k, v)| k == b"prefix:key2" && v == b"value2"));
    }
}
