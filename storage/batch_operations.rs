use std::sync::Arc;
use log::{error, info};

use crate::storage::block_store::{Block, BlockStore, BlockStoreError};
use crate::storage::tx_store::{TransactionRecord, TxStore, TxStoreError};
use crate::storage::state::AccountState;
use crate::storage::state_store::{StateStore, StateStoreError};
use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::block_store::Hash;

/// Error type for batch operations
#[derive(Debug, thiserror::Error)]
pub enum BatchOperationError {
    /// Block store error
    #[error("Block store error: {0}")]
    BlockStoreError(#[from] BlockStoreError),

    /// Transaction store error
    #[error("Transaction store error: {0}")]
    TxStoreError(#[from] TxStoreError),

    /// State store error
    #[error("State store error: {0}")]
    StateStoreError(#[from] StateStoreError),

    /// KV store error
    #[error("KV store error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Batch operations manager for atomic updates
pub struct BatchOperationManager<'a> {
    /// KV store
    store: Arc<dyn KVStore + 'a>,
    /// Block store
    block_store: Arc<BlockStore<'a>>,
    /// Transaction store
    tx_store: Arc<TxStore<'a>>,
    /// State store
    state_store: Arc<StateStore<'a>>,
}

impl<'a> BatchOperationManager<'a> {
    /// Create a new batch operation manager
    pub fn new(
        store: Arc<dyn KVStore + 'a>,
        block_store: Arc<BlockStore<'a>>,
        tx_store: Arc<TxStore<'a>>,
        state_store: Arc<StateStore<'a>>,
    ) -> Self {
        Self {
            store,
            block_store,
            tx_store,
            state_store,
        }
    }

    /// Commit a block with all its transactions and state changes atomically
    pub fn commit_block(
        &self,
        block: &Block,
        transactions: &[TransactionRecord],
        state_changes: &[(Hash, AccountState)],
    ) -> Result<(), BatchOperationError> {
        // Create a batch operation
        let mut batch = Vec::new();

        // Add block to batch
        let block_key = format!("block:{}", block.height);
        let block_value = bincode::serialize(block)
            .map_err(|e| BatchOperationError::Other(format!("Failed to serialize block: {}", e)))?;

        batch.push(WriteBatchOperation::Put {
            key: block_key.as_bytes().to_vec(),
            value: block_value,
        });

        // Add block hash index
        let hash_key = format!("block_hash:{}", hex::encode(&block.hash));
        batch.push(WriteBatchOperation::Put {
            key: hash_key.as_bytes().to_vec(),
            value: block.height.to_be_bytes().to_vec(),
        });

        // Update latest block height metadata
        batch.push(WriteBatchOperation::Put {
            key: b"meta:latest_block_height".to_vec(),
            value: block.height.to_be_bytes().to_vec(),
        });

        // Add transactions to batch
        for tx in transactions {
            // Primary index by transaction hash
            let tx_key = format!("tx:{}", hex::encode(&tx.tx_id));
            let tx_value = bincode::serialize(tx)
                .map_err(|e| BatchOperationError::Other(format!("Failed to serialize transaction: {}", e)))?;

            batch.push(WriteBatchOperation::Put {
                key: tx_key.as_bytes().to_vec(),
                value: tx_value,
            });

            // Secondary index: transactions by block
            let block_tx_key = format!("tx_block:{}:{}", tx.block_height, hex::encode(&tx.tx_id));
            batch.push(WriteBatchOperation::Put {
                key: block_tx_key.as_bytes().to_vec(),
                value: tx.tx_id.to_vec(),
            });

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
        }

        // Add state changes to batch
        for (address, state) in state_changes {
            let state_key = format!("state:{}", hex::encode(address));
            let state_value = bincode::serialize(state)
                .map_err(|e| BatchOperationError::Other(format!("Failed to serialize state: {}", e)))?;

            batch.push(WriteBatchOperation::Put {
                key: state_key.as_bytes().to_vec(),
                value: state_value,
            });
        }

        // Calculate and store state root
        let state_root = self.state_store.calculate_state_root(block.height, block.timestamp)
            .map_err(|e| BatchOperationError::StateStoreError(e))?;

        let state_root_key = format!("state_root:{}", block.height);
        let state_root_value = bincode::serialize(&state_root)
            .map_err(|e| BatchOperationError::Other(format!("Failed to serialize state root: {}", e)))?;

        batch.push(WriteBatchOperation::Put {
            key: state_root_key.as_bytes().to_vec(),
            value: state_root_value,
        });

        // Execute the batch
        self.store.write_batch(batch)
            .map_err(|e| BatchOperationError::KVStoreError(e))?;

        info!("Committed block {} with {} transactions and {} state changes",
              block.height, transactions.len(), state_changes.len());

        Ok(())
    }

    /// Rollback a block and all its effects
    pub fn rollback_block(&self, block_height: u64) -> Result<(), BatchOperationError> {
        // Get the block
        let block = match self.block_store.get_block_by_height(block_height)? {
            Some(block) => block,
            None => return Err(BatchOperationError::Other(format!("Block not found: {}", block_height))),
        };

        // Get all transactions in the block
        let transactions = self.tx_store.get_transactions_by_block(block_height)?;

        // Create a batch operation
        let mut batch = Vec::new();

        // Remove block
        let block_key = format!("block:{}", block.height);
        batch.push(WriteBatchOperation::Delete {
            key: block_key.as_bytes().to_vec(),
        });

        // Remove block hash index
        let hash_key = format!("block_hash:{}", hex::encode(&block.hash));
        batch.push(WriteBatchOperation::Delete {
            key: hash_key.as_bytes().to_vec(),
        });

        // Update latest block height metadata to previous block
        if block_height > 0 {
            batch.push(WriteBatchOperation::Put {
                key: b"meta:latest_block_height".to_vec(),
                value: (block_height - 1).to_be_bytes().to_vec(),
            });
        } else {
            batch.push(WriteBatchOperation::Delete {
                key: b"meta:latest_block_height".to_vec(),
            });
        }

        // Remove transactions
        for tx in &transactions {
            // Remove primary index
            let tx_key = format!("tx:{}", hex::encode(&tx.tx_id));
            batch.push(WriteBatchOperation::Delete {
                key: tx_key.as_bytes().to_vec(),
            });

            // Remove block index
            let block_tx_key = format!("tx_block:{}:{}", tx.block_height, hex::encode(&tx.tx_id));
            batch.push(WriteBatchOperation::Delete {
                key: block_tx_key.as_bytes().to_vec(),
            });

            // Remove sender index
            let sender_tx_key = format!("tx_sender:{}:{}", hex::encode(&tx.sender), hex::encode(&tx.tx_id));
            batch.push(WriteBatchOperation::Delete {
                key: sender_tx_key.as_bytes().to_vec(),
            });

            // Remove recipient index
            let recipient_tx_key = format!("tx_recipient:{}:{}", hex::encode(&tx.recipient), hex::encode(&tx.tx_id));
            batch.push(WriteBatchOperation::Delete {
                key: recipient_tx_key.as_bytes().to_vec(),
            });

            // Note: We don't remove the sender nonce as it would be complex to determine the previous value
        }

        // Remove state root
        let state_root_key = format!("state_root:{}", block.height);
        batch.push(WriteBatchOperation::Delete {
            key: state_root_key.as_bytes().to_vec(),
        });

        // Execute the batch
        self.store.write_batch(batch)
            .map_err(|e| BatchOperationError::KVStoreError(e))?;

        info!("Rolled back block {} with {} transactions", block_height, transactions.len());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::kv_store::RocksDBStore;

    #[test]
    fn test_commit_and_rollback_block() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());

        // Create stores
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
        let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

        // Create batch operation manager
        let batch_manager = BatchOperationManager::new(
            kv_store.clone(),
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
        );

        // Create a test block
        let block = Block {
            height: 1,
            hash: [1; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![[2; 32], [3; 32]],
            state_root: [4; 32],
            tx_root: [5; 32],
            nonce: 42,
            poh_seq: 100,
            poh_hash: [6; 32],
            difficulty: 1000,
            total_difficulty: 1000,
        };

        // Create test transactions
        let tx1 = TransactionRecord {
            tx_id: [2; 32],
            sender: [10; 32],
            recipient: [11; 32],
            value: 100,
            gas_price: 1,
            gas_limit: 21000,
            gas_used: 21000,
            nonce: 0,
            timestamp: 12345,
            block_height: 1,
            data: None,
            status: crate::storage::tx_store::TransactionStatus::Confirmed,
        };

        let tx2 = TransactionRecord {
            tx_id: [3; 32],
            sender: [12; 32],
            recipient: [13; 32],
            value: 200,
            gas_price: 1,
            gas_limit: 21000,
            gas_used: 21000,
            nonce: 0,
            timestamp: 12345,
            block_height: 1,
            data: None,
            status: crate::storage::tx_store::TransactionStatus::Confirmed,
        };

        // Create test state changes
        let state1 = AccountState {
            balance: 900,
            nonce: 1,
            account_type: crate::storage::state_store::AccountType::User,
        };

        let state2 = AccountState {
            balance: 200,
            nonce: 0,
            account_type: crate::storage::state_store::AccountType::User,
        };

        let state_changes = vec![
            ([10; 32], state1),
            ([11; 32], state2),
        ];

        // Commit the block
        batch_manager.commit_block(&block, &[tx1, tx2], &state_changes).unwrap();

        // Verify the block was stored
        let stored_block = block_store.get_block_by_height(1).unwrap().unwrap();
        assert_eq!(stored_block.hash, block.hash);

        // Verify transactions were stored
        let stored_tx1 = tx_store.get_transaction(&[2; 32]).unwrap().unwrap();
        assert_eq!(stored_tx1.value, 100);

        let stored_tx2 = tx_store.get_transaction(&[3; 32]).unwrap().unwrap();
        assert_eq!(stored_tx2.value, 200);

        // Verify state changes were stored
        let stored_state1 = state_store.get_account_state(&[10; 32]).unwrap().unwrap();
        assert_eq!(stored_state1.balance, 900);

        let stored_state2 = state_store.get_account_state(&[11; 32]).unwrap().unwrap();
        assert_eq!(stored_state2.balance, 200);

        // Rollback the block
        batch_manager.rollback_block(1).unwrap();

        // Verify the block was removed
        assert!(block_store.get_block_by_height(1).unwrap().is_none());

        // Verify transactions were removed
        assert!(tx_store.get_transaction(&[2; 32]).unwrap().is_none());
        assert!(tx_store.get_transaction(&[3; 32]).unwrap().is_none());

        // Note: State changes are not automatically rolled back as that would require
        // knowing the previous state. In a real implementation, you would need to
        // store and restore the previous state.
    }
}
