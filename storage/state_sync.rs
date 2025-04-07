//! State synchronization for VibeCoin blockchain
//!
//! This module provides functionality for synchronizing blockchain state
//! between nodes, enabling fast catch-up and state verification.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use log::{debug, error, info, warn};

use crate::storage::kv_store::{KVStore, KVStoreError};
use crate::storage::state::{AccountState, StateRoot, StateError};
use crate::storage::state_store::{StateStore, StateStoreError};
use crate::storage::block_store::{BlockStore, Hash, Block};

/// Error type for state synchronization operations
#[derive(Debug, Error)]
pub enum SyncError {
    /// KV store error
    #[error("KV store error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// State store error
    #[error("State store error: {0}")]
    StateStoreError(#[from] StateStoreError),

    /// State error
    #[error("State error: {0}")]
    StateError(#[from] StateError),

    /// Invalid state root
    #[error("Invalid state root: {0}")]
    InvalidStateRoot(String),

    /// Invalid block height
    #[error("Invalid block height: {0}")]
    InvalidBlockHeight(u64),

    /// Invalid proof
    #[error("Invalid proof: {0}")]
    InvalidProof(String),

    /// Sync already in progress
    #[error("Sync already in progress")]
    SyncInProgress,

    /// Sync timeout
    #[error("Sync timeout")]
    SyncTimeout,

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for synchronization operations
pub type SyncResult<T> = Result<T, SyncError>;

/// Synchronization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Full sync (download all state)
    Full,

    /// Fast sync (download recent state only)
    Fast,

    /// Light sync (download state roots only)
    Light,
}

/// Synchronization status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    /// Not syncing
    Idle,

    /// Syncing in progress
    InProgress,

    /// Sync completed
    Completed,

    /// Sync failed
    Failed,
}

/// Synchronization configuration
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Synchronization mode
    pub mode: SyncMode,

    /// Maximum number of concurrent downloads
    pub max_concurrent_downloads: usize,

    /// Timeout for sync operations (in seconds)
    pub timeout_seconds: u64,

    /// Batch size for state downloads
    pub batch_size: usize,

    /// Whether to verify proofs
    pub verify_proofs: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            mode: SyncMode::Fast,
            max_concurrent_downloads: 10,
            timeout_seconds: 300,
            batch_size: 1000,
            verify_proofs: true,
        }
    }
}

/// Synchronization progress
#[derive(Debug, Clone)]
pub struct SyncProgress {
    /// Current status
    pub status: SyncStatus,

    /// Target block height
    pub target_height: u64,

    /// Current block height
    pub current_height: u64,

    /// Number of accounts synced
    pub accounts_synced: u64,

    /// Total accounts to sync
    pub total_accounts: u64,

    /// Start time (Unix timestamp)
    pub start_time: u64,

    /// Last update time (Unix timestamp)
    pub last_update_time: u64,
}

impl SyncProgress {
    /// Create a new sync progress
    pub fn new(target_height: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            status: SyncStatus::Idle,
            target_height,
            current_height: 0,
            accounts_synced: 0,
            total_accounts: 0,
            start_time: now,
            last_update_time: now,
        }
    }

    /// Calculate progress percentage
    pub fn percentage(&self) -> f64 {
        if self.total_accounts == 0 {
            return 0.0;
        }

        (self.accounts_synced as f64 / self.total_accounts as f64) * 100.0
    }

    /// Calculate sync speed (accounts per second)
    pub fn speed(&self) -> f64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let elapsed = now.saturating_sub(self.start_time);
        if elapsed == 0 {
            return 0.0;
        }

        self.accounts_synced as f64 / elapsed as f64
    }

    /// Estimate time remaining (in seconds)
    pub fn estimated_time_remaining(&self) -> u64 {
        let speed = self.speed();
        if speed <= 0.0 {
            return 0;
        }

        let remaining_accounts = self.total_accounts.saturating_sub(self.accounts_synced);
        (remaining_accounts as f64 / speed) as u64
    }
}

/// State synchronizer
pub struct StateSynchronizer<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// State store
    state_store: &'a StateStore<'a>,

    /// Block store
    block_store: &'a BlockStore<'a>,

    /// Synchronization configuration
    config: SyncConfig,

    /// Synchronization progress
    progress: Mutex<SyncProgress>,

    /// Network client for fetching state data
    /// In a real implementation, this would be a network client
    /// For now, we'll use a placeholder
    network_client: Option<Arc<dyn NetworkClient>>,
}

impl<'a> StateSynchronizer<'a> {
    /// Create a new state synchronizer
    pub fn new(
        store: &'a dyn KVStore,
        state_store: &'a StateStore<'a>,
        block_store: &'a BlockStore<'a>,
        config: SyncConfig,
    ) -> Self {
        let target_height = 0; // Will be updated during sync
        let progress = Mutex::new(SyncProgress::new(target_height));

        Self {
            store,
            state_store,
            block_store,
            config,
            progress,
            network_client: None,
        }
    }

    /// Set network client
    pub fn set_network_client(&mut self, client: Arc<dyn NetworkClient>) {
        self.network_client = Some(client);
    }

    /// Get synchronization progress
    pub fn get_progress(&self) -> SyncProgress {
        self.progress.lock().unwrap().clone()
    }

    /// Start synchronization
    pub fn start_sync(&self, target_height: u64) -> SyncResult<()> {
        // Ensure we have a network client
        let network_client = match &self.network_client {
            Some(client) => client,
            None => return Err(SyncError::Other("Network client not set".to_string())),
        };

        // Check if sync is already in progress
        let mut progress = self.progress.lock().unwrap();
        if progress.status == SyncStatus::InProgress {
            return Err(SyncError::SyncInProgress);
        }

        // Update progress
        progress.status = SyncStatus::InProgress;
        progress.target_height = target_height;
        progress.current_height = 0;
        progress.accounts_synced = 0;
        progress.total_accounts = 0; // Will be updated during sync
        progress.start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        progress.last_update_time = progress.start_time;

        // Start sync in a separate thread
        let store = self.store;
        let state_store = self.state_store;
        let block_store = self.block_store;
        let config = self.config.clone();
        let progress_mutex = Arc::new(self.progress.clone());
        let network_client_clone = Arc::clone(network_client);

        std::thread::spawn(move || {
            let result = match config.mode {
                SyncMode::Full => Self::sync_full(
                    store,
                    state_store,
                    block_store,
                    &config,
                    progress_mutex.clone(),
                    network_client_clone,
                    target_height,
                ),
                SyncMode::Fast => Self::sync_fast(
                    store,
                    state_store,
                    block_store,
                    &config,
                    progress_mutex.clone(),
                    network_client_clone,
                    target_height,
                ),
                SyncMode::Light => Self::sync_light(
                    store,
                    state_store,
                    block_store,
                    &config,
                    progress_mutex.clone(),
                    network_client_clone,
                    target_height,
                ),
            };

            // Update progress based on result
            let mut progress = progress_mutex.lock().unwrap();
            match result {
                Ok(_) => {
                    progress.status = SyncStatus::Completed;
                    progress.current_height = target_height;
                    info!("State sync completed to height {}", target_height);
                },
                Err(e) => {
                    progress.status = SyncStatus::Failed;
                    error!("State sync failed: {}", e);
                },
            }
        });

        Ok(())
    }

    /// Stop synchronization
    pub fn stop_sync(&self) -> SyncResult<()> {
        let mut progress = self.progress.lock().unwrap();
        if progress.status != SyncStatus::InProgress {
            return Ok(());
        }

        progress.status = SyncStatus::Failed;
        Ok(())
    }

    /// Sync full state
    fn sync_full(
        store: &dyn KVStore,
        state_store: &StateStore,
        block_store: &BlockStore,
        config: &SyncConfig,
        progress: Arc<Mutex<SyncProgress>>,
        network_client: Arc<dyn NetworkClient>,
        target_height: u64,
    ) -> SyncResult<()> {
        // Get state root at target height
        let state_root = network_client.get_state_root(target_height)?;

        // Update progress with total accounts (if available)
        // In a real implementation, we would get this from the network
        let total_accounts = 1000; // Placeholder
        let mut progress_guard = progress.lock().unwrap();
        progress_guard.total_accounts = total_accounts;
        drop(progress_guard);

        // Sync all accounts
        let mut start_key = vec![0u8; 32];
        let end_key = vec![0xffu8; 32];
        let mut accounts_synced = 0;

        loop {
            // Get a batch of accounts
            let accounts = network_client.get_accounts_in_range(
                &start_key,
                &end_key,
                target_height,
                config.batch_size,
            )?;

            if accounts.is_empty() {
                break;
            }

            // Store accounts
            for (address, account) in &accounts {
                state_store.put_account(address, account, target_height)?
                    .map_err(|e| SyncError::StateStoreError(e))?;
                accounts_synced += 1;
            }

            // Update progress
            let mut progress_guard = progress.lock().unwrap();
            progress_guard.accounts_synced = accounts_synced;
            progress_guard.last_update_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            drop(progress_guard);

            // Update start key for next batch
            if let Some((last_address, _)) = accounts.last() {
                start_key = last_address.clone();
            } else {
                break;
            }

            // Check if sync was stopped
            let progress_guard = progress.lock().unwrap();
            if progress_guard.status != SyncStatus::InProgress {
                return Err(SyncError::Other("Sync stopped".to_string()));
            }
            drop(progress_guard);
        }

        // Store state root
        state_store.put_state_root(&state_root)?
            .map_err(|e| SyncError::StateStoreError(e))?;

        Ok(())
    }

    /// Sync fast (recent state only)
    fn sync_fast(
        store: &dyn KVStore,
        state_store: &StateStore,
        block_store: &BlockStore,
        config: &SyncConfig,
        progress: Arc<Mutex<SyncProgress>>,
        network_client: Arc<dyn NetworkClient>,
        target_height: u64,
    ) -> SyncResult<()> {
        // In a real implementation, this would sync only recent state
        // For now, we'll just call sync_full
        Self::sync_full(
            store,
            state_store,
            block_store,
            config,
            progress,
            network_client,
            target_height,
        )
    }

    /// Sync light (state roots only)
    fn sync_light(
        store: &dyn KVStore,
        state_store: &StateStore,
        block_store: &BlockStore,
        config: &SyncConfig,
        progress: Arc<Mutex<SyncProgress>>,
        network_client: Arc<dyn NetworkClient>,
        target_height: u64,
    ) -> SyncResult<()> {
        // Get state root at target height
        let state_root = network_client.get_state_root(target_height)?;

        // Store state root
        state_store.put_state_root(&state_root)?
            .map_err(|e| SyncError::StateStoreError(e))?;

        // Update progress
        let mut progress_guard = progress.lock().unwrap();
        progress_guard.accounts_synced = 1;
        progress_guard.total_accounts = 1;
        progress_guard.current_height = target_height;
        progress_guard.last_update_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        drop(progress_guard);

        Ok(())
    }
}

/// Network client trait for fetching state data
pub trait NetworkClient: Send + Sync {
    /// Get state root at height
    fn get_state_root(&self, height: u64) -> SyncResult<StateRoot>;

    /// Get account state
    fn get_account(&self, address: &[u8], height: u64) -> SyncResult<Option<AccountState>>;

    /// Get account proof
    fn get_account_proof(&self, address: &[u8], height: u64) -> SyncResult<Vec<u8>>;

    /// Get accounts in range
    fn get_accounts_in_range(&self, start_key: &[u8], end_key: &[u8], height: u64, limit: usize)
        -> SyncResult<Vec<(Vec<u8>, AccountState)>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::kv_store::RocksDBStore;
    use crate::storage::state::AccountType;
    use std::collections::HashMap;

    // Mock implementation of NetworkClient for testing
    struct MockNetworkClient {
        state_roots: HashMap<u64, StateRoot>,
        accounts: HashMap<Vec<u8>, AccountState>,
    }

    impl MockNetworkClient {
        fn new() -> Self {
            Self {
                state_roots: HashMap::new(),
                accounts: HashMap::new(),
            }
        }

        fn add_state_root(&mut self, height: u64, root: StateRoot) {
            self.state_roots.insert(height, root);
        }

        fn add_account(&mut self, address: Vec<u8>, account: AccountState) {
            self.accounts.insert(address, account);
        }
    }

    impl NetworkClient for MockNetworkClient {
        fn get_state_root(&self, height: u64) -> SyncResult<StateRoot> {
            match self.state_roots.get(&height) {
                Some(root) => Ok(root.clone()),
                None => Err(SyncError::InvalidBlockHeight(height)),
            }
        }

        fn get_account(&self, address: &[u8], _height: u64) -> SyncResult<Option<AccountState>> {
            Ok(self.accounts.get(address).cloned())
        }

        fn get_account_proof(&self, _address: &[u8], _height: u64) -> SyncResult<Vec<u8>> {
            // Mock implementation - just return empty proof
            Ok(Vec::new())
        }

        fn get_accounts_in_range(&self, start_key: &[u8], end_key: &[u8], _height: u64, limit: usize)
            -> SyncResult<Vec<(Vec<u8>, AccountState)>> {
            let mut result = Vec::new();

            for (address, account) in &self.accounts {
                if address >= start_key && address < end_key && result.len() < limit {
                    result.push((address.clone(), account.clone()));
                }
            }

            // Sort by address
            result.sort_by(|(a, _), (b, _)| a.cmp(b));

            Ok(result)
        }
    }

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

    // Helper function to create a test state root
    fn create_test_state_root(height: u64) -> StateRoot {
        let mut root_hash = [0u8; 32];
        root_hash[0] = (height % 256) as u8;

        StateRoot::new(root_hash, height, 12345)
    }

    // Helper function to create a test account
    fn create_test_account(balance: u64, nonce: u64, height: u64) -> AccountState {
        let mut account = AccountState::new_user(balance, height);
        account.nonce = nonce;
        account
    }

    // Helper function to create a test address
    fn create_test_address(id: u8) -> Vec<u8> {
        let mut address = vec![0u8; 32];
        address[0] = id;
        address
    }

    #[test]
    fn test_sync_light() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a mock network client
        let mut mock_client = MockNetworkClient::new();

        // Add a state root
        let height = 100;
        let state_root = create_test_state_root(height);
        mock_client.add_state_root(height, state_root.clone());

        // Create a synchronizer
        let config = SyncConfig {
            mode: SyncMode::Light,
            ..Default::default()
        };

        let mut synchronizer = StateSynchronizer::new(&kv_store, &state_store, &block_store, config);
        synchronizer.set_network_client(Arc::new(mock_client));

        // Start sync
        synchronizer.start_sync(height).unwrap();

        // Wait for sync to complete
        let mut attempts = 0;
        while attempts < 10 {
            let progress = synchronizer.get_progress();
            if progress.status == SyncStatus::Completed {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            attempts += 1;
        }

        // Verify sync completed
        let progress = synchronizer.get_progress();
        assert_eq!(progress.status, SyncStatus::Completed);
        assert_eq!(progress.current_height, height);

        // Verify state root was stored
        let stored_root = state_store.get_state_root_at_height(height).unwrap().unwrap();
        assert_eq!(stored_root.root_hash, state_root.root_hash);
    }

    #[test]
    fn test_sync_full() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create a mock network client
        let mut mock_client = MockNetworkClient::new();

        // Add a state root
        let height = 100;
        let state_root = create_test_state_root(height);
        mock_client.add_state_root(height, state_root.clone());

        // Add some accounts
        for i in 0..10 {
            let address = create_test_address(i);
            let account = create_test_account(1000 * (i as u64 + 1), i as u64, height);
            mock_client.add_account(address, account);
        }

        // Create a synchronizer
        let config = SyncConfig {
            mode: SyncMode::Full,
            ..Default::default()
        };

        let mut synchronizer = StateSynchronizer::new(&kv_store, &state_store, &block_store, config);
        synchronizer.set_network_client(Arc::new(mock_client));

        // Start sync
        synchronizer.start_sync(height).unwrap();

        // Wait for sync to complete
        let mut attempts = 0;
        while attempts < 10 {
            let progress = synchronizer.get_progress();
            if progress.status == SyncStatus::Completed {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            attempts += 1;
        }

        // Verify sync completed
        let progress = synchronizer.get_progress();
        assert_eq!(progress.status, SyncStatus::Completed);
        assert_eq!(progress.current_height, height);

        // Verify accounts were stored
        for i in 0..10 {
            let address = create_test_address(i);
            let account = state_store.get_account(&address, height).unwrap().unwrap();
            assert_eq!(account.balance, 1000 * (i as u64 + 1));
            assert_eq!(account.nonce, i as u64);
        }
    }
}
