//! State pruning for VibeCoin blockchain
//!
//! This module provides functionality for pruning old state data,
//! reducing storage requirements while maintaining blockchain integrity.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use log::{debug, error, info, warn};

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::state::{AccountState, StateRoot};
use crate::storage::state_store::{StateStore, StateStoreError};
use crate::storage::block_store::{BlockStore, Hash};

/// Error type for state pruning operations
#[derive(Debug, Error)]
pub enum PruningError {
    /// KV store error
    #[error("KV store error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// State store error
    #[error("State store error: {0}")]
    StateStoreError(#[from] StateStoreError),

    /// Invalid pruning mode
    #[error("Invalid pruning mode: {0}")]
    InvalidPruningMode(String),

    /// Invalid block height
    #[error("Invalid block height: {0}")]
    InvalidBlockHeight(u64),

    /// Invalid state root
    #[error("Invalid state root: {0}")]
    InvalidStateRoot(String),

    /// Pruning in progress
    #[error("Pruning already in progress")]
    PruningInProgress,

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for pruning operations
pub type PruningResult<T> = Result<T, PruningError>;

/// Pruning mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruningMode {
    /// Archive mode (keep all state)
    Archive,

    /// Keep state for the last N blocks
    KeepLastNBlocks(u64),

    /// Keep state at regular intervals
    KeepInterval(u64),

    /// Keep state for specific heights
    KeepSpecificHeights,
}

impl PruningMode {
    /// Parse pruning mode from string
    pub fn from_string(s: &str) -> Result<Self, PruningError> {
        if s == "archive" {
            return Ok(PruningMode::Archive);
        }

        if let Some(n) = s.strip_prefix("last_") {
            if let Ok(blocks) = n.parse::<u64>() {
                if blocks > 0 {
                    return Ok(PruningMode::KeepLastNBlocks(blocks));
                }
            }
        }

        if let Some(n) = s.strip_prefix("interval_") {
            if let Ok(interval) = n.parse::<u64>() {
                if interval > 0 {
                    return Ok(PruningMode::KeepInterval(interval));
                }
            }
        }

        if s == "specific" {
            return Ok(PruningMode::KeepSpecificHeights);
        }

        Err(PruningError::InvalidPruningMode(s.to_string()))
    }
}

/// State pruner configuration
#[derive(Debug, Clone)]
pub struct PrunerConfig {
    /// Pruning mode
    pub mode: PruningMode,

    /// Specific heights to keep (for KeepSpecificHeights mode)
    pub keep_heights: Option<Vec<u64>>,

    /// Maximum batch size for pruning operations
    pub max_batch_size: usize,

    /// Whether to compact the database after pruning
    pub compact_after_pruning: bool,
}

impl Default for PrunerConfig {
    fn default() -> Self {
        Self {
            mode: PruningMode::KeepLastNBlocks(10000),
            keep_heights: None,
            max_batch_size: 1000,
            compact_after_pruning: true,
        }
    }
}

/// State pruner
pub struct StatePruner<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// State store
    state_store: &'a StateStore<'a>,

    /// Block store
    block_store: &'a BlockStore<'a>,

    /// Pruner configuration
    config: PrunerConfig,

    /// Whether pruning is in progress
    pruning_in_progress: std::sync::atomic::AtomicBool,
}

impl<'a> StatePruner<'a> {
    /// Create a new state pruner
    pub fn new(
        store: &'a dyn KVStore,
        state_store: &'a StateStore<'a>,
        block_store: &'a BlockStore<'a>,
        config: PrunerConfig,
    ) -> Self {
        Self {
            store,
            state_store,
            block_store,
            config,
            pruning_in_progress: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Prune state data
    pub fn prune(&self) -> PruningResult<PruningStats> {
        // Ensure pruning is not already in progress
        let was_pruning = self.pruning_in_progress.swap(true, std::sync::atomic::Ordering::SeqCst);
        if was_pruning {
            return Err(PruningError::PruningInProgress);
        }

        // Ensure we reset the flag when we're done
        let _guard = scopeguard::guard((), |_| {
            self.pruning_in_progress.store(false, std::sync::atomic::Ordering::SeqCst);
        });

        // Get the current block height
        let current_height = self.block_store.get_latest_height()
            .map_err(|e| PruningError::Other(format!("Failed to get latest height: {}", e)))?;

        // Determine which heights to keep
        let heights_to_keep = self.determine_heights_to_keep(current_height)?;

        // Prune state data
        let stats = self.prune_state_data(heights_to_keep, current_height)?;

        // Compact the database if configured
        if self.config.compact_after_pruning {
            self.compact_database()?;
        }

        Ok(stats)
    }

    /// Determine which block heights to keep
    fn determine_heights_to_keep(&self, current_height: u64) -> PruningResult<HashSet<u64>> {
        let mut heights_to_keep = HashSet::new();

        match self.config.mode {
            PruningMode::Archive => {
                // Keep all heights
                for height in 0..=current_height {
                    heights_to_keep.insert(height);
                }
            }
            PruningMode::KeepLastNBlocks(n) => {
                // Keep the last n blocks
                let start_height = if current_height >= n { current_height - n + 1 } else { 0 };
                for height in start_height..=current_height {
                    heights_to_keep.insert(height);
                }
            }
            PruningMode::KeepInterval(interval) => {
                // Keep blocks at regular intervals
                let mut height = 0;
                while height <= current_height {
                    heights_to_keep.insert(height);
                    height += interval;
                }
                // Always keep the latest block
                heights_to_keep.insert(current_height);
            }
            PruningMode::KeepSpecificHeights => {
                // Keep specific heights
                if let Some(heights) = &self.config.keep_heights {
                    for &height in heights {
                        if height <= current_height {
                            heights_to_keep.insert(height);
                        }
                    }
                } else {
                    return Err(PruningError::InvalidPruningMode(
                        "No specific heights provided for KeepSpecificHeights mode".to_string(),
                    ));
                }
                // Always keep the latest block
                heights_to_keep.insert(current_height);
            }
        }

        // Always keep genesis block (height 0)
        heights_to_keep.insert(0);

        Ok(heights_to_keep)
    }

    /// Prune state data
    fn prune_state_data(
        &self,
        heights_to_keep: HashSet<u64>,
        current_height: u64,
    ) -> PruningResult<PruningStats> {
        let mut stats = PruningStats::default();

        // Get all state roots
        let state_roots = self.get_all_state_roots()?;

        // Determine which state roots to prune
        let mut state_roots_to_prune = Vec::new();
        for (height, root) in &state_roots {
            if !heights_to_keep.contains(height) {
                state_roots_to_prune.push((*height, *root));
            }
        }

        // Sort by height (ascending)
        state_roots_to_prune.sort_by_key(|(height, _)| *height);

        // Prune state roots in batches
        for chunk in state_roots_to_prune.chunks(self.config.max_batch_size) {
            let mut batch = Vec::new();

            for (height, root) in chunk {
                // Add state root deletion to batch
                let key = format!("state_root:{}", height);
                batch.push(WriteBatchOperation::Delete {
                    key: key.as_bytes().to_vec(),
                });

                // Add account states deletion to batch
                let accounts = self.get_accounts_at_height(*height, root)?;
                for address in accounts {
                    let key = format!("account:{}:{}", hex::encode(address), height);
                    batch.push(WriteBatchOperation::Delete {
                        key: key.as_bytes().to_vec(),
                    });
                    stats.accounts_pruned += 1;
                }

                stats.state_roots_pruned += 1;
            }

            // Execute the batch
            if !batch.is_empty() {
                self.store.write_batch(batch)
                    .map_err(PruningError::KVStoreError)?;
            }
        }

        stats.total_heights = current_height + 1;
        stats.heights_kept = heights_to_keep.len() as u64;
        stats.heights_pruned = stats.total_heights - stats.heights_kept;

        Ok(stats)
    }

    /// Get all state roots
    fn get_all_state_roots(&self) -> PruningResult<HashMap<u64, Hash>> {
        let mut state_roots = HashMap::new();

        // Scan all state root keys
        let prefix = "state_root:";
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(PruningError::KVStoreError)?;

        for (key, value) in results {
            // Extract height from key
            let key_str = String::from_utf8_lossy(&key);
            let height_str = key_str.strip_prefix(prefix).unwrap_or("");
            if let Ok(height) = height_str.parse::<u64>() {
                // Extract root hash from value
                if value.len() == 32 {
                    let mut root = [0u8; 32];
                    root.copy_from_slice(&value);
                    state_roots.insert(height, root);
                }
            }
        }

        Ok(state_roots)
    }

    /// Get accounts at a specific height
    fn get_accounts_at_height(&self, height: u64, root: &Hash) -> PruningResult<Vec<Hash>> {
        let mut accounts = Vec::new();

        // Scan all account keys for this height
        let prefix = format!("account:*:{}", height);
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(PruningError::KVStoreError)?;

        for (key, _) in results {
            // Extract address from key
            let key_str = String::from_utf8_lossy(&key);
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() == 3 {
                if let Ok(address) = hex::decode(parts[1]) {
                    if address.len() == 32 {
                        let mut addr = [0u8; 32];
                        addr.copy_from_slice(&address);
                        accounts.push(addr);
                    }
                }
            }
        }

        Ok(accounts)
    }

    /// Compact the database
    fn compact_database(&self) -> PruningResult<()> {
        // This would typically call a method on the KV store to compact the database
        // For now, we'll just log that compaction would happen
        info!("Compacting database after pruning");
        
        // If the KVStore implementation supports compaction, we would call it here
        // self.store.compact()?;
        
        Ok(())
    }
}

/// Pruning statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct PruningStats {
    /// Total number of block heights
    pub total_heights: u64,

    /// Number of heights kept
    pub heights_kept: u64,

    /// Number of heights pruned
    pub heights_pruned: u64,

    /// Number of state roots pruned
    pub state_roots_pruned: u64,

    /// Number of accounts pruned
    pub accounts_pruned: u64,
}

impl std::fmt::Display for PruningStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pruning stats: {} of {} heights pruned, {} state roots pruned, {} accounts pruned",
            self.heights_pruned, self.total_heights, self.state_roots_pruned, self.accounts_pruned
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::kv_store::RocksDBStore;

    // Helper function to create a test environment
    fn setup_test_env() -> (tempfile::TempDir, RocksDBStore, BlockStore, StateStore) {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        
        // Create a RocksDB store
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        
        // Create block and state stores
        let block_store = BlockStore::new(&kv_store);
        let state_store = StateStore::new(&kv_store);
        
        (temp_dir, kv_store, block_store, state_store)
    }

    #[test]
    fn test_pruning_mode_parsing() {
        assert_eq!(PruningMode::from_string("archive").unwrap(), PruningMode::Archive);
        assert_eq!(PruningMode::from_string("last_1000").unwrap(), PruningMode::KeepLastNBlocks(1000));
        assert_eq!(PruningMode::from_string("interval_100").unwrap(), PruningMode::KeepInterval(100));
        assert_eq!(PruningMode::from_string("specific").unwrap(), PruningMode::KeepSpecificHeights);
        
        assert!(PruningMode::from_string("invalid").is_err());
        assert!(PruningMode::from_string("last_0").is_err());
        assert!(PruningMode::from_string("interval_0").is_err());
    }

    #[test]
    fn test_determine_heights_to_keep() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();
        
        // Archive mode
        let config = PrunerConfig {
            mode: PruningMode::Archive,
            ..Default::default()
        };
        let pruner = StatePruner::new(&kv_store, &state_store, &block_store, config);
        let heights = pruner.determine_heights_to_keep(10).unwrap();
        assert_eq!(heights.len(), 11); // 0 to 10
        for i in 0..=10 {
            assert!(heights.contains(&i));
        }
        
        // Keep last N blocks
        let config = PrunerConfig {
            mode: PruningMode::KeepLastNBlocks(5),
            ..Default::default()
        };
        let pruner = StatePruner::new(&kv_store, &state_store, &block_store, config);
        let heights = pruner.determine_heights_to_keep(10).unwrap();
        assert_eq!(heights.len(), 6); // 0, 6, 7, 8, 9, 10
        assert!(heights.contains(&0)); // Genesis always kept
        for i in 6..=10 {
            assert!(heights.contains(&i));
        }
        
        // Keep interval
        let config = PrunerConfig {
            mode: PruningMode::KeepInterval(3),
            ..Default::default()
        };
        let pruner = StatePruner::new(&kv_store, &state_store, &block_store, config);
        let heights = pruner.determine_heights_to_keep(10).unwrap();
        assert_eq!(heights.len(), 5); // 0, 3, 6, 9, 10
        assert!(heights.contains(&0));
        assert!(heights.contains(&3));
        assert!(heights.contains(&6));
        assert!(heights.contains(&9));
        assert!(heights.contains(&10)); // Latest always kept
        
        // Keep specific heights
        let config = PrunerConfig {
            mode: PruningMode::KeepSpecificHeights,
            keep_heights: Some(vec![0, 5, 10, 15]),
            ..Default::default()
        };
        let pruner = StatePruner::new(&kv_store, &state_store, &block_store, config);
        let heights = pruner.determine_heights_to_keep(10).unwrap();
        assert_eq!(heights.len(), 3); // 0, 5, 10
        assert!(heights.contains(&0));
        assert!(heights.contains(&5));
        assert!(heights.contains(&10));
        assert!(!heights.contains(&15)); // Beyond current height
    }
}
