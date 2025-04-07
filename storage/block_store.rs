use serde::{Serialize, Deserialize};
use std::sync::Arc;
use log::{debug, error, info, warn};
use hex;

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};

/// Type alias for a 32-byte hash
pub type Hash = [u8; 32];

/// Block structure representing a block in the blockchain
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Block {
    /// Block height
    pub height: u64,

    /// Block hash
    pub hash: Hash,

    /// Previous block hash
    pub prev_hash: Hash,

    /// Block timestamp
    pub timestamp: u64,

    /// Transaction IDs included in this block
    pub transactions: Vec<Hash>,

    /// State root hash (Merkle root of the state trie)
    pub state_root: Hash,

    /// Proof of Work nonce
    pub nonce: u64,

    /// Proof of History sequence number
    pub poh_seq: u64,

    /// Proof of History hash
    pub poh_hash: Hash,

    /// Block difficulty
    pub difficulty: u64,

    /// Total cumulative difficulty
    pub total_difficulty: u128,
}

/// Error type for BlockStore operations
#[derive(Debug, thiserror::Error)]
pub enum BlockStoreError {
    /// KVStore error
    #[error("KVStore error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Block not found
    #[error("Block not found: {0}")]
    BlockNotFound(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Store for blockchain blocks
pub struct BlockStore<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// The latest block height (cached)
    latest_height: std::sync::RwLock<Option<u64>>,
}

impl<'a> BlockStore<'a> {
    /// Create a new BlockStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self {
            store,
            latest_height: std::sync::RwLock::new(None),
        }
    }

    /// Store a block, indexing by both height and hash
    pub fn put_block(&self, block: &Block) -> Result<(), BlockStoreError> {
        let value = bincode::serialize(block)
            .map_err(|e| BlockStoreError::SerializationError(e.to_string()))?;

        // Create a batch operation
        let mut batch = WriteBatchOperation::new();

        // Index by height
        let height_key = format!("block:height:{}", block.height);
        batch.put(height_key.as_bytes().to_vec(), value.clone());

        // Index by hash
        let hash_key = format!("block:hash:{}", hex::encode(&block.hash));
        batch.put(hash_key.as_bytes().to_vec(), value);

        // Execute the batch
        self.store.write_batch(batch)?;

        // Update the latest height cache if needed
        let mut latest_height = self.latest_height.write().unwrap();
        if latest_height.is_none() || latest_height.unwrap() < block.height {
            *latest_height = Some(block.height);
        }

        debug!("Stored block at height {}: {:?}", block.height, hex::encode(&block.hash));
        Ok(())
    }

    /// Retrieve a block by its height
    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        let key = format!("block:height:{}", height);
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                match bincode::deserialize(&bytes) {
                    Ok(block) => Some(block),
                    Err(e) => {
                        error!("Failed to deserialize block at height {}: {}", height, e);
                        None
                    }
                }
            },
            Ok(None) => None,
            Err(e) => {
                error!("Failed to get block at height {}: {}", height, e);
                None
            }
        }
    }

    /// Retrieve a block by its hash
    pub fn get_block_by_hash(&self, hash: &Hash) -> Option<Block> {
        let key = format!("block:hash:{}", hex::encode(hash));
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                match bincode::deserialize(&bytes) {
                    Ok(block) => Some(block),
                    Err(e) => {
                        error!("Failed to deserialize block with hash {}: {}", hex::encode(hash), e);
                        None
                    }
                }
            },
            Ok(None) => None,
            Err(e) => {
                error!("Failed to get block with hash {}: {}", hex::encode(hash), e);
                None
            }
        }
    }

    /// Get the latest block height
    pub fn get_latest_height(&self) -> Option<u64> {
        // Check the cache first
        {
            let latest_height = self.latest_height.read().unwrap();
            if latest_height.is_some() {
                return *latest_height;
            }
        }

        // Scan for the highest height
        let prefix = b"block:height:";
        match self.store.scan_prefix(prefix) {
            Ok(entries) => {
                let height = entries.iter()
                    .filter_map(|(key, _)| {
                        let key_str = std::str::from_utf8(key).ok()?;
                        let height_str = key_str.strip_prefix("block:height:")?;
                        height_str.parse::<u64>().ok()
                    })
                    .max();

                // Update the cache
                if height.is_some() {
                    let mut latest_height = self.latest_height.write().unwrap();
                    *latest_height = height;
                }

                height
            },
            Err(e) => {
                error!("Failed to scan for latest height: {}", e);
                None
            }
        }
    }

    /// Get the latest block
    pub fn get_latest_block(&self) -> Option<Block> {
        self.get_latest_height().and_then(|height| self.get_block_by_height(height))
    }

    /// Get a range of blocks by height
    pub fn get_blocks_by_height_range(&self, start: u64, end: u64) -> Vec<Block> {
        let mut blocks = Vec::new();
        for height in start..=end {
            if let Some(block) = self.get_block_by_height(height) {
                blocks.push(block);
            }
        }
        blocks
    }

    /// Check if a block exists by hash
    pub fn has_block_by_hash(&self, hash: &Hash) -> bool {
        let key = format!("block:hash:{}", hex::encode(hash));
        match self.store.get(key.as_bytes()) {
            Ok(Some(_)) => true,
            _ => false,
        }
    }

    /// Check if a block exists by height
    pub fn has_block_by_height(&self, height: u64) -> bool {
        let key = format!("block:height:{}", height);
        match self.store.get(key.as_bytes()) {
            Ok(Some(_)) => true,
            _ => false,
        }
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), BlockStoreError> {
        self.store.flush().map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[test]
    fn test_block_store() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);

        // Create a test block
        let block = Block {
            height: 1,
            hash: [1; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![[2; 32], [3; 32]],
            state_root: [4; 32],
            nonce: 42,
            poh_seq: 100,
            poh_hash: [5; 32],
            difficulty: 1000,
            total_difficulty: 1000,
        };

        // Store the block
        block_store.put_block(&block).unwrap();

        // Retrieve by height
        let retrieved = block_store.get_block_by_height(1).unwrap();
        assert_eq!(retrieved, block);

        // Retrieve by hash
        let retrieved = block_store.get_block_by_hash(&[1; 32]).unwrap();
        assert_eq!(retrieved, block);

        // Test latest height
        assert_eq!(block_store.get_latest_height(), Some(1));

        // Test latest block
        let latest = block_store.get_latest_block().unwrap();
        assert_eq!(latest, block);

        // Test has_block methods
        assert!(block_store.has_block_by_height(1));
        assert!(block_store.has_block_by_hash(&[1; 32]));
        assert!(!block_store.has_block_by_height(2));
        assert!(!block_store.has_block_by_hash(&[2; 32]));

        // Test flush
        block_store.flush().unwrap();
    }

    #[test]
    fn test_multiple_blocks() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);

        // Create and store multiple blocks
        for i in 0..5 {
            let block = Block {
                height: i,
                hash: [i as u8; 32],
                prev_hash: if i == 0 { [0; 32] } else { [(i-1) as u8; 32] },
                timestamp: 12345 + i,
                transactions: vec![[i as u8 + 1; 32]],
                state_root: [i as u8 + 2; 32],
                nonce: 42 + i,
                poh_seq: 100 + i,
                poh_hash: [i as u8 + 3; 32],
                difficulty: 1000 + i,
                total_difficulty: 1000 + (i as u128 * 1000),
            };

            block_store.put_block(&block).unwrap();
        }

        // Test get_latest_height
        assert_eq!(block_store.get_latest_height(), Some(4));

        // Test get_blocks_by_height_range
        let blocks = block_store.get_blocks_by_height_range(1, 3);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].height, 1);
        assert_eq!(blocks[1].height, 2);
        assert_eq!(blocks[2].height, 3);
    }
}
