use serde::{Serialize, Deserialize};
use crate::storage::kv_store::KVStore;

/// Transaction record structure
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TransactionRecord {
    pub tx_id: [u8; 32],
    pub sender: [u8; 32],
    pub recipient: [u8; 32],
    pub value: u64,
    pub gas_used: u64,
    pub block_height: u64,
}

/// Store for transaction records
pub struct TxStore<'a> {
    store: &'a dyn KVStore,
}

impl<'a> TxStore<'a> {
    /// Create a new TxStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }

    /// Store a transaction record
    pub fn put_transaction(&self, tx: &TransactionRecord) {
        let key = format!("tx:id:{:x?}", tx.tx_id);
        let value = bincode::serialize(tx).unwrap();
        self.store.put(key.as_bytes(), &value);
        
        // Index by sender
        let sender_key = format!("tx:sender:{:x?}:{:x?}", tx.sender, tx.tx_id);
        self.store.put(sender_key.as_bytes(), &value);
        
        // Index by recipient
        let recipient_key = format!("tx:recipient:{:x?}:{:x?}", tx.recipient, tx.tx_id);
        self.store.put(recipient_key.as_bytes(), &value);
        
        // Index by block
        let block_key = format!("tx:block:{}:{:x?}", tx.block_height, tx.tx_id);
        self.store.put(block_key.as_bytes(), &value);
    }

    /// Retrieve a transaction by its ID
    pub fn get_transaction(&self, tx_id: &[u8]) -> Option<TransactionRecord> {
        let key = format!("tx:id:{:x?}", tx_id);
        self.store.get(key.as_bytes())
            .and_then(|bytes| bincode::deserialize(&bytes).ok())
    }
    
    /// Get all transactions for a specific sender
    pub fn get_transactions_by_sender(&self, sender: &[u8]) -> Vec<TransactionRecord> {
        let prefix = format!("tx:sender:{:x?}:", sender);
        self.store.scan_prefix(prefix.as_bytes())
            .iter()
            .filter_map(|(_, v)| bincode::deserialize(v).ok())
            .collect()
    }
    
    /// Get all transactions for a specific recipient
    pub fn get_transactions_by_recipient(&self, recipient: &[u8]) -> Vec<TransactionRecord> {
        let prefix = format!("tx:recipient:{:x?}:", recipient);
        self.store.scan_prefix(prefix.as_bytes())
            .iter()
            .filter_map(|(_, v)| bincode::deserialize(v).ok())
            .collect()
    }
    
    /// Get all transactions in a specific block
    pub fn get_transactions_by_block(&self, block_height: u64) -> Vec<TransactionRecord> {
        let prefix = format!("tx:block:{}:", block_height);
        self.store.scan_prefix(prefix.as_bytes())
            .iter()
            .filter_map(|(_, v)| bincode::deserialize(v).ok())
            .collect()
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
            gas_used: 10,
            block_height: 5,
        };
        
        // Store the transaction
        tx_store.put_transaction(&tx);
        
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
    }
}
