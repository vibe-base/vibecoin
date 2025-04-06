use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use log::{debug, error, info, warn};

use crate::mempool::types::{TransactionRecord, TransactionStatus, Address, Hash};
use crate::storage::state_store::{StateStore, AccountState};
use crate::crypto::signer::{verify_signature, VibeSignature};
use crate::crypto::keys::VibePublicKey;

/// Error types for mempool operations
#[derive(Debug, Clone, PartialEq)]
pub enum MempoolError {
    /// Transaction already exists in the mempool
    DuplicateTransaction,
    
    /// Transaction has an invalid signature
    InvalidSignature,
    
    /// Transaction has an invalid nonce
    InvalidNonce,
    
    /// Sender has insufficient balance
    InsufficientBalance,
    
    /// Mempool is full
    PoolFull,
    
    /// Sender has too many transactions in the mempool
    TooManyFromSender,
    
    /// Transaction is expired
    Expired,
    
    /// Other error
    Other(String),
}

/// Configuration for the mempool
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum number of transactions in the mempool
    pub max_size: usize,
    
    /// Maximum number of transactions per sender
    pub max_per_sender: usize,
    
    /// Maximum age of transactions in seconds
    pub max_transaction_age: u64,
    
    /// Maximum size of the included transaction set
    pub max_included_size: usize,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            max_per_sender: 100,
            max_transaction_age: 3600, // 1 hour
            max_included_size: 10000,
        }
    }
}

/// Thread-safe transaction pool (mempool)
pub struct Mempool {
    /// All transactions by hash
    transactions: DashMap<Hash, TransactionRecord>,
    
    /// Transactions by sender
    by_sender: DashMap<Address, HashSet<Hash>>,
    
    /// Transactions included in blocks
    included: DashMap<Hash, u64>, // tx_id -> block_height
    
    /// Prioritized transactions (for block production)
    priority_queue: Arc<RwLock<BinaryHeap<TransactionRecord>>>,
    
    /// State store for account validation
    state_store: Option<Arc<StateStore<'static>>>,
    
    /// Configuration
    config: MempoolConfig,
}

impl Mempool {
    /// Create a new mempool with default configuration
    pub fn new() -> Self {
        Self::with_config(MempoolConfig::default())
    }
    
    /// Create a new mempool with the given configuration
    pub fn with_config(config: MempoolConfig) -> Self {
        Self {
            transactions: DashMap::new(),
            by_sender: DashMap::new(),
            included: DashMap::new(),
            priority_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            state_store: None,
            config,
        }
    }
    
    /// Set the state store for account validation
    pub fn with_state_store(mut self, state_store: Arc<StateStore<'static>>) -> Self {
        self.state_store = Some(state_store);
        self
    }
    
    /// Insert a transaction into the mempool
    pub async fn insert(&self, tx: TransactionRecord) -> Result<(), MempoolError> {
        // Check if the transaction is already in the mempool
        if self.transactions.contains_key(&tx.tx_id) {
            return Err(MempoolError::DuplicateTransaction);
        }
        
        // Check if the transaction is already included in a block
        if self.included.contains_key(&tx.tx_id) {
            return Err(MempoolError::DuplicateTransaction);
        }
        
        // Check if the transaction is expired
        if tx.is_expired(self.config.max_transaction_age) {
            return Err(MempoolError::Expired);
        }
        
        // Check if the mempool is full
        if self.transactions.len() >= self.config.max_size {
            return Err(MempoolError::PoolFull);
        }
        
        // Check if the sender has too many transactions
        let sender_txs = self.by_sender.entry(tx.sender).or_insert(HashSet::new());
        if sender_txs.len() >= self.config.max_per_sender {
            return Err(MempoolError::TooManyFromSender);
        }
        
        // Validate the transaction if we have a state store
        if let Some(state_store) = &self.state_store {
            self.validate_transaction(&tx, state_store)?;
        }
        
        // Add the transaction to the mempool
        self.transactions.insert(tx.tx_id, tx.clone());
        sender_txs.insert(tx.tx_id);
        
        // Add to priority queue
        {
            let mut queue = self.priority_queue.write().await;
            queue.push(tx);
        }
        
        Ok(())
    }
    
    /// Validate a transaction against the current state
    fn validate_transaction(
        &self,
        tx: &TransactionRecord,
        state_store: &StateStore<'_>,
    ) -> Result<(), MempoolError> {
        // Get the sender's account state
        let account = match state_store.get_account_state(&tx.sender) {
            Some(account) => account,
            None => {
                // If the account doesn't exist, it has zero balance
                return Err(MempoolError::InsufficientBalance);
            }
        };
        
        // Check the nonce
        if tx.nonce < account.nonce || tx.nonce > account.nonce + 1 {
            return Err(MempoolError::InvalidNonce);
        }
        
        // Check the balance
        if account.balance < tx.total_cost() {
            return Err(MempoolError::InsufficientBalance);
        }
        
        // In a real implementation, we would verify the signature here
        // For now, we'll just assume it's valid
        
        Ok(())
    }
    
    /// Get pending transactions for inclusion in a block
    pub async fn get_pending(&self, max_count: usize) -> Vec<TransactionRecord> {
        let queue = self.priority_queue.read().await;
        
        // Clone the top transactions from the priority queue
        // In a real implementation, we would use a more efficient approach
        let mut result = Vec::new();
        let mut temp_queue = queue.clone();
        
        for _ in 0..max_count {
            if let Some(tx) = temp_queue.pop() {
                result.push(tx);
            } else {
                break;
            }
        }
        
        result
    }
    
    /// Remove transactions from the mempool
    pub async fn remove(&self, tx_ids: &[Hash]) {
        // Remove from the main maps
        for tx_id in tx_ids {
            if let Some((_, tx)) = self.transactions.remove(tx_id) {
                // Remove from by_sender
                if let Some(mut sender_txs) = self.by_sender.get_mut(&tx.sender) {
                    sender_txs.remove(tx_id);
                }
            }
        }
        
        // Rebuild the priority queue
        // In a real implementation, we would use a more efficient approach
        let mut queue = self.priority_queue.write().await;
        *queue = BinaryHeap::new();
        
        for tx in self.transactions.iter().map(|entry| entry.value().clone()) {
            queue.push(tx);
        }
    }
    
    /// Mark transactions as included in a block
    pub async fn mark_included(&self, tx_ids: &[Hash], block_height: u64) {
        // Add to included
        for tx_id in tx_ids {
            self.included.insert(*tx_id, block_height);
        }
        
        // Remove from pending
        self.remove(tx_ids).await;
    }
    
    /// Clean up expired transactions
    pub async fn cleanup_expired(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let expired: Vec<Hash> = self.transactions
            .iter()
            .filter(|entry| {
                let tx = entry.value();
                now - tx.timestamp > self.config.max_transaction_age
            })
            .map(|entry| *entry.key())
            .collect();
        
        if !expired.is_empty() {
            debug!("Removing {} expired transactions", expired.len());
            self.remove(&expired).await;
        }
    }
    
    /// Prune old included transactions
    pub fn prune_included(&self) {
        // If we're under the limit, do nothing
        if self.included.len() <= self.config.max_included_size {
            return;
        }
        
        // In a real implementation, we would remove the oldest transactions
        // For now, we'll just clear everything
        self.included.clear();
    }
    
    /// Get the status of a transaction
    pub fn get_transaction_status(&self, tx_id: &Hash) -> TransactionStatus {
        if self.included.contains_key(tx_id) {
            TransactionStatus::Included
        } else if let Some(tx) = self.transactions.get(tx_id) {
            if tx.is_expired(self.config.max_transaction_age) {
                TransactionStatus::Expired
            } else {
                TransactionStatus::Pending
            }
        } else {
            TransactionStatus::Rejected
        }
    }
    
    /// Get the number of pending transactions
    pub fn pending_count(&self) -> usize {
        self.transactions.len()
    }
    
    /// Get the number of included transactions
    pub fn included_count(&self) -> usize {
        self.included.len()
    }
    
    /// Get the number of transactions from a specific sender
    pub fn sender_count(&self, sender: &Address) -> usize {
        match self.by_sender.get(sender) {
            Some(txs) => txs.len(),
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mempool::types::TransactionRecord;
    
    #[tokio::test]
    async fn test_insert_and_get() {
        // Create a mempool
        let mempool = Mempool::new();
        
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
        assert!(mempool.insert(tx.clone()).await.is_ok());
        
        // Check pending count
        assert_eq!(mempool.pending_count(), 1);
        
        // Get pending transactions
        let pending = mempool.get_pending(10).await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].tx_id, tx.tx_id);
    }
    
    #[tokio::test]
    async fn test_duplicate_transaction() {
        // Create a mempool
        let mempool = Mempool::new();
        
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
        assert!(mempool.insert(tx.clone()).await.is_ok());
        
        // Try to insert the same transaction again
        let result = mempool.insert(tx.clone()).await;
        assert_eq!(result, Err(MempoolError::DuplicateTransaction));
    }
    
    #[tokio::test]
    async fn test_remove_transaction() {
        // Create a mempool
        let mempool = Mempool::new();
        
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
        assert!(mempool.insert(tx.clone()).await.is_ok());
        
        // Remove the transaction
        mempool.remove(&[tx.tx_id]).await;
        
        // Check pending count
        assert_eq!(mempool.pending_count(), 0);
    }
    
    #[tokio::test]
    async fn test_mark_included() {
        // Create a mempool
        let mempool = Mempool::new();
        
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
        assert!(mempool.insert(tx.clone()).await.is_ok());
        
        // Mark the transaction as included
        mempool.mark_included(&[tx.tx_id], 1).await;
        
        // Check pending count
        assert_eq!(mempool.pending_count(), 0);
        
        // Check included count
        assert_eq!(mempool.included_count(), 1);
        
        // Check transaction status
        assert_eq!(mempool.get_transaction_status(&tx.tx_id), TransactionStatus::Included);
    }
    
    #[tokio::test]
    async fn test_transaction_priority() {
        // Create a mempool
        let mempool = Mempool::new();
        
        // Create transactions with different gas prices
        let tx1 = TransactionRecord::new(
            [1u8; 32],
            [2u8; 32],
            100,
            10, // Lower gas price
            1000,
            1,
            None,
        );
        
        let tx2 = TransactionRecord::new(
            [3u8; 32],
            [4u8; 32],
            200,
            20, // Higher gas price
            1000,
            1,
            None,
        );
        
        // Insert the transactions
        assert!(mempool.insert(tx1.clone()).await.is_ok());
        assert!(mempool.insert(tx2.clone()).await.is_ok());
        
        // Get pending transactions
        let pending = mempool.get_pending(10).await;
        
        // The transaction with higher gas price should come first
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].tx_id, tx2.tx_id);
        assert_eq!(pending[1].tx_id, tx1.tx_id);
    }
    
    #[tokio::test]
    async fn test_max_per_sender() {
        // Create a mempool with a low max_per_sender
        let config = MempoolConfig {
            max_per_sender: 2,
            ..Default::default()
        };
        let mempool = Mempool::with_config(config);
        
        // Create transactions from the same sender
        let tx1 = TransactionRecord::new(
            [1u8; 32], // Same sender
            [2u8; 32],
            100,
            10,
            1000,
            1,
            None,
        );
        
        let tx2 = TransactionRecord::new(
            [1u8; 32], // Same sender
            [2u8; 32],
            200,
            20,
            1000,
            2,
            None,
        );
        
        let tx3 = TransactionRecord::new(
            [1u8; 32], // Same sender
            [2u8; 32],
            300,
            30,
            1000,
            3,
            None,
        );
        
        // Insert the first two transactions
        assert!(mempool.insert(tx1.clone()).await.is_ok());
        assert!(mempool.insert(tx2.clone()).await.is_ok());
        
        // The third transaction should be rejected
        let result = mempool.insert(tx3.clone()).await;
        assert_eq!(result, Err(MempoolError::TooManyFromSender));
    }
}
