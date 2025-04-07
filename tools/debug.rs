//! Debug tools for VibeCoin blockchain
//!
//! This module provides debugging tools for the blockchain,
//! including state inspection, transaction tracing, and block analysis.

use std::sync::Arc;
use log::error;

use crate::storage::{
    BlockStore, StateStore, TxStore,
    Block, AccountState, TransactionRecord,
};

/// Debug tools for blockchain inspection
pub struct DebugTools<'a> {
    /// Block store
    block_store: Arc<BlockStore<'a>>,

    /// State store
    state_store: Arc<StateStore<'a>>,

    /// Transaction store
    tx_store: Arc<TxStore<'a>>,
}

impl<'a> DebugTools<'a> {
    /// Create a new instance of DebugTools
    pub fn new(
        block_store: Arc<BlockStore<'a>>,
        state_store: Arc<StateStore<'a>>,
        tx_store: Arc<TxStore<'a>>,
    ) -> Self {
        Self {
            block_store,
            state_store,
            tx_store,
        }
    }

    /// Inspect a block
    pub fn inspect_block(&self, hash: &[u8; 32]) -> Option<Block> {
        match self.block_store.get_block_by_hash(hash) {
            Ok(block) => block,
            Err(e) => {
                error!("Failed to get block: {}", e);
                None
            }
        }
    }

    /// Inspect an account
    pub fn inspect_account(&self, address: &[u8]) -> Option<AccountState> {
        match self.state_store.get_latest_account(address) {
            Ok(account) => account,
            Err(e) => {
                error!("Failed to get account: {}", e);
                None
            }
        }
    }

    /// Inspect a transaction
    pub fn inspect_transaction(&self, hash: &[u8; 32]) -> Option<TransactionRecord> {
        match self.tx_store.get_transaction(hash) {
            Ok(tx) => tx,
            Err(e) => {
                error!("Failed to get transaction: {}", e);
                None
            }
        }
    }

    /// Trace a transaction
    pub fn trace_transaction(&self, hash: &[u8; 32]) -> Vec<String> {
        let mut trace = Vec::new();

        // Get the transaction
        let tx = match self.tx_store.get_transaction(hash) {
            Ok(Some(tx)) => tx,
            Ok(None) => {
                trace.push("Transaction not found".to_string());
                return trace;
            }
            Err(e) => {
                trace.push(format!("Failed to get transaction: {}", e));
                return trace;
            }
        };

        trace.push(format!("Transaction: {:?}", tx));

        // Get the block containing the transaction
        if tx.block_height > 0 {
            match self.block_store.get_block_by_height(tx.block_height) {
                Ok(Some(block)) => {
                    trace.push(format!("Block: height={}, hash={:?}", block.height, block.hash));
                }
                Ok(None) => {
                    trace.push("Block not found".to_string());
                }
                Err(e) => {
                    trace.push(format!("Failed to get block: {}", e));
                }
            }
        } else {
            trace.push("Transaction not included in any block".to_string());
        }

        // Get the sender account
        match self.state_store.get_latest_account(&tx.sender) {
            Ok(Some(account)) => {
                trace.push(format!("Sender account: balance={}, nonce={}", account.balance, account.nonce));
            }
            Ok(None) => {
                trace.push("Sender account not found".to_string());
            }
            Err(e) => {
                trace.push(format!("Failed to get sender account: {}", e));
            }
        }

        // Get the receiver account
        match self.state_store.get_latest_account(&tx.recipient) {
            Ok(Some(account)) => {
                trace.push(format!("Receiver account: balance={}, nonce={}", account.balance, account.nonce));
            }
            Ok(None) => {
                trace.push("Receiver account not found".to_string());
            }
            Err(e) => {
                trace.push(format!("Failed to get receiver account: {}", e));
            }
        }

        trace
    }
}
