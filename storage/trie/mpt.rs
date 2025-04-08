use std::collections::HashMap;

use array_init::array_init;
use serde::{Serialize, Deserialize};

use crate::storage::block_store::Hash;
use crate::storage::trie::node::Node;
use crate::storage::trie::encode::{bytes_to_nibbles, nibbles_to_bytes, compact_encode, compact_decode};

/// Proof item for Merkle Patricia Trie verification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProofItem {
    /// The key for this proof item
    pub key: Vec<u8>,
    /// The node for this proof item
    pub node: Node,
}

/// Proof for Merkle Patricia Trie verification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proof {
    /// The key being proven
    pub key: Vec<u8>,
    /// The value being proven (if found)
    pub value: Option<Vec<u8>>,
    /// The proof items (nodes along the path)
    pub items: Vec<ProofItem>,
    /// The root hash of the trie
    pub root_hash: Hash,
}

/// Merkle Patricia Trie implementation
pub struct MerklePatriciaTrie {
    /// Root node of the trie
    root: Node,
    /// Cache of nodes by hash
    node_cache: HashMap<Hash, Node>,
    /// Maximum cache size
    max_cache_size: usize,
}

impl MerklePatriciaTrie {
    /// Create a new empty trie
    pub fn new() -> Self {
        Self {
            root: Node::empty(),
            node_cache: HashMap::new(),
            max_cache_size: 10000, // Default cache size
        }
    }

    /// Create a new trie with a custom cache size
    pub fn with_cache_size(max_cache_size: usize) -> Self {
        Self {
            root: Node::empty(),
            node_cache: HashMap::new(),
            max_cache_size,
        }
    }

    /// Get the root hash of the trie
    pub fn root_hash(&self) -> Hash {
        // Check if the root is cached
        if let Some(hash) = self.node_cache.get(&self.root.hash()) {
            return hash.hash();
        }

        // Calculate the hash
        let hash = self.root.hash();

        // Cache the result if the cache isn't too large
        if self.node_cache.len() < self.max_cache_size {
            self.node_cache.insert(hash, self.root.clone());
        }

        hash
    }

    /// Get a value from the trie
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let nibbles = bytes_to_nibbles(key);
        self.get_at(&self.root, &nibbles, 0)
    }

    /// Helper function to get a value from a node
    fn get_at(&self, node: &Node, nibbles: &[u8], depth: usize) -> Option<Vec<u8>> {
        if depth == nibbles.len() {
            // We've reached the end of the key
            return match node {
                Node::Leaf { value, .. } => Some(value.clone()),
                Node::Branch { value, .. } => value.clone(),
                _ => None,
            };
        }

        match node {
            Node::Empty => None,

            Node::Leaf { key, value } => {
                // Check if the remaining key matches
                let (decoded_key, _) = compact_decode(key);
                if &decoded_key == &nibbles[depth..] {
                    Some(value.clone())
                } else {
                    None
                }
            },

            Node::Extension { key, child } => {
                // Check if the key prefix matches
                let (decoded_key, _) = compact_decode(key);
                let prefix_len = decoded_key.len();

                if nibbles.len() - depth < prefix_len {
                    // Key is too short
                    return None;
                }

                if &decoded_key != &nibbles[depth..depth + prefix_len] {
                    // Prefix doesn't match
                    return None;
                }

                // Continue with the next node
                self.get_at(child, nibbles, depth + prefix_len)
            },

            Node::Branch { children, value: _ } => {
                let nibble = nibbles[depth] as usize;

                if let Some(child) = &children[nibble] {
                    // Continue with the child node
                    self.get_at(child, nibbles, depth + 1)
                } else {
                    // No child at this nibble
                    None
                }
            },
        }
    }

    /// Insert a key-value pair into the trie
    pub fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        let nibbles = bytes_to_nibbles(key);
        self.root = self.insert_at(self.root.clone(), &nibbles, 0, value);
    }

    /// Helper function to insert a key-value pair at a node
    fn insert_at(&mut self, node: Node, nibbles: &[u8], depth: usize, value: Vec<u8>) -> Node {
        // Clone the value to avoid ownership issues
        let value_clone = value.clone();
        if depth == nibbles.len() {
            // We've reached the end of the key
            match node {
                Node::Empty => {
                    // Create a leaf node with an empty key
                    Node::leaf(compact_encode(&[], true), value_clone)
                },
                Node::Leaf { key, .. } => {
                    // Replace the value in the leaf node
                    Node::leaf(key, value_clone)
                },
                Node::Extension { key: _, child: _ } => {
                    // Convert to a branch with a value
                    let children = array_init(|_| None);
                    Node::branch(children, Some(value_clone))
                },
                Node::Branch { children, .. } => {
                    // Update the value in the branch node
                    Node::branch(children, Some(value_clone))
                },
            }
        } else {
            match node {
                Node::Empty => {
                    // Create a leaf node with the remaining key
                    let remaining = &nibbles[depth..];
                    Node::leaf(compact_encode(remaining, true), value_clone)
                },

                Node::Leaf { key, value: old_value } => {
                    // Split the leaf node
                    let (decoded_key, _) = compact_decode(&key);

                    // Find the common prefix
                    let mut common_prefix_len = 0;
                    while common_prefix_len < decoded_key.len() &&
                          depth + common_prefix_len < nibbles.len() &&
                          decoded_key[common_prefix_len] == nibbles[depth + common_prefix_len] {
                        common_prefix_len += 1;
                    }

                    if common_prefix_len == decoded_key.len() && common_prefix_len == nibbles.len() - depth {
                        // The keys are identical, just update the value
                        return Node::leaf(key, value);
                    }

                    if common_prefix_len == 0 {
                        // No common prefix, create a branch node
                        let mut children = array_init(|_| None);

                        if !decoded_key.is_empty() {
                            let child_nibble = decoded_key[0] as usize;
                            let child_node = if decoded_key.len() == 1 {
                                Node::leaf(compact_encode(&[], true), old_value.clone())
                            } else {
                                Node::leaf(compact_encode(&decoded_key[1..], true), old_value.clone())
                            };
                            children[child_nibble] = Some(Box::new(child_node));
                        }

                        let nibble = nibbles[depth] as usize;
                        let new_node = if depth + 1 == nibbles.len() {
                            // This is the last nibble, create a leaf with empty key
                            Node::leaf(compact_encode(&[], true), value)
                        } else {
                            // Create a leaf with the remaining key
                            Node::leaf(compact_encode(&nibbles[depth+1..], true), value)
                        };
                        children[nibble] = Some(Box::new(new_node));

                        if decoded_key.is_empty() {
                            // The old leaf had an empty key, keep its value in the branch
                            return Node::branch(children, Some(old_value.clone()));
                        } else {
                            return Node::branch(children, None);
                        }
                    }

                    // We have a common prefix, create an extension node
                    let prefix = &nibbles[depth..depth + common_prefix_len];

                    // Create a branch for the divergent part
                    let mut children = array_init(|_| None);

                    if common_prefix_len == decoded_key.len() {
                        // The old leaf's key is a prefix of the new key
                        children[nibbles[depth + common_prefix_len] as usize] = Some(Box::new(
                            self.insert_at(Node::empty(), nibbles, depth + common_prefix_len + 1, value)
                        ));
                        let branch = Node::branch(children, Some(old_value));

                        if common_prefix_len == 0 {
                            return branch;
                        } else {
                            return Node::extension(compact_encode(prefix, false), branch);
                        }
                    } else if common_prefix_len == nibbles.len() - depth {
                        // The new key is a prefix of the old leaf's key
                        let remaining = &decoded_key[common_prefix_len..];
                        children[remaining[0] as usize] = Some(Box::new(
                            Node::leaf(compact_encode(&remaining[1..], true), old_value)
                        ));
                        let branch = Node::branch(children, Some(value));

                        if common_prefix_len == 0 {
                            return branch;
                        } else {
                            return Node::extension(compact_encode(prefix, false), branch);
                        }
                    } else {
                        // The keys diverge after the common prefix
                        let old_remaining = &decoded_key[common_prefix_len..];
                        let new_remaining = &nibbles[depth + common_prefix_len..];

                        children[old_remaining[0] as usize] = Some(Box::new(
                            Node::leaf(compact_encode(&old_remaining[1..], true), old_value)
                        ));

                        children[new_remaining[0] as usize] = Some(Box::new(
                            Node::leaf(compact_encode(&new_remaining[1..], true), value)
                        ));

                        let branch = Node::branch(children, None);

                        if common_prefix_len == 0 {
                            return branch;
                        } else {
                            return Node::extension(compact_encode(prefix, false), branch);
                        }
                    }
                },

                Node::Extension { key, child } => {
                    // Handle extension node
                    let (decoded_key, _) = compact_decode(&key);

                    // Find the common prefix
                    let mut common_prefix_len = 0;
                    while common_prefix_len < decoded_key.len() &&
                          depth + common_prefix_len < nibbles.len() &&
                          decoded_key[common_prefix_len] == nibbles[depth + common_prefix_len] {
                        common_prefix_len += 1;
                    }

                    if common_prefix_len == decoded_key.len() {
                        // The extension's key is a prefix of the new key
                        // Continue with the child node
                        let new_child = self.insert_at(*child, nibbles, depth + common_prefix_len, value);
                        return Node::extension(key, new_child);
                    }

                    // The keys diverge, split the extension
                    let prefix = &decoded_key[0..common_prefix_len];
                    let old_remaining = &decoded_key[common_prefix_len..];
                    let new_remaining = &nibbles[depth + common_prefix_len..];

                    let mut children = array_init(|_| None);

                    if old_remaining.is_empty() {
                        // The old extension's key is a prefix of the new key
                        // This shouldn't happen as it's handled above
                        unreachable!();
                    } else {
                        // Create a child for the old path
                        let old_child = if old_remaining.len() == 1 {
                            *child
                        } else {
                            Node::extension(compact_encode(&old_remaining[1..], false), *child)
                        };
                        children[old_remaining[0] as usize] = Some(Box::new(old_child));
                    }

                    // Create a child for the new path
                    if new_remaining.is_empty() {
                        // The new key ends here, add a value to the branch
                        children[new_remaining[0] as usize] = Some(Box::new(
                            Node::leaf(compact_encode(&[], true), value)
                        ));
                    } else {
                        // Continue with the remaining key
                        let new_child = Node::leaf(compact_encode(&new_remaining[1..], true), value);
                        children[new_remaining[0] as usize] = Some(Box::new(new_child));
                    }

                    let branch = Node::branch(children, None);

                    if common_prefix_len == 0 {
                        return branch;
                    } else {
                        return Node::extension(compact_encode(prefix, false), branch);
                    }
                },

                Node::Branch { mut children, value: mut _branch_value } => {
                    // Handle branch node
                    let nibble = nibbles[depth] as usize;

                    if depth + 1 == nibbles.len() {
                        // This is the last nibble, update the branch's value
                        _branch_value = Some(value_clone.clone());
                    } else {
                        // Continue with the child node
                        let child = children[nibble].take().unwrap_or(Box::new(Node::empty()));
                        let new_child = self.insert_at(*child, nibbles, depth + 1, value_clone.clone());
                        children[nibble] = Some(Box::new(new_child));
                    }

                    Node::branch(children, Some(value_clone.clone()))
                },
            }
        }
    }

    /// Delete a key from the trie
    pub fn delete(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        let nibbles = bytes_to_nibbles(key);
        let (new_root, value) = self.delete_at(self.root.clone(), &nibbles, 0);
        self.root = new_root;
        value
    }

    /// Helper function to delete a key from a node
    fn delete_at(&mut self, mut node: Node, nibbles: &[u8], depth: usize) -> (Node, Option<Vec<u8>>) {
        if depth == nibbles.len() {
            // We've reached the end of the key
            match node {
                Node::Empty => (Node::empty(), None),

                Node::Leaf { ref key, ref value } => {
                    // Check if this is the correct leaf
                    let (decoded_key, _) = compact_decode(&key);
                    if decoded_key.is_empty() {
                        // This is the correct leaf, remove it
                        (Node::empty(), Some(value.clone()))
                    } else {
                        // This leaf has a non-empty key, it's not the one we're looking for
                        (node, None)
                    }
                },

                Node::Branch { children, value } => {
                    // Remove the value from the branch
                    let new_branch = Node::branch(children, None);

                    // Check if the branch can be simplified
                    let (simplified, _) = self.simplify_node(new_branch);
                    (simplified, value)
                },

                Node::Extension { .. } => {
                    // Extensions don't have values
                    (node, None)
                },
            }
        } else {
            match node {
                Node::Empty => (Node::empty(), None),

                Node::Leaf { ref key, ref value } => {
                    // Check if this is the correct leaf
                    let (decoded_key, _) = compact_decode(&key);

                    if decoded_key.len() == nibbles.len() - depth &&
                       &decoded_key == &nibbles[depth..] {
                        // This is the correct leaf, remove it
                        (Node::empty(), Some(value.clone()))
                    } else {
                        // This is not the leaf we're looking for
                        (node, None)
                    }
                },

                Node::Extension { ref key, ref child } => {
                    // Check if the key prefix matches
                    let (decoded_key, _) = compact_decode(&key);
                    let prefix_len = decoded_key.len();

                    if nibbles.len() - depth < prefix_len {
                        // Key is too short
                        return (node, None);
                    }

                    if &decoded_key != &nibbles[depth..depth + prefix_len] {
                        // Prefix doesn't match
                        return (node, None);
                    }

                    // Continue with the child node
                    let (new_child, value) = self.delete_at(child.as_ref().clone(), nibbles, depth + prefix_len);

                    if new_child.is_empty() {
                        // Child was removed, remove this extension too
                        (Node::empty(), value)
                    } else {
                        // Create a new extension with the updated child
                        let new_extension = Node::extension(key.clone(), new_child);

                        // Check if the extension can be simplified
                        let (simplified, _) = self.simplify_node(new_extension);
                        (simplified, value)
                    }
                },

                Node::Branch { ref mut children, ref value } => {
                    // Get the child for this nibble
                    let nibble = nibbles[depth] as usize;

                    if let Some(child) = &children[nibble] {
                        // Continue with the child node
                        let (new_child, deleted_value) = self.delete_at(*child.clone(), nibbles, depth + 1);

                        if new_child.is_empty() {
                            // Child was removed
                            children[nibble] = None;
                        } else {
                            // Update the child
                            children[nibble] = Some(Box::new(new_child));
                        }

                        // Check if the branch can be simplified
                        let new_branch = Node::branch(children.clone(), value.clone());
                        let (simplified, _) = self.simplify_node(new_branch);

                        (simplified, deleted_value)
                    } else {
                        // No child at this nibble
                        (node, None)
                    }
                },
            }
        }
    }

    /// Simplify a node if possible
    fn simplify_node(&self, node: Node) -> (Node, bool) {
        match node {
            Node::Branch { ref children, ref value } => {
                // Count non-empty children
                let mut non_empty = 0;
                let mut last_index = 0;
                let mut last_child = None;

                for (i, child) in children.iter().enumerate() {
                    if let Some(c) = child {
                        non_empty += 1;
                        last_index = i;
                        last_child = Some(c.clone());
                    }
                }

                if non_empty == 0 {
                    // No children, convert to a leaf or empty node
                    if let Some(v) = value {
                        (Node::leaf(compact_encode(&[], true), v.clone()), true)
                    } else {
                        (Node::empty(), true)
                    }
                } else if non_empty == 1 && value.is_none() {
                    // One child and no value, merge with the child
                    let last_child_clone = last_child.clone().unwrap();
                    match *last_child_clone {
                        Node::Leaf { key, value: leaf_value } => {
                            // Prepend the branch index to the leaf's key
                            let (decoded_key, is_leaf) = compact_decode(&key);
                            let mut new_key = vec![last_index as u8];
                            new_key.extend_from_slice(&decoded_key);

                            (Node::leaf(compact_encode(&new_key, is_leaf), leaf_value), true)
                        },
                        Node::Extension { key, child: ext_child } => {
                            // Prepend the branch index to the extension's key
                            let (decoded_key, _) = compact_decode(&key);
                            let mut new_key = vec![last_index as u8];
                            new_key.extend_from_slice(&decoded_key);

                            (Node::extension(compact_encode(&new_key, false), *ext_child), true)
                        },
                        _ => {
                            // Create an extension with just the branch index
                            let key = compact_encode(&[last_index as u8], false);
                            let last_child_ref = last_child.unwrap();
                            (Node::extension(key, *last_child_ref), true)
                        }
                    }
                } else {
                    // Multiple children or has value, keep as branch
                    (node, false)
                }
            },
            Node::Extension { ref key, ref child } => {
                match child.as_ref() {
                    Node::Extension { key: child_key, child: grandchild } => {
                        // Merge with child extension
                        let (decoded_key, _) = compact_decode(&key);
                        let (decoded_child_key, _) = compact_decode(&child_key);

                        let mut new_key = decoded_key.clone();
                        new_key.extend_from_slice(&decoded_child_key);

                        (Node::extension(compact_encode(&new_key, false), (**grandchild).clone()), true)
                    },
                    Node::Leaf { key: child_key, value } => {
                        // Merge with child leaf
                        let (decoded_key, _) = compact_decode(&key);
                        let (decoded_child_key, is_leaf) = compact_decode(&child_key);

                        let mut new_key = decoded_key.clone();
                        new_key.extend_from_slice(&decoded_child_key);

                        (Node::leaf(compact_encode(&new_key, is_leaf), value.clone()), true)
                    },
                    _ => {
                        // Can't simplify
                        (node, false)
                    }
                }
            },
            _ => {
                // Leaf and Empty nodes can't be simplified
                (node, false)
            }
        }
    }

    /// Clear the node cache
    pub fn clear_cache(&mut self) {
        self.node_cache.clear();
    }

    /// Get the number of nodes in the cache
    pub fn cache_size(&self) -> usize {
        self.node_cache.len()
    }

    /// Generate a proof for a key
    pub fn generate_proof(&self, key: &[u8]) -> Proof {
        let nibbles = bytes_to_nibbles(key);
        let mut items = Vec::new();
        let value = self.get_proof_at(&self.root, &nibbles, 0, &mut items);

        // Optimize the proof by removing unnecessary nodes
        self.optimize_proof(&mut items);

        Proof {
            key: key.to_vec(),
            value,
            items,
            root_hash: self.root_hash(),
        }
    }

    /// Optimize a proof by removing unnecessary nodes
    fn optimize_proof(&self, items: &mut Vec<ProofItem>) {
        // Remove duplicate nodes
        let mut seen_hashes = std::collections::HashSet::new();
        items.retain(|item| {
            let hash = item.node.hash();
            let is_new = seen_hashes.insert(hash);
            is_new
        });

        // Sort items by depth (key length) for more efficient verification
        items.sort_by(|a, b| a.key.len().cmp(&b.key.len()));
    }

    /// Helper function to get a value and generate a proof
    fn get_proof_at(&self, node: &Node, nibbles: &[u8], depth: usize, items: &mut Vec<ProofItem>) -> Option<Vec<u8>> {
        // Add this node to the proof
        items.push(ProofItem {
            key: nibbles_to_bytes(&nibbles[..depth]),
            node: node.clone(),
        });

        if depth == nibbles.len() {
            // We've reached the end of the key
            return match node {
                Node::Leaf { value, .. } => Some(value.clone()),
                Node::Branch { value, .. } => value.clone(),
                _ => None,
            };
        }

        match node {
            Node::Empty => None,

            Node::Leaf { key, value } => {
                // Check if the remaining key matches
                let (decoded_key, _) = compact_decode(key);
                if &decoded_key == &nibbles[depth..] {
                    Some(value.clone())
                } else {
                    None
                }
            },

            Node::Extension { key, child } => {
                // Check if the key prefix matches
                let (decoded_key, _) = compact_decode(key);
                let prefix_len = decoded_key.len();

                if nibbles.len() - depth < prefix_len {
                    // Key is too short
                    return None;
                }

                if &decoded_key != &nibbles[depth..depth + prefix_len] {
                    // Prefix doesn't match
                    return None;
                }

                // Continue with the next node
                self.get_proof_at(child, nibbles, depth + prefix_len, items)
            },

            Node::Branch { children, value: _ } => {
                let nibble = nibbles[depth] as usize;

                if let Some(child) = &children[nibble] {
                    // Continue with the child node
                    self.get_proof_at(child, nibbles, depth + 1, items)
                } else {
                    // No child at this nibble
                    None
                }
            },
        }
    }

    /// Verify a proof
    pub fn verify_proof(proof: &Proof) -> bool {
        // Check if the proof is empty
        if proof.items.is_empty() {
            return false;
        }

        let nibbles = bytes_to_nibbles(&proof.key);
        let mut current_depth = 0;

        // Start with the root node
        let root_node = &proof.items[0].node;
        let calculated_root_hash = root_node.hash();

        // Check that the root hash matches
        if calculated_root_hash != proof.root_hash {
            return false;
        }

        // Create a map of nodes by key for faster lookup
        let mut node_map = std::collections::HashMap::new();
        for item in &proof.items {
            node_map.insert(item.key.clone(), &item.node);
        }

        // Verify the path through the trie using the node map
        let mut current_node = root_node;

        // Follow the path through the trie
        while current_depth < nibbles.len() {
            match current_node {
                Node::Empty => {
                    // Empty node means the key doesn't exist
                    return proof.value.is_none();
                },

                Node::Leaf { key, value } => {
                    // Check if the remaining key matches
                    let remaining_key = &nibbles[current_depth..];
                    if key != remaining_key {
                        return false;
                    }

                    // Check if the value matches
                    return match &proof.value {
                        Some(v) => v == value,
                        None => false,
                    };
                },

                Node::Extension { key, child } => {
                    // Check if the prefix matches
                    if nibbles.len() - current_depth < key.len() {
                        return false;
                    }

                    let prefix = &nibbles[current_depth..current_depth + key.len()];
                    if prefix != key {
                        return false;
                    }

                    // Move to the child node
                    current_depth += key.len();

                    // Find the child node in the map
                    let child_key = nibbles_to_bytes(&nibbles[..current_depth]);
                    if let Some(next_node) = node_map.get(&child_key) {
                        current_node = *next_node;
                    } else {
                        return false; // Missing node in proof
                    }
                },

                Node::Branch { children, value } => {
                    // Check if we're at the end of the key
                    if current_depth == nibbles.len() {
                        // Check if the value matches
                        return match (&proof.value, value) {
                            (Some(v), Some(node_v)) => v == node_v,
                            (None, None) => true,
                            _ => false,
                        };
                    }

                    // Get the next nibble
                    let nibble = nibbles[current_depth] as usize;

                    // Check if there's a child at this nibble
                    if let Some(child) = &children[nibble] {
                        // Move to the next nibble
                        current_depth += 1;

                        // Find the child node in the map
                        let child_key = nibbles_to_bytes(&nibbles[..current_depth]);
                        if let Some(next_node) = node_map.get(&child_key) {
                            current_node = *next_node;
                        } else {
                            return false; // Missing node in proof
                        }
                    } else {
                        // No child at this nibble, key doesn't exist
                        return proof.value.is_none();
                    }
                },
            }
        }

        // If we've reached the end of the key, check the value
        match current_node {
            Node::Empty => {
                // Empty node means the key doesn't exist
                proof.value.is_none()
            },

            Node::Leaf { key, value } => {
                // Check if the value matches
                match &proof.value {
                    Some(v) => v == value,
                    None => false,
                }
            },

            Node::Extension { key: _, child: _ } => {
                // Extension node should not be at the end of the key
                false
            },

            Node::Branch { children: _, value } => {
                // Branch node at the end of the key should have a value
                match (&proof.value, value) {
                    (Some(v), Some(node_v)) => v == node_v,
                    (None, None) => true,
                    _ => false,
                }
            }
        }
    }

    /// Verify a proof against a specific root hash
    pub fn verify_proof_with_root(proof: &Proof, root_hash: &[u8; 32]) -> bool {
        // First check if the root hash matches
        if &proof.root_hash != root_hash {
            return false;
        }

        // Then verify the proof itself
        Self::verify_proof(proof)
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
        trie.insert(b"key1", b"value1".to_vec());

        // Update the value
        trie.insert(b"key1", b"value2".to_vec());

        // Get the updated value
        let value = trie.get(b"key1");
        assert_eq!(value, Some(b"value2".to_vec()));
    }

    #[test]
    fn test_delete() {
        let mut trie = MerklePatriciaTrie::new();

        // Insert key-value pairs
        trie.insert(b"key1", b"value1".to_vec());
        trie.insert(b"key2", b"value2".to_vec());

        // Delete a key
        let value = trie.delete(b"key1");
        assert_eq!(value, Some(b"value1".to_vec()));

        // Verify the key is gone
        let value = trie.get(b"key1");
        assert_eq!(value, None);

        // Verify other keys still exist
        let value = trie.get(b"key2");
        assert_eq!(value, Some(b"value2".to_vec()));

        // Delete a non-existent key
        let value = trie.delete(b"key3");
        assert_eq!(value, None);
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

        // Insert a different key-value pair in trie2
        trie2.insert(b"key3", b"value3".to_vec());

        // The root hashes should now be different
        assert_ne!(trie1.root_hash(), trie2.root_hash());
    }

    #[test]
    fn test_complex_trie() {
        let mut trie = MerklePatriciaTrie::new();

        // Insert a bunch of key-value pairs
        for i in 0..100 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            trie.insert(key.as_bytes(), value.as_bytes().to_vec());
        }

        // Verify all keys exist
        for i in 0..100 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            assert_eq!(trie.get(key.as_bytes()), Some(value.as_bytes().to_vec()));
        }

        // Delete some keys
        for i in 0..50 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            assert_eq!(trie.delete(key.as_bytes()), Some(value.as_bytes().to_vec()));
        }

        // Verify deleted keys are gone
        for i in 0..50 {
            let key = format!("key{}", i);
            assert_eq!(trie.get(key.as_bytes()), None);
        }

        // Verify remaining keys still exist
        for i in 50..100 {
            let key = format!("key{}", i);
            let value = format!("value{}", i);
            assert_eq!(trie.get(key.as_bytes()), Some(value.as_bytes().to_vec()));
        }
    }

    #[test]
    fn test_proof_generation_and_verification() {
        let mut trie = MerklePatriciaTrie::new();

        // Insert some key-value pairs
        trie.insert(b"key1", b"value1".to_vec());
        trie.insert(b"key2", b"value2".to_vec());
        trie.insert(b"key3", b"value3".to_vec());

        // Generate a proof for key1
        let proof = trie.generate_proof(b"key1");

        // Verify the proof
        assert!(MerklePatriciaTrie::verify_proof(&proof));
        assert_eq!(proof.value, Some(b"value1".to_vec()));

        // Generate a proof for a non-existent key
        let proof = trie.generate_proof(b"key4");

        // Verify the proof (should still be valid, but with no value)
        assert!(MerklePatriciaTrie::verify_proof(&proof));
        assert_eq!(proof.value, None);

        // Tamper with the proof
        let mut tampered_proof = trie.generate_proof(b"key1");
        if let Some(ref mut value) = tampered_proof.value {
            value[0] = value[0].wrapping_add(1); // Change the value
        }

        // Verify the tampered proof (should fail)
        assert!(!MerklePatriciaTrie::verify_proof(&tampered_proof));
    }

    #[test]
    fn test_light_client_verification() {
        // This test simulates a light client verifying state without having the full trie

        // Create a "full node" trie with some data
        let mut full_trie = MerklePatriciaTrie::new();
        full_trie.insert(b"account1", b"balance:100".to_vec());
        full_trie.insert(b"account2", b"balance:200".to_vec());
        full_trie.insert(b"account3", b"balance:300".to_vec());

        // Get the root hash (this would be in a block header)
        let root_hash = full_trie.root_hash();

        // Generate a proof for account2
        let proof = full_trie.generate_proof(b"account2");

        // A light client would receive this proof and verify it
        // The light client only needs the proof and the root hash, not the full trie
        assert_eq!(proof.root_hash, root_hash);
        assert!(MerklePatriciaTrie::verify_proof(&proof));
        assert!(MerklePatriciaTrie::verify_proof_with_root(&proof, &root_hash));
        assert_eq!(proof.value, Some(b"balance:200".to_vec()));

        // The light client can be sure that account2 has a balance of 200
        // without having to download the entire state trie
    }
}
