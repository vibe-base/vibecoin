use std::sync::Arc;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use array_init::array_init;
use log::{debug, error, info, trace};

use crate::storage::block_store::Hash;

/// Node types in the Merkle Patricia Trie
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    /// Empty node (null)
    Empty,
    
    /// Leaf node containing a value
    Leaf {
        /// Nibble-encoded key suffix (remaining part of the key)
        key: Vec<u8>,
        /// Value stored at this leaf
        value: Vec<u8>,
    },
    
    /// Extension node with a shared prefix
    Extension {
        /// Shared nibble-encoded prefix
        key: Vec<u8>,
        /// Next node
        child: Box<Node>,
    },
    
    /// Branch node with up to 16 children
    Branch {
        /// Children nodes (one for each hex digit)
        children: [Option<Box<Node>>; 16],
        /// Value stored at this branch (if any)
        value: Option<Vec<u8>>,
    },
}

impl Node {
    /// Create a new empty node
    pub fn empty() -> Self {
        Node::Empty
    }
    
    /// Create a new leaf node
    pub fn leaf(key: Vec<u8>, value: Vec<u8>) -> Self {
        Node::Leaf { key, value }
    }
    
    /// Create a new extension node
    pub fn extension(key: Vec<u8>, child: Node) -> Self {
        Node::Extension { key, child: Box::new(child) }
    }
    
    /// Create a new branch node
    pub fn branch(children: [Option<Box<Node>>; 16], value: Option<Vec<u8>>) -> Self {
        Node::Branch { children, value }
    }
    
    /// Create a new branch node with no children or value
    pub fn empty_branch() -> Self {
        Node::Branch {
            children: array_init(|_| None),
            value: None,
        }
    }
    
    /// Check if the node is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, Node::Empty)
    }
    
    /// Calculate the hash of this node
    pub fn hash(&self) -> Hash {
        match self {
            Node::Empty => {
                // Empty node has a special hash
                let mut hasher = Sha256::new();
                hasher.update(b"empty");
                hasher.finalize().into()
            },
            Node::Leaf { key, value } => {
                let mut hasher = Sha256::new();
                hasher.update(b"leaf");
                hasher.update(&[key.len() as u8]);
                hasher.update(key);
                hasher.update(&[value.len() as u8]);
                hasher.update(value);
                hasher.finalize().into()
            },
            Node::Extension { key, child } => {
                let mut hasher = Sha256::new();
                hasher.update(b"extension");
                hasher.update(&[key.len() as u8]);
                hasher.update(key);
                hasher.update(&child.hash());
                hasher.finalize().into()
            },
            Node::Branch { children, value } => {
                let mut hasher = Sha256::new();
                hasher.update(b"branch");
                
                // Hash each child
                for child in children {
                    match child {
                        Some(node) => hasher.update(&node.hash()),
                        None => hasher.update([0u8; 32]),
                    }
                }
                
                // Hash the value if present
                if let Some(v) = value {
                    hasher.update(&[1u8]);
                    hasher.update(&[v.len() as u8]);
                    hasher.update(v);
                } else {
                    hasher.update(&[0u8]);
                }
                
                hasher.finalize().into()
            },
        }
    }
    
    /// Serialize the node to bytes
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize node")
    }
    
    /// Deserialize a node from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
    
    /// Get the value from a node if it's a leaf or branch with value
    pub fn value(&self) -> Option<&Vec<u8>> {
        match self {
            Node::Leaf { value, .. } => Some(value),
            Node::Branch { value: Some(v), .. } => Some(v),
            _ => None,
        }
    }
    
    /// Get the child of an extension node
    pub fn child(&self) -> Option<&Node> {
        match self {
            Node::Extension { child, .. } => Some(child),
            _ => None,
        }
    }
    
    /// Get a child of a branch node
    pub fn branch_child(&self, index: usize) -> Option<&Node> {
        match self {
            Node::Branch { children, .. } if index < 16 => {
                children[index].as_ref().map(|c| &**c)
            },
            _ => None,
        }
    }
    
    /// Get the key of a leaf or extension node
    pub fn key(&self) -> Option<&Vec<u8>> {
        match self {
            Node::Leaf { key, .. } => Some(key),
            Node::Extension { key, .. } => Some(key),
            _ => None,
        }
    }
    
    /// Get the node type as a string
    pub fn node_type(&self) -> &'static str {
        match self {
            Node::Empty => "empty",
            Node::Leaf { .. } => "leaf",
            Node::Extension { .. } => "extension",
            Node::Branch { .. } => "branch",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_node_creation() {
        // Test empty node
        let empty = Node::empty();
        assert!(empty.is_empty());
        
        // Test leaf node
        let leaf = Node::leaf(vec![1, 2, 3], vec![4, 5, 6]);
        assert_eq!(leaf.node_type(), "leaf");
        assert_eq!(leaf.key(), Some(&vec![1, 2, 3]));
        assert_eq!(leaf.value(), Some(&vec![4, 5, 6]));
        
        // Test extension node
        let extension = Node::extension(vec![1, 2, 3], leaf.clone());
        assert_eq!(extension.node_type(), "extension");
        assert_eq!(extension.key(), Some(&vec![1, 2, 3]));
        assert!(matches!(extension.child(), Some(Node::Leaf { .. })));
        
        // Test branch node
        let mut children: [Option<Box<Node>>; 16] = array_init(|_| None);
        children[0] = Some(Box::new(leaf.clone()));
        let branch = Node::branch(children, Some(vec![7, 8, 9]));
        assert_eq!(branch.node_type(), "branch");
        assert_eq!(branch.value(), Some(&vec![7, 8, 9]));
        assert!(matches!(branch.branch_child(0), Some(Node::Leaf { .. })));
        assert_eq!(branch.branch_child(1), None);
    }
    
    #[test]
    fn test_node_hash() {
        // Test empty node hash
        let empty = Node::empty();
        let empty_hash = empty.hash();
        assert_ne!(empty_hash, [0u8; 32]);
        
        // Test leaf node hash
        let leaf = Node::leaf(vec![1, 2, 3], vec![4, 5, 6]);
        let leaf_hash = leaf.hash();
        assert_ne!(leaf_hash, empty_hash);
        
        // Test extension node hash
        let extension = Node::extension(vec![1, 2, 3], leaf.clone());
        let extension_hash = extension.hash();
        assert_ne!(extension_hash, leaf_hash);
        assert_ne!(extension_hash, empty_hash);
        
        // Test branch node hash
        let mut children: [Option<Box<Node>>; 16] = array_init(|_| None);
        children[0] = Some(Box::new(leaf.clone()));
        let branch = Node::branch(children, Some(vec![7, 8, 9]));
        let branch_hash = branch.hash();
        assert_ne!(branch_hash, extension_hash);
        assert_ne!(branch_hash, leaf_hash);
        assert_ne!(branch_hash, empty_hash);
    }
    
    #[test]
    fn test_node_serialization() {
        // Test leaf node serialization
        let leaf = Node::leaf(vec![1, 2, 3], vec![4, 5, 6]);
        let serialized = leaf.serialize();
        let deserialized = Node::deserialize(&serialized).unwrap();
        assert_eq!(leaf, deserialized);
        
        // Test extension node serialization
        let extension = Node::extension(vec![1, 2, 3], leaf.clone());
        let serialized = extension.serialize();
        let deserialized = Node::deserialize(&serialized).unwrap();
        assert_eq!(extension, deserialized);
        
        // Test branch node serialization
        let mut children: [Option<Box<Node>>; 16] = array_init(|_| None);
        children[0] = Some(Box::new(leaf.clone()));
        let branch = Node::branch(children, Some(vec![7, 8, 9]));
        let serialized = branch.serialize();
        let deserialized = Node::deserialize(&serialized).unwrap();
        assert_eq!(branch, deserialized);
    }
}
