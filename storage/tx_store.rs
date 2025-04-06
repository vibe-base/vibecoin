use serde::{Serialize, Deserialize};
use std::sync::Arc;
use log::{debug, error, info, warn};
use hex;

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::block_store::Hash;

/// Transaction record structure
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TransactionRecord {
    /// Transaction ID (hash)
    pub tx_id: Hash,

    /// Sender address
    pub sender: Hash,

    /// Recipient address
    pub recipient: Hash,

    /// Transaction value
    pub value: u64,

    /// Gas price (fee per gas unit)
    pub gas_price: u64,

    /// Gas limit (maximum gas units)
    pub gas_limit: u64,

    /// Gas used (actual gas units consumed)
    pub gas_used: u64,

    /// Transaction nonce (to prevent replay attacks)
    pub nonce: u64,

    /// Transaction timestamp
    pub timestamp: u64,

    /// Block height where this transaction was included
    pub block_height: u64,

    /// Transaction data (for smart contracts)
    pub data: Option<Vec<u8>>,

    /// Transaction status
    pub status: TransactionStatus,
}

/// Transaction status
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum TransactionStatus {
    /// Transaction is pending in the mempool
    Pending,

    /// Transaction was included in a block
    Included,

    /// Transaction was confirmed (enough blocks on top)
    Confirmed,

    /// Transaction failed during execution
    Failed(TransactionError),
}

/// Transaction error
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum TransactionError {
    /// Insufficient balance
    InsufficientBalance,

    /// Invalid nonce
    InvalidNonce,

    /// Out of gas
    OutOfGas,

    /// Execution error
    ExecutionError,

    /// Invalid signature
    InvalidSignature,

    /// Other error
    Other,
}

/// Error type for TxStore operations
#[derive(Debug, thiserror::Error)]
pub enum TxStoreError {
    /// KVStore error
    #[error("KVStore error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Transaction not found
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Store for transaction records
pub struct TxStore<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,
}

impl<'a> TxStore<'a> {
    /// Create a new TxStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }

    /// Store a transaction record
    pub fn put_transaction(&self, tx: &TransactionRecord) -> Result<(), TxStoreError> {
        let value = bincode::serialize(tx)
            .map_err(|e| TxStoreError::SerializationError(e.to_string()))?;

        // Create a batch operation
        let mut batch = WriteBatchOperation::new();

        // Index by ID
        let id_key = format!("tx:id:{}", hex::encode(&tx.tx_id));
        batch.put(id_key.as_bytes().to_vec(), value.clone());

        // Index by sender
        let sender_key = format!("tx:sender:{}:{}", hex::encode(&tx.sender), hex::encode(&tx.tx_id));
        batch.put(sender_key.as_bytes().to_vec(), value.clone());

        // Index by recipient
        let recipient_key = format!("tx:recipient:{}:{}", hex::encode(&tx.recipient), hex::encode(&tx.tx_id));
        batch.put(recipient_key.as_bytes().to_vec(), value.clone());

        // Index by block
        if tx.block_height > 0 {
            let block_key = format!("tx:block:{}:{}", tx.block_height, hex::encode(&tx.tx_id));
            batch.put(block_key.as_bytes().to_vec(), value.clone());
        }

        // Index by nonce (for sender)
        let nonce_key = format!("tx:sender:{}:nonce:{}", hex::encode(&tx.sender), tx.nonce);
        batch.put(nonce_key.as_bytes().to_vec(), value);

        // Execute the batch
        self.store.write_batch(batch).map_err(|e| e.into())
    }

    /// Retrieve a transaction by its ID
    pub fn get_transaction(&self, tx_id: &Hash) -> Option<TransactionRecord> {
        let key = format!("tx:id:{}", hex::encode(tx_id));
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                match bincode::deserialize(&bytes) {
                    Ok(tx) => Some(tx),
                    Err(e) => {
                        error!("Failed to deserialize transaction {}: {}", hex::encode(tx_id), e);
                        None
                    }
                }
            },
            Ok(None) => None,
            Err(e) => {
                error!("Failed to get transaction {}: {}", hex::encode(tx_id), e);
                None
            }
        }
    }

    /// Get all transactions for a specific sender
    pub fn get_transactions_by_sender(&self, sender: &Hash) -> Vec<TransactionRecord> {
        let prefix = format!("tx:sender:{}:", hex::encode(sender));
        match self.store.scan_prefix(prefix.as_bytes()) {
            Ok(entries) => {
                entries.iter()
                    .filter_map(|(_, v)| {
                        match bincode::deserialize(v) {
                            Ok(tx) => Some(tx),
                            Err(e) => {
                                error!("Failed to deserialize transaction: {}", e);
                                None
                            }
                        }
                    })
                    .collect()
            },
            Err(e) => {
                error!("Failed to scan transactions by sender {}: {}", hex::encode(sender), e);
                Vec::new()
            }
        }
    }

    /// Get all transactions for a specific recipient
    pub fn get_transactions_by_recipient(&self, recipient: &Hash) -> Vec<TransactionRecord> {
        let prefix = format!("tx:recipient:{}:", hex::encode(recipient));
        match self.store.scan_prefix(prefix.as_bytes()) {
            Ok(entries) => {
                entries.iter()
                    .filter_map(|(_, v)| {
                        match bincode::deserialize(v) {
                            Ok(tx) => Some(tx),
                            Err(e) => {
                                error!("Failed to deserialize transaction: {}", e);
                                None
                            }
                        }
                    })
                    .collect()
            },
            Err(e) => {
                error!("Failed to scan transactions by recipient {}: {}", hex::encode(recipient), e);
                Vec::new()
            }
        }
    }

    /// Get all transactions in a specific block
    pub fn get_transactions_by_block(&self, block_height: u64) -> Vec<TransactionRecord> {
        let prefix = format!("tx:block:{}:", block_height);
        match self.store.scan_prefix(prefix.as_bytes()) {
            Ok(entries) => {
                entries.iter()
                    .filter_map(|(_, v)| {
                        match bincode::deserialize(v) {
                            Ok(tx) => Some(tx),
                            Err(e) => {
                                error!("Failed to deserialize transaction: {}", e);
                                None
                            }
                        }
                    })
                    .collect()
            },
            Err(e) => {
                error!("Failed to scan transactions by block {}: {}", block_height, e);
                Vec::new()
            }
        }
    }

    /// Get transaction by sender and nonce
    pub fn get_transaction_by_nonce(&self, sender: &Hash, nonce: u64) -> Option<TransactionRecord> {
        let key = format!("tx:sender:{}:nonce:{}", hex::encode(sender), nonce);
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                match bincode::deserialize(&bytes) {
                    Ok(tx) => Some(tx),
                    Err(e) => {
                        error!("Failed to deserialize transaction for sender {} nonce {}: {}",
                               hex::encode(sender), nonce, e);
                        None
                    }
                }
            },
            Ok(None) => None,
            Err(e) => {
                error!("Failed to get transaction for sender {} nonce {}: {}",
                       hex::encode(sender), nonce, e);
                None
            }
        }
    }

    /// Update transaction status
    pub fn update_transaction_status(&self, tx_id: &Hash, status: TransactionStatus) -> Result<(), TxStoreError> {
        // Get the transaction
        let mut tx = self.get_transaction(tx_id)
            .ok_or_else(|| TxStoreError::TransactionNotFound(hex::encode(tx_id)))?;

        // Update the status
        tx.status = status;

        // Store the updated transaction
        self.put_transaction(&tx)
    }

    /// Check if a transaction exists
    pub fn has_transaction(&self, tx_id: &Hash) -> bool {
        let key = format!("tx:id:{}", hex::encode(tx_id));
        match self.store.get(key.as_bytes()) {
            Ok(Some(_)) => true,
            _ => false,
        }
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), TxStoreError> {
        self.store.flush().map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[test]
    fn test_tx_store() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let tx_store = TxStore::new(&kv_store);

        // Create a test transaction
        let tx = TransactionRecord {
            tx_id: [1; 32],
            sender: [2; 32],
            recipient: [3; 32],
            value: 100,
            gas_price: 5,
            gas_limit: 21000,
            gas_used: 10,
            nonce: 1,
            timestamp: 12345,
            block_height: 5,
            data: None,
            status: TransactionStatus::Included,
        };

        // Store the transaction
        tx_store.put_transaction(&tx).unwrap();

        // Retrieve by ID
        let retrieved = tx_store.get_transaction(&[1; 32]).unwrap();
        assert_eq!(retrieved, tx);

        // Test retrieval by sender
        let sender_txs = tx_store.get_transactions_by_sender(&[2; 32]);
        assert_eq!(sender_txs.len(), 1);
        assert_eq!(sender_txs[0], tx);

        // Test retrieval by recipient
        let recipient_txs = tx_store.get_transactions_by_recipient(&[3; 32]);
        assert_eq!(recipient_txs.len(), 1);
        assert_eq!(recipient_txs[0], tx);

        // Test retrieval by block
        let block_txs = tx_store.get_transactions_by_block(5);
        assert_eq!(block_txs.len(), 1);
        assert_eq!(block_txs[0], tx);

        // Test retrieval by nonce
        let nonce_tx = tx_store.get_transaction_by_nonce(&[2; 32], 1).unwrap();
        assert_eq!(nonce_tx, tx);

        // Test has_transaction
        assert!(tx_store.has_transaction(&[1; 32]));
        assert!(!tx_store.has_transaction(&[4; 32]));

        // Test update_transaction_status
        tx_store.update_transaction_status(&[1; 32], TransactionStatus::Confirmed).unwrap();
        let updated = tx_store.get_transaction(&[1; 32]).unwrap();
        assert_eq!(updated.status, TransactionStatus::Confirmed);

        // Test flush
        tx_store.flush().unwrap();
    }

    #[test]
    fn test_multiple_transactions() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let tx_store = TxStore::new(&kv_store);

        // Create and store multiple transactions
        for i in 0..5 {
            let tx = TransactionRecord {
                tx_id: [i as u8 + 1; 32],
                sender: [2; 32],
                recipient: [3; 32],
                value: 100 * i,
                gas_price: 5,
                gas_limit: 21000,
                gas_used: 10 * i,
                nonce: i,
                timestamp: 12345 + i,
                block_height: 5 + i,
                data: None,
                status: TransactionStatus::Included,
            };

            tx_store.put_transaction(&tx).unwrap();
        }

        // Test get_transactions_by_sender
        let sender_txs = tx_store.get_transactions_by_sender(&[2; 32]);
        assert_eq!(sender_txs.len(), 5);

        // Test get_transaction_by_nonce
        let nonce_tx = tx_store.get_transaction_by_nonce(&[2; 32], 3).unwrap();
        assert_eq!(nonce_tx.value, 300);
    }

    #[test]
    fn test_transaction_status() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let tx_store = TxStore::new(&kv_store);

        // Create a pending transaction
        let tx = TransactionRecord {
            tx_id: [1; 32],
            sender: [2; 32],
            recipient: [3; 32],
            value: 100,
            gas_price: 5,
            gas_limit: 21000,
            gas_used: 0, // Not executed yet
            nonce: 1,
            timestamp: 12345,
            block_height: 0, // Not included in a block yet
            data: None,
            status: TransactionStatus::Pending,
        };

        // Store the transaction
        tx_store.put_transaction(&tx).unwrap();

        // Update to included
        tx_store.update_transaction_status(&[1; 32], TransactionStatus::Included).unwrap();
        let included = tx_store.get_transaction(&[1; 32]).unwrap();
        assert_eq!(included.status, TransactionStatus::Included);

        // Update to confirmed
        tx_store.update_transaction_status(&[1; 32], TransactionStatus::Confirmed).unwrap();
        let confirmed = tx_store.get_transaction(&[1; 32]).unwrap();
        assert_eq!(confirmed.status, TransactionStatus::Confirmed);

        // Update to failed
        tx_store.update_transaction_status(&[1; 32], TransactionStatus::Failed(TransactionError::OutOfGas)).unwrap();
        let failed = tx_store.get_transaction(&[1; 32]).unwrap();
        match failed.status {
            TransactionStatus::Failed(error) => {
                assert_eq!(error, TransactionError::OutOfGas);
            },
            _ => panic!("Expected Failed status"),
        }
    }
}
