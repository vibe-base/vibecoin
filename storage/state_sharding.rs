//! State sharding for VibeCoin blockchain
//!
//! This module provides functionality for sharding the blockchain state,
//! enabling horizontal scalability and improved performance.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use thiserror::Error;
use log::{debug, error, info, warn};

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::state::{AccountState, StateRoot, StateError};
use crate::storage::state_store::{StateStore, StateStoreError};
use crate::storage::block_store::{BlockStore, Hash};

/// Error type for state sharding operations
#[derive(Debug, Error)]
pub enum ShardingError {
    /// KV store error
    #[error("KV store error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// State store error
    #[error("State store error: {0}")]
    StateStoreError(#[from] StateStoreError),

    /// State error
    #[error("State error: {0}")]
    StateError(#[from] StateError),

    /// Invalid shard ID
    #[error("Invalid shard ID: {0}")]
    InvalidShardId(u32),

    /// Invalid shard count
    #[error("Invalid shard count: {0}")]
    InvalidShardCount(u32),

    /// Shard not found
    #[error("Shard not found: {0}")]
    ShardNotFound(u32),

    /// Account not found
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    /// Invalid address
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// Shard already exists
    #[error("Shard already exists: {0}")]
    ShardAlreadyExists(u32),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for sharding operations
pub type ShardingResult<T> = Result<T, ShardingError>;

/// Sharding strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardingStrategy {
    /// Shard by address prefix
    AddressPrefix,

    /// Shard by address modulo
    AddressModulo,

    /// Shard by account type
    AccountType,

    /// Shard by custom function
    Custom,
}

/// Shard configuration
#[derive(Clone)]
pub struct ShardConfig {
    /// Number of shards
    pub shard_count: u32,

    /// Sharding strategy
    pub strategy: ShardingStrategy,

    /// Whether to use separate databases for each shard
    pub separate_databases: bool,

    /// Base directory for shard databases
    pub base_dir: Option<std::path::PathBuf>,

    /// Custom shard mapping function
    pub custom_shard_fn: Option<Arc<dyn Fn(&[u8]) -> u32 + Send + Sync>>,
}

impl std::fmt::Debug for ShardConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShardConfig")
            .field("shard_count", &self.shard_count)
            .field("strategy", &self.strategy)
            .field("base_dir", &self.base_dir)
            .field("custom_shard_fn", &if self.custom_shard_fn.is_some() { "<function>" } else { "None" })
            .finish()
    }
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            shard_count: 4,
            strategy: ShardingStrategy::AddressModulo,
            separate_databases: false,
            base_dir: None,
            custom_shard_fn: None,
        }
    }
}

/// Shard info
#[derive(Debug, Clone)]
pub struct ShardInfo {
    /// Shard ID
    pub id: u32,

    /// Number of accounts in the shard
    pub account_count: u64,

    /// Total balance in the shard
    pub total_balance: u64,

    /// Last updated block height
    pub last_updated: u64,

    /// Shard state root
    pub state_root: Option<Hash>,
}

/// State shard
pub struct StateShard<'a> {
    /// Shard ID
    id: u32,

    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// State store
    state_store: &'a StateStore<'a>,

    /// Shard configuration
    config: ShardConfig,

    /// Shard info
    info: RwLock<ShardInfo>,
}

impl<'a> StateShard<'a> {
    /// Create a new state shard
    pub fn new(
        id: u32,
        store: &'a dyn KVStore,
        state_store: &'a StateStore<'a>,
        config: ShardConfig,
    ) -> Self {
        let info = ShardInfo {
            id,
            account_count: 0,
            total_balance: 0,
            last_updated: 0,
            state_root: None,
        };

        Self {
            id,
            store,
            state_store,
            config,
            info: RwLock::new(info),
        }
    }

    /// Get shard info
    pub fn get_info(&self) -> ShardInfo {
        self.info.read().unwrap().clone()
    }

    /// Update shard info
    pub fn update_info(&self, block_height: u64) -> ShardingResult<()> {
        let mut info = self.info.write().unwrap();

        // Count accounts in this shard
        let account_count = self.count_accounts()?;

        // Calculate total balance
        let total_balance = self.calculate_total_balance()?;

        // Calculate state root
        let state_root = self.calculate_state_root()?;

        // Update info
        info.account_count = account_count;
        info.total_balance = total_balance;
        info.last_updated = block_height;
        info.state_root = Some(state_root);

        Ok(())
    }

    /// Count accounts in this shard
    fn count_accounts(&self) -> ShardingResult<u64> {
        let prefix = format!("shard:{}:account:", self.id);
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(ShardingError::KVStoreError)?;

        Ok(results.len() as u64)
    }

    /// Calculate total balance in this shard
    fn calculate_total_balance(&self) -> ShardingResult<u64> {
        let mut total_balance = 0;

        let prefix = format!("shard:{}:account:", self.id);
        let results = self.store.scan_prefix(prefix.as_bytes())
            .map_err(ShardingError::KVStoreError)?;

        for (_, value) in results {
            let account: AccountState = bincode::deserialize(&value)
                .map_err(|e| ShardingError::Other(format!("Failed to deserialize account: {}", e)))?;

            total_balance += account.balance;
        }

        Ok(total_balance)
    }

    /// Calculate state root for this shard
    fn calculate_state_root(&self) -> ShardingResult<Hash> {
        // In a real implementation, this would calculate a Merkle root of all accounts in the shard
        // For now, we'll just return a dummy hash
        let mut root = [0u8; 32];
        root[0] = self.id as u8;
        Ok(root)
    }

    /// Get account from this shard
    pub fn get_account(&self, address: &[u8]) -> ShardingResult<Option<AccountState>> {
        let key = format!("shard:{}:account:{}", self.id, hex::encode(address));

        let result = self.store.get(key.as_bytes())
            .map_err(ShardingError::KVStoreError)?;

        match result {
            Some(value) => {
                let account: AccountState = bincode::deserialize(&value)
                    .map_err(|e| ShardingError::Other(format!("Failed to deserialize account: {}", e)))?;

                Ok(Some(account))
            },
            None => Ok(None),
        }
    }

    /// Put account in this shard
    pub fn put_account(&self, address: &[u8], account: &AccountState) -> ShardingResult<()> {
        let key = format!("shard:{}:account:{}", self.id, hex::encode(address));

        let value = bincode::serialize(account)
            .map_err(|e| ShardingError::Other(format!("Failed to serialize account: {}", e)))?;

        self.store.put(key.as_bytes(), &value)
            .map_err(ShardingError::KVStoreError)?;

        // Update shard info
        let mut info = self.info.write().unwrap();
        info.account_count += 1;
        info.total_balance += account.balance;
        info.last_updated = account.last_updated;

        Ok(())
    }

    /// Delete account from this shard
    pub fn delete_account(&self, address: &[u8]) -> ShardingResult<()> {
        // First get the account to update shard info
        let account = match self.get_account(address)? {
            Some(account) => account,
            None => return Err(ShardingError::AccountNotFound(hex::encode(address))),
        };

        let key = format!("shard:{}:account:{}", self.id, hex::encode(address));

        self.store.delete(key.as_bytes())
            .map_err(ShardingError::KVStoreError)?;

        // Update shard info
        let mut info = self.info.write().unwrap();
        info.account_count -= 1;
        info.total_balance -= account.balance;

        Ok(())
    }
}

/// State sharding manager
pub struct StateShardingManager<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// State store
    state_store: &'a StateStore<'a>,

    /// Block store
    block_store: &'a BlockStore<'a>,

    /// Sharding configuration
    config: ShardConfig,

    /// Shards
    shards: RwLock<HashMap<u32, Arc<StateShard<'a>>>>,
}

impl<'a> StateShardingManager<'a> {
    /// Create a new state sharding manager
    pub fn new(
        store: &'a dyn KVStore,
        state_store: &'a StateStore<'a>,
        block_store: &'a BlockStore<'a>,
        config: ShardConfig,
    ) -> Self {
        Self {
            store,
            state_store,
            block_store,
            config,
            shards: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize sharding
    pub fn init(&self) -> ShardingResult<()> {
        // Validate configuration
        if self.config.shard_count == 0 {
            return Err(ShardingError::InvalidShardCount(0));
        }

        // Create shards
        let mut shards = self.shards.write().unwrap();

        for id in 0..self.config.shard_count {
            let shard = Arc::new(StateShard::new(
                id,
                self.store,
                self.state_store,
                self.config.clone(),
            ));

            shards.insert(id, shard);
        }

        // Initialize shard info
        let current_height = match self.block_store.get_latest_height() {
            Some(height) => height,
            None => return Err(ShardingError::Other("Failed to get latest height".to_string())),
        };

        for (_, shard) in shards.iter() {
            shard.update_info(current_height)?;
        }

        Ok(())
    }

    /// Get shard for address
    pub fn get_shard_for_address(&self, address: &[u8]) -> ShardingResult<Arc<StateShard<'a>>> {
        let shard_id = self.calculate_shard_id(address)?;
        self.get_shard(shard_id)
    }

    /// Get shard by ID
    pub fn get_shard(&self, shard_id: u32) -> ShardingResult<Arc<StateShard<'a>>> {
        let shards = self.shards.read().unwrap();

        match shards.get(&shard_id) {
            Some(shard) => Ok(Arc::clone(shard)),
            None => Err(ShardingError::ShardNotFound(shard_id)),
        }
    }

    /// Calculate shard ID for address
    fn calculate_shard_id(&self, address: &[u8]) -> ShardingResult<u32> {
        if address.len() != 32 {
            return Err(ShardingError::InvalidAddress(hex::encode(address)));
        }

        match self.config.strategy {
            ShardingStrategy::AddressPrefix => {
                // Use first byte as shard ID
                let shard_id = address[0] as u32 % self.config.shard_count;
                Ok(shard_id)
            },
            ShardingStrategy::AddressModulo => {
                // Use modulo of address as shard ID
                let mut sum = 0u32;
                for &byte in address {
                    sum = sum.wrapping_add(byte as u32);
                }
                let shard_id = sum % self.config.shard_count;
                Ok(shard_id)
            },
            ShardingStrategy::AccountType => {
                // This would require knowing the account type
                // For now, we'll fall back to address modulo
                let mut sum = 0u32;
                for &byte in address {
                    sum = sum.wrapping_add(byte as u32);
                }
                let shard_id = sum % self.config.shard_count;
                Ok(shard_id)
            },
            ShardingStrategy::Custom => {
                // Use custom function if provided
                if let Some(custom_fn) = &self.config.custom_shard_fn {
                    let shard_id = custom_fn(address) % self.config.shard_count;
                    Ok(shard_id)
                } else {
                    Err(ShardingError::Other("Custom shard function not provided".to_string()))
                }
            },
        }
    }

    /// Get account from appropriate shard
    pub fn get_account(&self, address: &[u8]) -> ShardingResult<Option<AccountState>> {
        let shard = self.get_shard_for_address(address)?;
        shard.get_account(address)
    }

    /// Put account in appropriate shard
    pub fn put_account(&self, address: &[u8], account: &AccountState) -> ShardingResult<()> {
        let shard = self.get_shard_for_address(address)?;
        shard.put_account(address, account)
    }

    /// Delete account from appropriate shard
    pub fn delete_account(&self, address: &[u8]) -> ShardingResult<()> {
        let shard = self.get_shard_for_address(address)?;
        shard.delete_account(address)
    }

    /// Get all shard info
    pub fn get_all_shard_info(&self) -> ShardingResult<Vec<ShardInfo>> {
        let shards = self.shards.read().unwrap();

        let mut info = Vec::with_capacity(shards.len());
        for (_, shard) in shards.iter() {
            info.push(shard.get_info());
        }

        Ok(info)
    }

    /// Update all shard info
    pub fn update_all_shard_info(&self) -> ShardingResult<()> {
        let current_height = match self.block_store.get_latest_height() {
            Some(height) => height,
            None => return Err(ShardingError::Other("Failed to get latest height".to_string())),
        };

        let shards = self.shards.read().unwrap();

        for (_, shard) in shards.iter() {
            shard.update_info(current_height)?;
        }

        Ok(())
    }

    /// Rebalance shards
    pub fn rebalance_shards(&self) -> ShardingResult<RebalanceStats> {
        let mut stats = RebalanceStats::default();

        // Get all accounts
        let accounts = self.get_all_accounts()?;

        // Calculate ideal distribution
        let ideal_count_per_shard = accounts.len() as f64 / self.config.shard_count as f64;

        // Get current distribution
        let shards = self.shards.read().unwrap();
        let mut current_distribution = HashMap::new();
        for (id, shard) in shards.iter() {
            let info = shard.get_info();
            current_distribution.insert(*id, info.account_count);
        }

        // Calculate which accounts to move
        let mut accounts_to_move = Vec::new();

        for (address, account) in &accounts {
            let current_shard_id = self.calculate_shard_id(address)?;
            let current_count = current_distribution.get(&current_shard_id).copied().unwrap_or(0);

            // If this shard has more than ideal, consider moving accounts
            if (current_count as f64) > ideal_count_per_shard * 1.1 {
                // Find a shard with less than ideal
                for id in 0..self.config.shard_count {
                    if id == current_shard_id {
                        continue;
                    }

                    let count = current_distribution.get(&id).copied().unwrap_or(0);
                    if (count as f64) < ideal_count_per_shard * 0.9 {
                        // Move this account to the new shard
                        accounts_to_move.push((address.clone(), account.clone(), current_shard_id, id));

                        // Update distribution counts
                        *current_distribution.entry(current_shard_id).or_insert(0) -= 1;
                        *current_distribution.entry(id).or_insert(0) += 1;

                        break;
                    }
                }
            }
        }

        // Move accounts
        for (address, account, from_shard_id, to_shard_id) in accounts_to_move {
            // Delete from old shard
            let from_shard = self.get_shard(from_shard_id)?;
            from_shard.delete_account(&address)?;

            // Add to new shard
            let to_shard = self.get_shard(to_shard_id)?;
            to_shard.put_account(&address, &account)?;

            stats.accounts_moved += 1;
        }

        // Update shard info
        self.update_all_shard_info()?;

        stats.shards_rebalanced = self.config.shard_count;

        Ok(stats)
    }

    /// Get all accounts from all shards
    fn get_all_accounts(&self) -> ShardingResult<HashMap<Vec<u8>, AccountState>> {
        let mut accounts = HashMap::new();

        let shards = self.shards.read().unwrap();

        for (_, shard) in shards.iter() {
            let prefix = format!("shard:{}:account:", shard.id);
            let results = self.store.scan_prefix(prefix.as_bytes())
                .map_err(ShardingError::KVStoreError)?;

            for (key, value) in results {
                // Extract address from key
                let key_str = String::from_utf8_lossy(&key);
                let parts: Vec<&str> = key_str.split(':').collect();
                if parts.len() == 4 {
                    if let Ok(address) = hex::decode(parts[3]) {
                        let account: AccountState = bincode::deserialize(&value)
                            .map_err(|e| ShardingError::Other(format!("Failed to deserialize account: {}", e)))?;

                        accounts.insert(address, account);
                    }
                }
            }
        }

        Ok(accounts)
    }
}

/// Rebalance statistics
#[derive(Debug, Default, Clone, Copy)]
pub struct RebalanceStats {
    /// Number of shards rebalanced
    pub shards_rebalanced: u32,

    /// Number of accounts moved
    pub accounts_moved: u64,
}

impl std::fmt::Display for RebalanceStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Rebalance stats: {} shards rebalanced, {} accounts moved",
            self.shards_rebalanced, self.accounts_moved
        )
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
    fn create_test_account(balance: u64, block_height: u64) -> AccountState {
        AccountState::new_user(balance, block_height)
    }

    // Helper function to create a test address
    fn create_test_address(id: u8) -> [u8; 32] {
        let mut address = [0u8; 32];
        address[0] = id;
        address
    }

    #[test]
    fn test_shard_creation() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        let config = ShardConfig {
            shard_count: 4,
            ..Default::default()
        };

        let manager = StateShardingManager::new(&kv_store, &state_store, &block_store, config);
        manager.init().unwrap();

        // Verify shards were created
        for id in 0..4 {
            let shard = manager.get_shard(id).unwrap();
            let info = shard.get_info();
            assert_eq!(info.id, id);
            assert_eq!(info.account_count, 0);
            assert_eq!(info.total_balance, 0);
        }

        // Verify invalid shard ID
        assert!(manager.get_shard(4).is_err());
    }

    #[test]
    fn test_shard_for_address() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        let config = ShardConfig {
            shard_count: 4,
            strategy: ShardingStrategy::AddressPrefix,
            ..Default::default()
        };

        let manager = StateShardingManager::new(&kv_store, &state_store, &block_store, config);
        manager.init().unwrap();

        // Test address prefix strategy
        let address1 = create_test_address(0);
        let address2 = create_test_address(1);
        let address3 = create_test_address(2);
        let address4 = create_test_address(3);

        let shard1 = manager.get_shard_for_address(&address1).unwrap();
        let shard2 = manager.get_shard_for_address(&address2).unwrap();
        let shard3 = manager.get_shard_for_address(&address3).unwrap();
        let shard4 = manager.get_shard_for_address(&address4).unwrap();

        assert_eq!(shard1.id, 0);
        assert_eq!(shard2.id, 1);
        assert_eq!(shard3.id, 2);
        assert_eq!(shard4.id, 3);

        // Test address modulo strategy
        let config = ShardConfig {
            shard_count: 4,
            strategy: ShardingStrategy::AddressModulo,
            ..Default::default()
        };

        let manager = StateShardingManager::new(&kv_store, &state_store, &block_store, config);
        manager.init().unwrap();

        let shard1 = manager.get_shard_for_address(&address1).unwrap();
        assert!(shard1.id < 4);
    }

    #[test]
    fn test_account_operations() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        let config = ShardConfig {
            shard_count: 4,
            ..Default::default()
        };

        let manager = StateShardingManager::new(&kv_store, &state_store, &block_store, config);
        manager.init().unwrap();

        // Create test accounts
        let address1 = create_test_address(0);
        let account1 = create_test_account(1000, 100);

        let address2 = create_test_address(1);
        let account2 = create_test_account(2000, 100);

        // Put accounts
        manager.put_account(&address1, &account1).unwrap();
        manager.put_account(&address2, &account2).unwrap();

        // Get accounts
        let retrieved1 = manager.get_account(&address1).unwrap().unwrap();
        let retrieved2 = manager.get_account(&address2).unwrap().unwrap();

        assert_eq!(retrieved1.balance, 1000);
        assert_eq!(retrieved2.balance, 2000);

        // Delete account
        manager.delete_account(&address1).unwrap();

        // Verify account was deleted
        assert!(manager.get_account(&address1).unwrap().is_none());
        assert!(manager.get_account(&address2).unwrap().is_some());
    }

    #[test]
    fn test_shard_info() {
        let (_temp_dir, kv_store, block_store, state_store) = setup_test_env();

        let config = ShardConfig {
            shard_count: 2,
            strategy: ShardingStrategy::AddressPrefix,
            ..Default::default()
        };

        let manager = StateShardingManager::new(&kv_store, &state_store, &block_store, config);
        manager.init().unwrap();

        // Create test accounts
        let address1 = create_test_address(0);
        let account1 = create_test_account(1000, 100);

        let address2 = create_test_address(0);
        address2[1] = 1;
        let account2 = create_test_account(2000, 100);

        let address3 = create_test_address(1);
        let account3 = create_test_account(3000, 100);

        // Put accounts
        manager.put_account(&address1, &account1).unwrap();
        manager.put_account(&address2, &account2).unwrap();
        manager.put_account(&address3, &account3).unwrap();

        // Update shard info
        manager.update_all_shard_info().unwrap();

        // Get shard info
        let info = manager.get_all_shard_info().unwrap();
        assert_eq!(info.len(), 2);

        // Shard 0 should have 2 accounts with total balance 3000
        let shard0 = info.iter().find(|i| i.id == 0).unwrap();
        assert_eq!(shard0.account_count, 2);
        assert_eq!(shard0.total_balance, 3000);

        // Shard 1 should have 1 account with total balance 3000
        let shard1 = info.iter().find(|i| i.id == 1).unwrap();
        assert_eq!(shard1.account_count, 1);
        assert_eq!(shard1.total_balance, 3000);
    }
}
