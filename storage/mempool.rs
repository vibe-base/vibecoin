//! Mempool storage integration for VibeCoin blockchain
//!
//! This module provides persistent storage for the mempool (transaction pool),
//! allowing for transaction recovery after node restarts and efficient
//! management of pending transactions.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use log::{debug, error, info, warn};
use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::tx_store::{TransactionRecord, TransactionStatus};
use crate::storage::block_store::Hash;

/// Error types for mempool storage operations
#[derive(Error, Debug)]
pub enum MempoolStorageError {
    /// Key-value store error
    #[error("KV store error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// Transaction not found
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Mempool transaction metadata
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MempoolTransactionMetadata {
    /// Transaction ID (hash)
    pub tx_id: Hash,

    /// Timestamp when the transaction was added to the mempool
    pub added_timestamp: u64,

    /// Last time the transaction was validated
    pub last_validated: u64,

    /// Number of times the transaction was validated
    pub validation_count: u32,

    /// Priority score (higher is better)
    pub priority_score: u64,

    /// Whether the transaction is currently valid
    pub is_valid: bool,
}

impl MempoolTransactionMetadata {
    /// Create new metadata for a transaction
    pub fn new(tx_id: Hash, priority_score: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            tx_id,
            added_timestamp: now,
            last_validated: now,
            validation_count: 1,
            priority_score,
            is_valid: true,
        }
    }

    /// Update the validation timestamp and count
    pub fn update_validation(&mut self, is_valid: bool) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.last_validated = now;
        self.validation_count += 1;
        self.is_valid = is_valid;
    }
}

/// Store for mempool transactions
pub struct MempoolStore<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,
}

impl<'a> MempoolStore<'a> {
    /// Create a new MempoolStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }

    /// Add a transaction to the mempool storage
    pub fn add_transaction(
        &self,
        tx: &TransactionRecord,
        priority_score: u64,
    ) -> Result<(), MempoolStorageError> {
        // Create metadata for the transaction
        let metadata = MempoolTransactionMetadata::new(tx.tx_id, priority_score);

        // Serialize the transaction and metadata
        let tx_value = bincode::serialize(tx)
            .map_err(|e| MempoolStorageError::SerializationError(e.to_string()))?;

        let metadata_value = bincode::serialize(&metadata)
            .map_err(|e| MempoolStorageError::SerializationError(e.to_string()))?;

        // Create a batch operation
        let mut batch = Vec::new();

        // Store the transaction
        let tx_key = format!("mempool:tx:{}", hex::encode(&tx.tx_id));
        batch.push(WriteBatchOperation::Put {
            key: tx_key.as_bytes().to_vec(),
            value: tx_value,
        });

        // Store the metadata
        let metadata_key = format!("mempool:meta:{}", hex::encode(&tx.tx_id));
        batch.push(WriteBatchOperation::Put {
            key: metadata_key.as_bytes().to_vec(),
            value: metadata_value,
        });

        // Add to sender index
        let sender_key = format!("mempool:sender:{}:{}", hex::encode(&tx.sender), hex::encode(&tx.tx_id));
        batch.push(WriteBatchOperation::Put {
            key: sender_key.as_bytes().to_vec(),
            value: tx.tx_id.to_vec(),
        });

        // Add to priority index
        let priority_key = format!("mempool:priority:{}:{}", priority_score, hex::encode(&tx.tx_id));
        batch.push(WriteBatchOperation::Put {
            key: priority_key.as_bytes().to_vec(),
            value: tx.tx_id.to_vec(),
        });

        // Execute the batch operation
        self.store.write_batch(batch)?;

        debug!("Added transaction {} to mempool storage", hex::encode(&tx.tx_id));
        Ok(())
    }

    /// Remove a transaction from the mempool storage
    pub fn remove_transaction(&self, tx_id: &Hash) -> Result<(), MempoolStorageError> {
        // First, get the transaction to access its metadata
        let tx = self.get_transaction(tx_id)?;
        let metadata = self.get_transaction_metadata(tx_id)?;

        // Create a batch operation
        let mut batch = Vec::new();

        // Remove the transaction
        let tx_key = format!("mempool:tx:{}", hex::encode(tx_id));
        batch.push(WriteBatchOperation::Delete {
            key: tx_key.as_bytes().to_vec(),
        });

        // Remove the metadata
        let metadata_key = format!("mempool:meta:{}", hex::encode(tx_id));
        batch.push(WriteBatchOperation::Delete {
            key: metadata_key.as_bytes().to_vec(),
        });

        // Remove from sender index
        let sender_key = format!("mempool:sender:{}:{}", hex::encode(&tx.sender), hex::encode(tx_id));
        batch.push(WriteBatchOperation::Delete {
            key: sender_key.as_bytes().to_vec(),
        });

        // Remove from priority index
        let priority_key = format!("mempool:priority:{}:{}", metadata.priority_score, hex::encode(tx_id));
        batch.push(WriteBatchOperation::Delete {
            key: priority_key.as_bytes().to_vec(),
        });

        // Execute the batch operation
        self.store.write_batch(batch)?;

        debug!("Removed transaction {} from mempool storage", hex::encode(tx_id));
        Ok(())
    }

    /// Get a transaction from the mempool storage
    pub fn get_transaction(&self, tx_id: &Hash) -> Result<TransactionRecord, MempoolStorageError> {
        let tx_key = format!("mempool:tx:{}", hex::encode(tx_id));

        let value = self.store.get(tx_key.as_bytes())
            .map_err(MempoolStorageError::from)?
            .ok_or_else(|| MempoolStorageError::TransactionNotFound(hex::encode(tx_id)))?;

        bincode::deserialize(&value)
            .map_err(|e| MempoolStorageError::DeserializationError(e.to_string()))
    }

    /// Get transaction metadata from the mempool storage
    pub fn get_transaction_metadata(&self, tx_id: &Hash) -> Result<MempoolTransactionMetadata, MempoolStorageError> {
        let metadata_key = format!("mempool:meta:{}", hex::encode(tx_id));

        let value = self.store.get(metadata_key.as_bytes())
            .map_err(MempoolStorageError::from)?
            .ok_or_else(|| MempoolStorageError::TransactionNotFound(hex::encode(tx_id)))?;

        bincode::deserialize(&value)
            .map_err(|e| MempoolStorageError::DeserializationError(e.to_string()))
    }

    /// Update transaction metadata
    pub fn update_transaction_metadata(
        &self,
        tx_id: &Hash,
        is_valid: bool,
        new_priority_score: Option<u64>,
    ) -> Result<(), MempoolStorageError> {
        // Get the current metadata
        let mut metadata = self.get_transaction_metadata(tx_id)?;

        // Update validation info
        metadata.update_validation(is_valid);

        // Create a batch operation
        let mut batch = Vec::new();

        // If priority score is changing, update the priority index
        if let Some(new_score) = new_priority_score {
            if new_score != metadata.priority_score {
                // Remove old priority index entry
                let old_priority_key = format!("mempool:priority:{}:{}", metadata.priority_score, hex::encode(tx_id));
                batch.push(WriteBatchOperation::Delete {
                    key: old_priority_key.as_bytes().to_vec(),
                });

                // Add new priority index entry
                let new_priority_key = format!("mempool:priority:{}:{}", new_score, hex::encode(tx_id));
                batch.push(WriteBatchOperation::Put {
                    key: new_priority_key.as_bytes().to_vec(),
                    value: tx_id.to_vec(),
                });

                // Update the metadata priority score
                metadata.priority_score = new_score;
            }
        }

        // Serialize and store the updated metadata
        let metadata_value = bincode::serialize(&metadata)
            .map_err(|e| MempoolStorageError::SerializationError(e.to_string()))?;

        let metadata_key = format!("mempool:meta:{}", hex::encode(tx_id));
        batch.push(WriteBatchOperation::Put {
            key: metadata_key.as_bytes().to_vec(),
            value: metadata_value,
        });

        // Execute the batch operation
        self.store.write_batch(batch)?;

        debug!("Updated metadata for transaction {} in mempool storage", hex::encode(tx_id));
        Ok(())
    }

    /// Get all transactions in the mempool
    pub fn get_all_transactions(&self) -> Result<Vec<TransactionRecord>, MempoolStorageError> {
        let prefix = "mempool:tx:";
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(MempoolStorageError::from)?;

        let mut transactions = Vec::with_capacity(results.len());
        for (_, value) in results {
            let tx: TransactionRecord = bincode::deserialize(&value)
                .map_err(|e| MempoolStorageError::DeserializationError(e.to_string()))?;
            transactions.push(tx);
        }

        Ok(transactions)
    }

    /// Get transactions by sender
    pub fn get_transactions_by_sender(&self, sender: &Hash) -> Result<Vec<TransactionRecord>, MempoolStorageError> {
        let prefix = format!("mempool:sender:{}", hex::encode(sender));
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(MempoolStorageError::from)?;

        let mut transactions = Vec::with_capacity(results.len());
        for (_, tx_id_bytes) in results {
            let tx_id: Hash = tx_id_bytes.try_into()
                .map_err(|_| MempoolStorageError::Other("Invalid transaction ID".to_string()))?;

            let tx = self.get_transaction(&tx_id)?;
            transactions.push(tx);
        }

        Ok(transactions)
    }

    /// Get transactions by priority (highest first)
    pub fn get_transactions_by_priority(&self, limit: usize) -> Result<Vec<TransactionRecord>, MempoolStorageError> {
        let prefix = "mempool:priority:";
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(MempoolStorageError::from)?;

        // Sort by priority (descending)
        let mut sorted_results: Vec<_> = results.into_iter().collect();
        sorted_results.sort_by(|(a, _), (b, _)| b.cmp(a)); // Reverse order for highest first

        let mut transactions = Vec::with_capacity(limit.min(sorted_results.len()));
        for (_, tx_id_bytes) in sorted_results.into_iter().take(limit) {
            let tx_id: Hash = tx_id_bytes.try_into()
                .map_err(|_| MempoolStorageError::Other("Invalid transaction ID".to_string()))?;

            let tx = self.get_transaction(&tx_id)?;
            transactions.push(tx);
        }

        Ok(transactions)
    }

    /// Count transactions in the mempool
    pub fn count_transactions(&self) -> Result<usize, MempoolStorageError> {
        let prefix = "mempool:tx:";
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(MempoolStorageError::from)?;

        Ok(results.len())
    }

    /// Count transactions by sender
    pub fn count_transactions_by_sender(&self, sender: &Hash) -> Result<usize, MempoolStorageError> {
        let prefix = format!("mempool:sender:{}", hex::encode(sender));
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(MempoolStorageError::from)?;

        Ok(results.len())
    }

    /// Clear all transactions from the mempool
    pub fn clear(&self) -> Result<(), MempoolStorageError> {
        // Get all transaction IDs
        let prefix = "mempool:tx:";
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(MempoolStorageError::from)?;

        let mut batch = Vec::new();

        for (key, value) in results {
            // Extract tx_id from the key
            let tx_id_hex = String::from_utf8_lossy(&key[prefix.len()..])
                .to_string();

            let tx_id = hex::decode(&tx_id_hex)
                .map_err(|_| MempoolStorageError::Other("Invalid transaction ID".to_string()))?;

            let tx_id: Hash = tx_id.try_into()
                .map_err(|_| MempoolStorageError::Other("Invalid transaction ID length".to_string()))?;

            // Deserialize the transaction to get sender
            let tx: TransactionRecord = bincode::deserialize(&value)
                .map_err(|e| MempoolStorageError::DeserializationError(e.to_string()))?;

            // Get metadata to get priority score
            let metadata = self.get_transaction_metadata(&tx_id)?;

            // Delete all related keys
            batch.push(WriteBatchOperation::Delete {
                key: key,
            });

            let metadata_key = format!("mempool:meta:{}", tx_id_hex);
            batch.push(WriteBatchOperation::Delete {
                key: metadata_key.as_bytes().to_vec(),
            });

            let sender_key = format!("mempool:sender:{}:{}", hex::encode(&tx.sender), tx_id_hex);
            batch.push(WriteBatchOperation::Delete {
                key: sender_key.as_bytes().to_vec(),
            });

            let priority_key = format!("mempool:priority:{}:{}", metadata.priority_score, tx_id_hex);
            batch.push(WriteBatchOperation::Delete {
                key: priority_key.as_bytes().to_vec(),
            });
        }

        // Execute the batch operation if there are any operations
        if !batch.is_empty() {
            self.store.write_batch(batch)?;
        }

        info!("Cleared all transactions from mempool storage");
        Ok(())
    }

    /// Prune expired transactions
    pub fn prune_expired_transactions(&self, max_age_seconds: u64) -> Result<usize, MempoolStorageError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cutoff_time = now.saturating_sub(max_age_seconds);

        // Get all transaction metadata
        let prefix = "mempool:meta:";
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(MempoolStorageError::from)?;

        let mut expired_tx_ids = Vec::new();

        for (_, value) in results {
            let metadata: MempoolTransactionMetadata = bincode::deserialize(&value)
                .map_err(|e| MempoolStorageError::DeserializationError(e.to_string()))?;

            // Check if transaction is expired
            if metadata.added_timestamp < cutoff_time {
                expired_tx_ids.push(metadata.tx_id);
            }
        }

        // Remove all expired transactions
        for tx_id in &expired_tx_ids {
            self.remove_transaction(tx_id)?;
        }

        let count = expired_tx_ids.len();
        if count > 0 {
            info!("Pruned {} expired transactions from mempool storage", count);
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_transaction(tx_id: [u8; 32], sender: [u8; 32], nonce: u64) -> TransactionRecord {
        TransactionRecord {
            tx_id,
            sender,
            recipient: [0; 32],
            value: 100,
            gas_price: 10,
            gas_limit: 21000,
            gas_used: 0,
            nonce,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            block_height: 0,
            data: None,
            status: TransactionStatus::Pending,
        }
    }

    #[test]
    fn test_add_and_get_transaction() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        let tx_id = [1; 32];
        let sender = [2; 32];
        let tx = create_test_transaction(tx_id, sender, 1);

        // Add transaction
        mempool_store.add_transaction(&tx, 100).unwrap();

        // Get transaction
        let retrieved_tx = mempool_store.get_transaction(&tx_id).unwrap();
        assert_eq!(retrieved_tx.tx_id, tx.tx_id);
        assert_eq!(retrieved_tx.sender, tx.sender);
        assert_eq!(retrieved_tx.nonce, tx.nonce);

        // Get metadata
        let metadata = mempool_store.get_transaction_metadata(&tx_id).unwrap();
        assert_eq!(metadata.tx_id, tx_id);
        assert_eq!(metadata.priority_score, 100);
        assert!(metadata.is_valid);
    }

    #[test]
    fn test_remove_transaction() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        let tx_id = [1; 32];
        let sender = [2; 32];
        let tx = create_test_transaction(tx_id, sender, 1);

        // Add transaction
        mempool_store.add_transaction(&tx, 100).unwrap();

        // Remove transaction
        mempool_store.remove_transaction(&tx_id).unwrap();

        // Try to get transaction (should fail)
        let result = mempool_store.get_transaction(&tx_id);
        assert!(result.is_err());
        match result {
            Err(MempoolStorageError::TransactionNotFound(_)) => {},
            _ => panic!("Expected TransactionNotFound error"),
        }
    }

    #[test]
    fn test_update_transaction_metadata() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        let tx_id = [1; 32];
        let sender = [2; 32];
        let tx = create_test_transaction(tx_id, sender, 1);

        // Add transaction
        mempool_store.add_transaction(&tx, 100).unwrap();

        // Update metadata
        mempool_store.update_transaction_metadata(&tx_id, false, Some(200)).unwrap();

        // Get updated metadata
        let metadata = mempool_store.get_transaction_metadata(&tx_id).unwrap();
        assert_eq!(metadata.tx_id, tx_id);
        assert_eq!(metadata.priority_score, 200);
        assert_eq!(metadata.is_valid, false);
        assert_eq!(metadata.validation_count, 2);
    }

    #[test]
    fn test_get_transactions_by_sender() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        let sender = [2; 32];

        // Add multiple transactions from same sender
        for i in 0..3 {
            let tx_id = [i as u8; 32];
            let tx = create_test_transaction(tx_id, sender, i as u64);
            mempool_store.add_transaction(&tx, 100).unwrap();
        }

        // Get transactions by sender
        let txs = mempool_store.get_transactions_by_sender(&sender).unwrap();
        assert_eq!(txs.len(), 3);

        // Verify all transactions have the correct sender
        for tx in txs {
            assert_eq!(tx.sender, sender);
        }
    }

    #[test]
    fn test_get_transactions_by_priority() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        // Add transactions with different priorities
        for i in 0..5 {
            let tx_id = [i as u8; 32];
            let sender = [i as u8; 32];
            let tx = create_test_transaction(tx_id, sender, 1);
            mempool_store.add_transaction(&tx, (i * 100) as u64).unwrap();
        }

        // Get transactions by priority (highest first)
        let txs = mempool_store.get_transactions_by_priority(3).unwrap();
        assert_eq!(txs.len(), 3);

        // Verify transactions are in priority order
        assert_eq!(txs[0].tx_id, [4; 32]);
        assert_eq!(txs[1].tx_id, [3; 32]);
        assert_eq!(txs[2].tx_id, [2; 32]);
    }

    #[test]
    fn test_count_transactions() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        // Add multiple transactions
        for i in 0..5 {
            let tx_id = [i as u8; 32];
            let sender = [i as u8; 32];
            let tx = create_test_transaction(tx_id, sender, 1);
            mempool_store.add_transaction(&tx, 100).unwrap();
        }

        // Count all transactions
        let count = mempool_store.count_transactions().unwrap();
        assert_eq!(count, 5);

        // Count transactions by sender
        let sender_count = mempool_store.count_transactions_by_sender(&[0; 32]).unwrap();
        assert_eq!(sender_count, 1);
    }

    #[test]
    fn test_clear_mempool() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        // Add multiple transactions
        for i in 0..5 {
            let tx_id = [i as u8; 32];
            let sender = [i as u8; 32];
            let tx = create_test_transaction(tx_id, sender, 1);
            mempool_store.add_transaction(&tx, 100).unwrap();
        }

        // Clear mempool
        mempool_store.clear().unwrap();

        // Verify mempool is empty
        let count = mempool_store.count_transactions().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_prune_expired_transactions() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let mempool_store = MempoolStore::new(&kv_store);

        // Add transactions
        for i in 0..5 {
            let tx_id = [i as u8; 32];
            let sender = [i as u8; 32];
            let tx = create_test_transaction(tx_id, sender, 1);
            mempool_store.add_transaction(&tx, 100).unwrap();

            // Manually update the added timestamp for some transactions to make them appear older
            if i < 3 {
                let mut metadata = mempool_store.get_transaction_metadata(&tx_id).unwrap();
                metadata.added_timestamp -= 3600; // 1 hour ago

                let metadata_key = format!("mempool:meta:{}", hex::encode(&tx_id));
                let metadata_value = bincode::serialize(&metadata).unwrap();
                kv_store.put(metadata_key.as_bytes(), &metadata_value).unwrap();
            }
        }

        // Prune transactions older than 30 minutes
        let pruned = mempool_store.prune_expired_transactions(1800).unwrap();
        assert_eq!(pruned, 3);

        // Verify only 2 transactions remain
        let count = mempool_store.count_transactions().unwrap();
        assert_eq!(count, 2);
    }
}