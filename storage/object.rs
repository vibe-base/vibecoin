//! Object storage model for VibeCoin blockchain
//!
//! This module implements a Sui-style object ID system, extending the account-based
//! storage model into a more object-centric architecture. It provides the core
//! structures and functionality for creating, storing, and managing objects.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use thiserror::Error;
use crate::crypto::hash::sha256;

/// Type alias for ObjectId (32-byte hash)
pub type ObjectId = [u8; 32];

/// Ownership type for objects
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ownership {
    /// Owned by a specific address
    Address([u8; 32]),
    
    /// Shared object (can be accessed by anyone)
    Shared,
    
    /// Immutable object (cannot be modified)
    Immutable,
}

/// Object structure representing a Sui-style object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Object {
    /// Unique object identifier
    pub id: ObjectId,
    
    /// Who owns this object
    pub owner: Ownership,
    
    /// Version number, incremented on mutation
    pub version: u64,
    
    /// Type tag (e.g., "Coin", "NFT", etc.)
    pub type_tag: String,
    
    /// Serialized contents of the object
    pub contents: Vec<u8>,
    
    /// Creation timestamp
    pub created_at: u64,
    
    /// Last updated timestamp
    pub updated_at: u64,
    
    /// Block height when this object was last updated
    pub last_updated_block: u64,
    
    /// Additional metadata as key-value pairs
    pub metadata: HashMap<String, String>,
}

/// Error type for object operations
#[derive(Debug, Error)]
pub enum ObjectError {
    /// Object not found
    #[error("Object not found: {0}")]
    ObjectNotFound(String),
    
    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    
    /// Invalid ownership
    #[error("Invalid ownership: {0}")]
    InvalidOwnership(String),
    
    /// Version mismatch
    #[error("Version mismatch: expected {0}, got {1}")]
    VersionMismatch(u64, u64),
    
    /// Immutable object modification attempt
    #[error("Cannot modify immutable object: {0}")]
    ImmutableObjectModification(String),
    
    /// Unauthorized access
    #[error("Unauthorized access to object: {0}")]
    UnauthorizedAccess(String),
    
    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),
    
    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for object operations
pub type ObjectResult<T> = Result<T, ObjectError>;

impl Object {
    /// Create a new object with the given parameters
    pub fn new(
        owner: Ownership,
        type_tag: String,
        contents: Vec<u8>,
        metadata: Option<HashMap<String, String>>,
        timestamp: u64,
        block_height: u64,
    ) -> Self {
        // Generate a unique ID for the object
        let mut id_input = Vec::new();
        
        // Add timestamp to ensure uniqueness
        id_input.extend_from_slice(&timestamp.to_be_bytes());
        
        // Add type tag
        id_input.extend_from_slice(type_tag.as_bytes());
        
        // Add contents
        id_input.extend_from_slice(&contents);
        
        // Add owner information
        match &owner {
            Ownership::Address(addr) => id_input.extend_from_slice(addr),
            Ownership::Shared => id_input.extend_from_slice(b"shared"),
            Ownership::Immutable => id_input.extend_from_slice(b"immutable"),
        }
        
        // Hash to create the ID
        let id = sha256(&id_input);
        
        Self {
            id,
            owner,
            version: 0,
            type_tag,
            contents,
            created_at: timestamp,
            updated_at: timestamp,
            last_updated_block: block_height,
            metadata: metadata.unwrap_or_default(),
        }
    }
    
    /// Check if the object is owned by the given address
    pub fn is_owned_by(&self, address: &[u8; 32]) -> bool {
        match &self.owner {
            Ownership::Address(owner) => owner == address,
            _ => false,
        }
    }
    
    /// Check if the object is shared
    pub fn is_shared(&self) -> bool {
        matches!(self.owner, Ownership::Shared)
    }
    
    /// Check if the object is immutable
    pub fn is_immutable(&self) -> bool {
        matches!(self.owner, Ownership::Immutable)
    }
    
    /// Update the object contents
    pub fn update_contents(
        &mut self,
        new_contents: Vec<u8>,
        timestamp: u64,
        block_height: u64,
    ) -> ObjectResult<()> {
        if self.is_immutable() {
            return Err(ObjectError::ImmutableObjectModification(
                hex::encode(self.id)
            ));
        }
        
        self.contents = new_contents;
        self.version += 1;
        self.updated_at = timestamp;
        self.last_updated_block = block_height;
        
        Ok(())
    }
    
    /// Transfer ownership of the object
    pub fn transfer_ownership(
        &mut self,
        new_owner: Ownership,
        timestamp: u64,
        block_height: u64,
    ) -> ObjectResult<()> {
        if self.is_immutable() {
            return Err(ObjectError::ImmutableObjectModification(
                hex::encode(self.id)
            ));
        }
        
        self.owner = new_owner;
        self.version += 1;
        self.updated_at = timestamp;
        self.last_updated_block = block_height;
        
        Ok(())
    }
    
    /// Add or update metadata
    pub fn set_metadata(
        &mut self,
        key: String,
        value: String,
        timestamp: u64,
        block_height: u64,
    ) -> ObjectResult<()> {
        if self.is_immutable() {
            return Err(ObjectError::ImmutableObjectModification(
                hex::encode(self.id)
            ));
        }
        
        self.metadata.insert(key, value);
        self.version += 1;
        self.updated_at = timestamp;
        self.last_updated_block = block_height;
        
        Ok(())
    }
    
    /// Get object ID as hex string
    pub fn id_hex(&self) -> String {
        hex::encode(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    
    fn get_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    
    #[test]
    fn test_object_creation() {
        let timestamp = get_timestamp();
        let owner = Ownership::Address([1; 32]);
        let type_tag = "Coin".to_string();
        let contents = vec![1, 2, 3, 4];
        
        let object = Object::new(
            owner.clone(),
            type_tag.clone(),
            contents.clone(),
            None,
            timestamp,
            100,
        );
        
        assert_eq!(object.version, 0);
        assert_eq!(object.type_tag, type_tag);
        assert_eq!(object.contents, contents);
        assert_eq!(object.owner, owner);
        assert_eq!(object.created_at, timestamp);
        assert_eq!(object.updated_at, timestamp);
        assert_eq!(object.last_updated_block, 100);
        assert!(object.metadata.is_empty());
    }
    
    #[test]
    fn test_object_ownership() {
        let timestamp = get_timestamp();
        let address = [1; 32];
        let other_address = [2; 32];
        
        let object = Object::new(
            Ownership::Address(address),
            "Coin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        assert!(object.is_owned_by(&address));
        assert!(!object.is_owned_by(&other_address));
        assert!(!object.is_shared());
        assert!(!object.is_immutable());
        
        let shared_object = Object::new(
            Ownership::Shared,
            "SharedCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        assert!(shared_object.is_shared());
        assert!(!shared_object.is_owned_by(&address));
        assert!(!shared_object.is_immutable());
        
        let immutable_object = Object::new(
            Ownership::Immutable,
            "ImmutableCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        assert!(immutable_object.is_immutable());
        assert!(!immutable_object.is_shared());
        assert!(!immutable_object.is_owned_by(&address));
    }
    
    #[test]
    fn test_object_update() {
        let timestamp = get_timestamp();
        let address = [1; 32];
        
        let mut object = Object::new(
            Ownership::Address(address),
            "Coin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        let new_timestamp = timestamp + 100;
        let new_contents = vec![5, 6, 7, 8];
        
        object.update_contents(new_contents.clone(), new_timestamp, 101).unwrap();
        
        assert_eq!(object.version, 1);
        assert_eq!(object.contents, new_contents);
        assert_eq!(object.updated_at, new_timestamp);
        assert_eq!(object.last_updated_block, 101);
        
        // Test immutable object update
        let mut immutable_object = Object::new(
            Ownership::Immutable,
            "ImmutableCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        let result = immutable_object.update_contents(vec![5, 6, 7, 8], new_timestamp, 101);
        assert!(result.is_err());
        match result {
            Err(ObjectError::ImmutableObjectModification(_)) => {},
            _ => panic!("Expected ImmutableObjectModification error"),
        }
    }
    
    #[test]
    fn test_object_transfer() {
        let timestamp = get_timestamp();
        let address1 = [1; 32];
        let address2 = [2; 32];
        
        let mut object = Object::new(
            Ownership::Address(address1),
            "Coin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        let new_timestamp = timestamp + 100;
        let new_owner = Ownership::Address(address2);
        
        object.transfer_ownership(new_owner.clone(), new_timestamp, 101).unwrap();
        
        assert_eq!(object.version, 1);
        assert_eq!(object.owner, new_owner);
        assert_eq!(object.updated_at, new_timestamp);
        assert_eq!(object.last_updated_block, 101);
        assert!(object.is_owned_by(&address2));
        assert!(!object.is_owned_by(&address1));
        
        // Test immutable object transfer
        let mut immutable_object = Object::new(
            Ownership::Immutable,
            "ImmutableCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        let result = immutable_object.transfer_ownership(
            Ownership::Address(address1),
            new_timestamp,
            101
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_object_metadata() {
        let timestamp = get_timestamp();
        let address = [1; 32];
        
        let mut object = Object::new(
            Ownership::Address(address),
            "Coin".to_string(),
            vec![1, 2, 3, 4],
            None,
            timestamp,
            100,
        );
        
        let new_timestamp = timestamp + 100;
        
        object.set_metadata(
            "name".to_string(),
            "My Coin".to_string(),
            new_timestamp,
            101
        ).unwrap();
        
        assert_eq!(object.version, 1);
        assert_eq!(object.metadata.get("name"), Some(&"My Coin".to_string()));
        assert_eq!(object.updated_at, new_timestamp);
        assert_eq!(object.last_updated_block, 101);
    }
}
