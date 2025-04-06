use std::sync::Arc;
use tokio::sync::RwLock;

use crate::mempool::{Mempool, MempoolError, TransactionRecord, TransactionStatus};
use crate::storage::kv_store::RocksDBStore;
use crate::storage::state_store::StateStore;

#[tokio::test]
async fn test_mempool_lifecycle() {
    // Create a mempool
    let mempool = Mempool::new();
    
    // Create a transaction
    let sender = [1u8; 32];
    let recipient = [2u8; 32];
    
    let tx = TransactionRecord::new(
        sender,
        recipient,
        100, // value
        10,  // gas price
        1000, // gas limit
        1,    // nonce
        None, // data
    );
    
    // Insert the transaction
    let result = mempool.insert(tx.clone()).await;
    assert!(result.is_ok(), "Failed to insert transaction: {:?}", result);
    
    // Check transaction status
    let status = mempool.get_transaction_status(&tx.tx_id);
    assert_eq!(status, TransactionStatus::Pending);
    
    // Get pending transactions
    let pending = mempool.get_pending(10).await;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tx_id, tx.tx_id);
    
    // Mark the transaction as included in a block
    mempool.mark_included(&[tx.tx_id], 1).await;
    
    // Check transaction status again
    let status = mempool.get_transaction_status(&tx.tx_id);
    assert_eq!(status, TransactionStatus::Included);
    
    // Get pending transactions again
    let pending = mempool.get_pending(10).await;
    assert_eq!(pending.len(), 0, "Transaction should no longer be pending");
    
    // Check counts
    assert_eq!(mempool.pending_count(), 0);
    assert_eq!(mempool.included_count(), 1);
}

#[tokio::test]
async fn test_transaction_prioritization() {
    // Create a mempool
    let mempool = Mempool::new();
    
    // Create transactions with different gas prices
    let tx1 = TransactionRecord::new(
        [1u8; 32],
        [2u8; 32],
        100,
        5,  // Low gas price
        1000,
        1,
        None,
    );
    
    let tx2 = TransactionRecord::new(
        [3u8; 32],
        [4u8; 32],
        100,
        10, // Medium gas price
        1000,
        1,
        None,
    );
    
    let tx3 = TransactionRecord::new(
        [5u8; 32],
        [6u8; 32],
        100,
        15, // High gas price
        1000,
        1,
        None,
    );
    
    // Insert transactions in reverse order of priority
    mempool.insert(tx1.clone()).await.unwrap();
    mempool.insert(tx2.clone()).await.unwrap();
    mempool.insert(tx3.clone()).await.unwrap();
    
    // Get pending transactions
    let pending = mempool.get_pending(10).await;
    
    // Transactions should be ordered by gas price (highest first)
    assert_eq!(pending.len(), 3);
    assert_eq!(pending[0].tx_id, tx3.tx_id);
    assert_eq!(pending[1].tx_id, tx2.tx_id);
    assert_eq!(pending[2].tx_id, tx1.tx_id);
}

#[tokio::test]
async fn test_mempool_limits() {
    // Create a mempool with custom limits
    let config = crate::mempool::pool::MempoolConfig {
        max_size: 2,
        max_per_sender: 1,
        max_transaction_age: 3600,
        max_included_size: 100,
    };
    
    let mempool = Mempool::with_config(config);
    
    // Create transactions
    let tx1 = TransactionRecord::new(
        [1u8; 32],
        [2u8; 32],
        100,
        10,
        1000,
        1,
        None,
    );
    
    let tx2 = TransactionRecord::new(
        [3u8; 32], // Different sender
        [4u8; 32],
        100,
        10,
        1000,
        1,
        None,
    );
    
    let tx3 = TransactionRecord::new(
        [1u8; 32], // Same sender as tx1
        [2u8; 32],
        200,
        10,
        1000,
        2,
        None,
    );
    
    // Insert first transaction
    assert!(mempool.insert(tx1.clone()).await.is_ok());
    
    // Insert second transaction
    assert!(mempool.insert(tx2.clone()).await.is_ok());
    
    // Try to insert third transaction (should fail due to max_size)
    let result = mempool.insert(tx3.clone()).await;
    assert_eq!(result, Err(MempoolError::TooManyFromSender));
    
    // Remove one transaction
    mempool.remove(&[tx1.tx_id]).await;
    
    // Now we should be able to insert the third transaction
    assert!(mempool.insert(tx3.clone()).await.is_ok());
}

#[tokio::test]
async fn test_cleanup_expired() {
    // Create a mempool with a short expiration time
    let config = crate::mempool::pool::MempoolConfig {
        max_transaction_age: 1, // 1 second
        ..Default::default()
    };
    
    let mempool = Mempool::with_config(config);
    
    // Create a transaction
    let tx = TransactionRecord::new(
        [1u8; 32],
        [2u8; 32],
        100,
        10,
        1000,
        1,
        None,
    );
    
    // Insert the transaction
    mempool.insert(tx.clone()).await.unwrap();
    
    // Wait for the transaction to expire
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Clean up expired transactions
    mempool.cleanup_expired().await;
    
    // Check that the transaction is no longer in the mempool
    assert_eq!(mempool.pending_count(), 0);
}
