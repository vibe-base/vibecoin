//! Core state structures and abstractions for VibeCoin blockchain
//!
//! This module defines the fundamental state structures used throughout the blockchain,
//! including account states, global state, and state transitions. It provides a clear
//! abstraction layer for representing and manipulating blockchain state.

use std::collections::HashMap;
use std::fmt;
use serde::{Serialize, Deserialize};
use thiserror::Error;

/// Error type for state operations
#[derive(Debug, Error)]
pub enum StateError {
    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// Invalid state transition
    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),

    /// Invalid account
    #[error("Invalid account: {0}")]
    InvalidAccount(String),

    /// Invalid storage key
    #[error("Invalid storage key: {0}")]
    InvalidStorageKey(String),

    /// Invalid storage value
    #[error("Invalid storage value: {0}")]
    InvalidStorageValue(String),

    /// Insufficient balance
    #[error("Insufficient balance: required {0}, available {1}")]
    InsufficientBalance(u64, u64),

    /// Invalid nonce
    #[error("Invalid nonce: expected {0}, got {1}")]
    InvalidNonce(u64, u64),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for state operations
pub type StateResult<T> = Result<T, StateError>;

/// Account type
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountType {
    /// Regular user account
    User,

    /// Smart contract account
    Contract,

    /// System account (for special operations)
    System,

    /// Validator account (for consensus participation)
    Validator,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountType::User => write!(f, "User"),
            AccountType::Contract => write!(f, "Contract"),
            AccountType::System => write!(f, "System"),
            AccountType::Validator => write!(f, "Validator"),
        }
    }
}

/// Account state structure
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AccountState {
    /// Account balance
    pub balance: u64,

    /// Account nonce (for transaction ordering)
    pub nonce: u64,

    /// Smart contract code (if this is a contract account)
    pub code: Option<Vec<u8>>,

    /// Smart contract storage (key-value pairs)
    pub storage: HashMap<Vec<u8>, Vec<u8>>,

    /// Last updated block height
    pub last_updated: u64,

    /// Account type
    pub account_type: AccountType,

    /// Staked amount (for validator accounts)
    pub staked_amount: Option<u64>,

    /// Delegation information (for validator accounts)
    pub delegations: Option<HashMap<[u8; 32], u64>>,

    /// Account metadata (arbitrary key-value pairs)
    pub metadata: HashMap<String, String>,
}

impl AccountState {
    /// Create a new user account
    pub fn new_user(balance: u64, block_height: u64) -> Self {
        Self {
            balance,
            nonce: 0,
            code: None,
            storage: HashMap::new(),
            last_updated: block_height,
            account_type: AccountType::User,
            staked_amount: None,
            delegations: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new contract account
    pub fn new_contract(balance: u64, code: Vec<u8>, block_height: u64) -> Self {
        Self {
            balance,
            nonce: 0,
            code: Some(code),
            storage: HashMap::new(),
            last_updated: block_height,
            account_type: AccountType::Contract,
            staked_amount: None,
            delegations: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new system account
    pub fn new_system(balance: u64, block_height: u64) -> Self {
        Self {
            balance,
            nonce: 0,
            code: None,
            storage: HashMap::new(),
            last_updated: block_height,
            account_type: AccountType::System,
            staked_amount: None,
            delegations: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new validator account
    pub fn new_validator(balance: u64, staked_amount: u64, block_height: u64) -> Self {
        Self {
            balance,
            nonce: 0,
            code: None,
            storage: HashMap::new(),
            last_updated: block_height,
            account_type: AccountType::Validator,
            staked_amount: Some(staked_amount),
            delegations: Some(HashMap::new()),
            metadata: HashMap::new(),
        }
    }

    /// Check if the account has sufficient balance
    pub fn has_sufficient_balance(&self, amount: u64) -> bool {
        self.balance >= amount
    }

    /// Increment the account nonce
    pub fn increment_nonce(&mut self) {
        self.nonce += 1;
    }

    /// Add balance to the account
    pub fn add_balance(&mut self, amount: u64) -> StateResult<()> {
        self.balance = self.balance.checked_add(amount)
            .ok_or_else(|| StateError::Other("Balance overflow".to_string()))?;
        Ok(())
    }

    /// Subtract balance from the account
    pub fn subtract_balance(&mut self, amount: u64) -> StateResult<()> {
        if !self.has_sufficient_balance(amount) {
            return Err(StateError::InsufficientBalance(amount, self.balance));
        }

        self.balance -= amount;
        Ok(())
    }

    /// Set a storage value
    pub fn set_storage(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.storage.insert(key, value);
    }

    /// Get a storage value
    pub fn get_storage(&self, key: &[u8]) -> Option<&Vec<u8>> {
        self.storage.get(key)
    }

    /// Delete a storage value
    pub fn delete_storage(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.storage.remove(key)
    }

    /// Add a delegation (for validator accounts)
    pub fn add_delegation(&mut self, delegator: [u8; 32], amount: u64) -> StateResult<()> {
        if self.account_type != AccountType::Validator {
            return Err(StateError::InvalidAccount(
                format!("Cannot add delegation to non-validator account: {}", self.account_type)
            ));
        }

        let delegations = self.delegations.get_or_insert_with(HashMap::new);
        let current_amount = delegations.get(&delegator).copied().unwrap_or(0);
        let new_amount = current_amount.checked_add(amount)
            .ok_or_else(|| StateError::Other("Delegation amount overflow".to_string()))?;

        delegations.insert(delegator, new_amount);
        Ok(())
    }

    /// Remove a delegation (for validator accounts)
    pub fn remove_delegation(&mut self, delegator: &[u8; 32], amount: u64) -> StateResult<()> {
        if self.account_type != AccountType::Validator {
            return Err(StateError::InvalidAccount(
                format!("Cannot remove delegation from non-validator account: {}", self.account_type)
            ));
        }

        let delegations = match &mut self.delegations {
            Some(d) => d,
            None => return Err(StateError::Other("No delegations found".to_string())),
        };

        let current_amount = match delegations.get(delegator) {
            Some(a) => *a,
            None => return Err(StateError::Other(format!("No delegation found for {:?}", delegator))),
        };

        if current_amount < amount {
            return Err(StateError::InsufficientBalance(amount, current_amount));
        }

        let new_amount = current_amount - amount;
        if new_amount == 0 {
            delegations.remove(delegator);
        } else {
            delegations.insert(*delegator, new_amount);
        }

        Ok(())
    }

    /// Get the total delegated amount (for validator accounts)
    pub fn get_total_delegated(&self) -> u64 {
        match &self.delegations {
            Some(delegations) => delegations.values().sum(),
            None => 0,
        }
    }

    /// Set a metadata value
    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Get a metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Delete a metadata value
    pub fn delete_metadata(&mut self, key: &str) -> Option<String> {
        self.metadata.remove(key)
    }
}

/// State root hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRoot {
    /// Root hash of the state trie
    pub root_hash: [u8; 32],

    /// Block height at which this state root was calculated
    pub block_height: u64,

    /// Timestamp when this state root was calculated
    pub timestamp: u64,
}

impl StateRoot {
    /// Create a new state root
    pub fn new(root_hash: [u8; 32], block_height: u64, timestamp: u64) -> Self {
        Self {
            root_hash,
            block_height,
            timestamp,
        }
    }

    /// Convert the root hash to a hex string
    pub fn root_hash_hex(&self) -> String {
        hex::encode(self.root_hash)
    }
}

/// Global state structure
#[derive(Debug, Clone)]
pub struct GlobalState {
    /// Current state root
    pub state_root: StateRoot,

    /// Total supply of coins
    pub total_supply: u64,

    /// Total staked amount
    pub total_staked: u64,

    /// Total number of accounts
    pub total_accounts: u64,

    /// Total number of transactions
    pub total_transactions: u64,

    /// Chain parameters
    pub chain_params: ChainParameters,
}

impl GlobalState {
    /// Create a new global state
    pub fn new(state_root: StateRoot, chain_params: ChainParameters) -> Self {
        Self {
            state_root,
            total_supply: chain_params.initial_supply,
            total_staked: 0,
            total_accounts: 0,
            total_transactions: 0,
            chain_params,
        }
    }

    /// Update the state root
    pub fn update_state_root(&mut self, state_root: StateRoot) {
        self.state_root = state_root;
    }

    /// Update the total supply
    pub fn update_total_supply(&mut self, total_supply: u64) {
        self.total_supply = total_supply;
    }

    /// Update the total staked amount
    pub fn update_total_staked(&mut self, total_staked: u64) {
        self.total_staked = total_staked;
    }

    /// Update the total number of accounts
    pub fn update_total_accounts(&mut self, total_accounts: u64) {
        self.total_accounts = total_accounts;
    }

    /// Update the total number of transactions
    pub fn update_total_transactions(&mut self, total_transactions: u64) {
        self.total_transactions = total_transactions;
    }

    /// Get the current block reward
    pub fn get_block_reward(&self) -> u64 {
        self.chain_params.block_reward
    }

    /// Get the current transaction fee
    pub fn get_transaction_fee(&self) -> u64 {
        self.chain_params.transaction_fee
    }

    /// Get the current staking reward rate
    pub fn get_staking_reward_rate(&self) -> f64 {
        self.chain_params.staking_reward_rate
    }
}

/// Chain parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainParameters {
    /// Chain ID
    pub chain_id: u64,

    /// Initial supply of coins
    pub initial_supply: u64,

    /// Block reward
    pub block_reward: u64,

    /// Transaction fee
    pub transaction_fee: u64,

    /// Staking reward rate (annual percentage)
    pub staking_reward_rate: f64,

    /// Minimum stake amount
    pub min_stake_amount: u64,

    /// Maximum validators
    pub max_validators: u64,

    /// Block time target (in seconds)
    pub block_time_target: u64,

    /// Difficulty adjustment period (in blocks)
    pub difficulty_adjustment_period: u64,
}

impl Default for ChainParameters {
    fn default() -> Self {
        Self {
            chain_id: 1,
            initial_supply: 1_000_000_000,
            block_reward: 50,
            transaction_fee: 1,
            staking_reward_rate: 0.05, // 5% annual
            min_stake_amount: 1000,
            max_validators: 100,
            block_time_target: 10, // 10 seconds
            difficulty_adjustment_period: 2016, // ~2 weeks with 10-second blocks
        }
    }
}

/// State transition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Previous state root
    pub prev_state_root: [u8; 32],

    /// New state root
    pub new_state_root: [u8; 32],

    /// Block height
    pub block_height: u64,

    /// Timestamp
    pub timestamp: u64,

    /// Account changes
    pub account_changes: Vec<AccountChange>,
}

/// Account change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountChange {
    /// Account address
    pub address: [u8; 32],

    /// Previous account state
    pub prev_state: Option<AccountState>,

    /// New account state
    pub new_state: Option<AccountState>,

    /// Change type
    pub change_type: AccountChangeType,
}

/// Account change type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccountChangeType {
    /// Account created
    Created,

    /// Account updated
    Updated,

    /// Account deleted
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_state_new_user() {
        let account = AccountState::new_user(1000, 100);
        assert_eq!(account.balance, 1000);
        assert_eq!(account.nonce, 0);
        assert_eq!(account.account_type, AccountType::User);
        assert_eq!(account.last_updated, 100);
        assert!(account.code.is_none());
        assert!(account.storage.is_empty());
        assert!(account.staked_amount.is_none());
        assert!(account.delegations.is_none());
        assert!(account.metadata.is_empty());
    }

    #[test]
    fn test_account_state_new_contract() {
        let code = vec![1, 2, 3, 4];
        let account = AccountState::new_contract(500, code.clone(), 200);
        assert_eq!(account.balance, 500);
        assert_eq!(account.nonce, 0);
        assert_eq!(account.account_type, AccountType::Contract);
        assert_eq!(account.last_updated, 200);
        assert_eq!(account.code, Some(code));
        assert!(account.storage.is_empty());
        assert!(account.staked_amount.is_none());
        assert!(account.delegations.is_none());
        assert!(account.metadata.is_empty());
    }

    #[test]
    fn test_account_state_new_validator() {
        let account = AccountState::new_validator(2000, 1000, 300);
        assert_eq!(account.balance, 2000);
        assert_eq!(account.nonce, 0);
        assert_eq!(account.account_type, AccountType::Validator);
        assert_eq!(account.last_updated, 300);
        assert!(account.code.is_none());
        assert!(account.storage.is_empty());
        assert_eq!(account.staked_amount, Some(1000));
        assert!(account.delegations.unwrap().is_empty());
        assert!(account.metadata.is_empty());
    }

    #[test]
    fn test_account_state_balance_operations() {
        let mut account = AccountState::new_user(1000, 100);

        // Test has_sufficient_balance
        assert!(account.has_sufficient_balance(1000));
        assert!(account.has_sufficient_balance(500));
        assert!(!account.has_sufficient_balance(1001));

        // Test add_balance
        account.add_balance(500).unwrap();
        assert_eq!(account.balance, 1500);

        // Test subtract_balance
        account.subtract_balance(300).unwrap();
        assert_eq!(account.balance, 1200);

        // Test insufficient balance
        let result = account.subtract_balance(1500);
        assert!(result.is_err());
        if let Err(StateError::InsufficientBalance(required, available)) = result {
            assert_eq!(required, 1500);
            assert_eq!(available, 1200);
        } else {
            panic!("Expected InsufficientBalance error");
        }
    }

    #[test]
    fn test_account_state_storage_operations() {
        let mut account = AccountState::new_user(1000, 100);

        // Test set_storage and get_storage
        let key = vec![1, 2, 3];
        let value = vec![4, 5, 6];
        account.set_storage(key.clone(), value.clone());
        assert_eq!(account.get_storage(&key), Some(&value));

        // Test delete_storage
        let removed = account.delete_storage(&key);
        assert_eq!(removed, Some(value));
        assert_eq!(account.get_storage(&key), None);
    }

    #[test]
    fn test_account_state_delegation_operations() {
        let mut account = AccountState::new_validator(2000, 1000, 300);
        let delegator = [1; 32];

        // Test add_delegation
        account.add_delegation(delegator, 500).unwrap();
        let delegations = account.delegations.as_ref().unwrap();
        assert_eq!(delegations.get(&delegator), Some(&500));

        // Test get_total_delegated
        assert_eq!(account.get_total_delegated(), 500);

        // Test add more delegation
        account.add_delegation(delegator, 300).unwrap();
        let delegations = account.delegations.as_ref().unwrap();
        assert_eq!(delegations.get(&delegator), Some(&800));
        assert_eq!(account.get_total_delegated(), 800);

        // Test remove_delegation
        account.remove_delegation(&delegator, 300).unwrap();
        let delegations = account.delegations.as_ref().unwrap();
        assert_eq!(delegations.get(&delegator), Some(&500));
        assert_eq!(account.get_total_delegated(), 500);

        // Test remove all delegation
        account.remove_delegation(&delegator, 500).unwrap();
        let delegations = account.delegations.as_ref().unwrap();
        assert!(delegations.is_empty());
        assert_eq!(account.get_total_delegated(), 0);
    }

    #[test]
    fn test_account_state_metadata_operations() {
        let mut account = AccountState::new_user(1000, 100);

        // Test set_metadata and get_metadata
        account.set_metadata("name".to_string(), "Alice".to_string());
        assert_eq!(account.get_metadata("name"), Some(&"Alice".to_string()));

        // Test delete_metadata
        let removed = account.delete_metadata("name");
        assert_eq!(removed, Some("Alice".to_string()));
        assert_eq!(account.get_metadata("name"), None);
    }

    #[test]
    fn test_state_root() {
        let root_hash = [1; 32];
        let state_root = StateRoot::new(root_hash, 100, 12345);

        assert_eq!(state_root.root_hash, root_hash);
        assert_eq!(state_root.block_height, 100);
        assert_eq!(state_root.timestamp, 12345);
        assert_eq!(state_root.root_hash_hex(), "0101010101010101010101010101010101010101010101010101010101010101");
    }

    #[test]
    fn test_global_state() {
        let root_hash = [1; 32];
        let state_root = StateRoot::new(root_hash, 100, 12345);
        let chain_params = ChainParameters::default();

        let mut global_state = GlobalState::new(state_root.clone(), chain_params.clone());

        assert_eq!(global_state.state_root, state_root);
        assert_eq!(global_state.total_supply, chain_params.initial_supply);
        assert_eq!(global_state.total_staked, 0);
        assert_eq!(global_state.total_accounts, 0);
        assert_eq!(global_state.total_transactions, 0);

        // Test update methods
        let new_root_hash = [2; 32];
        let new_state_root = StateRoot::new(new_root_hash, 101, 12346);
        global_state.update_state_root(new_state_root.clone());
        global_state.update_total_supply(2_000_000_000);
        global_state.update_total_staked(500_000);
        global_state.update_total_accounts(1000);
        global_state.update_total_transactions(5000);

        assert_eq!(global_state.state_root, new_state_root);
        assert_eq!(global_state.total_supply, 2_000_000_000);
        assert_eq!(global_state.total_staked, 500_000);
        assert_eq!(global_state.total_accounts, 1000);
        assert_eq!(global_state.total_transactions, 5000);

        // Test getter methods
        assert_eq!(global_state.get_block_reward(), chain_params.block_reward);
        assert_eq!(global_state.get_transaction_fee(), chain_params.transaction_fee);
        assert_eq!(global_state.get_staking_reward_rate(), chain_params.staking_reward_rate);
    }

    #[test]
    fn test_chain_parameters_default() {
        let params = ChainParameters::default();

        assert_eq!(params.chain_id, 1);
        assert_eq!(params.initial_supply, 1_000_000_000);
        assert_eq!(params.block_reward, 50);
        assert_eq!(params.transaction_fee, 1);
        assert_eq!(params.staking_reward_rate, 0.05);
        assert_eq!(params.min_stake_amount, 1000);
        assert_eq!(params.max_validators, 100);
        assert_eq!(params.block_time_target, 10);
        assert_eq!(params.difficulty_adjustment_period, 2016);
    }
}