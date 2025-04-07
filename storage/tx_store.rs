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

    /// Invalid data format
    #[error("Invalid data format: {0}")]
    InvalidDataFormat(String),

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
        let mut batch = Vec::new();

        // Primary index by transaction hash
        let tx_key = format!("tx:{}", hex::encode(&tx.tx_id));
        batch.push(WriteBatchOperation::Put {
            key: tx_key.as_bytes().to_vec(),
            value: value.clone(),
        });

        // Secondary index: transactions by block
        if tx.block_height > 0 {
            let block_tx_key = format!("tx_block:{}:{}", tx.block_height, hex::encode(&tx.tx_id));
            batch.push(WriteBatchOperation::Put {
                key: block_tx_key.as_bytes().to_vec(),
                value: tx.tx_id.to_vec(),
            });
        }

        // Secondary index: transactions by sender
        let sender_tx_key = format!("tx_sender:{}:{}", hex::encode(&tx.sender), hex::encode(&tx.tx_id));
        batch.push(WriteBatchOperation::Put {
            key: sender_tx_key.as_bytes().to_vec(),
            value: tx.tx_id.to_vec(),
        });

        // Secondary index: transactions by recipient
        let recipient_tx_key = format!("tx_recipient:{}:{}", hex::encode(&tx.recipient), hex::encode(&tx.tx_id));
        batch.push(WriteBatchOperation::Put {
            key: recipient_tx_key.as_bytes().to_vec(),
            value: tx.tx_id.to_vec(),
        });

        // Secondary index: latest nonce for sender
        let sender_nonce_key = format!("tx_sender_nonce:{}", hex::encode(&tx.sender));

        // Only update if this nonce is higher than any previously stored
        match self.store.get(sender_nonce_key.as_bytes()) {
            Ok(Some(bytes)) => {
                if bytes.len() == 8 {
                    let mut nonce_arr = [0u8; 8];
                    nonce_arr.copy_from_slice(&bytes);
                    let stored_nonce = u64::from_be_bytes(nonce_arr);

                    if tx.nonce > stored_nonce {
                        batch.push(WriteBatchOperation::Put {
                            key: sender_nonce_key.as_bytes().to_vec(),
                            value: tx.nonce.to_be_bytes().to_vec(),
                        });
                    }
                } else {
                    // Invalid format, overwrite
                    batch.push(WriteBatchOperation::Put {
                        key: sender_nonce_key.as_bytes().to_vec(),
                        value: tx.nonce.to_be_bytes().to_vec(),
                    });
                }
            },
            _ => {
                // No existing nonce, store this one
                batch.push(WriteBatchOperation::Put {
                    key: sender_nonce_key.as_bytes().to_vec(),
                    value: tx.nonce.to_be_bytes().to_vec(),
                });
            }
        }

        // Execute the batch
        self.store.write_batch(batch).map_err(|e| e.into())
    }

    /// Retrieve a transaction by its ID
    pub fn get_transaction(&self, tx_id: &Hash) -> Result<Option<TransactionRecord>, TxStoreError> {
        let key = format!("tx:{}", hex::encode(tx_id));
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                match bincode::deserialize(&bytes) {
                    Ok(tx) => Ok(Some(tx)),
                    Err(e) => {
                        error!("Failed to deserialize transaction {}: {}", hex::encode(tx_id), e);
                        Err(TxStoreError::SerializationError(format!(
                            "Failed to deserialize transaction {}: {}", hex::encode(tx_id), e
                        )))
                    }
                }
            },
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Failed to get transaction {}: {}", hex::encode(tx_id), e);
                Err(TxStoreError::KVStoreError(e))
            }
        }
    }

    /// Get all transactions for a specific sender
    pub fn get_transactions_by_sender(&self, sender: &Hash) -> Result<Vec<TransactionRecord>, TxStoreError> {
        let prefix = format!("tx_sender:{}:", hex::encode(sender));
        let mut transactions = Vec::new();

        // Scan all keys with the sender prefix
        match self.store.scan_prefix(prefix.as_bytes()) {
            Ok(items) => {
                for (_, tx_id_bytes) in items {
                    // The value is the transaction ID
                    if tx_id_bytes.len() == 32 {
                        let mut tx_id = [0u8; 32];
                        tx_id.copy_from_slice(&tx_id_bytes);

                        // Get the full transaction
                        match self.get_transaction(&tx_id)? {
                            Some(tx) => transactions.push(tx),
                            None => {
                                // This shouldn't happen, but log it if it does
                                warn!("Transaction {} referenced in index but not found", hex::encode(&tx_id));
                            }
                        }
                    } else {
                        warn!("Invalid transaction ID format in sender index");
                    }
                }
                Ok(transactions)
            },
            Err(e) => {
                error!("Failed to scan transactions by sender: {}", e);
                Err(TxStoreError::KVStoreError(e))
            }
        }
    }

    /// Get all transactions for a specific recipient
    pub fn get_transactions_by_recipient(&self, recipient: &Hash) -> Result<Vec<TransactionRecord>, TxStoreError> {
        let prefix = format!("tx_recipient:{}:", hex::encode(recipient));
        let mut transactions = Vec::new();

        // Scan all keys with the recipient prefix
        match self.store.scan_prefix(prefix.as_bytes()) {
            Ok(items) => {
                for (_, tx_id_bytes) in items {
                    // The value is the transaction ID
                    if tx_id_bytes.len() == 32 {
                        let mut tx_id = [0u8; 32];
                        tx_id.copy_from_slice(&tx_id_bytes);

                        // Get the full transaction
                        match self.get_transaction(&tx_id)? {
                            Some(tx) => transactions.push(tx),
                            None => {
                                // This shouldn't happen, but log it if it does
                                warn!("Transaction {} referenced in index but not found", hex::encode(&tx_id));
                            }
                        }
                    } else {
                        warn!("Invalid transaction ID format in recipient index");
                    }
                }
                Ok(transactions)
            },
            Err(e) => {
                error!("Failed to scan transactions by recipient: {}", e);
                Err(TxStoreError::KVStoreError(e))
            }
        }
    }

    /// Get all transactions in a specific block
    pub fn get_transactions_by_block(&self, block_height: u64) -> Result<Vec<TransactionRecord>, TxStoreError> {
        let prefix = format!("tx_block:{}:", block_height);
        let mut transactions = Vec::new();

        // Scan all keys with the block prefix
        match self.store.scan_prefix(prefix.as_bytes()) {
            Ok(items) => {
                for (_, tx_id_bytes) in items {
                    // The value is the transaction ID
                    if tx_id_bytes.len() == 32 {
                        let mut tx_id = [0u8; 32];
                        tx_id.copy_from_slice(&tx_id_bytes);

                        // Get the full transaction
                        match self.get_transaction(&tx_id)? {
                            Some(tx) => transactions.push(tx),
                            None => {
                                // This shouldn't happen, but log it if it does
                                warn!("Transaction {} referenced in block {} but not found",
                                     hex::encode(&tx_id), block_height);
                            }
                        }
                    } else {
                        warn!("Invalid transaction ID format in block index");
                    }
                }
                Ok(transactions)
            },
            Err(e) => {
                error!("Failed to scan transactions by block {}: {}", block_height, e);
                Err(TxStoreError::KVStoreError(e))
            }
        }
    }

    /// Get the latest nonce for a sender
    pub fn get_latest_nonce(&self, sender: &Hash) -> Result<Option<u64>, TxStoreError> {
        let key = format!("tx_sender_nonce:{}", hex::encode(sender));
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                if bytes.len() == 8 {
                    let mut nonce_arr = [0u8; 8];
                    nonce_arr.copy_from_slice(&bytes);
                    let nonce = u64::from_be_bytes(nonce_arr);
                    Ok(Some(nonce))
                } else {
                    error!("Invalid nonce format for sender {}", hex::encode(sender));
                    Err(TxStoreError::InvalidDataFormat(format!(
                        "Invalid nonce format for sender {}", hex::encode(sender)
                    )))
                }
            },
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Failed to get latest nonce for sender {}: {}",
                       hex::encode(sender), e);
                Err(TxStoreError::KVStoreError(e))
            }
        }
    }

    /// Update transaction status
    pub fn update_transaction_status(&self, tx_id: &Hash, status: TransactionStatus) -> Result<(), TxStoreError> {
        // Get the transaction
        let tx = match self.get_transaction(tx_id)? {
            Some(tx) => tx,
            None => return Err(TxStoreError::TransactionNotFound(hex::encode(tx_id))),
        };

        // Create updated transaction
        let mut updated_tx = tx.clone();
        updated_tx.status = status;

        // Store the updated transaction
        self.put_transaction(&updated_tx)
    }

    /// Check if a transaction exists
    pub fn has_transaction(&self, tx_id: &Hash) -> Result<bool, TxStoreError> {
        let key = format!("tx:{}", hex::encode(tx_id));
        match self.store.get(key.as_bytes()) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => {
                error!("Failed to check if transaction exists {}: {}", hex::encode(tx_id), e);
                Err(TxStoreError::KVStoreError(e))
            }
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
