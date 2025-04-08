//! Object store for VibeCoin blockchain
//!
//! This module provides storage functionality for Sui-style objects,
//! including creating, retrieving, updating, and querying objects.

use log::{debug, info};
use thiserror::Error;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::storage::kv_store::{KVStore, KVStoreError, WriteBatchOperation};
use crate::storage::object::{Object, ObjectId, Ownership, ObjectError, ObjectResult};

/// Error type for ObjectStore operations
#[derive(Debug, Error)]
pub enum ObjectStoreError {
    /// KVStore error
    #[error("KVStore error: {0}")]
    KVStoreError(#[from] KVStoreError),

    /// Object error
    #[error("Object error: {0}")]
    ObjectError(#[from] ObjectError),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// Object not found
    #[error("Object not found: {0}")]
    ObjectNotFound(String),

    /// Object already exists
    #[error("Object already exists: {0}")]
    ObjectAlreadyExists(String),

    /// Invalid object ID
    #[error("Invalid object ID: {0}")]
    InvalidObjectId(String),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Result type for ObjectStore operations
pub type ObjectStoreResult<T> = Result<T, ObjectStoreError>;

/// Store for objects
pub struct ObjectStore<'a> {
    /// The underlying key-value store
    store: &'a dyn KVStore,
}

impl<'a> ObjectStore<'a> {
    /// Create a new ObjectStore with the given KVStore implementation
    pub fn new(store: &'a dyn KVStore) -> Self {
        Self { store }
    }

    /// Store an object
    pub fn put_object(&self, object: &Object) -> ObjectStoreResult<()> {
        let object_id_hex = hex::encode(&object.id);
        let key = format!("objects:{}", object_id_hex);

        // Check if the object already exists
        if let Some(existing) = self.get_object(&object.id)? {
            // Ensure version is incremented correctly
            if object.version != existing.version + 1 && object.version != existing.version {
                return Err(ObjectStoreError::ObjectError(
                    ObjectError::VersionMismatch(existing.version + 1, object.version)
                ));
            }
        }

        // Serialize the object
        let value = bincode::serialize(object)
            .map_err(|e| ObjectStoreError::SerializationError(e.to_string()))?;

        // Store the object
        self.store.put(key.as_bytes(), &value)?;

        // Create secondary indices
        self.create_indices(object)?;

        debug!("Stored object {} (version {})", object_id_hex, object.version);
        Ok(())
    }

    /// Get an object by ID
    pub fn get_object(&self, id: &ObjectId) -> ObjectStoreResult<Option<Object>> {
        let object_id_hex = hex::encode(id);
        let key = format!("objects:{}", object_id_hex);

        match self.store.get(key.as_bytes())? {
            Some(value) => {
                let object = bincode::deserialize(&value)
                    .map_err(|e| ObjectStoreError::DeserializationError(e.to_string()))?;
                Ok(Some(object))
            },
            None => Ok(None),
        }
    }

    /// Delete an object
    pub fn delete_object(&self, id: &ObjectId) -> ObjectStoreResult<()> {
        let object_id_hex = hex::encode(id);
        let key = format!("objects:{}", object_id_hex);

        // Check if the object exists
        if let Some(object) = self.get_object(id)? {
            // Remove the object
            self.store.delete(key.as_bytes())?;

            // Remove indices
            self.remove_indices(&object)?;

            debug!("Deleted object {}", object_id_hex);
            Ok(())
        } else {
            Err(ObjectStoreError::ObjectNotFound(object_id_hex))
        }
    }

    /// Update an object
    pub fn update_object(
        &self,
        id: &ObjectId,
        updater: impl FnOnce(&mut Object) -> ObjectResult<()>,
    ) -> ObjectStoreResult<Object> {
        let object_id_hex = hex::encode(id);

        // Get the current object
        let mut object = match self.get_object(id)? {
            Some(obj) => obj,
            None => return Err(ObjectStoreError::ObjectNotFound(object_id_hex)),
        };

        // Remove old indices
        self.remove_indices(&object)?;

        // Apply the update
        updater(&mut object)?;

        // Store the updated object
        self.put_object(&object)?;

        debug!("Updated object {} (version {})", object_id_hex, object.version);
        Ok(object)
    }

    /// Create a new object
    pub fn create_object(
        &self,
        owner: Ownership,
        type_tag: String,
        contents: Vec<u8>,
        metadata: Option<std::collections::HashMap<String, String>>,
    ) -> ObjectStoreResult<Object> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create the object
        let object = Object::new(
            owner,
            type_tag,
            contents,
            metadata,
            timestamp,
            0, // Block height will be updated when included in a block
        );

        // Store the object
        self.put_object(&object)?;

        info!("Created new object {} (type: {})", object.id_hex(), object.type_tag);
        Ok(object)
    }

    /// Get objects owned by an address
    pub fn get_objects_by_owner(&self, owner: &[u8; 32]) -> ObjectStoreResult<Vec<Object>> {
        let owner_hex = hex::encode(owner);
        let prefix = format!("objects_by_owner:{}", owner_hex);

        let results = self.store.scan_prefix(prefix.as_bytes())?;
        let mut objects = Vec::new();

        for (_, value) in results {
            let object_id: ObjectId = bincode::deserialize(&value)
                .map_err(|e| ObjectStoreError::DeserializationError(e.to_string()))?;

            if let Some(object) = self.get_object(&object_id)? {
                objects.push(object);
            }
        }

        Ok(objects)
    }

    /// Get objects by type
    pub fn get_objects_by_type(&self, type_tag: &str) -> ObjectStoreResult<Vec<Object>> {
        let prefix = format!("objects_by_type:{}", type_tag);

        let results = self.store.scan_prefix(prefix.as_bytes())?;
        let mut objects = Vec::new();

        for (_, value) in results {
            let object_id: ObjectId = bincode::deserialize(&value)
                .map_err(|e| ObjectStoreError::DeserializationError(e.to_string()))?;

            if let Some(object) = self.get_object(&object_id)? {
                objects.push(object);
            }
        }

        Ok(objects)
    }

    /// Check if an object exists
    pub fn object_exists(&self, id: &ObjectId) -> ObjectStoreResult<bool> {
        let object_id_hex = hex::encode(id);
        let key = format!("objects:{}", object_id_hex);

        Ok(self.store.exists(key.as_bytes())?)
    }

    /// Create secondary indices for an object
    fn create_indices(&self, object: &Object) -> ObjectStoreResult<()> {
        let mut batch = Vec::new();

        // Index by owner (if owned by an address)
        if let Ownership::Address(owner) = object.owner {
            let owner_hex = hex::encode(&owner);
            let key = format!("objects_by_owner:{}:{}", owner_hex, hex::encode(&object.id));
            let value = bincode::serialize(&object.id)
                .map_err(|e| ObjectStoreError::SerializationError(e.to_string()))?;

            batch.push(WriteBatchOperation::Put {
                key: key.as_bytes().to_vec(),
                value,
            });
        }

        // Index by type
        let key = format!("objects_by_type:{}:{}", object.type_tag, hex::encode(&object.id));
        let value = bincode::serialize(&object.id)
            .map_err(|e| ObjectStoreError::SerializationError(e.to_string()))?;

        batch.push(WriteBatchOperation::Put {
            key: key.as_bytes().to_vec(),
            value,
        });

        // Execute the batch
        self.store.write_batch(batch)?;

        Ok(())
    }

    /// Remove secondary indices for an object
    fn remove_indices(&self, object: &Object) -> ObjectStoreResult<()> {
        let mut batch = Vec::new();

        // Remove owner index
        if let Ownership::Address(owner) = object.owner {
            let owner_hex = hex::encode(&owner);
            let key = format!("objects_by_owner:{}:{}", owner_hex, hex::encode(&object.id));

            batch.push(WriteBatchOperation::Delete {
                key: key.as_bytes().to_vec(),
            });
        }

        // Remove type index
        let key = format!("objects_by_type:{}:{}", object.type_tag, hex::encode(&object.id));

        batch.push(WriteBatchOperation::Delete {
            key: key.as_bytes().to_vec(),
        });

        // Execute the batch
        self.store.write_batch(batch)?;

        Ok(())
    }

    /// Get all objects (use with caution - can be expensive)
    pub fn get_all_objects(&self) -> ObjectStoreResult<Vec<Object>> {
        let prefix = "objects:";

        let results = self.store.scan_prefix(prefix.as_bytes())?;
        let mut objects = Vec::new();

        for (_, value) in results {
            let object: Object = bincode::deserialize(&value)
                .map_err(|e| ObjectStoreError::DeserializationError(e.to_string()))?;

            objects.push(object);
        }

        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    use std::collections::HashMap;

    fn create_test_object_store() -> (tempfile::TempDir, ObjectStore<'static>) {
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
        let kv_store_static = Box::leak(Box::new(kv_store));
        let object_store = ObjectStore::new(kv_store_static);

        (temp_dir, object_store)
    }

    #[test]
    fn test_object_store_basic() {
        let (_temp_dir, object_store) = create_test_object_store();

        // Create a test object
        let owner = Ownership::Address([1; 32]);
        let object = object_store.create_object(
            owner,
            "TestCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Retrieve the object
        let retrieved = object_store.get_object(&object.id).unwrap().unwrap();
        assert_eq!(retrieved.id, object.id);
        assert_eq!(retrieved.type_tag, "TestCoin");
        assert_eq!(retrieved.contents, vec![1, 2, 3, 4]);

        // Check existence
        assert!(object_store.object_exists(&object.id).unwrap());
        assert!(!object_store.object_exists(&[0; 32]).unwrap());

        // Update the object
        let updated = object_store.update_object(&object.id, |obj| {
            obj.update_contents(vec![5, 6, 7, 8], obj.updated_at + 1, 1)
        }).unwrap();

        assert_eq!(updated.version, 1);
        assert_eq!(updated.contents, vec![5, 6, 7, 8]);

        // Delete the object
        object_store.delete_object(&object.id).unwrap();
        assert!(!object_store.object_exists(&object.id).unwrap());
    }

    #[test]
    fn test_object_store_queries() {
        let (_temp_dir, object_store) = create_test_object_store();

        let owner1 = [1; 32];
        let owner2 = [2; 32];

        // Create multiple objects
        let coin1 = object_store.create_object(
            Ownership::Address(owner1),
            "Coin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        let coin2 = object_store.create_object(
            Ownership::Address(owner1),
            "Coin".to_string(),
            vec![5, 6, 7, 8],
            None,
        ).unwrap();

        let nft = object_store.create_object(
            Ownership::Address(owner2),
            "NFT".to_string(),
            vec![9, 10, 11, 12],
            None,
        ).unwrap();

        // Query by owner
        let owner1_objects = object_store.get_objects_by_owner(&owner1).unwrap();
        assert_eq!(owner1_objects.len(), 2);
        assert!(owner1_objects.iter().any(|obj| obj.id == coin1.id));
        assert!(owner1_objects.iter().any(|obj| obj.id == coin2.id));

        let owner2_objects = object_store.get_objects_by_owner(&owner2).unwrap();
        assert_eq!(owner2_objects.len(), 1);
        assert_eq!(owner2_objects[0].id, nft.id);

        // Query by type
        let coins = object_store.get_objects_by_type("Coin").unwrap();
        assert_eq!(coins.len(), 2);

        let nfts = object_store.get_objects_by_type("NFT").unwrap();
        assert_eq!(nfts.len(), 1);
        assert_eq!(nfts[0].id, nft.id);

        // Get all objects
        let all_objects = object_store.get_all_objects().unwrap();
        assert_eq!(all_objects.len(), 3);
    }

    #[test]
    fn test_object_store_ownership() {
        let (_temp_dir, object_store) = create_test_object_store();

        let owner1 = [1; 32];
        let owner2 = [2; 32];

        // Create an owned object
        let object = object_store.create_object(
            Ownership::Address(owner1),
            "Coin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Transfer ownership
        let updated = object_store.update_object(&object.id, |obj| {
            obj.transfer_ownership(Ownership::Address(owner2), obj.updated_at + 1, 1)
        }).unwrap();

        assert_eq!(updated.version, 1);
        match updated.owner {
            Ownership::Address(addr) => assert_eq!(addr, owner2),
            _ => panic!("Expected Address ownership"),
        }

        // Check that indices are updated
        let owner1_objects = object_store.get_objects_by_owner(&owner1).unwrap();
        assert_eq!(owner1_objects.len(), 0);

        let owner2_objects = object_store.get_objects_by_owner(&owner2).unwrap();
        assert_eq!(owner2_objects.len(), 1);
        assert_eq!(owner2_objects[0].id, object.id);
    }

    #[test]
    fn test_object_store_immutable() {
        let (_temp_dir, object_store) = create_test_object_store();

        // Create an immutable object
        let object = object_store.create_object(
            Ownership::Immutable,
            "ImmutableCoin".to_string(),
            vec![1, 2, 3, 4],
            None,
        ).unwrap();

        // Try to update it (should fail)
        let result = object_store.update_object(&object.id, |obj| {
            obj.update_contents(vec![5, 6, 7, 8], obj.updated_at + 1, 1)
        });

        assert!(result.is_err());
        match result {
            Err(ObjectStoreError::ObjectError(ObjectError::ImmutableObjectModification(_))) => {},
            _ => panic!("Expected ImmutableObjectModification error"),
        }
    }

    #[test]
    fn test_object_store_metadata() {
        let (_temp_dir, object_store) = create_test_object_store();

        // Create an object with metadata
        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), "Test Coin".to_string());
        metadata.insert("symbol".to_string(), "TST".to_string());

        let object = object_store.create_object(
            Ownership::Address([1; 32]),
            "Coin".to_string(),
            vec![1, 2, 3, 4],
            Some(metadata),
        ).unwrap();

        // Check metadata
        assert_eq!(object.metadata.get("name"), Some(&"Test Coin".to_string()));
        assert_eq!(object.metadata.get("symbol"), Some(&"TST".to_string()));

        // Update metadata
        let updated = object_store.update_object(&object.id, |obj| {
            obj.set_metadata(
                "description".to_string(),
                "A test coin".to_string(),
                obj.updated_at + 1,
                1
            )
        }).unwrap();

        assert_eq!(updated.version, 1);
        assert_eq!(updated.metadata.get("name"), Some(&"Test Coin".to_string()));
        assert_eq!(updated.metadata.get("symbol"), Some(&"TST".to_string()));
        assert_eq!(updated.metadata.get("description"), Some(&"A test coin".to_string()));
    }
}
