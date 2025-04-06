// VibeCoin Cryptography Module
//
// This module provides cryptographic primitives for the VibeCoin blockchain:
// - Key generation and management
// - Digital signatures
// - Message verification
// - Hashing (for blocks, transactions, PoH, state roots)

pub mod keys;
pub mod hash;
pub mod signer;

// Re-export main components for easier access
pub use keys::{VibeKeypair, address_from_pubkey};
pub use hash::{sha256, double_sha256};
pub use signer::{sign_message, verify_signature};
