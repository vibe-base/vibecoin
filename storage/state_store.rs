use serde::{Serialize, Deserialize};
use crate::storage::kv_store::KVStore;

/// Account state structure
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub contract_data: Option<Vec<u8>>,
}

/// Store for account states
pub struct StateStore<'a> {
    store: &'a dyn KVStore,
}

impl<'a> StateStore<'a> {
    /// Create a new StateStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }

    /// Get the state of an account
    pub fn get_account_state(&self, address: &[u8]) -> Option<AccountState> {
        let key = format!("state:addr:{:x?}", address);
        self.store.get(key.as_bytes())
            .and_then(|bytes| bincode::deserialize(&bytes).ok())
    }

    /// Set the state of an account
    pub fn set_account_state(&self, address: &[u8], state: &AccountState) {
        let key = format!("state:addr:{:x?}", address);
        let value = bincode::serialize(state).unwrap();
        self.store.put(key.as_bytes(), &value);
    }
    
    /// Update account balance
    pub fn update_balance(&self, address: &[u8], new_balance: u64) -> Result<(), &'static str> {
        if let Some(mut state) = self.get_account_state(address) {
            state.balance = new_balance;
            self.set_account_state(address, &state);
            Ok(())
        } else {
            Err("Account not found")
        }
    }
    
    /// Increment account nonce
    pub fn increment_nonce(&self, address: &[u8]) -> Result<u64, &'static str> {
        if let Some(mut state) = self.get_account_state(address) {
            state.nonce += 1;
            self.set_account_state(address, &state);
            Ok(state.nonce)
        } else {
            Err("Account not found")
        }
    }
    
    /// Create a new account with default values
    pub fn create_account(&self, address: &[u8], initial_balance: u64) {
        let state = AccountState {
            balance: initial_balance,
            nonce: 0,
            contract_data: None,
        };
        self.set_account_state(address, &state);
    }
    
    /// Set contract data for an account
    pub fn set_contract_data(&self, address: &[u8], data: Vec<u8>) -> Result<(), &'static str> {
        if let Some(mut state) = self.get_account_state(address) {
            state.contract_data = Some(data);
            self.set_account_state(address, &state);
            Ok(())
        } else {
            Err("Account not found")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[test]
    fn test_state_store() {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let state_store = StateStore::new(&kv_store);
        
        let address = [1; 32];
        
        // Create a new account
        state_store.create_account(&address, 1000);
        
        // Get the account state
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.balance, 1000);
        assert_eq!(state.nonce, 0);
        assert_eq!(state.contract_data, None);
        
        // Update balance
        state_store.update_balance(&address, 2000).unwrap();
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.balance, 2000);
        
        // Increment nonce
        let new_nonce = state_store.increment_nonce(&address).unwrap();
        assert_eq!(new_nonce, 1);
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.nonce, 1);
        
        // Set contract data
        let contract_data = vec![1, 2, 3, 4];
        state_store.set_contract_data(&address, contract_data.clone()).unwrap();
        let state = state_store.get_account_state(&address).unwrap();
        assert_eq!(state.contract_data, Some(contract_data));
    }
}
