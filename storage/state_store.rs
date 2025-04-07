use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::collections::HashMap;
use log::{debug, error, info, warn};
use hex;

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::block_store::{Hash, Block};
use crate::storage::tx_store::{TxStore, TransactionStatus, TransactionError};

/// Account state structure
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AccountState {
    /// Account balance
    pub balance: u64,

    /// Account nonce (for transaction ordering)
    pub nonce: u64,

    /// Smart contract code
    pub code: Option<Vec<u8>>,

    /// Smart contract storage (key-value pairs)
    pub storage: HashMap<Vec<u8>, Vec<u8>>,

    /// Last updated block height
    pub last_updated: u64,

    /// Account type
    pub account_type: AccountType,
}

/// Account type
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum AccountType {
    /// Regular user account
    User,

    /// Smart contract account
    Contract,

    /// System account
    System,
}

/// Error type for StateStore operations
#[derive(Debug, thiserror::Error)]
pub enum StateStoreError {
    /// KVStore error
    #[error("KVStore error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Account not found
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    /// Insufficient balance
    #[error("Insufficient balance: required {0}, available {1}")]
    InsufficientBalance(u64, u64),

    /// Invalid nonce
    #[error("Invalid nonce: expected {0}, got {1}")]
    InvalidNonce(u64, u64),

    /// Storage key not found
    #[error("Storage key not found: {0}")]
    StorageKeyNotFound(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// State root hash
#[derive(Debug, Clone, PartialEq)]
pub struct StateRoot {
    /// Root hash of the state trie
    pub root_hash: Hash,

    /// Block height at which this state root was calculated
    pub block_height: u64,

    /// Timestamp when this state root was calculated
    pub timestamp: u64,
}

/// Store for account states
pub struct StateStore<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// Current state root
    state_root: std::sync::RwLock<Option<StateRoot>>,

    /// Cache of recently accessed accounts
    account_cache: dashmap::DashMap<String, AccountState>,

    /// Maximum number of accounts to cache
    max_cache_size: usize,
}

impl<'a> StateStore<'a> {
    /// Create a new StateStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self {
            store,
            state_root: std::sync::RwLock::new(None),
            account_cache: dashmap::DashMap::new(),
            max_cache_size: 10000, // Default cache size
        }
    }

    /// Create a new StateStore with custom cache size
    pub fn with_cache_size(store: &'a dyn KVStore, max_cache_size: usize) -> Self {
        Self {
            store,
            state_root: std::sync::RwLock::new(None),
            account_cache: dashmap::DashMap::new(),
            max_cache_size,
        }
    }

    /// Get the state of an account
    pub fn get_account_state(&self, address: &Hash) -> Option<AccountState> {
        let addr_str = hex::encode(address);

        // Check the cache first
        if let Some(cached) = self.account_cache.get(&addr_str) {
            return Some(cached.clone());
        }

        // If not in cache, get from store
        let key = format!("state:account:{}", addr_str);
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                match bincode::deserialize(&bytes) {
                    Ok(state) => {
                        // Add to cache
                        self.add_to_cache(addr_str, state.clone());
                        Some(state)
                    },
                    Err(e) => {
                        error!("Failed to deserialize account state for {}: {}", addr_str, e);
                        None
                    }
                }
            },
            Ok(None) => None,
            Err(e) => {
                error!("Failed to get account state for {}: {}", addr_str, e);
                None
            }
        }
    }

    /// Set the state of an account
    pub fn set_account_state(&self, address: &Hash, state: &AccountState) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);
        let key = format!("state:account:{}", addr_str);

        let value = bincode::serialize(state)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

        // Update the store
        self.store.put(key.as_bytes(), &value)?;

        // Update the cache
        self.add_to_cache(addr_str, state.clone());

        // Invalidate the state root
        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        Ok(())
    }

    /// Add an account state to the cache
    fn add_to_cache(&self, address: String, state: AccountState) {
        // If cache is full, remove a random entry
        if self.account_cache.len() >= self.max_cache_size {
            if let Some(entry) = self.account_cache.iter().next() {
                self.account_cache.remove(entry.key());
            }
        }

        // Add to cache
        self.account_cache.insert(address, state);
    }

    /// Update account balance
    pub fn update_balance(&self, address: &Hash, new_balance: u64) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        if let Some(mut state) = self.get_account_state(address) {
            state.balance = new_balance;
            state.last_updated = self.get_current_block_height().unwrap_or(0);
            self.set_account_state(address, &state)?;
            debug!("Updated balance for account {}: {}", addr_str, new_balance);
            Ok(())
        } else {
            Err(StateStoreError::AccountNotFound(addr_str))
        }
    }

    /// Transfer balance between accounts
    pub fn transfer_balance(
        &self,
        from: &Hash,
        to: &Hash,
        amount: u64,
        block_height: u64,
    ) -> Result<(), StateStoreError> {
        let from_str = hex::encode(from);
        let to_str = hex::encode(to);

        // Get sender account
        let mut sender = self.get_account_state(from)
            .ok_or_else(|| StateStoreError::AccountNotFound(from_str.clone()))?;

        // Check balance
        if sender.balance < amount {
            return Err(StateStoreError::InsufficientBalance(amount, sender.balance));
        }

        // Get recipient account
        let mut recipient = self.get_account_state(to)
            .unwrap_or_else(|| {
                // Create new account if it doesn't exist
                AccountState {
                    balance: 0,
                    nonce: 0,
                    code: None,
                    storage: HashMap::new(),
                    last_updated: block_height,
                    account_type: AccountType::User,
                }
            });

        // Update balances
        sender.balance -= amount;
        recipient.balance += amount;
        sender.last_updated = block_height;
        recipient.last_updated = block_height;

        // Create a batch operation
        let mut batch = WriteBatchOperation::new();

        // Serialize accounts
        let sender_key = format!("state:account:{}", from_str);
        let sender_value = bincode::serialize(&sender)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

        let recipient_key = format!("state:account:{}", to_str);
        let recipient_value = bincode::serialize(&recipient)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

        // Add to batch
        batch.put(sender_key.as_bytes().to_vec(), sender_value);
        batch.put(recipient_key.as_bytes().to_vec(), recipient_value);

        // Execute the batch
        self.store.write_batch(batch)?;

        // Update the cache
        self.add_to_cache(from_str, sender);
        self.add_to_cache(to_str, recipient);

        // Invalidate the state root
        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        info!("Transferred {} from {} to {}", amount, from_str, to_str);
        Ok(())
    }

    /// Increment account nonce
    pub fn increment_nonce(&self, address: &Hash) -> Result<u64, StateStoreError> {
        let addr_str = hex::encode(address);

        if let Some(mut state) = self.get_account_state(address) {
            state.nonce += 1;
            state.last_updated = self.get_current_block_height().unwrap_or(0);
            self.set_account_state(address, &state)?;
            debug!("Incremented nonce for account {}: {}", addr_str, state.nonce);
            Ok(state.nonce)
        } else {
            Err(StateStoreError::AccountNotFound(addr_str))
        }
    }

    /// Create a new account with default values
    pub fn create_account(
        &self,
        address: &Hash,
        initial_balance: u64,
        account_type: AccountType,
    ) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        // Check if account already exists
        if self.get_account_state(address).is_some() {
            debug!("Account {} already exists", addr_str);
            return Ok(());
        }

        let state = AccountState {
            balance: initial_balance,
            nonce: 0,
            code: None,
            storage: HashMap::new(),
            last_updated: self.get_current_block_height().unwrap_or(0),
            account_type,
        };

        self.set_account_state(address, &state)?;
        info!("Created new account {} with balance {}", addr_str, initial_balance);
        Ok(())
    }

    /// Set contract code for an account
    pub fn set_contract_code(&self, address: &Hash, code: Vec<u8>) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        if let Some(mut state) = self.get_account_state(address) {
            state.code = Some(code);
            state.account_type = AccountType::Contract;
            state.last_updated = self.get_current_block_height().unwrap_or(0);
            self.set_account_state(address, &state)?;
            info!("Set contract code for account {}", addr_str);
            Ok(())
        } else {
            Err(StateStoreError::AccountNotFound(addr_str))
        }
    }

    /// Get contract code for an account
    pub fn get_contract_code(&self, address: &Hash) -> Option<Vec<u8>> {
        self.get_account_state(address).and_then(|state| state.code)
    }

    /// Set storage value for a contract
    pub fn set_storage_value(
        &self,
        address: &Hash,
        key: &[u8],
        value: Vec<u8>,
    ) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        if let Some(mut state) = self.get_account_state(address) {
            state.storage.insert(key.to_vec(), value);
            state.last_updated = self.get_current_block_height().unwrap_or(0);
            self.set_account_state(address, &state)?;
            debug!("Set storage value for account {} key {}", addr_str, hex::encode(key));
            Ok(())
        } else {
            Err(StateStoreError::AccountNotFound(addr_str))
        }
    }

    /// Get storage value for a contract
    pub fn get_storage_value(&self, address: &Hash, key: &[u8]) -> Option<Vec<u8>> {
        self.get_account_state(address)
            .and_then(|state| state.storage.get(key).cloned())
    }

    /// Delete storage value for a contract
    pub fn delete_storage_value(&self, address: &Hash, key: &[u8]) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        if let Some(mut state) = self.get_account_state(address) {
            if state.storage.remove(key).is_none() {
                return Err(StateStoreError::StorageKeyNotFound(hex::encode(key)));
            }
            state.last_updated = self.get_current_block_height().unwrap_or(0);
            self.set_account_state(address, &state)?;
            debug!("Deleted storage value for account {} key {}", addr_str, hex::encode(key));
            Ok(())
        } else {
            Err(StateStoreError::AccountNotFound(addr_str))
        }
    }

    /// Get all accounts
    pub fn get_all_accounts(&self) -> Vec<(Hash, AccountState)> {
        let prefix = b"state:account:";
        match self.store.scan_prefix(prefix) {
            Ok(entries) => {
                entries.iter()
                    .filter_map(|(key, value)| {
                        // Extract address from key
                        let key_str = std::str::from_utf8(key).ok()?;
                        let addr_hex = key_str.strip_prefix("state:account:")?;
                        let addr_bytes = hex::decode(addr_hex).ok()?;

                        // Convert to Hash
                        let mut addr = [0u8; 32];
                        if addr_bytes.len() == 32 {
                            addr.copy_from_slice(&addr_bytes);
                        } else {
                            return None;
                        }

                        // Deserialize account state
                        match bincode::deserialize(value) {
                            Ok(state) => Some((addr, state)),
                            Err(e) => {
                                error!("Failed to deserialize account state: {}", e);
                                None
                            }
                        }
                    })
                    .collect()
            },
            Err(e) => {
                error!("Failed to scan accounts: {}", e);
                Vec::new()
            }
        }
    }

    /// Get accounts updated since a specific block height
    pub fn get_accounts_updated_since(&self, block_height: u64) -> Vec<(Hash, AccountState)> {
        self.get_all_accounts()
            .into_iter()
            .filter(|(_, state)| state.last_updated > block_height)
            .collect()
    }

    /// Get the current block height
    fn get_current_block_height(&self) -> Option<u64> {
        // This would normally come from the blockchain
        // For now, we'll just return None
        None
    }

    /// Calculate the state root hash
    pub fn calculate_state_root(&self, block_height: u64, timestamp: u64) -> Result<StateRoot, StateStoreError> {
        // Check if we already have a cached state root
        {
            let state_root = self.state_root.read().unwrap();
            if let Some(root) = &*state_root {
                return Ok(root.clone());
            }
        }

        // Get all accounts
        let accounts = self.get_all_accounts();

        // In a real implementation, we would build a Merkle Patricia Trie
        // and calculate the root hash. For now, we'll just create a simple hash.
        let mut hasher = sha2::Sha256::new();

        // Sort accounts by address for deterministic ordering
        let mut sorted_accounts = accounts;
        sorted_accounts.sort_by(|(a, _), (b, _)| a.cmp(b));

        // Hash each account
        for (addr, state) in sorted_accounts {
            use sha2::Digest;

            // Hash address
            hasher.update(addr);

            // Hash account state
            let state_bytes = bincode::serialize(&state)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;
            hasher.update(state_bytes);
        }

        // Finalize hash
        let root_hash: [u8; 32] = hasher.finalize().into();

        // Create state root
        let state_root = StateRoot {
            root_hash,
            block_height,
            timestamp,
        };

        // Cache the state root
        let mut root = self.state_root.write().unwrap();
        *root = Some(state_root.clone());

        Ok(state_root)
    }

    /// Get the current state root
    pub fn get_state_root(&self) -> Option<StateRoot> {
        let state_root = self.state_root.read().unwrap();
        state_root.clone()
    }

    /// Set the state root
    pub fn set_state_root(&self, root: StateRoot) {
        let mut state_root = self.state_root.write().unwrap();
        *state_root = Some(root);
    }

    /// Flush all pending writes to disk
    pub fn flush(&self) -> Result<(), StateStoreError> {
        self.store.flush().map_err(|e| e.into())
    }

    /// Clear the account cache
    pub fn clear_cache(&self) {
        self.account_cache.clear();
    }

    /// Apply a block's transactions to the state store
    ///
    /// This method processes all transactions in a block and updates the account states accordingly.
    /// It ensures that transactions are applied in the correct order and validates each transaction
    /// before applying it.
    ///
    /// # Arguments
    ///
    /// * `block` - The block containing transactions to apply
    /// * `tx_store` - The transaction store to retrieve transaction details
    ///
    /// # Returns
    ///
    /// * `Result<(), StateStoreError>` - Success or error
    pub fn apply_block(&self, block: &Block, tx_store: &TxStore) -> Result<(), StateStoreError> {
        info!("Applying block {} with {} transactions", block.height, block.transactions.len());

        // Create a batch operation for all state changes
        let mut batch = WriteBatchOperation::new();

        // Track modified accounts to update them in one batch
        let mut modified_accounts: HashMap<Hash, AccountState> = HashMap::new();

        // Process each transaction in the block
        for tx_hash in &block.transactions {
            // Get the transaction details
            let tx = match tx_store.get_transaction(tx_hash) {
                Some(tx) => tx,
                None => {
                    error!("Transaction {} not found in tx_store", hex::encode(tx_hash));
                    return Err(StateStoreError::Other(format!("Transaction {} not found", hex::encode(tx_hash))));
                }
            };

            // Verify transaction block height matches current block
            if tx.block_height != block.height {
                warn!("Transaction {} has mismatched block height: {} vs {}",
                      hex::encode(&tx.tx_id), tx.block_height, block.height);
            }

            // Get or create sender account state
            let sender_addr_str = hex::encode(&tx.sender);
            let mut sender = match modified_accounts.get(&tx.sender) {
                Some(state) => state.clone(),
                None => self.get_account_state(&tx.sender)
                    .ok_or_else(|| StateStoreError::AccountNotFound(sender_addr_str.clone()))?,
            };

            // Verify sender has sufficient balance
            let total_cost = tx.value + (tx.gas_used * tx.gas_price);
            if sender.balance < total_cost {
                error!("Insufficient balance for tx {}: required {}, available {}",
                       hex::encode(&tx.tx_id), total_cost, sender.balance);

                // Update transaction status to failed
                tx_store.update_transaction_status(
                    &tx.tx_id,
                    TransactionStatus::Failed(TransactionError::InsufficientBalance)
                )?;

                continue; // Skip this transaction and move to the next
            }

            // Verify nonce
            if tx.nonce != sender.nonce {
                error!("Invalid nonce for tx {}: expected {}, got {}",
                       hex::encode(&tx.tx_id), sender.nonce, tx.nonce);

                // Update transaction status to failed
                tx_store.update_transaction_status(
                    &tx.tx_id,
                    TransactionStatus::Failed(TransactionError::InvalidNonce)
                )?;

                continue; // Skip this transaction and move to the next
            }

            // Get or create recipient account state
            let recipient_addr_str = hex::encode(&tx.recipient);
            let mut recipient = match modified_accounts.get(&tx.recipient) {
                Some(state) => state.clone(),
                None => self.get_account_state(&tx.recipient).unwrap_or_else(|| {
                    // Create new account if it doesn't exist
                    AccountState {
                        balance: 0,
                        nonce: 0,
                        code: None,
                        storage: HashMap::new(),
                        last_updated: block.height,
                        account_type: AccountType::User,
                    }
                }),
            };

            // Update account states
            sender.balance -= total_cost;
            sender.nonce += 1;
            sender.last_updated = block.height;

            recipient.balance += tx.value;
            recipient.last_updated = block.height;

            // Handle contract execution if this is a contract call
            if let Some(data) = &tx.data {
                if recipient.account_type == AccountType::Contract && recipient.code.is_some() {
                    // In a real implementation, we would execute the contract code here
                    // For now, we'll just log that a contract call was made
                    debug!("Contract call in tx {}: {} -> {}",
                           hex::encode(&tx.tx_id), sender_addr_str, recipient_addr_str);
                }
            }

            // Store updated account states in our tracking map
            modified_accounts.insert(tx.sender, sender);
            modified_accounts.insert(tx.recipient, recipient);

            // Update transaction status to confirmed
            tx_store.update_transaction_status(&tx.tx_id, TransactionStatus::Confirmed)?;
        }

        // Prepare batch operations for all modified accounts
        for (address, state) in modified_accounts {
            let addr_str = hex::encode(&address);
            let key = format!("state:account:{}", addr_str);

            let value = bincode::serialize(&state)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

            batch.put(key.as_bytes().to_vec(), value);

            // Update the cache
            self.add_to_cache(addr_str, state);
        }

        // Execute the batch
        self.store.write_batch(batch)?;

        // Invalidate the state root since we've modified the state
        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        // Calculate the new state root
        drop(state_root); // Release the write lock before calculating
        let new_root = self.calculate_state_root(block.height, block.timestamp)?;

        // Verify that the calculated state root matches the block's state root
        if new_root.root_hash != block.state_root {
            warn!("Calculated state root {} does not match block's state root {}",
                  hex::encode(&new_root.root_hash), hex::encode(&block.state_root));
        }

        info!("Successfully applied block {} with {} transactions", block.height, block.transactions.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    use sha2::{Sha256, Digest};

    #[test]
    fn test_state_store() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::new(&kv_store);

        let address = [1; 32];

        // Create a new account
        state_store.create_account(&address, 1000, AccountType::User).unwrap();

        // Get the account state
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.balance, 1000);
        assert_eq!(state.nonce, 0);
        assert_eq!(state.code, None);
        assert_eq!(state.account_type, AccountType::User);

        // Update balance
        state_store.update_balance(&address, 2000).unwrap();
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.balance, 2000);

        // Increment nonce
        let new_nonce = state_store.increment_nonce(&address).unwrap();
        assert_eq!(new_nonce, 1);
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.nonce, 1);

        // Set contract code
        let contract_code = vec![1, 2, 3, 4];
        state_store.set_contract_code(&address, contract_code.clone()).unwrap();
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.code, Some(contract_code));
        assert_eq!(state.account_type, AccountType::Contract);

        // Test contract storage
        let storage_key = b"test_key";
        let storage_value = b"test_value".to_vec();
        state_store.set_storage_value(&address, storage_key, storage_value.clone()).unwrap();

        let retrieved_value = state_store.get_storage_value(&address, storage_key).unwrap();
        assert_eq!(retrieved_value, storage_value);

        // Test delete storage value
        state_store.delete_storage_value(&address, storage_key).unwrap();
        assert!(state_store.get_storage_value(&address, storage_key).is_none());

        // Test flush
        state_store.flush().unwrap();
    }

    #[test]
    fn test_transfer_balance() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::new(&kv_store);

        let sender = [1; 32];
        let recipient = [2; 32];

        // Create accounts
        state_store.create_account(&sender, 1000, AccountType::User).unwrap();
        state_store.create_account(&recipient, 500, AccountType::User).unwrap();

        // Transfer balance
        state_store.transfer_balance(&sender, &recipient, 300, 1).unwrap();

        // Check balances
        let sender_state = state_store.get_account_state(&sender).unwrap();
        let recipient_state = state_store.get_account_state(&recipient).unwrap();

        assert_eq!(sender_state.balance, 700);
        assert_eq!(recipient_state.balance, 800);

        // Test insufficient balance
        let result = state_store.transfer_balance(&sender, &recipient, 1000, 2);
        assert!(matches!(result, Err(StateStoreError::InsufficientBalance(1000, 700))));
    }

    #[test]
    fn test_state_root() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::new(&kv_store);

        // Create some accounts
        let addr1 = [1; 32];
        let addr2 = [2; 32];

        state_store.create_account(&addr1, 1000, AccountType::User).unwrap();
        state_store.create_account(&addr2, 2000, AccountType::User).unwrap();

        // Calculate state root
        let state_root = state_store.calculate_state_root(1, 12345).unwrap();

        // Verify state root is not zero
        assert_ne!(state_root.root_hash, [0; 32]);
        assert_eq!(state_root.block_height, 1);
        assert_eq!(state_root.timestamp, 12345);

        // Get state root
        let retrieved_root = state_store.get_state_root().unwrap();
        assert_eq!(retrieved_root, state_root);

        // Modify state and check that root is invalidated
        state_store.update_balance(&addr1, 1500).unwrap();

        // Calculate new root
        let new_root = state_store.calculate_state_root(2, 12346).unwrap();
        assert_ne!(new_root.root_hash, state_root.root_hash);
    }

    #[test]
    fn test_get_all_accounts() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::new(&kv_store);

        // Create some accounts
        let addr1 = [1; 32];
        let addr2 = [2; 32];
        let addr3 = [3; 32];

        state_store.create_account(&addr1, 1000, AccountType::User).unwrap();
        state_store.create_account(&addr2, 2000, AccountType::User).unwrap();
        state_store.create_account(&addr3, 3000, AccountType::Contract).unwrap();

        // Get all accounts
        let accounts = state_store.get_all_accounts();
        assert_eq!(accounts.len(), 3);

        // Verify account data
        let mut found_addr1 = false;
        let mut found_addr2 = false;
        let mut found_addr3 = false;

        for (addr, state) in accounts {
            if addr == addr1 {
                assert_eq!(state.balance, 1000);
                found_addr1 = true;
            } else if addr == addr2 {
                assert_eq!(state.balance, 2000);
                found_addr2 = true;
            } else if addr == addr3 {
                assert_eq!(state.balance, 3000);
                assert_eq!(state.account_type, AccountType::Contract);
                found_addr3 = true;
            }
        }

        assert!(found_addr1 && found_addr2 && found_addr3);
    }

    #[test]
    fn test_account_cache() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::with_cache_size(&kv_store, 2); // Small cache for testing

        // Create some accounts
        let addr1 = [1; 32];
        let addr2 = [2; 32];
        let addr3 = [3; 32];

        state_store.create_account(&addr1, 1000, AccountType::User).unwrap();
        state_store.create_account(&addr2, 2000, AccountType::User).unwrap();

        // Access accounts to populate cache
        state_store.get_account_state(&addr1);
        state_store.get_account_state(&addr2);

        // Add a third account, which should evict one from the cache
        state_store.create_account(&addr3, 3000, AccountType::User).unwrap();
        state_store.get_account_state(&addr3);

        // Clear cache
        state_store.clear_cache();

        // Cache should be empty now
        assert_eq!(state_store.account_cache.len(), 0);
    }

    #[test]
    fn test_apply_block() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::new(&kv_store);
        let tx_store = TxStore::new(&kv_store);

        // Create sender and recipient accounts
        let sender = [1; 32];
        let recipient = [2; 32];

        state_store.create_account(&sender, 1000, AccountType::User).unwrap();
        state_store.create_account(&recipient, 500, AccountType::User).unwrap();

        // Create a transaction
        let tx = TransactionRecord {
            tx_id: [10; 32],
            sender,
            recipient,
            value: 300,
            gas_price: 5,
            gas_limit: 21000,
            gas_used: 10,
            nonce: 0, // Matches the sender's initial nonce
            timestamp: 12345,
            block_height: 1, // Will be included in block 1
            data: None,
            status: TransactionStatus::Included,
        };

        // Store the transaction
        tx_store.put_transaction(&tx).unwrap();

        // Create a block with this transaction
        let block = Block {
            height: 1,
            hash: [20; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![tx.tx_id],
            state_root: [0; 32], // This would normally be calculated
            nonce: 42,
            poh_seq: 100,
            poh_hash: [30; 32],
            difficulty: 1000,
            total_difficulty: 1000,
        };

        // Apply the block
        state_store.apply_block(&block, &tx_store).unwrap();

        // Verify account states
        let sender_state = state_store.get_account_state(&sender).unwrap();
        let recipient_state = state_store.get_account_state(&recipient).unwrap();

        // Sender should have 1000 - 300 - (10 * 5) = 650 balance and nonce incremented
        assert_eq!(sender_state.balance, 650);
        assert_eq!(sender_state.nonce, 1);

        // Recipient should have 500 + 300 = 800 balance
        assert_eq!(recipient_state.balance, 800);

        // Transaction status should be updated to Confirmed
        let updated_tx = tx_store.get_transaction(&tx.tx_id).unwrap();
        assert_eq!(updated_tx.status, TransactionStatus::Confirmed);
    }

    #[test]
    fn test_apply_block_with_invalid_transactions() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::new(&kv_store);
        let tx_store = TxStore::new(&kv_store);

        // Create sender and recipient accounts
        let sender = [1; 32];
        let recipient = [2; 32];

        state_store.create_account(&sender, 100, AccountType::User).unwrap(); // Only 100 balance
        state_store.create_account(&recipient, 500, AccountType::User).unwrap();

        // Create a transaction with insufficient balance
        let tx1 = TransactionRecord {
            tx_id: [10; 32],
            sender,
            recipient,
            value: 300, // More than sender's balance
            gas_price: 5,
            gas_limit: 21000,
            gas_used: 10,
            nonce: 0,
            timestamp: 12345,
            block_height: 1,
            data: None,
            status: TransactionStatus::Included,
        };

        // Create a transaction with invalid nonce
        let tx2 = TransactionRecord {
            tx_id: [11; 32],
            sender,
            recipient,
            value: 50, // Valid amount
            gas_price: 5,
            gas_limit: 21000,
            gas_used: 10,
            nonce: 5, // Invalid nonce (should be 0)
            timestamp: 12345,
            block_height: 1,
            data: None,
            status: TransactionStatus::Included,
        };

        // Store the transactions
        tx_store.put_transaction(&tx1).unwrap();
        tx_store.put_transaction(&tx2).unwrap();

        // Create a block with these transactions
        let block = Block {
            height: 1,
            hash: [20; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![tx1.tx_id, tx2.tx_id],
            state_root: [0; 32],
            nonce: 42,
            poh_seq: 100,
            poh_hash: [30; 32],
            difficulty: 1000,
            total_difficulty: 1000,
        };

        // Apply the block
        state_store.apply_block(&block, &tx_store).unwrap();

        // Verify account states - should be unchanged since both transactions failed
        let sender_state = state_store.get_account_state(&sender).unwrap();
        let recipient_state = state_store.get_account_state(&recipient).unwrap();

        assert_eq!(sender_state.balance, 100); // Unchanged
        assert_eq!(sender_state.nonce, 0); // Unchanged
        assert_eq!(recipient_state.balance, 500); // Unchanged

        // Transaction statuses should be updated to Failed
        let updated_tx1 = tx_store.get_transaction(&tx1.tx_id).unwrap();
        match updated_tx1.status {
            TransactionStatus::Failed(TransactionError::InsufficientBalance) => {}, // Expected
            _ => panic!("Expected InsufficientBalance error"),
        }

        let updated_tx2 = tx_store.get_transaction(&tx2.tx_id).unwrap();
        match updated_tx2.status {
            TransactionStatus::Failed(TransactionError::InvalidNonce) => {}, // Expected
            _ => panic!("Expected InvalidNonce error"),
        }
    }
}
