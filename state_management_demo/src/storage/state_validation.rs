//! State transition validation for VibeCoin blockchain
//!
//! This module provides functionality for validating state transitions,
//! ensuring that all changes to the blockchain state follow the protocol rules.

use std::collections::HashMap;
use thiserror::Error;
use log::{debug, error, info, warn};

use crate::storage::state::{
    AccountState, AccountType, StateRoot, StateError, StateResult,
    StateTransition, AccountChange, AccountChangeType, GlobalState,
    ChainParameters,
};
use crate::storage::block_store::Hash;

/// Error type for state validation operations
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Invalid state root
    #[error("Invalid state root: {0}")]
    InvalidStateRoot(String),

    /// Invalid account change
    #[error("Invalid account change: {0}")]
    InvalidAccountChange(String),

    /// Invalid balance change
    #[error("Invalid balance change: {0}")]
    InvalidBalanceChange(String),

    /// Invalid nonce change
    #[error("Invalid nonce change: expected increment by 1, got {0}")]
    InvalidNonceChange(u64),

    /// Invalid code change
    #[error("Invalid code change: {0}")]
    InvalidCodeChange(String),

    /// Invalid storage change
    #[error("Invalid storage change: {0}")]
    InvalidStorageChange(String),

    /// Invalid account type change
    #[error("Invalid account type change: cannot change from {0} to {1}")]
    InvalidAccountTypeChange(AccountType, AccountType),

    /// Invalid staking operation
    #[error("Invalid staking operation: {0}")]
    InvalidStakingOperation(String),

    /// Invalid delegation operation
    #[error("Invalid delegation operation: {0}")]
    InvalidDelegationOperation(String),

    /// State error
    #[error("State error: {0}")]
    StateError(#[from] StateError),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for validation operations
pub type ValidationResult<T> = Result<T, ValidationError>;

/// State transition validator
pub struct StateValidator {
    /// Chain parameters
    chain_params: ChainParameters,
}

impl StateValidator {
    /// Create a new state validator
    pub fn new(chain_params: ChainParameters) -> Self {
        Self { chain_params }
    }

    /// Validate a state transition
    pub fn validate_transition(&self, transition: &StateTransition) -> ValidationResult<()> {
        // Validate basic transition properties
        self.validate_transition_basics(transition)?;

        // Validate each account change
        for account_change in &transition.account_changes {
            self.validate_account_change(account_change)?;
        }

        // Validate global state consistency
        self.validate_global_consistency(transition)?;

        Ok(())
    }

    /// Validate basic transition properties
    fn validate_transition_basics(&self, transition: &StateTransition) -> ValidationResult<()> {
        // Ensure block height is valid
        if transition.block_height == 0 {
            return Err(ValidationError::Other("Block height cannot be zero".to_string()));
        }

        // Ensure timestamp is valid
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Timestamp should not be in the future (with some tolerance)
        const TIMESTAMP_TOLERANCE_SECONDS: u64 = 300; // 5 minutes
        if transition.timestamp > current_time + TIMESTAMP_TOLERANCE_SECONDS {
            return Err(ValidationError::Other(format!(
                "Timestamp is too far in the future: {} > {}",
                transition.timestamp,
                current_time + TIMESTAMP_TOLERANCE_SECONDS
            )));
        }

        Ok(())
    }

    /// Validate an account change
    fn validate_account_change(&self, change: &AccountChange) -> ValidationResult<()> {
        match change.change_type {
            AccountChangeType::Created => self.validate_account_creation(change),
            AccountChangeType::Updated => self.validate_account_update(change),
            AccountChangeType::Deleted => self.validate_account_deletion(change),
        }
    }

    /// Validate account creation
    fn validate_account_creation(&self, change: &AccountChange) -> ValidationResult<()> {
        // Ensure previous state is None
        if change.prev_state.is_some() {
            return Err(ValidationError::InvalidAccountChange(
                "Previous state must be None for account creation".to_string(),
            ));
        }

        // Ensure new state is Some
        let new_state = match &change.new_state {
            Some(state) => state,
            None => {
                return Err(ValidationError::InvalidAccountChange(
                    "New state must be Some for account creation".to_string(),
                ))
            }
        };

        // Validate initial account state
        match new_state.account_type {
            AccountType::User => {
                // User accounts should have no code
                if new_state.code.is_some() {
                    return Err(ValidationError::InvalidAccountChange(
                        "User account cannot have code".to_string(),
                    ));
                }
            }
            AccountType::Contract => {
                // Contract accounts must have code
                if new_state.code.is_none() {
                    return Err(ValidationError::InvalidAccountChange(
                        "Contract account must have code".to_string(),
                    ));
                }
            }
            AccountType::System => {
                // System accounts can only be created by the system
                // This would typically be validated at a higher level
            }
            AccountType::Validator => {
                // Validator accounts must have a staked amount
                if new_state.staked_amount.is_none() {
                    return Err(ValidationError::InvalidAccountChange(
                        "Validator account must have a staked amount".to_string(),
                    ));
                }

                // Ensure staked amount meets minimum requirement
                let staked_amount = new_state.staked_amount.unwrap();
                if staked_amount < self.chain_params.min_stake_amount {
                    return Err(ValidationError::InvalidStakingOperation(format!(
                        "Staked amount {} is less than minimum required {}",
                        staked_amount, self.chain_params.min_stake_amount
                    )));
                }
            }
        }

        // Ensure nonce starts at 0
        if new_state.nonce != 0 {
            return Err(ValidationError::InvalidNonceChange(new_state.nonce));
        }

        Ok(())
    }

    /// Validate account update
    fn validate_account_update(&self, change: &AccountChange) -> ValidationResult<()> {
        // Ensure both previous and new states are Some
        let prev_state = match &change.prev_state {
            Some(state) => state,
            None => {
                return Err(ValidationError::InvalidAccountChange(
                    "Previous state must be Some for account update".to_string(),
                ))
            }
        };

        let new_state = match &change.new_state {
            Some(state) => state,
            None => {
                return Err(ValidationError::InvalidAccountChange(
                    "New state must be Some for account update".to_string(),
                ))
            }
        };

        // Validate account type change (generally not allowed)
        if prev_state.account_type != new_state.account_type {
            return Err(ValidationError::InvalidAccountTypeChange(
                prev_state.account_type,
                new_state.account_type,
            ));
        }

        // Validate nonce change
        if new_state.nonce < prev_state.nonce {
            return Err(ValidationError::InvalidNonceChange(new_state.nonce));
        }

        // Validate code change (generally not allowed after creation)
        match prev_state.account_type {
            AccountType::Contract => {
                // Contract code should not change
                if prev_state.code != new_state.code {
                    return Err(ValidationError::InvalidCodeChange(
                        "Contract code cannot be changed after creation".to_string(),
                    ));
                }
            }
            AccountType::User | AccountType::System => {
                // These account types should never have code
                if new_state.code.is_some() {
                    return Err(ValidationError::InvalidCodeChange(
                        "Non-contract accounts cannot have code".to_string(),
                    ));
                }
            }
            AccountType::Validator => {
                // Validator accounts should never have code
                if new_state.code.is_some() {
                    return Err(ValidationError::InvalidCodeChange(
                        "Validator accounts cannot have code".to_string(),
                    ));
                }

                // Validate staking changes
                self.validate_staking_changes(prev_state, new_state)?;
            }
        }

        // Validate last_updated
        if new_state.last_updated <= prev_state.last_updated {
            return Err(ValidationError::InvalidAccountChange(
                format!(
                    "New last_updated ({}) must be greater than previous ({})",
                    new_state.last_updated, prev_state.last_updated
                ),
            ));
        }

        Ok(())
    }

    /// Validate staking changes
    fn validate_staking_changes(
        &self,
        prev_state: &AccountState,
        new_state: &AccountState,
    ) -> ValidationResult<()> {
        // Ensure validator accounts have staked amounts
        let prev_staked = prev_state.staked_amount.unwrap_or(0);
        let new_staked = match new_state.staked_amount {
            Some(amount) => amount,
            None => {
                return Err(ValidationError::InvalidStakingOperation(
                    "Validator account must have a staked amount".to_string(),
                ))
            }
        };

        // If staked amount decreased, ensure it's still above minimum
        if new_staked < prev_staked && new_staked < self.chain_params.min_stake_amount {
            return Err(ValidationError::InvalidStakingOperation(format!(
                "Staked amount {} is less than minimum required {}",
                new_staked, self.chain_params.min_stake_amount
            )));
        }

        // Validate delegation changes
        let prev_delegations = prev_state.delegations.as_ref().unwrap_or(&HashMap::new());
        let new_delegations = match &new_state.delegations {
            Some(delegations) => delegations,
            None => {
                return Err(ValidationError::InvalidDelegationOperation(
                    "Validator account must have delegations field".to_string(),
                ))
            }
        };

        // Check for invalid delegation removals
        for (delegator, prev_amount) in prev_delegations {
            if let Some(new_amount) = new_delegations.get(delegator) {
                // Delegation amount decreased
                if new_amount < prev_amount {
                    // This is allowed, but we might want to validate the withdrawal process
                }
            } else {
                // Delegation was completely removed
                // This is allowed, but we might want to validate the withdrawal process
            }
        }

        // Check for invalid delegation additions
        for (delegator, new_amount) in new_delegations {
            let prev_amount = prev_delegations.get(delegator).copied().unwrap_or(0);
            if new_amount > &prev_amount {
                // Delegation amount increased
                // This is allowed, but we might want to validate the deposit process
            }
        }

        Ok(())
    }

    /// Validate account deletion
    fn validate_account_deletion(&self, change: &AccountChange) -> ValidationResult<()> {
        // Ensure previous state is Some
        let prev_state = match &change.prev_state {
            Some(state) => state,
            None => {
                return Err(ValidationError::InvalidAccountChange(
                    "Previous state must be Some for account deletion".to_string(),
                ))
            }
        };

        // Ensure new state is None
        if change.new_state.is_some() {
            return Err(ValidationError::InvalidAccountChange(
                "New state must be None for account deletion".to_string(),
            ));
        }

        // Validate account can be deleted
        match prev_state.account_type {
            AccountType::Contract => {
                // Contracts can be deleted if they have no balance
                if prev_state.balance > 0 {
                    return Err(ValidationError::InvalidAccountChange(
                        "Cannot delete contract with non-zero balance".to_string(),
                    ));
                }
            }
            AccountType::Validator => {
                // Validators can be deleted if they have no stake and no delegations
                if prev_state.staked_amount.unwrap_or(0) > 0 {
                    return Err(ValidationError::InvalidAccountChange(
                        "Cannot delete validator with non-zero stake".to_string(),
                    ));
                }

                if let Some(delegations) = &prev_state.delegations {
                    if !delegations.is_empty() {
                        return Err(ValidationError::InvalidAccountChange(
                            "Cannot delete validator with active delegations".to_string(),
                        ));
                    }
                }
            }
            AccountType::User | AccountType::System => {
                // These account types can be deleted if they have no balance
                if prev_state.balance > 0 {
                    return Err(ValidationError::InvalidAccountChange(
                        "Cannot delete account with non-zero balance".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate global state consistency
    fn validate_global_consistency(&self, transition: &StateTransition) -> ValidationResult<()> {
        // This would validate that the state transition maintains global invariants
        // For example, total supply should remain constant unless there's minting/burning
        // For now, we'll just do a basic check

        // Ensure the new state root is different from the previous
        if transition.new_state_root == transition.prev_state_root {
            return Err(ValidationError::InvalidStateRoot(
                "New state root must be different from previous".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::state::ChainParameters;

    fn create_test_chain_params() -> ChainParameters {
        ChainParameters {
            chain_id: 1,
            initial_supply: 1_000_000_000,
            block_reward: 50,
            transaction_fee: 1,
            staking_reward_rate: 0.05,
            min_stake_amount: 1000,
            max_validators: 100,
            block_time_target: 10,
            difficulty_adjustment_period: 2016,
        }
    }

    fn create_test_account_state(account_type: AccountType, balance: u64, block_height: u64) -> AccountState {
        match account_type {
            AccountType::User => AccountState::new_user(balance, block_height),
            AccountType::Contract => {
                let code = vec![1, 2, 3, 4]; // Dummy code
                AccountState::new_contract(balance, code, block_height)
            }
            AccountType::System => AccountState::new_system(balance, block_height),
            AccountType::Validator => AccountState::new_validator(balance, 1000, block_height),
        }
    }

    fn create_test_state_transition(
        prev_root: [u8; 32],
        new_root: [u8; 32],
        block_height: u64,
        account_changes: Vec<AccountChange>,
    ) -> StateTransition {
        StateTransition {
            prev_state_root: prev_root,
            new_state_root: new_root,
            block_height,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            account_changes,
        }
    }

    #[test]
    fn test_validate_account_creation() {
        let chain_params = create_test_chain_params();
        let validator = StateValidator::new(chain_params);

        // Valid user account creation
        let user_state = create_test_account_state(AccountType::User, 1000, 100);
        let change = AccountChange {
            address: [1; 32],
            prev_state: None,
            new_state: Some(user_state),
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_ok());

        // Valid contract account creation
        let contract_state = create_test_account_state(AccountType::Contract, 500, 100);
        let change = AccountChange {
            address: [2; 32],
            prev_state: None,
            new_state: Some(contract_state),
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_ok());

        // Valid validator account creation
        let validator_state = create_test_account_state(AccountType::Validator, 2000, 100);
        let change = AccountChange {
            address: [3; 32],
            prev_state: None,
            new_state: Some(validator_state),
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_ok());

        // Invalid account creation (prev_state is Some)
        let user_state1 = create_test_account_state(AccountType::User, 1000, 100);
        let user_state2 = create_test_account_state(AccountType::User, 2000, 100);
        let change = AccountChange {
            address: [4; 32],
            prev_state: Some(user_state1),
            new_state: Some(user_state2),
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account creation (new_state is None)
        let change = AccountChange {
            address: [5; 32],
            prev_state: None,
            new_state: None,
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account creation (contract without code)
        let mut invalid_contract = create_test_account_state(AccountType::User, 1000, 100);
        invalid_contract.account_type = AccountType::Contract;
        invalid_contract.code = None;
        let change = AccountChange {
            address: [6; 32],
            prev_state: None,
            new_state: Some(invalid_contract),
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account creation (user with code)
        let mut invalid_user = create_test_account_state(AccountType::User, 1000, 100);
        invalid_user.code = Some(vec![1, 2, 3]);
        let change = AccountChange {
            address: [7; 32],
            prev_state: None,
            new_state: Some(invalid_user),
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account creation (validator with insufficient stake)
        let mut invalid_validator = create_test_account_state(AccountType::Validator, 2000, 100);
        invalid_validator.staked_amount = Some(500); // Below minimum
        let change = AccountChange {
            address: [8; 32],
            prev_state: None,
            new_state: Some(invalid_validator),
            change_type: AccountChangeType::Created,
        };
        assert!(validator.validate_account_change(&change).is_err());
    }

    #[test]
    fn test_validate_account_update() {
        let chain_params = create_test_chain_params();
        let validator = StateValidator::new(chain_params);

        // Valid user account update
        let prev_user = create_test_account_state(AccountType::User, 1000, 100);
        let mut new_user = prev_user.clone();
        new_user.balance = 1500;
        new_user.nonce = 1;
        new_user.last_updated = 101;
        let change = AccountChange {
            address: [1; 32],
            prev_state: Some(prev_user),
            new_state: Some(new_user),
            change_type: AccountChangeType::Updated,
        };
        assert!(validator.validate_account_change(&change).is_ok());

        // Invalid account update (prev_state is None)
        let user_state = create_test_account_state(AccountType::User, 1000, 100);
        let change = AccountChange {
            address: [2; 32],
            prev_state: None,
            new_state: Some(user_state),
            change_type: AccountChangeType::Updated,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account update (new_state is None)
        let user_state = create_test_account_state(AccountType::User, 1000, 100);
        let change = AccountChange {
            address: [3; 32],
            prev_state: Some(user_state),
            new_state: None,
            change_type: AccountChangeType::Updated,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account update (account type change)
        let prev_user = create_test_account_state(AccountType::User, 1000, 100);
        let new_contract = create_test_account_state(AccountType::Contract, 1000, 101);
        let change = AccountChange {
            address: [4; 32],
            prev_state: Some(prev_user),
            new_state: Some(new_contract),
            change_type: AccountChangeType::Updated,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account update (nonce decrease)
        let mut prev_user = create_test_account_state(AccountType::User, 1000, 100);
        prev_user.nonce = 5;
        let mut new_user = prev_user.clone();
        new_user.nonce = 3; // Decreased
        new_user.last_updated = 101;
        let change = AccountChange {
            address: [5; 32],
            prev_state: Some(prev_user),
            new_state: Some(new_user),
            change_type: AccountChangeType::Updated,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account update (contract code change)
        let prev_contract = create_test_account_state(AccountType::Contract, 1000, 100);
        let mut new_contract = prev_contract.clone();
        new_contract.code = Some(vec![5, 6, 7, 8]); // Different code
        new_contract.last_updated = 101;
        let change = AccountChange {
            address: [6; 32],
            prev_state: Some(prev_contract),
            new_state: Some(new_contract),
            change_type: AccountChangeType::Updated,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account update (last_updated not increased)
        let prev_user = create_test_account_state(AccountType::User, 1000, 100);
        let mut new_user = prev_user.clone();
        new_user.balance = 1500;
        new_user.last_updated = 100; // Same as before
        let change = AccountChange {
            address: [7; 32],
            prev_state: Some(prev_user),
            new_state: Some(new_user),
            change_type: AccountChangeType::Updated,
        };
        assert!(validator.validate_account_change(&change).is_err());
    }

    #[test]
    fn test_validate_account_deletion() {
        let chain_params = create_test_chain_params();
        let validator = StateValidator::new(chain_params);

        // Valid user account deletion (zero balance)
        let mut user_state = create_test_account_state(AccountType::User, 0, 100);
        user_state.balance = 0;
        let change = AccountChange {
            address: [1; 32],
            prev_state: Some(user_state),
            new_state: None,
            change_type: AccountChangeType::Deleted,
        };
        assert!(validator.validate_account_change(&change).is_ok());

        // Invalid account deletion (prev_state is None)
        let change = AccountChange {
            address: [2; 32],
            prev_state: None,
            new_state: None,
            change_type: AccountChangeType::Deleted,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account deletion (new_state is Some)
        let user_state = create_test_account_state(AccountType::User, 0, 100);
        let change = AccountChange {
            address: [3; 32],
            prev_state: Some(user_state.clone()),
            new_state: Some(user_state),
            change_type: AccountChangeType::Deleted,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid account deletion (non-zero balance)
        let user_state = create_test_account_state(AccountType::User, 1000, 100);
        let change = AccountChange {
            address: [4; 32],
            prev_state: Some(user_state),
            new_state: None,
            change_type: AccountChangeType::Deleted,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid validator deletion (with stake)
        let mut validator_state = create_test_account_state(AccountType::Validator, 2000, 100);
        validator_state.staked_amount = Some(1000);
        let change = AccountChange {
            address: [5; 32],
            prev_state: Some(validator_state),
            new_state: None,
            change_type: AccountChangeType::Deleted,
        };
        assert!(validator.validate_account_change(&change).is_err());

        // Invalid validator deletion (with delegations)
        let mut validator_state = create_test_account_state(AccountType::Validator, 2000, 100);
        validator_state.staked_amount = Some(0);
        let mut delegations = HashMap::new();
        delegations.insert([1; 32], 500);
        validator_state.delegations = Some(delegations);
        let change = AccountChange {
            address: [6; 32],
            prev_state: Some(validator_state),
            new_state: None,
            change_type: AccountChangeType::Deleted,
        };
        assert!(validator.validate_account_change(&change).is_err());
    }

    #[test]
    fn test_validate_transition() {
        let chain_params = create_test_chain_params();
        let validator = StateValidator::new(chain_params);

        // Create a valid state transition
        let prev_root = [1; 32];
        let new_root = [2; 32];
        let block_height = 100;

        // Create a valid account change
        let user_state = create_test_account_state(AccountType::User, 1000, block_height);
        let account_change = AccountChange {
            address: [1; 32],
            prev_state: None,
            new_state: Some(user_state),
            change_type: AccountChangeType::Created,
        };

        let transition = create_test_state_transition(
            prev_root,
            new_root,
            block_height,
            vec![account_change],
        );

        // Validate the transition
        assert!(validator.validate_transition(&transition).is_ok());

        // Invalid transition (same state root)
        let invalid_transition = create_test_state_transition(
            prev_root,
            prev_root, // Same as prev_root
            block_height,
            vec![],
        );
        assert!(validator.validate_transition(&invalid_transition).is_err());

        // Invalid transition (zero block height)
        let invalid_transition = create_test_state_transition(
            prev_root,
            new_root,
            0, // Zero block height
            vec![],
        );
        assert!(validator.validate_transition(&invalid_transition).is_err());

        // Invalid transition (future timestamp)
        let mut invalid_transition = create_test_state_transition(
            prev_root,
            new_root,
            block_height,
            vec![],
        );
        invalid_transition.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() + 3600; // 1 hour in the future
        assert!(validator.validate_transition(&invalid_transition).is_err());
    }
}
