// VibeCoin Cryptography Module
//
// This module provides cryptographic primitives for the VibeCoin blockchain:
// - Key generation and management
// - Digital signatures (Ed25519 and BLS)
// - Message verification
// - Hashing (for blocks, transactions, PoH, state roots)
// - Zero-knowledge proofs (optional)

pub mod keys;
pub mod hash;
pub mod signer;
pub mod bls;
pub mod zk;

// Re-export main components for easier access
pub use keys::{VibeKeypair, address_from_pubkey};
pub use hash::{sha256, double_sha256};
pub use signer::{sign_message, verify_signature};
pub use bls::{BlsKeypair, BlsSignature, aggregate_signatures, verify_aggregate_signature};
