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
    /// Key not found
    KeyNotFound,
    /// Other error
    Other(String),
}

impl fmt::Display for KVStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KVStoreError::IoError(err) => write!(f, "IO error: {}", err),
            KVStoreError::RocksDBError(err) => write!(f, "RocksDB error: {}", err),
            KVStoreError::SerializationError(err) => write!(f, "Serialization error: {}", err),
            KVStoreError::KeyNotFound => write!(f, "Key not found"),
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

/// Batch operation for atomic writes
pub struct WriteBatchOperation {
    /// Operations to perform
    pub operations: Vec<BatchOperation>,
}

impl WriteBatchOperation {
    /// Create a new batch operation
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    /// Add a put operation to the batch
    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.operations.push(BatchOperation::Put(key, value));
    }

    /// Add a delete operation to the batch
    pub fn delete(&mut self, key: Vec<u8>) {
        self.operations.push(BatchOperation::Delete(key));
    }
}

/// Batch operation type
pub enum BatchOperation {
    /// Put operation
    Put(Vec<u8>, Vec<u8>),
    /// Delete operation
    Delete(Vec<u8>),
}

/// Key-Value Store trait defining the interface for storage operations
pub trait KVStore {
    /// Store a key-value pair
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError>;
    
    /// Retrieve a value by key
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError>;
    
    /// Delete a key-value pair
    fn delete(&self, key: &[u8]) -> Result<(), KVStoreError>;
    
    /// Scan for keys with a given prefix
    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError>;
    
    /// Execute a batch of operations atomically
    fn write_batch(&self, batch: WriteBatchOperation) -> Result<(), KVStoreError>;
    
    /// Create a snapshot of the database
    fn create_snapshot(&self) -> Result<(), KVStoreError>;
    
    /// Flush all write operations to disk
    fn flush(&self) -> Result<(), KVStoreError>;
}

/// RocksDB implementation of the KVStore trait
pub struct RocksDBStore {
    db: Arc<DB>,
}

impl RocksDBStore {
    /// Create a new RocksDB store at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Snappy);
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        opts.set_max_write_buffer_number(3);
        opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB
        opts.set_level_zero_file_num_compaction_trigger(8);
        opts.set_level_zero_slowdown_writes_trigger(17);
        opts.set_level_zero_stop_writes_trigger(24);
        opts.set_num_levels(7);
        opts.set_max_bytes_for_level_base(512 * 1024 * 1024); // 512MB
        opts.set_max_bytes_for_level_multiplier(10.0);
        
        let db = DB::open(&opts, path).expect("Failed to open RocksDB");
        Self { db: Arc::new(db) }
    }
    
    /// Create a new RocksDB store with custom options
    pub fn with_options<P: AsRef<Path>>(path: P, options: Options) -> Self {
        let db = DB::open(&options, path).expect("Failed to open RocksDB");
        Self { db: Arc::new(db) }
    }
    
    /// Get the underlying RocksDB instance
    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }
}

impl KVStore for RocksDBStore {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        self.db.put(key, value).map_err(|e| e.into())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        self.db.get(key).map_err(|e| e.into())
    }

    fn delete(&self, key: &[u8]) -> Result<(), KVStoreError> {
        self.db.delete(key).map_err(|e| e.into())
    }

    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError> {
        let mut result = Vec::new();
        let iterator = self.db.prefix_iterator(prefix);
        for item in iterator {
            let (key, value) = item.map_err(|e| e.into())?;
            result.push((key.to_vec(), value.to_vec()));
        }
        Ok(result)
    }
    
    fn write_batch(&self, batch: WriteBatchOperation) -> Result<(), KVStoreError> {
        let mut write_batch = WriteBatch::default();
        
        for op in batch.operations {
            match op {
                BatchOperation::Put(key, value) => {
                    write_batch.put(&key, &value);
                }
                BatchOperation::Delete(key) => {
                    write_batch.delete(&key);
                }
            }
        }
        
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true); // Ensure durability
        
        self.db.write_opt(write_batch, &write_opts).map_err(|e| e.into())
    }
    
    fn create_snapshot(&self) -> Result<(), KVStoreError> {
        // RocksDB doesn't have a direct API for creating snapshots that persist to disk
        // Instead, we can use checkpoints or simply flush the database
        self.flush()
    }
    
    fn flush(&self) -> Result<(), KVStoreError> {
        self.db.flush().map_err(|e| e.into())
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
        store.put(key, value).unwrap();

        let retrieved = store.get(key).unwrap().unwrap();
        assert_eq!(retrieved, value);

        // Test delete
        store.delete(key).unwrap();
        assert!(store.get(key).unwrap().is_none());

        // Test scan_prefix
        store.put(b"prefix1:a", b"value1").unwrap();
        store.put(b"prefix1:b", b"value2").unwrap();
        store.put(b"prefix2:c", b"value3").unwrap();

        let results = store.scan_prefix(b"prefix1:").unwrap();
        assert_eq!(results.len(), 2);
    }
    
    #[test]
    fn test_write_batch() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path());
        
        // Create a batch operation
        let mut batch = WriteBatchOperation::new();
        batch.put(b"batch_key1".to_vec(), b"batch_value1".to_vec());
        batch.put(b"batch_key2".to_vec(), b"batch_value2".to_vec());
        
        // Execute the batch
        store.write_batch(batch).unwrap();
        
        // Verify the results
        let value1 = store.get(b"batch_key1").unwrap().unwrap();
        let value2 = store.get(b"batch_key2").unwrap().unwrap();
        
        assert_eq!(value1, b"batch_value1");
        assert_eq!(value2, b"batch_value2");
    }
    
    #[test]
    fn test_flush() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path());
        
        // Put some data
        store.put(b"flush_key", b"flush_value").unwrap();
        
        // Flush the database
        store.flush().unwrap();
        
        // Verify the data is still there
        let value = store.get(b"flush_key").unwrap().unwrap();
        assert_eq!(value, b"flush_value");
    }
}
