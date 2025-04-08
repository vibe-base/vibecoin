//! Object transaction processor for VibeCoin blockchain
//!
//! This module provides functionality for processing object transactions,
//! including validation and execution.

use log::{error, info};
use thiserror::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage::object::ObjectError;
use crate::storage::object_store::{ObjectStore, ObjectStoreError};
use crate::storage::object_transaction::{ObjectTransactionRecord, ObjectTransactionKind};
use crate::storage::state_store::{StateStore, StateStoreError};
use crate::crypto::keys::VibePublicKey;
use crate::crypto::signer::verify_signature;

/// Error type for object transaction processing
#[derive(Debug, Error)]
pub enum ObjectProcessorError {
    /// Object store error
    #[error("Object store error: {0}")]
    ObjectStoreError(#[from] ObjectStoreError),

    /// Object error
    #[error("Object error: {0}")]
    ObjectError(#[from] ObjectError),

    /// State store error
    #[error("State store error: {0}")]
    StateStoreError(#[from] StateStoreError),

    /// Invalid signature
    #[error("Invalid signature")]
    InvalidSignature,

    /// Invalid nonce
    #[error("Invalid nonce: expected {0}, got {1}")]
    InvalidNonce(u64, u64),

    /// Insufficient balance
    #[error("Insufficient balance: required {0}, available {1}")]
    InsufficientBalance(u64, u64),

    /// Object not found
    #[error("Object not found: {0}")]
    ObjectNotFound(String),

    /// Unauthorized access
    #[error("Unauthorized access: {0}")]
    UnauthorizedAccess(String),

    /// Invalid transaction
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for object transaction processing
pub type ObjectProcessorResult<T> = Result<T, ObjectProcessorError>;

/// Object transaction processor
pub struct ObjectProcessor<'a> {
    /// Object store
    object_store: &'a ObjectStore<'a>,

    /// State store (for account balances and nonces)
    state_store: &'a StateStore<'a>,

    /// Current block height
    block_height: u64,
}

impl<'a> ObjectProcessor<'a> {
    /// Create a new object transaction processor
    pub fn new(
        object_store: &'a ObjectStore<'a>,
        state_store: &'a StateStore<'a>,
        block_height: u64,
    ) -> Self {
        Self {
            object_store,
            state_store,
            block_height,
        }
    }

    /// Process an object transaction
    pub fn process_transaction(
        &self,
        tx: &ObjectTransactionRecord,
    ) -> ObjectProcessorResult<()> {
        // Validate the transaction
        self.validate_transaction(tx)?;

        // Execute the transaction
        self.execute_transaction(tx)?;

        Ok(())
    }

    /// Validate an object transaction
    fn validate_transaction(
        &self,
        tx: &ObjectTransactionRecord,
    ) -> ObjectProcessorResult<()> {
        // Verify signature
        // First, get the public key from the sender address
        // In a real implementation, we would look up the public key from the account state
        // For now, we'll just use a placeholder approach
        let sender_pubkey = match VibePublicKey::try_from(&tx.sender[..]) {
            Ok(pubkey) => pubkey,
            Err(_) => return Err(ObjectProcessorError::InvalidSignature),
        };

        // Convert to ed25519_dalek PublicKey
        let dalek_pubkey = match sender_pubkey.to_dalek_pubkey() {
            Ok(pubkey) => pubkey,
            Err(_) => return Err(ObjectProcessorError::InvalidSignature),
        };

        // Verify the signature
        if !verify_signature(&tx.serialize_for_signing(), &tx.signature, &dalek_pubkey) {
            return Err(ObjectProcessorError::InvalidSignature);
        }

        // Check nonce
        let account_state = self.state_store.get_account_state(&tx.sender);

        if let Some(account) = account_state {
            if tx.nonce != account.nonce {
                return Err(ObjectProcessorError::InvalidNonce(account.nonce, tx.nonce));
            }

            // Check balance for gas cost
            if account.balance < tx.gas_cost() {
                return Err(ObjectProcessorError::InsufficientBalance(
                    tx.gas_cost(),
                    account.balance,
                ));
            }
        } else {
            return Err(ObjectProcessorError::InvalidTransaction(
                format!("Account does not exist: {}", hex::encode(&tx.sender))
            ));
        }

        // Validate transaction-specific requirements
        match &tx.kind {
            ObjectTransactionKind::CreateObject { .. } => {
                // No additional validation needed for object creation
            },
            ObjectTransactionKind::MutateObject { id, .. } => {
                // Check that the object exists
                let object = match self.object_store.get_object(id)? {
                    Some(obj) => obj,
                    None => return Err(ObjectProcessorError::ObjectNotFound(hex::encode(id))),
                };

                // Check ownership
                if object.is_immutable() {
                    return Err(ObjectProcessorError::UnauthorizedAccess(
                        format!("Cannot mutate immutable object: {}", hex::encode(id))
                    ));
                }

                if !object.is_shared() && !object.is_owned_by(&tx.sender) {
                    return Err(ObjectProcessorError::UnauthorizedAccess(
                        format!("Sender does not own object: {}", hex::encode(id))
                    ));
                }
            },
            ObjectTransactionKind::TransferObject { id, .. } => {
                // Check that the object exists
                let object = match self.object_store.get_object(id)? {
                    Some(obj) => obj,
                    None => return Err(ObjectProcessorError::ObjectNotFound(hex::encode(id))),
                };

                // Check ownership
                if object.is_immutable() {
                    return Err(ObjectProcessorError::UnauthorizedAccess(
                        format!("Cannot transfer immutable object: {}", hex::encode(id))
                    ));
                }

                if !object.is_owned_by(&tx.sender) {
                    return Err(ObjectProcessorError::UnauthorizedAccess(
                        format!("Sender does not own object: {}", hex::encode(id))
                    ));
                }
            },
            ObjectTransactionKind::DeleteObject { id } => {
                // Check that the object exists
                let object = match self.object_store.get_object(id)? {
                    Some(obj) => obj,
                    None => return Err(ObjectProcessorError::ObjectNotFound(hex::encode(id))),
                };

                // Check ownership
                if object.is_immutable() {
                    return Err(ObjectProcessorError::UnauthorizedAccess(
                        format!("Cannot delete immutable object: {}", hex::encode(id))
                    ));
                }

                if !object.is_owned_by(&tx.sender) {
                    return Err(ObjectProcessorError::UnauthorizedAccess(
                        format!("Sender does not own object: {}", hex::encode(id))
                    ));
                }
            },
        }

        Ok(())
    }

    /// Execute an object transaction
    fn execute_transaction(
        &self,
        tx: &ObjectTransactionRecord,
    ) -> ObjectProcessorResult<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Execute transaction-specific logic
        match &tx.kind {
            ObjectTransactionKind::CreateObject { type_tag, owner, contents, metadata } => {
                // Create the object
                self.object_store.create_object(
                    owner.clone(),
                    type_tag.clone(),
                    contents.clone(),
                    metadata.clone(),
                )?;

                info!("Created new object of type {}", type_tag);
            },
            ObjectTransactionKind::MutateObject { id, new_contents } => {
                // Update the object
                self.object_store.update_object(id, |obj| {
                    obj.update_contents(
                        new_contents.clone(),
                        timestamp,
                        self.block_height,
                    )
                })?;

                info!("Updated object {}", hex::encode(id));
            },
            ObjectTransactionKind::TransferObject { id, new_owner } => {
                // Transfer the object
                self.object_store.update_object(id, |obj| {
                    obj.transfer_ownership(
                        new_owner.clone(),
                        timestamp,
                        self.block_height,
                    )
                })?;

                info!("Transferred object {}", hex::encode(id));
            },
            ObjectTransactionKind::DeleteObject { id } => {
                // Delete the object
                self.object_store.delete_object(id)?;

                info!("Deleted object {}", hex::encode(id));
            },
        }

        // Update account nonce
        match self.state_store.increment_nonce(&tx.sender) {
            Ok(_) => {},
            Err(e) => return Err(ObjectProcessorError::StateStoreError(e)),
        };

        // Deduct gas cost by updating balance
        let account = match self.state_store.get_account_state(&tx.sender) {
            Some(account) => account,
            None => return Err(ObjectProcessorError::Other(format!("Account not found: {}", hex::encode(&tx.sender)))),
        };

        // Calculate new balance after gas deduction
        let new_balance = match account.balance.checked_sub(tx.gas_cost()) {
            Some(balance) => balance,
            None => return Err(ObjectProcessorError::InsufficientBalance(tx.gas_cost(), account.balance)),
        };

        // Update the balance
        match self.state_store.update_balance(&tx.sender, new_balance) {
            Ok(_) => {},
            Err(e) => return Err(ObjectProcessorError::StateStoreError(e)),
        };

        Ok(())
    }

    /// Set the current block height
    pub fn set_block_height(&mut self, block_height: u64) {
        self.block_height = block_height;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use crate::storage::state::{AccountState, AccountType};
    use crate::crypto::keys::VibeKeypair;
    use tempfile::tempdir;

    fn setup_test_environment() -> (
        tempfile::TempDir,
        ObjectStore<'static>,
        StateStore<'static>,
        ObjectProcessor<'static>,
        VibeKeypair,
    ) {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let kv_store_static = Box::leak(Box::new(kv_store));

        let object_store = ObjectStore::new(kv_store_static);
        let state_store = StateStore::new(kv_store_static);
        let processor = ObjectProcessor::new(&object_store, &state_store, 1);

        let keypair = VibeKeypair::generate();
        let sender = keypair.address();

        // Create an account with balance
        state_store.create_account(&sender, 1_000_000, AccountType::User).unwrap();

        (temp_dir, object_store, state_store, processor, keypair)
    }

    #[test]
    fn test_create_object_transaction() {
        let (_temp_dir, object_store, _state_store, processor, keypair) = setup_test_environment();
        let sender = keypair.address();

        // Create a transaction to create an object
        let kind = ObjectTransactionKind::CreateObject {
            type_tag: "TestCoin".to_string(),
            owner: Ownership::Address(sender),
            contents: vec![1, 2, 3, 4],
            metadata: None,
        };

        let tx = ObjectTransactionRecord::create_signed(
            &keypair,
            kind,
            5,
            21000,
            0, // Nonce should be 0 for a new account
        );

        // Process the transaction
        processor.process_transaction(&tx).unwrap();

        // Check that the object was created
        let objects = object_store.get_objects_by_owner(&sender).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].type_tag, "TestCoin");
        assert_eq!(objects[0].contents, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_mutate_object_transaction() {
        let (_temp_dir, object_store, state_store, processor, keypair) = setup_test_environment();
        let sender = keypair.address();

        // Create an object
        let object = object_store.create_object(
            Ownership::Address(sender),
            "TestCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Increment nonce after first transaction
        state_store.increment_nonce(&sender).unwrap();

        // Create a transaction to mutate the object
        let kind = ObjectTransactionKind::MutateObject {
            id: object.id,
            new_contents: vec![5, 6, 7, 8],
        };

        let tx = ObjectTransactionRecord::create_signed(
            &keypair,
            kind,
            5,
            21000,
            1, // Nonce should be 1 now
        );

        // Process the transaction
        processor.process_transaction(&tx).unwrap();

        // Check that the object was updated
        let updated = object_store.get_object(&object.id).unwrap().unwrap();
        assert_eq!(updated.version, 1);
        assert_eq!(updated.contents, vec![5, 6, 7, 8]);
    }

    #[test]
    fn test_transfer_object_transaction() {
        let (_temp_dir, object_store, state_store, processor, keypair) = setup_test_environment();
        let sender = keypair.address();
        let recipient = [2; 32];

        // Create an object
        let object = object_store.create_object(
            Ownership::Address(sender),
            "TestCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Increment nonce after first transaction
        state_store.increment_nonce(&sender).unwrap();

        // Create a transaction to transfer the object
        let kind = ObjectTransactionKind::TransferObject {
            id: object.id,
            new_owner: Ownership::Address(recipient),
        };

        let tx = ObjectTransactionRecord::create_signed(
            &keypair,
            kind,
            5,
            21000,
            1, // Nonce should be 1 now
        );

        // Process the transaction
        processor.process_transaction(&tx).unwrap();

        // Check that the object was transferred
        let updated = object_store.get_object(&object.id).unwrap().unwrap();
        match updated.owner {
            Ownership::Address(addr) => assert_eq!(addr, recipient),
            _ => panic!("Expected Address ownership"),
        }

        // Check that the object is now owned by the recipient
        let recipient_objects = object_store.get_objects_by_owner(&recipient).unwrap();
        assert_eq!(recipient_objects.len(), 1);
        assert_eq!(recipient_objects[0].id, object.id);

        // Check that the sender no longer owns the object
        let sender_objects = object_store.get_objects_by_owner(&sender).unwrap();
        assert_eq!(sender_objects.len(), 0);
    }

    #[test]
    fn test_delete_object_transaction() {
        let (_temp_dir, object_store, state_store, processor, keypair) = setup_test_environment();
        let sender = keypair.address();

        // Create an object
        let object = object_store.create_object(
            Ownership::Address(sender),
            "TestCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Increment nonce after first transaction
        state_store.increment_nonce(&sender).unwrap();

        // Create a transaction to delete the object
        let kind = ObjectTransactionKind::DeleteObject {
            id: object.id,
        };

        let tx = ObjectTransactionRecord::create_signed(
            &keypair,
            kind,
            5,
            21000,
            1, // Nonce should be 1 now
        );

        // Process the transaction
        processor.process_transaction(&tx).unwrap();

        // Check that the object was deleted
        let result = object_store.get_object(&object.id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_unauthorized_access() {
        let (_temp_dir, object_store, state_store, processor, keypair) = setup_test_environment();
        let sender = keypair.address();
        let other_keypair = VibeKeypair::generate();
        let other_sender = other_keypair.address();

        // Create an account for the other sender
        state_store.create_account(&other_sender, 1_000_000, AccountType::User).unwrap();

        // Create an object owned by the first sender
        let object = object_store.create_object(
            Ownership::Address(sender),
            "TestCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Try to mutate the object with the other sender
        let kind = ObjectTransactionKind::MutateObject {
            id: object.id,
            new_contents: vec![5, 6, 7, 8],
        };

        let tx = ObjectTransactionRecord::create_signed(
            &other_keypair,
            kind,
            5,
            21000,
            0,
        );

        // Process the transaction (should fail)
        let result = processor.process_transaction(&tx);
        assert!(result.is_err());
        match result {
            Err(ObjectProcessorError::UnauthorizedAccess(_)) => {},
            _ => panic!("Expected UnauthorizedAccess error"),
        }
    }

    #[test]
    fn test_immutable_object() {
        let (_temp_dir, object_store, state_store, processor, keypair) = setup_test_environment();
        let sender = keypair.address();

        // Create an immutable object
        let object = object_store.create_object(
            Ownership::Immutable,
            "ImmutableCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Increment nonce after first transaction
        state_store.increment_nonce(&sender).unwrap();

        // Try to mutate the immutable object
        let kind = ObjectTransactionKind::MutateObject {
            id: object.id,
            new_contents: vec![5, 6, 7, 8],
        };

        let tx = ObjectTransactionRecord::create_signed(
            &keypair,
            kind,
            5,
            21000,
            1,
        );

        // Process the transaction (should fail)
        let result = processor.process_transaction(&tx);
        assert!(result.is_err());
        match result {
            Err(ObjectProcessorError::UnauthorizedAccess(_)) => {},
            _ => panic!("Expected UnauthorizedAccess error"),
        }
    }

    #[test]
    fn test_shared_object() {
        let (_temp_dir, object_store, state_store, processor, keypair) = setup_test_environment();
        let sender = keypair.address();
        let other_keypair = VibeKeypair::generate();
        let other_sender = other_keypair.address();

        // Create an account for the other sender
        state_store.create_account(&other_sender, 1_000_000, AccountType::User).unwrap();

        // Create a shared object
        let object = object_store.create_object(
            Ownership::Shared,
            "SharedCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Increment nonce after first transaction
        state_store.increment_nonce(&sender).unwrap();

        // Try to mutate the shared object with the other sender
        let kind = ObjectTransactionKind::MutateObject {
            id: object.id,
            new_contents: vec![5, 6, 7, 8],
        };

        let tx = ObjectTransactionRecord::create_signed(
            &other_keypair,
            kind,
            5,
            21000,
            0,
        );

        // Process the transaction (should succeed)
        processor.process_transaction(&tx).unwrap();

        // Check that the object was updated
        let updated = object_store.get_object(&object.id).unwrap().unwrap();
        assert_eq!(updated.version, 1);
        assert_eq!(updated.contents, vec![5, 6, 7, 8]);
    }
}
