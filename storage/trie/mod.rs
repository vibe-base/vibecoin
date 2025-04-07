//! Merkle Patricia Trie implementation for VibeCoin
//!
//! This module provides a production-ready implementation of a Merkle Patricia Trie (MPT)
//! for storing and verifying the blockchain state. The MPT enables efficient lookups,
//! updates, and cryptographic verification of the state.
//!
//! The implementation follows the Ethereum Yellow Paper specification with some
//! VibeCoin-specific optimizations.

pub mod node;
pub mod encode;
pub mod mpt;

// Re-export main components
pub use node::Node;
pub use mpt::MerklePatriciaTrie;
pub use mpt::{Proof, ProofItem};
