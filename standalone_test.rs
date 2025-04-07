use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;
use rocksdb::{DB, Options, WriteBatch, WriteOptions, IteratorMode};
use std::io::{Error, ErrorKind};

fn main() {
    println!("Testing RocksDB Integration");
    
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    println!("Created temporary directory at: {:?}", temp_dir.path());
    
    // Create RocksDB options
    let mut options = Options::default();
    options.create_if_missing(true);
    
    // Open the database
    let db = match DB::open(&options, temp_dir.path()) {
        Ok(db) => {
            println!("Successfully opened RocksDB database");
            db
        },
        Err(e) => {
            println!("Failed to open RocksDB database: {:?}", e);
            return;
        }
    };
    
    // Test basic operations
    println!("\nTesting basic operations...");
    match db.put(b"test_key", b"test_value") {
        Ok(_) => println!("Successfully put key-value pair"),
        Err(e) => {
            println!("Failed to put key-value pair: {:?}", e);
            return;
        }
    }
    
    match db.get(b"test_key") {
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
    let mut batch = WriteBatch::default();
    batch.put(b"batch_key1", b"batch_value1");
    batch.put(b"batch_key2", b"batch_value2");
    
    match db.write(batch) {
        Ok(_) => println!("Successfully wrote batch"),
        Err(e) => {
            println!("Failed to write batch: {:?}", e);
            return;
        }
    }
    
    // Verify batch operations
    match db.get(b"batch_key1") {
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
    
    match db.get(b"batch_key2") {
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
    let iter = db.iterator(IteratorMode::From(b"batch_", rocksdb::Direction::Forward));
    let mut count = 0;
    for (key, value) in iter {
        if key.starts_with(b"batch_") {
            count += 1;
            println!("Key: {:?}, Value: {:?}", String::from_utf8_lossy(&key), String::from_utf8_lossy(&value));
        } else {
            break;
        }
    }
    println!("Found {} results with prefix 'batch_'", count);
    assert_eq!(count, 2);
    
    // Test delete operation
    println!("\nTesting delete operation...");
    match db.delete(b"test_key") {
        Ok(_) => println!("Successfully deleted key"),
        Err(e) => {
            println!("Failed to delete key: {:?}", e);
            return;
        }
    }
    
    match db.get(b"test_key") {
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
