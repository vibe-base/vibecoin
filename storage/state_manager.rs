use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use log::{debug, error, info, warn};
use dashmap::DashMap;

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::state::{AccountState, AccountType, StateRoot};
use crate::storage::state_store::StateStoreError;
use crate::storage::block_store::{Block, Hash};
use crate::storage::rocksdb_schema::Schema;
use crate::storage::trie::mpt::MerklePatriciaTrie;

/// State manager for efficient state management
pub struct StateManager<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,

    /// Current state root
    state_root: RwLock<Option<StateRoot>>,

    /// Account cache
    account_cache: DashMap<String, AccountState>,

    /// Storage cache
    storage_cache: DashMap<String, DashMap<String, Vec<u8>>>,

    /// Maximum number of accounts to cache
    max_account_cache_size: usize,

    /// Maximum number of storage entries to cache per account
    max_storage_cache_size: usize,

    /// Merkle Patricia Trie for state verification
    trie: RwLock<Option<MerklePatriciaTrie>>,
}

impl<'a> StateManager<'a> {
    /// Create a new StateManager
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self {
            store,
            state_root: RwLock::new(None),
            account_cache: DashMap::new(),
            storage_cache: DashMap::new(),
            max_account_cache_size: 10000,
            max_storage_cache_size: 1000,
            trie: RwLock::new(None),
        }
    }

    /// Create a new StateManager with custom cache sizes
    pub fn with_cache_sizes(
        store: &'a dyn KVStore,
        max_account_cache_size: usize,
        max_storage_cache_size: usize,
    ) -> Self {
        Self {
            store,
            state_root: RwLock::new(None),
            account_cache: DashMap::new(),
            storage_cache: DashMap::new(),
            max_account_cache_size,
            max_storage_cache_size,
            trie: RwLock::new(None),
        }
    }

    /// Get the state of an account
    pub fn get_account_state(&self, address: &Hash) -> Result<Option<AccountState>, StateStoreError> {
        let addr_str = hex::encode(address);

        // Check the cache first
        if let Some(cached) = self.account_cache.get(&addr_str) {
            return Ok(Some(cached.clone()));
        }

        // If not in cache, get from store
        let key = Schema::account_state_key(address);
        match self.store.get(key.as_bytes()) {
            Ok(Some(bytes)) => {
                match bincode::deserialize::<AccountState>(&bytes) {
                    Ok(state) => {
                        // Add to cache
                        self.add_to_account_cache(addr_str, state.clone());
                        Ok(Some(state))
                    },
                    Err(e) => {
                        error!("Failed to deserialize account state for {}: {}", addr_str, e);
                        Err(StateStoreError::SerializationError(e.to_string()))
                    }
                }
            },
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Failed to get account state for {}: {}", addr_str, e);
                Err(StateStoreError::KVStoreError(e))
            }
        }
    }

    /// Set the state of an account
    pub fn set_account_state(&self, address: &Hash, state: &AccountState) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);
        let key = Schema::account_state_key(address);

        let value = bincode::serialize(state)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

        // Update the store
        self.store.put(key.as_bytes(), &value)
            .map_err(|e| StateStoreError::KVStoreError(e))?;

        // Update the cache
        self.add_to_account_cache(addr_str, state.clone());

        // Invalidate the state root and trie
        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        let mut trie = self.trie.write().unwrap();
        *trie = None;

        Ok(())
    }

    /// Create a new account
    pub fn create_account(
        &self,
        address: &Hash,
        initial_balance: u64,
        account_type: AccountType,
    ) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        // Check if the account already exists
        if let Some(_) = self.get_account_state(address)? {
            return Err(StateStoreError::AccountAlreadyExists(addr_str));
        }

        // Get the current block height
        let current_height = 0; // TODO: Get actual block height

        // Create a new account state based on account type
        let state = match account_type {
            AccountType::User => AccountState::new_user(initial_balance, current_height),
            AccountType::Contract => AccountState::new_contract(initial_balance, Vec::new(), current_height),
            AccountType::System => AccountState::new_system(initial_balance, current_height),
            AccountType::Validator => AccountState::new_validator(initial_balance, 0, current_height),
        };

        // Set the account state
        self.set_account_state(address, &state)
    }

    /// Update an account's balance
    pub fn update_balance(&self, address: &Hash, new_balance: u64) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        // Get the current account state
        let mut state = match self.get_account_state(address)? {
            Some(state) => state,
            None => return Err(StateStoreError::AccountNotFound(addr_str)),
        };

        // Update the balance
        state.balance = new_balance;

        // Set the updated account state
        self.set_account_state(address, &state)
    }

    /// Update an account's nonce
    pub fn update_nonce(&self, address: &Hash, new_nonce: u64) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);

        // Get the current account state
        let mut state = match self.get_account_state(address)? {
            Some(state) => state,
            None => return Err(StateStoreError::AccountNotFound(addr_str)),
        };

        // Update the nonce
        state.nonce = new_nonce;

        // Set the updated account state
        self.set_account_state(address, &state)
    }

    /// Get a storage value for a contract
    pub fn get_storage_value(
        &self,
        address: &Hash,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, StateStoreError> {
        let addr_str = hex::encode(address);
        let key_str = hex::encode(key);

        // Check the cache first
        if let Some(account_storage) = self.storage_cache.get(&addr_str) {
            if let Some(value) = account_storage.get(&key_str) {
                return Ok(Some(value.clone()));
            }
        }

        // If not in cache, get from store
        let storage_key = Schema::contract_storage_key(address, key);
        match self.store.get(storage_key.as_bytes()) {
            Ok(Some(bytes)) => {
                // Add to cache
                self.add_to_storage_cache(addr_str, key_str, bytes.clone());
                Ok(Some(bytes))
            },
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Failed to get storage value for {}:{}: {}", addr_str, key_str, e);
                Err(StateStoreError::KVStoreError(e))
            }
        }
    }

    /// Set a storage value for a contract
    pub fn set_storage_value(
        &self,
        address: &Hash,
        key: &[u8],
        value: Vec<u8>,
    ) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);
        let key_str = hex::encode(key);

        // Check if the account exists
        if let None = self.get_account_state(address)? {
            return Err(StateStoreError::AccountNotFound(addr_str));
        }

        // Set the storage value
        let storage_key = Schema::contract_storage_key(address, key);
        self.store.put(storage_key.as_bytes(), &value)
            .map_err(|e| StateStoreError::KVStoreError(e))?;

        // Update the cache
        self.add_to_storage_cache(addr_str, key_str, value);

        // Invalidate the state root and trie
        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        let mut trie = self.trie.write().unwrap();
        *trie = None;

        Ok(())
    }

    /// Delete a storage value for a contract
    pub fn delete_storage_value(
        &self,
        address: &Hash,
        key: &[u8],
    ) -> Result<(), StateStoreError> {
        let addr_str = hex::encode(address);
        let key_str = hex::encode(key);

        // Check if the account exists
        if let None = self.get_account_state(address)? {
            return Err(StateStoreError::AccountNotFound(addr_str));
        }

        // Delete the storage value
        let storage_key = Schema::contract_storage_key(address, key);
        self.store.delete(storage_key.as_bytes())
            .map_err(|e| StateStoreError::KVStoreError(e))?;

        // Remove from cache
        if let Some(account_storage) = self.storage_cache.get_mut(&addr_str) {
            account_storage.remove(&key_str);
        }

        // Invalidate the state root and trie
        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        let mut trie = self.trie.write().unwrap();
        *trie = None;

        Ok(())
    }

    /// Get all accounts
    pub fn get_all_accounts(&self) -> Result<Vec<(Hash, AccountState)>, StateStoreError> {
        let mut accounts = Vec::new();

        // Scan all account keys
        let prefix = "state:".as_bytes();
        let entries = self.store.scan_prefix(prefix)
            .map_err(|e| StateStoreError::KVStoreError(e))?;

        for (key, value) in entries {
            // Parse the key to extract the address
            let key_str = String::from_utf8_lossy(&key);
            if let Some(addr) = Schema::extract_account_address(&key_str) {
                // Deserialize the account state
                match bincode::deserialize(&value) {
                    Ok(state) => {
                        accounts.push((addr, state));
                    },
                    Err(e) => {
                        error!("Failed to deserialize account state: {}", e);
                        return Err(StateStoreError::SerializationError(e.to_string()));
                    }
                }
            }
        }

        Ok(accounts)
    }

    /// Calculate the state root
    pub fn calculate_state_root(
        &self,
        block_height: u64,
        timestamp: u64,
    ) -> Result<StateRoot, StateStoreError> {
        // Check if we already have a cached state root
        {
            let state_root = self.state_root.read().unwrap();
            if let Some(root) = &*state_root {
                return Ok(root.clone());
            }
        }

        // Get all accounts
        let accounts = self.get_all_accounts()?;

        // Build a Merkle Patricia Trie with all accounts
        let mut trie = MerklePatriciaTrie::new();

        // Sort accounts by address for deterministic ordering
        let mut sorted_accounts = accounts;
        sorted_accounts.sort_by(|(a, _), (b, _)| a.cmp(b));

        // Insert each account into the trie
        for (addr, state) in sorted_accounts {
            // Serialize the account state
            let state_bytes = bincode::serialize(&state)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

            // Insert into the trie (address -> state)
            trie.insert(&addr, state_bytes);
        }

        // Calculate the root hash
        let root_hash = trie.root_hash();

        // Create state root
        let state_root = StateRoot {
            root_hash,
            block_height,
            timestamp,
        };

        // Cache the state root and trie
        {
            let mut cached_state_root = self.state_root.write().unwrap();
            *cached_state_root = Some(state_root.clone());

            let mut cached_trie = self.trie.write().unwrap();
            *cached_trie = Some(trie);
        }

        Ok(state_root)
    }

    /// Generate a proof for an account
    pub fn generate_account_proof(
        &self,
        address: &Hash,
    ) -> Result<Vec<u8>, StateStoreError> {
        // Ensure we have a trie
        self.ensure_trie()?;

        // Get the trie
        let trie_guard = self.trie.read().unwrap();
        let trie = trie_guard.as_ref().unwrap();

        // Generate the proof
        let proof = trie.generate_proof(address);

        // Serialize the proof
        bincode::serialize(&proof)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))
    }

    /// Verify an account proof
    pub fn verify_account_proof(
        proof_bytes: &[u8],
        root_hash: &Hash,
    ) -> Result<bool, StateStoreError> {
        // Deserialize the proof
        let proof = bincode::deserialize(proof_bytes)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

        // Verify the proof
        Ok(MerklePatriciaTrie::verify_proof(&proof, root_hash))
    }

    /// Apply a block to the state
    pub fn apply_block(&self, block: &Block) -> Result<(), StateStoreError> {
        // Get the transactions for this block
        let tx_prefix = format!("tx_block:{}:", block.height);
        let tx_entries = self.store.scan_prefix(tx_prefix.as_bytes())
            .map_err(|e| StateStoreError::KVStoreError(e))?;

        // Count transactions for logging
        let tx_count = tx_entries.len();

        // Create a batch for all state changes
        let mut batch = Vec::new();

        // Process each transaction
        for (_, tx_hash_bytes) in tx_entries {
            // Get the transaction
            let tx_key = format!("tx:{}", hex::encode(&tx_hash_bytes));
            let tx_bytes = match self.store.get(tx_key.as_bytes()) {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    warn!("Transaction not found: {}", hex::encode(&tx_hash_bytes));
                    continue;
                },
                Err(e) => {
                    error!("Failed to get transaction: {}", e);
                    return Err(StateStoreError::KVStoreError(e));
                }
            };

            // Deserialize the transaction
            let tx: crate::storage::tx_store::TransactionRecord = bincode::deserialize(&tx_bytes)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

            // Get sender account
            let sender_state = match self.get_account_state(&tx.sender)? {
                Some(state) => state,
                None => {
                    error!("Sender account not found: {}", hex::encode(&tx.sender));
                    return Err(StateStoreError::AccountNotFound(hex::encode(&tx.sender)));
                }
            };

            // Get recipient account (or create if it doesn't exist)
            let recipient_state = match self.get_account_state(&tx.recipient)? {
                Some(state) => state,
                None => {
                    // Create a new account for the recipient
                    // Get the current block height
                    let current_height = 0; // TODO: Get actual block height
                    let new_state = AccountState::new_user(0, current_height);

                    // Add to batch
                    let recipient_key = Schema::account_state_key(&tx.recipient);
                    let recipient_value = bincode::serialize(&new_state)
                        .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

                    batch.push(WriteBatchOperation::Put {
                        key: recipient_key.as_bytes().to_vec(),
                        value: recipient_value,
                    });

                    new_state
                }
            };

            // Calculate fees
            let fee = tx.gas_price * tx.gas_used;

            // Update sender balance and nonce
            let new_sender_balance = sender_state.balance.checked_sub(tx.value + fee)
                .ok_or_else(|| StateStoreError::InsufficientBalance(tx.value + fee, sender_state.balance))?;

            // Create a new sender state with updated balance and nonce
            let mut new_sender_state = sender_state.clone();
            new_sender_state.balance = new_sender_balance;
            new_sender_state.nonce += 1;

            // Update recipient balance
            let new_recipient_balance = recipient_state.balance.checked_add(tx.value)
                .ok_or_else(|| StateStoreError::BalanceOverflow(hex::encode(&tx.recipient)))?;

            // Create a new recipient state with updated balance
            let mut new_recipient_state = recipient_state.clone();
            new_recipient_state.balance = new_recipient_balance;

            // Add sender update to batch
            let sender_key = Schema::account_state_key(&tx.sender);
            let sender_value = bincode::serialize(&new_sender_state)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

            batch.push(WriteBatchOperation::Put {
                key: sender_key.as_bytes().to_vec(),
                value: sender_value,
            });

            // Add recipient update to batch
            let recipient_key = Schema::account_state_key(&tx.recipient);
            let recipient_value = bincode::serialize(&new_recipient_state)
                .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

            batch.push(WriteBatchOperation::Put {
                key: recipient_key.as_bytes().to_vec(),
                value: recipient_value,
            });

            // Update caches
            self.add_to_account_cache(hex::encode(&tx.sender), new_sender_state);
            self.add_to_account_cache(hex::encode(&tx.recipient), new_recipient_state);
        }

        // Execute the batch
        self.store.write_batch(batch)
            .map_err(|e| StateStoreError::KVStoreError(e))?;

        // Invalidate the state root and trie
        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        let mut trie = self.trie.write().unwrap();
        *trie = None;

        // Calculate the new state root
        drop(state_root); // Release the write lock before calculating
        drop(trie);
        let new_root = self.calculate_state_root(block.height, block.timestamp)?;

        // Store the state root
        let state_root_key = Schema::state_root_key(block.height);
        let state_root_value = bincode::serialize(&new_root)
            .map_err(|e| StateStoreError::SerializationError(e.to_string()))?;

        self.store.put(state_root_key.as_bytes(), &state_root_value)
            .map_err(|e| StateStoreError::KVStoreError(e))?;

        // Verify that the calculated state root matches the block's state root
        if new_root.root_hash != block.state_root {
            warn!("Calculated state root {} does not match block's state root {}",
                  hex::encode(&new_root.root_hash), hex::encode(&block.state_root));
        }

        info!("Successfully applied block {} with {} transactions",
              block.height, tx_count);

        Ok(())
    }

    /// Clear the caches
    pub fn clear_caches(&self) {
        self.account_cache.clear();
        self.storage_cache.clear();

        let mut state_root = self.state_root.write().unwrap();
        *state_root = None;

        let mut trie = self.trie.write().unwrap();
        *trie = None;
    }

    /// Add an account to the cache
    fn add_to_account_cache(&self, address: String, state: AccountState) {
        // If the cache is full, remove a random entry
        if self.account_cache.len() >= self.max_account_cache_size {
            if let Some(entry) = self.account_cache.iter().next() {
                self.account_cache.remove(entry.key());
            }
        }

        // Add to cache
        self.account_cache.insert(address, state);
    }

    /// Add a storage value to the cache
    fn add_to_storage_cache(&self, address: String, key: String, value: Vec<u8>) {
        // Get or create the account storage cache
        let account_storage = self.storage_cache.entry(address.clone())
            .or_insert_with(|| DashMap::new());

        // If the cache is full, remove a random entry
        if account_storage.len() >= self.max_storage_cache_size {
            if let Some(entry) = account_storage.iter().next() {
                account_storage.remove(entry.key());
            }
        }

        // Add to cache
        account_storage.insert(key, value);
    }

    /// Ensure that the trie is initialized
    fn ensure_trie(&self) -> Result<(), StateStoreError> {
        let trie_guard = self.trie.read().unwrap();

        if trie_guard.is_none() {
            // Release the read lock
            drop(trie_guard);

            // Calculate the state root, which will also initialize the trie
            self.calculate_state_root(0, 0)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::{RocksDBStore, MemoryStore};
    use tempfile::tempdir;

    #[test]
    fn test_account_management() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path()).unwrap();
        let state_manager = StateManager::new(&store);

        // Create an account
        let address = [1u8; 32];
        state_manager.create_account(&address, 1000, AccountType::User).unwrap();

        // Get the account
        let state = state_manager.get_account_state(&address).unwrap().unwrap();
        assert_eq!(state.balance, 1000);
        assert_eq!(state.nonce, 0);
        assert_eq!(state.account_type, AccountType::User);

        // Update the balance
        state_manager.update_balance(&address, 2000).unwrap();
        let state = state_manager.get_account_state(&address).unwrap().unwrap();
        assert_eq!(state.balance, 2000);

        // Update the nonce
        state_manager.update_nonce(&address, 1).unwrap();
        let state = state_manager.get_account_state(&address).unwrap().unwrap();
        assert_eq!(state.nonce, 1);
    }

    #[test]
    fn test_storage_management() {
        let temp_dir = tempdir().unwrap();
        let store = RocksDBStore::new(temp_dir.path()).unwrap();
        let state_manager = StateManager::new(&store);

        // Create an account
        let address = [1u8; 32];
        state_manager.create_account(&address, 1000, AccountType::Contract).unwrap();

        // Set a storage value
        let key = [2u8; 32];
        let value = vec![3u8; 32];
        state_manager.set_storage_value(&address, &key, value.clone()).unwrap();

        // Get the storage value
        let retrieved_value = state_manager.get_storage_value(&address, &key).unwrap().unwrap();
        assert_eq!(retrieved_value, value);

        // Delete the storage value
        state_manager.delete_storage_value(&address, &key).unwrap();
        let retrieved_value = state_manager.get_storage_value(&address, &key).unwrap();
        assert!(retrieved_value.is_none());
    }

    #[test]
    fn test_state_root() {
        let store = MemoryStore::new();
        let state_manager = StateManager::new(&store);

        // Create some accounts
        let addr1 = [1u8; 32];
        let addr2 = [2u8; 32];
        let addr3 = [3u8; 32];

        state_manager.create_account(&addr1, 1000, AccountType::User).unwrap();
        state_manager.create_account(&addr2, 2000, AccountType::User).unwrap();
        state_manager.create_account(&addr3, 3000, AccountType::User).unwrap();

        // Calculate the state root
        let state_root = state_manager.calculate_state_root(1, 12345).unwrap();

        // Generate a proof for addr2
        let proof = state_manager.generate_account_proof(&addr2).unwrap();

        // Verify the proof
        assert!(StateManager::verify_account_proof(&proof, &state_root.root_hash).unwrap());
    }

    #[test]
    fn test_apply_block() {
        let store = MemoryStore::new();
        let state_manager = StateManager::new(&store);

        // Create sender and recipient accounts
        let sender = [1u8; 32];
        let recipient = [2u8; 32];

        state_manager.create_account(&sender, 1000, AccountType::User).unwrap();

        // Create a transaction
        let tx = crate::storage::tx_store::TransactionRecord {
            tx_id: [3u8; 32],
            sender,
            recipient,
            value: 500,
            gas_price: 1,
            gas_limit: 21000,
            gas_used: 21000,
            nonce: 0,
            timestamp: 12345,
            block_height: 1,
            data: None,
            status: crate::storage::tx_store::TransactionStatus::Confirmed,
        };

        // Store the transaction
        let tx_key = Schema::tx_by_hash_key(&tx.tx_id);
        let tx_value = bincode::serialize(&tx).unwrap();
        store.put(tx_key.as_bytes(), &tx_value).unwrap();

        // Create a block index entry
        let tx_block_key = Schema::tx_by_block_key(1, &tx.tx_id);
        store.put(tx_block_key.as_bytes(), &tx.tx_id).unwrap();

        // Create a block
        let block = Block {
            height: 1,
            hash: [4u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 12345,
            transactions: vec![tx.tx_id],
            state_root: [0u8; 32], // Will be calculated
            tx_root: [0u8; 32],
            nonce: 0,
            poh_seq: 0,
            poh_hash: [0u8; 32],
            difficulty: 0,
            total_difficulty: 0,
        };

        // Apply the block
        state_manager.apply_block(&block).unwrap();

        // Check the sender's balance
        let sender_state = state_manager.get_account_state(&sender).unwrap().unwrap();
        assert_eq!(sender_state.balance, 479); // 1000 - 500 - 21
        assert_eq!(sender_state.nonce, 1);

        // Check the recipient's balance
        let recipient_state = state_manager.get_account_state(&recipient).unwrap().unwrap();
        assert_eq!(recipient_state.balance, 500);
        assert_eq!(recipient_state.nonce, 0);
    }
}
