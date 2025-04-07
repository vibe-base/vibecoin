use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use crate::storage::tx_store::TransactionRecord;

/// Memory pool for pending transactions
pub struct Mempool {
    /// Pending transactions
    pending: Mutex<HashMap<[u8; 32], TransactionRecord>>,
    
    /// Transactions included in blocks
    included: Mutex<HashSet<[u8; 32]>>,
    
    /// Maximum number of transactions in the mempool
    max_size: usize,
}

impl Mempool {
    /// Create a new mempool
    pub fn new(max_size: usize) -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            included: Mutex::new(HashSet::new()),
            max_size,
        }
    }
    
    /// Add a transaction to the mempool
    pub fn add_transaction(&self, tx: TransactionRecord) -> bool {
        let tx_id = tx.tx_id;
        
        // Check if the transaction is already included in a block
        {
            let included = self.included.lock().unwrap();
            if included.contains(&tx_id) {
                return false;
            }
        }
        
        // Add the transaction to the pending set
        let mut pending = self.pending.lock().unwrap();
        
        // Check if the mempool is full
        if pending.len() >= self.max_size {
            // In a real implementation, we would evict the lowest fee transactions
            // For now, we'll just reject the transaction
            return false;
        }
        
        pending.insert(tx_id, tx);
        true
    }
    
    /// Remove a transaction from the mempool
    pub fn remove_transaction(&self, tx_id: &[u8; 32]) -> bool {
        let mut pending = self.pending.lock().unwrap();
        pending.remove(tx_id).is_some()
    }
    
    /// Mark a transaction as included in a block
    pub fn mark_included(&self, tx_id: &[u8; 32]) {
        // Remove from pending
        {
            let mut pending = self.pending.lock().unwrap();
            pending.remove(tx_id);
        }
        
        // Add to included
        {
            let mut included = self.included.lock().unwrap();
            included.insert(*tx_id);
        }
    }
    
    /// Get pending transactions up to a limit
    pub fn get_pending_transactions(&self, limit: usize) -> Vec<TransactionRecord> {
        let pending = self.pending.lock().unwrap();
        
        pending.values()
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Get the number of pending transactions
    pub fn pending_count(&self) -> usize {
        let pending = self.pending.lock().unwrap();
        pending.len()
    }
    
    /// Clear old included transactions
    pub fn prune_included(&self, max_included: usize) {
        let mut included = self.included.lock().unwrap();
        
        // If we're under the limit, do nothing
        if included.len() <= max_included {
            return;
        }
        
        // In a real implementation, we would remove the oldest transactions
        // For now, we'll just clear everything
        included.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mempool() {
        // Create a mempool
        let mempool = Mempool::new(100);
        
        // Create some transactions
        let tx1 = TransactionRecord {
            tx_id: [1u8; 32],
            sender: [2u8; 32],
            recipient: [3u8; 32],
            value: 100,
            gas_used: 10,
            block_height: 0, // Not yet included in a block
        };
        
        let tx2 = TransactionRecord {
            tx_id: [2u8; 32],
            sender: [2u8; 32],
            recipient: [3u8; 32],
            value: 200,
            gas_used: 10,
            block_height: 0,
        };
        
        // Add the transactions
        assert!(mempool.add_transaction(tx1.clone()));
        assert!(mempool.add_transaction(tx2.clone()));
        
        // Check pending count
        assert_eq!(mempool.pending_count(), 2);
        
        // Get pending transactions
        let pending = mempool.get_pending_transactions(10);
        assert_eq!(pending.len(), 2);
        
        // Mark tx1 as included
        mempool.mark_included(&tx1.tx_id);
        
        // Check pending count
        assert_eq!(mempool.pending_count(), 1);
        
        // Try to add tx1 again
        assert!(!mempool.add_transaction(tx1.clone()));
        
        // Prune included transactions
        mempool.prune_included(0);
        
        // Now we should be able to add tx1 again
        assert!(mempool.add_transaction(tx1));
        
        // Check pending count
        assert_eq!(mempool.pending_count(), 2);
    }
}
