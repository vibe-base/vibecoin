//! Object transaction types for VibeCoin blockchain
//!
//! This module defines transaction types for object operations,
//! extending the existing transaction model to support Sui-style objects.

use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

use crate::crypto::signer::{VibeSignature, sign_message};
use crate::crypto::keys::VibeKeypair;
use crate::storage::object::{ObjectId, Ownership};

/// Type alias for address (public key hash)
pub type Address = [u8; 32];

/// Type alias for transaction hash
pub type Hash = [u8; 32];

/// Object transaction kinds
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObjectTransactionKind {
    /// Create a new object
    CreateObject {
        /// Type tag for the object
        type_tag: String,
        
        /// Initial ownership of the object
        owner: Ownership,
        
        /// Object contents
        contents: Vec<u8>,
        
        /// Optional metadata
        metadata: Option<HashMap<String, String>>,
    },
    
    /// Mutate an existing object
    MutateObject {
        /// Object ID to mutate
        id: ObjectId,
        
        /// New contents
        new_contents: Vec<u8>,
    },
    
    /// Transfer ownership of an object
    TransferObject {
        /// Object ID to transfer
        id: ObjectId,
        
        /// New owner
        new_owner: Ownership,
    },
    
    /// Delete an object
    DeleteObject {
        /// Object ID to delete
        id: ObjectId,
    },
}

/// Object transaction record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectTransactionRecord {
    /// Unique transaction ID (hash)
    pub tx_id: Hash,
    
    /// Sender address
    pub sender: Address,
    
    /// Transaction kind
    pub kind: ObjectTransactionKind,
    
    /// Gas price (fee per gas unit)
    pub gas_price: u64,
    
    /// Gas limit (maximum gas units)
    pub gas_limit: u64,
    
    /// Account nonce (prevents replay attacks)
    pub nonce: u64,
    
    /// Timestamp when the transaction was created
    pub timestamp: u64,
    
    /// Transaction signature
    pub signature: VibeSignature,
}

impl ObjectTransactionRecord {
    /// Create a new object transaction record
    pub fn new(
        sender: Address,
        kind: ObjectTransactionKind,
        gas_price: u64,
        gas_limit: u64,
        nonce: u64,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Create a placeholder signature
        let signature = VibeSignature::new([0u8; 64]);
        
        // Create the transaction without a valid tx_id
        let mut tx = Self {
            tx_id: [0u8; 32],
            sender,
            kind,
            gas_price,
            gas_limit,
            nonce,
            timestamp,
            signature,
        };
        
        // Compute the transaction ID based on the contents
        tx.tx_id = tx.compute_tx_id();
        
        tx
    }
    
    /// Serialize the transaction for signing (excluding the signature)
    pub fn serialize_for_signing(&self) -> Vec<u8> {
        // Create a canonical representation for signing
        let mut data = Vec::new();
        
        // Add sender
        data.extend_from_slice(&self.sender);
        
        // Add transaction kind
        let kind_bytes = bincode::serialize(&self.kind).unwrap();
        data.extend_from_slice(&kind_bytes);
        
        // Add other transaction fields
        data.extend_from_slice(&self.gas_price.to_be_bytes());
        data.extend_from_slice(&self.gas_limit.to_be_bytes());
        data.extend_from_slice(&self.nonce.to_be_bytes());
        data.extend_from_slice(&self.timestamp.to_be_bytes());
        
        data
    }
    
    /// Compute the transaction ID (hash of all fields except signature)
    pub fn compute_tx_id(&self) -> Hash {
        use crate::crypto::hash::sha256;
        
        // Get the serialized transaction data
        let data = self.serialize_for_signing();
        
        // Hash the data to create the transaction ID
        sha256(&data)
    }
    
    /// Sign the transaction with the given keypair
    pub fn sign(&mut self, keypair: &VibeKeypair) {
        // Make sure the transaction ID is computed
        self.tx_id = self.compute_tx_id();
        
        // Get the data to sign (serialized transaction)
        let data = self.serialize_for_signing();
        
        // Sign the data
        self.signature = sign_message(keypair, &data);
    }
    
    /// Create a signed transaction
    pub fn create_signed(
        keypair: &VibeKeypair,
        kind: ObjectTransactionKind,
        gas_price: u64,
        gas_limit: u64,
        nonce: u64,
    ) -> Self {
        // Get the sender's address
        let sender = keypair.address();
        
        // Create an unsigned transaction
        let mut tx = Self::new(
            sender,
            kind,
            gas_price,
            gas_limit,
            nonce,
        );
        
        // Sign the transaction
        tx.sign(keypair);
        
        tx
    }
    
    /// Get the total gas cost (gas_price * gas_limit)
    pub fn gas_cost(&self) -> u64 {
        self.gas_price * self.gas_limit
    }
    
    /// Check if the transaction is expired
    pub fn is_expired(&self, max_age_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.timestamp > max_age_secs
    }
    
    /// Get a description of the transaction
    pub fn description(&self) -> String {
        match &self.kind {
            ObjectTransactionKind::CreateObject { type_tag, owner, .. } => {
                let owner_str = match owner {
                    Ownership::Address(addr) => format!("address {}", hex::encode(addr)),
                    Ownership::Shared => "shared".to_string(),
                    Ownership::Immutable => "immutable".to_string(),
                };
                format!("Create {} object owned by {}", type_tag, owner_str)
            },
            ObjectTransactionKind::MutateObject { id, .. } => {
                format!("Mutate object {}", hex::encode(id))
            },
            ObjectTransactionKind::TransferObject { id, new_owner } => {
                let owner_str = match new_owner {
                    Ownership::Address(addr) => format!("address {}", hex::encode(addr)),
                    Ownership::Shared => "shared".to_string(),
                    Ownership::Immutable => "immutable".to_string(),
                };
                format!("Transfer object {} to {}", hex::encode(id), owner_str)
            },
            ObjectTransactionKind::DeleteObject { id } => {
                format!("Delete object {}", hex::encode(id))
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::VibeKeypair;
    
    #[test]
    fn test_create_object_transaction() {
        let keypair = VibeKeypair::generate();
        let sender = keypair.address();
        
        let kind = ObjectTransactionKind::CreateObject {
            type_tag: "Coin".to_string(),
            owner: Ownership::Address(sender),
            contents: vec![1, 2, 3, 4],
            metadata: None,
        };
        
        let tx = ObjectTransactionRecord::new(
            sender,
            kind,
            5,
            21000,
            1,
        );
        
        assert_eq!(tx.sender, sender);
        assert_eq!(tx.gas_price, 5);
        assert_eq!(tx.gas_limit, 21000);
        assert_eq!(tx.nonce, 1);
        assert_eq!(tx.gas_cost(), 105000);
        
        // Test description
        assert!(tx.description().contains("Create Coin object"));
    }
    
    #[test]
    fn test_mutate_object_transaction() {
        let keypair = VibeKeypair::generate();
        let sender = keypair.address();
        let object_id = [1; 32];
        
        let kind = ObjectTransactionKind::MutateObject {
            id: object_id,
            new_contents: vec![5, 6, 7, 8],
        };
        
        let tx = ObjectTransactionRecord::new(
            sender,
            kind,
            5,
            21000,
            1,
        );
        
        // Test description
        assert!(tx.description().contains("Mutate object"));
    }
    
    #[test]
    fn test_transfer_object_transaction() {
        let keypair = VibeKeypair::generate();
        let sender = keypair.address();
        let recipient = [2; 32];
        let object_id = [1; 32];
        
        let kind = ObjectTransactionKind::TransferObject {
            id: object_id,
            new_owner: Ownership::Address(recipient),
        };
        
        let tx = ObjectTransactionRecord::new(
            sender,
            kind,
            5,
            21000,
            1,
        );
        
        // Test description
        assert!(tx.description().contains("Transfer object"));
    }
    
    #[test]
    fn test_delete_object_transaction() {
        let keypair = VibeKeypair::generate();
        let sender = keypair.address();
        let object_id = [1; 32];
        
        let kind = ObjectTransactionKind::DeleteObject {
            id: object_id,
        };
        
        let tx = ObjectTransactionRecord::new(
            sender,
            kind,
            5,
            21000,
            1,
        );
        
        // Test description
        assert!(tx.description().contains("Delete object"));
    }
    
    #[test]
    fn test_sign_transaction() {
        let keypair = VibeKeypair::generate();
        let sender = keypair.address();
        let object_id = [1; 32];
        
        let kind = ObjectTransactionKind::MutateObject {
            id: object_id,
            new_contents: vec![5, 6, 7, 8],
        };
        
        let mut tx = ObjectTransactionRecord::new(
            sender,
            kind,
            5,
            21000,
            1,
        );
        
        // Sign the transaction
        tx.sign(&keypair);
        
        // Verify that the signature is not zero
        assert_ne!(tx.signature.as_bytes(), [0u8; 64]);
        
        // Create a signed transaction directly
        let signed_tx = ObjectTransactionRecord::create_signed(
            &keypair,
            ObjectTransactionKind::DeleteObject {
                id: object_id,
            },
            5,
            21000,
            2,
        );
        
        assert_eq!(signed_tx.sender, sender);
        assert_eq!(signed_tx.nonce, 2);
        assert_ne!(signed_tx.signature.as_bytes(), [0u8; 64]);
    }
}
