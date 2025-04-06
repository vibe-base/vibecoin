use std::collections::HashMap;
use std::sync::Arc;
use sha2::{Sha256, Digest};
use log::{debug, error, info, warn};

use crate::storage::block_store::Hash;

/// Node types in the Merkle Patricia Trie
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// Leaf node containing a value
    Leaf {
        /// Key suffix (remaining part of the key)
        key_suffix: Vec<u8>,
        /// Value stored at this leaf
        value: Vec<u8>,
    },
    
    /// Branch node with up to 16 children
    Branch {
        /// Children nodes (one for each hex digit)
        children: [Option<Arc<Node>>; 16],
        /// Value stored at this branch (if any)
        value: Option<Vec<u8>>,
    },
    
    /// Extension node with a shared prefix
    Extension {
        /// Shared prefix
        prefix: Vec<u8>,
        /// Next node
        next: Arc<Node>,
    },
    
    /// Empty node (null)
    Empty,
}

impl Node {
    /// Calculate the hash of this node
    pub fn hash(&self) -> Hash {
        match self {
            Node::Leaf { key_suffix, value } => {
                let mut hasher = Sha256::new();
                hasher.update(b"leaf");
                hasher.update(key_suffix);
                hasher.update(value);
                hasher.finalize().into()
            },
            Node::Branch { children, value } => {
                let mut hasher = Sha256::new();
                hasher.update(b"branch");
                
                for child in children {
                    if let Some(node) = child {
                        hasher.update(node.hash());
                    } else {
                        hasher.update([0u8; 32]);
                    }
                }
                
                if let Some(v) = value {
                    hasher.update(v);
                }
                
                hasher.finalize().into()
            },
            Node::Extension { prefix, next } => {
                let mut hasher = Sha256::new();
                hasher.update(b"extension");
                hasher.update(prefix);
                hasher.update(next.hash());
                hasher.finalize().into()
            },
            Node::Empty => {
                // Empty node has a special hash
                let mut hasher = Sha256::new();
                hasher.update(b"empty");
                hasher.finalize().into()
            },
        }
    }
}

/// Merkle Patricia Trie implementation
pub struct MerklePatriciaTrie {
    /// Root node of the trie
    root: Arc<Node>,
}

impl MerklePatriciaTrie {
    /// Create a new empty trie
    pub fn new() -> Self {
        Self {
            root: Arc::new(Node::Empty),
        }
    }
    
    /// Get the root hash of the trie
    pub fn root_hash(&self) -> Hash {
        self.root.hash()
    }
    
    /// Get a value from the trie
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.get_node(&self.root, &self.encode_key(key), 0)
    }
    
    /// Helper function to get a value from a node
    fn get_node(&self, node: &Arc<Node>, encoded_key: &[u8], pos: usize) -> Option<Vec<u8>> {
        if pos >= encoded_key.len() {
            return None;
        }
        
        match &**node {
            Node::Leaf { key_suffix, value } => {
                // Check if the remaining key matches the suffix
                if &encoded_key[pos..] == key_suffix {
                    Some(value.clone())
                } else {
                    None
                }
            },
            Node::Branch { children, value } => {
                if pos == encoded_key.len() {
                    // We've reached the end of the key, return the value at this branch
                    value.clone()
                } else {
                    // Get the next nibble and follow the corresponding child
                    let nibble = encoded_key[pos] as usize;
                    if let Some(child) = &children[nibble] {
                        self.get_node(child, encoded_key, pos + 1)
                    } else {
                        None
                    }
                }
            },
            Node::Extension { prefix, next } => {
                // Check if the key matches the prefix
                if encoded_key[pos..].starts_with(prefix) {
                    self.get_node(next, encoded_key, pos + prefix.len())
                } else {
                    None
                }
            },
            Node::Empty => None,
        }
    }
    
    /// Insert a key-value pair into the trie
    pub fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        let encoded_key = self.encode_key(key);
        let new_root = self.insert_node(self.root.clone(), &encoded_key, 0, value);
        self.root = new_root;
    }
    
    /// Helper function to insert a key-value pair into a node
    fn insert_node(&self, node: Arc<Node>, encoded_key: &[u8], pos: usize, value: Vec<u8>) -> Arc<Node> {
        match &*node {
            Node::Empty => {
                // Create a new leaf node
                Arc::new(Node::Leaf {
                    key_suffix: encoded_key[pos..].to_vec(),
                    value,
                })
            },
            Node::Leaf { key_suffix, value: old_value } => {
                // Find the common prefix between the current key suffix and the new key
                let common_prefix_len = self.common_prefix_len(&encoded_key[pos..], key_suffix);
                
                if common_prefix_len == key_suffix.len() && common_prefix_len == encoded_key.len() - pos {
                    // The keys are identical, just update the value
                    Arc::new(Node::Leaf {
                        key_suffix: key_suffix.clone(),
                        value,
                    })
                } else if common_prefix_len == key_suffix.len() {
                    // The new key contains the current key as a prefix
                    // Create a branch node
                    let mut children = [None; 16];
                    let next_pos = pos + common_prefix_len;
                    let next_nibble = encoded_key[next_pos] as usize;
                    
                    children[next_nibble] = Some(self.insert_node(
                        Arc::new(Node::Empty),
                        encoded_key,
                        next_pos + 1,
                        value,
                    ));
                    
                    Arc::new(Node::Branch {
                        children,
                        value: Some(old_value.clone()),
                    })
                } else if common_prefix_len == encoded_key.len() - pos {
                    // The current key contains the new key as a prefix
                    // Create a branch node
                    let mut children = [None; 16];
                    let next_nibble = key_suffix[common_prefix_len] as usize;
                    
                    children[next_nibble] = Some(Arc::new(Node::Leaf {
                        key_suffix: key_suffix[common_prefix_len + 1..].to_vec(),
                        value: old_value.clone(),
                    }));
                    
                    Arc::new(Node::Branch {
                        children,
                        value: Some(value),
                    })
                } else {
                    // The keys share a common prefix but diverge
                    // Create an extension node followed by a branch
                    let mut children = [None; 16];
                    
                    let next_nibble_old = key_suffix[common_prefix_len] as usize;
                    let next_nibble_new = encoded_key[pos + common_prefix_len] as usize;
                    
                    children[next_nibble_old] = Some(Arc::new(Node::Leaf {
                        key_suffix: key_suffix[common_prefix_len + 1..].to_vec(),
                        value: old_value.clone(),
                    }));
                    
                    children[next_nibble_new] = Some(self.insert_node(
                        Arc::new(Node::Empty),
                        encoded_key,
                        pos + common_prefix_len + 1,
                        value,
                    ));
                    
                    if common_prefix_len > 0 {
                        Arc::new(Node::Extension {
                            prefix: encoded_key[pos..pos + common_prefix_len].to_vec(),
                            next: Arc::new(Node::Branch {
                                children,
                                value: None,
                            }),
                        })
                    } else {
                        Arc::new(Node::Branch {
                            children,
                            value: None,
                        })
                    }
                }
            },
            Node::Branch { children, value: branch_value } => {
                if pos == encoded_key.len() {
                    // We've reached the end of the key, update the value at this branch
                    let mut new_children = children.clone();
                    Arc::new(Node::Branch {
                        children: new_children,
                        value: Some(value),
                    })
                } else {
                    // Get the next nibble and follow the corresponding child
                    let nibble = encoded_key[pos] as usize;
                    let mut new_children = children.clone();
                    
                    new_children[nibble] = Some(self.insert_node(
                        children[nibble].clone().unwrap_or_else(|| Arc::new(Node::Empty)),
                        encoded_key,
                        pos + 1,
                        value,
                    ));
                    
                    Arc::new(Node::Branch {
                        children: new_children,
                        value: branch_value.clone(),
                    })
                }
            },
            Node::Extension { prefix, next } => {
                let common_prefix_len = self.common_prefix_len(&encoded_key[pos..], prefix);
                
                if common_prefix_len == prefix.len() {
                    // The extension prefix is a prefix of the key
                    // Continue with the next node
                    Arc::new(Node::Extension {
                        prefix: prefix.clone(),
                        next: self.insert_node(
                            next.clone(),
                            encoded_key,
                            pos + prefix.len(),
                            value,
                        ),
                    })
                } else {
                    // The extension prefix and the key share a common prefix but diverge
                    // Split the extension node
                    let mut children = [None; 16];
                    
                    let next_nibble_old = prefix[common_prefix_len] as usize;
                    let next_nibble_new = encoded_key[pos + common_prefix_len] as usize;
                    
                    if common_prefix_len == 0 {
                        // No common prefix, create a branch
                        children[next_nibble_old] = Some(Arc::new(Node::Extension {
                            prefix: prefix[1..].to_vec(),
                            next: next.clone(),
                        }));
                        
                        children[next_nibble_new] = Some(self.insert_node(
                            Arc::new(Node::Empty),
                            encoded_key,
                            pos + 1,
                            value,
                        ));
                        
                        Arc::new(Node::Branch {
                            children,
                            value: None,
                        })
                    } else {
                        // Common prefix exists, create an extension to a branch
                        children[next_nibble_old] = Some(Arc::new(Node::Extension {
                            prefix: prefix[common_prefix_len + 1..].to_vec(),
                            next: next.clone(),
                        }));
                        
                        children[next_nibble_new] = Some(self.insert_node(
                            Arc::new(Node::Empty),
                            encoded_key,
                            pos + common_prefix_len + 1,
                            value,
                        ));
                        
                        Arc::new(Node::Extension {
                            prefix: encoded_key[pos..pos + common_prefix_len].to_vec(),
                            next: Arc::new(Node::Branch {
                                children,
                                value: None,
                            }),
                        })
                    }
                }
            },
        }
    }
    
    /// Delete a key from the trie
    pub fn delete(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        let encoded_key = self.encode_key(key);
        let (new_root, value) = self.delete_node(self.root.clone(), &encoded_key, 0);
        self.root = new_root;
        value
    }
    
    /// Helper function to delete a key from a node
    fn delete_node(&self, node: Arc<Node>, encoded_key: &[u8], pos: usize) -> (Arc<Node>, Option<Vec<u8>>) {
        match &*node {
            Node::Empty => (node, None),
            Node::Leaf { key_suffix, value } => {
                if &encoded_key[pos..] == key_suffix {
                    // Found the key, delete it
                    (Arc::new(Node::Empty), Some(value.clone()))
                } else {
                    // Key not found
                    (node, None)
                }
            },
            Node::Branch { children, value } => {
                if pos == encoded_key.len() {
                    // We've reached the end of the key, delete the value at this branch
                    if value.is_some() {
                        let mut new_children = children.clone();
                        (Arc::new(Node::Branch {
                            children: new_children,
                            value: None,
                        }), value.clone())
                    } else {
                        // No value to delete
                        (node, None)
                    }
                } else {
                    // Get the next nibble and follow the corresponding child
                    let nibble = encoded_key[pos] as usize;
                    
                    if let Some(child) = &children[nibble] {
                        let (new_child, deleted_value) = self.delete_node(child.clone(), encoded_key, pos + 1);
                        
                        if deleted_value.is_some() {
                            // Update the child
                            let mut new_children = children.clone();
                            
                            if matches!(&*new_child, Node::Empty) {
                                new_children[nibble] = None;
                            } else {
                                new_children[nibble] = Some(new_child);
                            }
                            
                            // Check if we need to collapse the branch
                            let mut child_count = 0;
                            let mut last_child_idx = 0;
                            let mut last_child = None;
                            
                            for (i, child) in new_children.iter().enumerate() {
                                if child.is_some() {
                                    child_count += 1;
                                    last_child_idx = i;
                                    last_child = child.clone();
                                }
                            }
                            
                            if child_count == 0 {
                                // No children left, convert to empty or leaf
                                if value.is_some() {
                                    (Arc::new(Node::Leaf {
                                        key_suffix: vec![],
                                        value: value.clone().unwrap(),
                                    }), deleted_value)
                                } else {
                                    (Arc::new(Node::Empty), deleted_value)
                                }
                            } else if child_count == 1 && value.is_none() {
                                // Only one child and no value, collapse
                                match &*last_child.unwrap() {
                                    Node::Leaf { key_suffix, value } => {
                                        let mut new_key_suffix = vec![last_child_idx as u8];
                                        new_key_suffix.extend_from_slice(key_suffix);
                                        
                                        (Arc::new(Node::Leaf {
                                            key_suffix: new_key_suffix,
                                            value: value.clone(),
                                        }), deleted_value)
                                    },
                                    Node::Extension { prefix, next } => {
                                        let mut new_prefix = vec![last_child_idx as u8];
                                        new_prefix.extend_from_slice(prefix);
                                        
                                        (Arc::new(Node::Extension {
                                            prefix: new_prefix,
                                            next: next.clone(),
                                        }), deleted_value)
                                    },
                                    _ => (Arc::new(Node::Branch {
                                        children: new_children,
                                        value: None,
                                    }), deleted_value),
                                }
                            } else {
                                // Multiple children or has value, keep as branch
                                (Arc::new(Node::Branch {
                                    children: new_children,
                                    value: value.clone(),
                                }), deleted_value)
                            }
                        } else {
                            // No value was deleted, return the original node
                            (node, None)
                        }
                    } else {
                        // Child doesn't exist, key not found
                        (node, None)
                    }
                }
            },
            Node::Extension { prefix, next } => {
                if encoded_key[pos..].starts_with(prefix) {
                    // The key matches the prefix, continue with the next node
                    let (new_next, deleted_value) = self.delete_node(next.clone(), encoded_key, pos + prefix.len());
                    
                    if deleted_value.is_some() {
                        // Update the next node
                        if matches!(&*new_next, Node::Empty) {
                            // Next node is empty, convert to empty
                            (Arc::new(Node::Empty), deleted_value)
                        } else if let Node::Extension { prefix: next_prefix, next: next_next } = &*new_next {
                            // Next node is an extension, merge the prefixes
                            let mut merged_prefix = prefix.clone();
                            merged_prefix.extend_from_slice(next_prefix);
                            
                            (Arc::new(Node::Extension {
                                prefix: merged_prefix,
                                next: next_next.clone(),
                            }), deleted_value)
                        } else {
                            // Keep as extension
                            (Arc::new(Node::Extension {
                                prefix: prefix.clone(),
                                next: new_next,
                            }), deleted_value)
                        }
                    } else {
                        // No value was deleted, return the original node
                        (node, None)
                    }
                } else {
                    // The key doesn't match the prefix, key not found
                    (node, None)
                }
            },
        }
    }
    
    /// Encode a key for use in the trie
    fn encode_key(&self, key: &[u8]) -> Vec<u8> {
        // In a real implementation, we would encode the key as a sequence of nibbles
        // For simplicity, we'll just use the key as is
        key.to_vec()
    }
    
    /// Find the length of the common prefix between two byte slices
    fn common_prefix_len(&self, a: &[u8], b: &[u8]) -> usize {
        let mut i = 0;
        while i < a.len() && i < b.len() && a[i] == b[i] {
            i += 1;
        }
        i
    }
    
    /// Verify a Merkle proof
    pub fn verify_proof(&self, key: &[u8], value: &[u8], proof: &[Hash]) -> bool {
        // In a real implementation, we would verify the proof
        // For now, we'll just return true
        true
    }
    
    /// Generate a Merkle proof for a key
    pub fn generate_proof(&self, key: &[u8]) -> Vec<Hash> {
        // In a real implementation, we would generate a proof
        // For now, we'll just return an empty vector
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_trie() {
        let trie = MerklePatriciaTrie::new();
        assert_eq!(trie.get(b"key"), None);
    }
    
    #[test]
    fn test_insert_and_get() {
        let mut trie = MerklePatriciaTrie::new();
        
        // Insert a key-value pair
        trie.insert(b"key1", b"value1".to_vec());
        
        // Get the value
        let value = trie.get(b"key1");
        assert_eq!(value, Some(b"value1".to_vec()));
        
        // Get a non-existent key
        let value = trie.get(b"key2");
        assert_eq!(value, None);
    }
    
    #[test]
    fn test_update() {
        let mut trie = MerklePatriciaTrie::new();
        
        // Insert a key-value pair
        trie.insert(b"key", b"value1".to_vec());
        
        // Update the value
        trie.insert(b"key", b"value2".to_vec());
        
        // Get the updated value
        let value = trie.get(b"key");
        assert_eq!(value, Some(b"value2".to_vec()));
    }
    
    #[test]
    fn test_delete() {
        let mut trie = MerklePatriciaTrie::new();
        
        // Insert a key-value pair
        trie.insert(b"key", b"value".to_vec());
        
        // Delete the key
        let deleted = trie.delete(b"key");
        assert_eq!(deleted, Some(b"value".to_vec()));
        
        // Get the deleted key
        let value = trie.get(b"key");
        assert_eq!(value, None);
        
        // Delete a non-existent key
        let deleted = trie.delete(b"non-existent");
        assert_eq!(deleted, None);
    }
    
    #[test]
    fn test_multiple_keys() {
        let mut trie = MerklePatriciaTrie::new();
        
        // Insert multiple key-value pairs
        trie.insert(b"key1", b"value1".to_vec());
        trie.insert(b"key2", b"value2".to_vec());
        trie.insert(b"key3", b"value3".to_vec());
        
        // Get the values
        assert_eq!(trie.get(b"key1"), Some(b"value1".to_vec()));
        assert_eq!(trie.get(b"key2"), Some(b"value2".to_vec()));
        assert_eq!(trie.get(b"key3"), Some(b"value3".to_vec()));
        
        // Delete a key
        trie.delete(b"key2");
        
        // Check the remaining keys
        assert_eq!(trie.get(b"key1"), Some(b"value1".to_vec()));
        assert_eq!(trie.get(b"key2"), None);
        assert_eq!(trie.get(b"key3"), Some(b"value3".to_vec()));
    }
    
    #[test]
    fn test_root_hash() {
        let mut trie1 = MerklePatriciaTrie::new();
        let mut trie2 = MerklePatriciaTrie::new();
        
        // Insert the same key-value pairs in both tries
        trie1.insert(b"key1", b"value1".to_vec());
        trie1.insert(b"key2", b"value2".to_vec());
        
        trie2.insert(b"key1", b"value1".to_vec());
        trie2.insert(b"key2", b"value2".to_vec());
        
        // The root hashes should be the same
        assert_eq!(trie1.root_hash(), trie2.root_hash());
        
        // Insert a different key in trie2
        trie2.insert(b"key3", b"value3".to_vec());
        
        // The root hashes should be different
        assert_ne!(trie1.root_hash(), trie2.root_hash());
    }
}
