use crate::ledger::transaction::Transaction;
use crate::types::error::VibecoinError;
use crate::types::primitives::Hash;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of transactions to include in a block
pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 100;

/// Transaction pool (mempool) for storing pending transactions
pub struct TransactionPool {
    /// Transactions indexed by their hash
    transactions: HashMap<Hash, Transaction>,
    /// Timestamp when each transaction was added to the pool
    timestamps: HashMap<Hash, u64>,
}

impl TransactionPool {
    /// Create a new empty transaction pool
    pub fn new() -> Self {
        TransactionPool {
            transactions: HashMap::new(),
            timestamps: HashMap::new(),
        }
    }

    /// Add a transaction to the pool
    pub fn add_transaction(&mut self, transaction: Transaction) -> Result<(), VibecoinError> {
        // Validate the transaction
        transaction.is_valid()?;

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VibecoinError::InvalidTimestamp)?
            .as_secs();

        // Add to pool if not already present
        if !self.transactions.contains_key(&transaction.hash) {
            let hash = transaction.hash; // Store hash before moving transaction
            self.transactions.insert(hash, transaction);
            self.timestamps.insert(hash, now);
        }

        Ok(())
    }

    /// Get a transaction by its hash
    pub fn get_transaction(&self, hash: &Hash) -> Option<&Transaction> {
        self.transactions.get(hash)
    }

    /// Remove a transaction from the pool
    pub fn remove_transaction(&mut self, hash: &Hash) -> Option<Transaction> {
        self.timestamps.remove(hash);
        self.transactions.remove(hash)
    }

    /// Select transactions for inclusion in a block
    pub fn select_transactions(&self, max_count: usize) -> Vec<Transaction> {
        // Sort transactions by fee (in a real implementation)
        // For now, we'll just take the first max_count transactions
        self.transactions.values()
            .take(max_count)
            .cloned()
            .collect()
    }

    /// Remove transactions that were included in a block
    pub fn remove_included_transactions(&mut self, transactions: &[Transaction]) {
        for tx in transactions {
            self.remove_transaction(&tx.hash);
        }
    }

    /// Clean up old transactions
    pub fn cleanup_old_transactions(&mut self, max_age_seconds: u64) -> usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let old_tx_hashes: Vec<Hash> = self.timestamps.iter()
            .filter(|&(_, &timestamp)| now - timestamp > max_age_seconds)
            .map(|(hash, _)| *hash)
            .collect();

        let count = old_tx_hashes.len();

        for hash in old_tx_hashes {
            self.remove_transaction(&hash);
        }

        count
    }

    /// Get the number of transactions in the pool
    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    /// Get all transactions in the pool
    pub fn get_all_transactions(&self) -> Vec<&Transaction> {
        self.transactions.values().collect()
    }
}
