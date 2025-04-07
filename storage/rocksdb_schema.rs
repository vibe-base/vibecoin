use std::fmt;
use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::block_store::Hash;

/// Schema for RocksDB keys
///
/// This module defines the schema for all keys used in the RocksDB database.
/// It provides methods to create and parse keys for different types of data.
pub struct Schema;

impl Schema {
    // Block-related keys
    
    /// Create a key for a block by height
    pub fn block_by_height_key(height: u64) -> String {
        format!("block:{}", height)
    }
    
    /// Create a key for a block by hash
    pub fn block_by_hash_key(hash: &Hash) -> String {
        format!("block_hash:{}", hex::encode(hash))
    }
    
    /// Create a key for the latest block height
    pub fn latest_block_height_key() -> &'static str {
        "meta:latest_block_height"
    }
    
    // Transaction-related keys
    
    /// Create a key for a transaction by hash
    pub fn tx_by_hash_key(hash: &Hash) -> String {
        format!("tx:{}", hex::encode(hash))
    }
    
    /// Create a key for transactions by block
    pub fn tx_by_block_key(block_height: u64, tx_hash: &Hash) -> String {
        format!("tx_block:{}:{}", block_height, hex::encode(tx_hash))
    }
    
    /// Create a key for transactions by sender
    pub fn tx_by_sender_key(sender: &Hash, tx_hash: &Hash) -> String {
        format!("tx_sender:{}:{}", hex::encode(sender), hex::encode(tx_hash))
    }
    
    /// Create a key for transactions by recipient
    pub fn tx_by_recipient_key(recipient: &Hash, tx_hash: &Hash) -> String {
        format!("tx_recipient:{}:{}", hex::encode(recipient), hex::encode(tx_hash))
    }
    
    /// Create a key for the latest nonce of a sender
    pub fn tx_sender_nonce_key(sender: &Hash) -> String {
        format!("tx_sender_nonce:{}", hex::encode(sender))
    }
    
    // Account state-related keys
    
    /// Create a key for an account state
    pub fn account_state_key(address: &Hash) -> String {
        format!("state:{}", hex::encode(address))
    }
    
    /// Create a key for a contract storage slot
    pub fn contract_storage_key(address: &Hash, slot: &[u8]) -> String {
        format!("state_storage:{}:{}", hex::encode(address), hex::encode(slot))
    }
    
    /// Create a key for a state root
    pub fn state_root_key(block_height: u64) -> String {
        format!("state_root:{}", block_height)
    }
    
    /// Create a key for the latest state root
    pub fn latest_state_root_key() -> &'static str {
        "meta:latest_state_root"
    }
    
    // PoH-related keys
    
    /// Create a key for a PoH entry
    pub fn poh_entry_key(seq: u64) -> String {
        format!("poh:{}", seq)
    }
    
    /// Create a key for the latest PoH sequence
    pub fn latest_poh_seq_key() -> &'static str {
        "meta:latest_poh_seq"
    }
    
    // MPT-related keys
    
    /// Create a key for an MPT node
    pub fn mpt_node_key(hash: &Hash) -> String {
        format!("mpt_node:{}", hex::encode(hash))
    }
    
    // Metadata keys
    
    /// Create a key for a metadata value
    pub fn metadata_key(key: &str) -> String {
        format!("meta:{}", key)
    }
    
    // Key parsing
    
    /// Parse a key to determine its type
    pub fn parse_key(key: &[u8]) -> KeyType {
        let key_str = match std::str::from_utf8(key) {
            Ok(s) => s,
            Err(_) => return KeyType::Unknown,
        };
        
        if key_str.starts_with("block:") {
            KeyType::BlockByHeight
        } else if key_str.starts_with("block_hash:") {
            KeyType::BlockByHash
        } else if key_str.starts_with("tx:") {
            KeyType::TransactionByHash
        } else if key_str.starts_with("tx_block:") {
            KeyType::TransactionByBlock
        } else if key_str.starts_with("tx_sender:") {
            KeyType::TransactionBySender
        } else if key_str.starts_with("tx_recipient:") {
            KeyType::TransactionByRecipient
        } else if key_str.starts_with("tx_sender_nonce:") {
            KeyType::TransactionSenderNonce
        } else if key_str.starts_with("state:") {
            KeyType::AccountState
        } else if key_str.starts_with("state_storage:") {
            KeyType::ContractStorage
        } else if key_str.starts_with("state_root:") {
            KeyType::StateRoot
        } else if key_str.starts_with("poh:") {
            KeyType::PoHEntry
        } else if key_str.starts_with("mpt_node:") {
            KeyType::MPTNode
        } else if key_str.starts_with("meta:") {
            KeyType::Metadata
        } else {
            KeyType::Unknown
        }
    }
    
    /// Extract the block height from a block key
    pub fn extract_block_height(key: &str) -> Option<u64> {
        if !key.starts_with("block:") {
            return None;
        }
        
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        
        parts[1].parse::<u64>().ok()
    }
    
    /// Extract the transaction hash from a transaction key
    pub fn extract_tx_hash(key: &str) -> Option<Hash> {
        if !key.starts_with("tx:") {
            return None;
        }
        
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        
        let hash_hex = parts[1];
        let hash_bytes = hex::decode(hash_hex).ok()?;
        
        if hash_bytes.len() != 32 {
            return None;
        }
        
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes);
        Some(hash)
    }
    
    /// Extract the account address from an account state key
    pub fn extract_account_address(key: &str) -> Option<Hash> {
        if !key.starts_with("state:") {
            return None;
        }
        
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        
        let addr_hex = parts[1];
        let addr_bytes = hex::decode(addr_hex).ok()?;
        
        if addr_bytes.len() != 32 {
            return None;
        }
        
        let mut addr = [0u8; 32];
        addr.copy_from_slice(&addr_bytes);
        Some(addr)
    }
}

/// Type of key in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Block by height
    BlockByHeight,
    /// Block by hash
    BlockByHash,
    /// Transaction by hash
    TransactionByHash,
    /// Transaction by block
    TransactionByBlock,
    /// Transaction by sender
    TransactionBySender,
    /// Transaction by recipient
    TransactionByRecipient,
    /// Transaction sender nonce
    TransactionSenderNonce,
    /// Account state
    AccountState,
    /// Contract storage
    ContractStorage,
    /// State root
    StateRoot,
    /// PoH entry
    PoHEntry,
    /// MPT node
    MPTNode,
    /// Metadata
    Metadata,
    /// Unknown key type
    Unknown,
}

impl fmt::Display for KeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyType::BlockByHeight => write!(f, "BlockByHeight"),
            KeyType::BlockByHash => write!(f, "BlockByHash"),
            KeyType::TransactionByHash => write!(f, "TransactionByHash"),
            KeyType::TransactionByBlock => write!(f, "TransactionByBlock"),
            KeyType::TransactionBySender => write!(f, "TransactionBySender"),
            KeyType::TransactionByRecipient => write!(f, "TransactionByRecipient"),
            KeyType::TransactionSenderNonce => write!(f, "TransactionSenderNonce"),
            KeyType::AccountState => write!(f, "AccountState"),
            KeyType::ContractStorage => write!(f, "ContractStorage"),
            KeyType::StateRoot => write!(f, "StateRoot"),
            KeyType::PoHEntry => write!(f, "PoHEntry"),
            KeyType::MPTNode => write!(f, "MPTNode"),
            KeyType::Metadata => write!(f, "Metadata"),
            KeyType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Database statistics
#[derive(Debug, Clone, Default)]
pub struct DatabaseStats {
    /// Number of blocks
    pub block_count: u64,
    /// Number of transactions
    pub transaction_count: u64,
    /// Number of accounts
    pub account_count: u64,
    /// Number of contract storage entries
    pub contract_storage_count: u64,
    /// Number of state roots
    pub state_root_count: u64,
    /// Number of PoH entries
    pub poh_entry_count: u64,
    /// Number of MPT nodes
    pub mpt_node_count: u64,
    /// Number of metadata entries
    pub metadata_count: u64,
    /// Total database size in bytes
    pub total_size_bytes: u64,
}

/// Database manager for RocksDB
pub struct RocksDBManager<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,
}

impl<'a> RocksDBManager<'a> {
    /// Create a new RocksDBManager
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }
    
    /// Get database statistics
    pub fn get_stats(&self) -> Result<DatabaseStats, KVStoreError> {
        let mut stats = DatabaseStats::default();
        
        // Count blocks
        let block_prefix = "block:".as_bytes();
        let blocks = self.store.scan_prefix(block_prefix)?;
        stats.block_count = blocks.len() as u64;
        
        // Count transactions
        let tx_prefix = "tx:".as_bytes();
        let txs = self.store.scan_prefix(tx_prefix)?;
        stats.transaction_count = txs.len() as u64;
        
        // Count accounts
        let account_prefix = "state:".as_bytes();
        let accounts = self.store.scan_prefix(account_prefix)?;
        stats.account_count = accounts.len() as u64;
        
        // Count contract storage entries
        let storage_prefix = "state_storage:".as_bytes();
        let storage_entries = self.store.scan_prefix(storage_prefix)?;
        stats.contract_storage_count = storage_entries.len() as u64;
        
        // Count state roots
        let state_root_prefix = "state_root:".as_bytes();
        let state_roots = self.store.scan_prefix(state_root_prefix)?;
        stats.state_root_count = state_roots.len() as u64;
        
        // Count PoH entries
        let poh_prefix = "poh:".as_bytes();
        let poh_entries = self.store.scan_prefix(poh_prefix)?;
        stats.poh_entry_count = poh_entries.len() as u64;
        
        // Count MPT nodes
        let mpt_prefix = "mpt_node:".as_bytes();
        let mpt_nodes = self.store.scan_prefix(mpt_prefix)?;
        stats.mpt_node_count = mpt_nodes.len() as u64;
        
        // Count metadata entries
        let meta_prefix = "meta:".as_bytes();
        let meta_entries = self.store.scan_prefix(meta_prefix)?;
        stats.metadata_count = meta_entries.len() as u64;
        
        // Calculate total size
        let all_entries = self.store.scan_prefix(&[])?;
        stats.total_size_bytes = all_entries.iter()
            .map(|(k, v)| k.len() as u64 + v.len() as u64)
            .sum();
        
        Ok(stats)
    }
    
    /// Compact the database
    pub fn compact(&self) -> Result<(), KVStoreError> {
        // This would call the RocksDB compact_range method
        // For now, we'll just log that compaction was requested
        info!("Database compaction requested");
        Ok(())
    }
    
    /// Backup the database
    pub fn backup(&self, backup_path: &str) -> Result<(), KVStoreError> {
        // This would create a backup of the RocksDB database
        // For now, we'll just log that backup was requested
        info!("Database backup requested to path: {}", backup_path);
        Ok(())
    }
    
    /// Restore the database from a backup
    pub fn restore(&self, backup_path: &str) -> Result<(), KVStoreError> {
        // This would restore the RocksDB database from a backup
        // For now, we'll just log that restore was requested
        info!("Database restore requested from path: {}", backup_path);
        Ok(())
    }
    
    /// Prune old data
    pub fn prune(&self, keep_blocks: u64) -> Result<(), KVStoreError> {
        // Get the latest block height
        let latest_height_key = Schema::latest_block_height_key();
        let latest_height_bytes = match self.store.get(latest_height_key.as_bytes())? {
            Some(bytes) => bytes,
            None => {
                info!("No blocks to prune");
                return Ok(());
            }
        };
        
        if latest_height_bytes.len() != 8 {
            return Err(KVStoreError::InvalidDataFormat(
                "Invalid latest block height format".to_string()
            ));
        }
        
        let mut height_arr = [0u8; 8];
        height_arr.copy_from_slice(&latest_height_bytes);
        let latest_height = u64::from_be_bytes(height_arr);
        
        // Calculate the pruning threshold
        if latest_height <= keep_blocks {
            info!("Not enough blocks to prune");
            return Ok(());
        }
        
        let prune_below = latest_height - keep_blocks;
        info!("Pruning blocks below height {}", prune_below);
        
        // Create a batch for all deletions
        let mut batch = Vec::new();
        
        // Prune blocks
        for height in 0..prune_below {
            let block_key = Schema::block_by_height_key(height);
            batch.push(WriteBatchOperation::Delete {
                key: block_key.as_bytes().to_vec(),
            });
            
            // Also prune state roots
            let state_root_key = Schema::state_root_key(height);
            batch.push(WriteBatchOperation::Delete {
                key: state_root_key.as_bytes().to_vec(),
            });
        }
        
        // Execute the batch
        self.store.write_batch(batch)?;
        
        info!("Pruned {} blocks", prune_below);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    
    #[test]
    fn test_schema_keys() {
        // Test block keys
        let block_key = Schema::block_by_height_key(123);
        assert_eq!(block_key, "block:123");
        
        let hash = [1u8; 32];
        let block_hash_key = Schema::block_by_hash_key(&hash);
        assert_eq!(block_hash_key, "block_hash:0101010101010101010101010101010101010101010101010101010101010101");
        
        // Test transaction keys
        let tx_key = Schema::tx_by_hash_key(&hash);
        assert_eq!(tx_key, "tx:0101010101010101010101010101010101010101010101010101010101010101");
        
        let tx_block_key = Schema::tx_by_block_key(123, &hash);
        assert_eq!(tx_block_key, "tx_block:123:0101010101010101010101010101010101010101010101010101010101010101");
        
        // Test account keys
        let account_key = Schema::account_state_key(&hash);
        assert_eq!(account_key, "state:0101010101010101010101010101010101010101010101010101010101010101");
        
        let storage_key = Schema::contract_storage_key(&hash, &[2u8; 32]);
        assert_eq!(storage_key, "state_storage:0101010101010101010101010101010101010101010101010101010101010101:0202020202020202020202020202020202020202020202020202020202020202");
    }
    
    #[test]
    fn test_key_parsing() {
        // Test block key parsing
        let block_key = "block:123";
        assert_eq!(Schema::parse_key(block_key.as_bytes()), KeyType::BlockByHeight);
        assert_eq!(Schema::extract_block_height(block_key), Some(123));
        
        // Test transaction key parsing
        let tx_key = "tx:0101010101010101010101010101010101010101010101010101010101010101";
        assert_eq!(Schema::parse_key(tx_key.as_bytes()), KeyType::TransactionByHash);
        
        let hash = [1u8; 32];
        assert_eq!(Schema::extract_tx_hash(tx_key), Some(hash));
        
        // Test account key parsing
        let account_key = "state:0101010101010101010101010101010101010101010101010101010101010101";
        assert_eq!(Schema::parse_key(account_key.as_bytes()), KeyType::AccountState);
        assert_eq!(Schema::extract_account_address(account_key), Some(hash));
    }
    
    #[test]
    fn test_database_manager() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path()).unwrap();
        let manager = RocksDBManager::new(&store);
        
        // Test getting stats on an empty database
        let stats = manager.get_stats().unwrap();
        assert_eq!(stats.block_count, 0);
        assert_eq!(stats.transaction_count, 0);
        assert_eq!(stats.account_count, 0);
        
        // Add some data
        store.put(Schema::block_by_height_key(1).as_bytes(), &[1]).unwrap();
        store.put(Schema::block_by_height_key(2).as_bytes(), &[2]).unwrap();
        store.put(Schema::tx_by_hash_key(&[1u8; 32]).as_bytes(), &[3]).unwrap();
        store.put(Schema::account_state_key(&[2u8; 32]).as_bytes(), &[4]).unwrap();
        
        // Test getting stats with data
        let stats = manager.get_stats().unwrap();
        assert_eq!(stats.block_count, 2);
        assert_eq!(stats.transaction_count, 1);
        assert_eq!(stats.account_count, 1);
        
        // Test compaction
        manager.compact().unwrap();
        
        // Test pruning
        store.put(Schema::latest_block_height_key().as_bytes(), &10u64.to_be_bytes()).unwrap();
        manager.prune(5).unwrap();
    }
}
