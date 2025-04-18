use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::storage::{
    BlockStore, TxStore, StateStore, BatchOperationManager,
    Block, TransactionRecord, AccountState, Hash,
};
use crate::consensus::validation::{BlockValidator, BlockValidationResult};
use crate::consensus::validation::fork_choice::{choose_fork, resolve_fork, ForkChoice};
use crate::consensus::types::{Target, ChainState};
use crate::consensus::mining::mempool::Mempool;

/// Result of block processing
#[derive(Debug, Clone, PartialEq)]
pub enum BlockProcessingResult {
    /// Block was successfully processed
    Success,

    /// Block was already known
    AlreadyKnown,

    /// Block has an unknown parent
    UnknownParent,

    /// Block is invalid
    Invalid(String),

    /// Error occurred during processing
    Error(String),
}

/// Block processor for handling new blocks
pub struct BlockProcessor<'a> {
    /// Block store
    block_store: Arc<BlockStore<'a>>,

    /// Transaction store
    tx_store: Arc<TxStore<'a>>,

    /// State store
    state_store: Arc<StateStore<'a>>,

    /// Batch operation manager
    batch_manager: Arc<BatchOperationManager<'a>>,

    /// Block validator
    validator: Arc<BlockValidator<'a>>,

    /// Mempool
    mempool: Option<Arc<Mempool>>,
}

impl<'a> BlockProcessor<'a> {
    /// Create a new block processor
    pub fn new(
        block_store: Arc<BlockStore<'a>>,
        tx_store: Arc<TxStore<'a>>,
        state_store: Arc<StateStore<'a>>,
        batch_manager: Arc<BatchOperationManager<'a>>,
        validator: Arc<BlockValidator<'a>>,
        mempool: Option<Arc<Mempool>>,
    ) -> Self {
        Self {
            block_store,
            tx_store,
            state_store,
            batch_manager,
            validator,
            mempool,
        }
    }

    /// Process a new block
    pub async fn process_block(&self, block: &Block, target: &Target, chain_state: &ChainState) -> BlockProcessingResult {
        info!("Processing block at height {}", block.height);

        // Validate the block
        match self.validator.validate_block(block, target) {
            BlockValidationResult::Valid => {
                debug!("Block is valid");

                // Check if this block is part of the main chain or a fork
                let fork_choice = choose_fork(&self.block_store, chain_state, block);

                match fork_choice {
                    ForkChoice::Accept => {
                        // Block builds on the current chain, continue processing
                        debug!("Block builds on current chain");
                    },
                    ForkChoice::Reject => {
                        // Block is not part of the best chain, reject it
                        warn!("Block rejected by fork choice rule: height={}", block.height);
                        return BlockProcessingResult::Invalid("Rejected by fork choice rule".to_string());
                    },
                    ForkChoice::Fork => {
                        // We have a fork, need to resolve it
                        warn!("Fork detected at height {}", block.height);

                        // Get the current tip
                        let current_tip = match self.block_store.get_block_by_hash(&chain_state.tip_hash) {
                            Ok(Some(tip)) => tip,
                            Ok(None) => {
                                error!("Current tip block not found");
                                return BlockProcessingResult::Error("Current tip block not found".to_string());
                            },
                            Err(e) => {
                                error!("Failed to get current tip: {}", e);
                                return BlockProcessingResult::Error(format!("Failed to get current tip: {}", e));
                            }
                        };

                        // Resolve the fork
                        match resolve_fork(&self.block_store, &current_tip, block) {
                            Ok(canonical_tip) => {
                                if canonical_tip.hash != block.hash {
                                    // The current chain is still the best chain
                                    info!("Current chain is the canonical tip, rejecting new block");
                                    return BlockProcessingResult::Invalid("Not the canonical tip".to_string());
                                }

                                // The new block is the canonical tip, continue processing
                                info!("New block is the canonical tip, accepting it");
                            },
                            Err(e) => {
                                error!("Failed to resolve fork: {}", e);
                                return BlockProcessingResult::Error(format!("Failed to resolve fork: {}", e));
                            }
                        }
                    },
                    ForkChoice::Replace => {
                        // Replace the current chain with this block
                        debug!("Replacing current chain with this block");
                    }
                }
            },
            BlockValidationResult::AlreadyKnown => {
                debug!("Block already known: height={}", block.height);
                return BlockProcessingResult::AlreadyKnown;
            },
            BlockValidationResult::UnknownParent => {
                warn!("Block has unknown parent: height={}", block.height);
                return BlockProcessingResult::UnknownParent;
            },
            BlockValidationResult::Invalid(reason) => {
                warn!("Invalid block: {}", reason);
                return BlockProcessingResult::Invalid(reason);
            }
        }

        // Get the transactions for this block
        let transactions = match self.get_block_transactions(block) {
            Ok(txs) => txs,
            Err(e) => {
                error!("Failed to get block transactions: {}", e);
                return BlockProcessingResult::Error(format!("Failed to get transactions: {}", e));
            }
        };

        // Apply the transactions to get state changes
        let state_changes = match self.apply_transactions(block, &transactions) {
            Ok(changes) => changes,
            Err(e) => {
                error!("Failed to apply transactions: {}", e);
                return BlockProcessingResult::Error(format!("Failed to apply transactions: {}", e));
            }
        };

        // Commit the block, transactions, and state changes atomically
        match self.batch_manager.commit_block(block, &transactions, &state_changes) {
            Ok(_) => {
                info!("Block committed successfully: height={}", block.height);

                // Update mempool to remove included transactions
                if let Some(mempool) = &self.mempool {
                    self.update_mempool(block, mempool).await;
                }

                BlockProcessingResult::Success
            },
            Err(e) => {
                error!("Failed to commit block: {}", e);
                BlockProcessingResult::Error(format!("Failed to commit block: {}", e))
            }
        }
    }

    /// Get the transactions for a block
    fn get_block_transactions(&self, block: &Block) -> Result<Vec<TransactionRecord>, String> {
        let mut transactions = Vec::new();

        for tx_hash in &block.transactions {
            match self.tx_store.get_transaction(tx_hash) {
                Ok(Some(tx)) => {
                    transactions.push(tx);
                },
                Ok(None) => {
                    // Transaction not found, try to get it from the mempool
                    if let Some(mempool) = &self.mempool {
                        if let Some(tx) = mempool.get_pending_transactions(100).iter().find(|tx| tx.tx_id == *tx_hash) {
                            // Create a confirmed transaction record
                            let mut tx_record = tx.clone();
                            tx_record.block_height = block.height;
                            tx_record.status = crate::storage::TransactionStatus::Confirmed;
                            transactions.push(tx_record);
                        } else {
                            return Err(format!("Transaction not found: {}", hex::encode(tx_hash)));
                        }
                    } else {
                        return Err(format!("Transaction not found: {}", hex::encode(tx_hash)));
                    }
                },
                Err(e) => {
                    return Err(format!("Failed to get transaction {}: {}", hex::encode(tx_hash), e));
                }
            }
        }

        Ok(transactions)
    }

    /// Apply transactions to get state changes
    fn apply_transactions(&self, _block: &Block, transactions: &[TransactionRecord]) -> Result<Vec<(Hash, AccountState)>, String> {
        let mut state_changes = Vec::new();

        // Create a temporary state store for validation
        let temp_state = self.state_store.clone_for_validation();

        // Apply all transactions to the temporary state
        for tx in transactions {
            // Get sender account
            let sender = match temp_state.get_account_state(&tx.sender) {
                Some(account) => account,
                None => {
                    return Err(format!("Sender account not found: {}", hex::encode(&tx.sender)));
                }
            };

            // Get recipient account (or create if it doesn't exist)
            let recipient = match temp_state.get_account_state(&tx.recipient) {
                Some(account) => account,
                None => {
                    // Create a new account for the recipient
                    match temp_state.create_account(&tx.recipient, 0, crate::storage::state::AccountType::User) {
                        Ok(()) => {
                            match temp_state.get_account_state(&tx.recipient) {
                                Some(account) => account,
                                None => {
                                    return Err(format!("Failed to create recipient account: {}", hex::encode(&tx.recipient)));
                                }
                            }
                        },
                        Err(e) => {
                            return Err(format!("Failed to create recipient account: {}", e));
                        }
                    }
                }
            };

            // Check sender balance
            if sender.balance < tx.value + tx.gas_price * tx.gas_used {
                return Err(format!("Insufficient balance for sender: {}", hex::encode(&tx.sender)));
            }

            // Update sender account
            let new_sender_balance = sender.balance - (tx.value + tx.gas_price * tx.gas_used);
            let new_sender_nonce = sender.nonce + 1;

            let mut new_sender = sender.clone();
            new_sender.balance = new_sender_balance;
            new_sender.nonce = new_sender_nonce;

            // Update recipient account
            let new_recipient_balance = recipient.balance + tx.value;

            let mut new_recipient = recipient.clone();
            new_recipient.balance = new_recipient_balance;

            // Add state changes
            state_changes.push((tx.sender, new_sender.clone()));
            state_changes.push((tx.recipient, new_recipient.clone()));

            // Update the temporary state
            match temp_state.update_account(&tx.sender, &new_sender) {
                Ok(()) => {},
                Err(e) => {
                    return Err(format!("Failed to update sender account: {}", e));
                }
            }

            match temp_state.update_account(&tx.recipient, &new_recipient) {
                Ok(()) => {},
                Err(e) => {
                    return Err(format!("Failed to update recipient account: {}", e));
                }
            }
        }

        Ok(state_changes)
    }

    /// Update mempool to remove transactions included in the block
    async fn update_mempool(&self, block: &Block, mempool: &Mempool) {
        // Mark transactions as included in a block
        for tx_hash in &block.transactions {
            mempool.mark_included(tx_hash);
        }
    }

    /// Rollback a block
    pub async fn rollback_block(&self, height: u64) -> Result<(), String> {
        info!("Rolling back block at height {}", height);

        match self.batch_manager.rollback_block(height) {
            Ok(()) => {
                info!("Block rolled back successfully: height={}", height);
                Ok(())
            },
            Err(e) => {
                error!("Failed to rollback block: {}", e);
                Err(format!("Failed to rollback block: {}", e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::{RocksDBStore, KVStore};
    use crate::storage::tx_store::TransactionStatus;

    #[tokio::test]
    async fn test_block_processing() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());

        // Create stores
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
        let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

        // Create batch operation manager
        let batch_manager = Arc::new(BatchOperationManager::new(
            kv_store.clone(),
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
        ));

        // Create validator
        let validator = Arc::new(BlockValidator::new(
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
        ));

        // Create block processor
        let processor = BlockProcessor::new(
            block_store.clone(),
            tx_store.clone(),
            state_store.clone(),
            batch_manager.clone(),
            validator.clone(),
            None,
        );

        // Create a genesis block
        let genesis = Block {
            height: 0,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 0,
            poh_hash: [0u8; 32],
            difficulty: 1,
            total_difficulty: 1,
        };

        // Create a chain state
        let chain_state = ChainState {
            height: 0,
            current_target: Target::from_difficulty(1),
            latest_hash: [0u8; 32],
            latest_timestamp: 0,
            latest_poh_sequence: 0,
        };

        // Process the genesis block
        let result = processor.process_block(&genesis, &Target::from_difficulty(1), &chain_state).await;
        assert_eq!(result, BlockProcessingResult::Success);

        // Create some accounts
        state_store.create_account(&[1u8; 32], 1000, crate::storage::AccountType::User).unwrap();
        state_store.create_account(&[2u8; 32], 0, crate::storage::AccountType::User).unwrap();

        // Create a transaction
        let tx = TransactionRecord {
            tx_id: [3u8; 32],
            sender: [1u8; 32],
            recipient: [2u8; 32],
            value: 100,
            gas_price: 1,
            gas_limit: 21000,
            gas_used: 21000,
            nonce: 0,
            timestamp: 10,
            block_height: 1,
            data: None,
            status: TransactionStatus::Confirmed,
        };

        // Store the transaction
        tx_store.put_transaction(&tx).unwrap();

        // Create a block with the transaction
        let block = Block {
            height: 1,
            hash: [1u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 10,
            transactions: vec![[3u8; 32]],
            state_root: [0u8; 32], // This would normally be calculated
            tx_root: [0u8; 32], // This would normally be calculated
            nonce: 42,
            poh_seq: 10,
            poh_hash: [0u8; 32],
            difficulty: 1,
            total_difficulty: 2,
        };

        // Update the chain state to reflect the genesis block
        let chain_state = ChainState {
            height: 0,
            current_target: Target::from_difficulty(1),
            latest_hash: [0u8; 32],
            latest_timestamp: 0,
            latest_poh_sequence: 0,
        };

        // Process the block
        let result = processor.process_block(&block, &Target::from_difficulty(1), &chain_state).await;
        assert_eq!(result, BlockProcessingResult::Success);

        // Check that the block was stored
        let stored_block = block_store.get_block_by_height(1).unwrap().unwrap();
        assert_eq!(stored_block.hash, block.hash);

        // Check that the transaction was stored
        let stored_tx = tx_store.get_transaction(&[3u8; 32]).unwrap().unwrap();
        assert_eq!(stored_tx.value, 100);

        // Check that the state was updated
        let sender_state = state_store.get_account_state(&[1u8; 32]).unwrap().unwrap();
        assert_eq!(sender_state.balance, 879); // 1000 - 100 - 21

        let recipient_state = state_store.get_account_state(&[2u8; 32]).unwrap().unwrap();
        assert_eq!(recipient_state.balance, 100);

        // Rollback the block
        processor.rollback_block(1).await.unwrap();

        // Check that the block was removed
        assert!(block_store.get_block_by_height(1).unwrap().is_none());
    }
}
