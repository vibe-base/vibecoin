use rocksdb::{DB, Options, WriteBatch, IteratorMode};
use std::path::Path;
use std::io::Error;
use std::fmt;
use log::error;

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

impl WriteBatchOperation {
    /// Create a new batch operation
    pub fn new() -> Vec<WriteBatchOperation> {
        Vec::new()
    }
}

/// Extension trait for Vec<WriteBatchOperation>
pub trait WriteBatchOperationExt {
    /// Add a put operation to the batch
    fn put(&mut self, key: Vec<u8>, value: Vec<u8>);

    /// Add a delete operation to the batch
    fn delete(&mut self, key: Vec<u8>);
}

impl WriteBatchOperationExt for Vec<WriteBatchOperation> {
    fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.push(WriteBatchOperation::Put { key, value });
    }

    fn delete(&mut self, key: Vec<u8>) {
        self.push(WriteBatchOperation::Delete { key });
    }
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
    pub fn new(path: &Path) -> Result<Self, KVStoreError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to open RocksDB: {}", e)))?;
        Ok(Self { db })
    }

    /// Create a new RocksDBStore with custom options
    pub fn with_options(path: &Path, options: Options) -> Result<Self, KVStoreError> {
        let db = DB::open(&options, path)
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to open RocksDB: {}", e)))?;
        Ok(Self { db })
    }

    /// Get the underlying RocksDB instance
    pub fn get_db(&self) -> &DB {
        &self.db
    }

    /// Compact the database
    pub fn compact(&self) -> Result<(), KVStoreError> {
        self.db.compact_range(None::<&[u8]>, None::<&[u8]>);
        Ok(())
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

            // Check if key starts with prefix
            if key.starts_with(prefix) {
                results.push((key.to_vec(), value.to_vec()));
            } else {
                // We've moved past the prefix
                break;
            }
        }

        Ok(results)
    }

    fn flush(&self) -> Result<(), KVStoreError> {
        self.db.flush()
            .map_err(|e| KVStoreError::RocksDBError(format!("Failed to flush: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_basic_operations() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path());

        // Test put and get
        let key = b"test_key";
        let value = b"test_value";
        store.put(key, value).unwrap();

        let result = store.get(key).unwrap();
        assert_eq!(result, Some(value.to_vec()));

        // Test exists
        assert!(store.exists(key).unwrap());
        assert!(!store.exists(b"nonexistent_key").unwrap());

        // Test delete
        store.delete(key).unwrap();
        let result = store.get(key).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_batch_operations() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path());

        // Create a batch of operations
        let mut batch = Vec::new();
        batch.put(b"key1".to_vec(), b"value1".to_vec());
        batch.put(b"key2".to_vec(), b"value2".to_vec());
        batch.put(b"key3".to_vec(), b"value3".to_vec());

        // Write the batch
        store.write_batch(batch).unwrap();

        // Verify the results
        assert_eq!(store.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(store.get(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(store.get(b"key3").unwrap(), Some(b"value3".to_vec()));

        // Test batch with delete
        let mut batch = Vec::new();
        batch.delete(b"key1".to_vec());
        batch.put(b"key2".to_vec(), b"updated".to_vec());

        // Write the batch
        store.write_batch(batch).unwrap();

        // Verify the results
        assert_eq!(store.get(b"key1").unwrap(), None);
        assert_eq!(store.get(b"key2").unwrap(), Some(b"updated".to_vec()));
    }

    #[test]
    fn test_scan_prefix() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path());

        // Insert some keys with a common prefix
        store.put(b"prefix:1", b"value1").unwrap();
        store.put(b"prefix:2", b"value2").unwrap();
        store.put(b"prefix:3", b"value3").unwrap();
        store.put(b"other:1", b"other1").unwrap();

        // Scan with prefix
        let results = store.scan_prefix(b"prefix:").unwrap();

        // Should find 3 keys with the prefix
        assert_eq!(results.len(), 3);

        // Check the results
        let mut found_keys = 0;
        for (key, value) in results {
            if key == b"prefix:1".to_vec() {
                assert_eq!(value, b"value1".to_vec());
                found_keys += 1;
            } else if key == b"prefix:2".to_vec() {
                assert_eq!(value, b"value2".to_vec());
                found_keys += 1;
            } else if key == b"prefix:3".to_vec() {
                assert_eq!(value, b"value3".to_vec());
                found_keys += 1;
            }
        }

        assert_eq!(found_keys, 3);
    }
}
