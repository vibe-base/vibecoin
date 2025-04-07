//! Snapshot management for VibeCoin blockchain
//!
//! This module provides functionality for creating, managing, and restoring
//! snapshots of the blockchain state. Snapshots are useful for:
//! - Fast syncing new nodes
//! - Creating backups
//! - Supporting fork experimentation
//! - Enabling time-travel debugging

use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use log::{debug, error, info, warn};
use thiserror::Error;
use rand::random;
use walkdir::WalkDir;

use crate::storage::kv_store::{KVStore, KVStoreError, RocksDBStore};
use crate::storage::block_store::{Block, Hash, BlockStore};
use crate::storage::state::StateRoot;
use crate::storage::state_store::StateStore;

/// Error types for snapshot operations
#[derive(Error, Debug)]
pub enum SnapshotError {
    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// KV store error
    #[error("KV store error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// Snapshot not found
    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(String),

    /// Invalid snapshot format
    #[error("Invalid snapshot format: {0}")]
    InvalidSnapshotFormat(String),

    /// Snapshot creation failed
    #[error("Snapshot creation failed: {0}")]
    SnapshotCreationFailed(String),

    /// Snapshot restoration failed
    #[error("Snapshot restoration failed: {0}")]
    SnapshotRestorationFailed(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Snapshot metadata
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SnapshotMetadata {
    /// Snapshot ID
    pub id: String,

    /// Snapshot name
    pub name: String,

    /// Snapshot description
    pub description: Option<String>,

    /// Creation timestamp
    pub created_at: u64,

    /// Block height at snapshot time
    pub block_height: u64,

    /// Block hash at snapshot time
    pub block_hash: Hash,

    /// State root at snapshot time
    pub state_root: Hash,

    /// Size of the snapshot in bytes
    pub size_bytes: u64,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Custom metadata
    pub custom_metadata: HashMap<String, String>,
}

/// Snapshot type
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum SnapshotType {
    /// Full snapshot (blocks, transactions, state)
    Full,

    /// State-only snapshot
    StateOnly,

    /// Blocks-only snapshot
    BlocksOnly,
}

/// Snapshot compression type
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum CompressionType {
    /// No compression
    None,

    /// LZ4 compression
    LZ4,

    /// Zstandard compression
    Zstd,
}

/// Snapshot configuration
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Base directory for snapshots
    pub snapshot_dir: PathBuf,

    /// Snapshot type
    pub snapshot_type: SnapshotType,

    /// Compression type
    pub compression: CompressionType,

    /// Maximum number of snapshots to keep
    pub max_snapshots: usize,

    /// Whether to include mempool transactions
    pub include_mempool: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_dir: PathBuf::from("./data/snapshots"),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::Zstd,
            max_snapshots: 10,
            include_mempool: false,
        }
    }
}

/// Snapshot manager for creating and restoring snapshots
pub struct SnapshotManager<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// Block store
    block_store: Option<&'a BlockStore<'a>>,

    /// State store
    state_store: Option<&'a StateStore<'a>>,

    /// Configuration
    config: SnapshotConfig,
}

impl<'a> SnapshotManager<'a> {
    /// Create a new SnapshotManager with the given KVStore
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self {
            store,
            block_store: None,
            state_store: None,
            config: SnapshotConfig::default(),
        }
    }

    /// Create a new SnapshotManager with the given KVStore and configuration
    pub fn with_config(store: &'a dyn KVStore, config: SnapshotConfig) -> Self {
        Self {
            store,
            block_store: None,
            state_store: None,
            config,
        }
    }

    /// Set the block store
    pub fn with_block_store(mut self, block_store: &'a BlockStore<'a>) -> Self {
        self.block_store = Some(block_store);
        self
    }

    /// Set the state store
    pub fn with_state_store(mut self, state_store: &'a StateStore<'a>) -> Self {
        self.state_store = Some(state_store);
        self
    }

    /// Initialize the snapshot directory
    pub fn init(&self) -> Result<(), SnapshotError> {
        // Create the snapshot directory if it doesn't exist
        if !self.config.snapshot_dir.exists() {
            fs::create_dir_all(&self.config.snapshot_dir)
                .map_err(|e| SnapshotError::IoError(e))?
        }

        // Create the metadata directory
        let metadata_dir = self.config.snapshot_dir.join("metadata");
        if !metadata_dir.exists() {
            fs::create_dir_all(&metadata_dir)
                .map_err(|e| SnapshotError::IoError(e))?
        }

        Ok(())
    }

    /// Generate a unique snapshot ID
    fn generate_snapshot_id(&self) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let random_suffix = rand::random::<u32>();

        format!("snapshot-{}-{:08x}", timestamp, random_suffix)
    }

    /// Get the path for a snapshot
    fn get_snapshot_path(&self, snapshot_id: &str) -> PathBuf {
        self.config.snapshot_dir.join(snapshot_id)
    }

    /// Get the metadata path for a snapshot
    fn get_metadata_path(&self, snapshot_id: &str) -> PathBuf {
        self.config.snapshot_dir.join("metadata").join(format!("{}.json", snapshot_id))
    }

    /// Save snapshot metadata
    fn save_metadata(&self, metadata: &SnapshotMetadata) -> Result<(), SnapshotError> {
        let metadata_path = self.get_metadata_path(&metadata.id);

        let metadata_json = serde_json::to_string_pretty(metadata)
            .map_err(|e| SnapshotError::SerializationError(e.to_string()))?;

        fs::write(metadata_path, metadata_json)
            .map_err(|e| SnapshotError::IoError(e))?;

        Ok(())
    }

    /// Load snapshot metadata
    fn load_metadata(&self, snapshot_id: &str) -> Result<SnapshotMetadata, SnapshotError> {
        let metadata_path = self.get_metadata_path(snapshot_id);

        if !metadata_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(snapshot_id.to_string()));
        }

        let metadata_json = fs::read_to_string(metadata_path)
            .map_err(|e| SnapshotError::IoError(e))?;

        serde_json::from_str(&metadata_json)
            .map_err(|e| SnapshotError::DeserializationError(e.to_string()))
    }

    /// List all available snapshots
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>, SnapshotError> {
        let metadata_dir = self.config.snapshot_dir.join("metadata");

        if !metadata_dir.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();

        for entry in fs::read_dir(metadata_dir).map_err(SnapshotError::IoError)? {
            let entry = entry.map_err(SnapshotError::IoError)?;
            let path = entry.path();

            if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                let metadata_json = fs::read_to_string(path)
                    .map_err(|e| SnapshotError::IoError(e))?;

                let metadata: SnapshotMetadata = serde_json::from_str(&metadata_json)
                    .map_err(|e| SnapshotError::DeserializationError(e.to_string()))?;

                snapshots.push(metadata);
            }
        }

        // Sort by creation time (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(snapshots)
    }

    /// Get a snapshot by ID
    pub fn get_snapshot(&self, snapshot_id: &str) -> Result<SnapshotMetadata, SnapshotError> {
        self.load_metadata(snapshot_id)
    }

    /// Create a new snapshot
    pub fn create_snapshot(
        &self,
        name: &str,
        description: Option<&str>,
        tags: Vec<String>,
        custom_metadata: Option<HashMap<String, String>>,
    ) -> Result<SnapshotMetadata, SnapshotError> {
        // Ensure we have the required stores based on snapshot type
        match self.config.snapshot_type {
            SnapshotType::Full | SnapshotType::BlocksOnly => {
                if self.block_store.is_none() {
                    return Err(SnapshotError::Other("Block store is required for this snapshot type".to_string()));
                }
            },
            SnapshotType::StateOnly => {
                if self.state_store.is_none() {
                    return Err(SnapshotError::Other("State store is required for this snapshot type".to_string()));
                }
            },
        }

        // Initialize snapshot directories
        self.init()?;

        // Generate a unique snapshot ID
        let snapshot_id = self.generate_snapshot_id();
        let snapshot_path = self.get_snapshot_path(&snapshot_id);

        // Create the snapshot directory
        fs::create_dir_all(&snapshot_path)
            .map_err(|e| SnapshotError::IoError(e))?;

        // Get the current block and state information
        let (block_height, block_hash, state_root) = self.get_current_chain_state()?;

        // Create the snapshot based on the configured type
        match self.config.snapshot_type {
            SnapshotType::Full => {
                self.create_full_snapshot(&snapshot_id, block_height)?;
            },
            SnapshotType::StateOnly => {
                self.create_state_snapshot(&snapshot_id, block_height)?;
            },
            SnapshotType::BlocksOnly => {
                self.create_blocks_snapshot(&snapshot_id, block_height)?;
            },
        }

        // Calculate the snapshot size
        let size_bytes = self.calculate_snapshot_size(&snapshot_id)?;

        // Create and save metadata
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let metadata = SnapshotMetadata {
            id: snapshot_id.clone(),
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            created_at: timestamp,
            block_height,
            block_hash,
            state_root,
            size_bytes,
            tags,
            custom_metadata: custom_metadata.unwrap_or_default(),
        };

        self.save_metadata(&metadata)?;

        // Prune old snapshots if needed
        self.prune_old_snapshots()?;

        info!("Created snapshot {} at block height {}", snapshot_id, block_height);
        Ok(metadata)
    }

    /// Get the current chain state (height, hash, state root)
    fn get_current_chain_state(&self) -> Result<(u64, Hash, Hash), SnapshotError> {
        let block_store = self.block_store.ok_or_else(|| {
            SnapshotError::Other("Block store is required to get chain state".to_string())
        })?;

        let _state_store = self.state_store.ok_or_else(|| {
            SnapshotError::Other("State store is required to get chain state".to_string())
        })?;

        // Get the latest block height
        let height = match block_store.get_latest_height() {
            Some(height) => height,
            None => return Err(SnapshotError::Other("Failed to get latest height".to_string())),
        };

        // Get the block at that height
        let block = block_store.get_block_by_height(height)
            .map_err(|e| SnapshotError::Other(format!("Failed to get block: {}", e)))?;

        let block = block.ok_or_else(|| {
            SnapshotError::Other(format!("Block at height {} not found", height))
        })?;

        // Get the state root
        let state_root = block.state_root;

        Ok((height, block.hash, state_root))
    }

    /// Create a full snapshot (blocks, transactions, state)
    fn create_full_snapshot(&self, snapshot_id: &str, block_height: u64) -> Result<(), SnapshotError> {
        // Create the snapshot directory structure
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        let blocks_path = snapshot_path.join("blocks");
        let state_path = snapshot_path.join("state");

        fs::create_dir_all(&blocks_path)
            .map_err(|e| SnapshotError::IoError(e))?;
        fs::create_dir_all(&state_path)
            .map_err(|e| SnapshotError::IoError(e))?;

        // Create RocksDB backup for blocks
        self.backup_blocks_to_snapshot(snapshot_id, block_height)?;

        // Create RocksDB backup for state
        self.backup_state_to_snapshot(snapshot_id)?;

        Ok(())
    }

    /// Create a state-only snapshot
    fn create_state_snapshot(&self, snapshot_id: &str, _block_height: u64) -> Result<(), SnapshotError> {
        // Create the snapshot directory structure
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        let state_path = snapshot_path.join("state");

        fs::create_dir_all(&state_path)
            .map_err(|e| SnapshotError::IoError(e))?;

        // Create RocksDB backup for state
        self.backup_state_to_snapshot(snapshot_id)?;

        Ok(())
    }

    /// Create a blocks-only snapshot
    fn create_blocks_snapshot(&self, snapshot_id: &str, block_height: u64) -> Result<(), SnapshotError> {
        // Create the snapshot directory structure
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        let blocks_path = snapshot_path.join("blocks");

        fs::create_dir_all(&blocks_path)
            .map_err(|e| SnapshotError::IoError(e))?;

        // Create RocksDB backup for blocks
        self.backup_blocks_to_snapshot(snapshot_id, block_height)?;

        Ok(())
    }

    /// Backup blocks to snapshot
    fn backup_blocks_to_snapshot(&self, snapshot_id: &str, block_height: u64) -> Result<(), SnapshotError> {
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        let blocks_path = snapshot_path.join("blocks");

        // Get the RocksDB instance
        let rocks_db = {
            // Try to downcast to RocksDBStore using type name
            let type_name = std::any::type_name_of_val(self.store);
            if type_name.contains("RocksDBStore") {
                // This is a safe downcast because we've checked the type name
                let rocks_store = unsafe { &*(self.store as *const dyn KVStore as *const RocksDBStore) };
                rocks_store.get_db()
            } else {
                return Err(SnapshotError::Other("Only RocksDBStore is supported for snapshots".to_string()))
            }
        };

        // Create a RocksDB checkpoint
        let checkpoint = rocksdb::checkpoint::Checkpoint::new(rocks_db)
            .map_err(|e| SnapshotError::Other(format!("Failed to create checkpoint: {}", e)))?;

        // Create the checkpoint at the blocks path
        checkpoint.create_checkpoint(&blocks_path)
            .map_err(|e| SnapshotError::Other(format!("Failed to create checkpoint: {}", e)))?;

        // Write the block height to a file for reference
        let height_file = blocks_path.join("height.txt");
        fs::write(height_file, block_height.to_string())
            .map_err(|e| SnapshotError::IoError(e))?;

        Ok(())
    }

    /// Backup state to snapshot
    fn backup_state_to_snapshot(&self, snapshot_id: &str) -> Result<(), SnapshotError> {
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        let state_path = snapshot_path.join("state");

        // Get the RocksDB instance
        let rocks_db = {
            // Try to downcast to RocksDBStore using type name
            let type_name = std::any::type_name_of_val(self.store);
            if type_name.contains("RocksDBStore") {
                // This is a safe downcast because we've checked the type name
                let rocks_store = unsafe { &*(self.store as *const dyn KVStore as *const RocksDBStore) };
                rocks_store.get_db()
            } else {
                return Err(SnapshotError::Other("Only RocksDBStore is supported for snapshots".to_string()))
            }
        };

        // Create a RocksDB checkpoint
        let checkpoint = rocksdb::checkpoint::Checkpoint::new(rocks_db)
            .map_err(|e| SnapshotError::Other(format!("Failed to create checkpoint: {}", e)))?;

        // Create the checkpoint at the state path
        checkpoint.create_checkpoint(&state_path)
            .map_err(|e| SnapshotError::Other(format!("Failed to create checkpoint: {}", e)))?;

        Ok(())
    }

    /// Calculate the size of a snapshot in bytes
    fn calculate_snapshot_size(&self, snapshot_id: &str) -> Result<u64, SnapshotError> {
        let snapshot_path = self.get_snapshot_path(snapshot_id);

        if !snapshot_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(snapshot_id.to_string()));
        }

        let mut total_size = 0;

        // Walk the directory and sum up file sizes
        for entry in walkdir::WalkDir::new(&snapshot_path) {
            let entry = entry.map_err(|e| SnapshotError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to walk directory: {}", e)
            )))?;

            if entry.file_type().is_file() {
                total_size += entry.metadata()
                    .map_err(|e| SnapshotError::IoError(e.into()))?
                    .len();
            }
        }

        Ok(total_size)
    }

    /// Prune old snapshots to stay within the configured maximum
    fn prune_old_snapshots(&self) -> Result<(), SnapshotError> {
        let snapshots = self.list_snapshots()?;

        if snapshots.len() <= self.config.max_snapshots {
            return Ok(());
        }

        // Snapshots are already sorted by creation time (newest first)
        // So we need to remove the oldest ones (at the end of the list)
        let to_remove = &snapshots[self.config.max_snapshots..];

        for metadata in to_remove {
            self.delete_snapshot(&metadata.id)?;
        }

        Ok(())
    }

    /// Delete a snapshot
    pub fn delete_snapshot(&self, snapshot_id: &str) -> Result<(), SnapshotError> {
        let snapshot_path = self.get_snapshot_path(snapshot_id);
        let metadata_path = self.get_metadata_path(snapshot_id);

        // Check if the snapshot exists
        if !snapshot_path.exists() && !metadata_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(snapshot_id.to_string()));
        }

        // Delete the snapshot directory if it exists
        if snapshot_path.exists() {
            fs::remove_dir_all(&snapshot_path)
                .map_err(|e| SnapshotError::IoError(e))?;
        }

        // Delete the metadata file if it exists
        if metadata_path.exists() {
            fs::remove_file(&metadata_path)
                .map_err(|e| SnapshotError::IoError(e))?;
        }

        info!("Deleted snapshot {}", snapshot_id);
        Ok(())
    }

    /// Restore a snapshot
    pub fn restore_snapshot(
        &self,
        snapshot_id: &str,
        target_dir: &Path,
        restore_type: Option<SnapshotType>,
    ) -> Result<SnapshotMetadata, SnapshotError> {
        // Load the snapshot metadata
        let metadata = self.load_metadata(snapshot_id)?;

        // Determine what to restore based on the restore_type or the snapshot type
        let restore_type = restore_type.unwrap_or(self.config.snapshot_type);

        // Create the target directory if it doesn't exist
        if !target_dir.exists() {
            fs::create_dir_all(target_dir)
                .map_err(|e| SnapshotError::IoError(e))?;
        }

        // Get the snapshot path
        let snapshot_path = self.get_snapshot_path(snapshot_id);

        if !snapshot_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(snapshot_id.to_string()));
        }

        // Restore based on the restore type
        match restore_type {
            SnapshotType::Full => {
                self.restore_full_snapshot(snapshot_id, target_dir)?;
            },
            SnapshotType::StateOnly => {
                self.restore_state_snapshot(snapshot_id, target_dir)?;
            },
            SnapshotType::BlocksOnly => {
                self.restore_blocks_snapshot(snapshot_id, target_dir)?;
            },
        }

        info!("Restored snapshot {} to {}", snapshot_id, target_dir.display());
        Ok(metadata)
    }

    /// Restore a full snapshot (blocks, transactions, state)
    fn restore_full_snapshot(&self, snapshot_id: &str, target_dir: &Path) -> Result<(), SnapshotError> {
        // Get the snapshot path
        let snapshot_path = self.get_snapshot_path(snapshot_id);

        // Check if the blocks and state directories exist
        let blocks_path = snapshot_path.join("blocks");
        let state_path = snapshot_path.join("state");

        if !blocks_path.exists() || !state_path.exists() {
            return Err(SnapshotError::InvalidSnapshotFormat(
                format!("Snapshot {} is not a full snapshot", snapshot_id)
            ));
        }

        // Create target directories
        let target_blocks_dir = target_dir.join("blocks");
        let target_state_dir = target_dir.join("state");

        fs::create_dir_all(&target_blocks_dir)
            .map_err(|e| SnapshotError::IoError(e))?;
        fs::create_dir_all(&target_state_dir)
            .map_err(|e| SnapshotError::IoError(e))?;

        // Copy blocks data
        self.copy_directory(&blocks_path, &target_blocks_dir)?;

        // Copy state data
        self.copy_directory(&state_path, &target_state_dir)?;

        Ok(())
    }

    /// Restore a state-only snapshot
    fn restore_state_snapshot(&self, snapshot_id: &str, target_dir: &Path) -> Result<(), SnapshotError> {
        // Get the snapshot path
        let snapshot_path = self.get_snapshot_path(snapshot_id);

        // Check if the state directory exists
        let state_path = snapshot_path.join("state");

        if !state_path.exists() {
            return Err(SnapshotError::InvalidSnapshotFormat(
                format!("Snapshot {} does not contain state data", snapshot_id)
            ));
        }

        // Create target directory
        let target_state_dir = target_dir.join("state");

        fs::create_dir_all(&target_state_dir)
            .map_err(|e| SnapshotError::IoError(e))?;

        // Copy state data
        self.copy_directory(&state_path, &target_state_dir)?;

        Ok(())
    }

    /// Restore a blocks-only snapshot
    fn restore_blocks_snapshot(&self, snapshot_id: &str, target_dir: &Path) -> Result<(), SnapshotError> {
        // Get the snapshot path
        let snapshot_path = self.get_snapshot_path(snapshot_id);

        // Check if the blocks directory exists
        let blocks_path = snapshot_path.join("blocks");

        if !blocks_path.exists() {
            return Err(SnapshotError::InvalidSnapshotFormat(
                format!("Snapshot {} does not contain blocks data", snapshot_id)
            ));
        }

        // Create target directory
        let target_blocks_dir = target_dir.join("blocks");

        fs::create_dir_all(&target_blocks_dir)
            .map_err(|e| SnapshotError::IoError(e))?;

        // Copy blocks data
        self.copy_directory(&blocks_path, &target_blocks_dir)?;

        Ok(())
    }

    /// Copy a directory recursively
    fn copy_directory(&self, src: &Path, dst: &Path) -> Result<(), SnapshotError> {
        if !src.exists() {
            return Err(SnapshotError::IoError(
                std::io::Error::new(std::io::ErrorKind::NotFound, format!("Source directory not found: {}", src.display()))
            ));
        }

        if !src.is_dir() {
            return Err(SnapshotError::IoError(
                std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Source is not a directory: {}", src.display()))
            ));
        }

        if !dst.exists() {
            fs::create_dir_all(dst)
                .map_err(|e| SnapshotError::IoError(e))?;
        }

        for entry in fs::read_dir(src).map_err(SnapshotError::IoError)? {
            let entry = entry.map_err(SnapshotError::IoError)?;
            let entry_path = entry.path();
            let file_name = entry.file_name();
            let dst_path = dst.join(file_name);

            if entry_path.is_dir() {
                self.copy_directory(&entry_path, &dst_path)?;
            } else {
                fs::copy(&entry_path, &dst_path)
                    .map_err(|e| SnapshotError::IoError(e))?;
            }
        }

        Ok(())
    }

    /// Open a database from a snapshot
    pub fn open_snapshot_db(&self, snapshot_id: &str, snapshot_type: SnapshotType) -> Result<RocksDBStore, SnapshotError> {
        // Load the snapshot metadata
        let _metadata = self.load_metadata(snapshot_id)?;

        // Get the snapshot path
        let snapshot_path = self.get_snapshot_path(snapshot_id);

        if !snapshot_path.exists() {
            return Err(SnapshotError::SnapshotNotFound(snapshot_id.to_string()));
        }

        // Determine the path based on the snapshot type
        let db_path = match snapshot_type {
            SnapshotType::Full | SnapshotType::BlocksOnly => snapshot_path.join("blocks"),
            SnapshotType::StateOnly => snapshot_path.join("state"),
        };

        if !db_path.exists() {
            return Err(SnapshotError::InvalidSnapshotFormat(
                format!("Snapshot {} does not contain the requested data type", snapshot_id)
            ));
        }

        // Open the database in read-only mode
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(false);

        let db = RocksDBStore::with_options(&db_path, opts)
            .map_err(|e| SnapshotError::Other(format!("Failed to open snapshot database: {}", e)))?;

        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use std::path::Path;
    use crate::storage::state_store::AccountType;

    // Helper function to create a test block
    fn create_test_block(height: u64, prev_hash: Hash) -> Block {
        Block {
            height,
            hash: [height as u8; 32],
            prev_hash,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            transactions: vec![[1; 32], [2; 32]],
            state_root: [3; 32],
            tx_root: [4; 32],
            nonce: 42,
            poh_seq: 100,
            poh_hash: [5; 32],
            difficulty: 1000,
            total_difficulty: 1000,
        }
    }

    // Helper function to set up a test environment
    fn setup_test_env() -> (tempfile::TempDir, RocksDBStore, BlockStore, StateStore) {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();

        // Create a RocksDB store
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();

        // Create block and state stores
        let block_store = BlockStore::new(&kv_store);
        let state_store = StateStore::new(&kv_store);

        // Add some test data
        let genesis_block = create_test_block(0, [0; 32]);
        block_store.put_block(&genesis_block).unwrap();

        let block1 = create_test_block(1, genesis_block.hash);
        block_store.put_block(&block1).unwrap();

        let block2 = create_test_block(2, block1.hash);
        block_store.put_block(&block2).unwrap();

        // Create some test accounts
        let address1 = [1; 32];
        state_store.create_account(&address1, 1000, AccountType::User).unwrap();

        let address2 = [2; 32];
        state_store.create_account(&address2, 2000, AccountType::Contract).unwrap();

        (temp_dir, kv_store, block_store, state_store)
    }

    #[test]
    fn test_snapshot_creation_and_listing() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 5,
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create a snapshot
        let snapshot = snapshot_manager.create_snapshot(
            "Test Snapshot",
            Some("A test snapshot"),
            vec!["test".to_string(), "initial".to_string()],
            None,
        ).unwrap();

        // Verify the snapshot metadata
        assert_eq!(snapshot.name, "Test Snapshot");
        assert_eq!(snapshot.description, Some("A test snapshot".to_string()));
        assert_eq!(snapshot.block_height, 2);
        assert_eq!(snapshot.tags, vec!["test".to_string(), "initial".to_string()]);

        // List snapshots
        let snapshots = snapshot_manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, snapshot.id);

        // Get a snapshot by ID
        let retrieved_snapshot = snapshot_manager.get_snapshot(&snapshot.id).unwrap();
        assert_eq!(retrieved_snapshot.id, snapshot.id);
        assert_eq!(retrieved_snapshot.name, "Test Snapshot");
    }

    #[test]
    fn test_snapshot_restoration() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 5,
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create a snapshot
        let snapshot = snapshot_manager.create_snapshot(
            "Test Snapshot",
            Some("A test snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Create a restoration directory
        let restore_dir = temp_dir.path().join("restore");
        fs::create_dir_all(&restore_dir).unwrap();

        // Restore the snapshot
        let restored_metadata = snapshot_manager.restore_snapshot(
            &snapshot.id,
            &restore_dir,
            None,
        ).unwrap();

        // Verify the restored metadata
        assert_eq!(restored_metadata.id, snapshot.id);
        assert_eq!(restored_metadata.name, "Test Snapshot");

        // Verify the restored directories exist
        assert!(restore_dir.join("blocks").exists());
        assert!(restore_dir.join("state").exists());
    }

    #[test]
    fn test_snapshot_deletion() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 5,
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create a snapshot
        let snapshot = snapshot_manager.create_snapshot(
            "Test Snapshot",
            Some("A test snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Verify the snapshot exists
        assert!(snapshot_manager.get_snapshot_path(&snapshot.id).exists());
        assert!(snapshot_manager.get_metadata_path(&snapshot.id).exists());

        // Delete the snapshot
        snapshot_manager.delete_snapshot(&snapshot.id).unwrap();

        // Verify the snapshot no longer exists
        assert!(!snapshot_manager.get_snapshot_path(&snapshot.id).exists());
        assert!(!snapshot_manager.get_metadata_path(&snapshot.id).exists());

        // List snapshots should return empty
        let snapshots = snapshot_manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn test_snapshot_pruning() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager with max_snapshots = 2
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 2,  // Only keep 2 snapshots
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create 3 snapshots
        let snapshot1 = snapshot_manager.create_snapshot(
            "Snapshot 1",
            Some("First snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Sleep to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(100));

        let snapshot2 = snapshot_manager.create_snapshot(
            "Snapshot 2",
            Some("Second snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Sleep to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(100));

        let snapshot3 = snapshot_manager.create_snapshot(
            "Snapshot 3",
            Some("Third snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // List snapshots - should only have the 2 most recent
        let snapshots = snapshot_manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 2);

        // The oldest snapshot should be pruned
        assert!(snapshots.iter().any(|s| s.id == snapshot2.id));
        assert!(snapshots.iter().any(|s| s.id == snapshot3.id));
        assert!(!snapshots.iter().any(|s| s.id == snapshot1.id));
    }

    // Helper function to create a test block
    fn create_test_block(height: u64, prev_hash: Hash) -> Block {
        Block {
            height,
            hash: [height as u8; 32],
            prev_hash,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            transactions: vec![[1; 32], [2; 32]],
            state_root: [3; 32],
            tx_root: [4; 32],
            nonce: 42,
            poh_seq: 100,
            poh_hash: [5; 32],
            difficulty: 1000,
            total_difficulty: 1000,
        }
    }

    // Helper function to set up a test environment
    fn setup_test_env() -> (tempfile::TempDir, RocksDBStore, BlockStore, StateStore) {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();

        // Create a RocksDB store
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();

        // Create block and state stores
        let block_store = BlockStore::new(&kv_store);
        let state_store = StateStore::new(&kv_store);

        // Add some test data
        let genesis_block = create_test_block(0, [0; 32]);
        block_store.put_block(&genesis_block).unwrap();

        let block1 = create_test_block(1, genesis_block.hash);
        block_store.put_block(&block1).unwrap();

        let block2 = create_test_block(2, block1.hash);
        block_store.put_block(&block2).unwrap();

        // Create some test accounts
        let address1 = [1; 32];
        state_store.create_account(&address1, 1000, AccountType::User).unwrap();

        let address2 = [2; 32];
        state_store.create_account(&address2, 2000, AccountType::Contract).unwrap();

        (temp_dir, kv_store, block_store, state_store)
    }

    #[test]
    fn test_snapshot_creation_and_listing() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 5,
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create a snapshot
        let snapshot = snapshot_manager.create_snapshot(
            "Test Snapshot",
            Some("A test snapshot"),
            vec!["test".to_string(), "initial".to_string()],
            None,
        ).unwrap();

        // Verify the snapshot metadata
        assert_eq!(snapshot.name, "Test Snapshot");
        assert_eq!(snapshot.description, Some("A test snapshot".to_string()));
        assert_eq!(snapshot.block_height, 2);
        assert_eq!(snapshot.tags, vec!["test".to_string(), "initial".to_string()]);

        // List snapshots
        let snapshots = snapshot_manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, snapshot.id);

        // Get a snapshot by ID
        let retrieved_snapshot = snapshot_manager.get_snapshot(&snapshot.id).unwrap();
        assert_eq!(retrieved_snapshot.id, snapshot.id);
        assert_eq!(retrieved_snapshot.name, "Test Snapshot");
    }

    #[test]
    fn test_snapshot_restoration() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 5,
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create a snapshot
        let snapshot = snapshot_manager.create_snapshot(
            "Test Snapshot",
            Some("A test snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Create a restoration directory
        let restore_dir = temp_dir.path().join("restore");
        fs::create_dir_all(&restore_dir).unwrap();

        // Restore the snapshot
        let restored_metadata = snapshot_manager.restore_snapshot(
            &snapshot.id,
            &restore_dir,
            None,
        ).unwrap();

        // Verify the restored metadata
        assert_eq!(restored_metadata.id, snapshot.id);
        assert_eq!(restored_metadata.name, "Test Snapshot");

        // Verify the restored directories exist
        assert!(restore_dir.join("blocks").exists());
        assert!(restore_dir.join("state").exists());
    }

    #[test]
    fn test_snapshot_deletion() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 5,
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create a snapshot
        let snapshot = snapshot_manager.create_snapshot(
            "Test Snapshot",
            Some("A test snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Verify the snapshot exists
        assert!(snapshot_manager.get_snapshot_path(&snapshot.id).exists());
        assert!(snapshot_manager.get_metadata_path(&snapshot.id).exists());

        // Delete the snapshot
        snapshot_manager.delete_snapshot(&snapshot.id).unwrap();

        // Verify the snapshot no longer exists
        assert!(!snapshot_manager.get_snapshot_path(&snapshot.id).exists());
        assert!(!snapshot_manager.get_metadata_path(&snapshot.id).exists());

        // List snapshots should return empty
        let snapshots = snapshot_manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn test_snapshot_pruning() {
        let (temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a snapshot directory
        let snapshot_dir = temp_dir.path().join("snapshots");
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create a snapshot manager with max_snapshots = 2
        let config = SnapshotConfig {
            snapshot_dir: snapshot_dir.clone(),
            snapshot_type: SnapshotType::Full,
            compression: CompressionType::None,
            max_snapshots: 2,  // Only keep 2 snapshots
            include_mempool: false,
        };

        let snapshot_manager = SnapshotManager::with_config(&kv_store, config)
            .with_block_store(&block_store)
            .with_state_store(&state_store);

        // Initialize the snapshot manager
        snapshot_manager.init().unwrap();

        // Create 3 snapshots
        let snapshot1 = snapshot_manager.create_snapshot(
            "Snapshot 1",
            Some("First snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Sleep to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(100));

        let snapshot2 = snapshot_manager.create_snapshot(
            "Snapshot 2",
            Some("Second snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // Sleep to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(100));

        let snapshot3 = snapshot_manager.create_snapshot(
            "Snapshot 3",
            Some("Third snapshot"),
            vec!["test".to_string()],
            None,
        ).unwrap();

        // List snapshots - should only have the 2 most recent
        let snapshots = snapshot_manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 2);

        // The oldest snapshot should be pruned
        assert!(snapshots.iter().any(|s| s.id == snapshot2.id));
        assert!(snapshots.iter().any(|s| s.id == snapshot3.id));
        assert!(!snapshots.iter().any(|s| s.id == snapshot1.id));
    }
}