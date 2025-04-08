use std::sync::Arc;
use log::error;

use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::storage::state_store::StateStore;
use crate::crypto::keys::VibePublicKey;
use crate::crypto::signer::VibeSignature;
use crate::mempool::types::TransactionRecord as MempoolTransactionRecord;

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

    /// Serialize a transaction for signature verification
    fn serialize_for_signing(&self, tx: &TransactionRecord) -> Vec<u8> {
        // Create a canonical representation for signing
        let mut data = Vec::new();

        // Add all transaction fields except signature
        data.extend_from_slice(&tx.sender);
        data.extend_from_slice(&tx.recipient);
        data.extend_from_slice(&tx.value.to_be_bytes());
        data.extend_from_slice(&tx.gas_price.to_be_bytes());
        data.extend_from_slice(&tx.gas_limit.to_be_bytes());
        data.extend_from_slice(&tx.nonce.to_be_bytes());
        data.extend_from_slice(&tx.timestamp.to_be_bytes());

        // Add optional data if present
        if let Some(tx_data) = &tx.data {
            data.extend_from_slice(tx_data);
        }

        data
    }

    /// Verify the transaction signature
    fn verify_signature(
        &self,
        tx: &TransactionRecord,
        signature: &VibeSignature,
        sender_pubkey: &VibePublicKey,
    ) -> bool {
        // Get the serialized transaction data for signing
        let tx_data = self.serialize_for_signing(tx);

        // Convert VibePublicKey to ed25519_dalek PublicKey
        match sender_pubkey.to_dalek_pubkey() {
            Ok(pubkey) => {
                // Verify the signature using the crypto module
                crate::crypto::signer::verify_signature(&tx_data, signature, &pubkey)
            },
            Err(e) => {
                error!("Failed to convert public key: {:?}", e);
                false
            }
        }
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
    fn check_nonce(&self, _tx: &TransactionRecord, sender_address: &[u8; 32]) -> bool {
        // Get the sender's account state
        let _sender_state = match self.state_store.get_account_state(sender_address) {
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
