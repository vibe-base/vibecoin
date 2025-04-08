// VibeCoin Cryptography Module
//
// This module provides cryptographic primitives for the VibeCoin blockchain:
// - Key generation and management
// - Digital signatures (Ed25519)
// - Message verification
// - Hashing (for blocks, transactions, PoH, state roots)
// - Advanced cryptography (BLS signatures, zero-knowledge proofs) when enabled

pub mod keys;
pub mod hash;
pub mod signer;

// Advanced cryptography modules (enabled with the "advanced-crypto" feature)
#[cfg(feature = "advanced-crypto")]
pub mod bls;
#[cfg(feature = "advanced-crypto")]
pub mod zk;

// Re-export main components for easier access
pub use keys::{VibeKeypair, address_from_pubkey};
pub use hash::{sha256, double_sha256};
pub use signer::{sign_message, verify_signature};

// Re-export advanced cryptography components when enabled
#[cfg(feature = "advanced-crypto")]
pub use bls::{BlsKeypair, BlsSignature, aggregate_signatures, verify_aggregate_signature};
