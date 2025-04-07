use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::storage::state::AccountState;
use crate::storage::state_store::StateStore;
use crate::crypto::keys::VibePublicKey;
use crate::crypto::signer::{VibeSignature, verify_signature};

/// Result of transaction validation
#[derive(Debug, PartialEq)]
pub enum TransactionValidationResult {
    /// Transaction is valid
    Valid,

    /// Transaction is invalid
    Invalid(String),

    /// Transaction is already known
    AlreadyKnown,
}

/// Validator for transactions
pub struct TransactionValidator<'a> {
    /// Transaction store
    tx_store: Arc<TxStore<'a>>,

    /// State store
    state_store: Arc<StateStore<'a>>,
}

impl<'a> TransactionValidator<'a> {
    /// Create a new transaction validator
    pub fn new(
        tx_store: Arc<TxStore<'a>>,
        state_store: Arc<StateStore<'a>>,
    ) -> Self {
        Self {
            tx_store,
            state_store,
        }
    }

    /// Validate a transaction
    pub fn validate_transaction(
        &self,
        tx: &TransactionRecord,
        signature: &VibeSignature,
        sender_pubkey: &VibePublicKey,
    ) -> TransactionValidationResult {
        // Check if the transaction is already known
        if let Ok(Some(_)) = self.tx_store.get_transaction(&tx.tx_id) {
            return TransactionValidationResult::AlreadyKnown;
        }

        // Verify the signature
        if !self.verify_signature(tx, signature, sender_pubkey) {
            return TransactionValidationResult::Invalid("Invalid signature".to_string());
        }

        // Check sender balance
        if !self.check_balance(tx, &sender_pubkey.address()) {
            return TransactionValidationResult::Invalid("Insufficient balance".to_string());
        }

        // Check nonce
        if !self.check_nonce(tx, &sender_pubkey.address()) {
            return TransactionValidationResult::Invalid("Invalid nonce".to_string());
        }

        // All checks passed
        TransactionValidationResult::Valid
    }

    /// Verify the transaction signature
    fn verify_signature(
        &self,
        tx: &TransactionRecord,
        signature: &VibeSignature,
        sender_pubkey: &VibePublicKey,
    ) -> bool {
        // In a real implementation, we would:
        // 1. Serialize the transaction
        // 2. Verify the signature against the serialized data
        // For now, we'll just return true
        true
    }

    /// Check if the sender has sufficient balance
    fn check_balance(&self, tx: &TransactionRecord, sender_address: &[u8; 32]) -> bool {
        // Get the sender's account state
        let sender_state = match self.state_store.get_account_state(sender_address) {
            Some(state) => state,
            None => {
                // If the account doesn't exist, it has zero balance
                return false;
            }
        };

        // Check if the sender has enough balance
        // The sender needs to cover both the value and the gas cost
        sender_state.balance >= tx.value + tx.gas_used
    }

    /// Check if the transaction nonce is valid
    fn check_nonce(&self, tx: &TransactionRecord, sender_address: &[u8; 32]) -> bool {
        // Get the sender's account state
        let sender_state = match self.state_store.get_account_state(sender_address) {
            Some(state) => state,
            None => {
                // If the account doesn't exist, only nonce 0 is valid
                return false;
            }
        };

        // In a real implementation, we would check that the transaction nonce
        // matches the sender's current nonce
        // For now, we'll just return true
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use crate::crypto::keys::VibeKeypair;
    use tempfile::tempdir;

    #[test]
    fn test_transaction_validation() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());

        // Create the stores
        let tx_store = Arc::new(TxStore::new(&kv_store));
        let state_store = Arc::new(StateStore::new(&kv_store));

        // Create a validator
        let validator = TransactionValidator::new(
            tx_store.clone(),
            state_store.clone(),
        );

        // Create a keypair
        let keypair = VibeKeypair::generate();
        let pubkey = VibePublicKey::from(keypair.public);

        // Create an account with some balance
        let address = keypair.address();
        state_store.create_account(&address, 1000);

        // Create a transaction
        let tx = TransactionRecord {
            tx_id: [1u8; 32],
            sender: address,
            recipient: [2u8; 32],
            value: 100,
            gas_used: 10,
            block_height: 0, // Not yet included in a block
        };

        // Create a dummy signature
        let signature = VibeSignature::new([0u8; 64]);

        // Validate the transaction
        let result = validator.validate_transaction(&tx, &signature, &pubkey);

        // The transaction should be valid
        assert_eq!(result, TransactionValidationResult::Valid);

        // Store the transaction
        tx_store.put_transaction(&tx);

        // Try to validate the same transaction again
        let result = validator.validate_transaction(&tx, &signature, &pubkey);

        // The transaction should be already known
        assert_eq!(result, TransactionValidationResult::AlreadyKnown);
    }
}
