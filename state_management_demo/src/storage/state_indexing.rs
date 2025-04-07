//! State indexing for VibeCoin blockchain
//!
//! This module provides functionality for indexing blockchain state data,
//! enabling efficient queries and analytics.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use thiserror::Error;
use log::{debug, error, info, warn};

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::state::{AccountState, StateRoot, StateError};
use crate::storage::state_store::{StateStore, StateStoreError};
use crate::storage::block_store::{BlockStore, Hash, Block};

/// Error type for state indexing operations
#[derive(Debug, Error)]
pub enum IndexingError {
    /// KV store error
    #[error("KV store error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// State store error
    #[error("State store error: {0}")]
    StateStoreError(#[from] StateStoreError),

    /// State error
    #[error("State error: {0}")]
    StateError(#[from] StateError),

    /// Invalid index name
    #[error("Invalid index name: {0}")]
    InvalidIndexName(String),

    /// Invalid index configuration
    #[error("Invalid index configuration: {0}")]
    InvalidIndexConfig(String),

    /// Index already exists
    #[error("Index already exists: {0}")]
    IndexAlreadyExists(String),

    /// Index not found
    #[error("Index not found: {0}")]
    IndexNotFound(String),

    /// Indexing already in progress
    #[error("Indexing already in progress")]
    IndexingInProgress,

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for indexing operations
pub type IndexingResult<T> = Result<T, IndexingError>;

/// Index type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    /// Balance index (for querying accounts by balance)
    Balance,

    /// Account type index (for querying accounts by type)
    AccountType,

    /// Transaction count index (for querying accounts by transaction count)
    TransactionCount,

    /// Custom index (for custom indexing logic)
    Custom,
}

/// Index configuration
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// Index name
    pub name: String,

    /// Index type
    pub index_type: IndexType,

    /// Whether to index historical data
    pub historical: bool,

    /// Custom indexing function (for custom indexes)
    pub custom_index_fn: Option<Arc<dyn Fn(&AccountState) -> Vec<String> + Send + Sync>>,
}

/// Indexing status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexingStatus {
    /// Not indexing
    Idle,

    /// Indexing in progress
    InProgress,

    /// Indexing completed
    Completed,

    /// Indexing failed
    Failed,
}

/// Indexing progress
#[derive(Debug, Clone)]
pub struct IndexingProgress {
    /// Current status
    pub status: IndexingStatus,

    /// Target block height
    pub target_height: u64,

    /// Current block height
    pub current_height: u64,

    /// Number of accounts indexed
    pub accounts_indexed: u64,

    /// Total accounts to index
    pub total_accounts: u64,

    /// Start time (Unix timestamp)
    pub start_time: u64,

    /// Last update time (Unix timestamp)
    pub last_update_time: u64,
}

impl IndexingProgress {
    /// Create a new indexing progress
    pub fn new(target_height: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            status: IndexingStatus::Idle,
            target_height,
            current_height: 0,
            accounts_indexed: 0,
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

        (self.accounts_indexed as f64 / self.total_accounts as f64) * 100.0
    }

    /// Calculate indexing speed (accounts per second)
    pub fn speed(&self) -> f64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let elapsed = now.saturating_sub(self.start_time);
        if elapsed == 0 {
            return 0.0;
        }

        self.accounts_indexed as f64 / elapsed as f64
    }

    /// Estimate time remaining (in seconds)
    pub fn estimated_time_remaining(&self) -> u64 {
        let speed = self.speed();
        if speed <= 0.0 {
            return 0;
        }

        let remaining_accounts = self.total_accounts.saturating_sub(self.accounts_indexed);
        (remaining_accounts as f64 / speed) as u64
    }
}

/// State index
pub struct StateIndex<'a> {
    /// Index name
    name: String,

    /// Index configuration
    config: IndexConfig,

    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// State store
    state_store: &'a StateStore<'a>,

    /// Indexing progress
    progress: Mutex<IndexingProgress>,

    /// Whether the index is enabled
    enabled: bool,
}

impl<'a> StateIndex<'a> {
    /// Create a new state index
    pub fn new(
        name: &str,
        config: IndexConfig,
        store: &'a dyn KVStore,
        state_store: &'a StateStore<'a>,
    ) -> Self {
        let target_height = 0; // Will be updated during indexing
        let progress = Mutex::new(IndexingProgress::new(target_height));

        Self {
            name: name.to_string(),
            config,
            store,
            state_store,
            progress,
            enabled: true,
        }
    }

    /// Get index name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get index configuration
    pub fn config(&self) -> &IndexConfig {
        &self.config
    }

    /// Get indexing progress
    pub fn get_progress(&self) -> IndexingProgress {
        self.progress.lock().unwrap().clone()
    }

    /// Enable the index
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable the index
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if the index is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Index an account
    pub fn index_account(&self, address: &[u8], account: &AccountState, height: u64) -> IndexingResult<()> {
        if !self.enabled {
            return Ok(());
        }

        match self.config.index_type {
            IndexType::Balance => self.index_balance(address, account, height),
            IndexType::AccountType => self.index_account_type(address, account, height),
            IndexType::TransactionCount => self.index_transaction_count(address, account, height),
            IndexType::Custom => self.index_custom(address, account, height),
        }
    }

    /// Index account by balance
    fn index_balance(&self, address: &[u8], account: &AccountState, height: u64) -> IndexingResult<()> {
        let balance = account.balance;
        let key = format!("index:{}:balance:{}:{}", self.name, balance, hex::encode(address));

        self.store.put(key.as_bytes(), &[])
            .map_err(IndexingError::KVStoreError)?;

        if self.config.historical {
            let historical_key = format!("index:{}:balance:{}:{}:{}", self.name, balance, height, hex::encode(address));
            self.store.put(historical_key.as_bytes(), &[])
                .map_err(IndexingError::KVStoreError)?;
        }

        Ok(())
    }

    /// Index account by account type
    fn index_account_type(&self, address: &[u8], account: &AccountState, height: u64) -> IndexingResult<()> {
        let account_type = format!("{:?}", account.account_type);
        let key = format!("index:{}:account_type:{}:{}", self.name, account_type, hex::encode(address));

        self.store.put(key.as_bytes(), &[])
            .map_err(IndexingError::KVStoreError)?;

        if self.config.historical {
            let historical_key = format!("index:{}:account_type:{}:{}:{}", self.name, account_type, height, hex::encode(address));
            self.store.put(historical_key.as_bytes(), &[])
                .map_err(IndexingError::KVStoreError)?;
        }

        Ok(())
    }

    /// Index account by transaction count (nonce)
    fn index_transaction_count(&self, address: &[u8], account: &AccountState, height: u64) -> IndexingResult<()> {
        let nonce = account.nonce;
        let key = format!("index:{}:tx_count:{}:{}", self.name, nonce, hex::encode(address));

        self.store.put(key.as_bytes(), &[])
            .map_err(IndexingError::KVStoreError)?;

        if self.config.historical {
            let historical_key = format!("index:{}:tx_count:{}:{}:{}", self.name, nonce, height, hex::encode(address));
            self.store.put(historical_key.as_bytes(), &[])
                .map_err(IndexingError::KVStoreError)?;
        }

        Ok(())
    }

    /// Index account using custom function
    fn index_custom(&self, address: &[u8], account: &AccountState, height: u64) -> IndexingResult<()> {
        if let Some(custom_fn) = &self.config.custom_index_fn {
            let index_values = custom_fn(account);

            for value in index_values {
                let key = format!("index:{}:custom:{}:{}", self.name, value, hex::encode(address));

                self.store.put(key.as_bytes(), &[])
                    .map_err(IndexingError::KVStoreError)?;

                if self.config.historical {
                    let historical_key = format!("index:{}:custom:{}:{}:{}", self.name, value, height, hex::encode(address));
                    self.store.put(historical_key.as_bytes(), &[])
                        .map_err(IndexingError::KVStoreError)?;
                }
            }
        } else {
            return Err(IndexingError::InvalidIndexConfig(
                "Custom index function not provided".to_string(),
            ));
        }

        Ok(())
    }

    /// Query accounts by balance range
    pub fn query_by_balance_range(
        &self,
        min_balance: u64,
        max_balance: u64,
        limit: usize,
        height: Option<u64>,
    ) -> IndexingResult<Vec<(Vec<u8>, AccountState)>> {
        if self.config.index_type != IndexType::Balance {
            return Err(IndexingError::InvalidIndexConfig(
                format!("Index {} is not a balance index", self.name),
            ));
        }

        let mut results = Vec::new();

        if let Some(height) = height {
            // Query historical data
            if !self.config.historical {
                return Err(IndexingError::InvalidIndexConfig(
                    format!("Index {} does not support historical queries", self.name),
                ));
            }

            for balance in min_balance..=max_balance {
                let prefix = format!("index:{}:balance:{}:{}", self.name, balance, height);
                let entries = self.store.scan_prefix(prefix.as_bytes())
                    .map_err(IndexingError::KVStoreError)?;

                for (key, _) in entries {
                    if results.len() >= limit {
                        break;
                    }

                    // Extract address from key
                    let key_str = String::from_utf8_lossy(&key);
                    let parts: Vec<&str> = key_str.split(':').collect();
                    if parts.len() >= 5 {
                        if let Ok(address) = hex::decode(parts[4]) {
                            if let Some(account) = self.state_store.get_account(&address, height)? {
                                results.push((address, account));
                            }
                        }
                    }
                }

                if results.len() >= limit {
                    break;
                }
            }
        } else {
            // Query current data
            for balance in min_balance..=max_balance {
                let prefix = format!("index:{}:balance:{}:", self.name, balance);
                let entries = self.store.scan_prefix(prefix.as_bytes())
                    .map_err(IndexingError::KVStoreError)?;

                for (key, _) in entries {
                    if results.len() >= limit {
                        break;
                    }

                    // Extract address from key
                    let key_str = String::from_utf8_lossy(&key);
                    let parts: Vec<&str> = key_str.split(':').collect();
                    if parts.len() >= 4 {
                        if let Ok(address) = hex::decode(parts[3]) {
                            if let Some(account) = self.state_store.get_latest_account(&address)? {
                                results.push((address, account));
                            }
                        }
                    }
                }

                if results.len() >= limit {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Query accounts by account type
    pub fn query_by_account_type(
        &self,
        account_type: &str,
        limit: usize,
        height: Option<u64>,
    ) -> IndexingResult<Vec<(Vec<u8>, AccountState)>> {
        if self.config.index_type != IndexType::AccountType {
            return Err(IndexingError::InvalidIndexConfig(
                format!("Index {} is not an account type index", self.name),
            ));
        }

        let mut results = Vec::new();

        if let Some(height) = height {
            // Query historical data
            if !self.config.historical {
                return Err(IndexingError::InvalidIndexConfig(
                    format!("Index {} does not support historical queries", self.name),
                ));
            }

            let prefix = format!("index:{}:account_type:{}:{}:", self.name, account_type, height);
            let entries = self.store.scan_prefix(prefix.as_bytes())
                .map_err(IndexingError::KVStoreError)?;

            for (key, _) in entries {
                if results.len() >= limit {
                    break;
                }

                // Extract address from key
                let key_str = String::from_utf8_lossy(&key);
                let parts: Vec<&str> = key_str.split(':').collect();
                if parts.len() >= 5 {
                    if let Ok(address) = hex::decode(parts[4]) {
                        if let Some(account) = self.state_store.get_account(&address, height)? {
                            results.push((address, account));
                        }
                    }
                }
            }
        } else {
            // Query current data
            let prefix = format!("index:{}:account_type:{}:", self.name, account_type);
            let entries = self.store.scan_prefix(prefix.as_bytes())
                .map_err(IndexingError::KVStoreError)?;

            for (key, _) in entries {
                if results.len() >= limit {
                    break;
                }

                // Extract address from key
                let key_str = String::from_utf8_lossy(&key);
                let parts: Vec<&str> = key_str.split(':').collect();
                if parts.len() >= 4 {
                    if let Ok(address) = hex::decode(parts[3]) {
                        if let Some(account) = self.state_store.get_latest_account(&address)? {
                            results.push((address, account));
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query accounts by custom index
    pub fn query_by_custom(
        &self,
        value: &str,
        limit: usize,
        height: Option<u64>,
    ) -> IndexingResult<Vec<(Vec<u8>, AccountState)>> {
        if self.config.index_type != IndexType::Custom {
            return Err(IndexingError::InvalidIndexConfig(
                format!("Index {} is not a custom index", self.name),
            ));
        }

        let mut results = Vec::new();

        if let Some(height) = height {
            // Query historical data
            if !self.config.historical {
                return Err(IndexingError::InvalidIndexConfig(
                    format!("Index {} does not support historical queries", self.name),
                ));
            }

            let prefix = format!("index:{}:custom:{}:{}:", self.name, value, height);
            let entries = self.store.scan_prefix(prefix.as_bytes())
                .map_err(IndexingError::KVStoreError)?;

            for (key, _) in entries {
                if results.len() >= limit {
                    break;
                }

                // Extract address from key
                let key_str = String::from_utf8_lossy(&key);
                let parts: Vec<&str> = key_str.split(':').collect();
                if parts.len() >= 5 {
                    if let Ok(address) = hex::decode(parts[4]) {
                        if let Some(account) = self.state_store.get_account(&address, height)? {
                            results.push((address, account));
                        }
                    }
                }
            }
        } else {
            // Query current data
            let prefix = format!("index:{}:custom:{}:", self.name, value);
            let entries = self.store.scan_prefix(prefix.as_bytes())
                .map_err(IndexingError::KVStoreError)?;

            for (key, _) in entries {
                if results.len() >= limit {
                    break;
                }

                // Extract address from key
                let key_str = String::from_utf8_lossy(&key);
                let parts: Vec<&str> = key_str.split(':').collect();
                if parts.len() >= 4 {
                    if let Ok(address) = hex::decode(parts[3]) {
                        if let Some(account) = self.state_store.get_latest_account(&address)? {
                            results.push((address, account));
                        }
                    }
                }
            }
        }

        Ok(results)
    }
}

/// State indexing manager
pub struct StateIndexingManager<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// State store
    state_store: &'a StateStore<'a>,

    /// Block store
    block_store: &'a BlockStore<'a>,

    /// Indexes
    indexes: RwLock<HashMap<String, Arc<StateIndex<'a>>>>,

    /// Indexing in progress flag
    indexing_in_progress: std::sync::atomic::AtomicBool,
}

impl<'a> StateIndexingManager<'a> {
    /// Create a new state indexing manager
    pub fn new(
        store: &'a dyn KVStore,
        state_store: &'a StateStore<'a>,
        block_store: &'a BlockStore<'a>,
    ) -> Self {
        Self {
            store,
            state_store,
            block_store,
            indexes: RwLock::new(HashMap::new()),
            indexing_in_progress: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Create a new index
    pub fn create_index(&self, name: &str, config: IndexConfig) -> IndexingResult<()> {
        let mut indexes = self.indexes.write().unwrap();

        // Check if index already exists
        if indexes.contains_key(name) {
            return Err(IndexingError::IndexAlreadyExists(name.to_string()));
        }

        // Create the index
        let index = Arc::new(StateIndex::new(name, config, self.store, self.state_store));
        indexes.insert(name.to_string(), index);

        Ok(())
    }

    /// Get an index by name
    pub fn get_index(&self, name: &str) -> IndexingResult<Arc<StateIndex<'a>>> {
        let indexes = self.indexes.read().unwrap();

        match indexes.get(name) {
            Some(index) => Ok(Arc::clone(index)),
            None => Err(IndexingError::IndexNotFound(name.to_string())),
        }
    }

    /// Delete an index
    pub fn delete_index(&self, name: &str) -> IndexingResult<()> {
        let mut indexes = self.indexes.write().unwrap();

        // Check if index exists
        if !indexes.contains_key(name) {
            return Err(IndexingError::IndexNotFound(name.to_string()));
        }

        // Remove the index
        indexes.remove(name);

        // Delete index data from the store
        let prefix = format!("index:{}:", name);
        let entries = self.store.scan_prefix(prefix.as_bytes())
            .map_err(IndexingError::KVStoreError)?;

        let mut batch = Vec::new();
        for (key, _) in entries {
            batch.push(WriteBatchOperation::Delete {
                key: key.clone(),
            });
        }

        if !batch.is_empty() {
            self.store.write_batch(batch)
                .map_err(IndexingError::KVStoreError)?;
        }

        Ok(())
    }

    /// List all indexes
    pub fn list_indexes(&self) -> Vec<String> {
        let indexes = self.indexes.read().unwrap();
        indexes.keys().cloned().collect()
    }

    /// Index an account in all indexes
    pub fn index_account(&self, address: &[u8], account: &AccountState, height: u64) -> IndexingResult<()> {
        let indexes = self.indexes.read().unwrap();

        for (_, index) in indexes.iter() {
            index.index_account(address, account, height)?;
        }

        Ok(())
    }

    /// Start indexing
    pub fn start_indexing(&self, target_height: u64) -> IndexingResult<()> {
        // Check if indexing is already in progress
        let was_indexing = self.indexing_in_progress.swap(true, std::sync::atomic::Ordering::SeqCst);
        if was_indexing {
            return Err(IndexingError::IndexingInProgress);
        }

        // Ensure we reset the flag when we're done
        let _guard = scopeguard::guard((), |_| {
            self.indexing_in_progress.store(false, std::sync::atomic::Ordering::SeqCst);
        });

        // Get all accounts
        let accounts = self.get_all_accounts(target_height)?;

        // Update progress for all indexes
        let indexes = self.indexes.read().unwrap();
        for (_, index) in indexes.iter() {
            let mut progress = index.progress.lock().unwrap();
            progress.status = IndexingStatus::InProgress;
            progress.target_height = target_height;
            progress.current_height = 0;
            progress.accounts_indexed = 0;
            progress.total_accounts = accounts.len() as u64;
            progress.start_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            progress.last_update_time = progress.start_time;
        }

        // Index all accounts
        let mut accounts_indexed = 0;
        for (address, account) in accounts {
            self.index_account(&address, &account, target_height)?;
            accounts_indexed += 1;

            // Update progress every 100 accounts
            if accounts_indexed % 100 == 0 {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                for (_, index) in indexes.iter() {
                    let mut progress = index.progress.lock().unwrap();
                    progress.accounts_indexed = accounts_indexed;
                    progress.last_update_time = now;
                }
            }
        }

        // Update final progress
        for (_, index) in indexes.iter() {
            let mut progress = index.progress.lock().unwrap();
            progress.status = IndexingStatus::Completed;
            progress.current_height = target_height;
            progress.accounts_indexed = accounts_indexed;
            progress.last_update_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }

        Ok(())
    }

    /// Get all accounts at a specific height
    fn get_all_accounts(&self, height: u64) -> IndexingResult<Vec<(Vec<u8>, AccountState)>> {
        // In a real implementation, this would efficiently retrieve all accounts
        // For now, we'll use a simple scan of the state store

        let mut accounts = Vec::new();
        let prefix = "account:";
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(IndexingError::KVStoreError)?;

        for (key, value) in results {
            // Extract address from key
            let key_str = String::from_utf8_lossy(&key);
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() >= 2 {
                if let Ok(address) = hex::decode(parts[1]) {
                    if let Some(account) = self.state_store.get_account(&address, height)? {
                        accounts.push((address, account));
                    }
                }
            }
        }

        Ok(accounts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::kv_store::RocksDBStore;
    use crate::storage::state::AccountType;

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

    // Helper function to create a test account
    fn create_test_account(balance: u64, nonce: u64, account_type: AccountType, height: u64) -> AccountState {
        match account_type {
            AccountType::User => {
                let mut account = AccountState::new_user(balance, height);
                account.nonce = nonce;
                account
            },
            AccountType::Contract => {
                let code = vec![1, 2, 3, 4]; // Dummy code
                let mut account = AccountState::new_contract(balance, code, height);
                account.nonce = nonce;
                account
            },
            AccountType::System => {
                let mut account = AccountState::new_system(balance, height);
                account.nonce = nonce;
                account
            },
            AccountType::Validator => {
                let mut account = AccountState::new_validator(balance, 1000, height);
                account.nonce = nonce;
                account
            },
        }
    }

    // Helper function to create a test address
    fn create_test_address(id: u8) -> Vec<u8> {
        let mut address = vec![0u8; 32];
        address[0] = id;
        address
    }

    #[test]
    fn test_create_and_get_index() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        let manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);

        // Create an index
        let config = IndexConfig {
            name: "balance_index".to_string(),
            index_type: IndexType::Balance,
            historical: true,
            custom_index_fn: None,
        };

        manager.create_index("balance_index", config).unwrap();

        // Get the index
        let index = manager.get_index("balance_index").unwrap();
        assert_eq!(index.name(), "balance_index");
        assert_eq!(index.config().index_type, IndexType::Balance);
        assert!(index.config().historical);

        // Try to get a non-existent index
        let result = manager.get_index("non_existent");
        assert!(result.is_err());
        match result {
            Err(IndexingError::IndexNotFound(_)) => {},
            _ => panic!("Expected IndexNotFound error"),
        }
    }

    #[test]
    fn test_index_and_query_by_balance() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create accounts in the state store
        let height = 100;

        let address1 = create_test_address(1);
        let account1 = create_test_account(1000, 1, AccountType::User, height);
        state_store.put_account(&address1, &account1, height).unwrap().unwrap();

        let address2 = create_test_address(2);
        let account2 = create_test_account(2000, 2, AccountType::User, height);
        state_store.put_account(&address2, &account2, height).unwrap().unwrap();

        let address3 = create_test_address(3);
        let account3 = create_test_account(3000, 3, AccountType::User, height);
        state_store.put_account(&address3, &account3, height).unwrap().unwrap();

        // Create an index manager
        let manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);

        // Create a balance index
        let config = IndexConfig {
            name: "balance_index".to_string(),
            index_type: IndexType::Balance,
            historical: true,
            custom_index_fn: None,
        };

        manager.create_index("balance_index", config).unwrap();

        // Index the accounts
        manager.index_account(&address1, &account1, height).unwrap();
        manager.index_account(&address2, &account2, height).unwrap();
        manager.index_account(&address3, &account3, height).unwrap();

        // Get the index
        let index = manager.get_index("balance_index").unwrap();

        // Query by balance range
        let results = index.query_by_balance_range(1500, 2500, 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, address2);
        assert_eq!(results[0].1.balance, 2000);

        // Query by balance range with height
        let results = index.query_by_balance_range(1500, 3500, 10, Some(height)).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1.balance, 2000);
        assert_eq!(results[1].1.balance, 3000);
    }

    #[test]
    fn test_index_and_query_by_account_type() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create accounts in the state store
        let height = 100;

        let address1 = create_test_address(1);
        let account1 = create_test_account(1000, 1, AccountType::User, height);
        state_store.put_account(&address1, &account1, height).unwrap().unwrap();

        let address2 = create_test_address(2);
        let account2 = create_test_account(2000, 2, AccountType::Contract, height);
        state_store.put_account(&address2, &account2, height).unwrap().unwrap();

        let address3 = create_test_address(3);
        let account3 = create_test_account(3000, 3, AccountType::Validator, height);
        state_store.put_account(&address3, &account3, height).unwrap().unwrap();

        // Create an index manager
        let manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);

        // Create an account type index
        let config = IndexConfig {
            name: "account_type_index".to_string(),
            index_type: IndexType::AccountType,
            historical: true,
            custom_index_fn: None,
        };

        manager.create_index("account_type_index", config).unwrap();

        // Index the accounts
        manager.index_account(&address1, &account1, height).unwrap();
        manager.index_account(&address2, &account2, height).unwrap();
        manager.index_account(&address3, &account3, height).unwrap();

        // Get the index
        let index = manager.get_index("account_type_index").unwrap();

        // Query by account type
        let results = index.query_by_account_type("User", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, address1);

        // Query by account type with height
        let results = index.query_by_account_type("Validator", 10, Some(height)).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, address3);
    }

    #[test]
    fn test_custom_index() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        // Create accounts in the state store
        let height = 100;

        let address1 = create_test_address(1);
        let account1 = create_test_account(1000, 1, AccountType::User, height);
        state_store.put_account(&address1, &account1, height).unwrap().unwrap();

        let address2 = create_test_address(2);
        let account2 = create_test_account(2000, 2, AccountType::User, height);
        state_store.put_account(&address2, &account2, height).unwrap().unwrap();

        // Create an index manager
        let manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);

        // Create a custom index that categorizes accounts by balance tier
        let custom_fn = Arc::new(|account: &AccountState| -> Vec<String> {
            let tier = if account.balance < 1500 {
                "low"
            } else if account.balance < 2500 {
                "medium"
            } else {
                "high"
            };

            vec![tier.to_string()]
        });

        let config = IndexConfig {
            name: "balance_tier_index".to_string(),
            index_type: IndexType::Custom,
            historical: true,
            custom_index_fn: Some(custom_fn),
        };

        manager.create_index("balance_tier_index", config).unwrap();

        // Index the accounts
        manager.index_account(&address1, &account1, height).unwrap();
        manager.index_account(&address2, &account2, height).unwrap();

        // Get the index
        let index = manager.get_index("balance_tier_index").unwrap();

        // Query by custom index
        let results = index.query_by_custom("low", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, address1);

        let results = index.query_by_custom("medium", 10, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, address2);
    }

    #[test]
    fn test_delete_index() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        let manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);

        // Create an index
        let config = IndexConfig {
            name: "balance_index".to_string(),
            index_type: IndexType::Balance,
            historical: true,
            custom_index_fn: None,
        };

        manager.create_index("balance_index", config).unwrap();

        // Verify the index exists
        assert!(manager.get_index("balance_index").is_ok());

        // Delete the index
        manager.delete_index("balance_index").unwrap();

        // Verify the index no longer exists
        assert!(manager.get_index("balance_index").is_err());
    }
}