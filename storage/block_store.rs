use serde::{Serialize, Deserialize};
use crate::storage::kv_store::KVStore;

/// Block structure representing a block in the blockchain
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Block {
    pub height: u64,
    pub hash: [u8; 32],
    pub prev_hash: [u8; 32],
    pub timestamp: u64,
    pub transactions: Vec<[u8; 32]>, // tx hashes
    pub state_root: [u8; 32],
}

/// Store for blockchain blocks
pub struct BlockStore<'a> {
    store: &'a dyn KVStore,
}

impl<'a> BlockStore<'a> {
    /// Create a new BlockStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }

    /// Store a block, indexing by both height and hash
    pub fn put_block(&self, block: &Block) {
        let key = format!("block:height:{}", block.height);
        let value = bincode::serialize(block).unwrap();
        self.store.put(key.as_bytes(), &value);

        // Index by hash
        let hash_key = format!("block:hash:{:x?}", block.hash);
        self.store.put(hash_key.as_bytes(), &value);
    }

    /// Retrieve a block by its height
    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        let key = format!("block:height:{}", height);
        self.store.get(key.as_bytes())
            .and_then(|bytes| bincode::deserialize(&bytes).ok())
    }

    /// Retrieve a block by its hash
    pub fn get_block_by_hash(&self, hash: &[u8]) -> Option<Block> {
        let key = format!("block:hash:{:x?}", hash);
        self.store.get(key.as_bytes())
            .and_then(|bytes| bincode::deserialize(&bytes).ok())
    }
    
    /// Get the latest block height
    pub fn get_latest_height(&self) -> Option<u64> {
        // Scan for the highest height
        let prefix = b"block:height:";
        let entries = self.store.scan_prefix(prefix);
        
        entries.iter()
            .map(|(key, _)| {
                let key_str = std::str::from_utf8(key).unwrap();
                let height_str = key_str.strip_prefix("block:height:").unwrap();
                height_str.parse::<u64>().unwrap()
            })
            .max()
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
        };
        
        // Store the block
        block_store.put_block(&block);
        
        // Retrieve by height
        let retrieved = block_store.get_block_by_height(1).unwrap();
        assert_eq!(retrieved, block);
        
        // Retrieve by hash
        let retrieved = block_store.get_block_by_hash(&[1; 32]).unwrap();
        assert_eq!(retrieved, block);
        
        // Test latest height
        assert_eq!(block_store.get_latest_height(), Some(1));
    }
}
