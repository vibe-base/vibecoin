use std::sync::Arc;
use tempfile::tempdir;

fn main() {
    println!("Testing VibeCoin Storage System");

    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    println!("Created temporary directory at: {:?}", temp_dir.path());

    // Create a RocksDB store
    let store = match vibecoin::storage::RocksDBStore::new(temp_dir.path()) {
        Ok(store) => {
            println!("Successfully created RocksDBStore");
            store
        },
        Err(e) => {
            println!("Failed to create RocksDBStore: {:?}", e);
            return;
        }
    };

    // Test basic operations
    println!("\nTesting basic operations...");
    match store.put(b"test_key", b"test_value") {
        Ok(_) => println!("Successfully put key-value pair"),
        Err(e) => {
            println!("Failed to put key-value pair: {:?}", e);
            return;
        }
    }

    match store.get(b"test_key") {
        Ok(Some(value)) => {
            println!("Successfully retrieved value: {:?}", value);
            assert_eq!(value, b"test_value");
        },
        Ok(None) => {
            println!("Key not found");
            return;
        },
        Err(e) => {
            println!("Failed to get value: {:?}", e);
            return;
        }
    }

    // Test batch operations
    println!("\nTesting batch operations...");
    let mut batch = Vec::new();
    batch.push(vibecoin::storage::WriteBatchOperation::Put {
        key: b"batch_key1".to_vec(),
        value: b"batch_value1".to_vec(),
    });
    batch.push(vibecoin::storage::WriteBatchOperation::Put {
        key: b"batch_key2".to_vec(),
        value: b"batch_value2".to_vec(),
    });

    match store.write_batch(batch) {
        Ok(_) => println!("Successfully wrote batch"),
        Err(e) => {
            println!("Failed to write batch: {:?}", e);
            return;
        }
    }

    // Verify batch operations
    match store.get(b"batch_key1") {
        Ok(Some(value)) => {
            println!("Successfully retrieved batch_key1: {:?}", value);
            assert_eq!(value, b"batch_value1");
        },
        Ok(None) => {
            println!("batch_key1 not found");
            return;
        },
        Err(e) => {
            println!("Failed to get batch_key1: {:?}", e);
            return;
        }
    }

    match store.get(b"batch_key2") {
        Ok(Some(value)) => {
            println!("Successfully retrieved batch_key2: {:?}", value);
            assert_eq!(value, b"batch_value2");
        },
        Ok(None) => {
            println!("batch_key2 not found");
            return;
        },
        Err(e) => {
            println!("Failed to get batch_key2: {:?}", e);
            return;
        }
    }

    // Test prefix scan
    println!("\nTesting prefix scan...");
    match store.scan_prefix(b"batch_") {
        Ok(results) => {
            println!("Successfully scanned prefix, found {} results", results.len());
            assert_eq!(results.len(), 2);
            for (key, value) in results {
                println!("Key: {:?}, Value: {:?}", String::from_utf8_lossy(&key), String::from_utf8_lossy(&value));
            }
        },
        Err(e) => {
            println!("Failed to scan prefix: {:?}", e);
            return;
        }
    }

    // Test delete operation
    println!("\nTesting delete operation...");
    match store.delete(b"test_key") {
        Ok(_) => println!("Successfully deleted key"),
        Err(e) => {
            println!("Failed to delete key: {:?}", e);
            return;
        }
    }

    match store.get(b"test_key") {
        Ok(Some(_)) => {
            println!("Key still exists after deletion");
            return;
        },
        Ok(None) => {
            println!("Successfully verified key deletion");
        },
        Err(e) => {
            println!("Failed to verify key deletion: {:?}", e);
            return;
        }
    }

    println!("\nAll tests passed!");
}
