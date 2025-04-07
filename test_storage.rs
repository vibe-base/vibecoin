use std::sync::Arc;
use tempfile::tempdir;

fn main() {
    println!("Testing RocksDB integration...");
    
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let store = vibecoin::storage::RocksDBStore::new(temp_dir.path()).unwrap();
    
    // Test basic operations
    store.put(b"test_key", b"test_value").unwrap();
    let value = store.get(b"test_key").unwrap().unwrap();
    assert_eq!(value, b"test_value");
    
    println!("Basic operations test passed!");
    
    // Test batch operations
    let mut batch = Vec::new();
    batch.push(vibecoin::storage::WriteBatchOperation::Put {
        key: b"batch_key1".to_vec(),
        value: b"batch_value1".to_vec(),
    });
    batch.push(vibecoin::storage::WriteBatchOperation::Put {
        key: b"batch_key2".to_vec(),
        value: b"batch_value2".to_vec(),
    });
    store.write_batch(batch).unwrap();
    
    let value1 = store.get(b"batch_key1").unwrap().unwrap();
    let value2 = store.get(b"batch_key2").unwrap().unwrap();
    assert_eq!(value1, b"batch_value1");
    assert_eq!(value2, b"batch_value2");
    
    println!("Batch operations test passed!");
    
    // Test prefix scan
    let prefix = b"batch_";
    let results = store.scan_prefix(prefix).unwrap();
    assert_eq!(results.len(), 2);
    
    println!("Prefix scan test passed!");
    
    println!("All tests passed!");
}
