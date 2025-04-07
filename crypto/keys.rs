use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Serialize, Deserialize};
use std::fmt;
use std::convert::TryFrom;
use crate::crypto::hash::sha256;

/// VibeCoin keypair for signing and verification
pub struct VibeKeypair {
    /// Public key for verification
    pub public: PublicKey,

    /// Secret key for signing
    pub secret: SecretKey,
}

impl Clone for VibeKeypair {
    fn clone(&self) -> Self {
        // Clone by reconstructing from the secret key bytes
        let secret_bytes = self.secret.as_bytes();
        let secret = SecretKey::from_bytes(secret_bytes).unwrap();
        let public = PublicKey::from(&secret);

        Self { public, secret }
    }
}

impl VibeKeypair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        // Use a different approach to generate a keypair
        // This avoids the version conflict with rand_core
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        let secret = SecretKey::from_bytes(&seed).expect("Failed to create secret key");
        let public = PublicKey::from(&secret);
        Self {
            public,
            secret,
        }
    }

    /// Create a keypair from secret key bytes
    pub fn from_bytes(secret_bytes: &[u8]) -> Result<Self, ed25519_dalek::SignatureError> {
        if secret_bytes.len() != 32 {
            return Err(ed25519_dalek::SignatureError::new());
        }

        let secret = SecretKey::from_bytes(secret_bytes)?;
        let public = PublicKey::from(&secret);

        Ok(Self { public, secret })
    }

    /// Get the keypair as a dalek Keypair for signing
    pub fn as_dalek_keypair(&self) -> Keypair {
        let mut keypair_bytes = [0u8; 64];
        keypair_bytes[..32].copy_from_slice(self.secret.as_bytes());
        keypair_bytes[32..].copy_from_slice(self.public.as_bytes());

        // This should never fail as we're constructing it from valid keys
        Keypair::from_bytes(&keypair_bytes).unwrap()
    }

    /// Export the public key as bytes
    pub fn public_bytes(&self) -> [u8; 32] {
        *self.public.as_bytes()
    }

    /// Export the secret key as bytes (handle with care!)
    pub fn secret_bytes(&self) -> [u8; 32] {
        *self.secret.as_bytes()
    }

    /// Get the address derived from this keypair
    pub fn address(&self) -> [u8; 32] {
        address_from_pubkey(&self.public)
    }
}

/// Derive an address from a public key
pub fn address_from_pubkey(pubkey: &PublicKey) -> [u8; 32] {
    sha256(pubkey.as_bytes())
}

/// Serializable public key wrapper
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct VibePublicKey(pub [u8; 32]);

impl VibePublicKey {
    /// Create a new public key from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Convert to ed25519_dalek PublicKey
    pub fn to_dalek_pubkey(&self) -> Result<PublicKey, ed25519_dalek::SignatureError> {
        PublicKey::from_bytes(&self.0)
    }

    /// Get the address derived from this public key
    pub fn address(&self) -> [u8; 32] {
        match self.to_dalek_pubkey() {
            Ok(pubkey) => address_from_pubkey(&pubkey),
            Err(_) => [0u8; 32], // Should never happen with valid public key
        }
    }
}

impl From<PublicKey> for VibePublicKey {
    fn from(pubkey: PublicKey) -> Self {
        Self(*pubkey.as_bytes())
    }
}

impl TryFrom<&[u8]> for VibePublicKey {
    type Error = ed25519_dalek::SignatureError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 32 {
            return Err(ed25519_dalek::SignatureError::new());
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(bytes);
        Ok(Self(key_bytes))
    }
}

impl fmt::Debug for VibePublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VibePublicKey({})", hex::encode(&self.0[..]))
    }
}

impl fmt::Debug for VibeKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VibeKeypair {{ public: {} }}", hex::encode(self.public.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = VibeKeypair::generate();
        assert_eq!(keypair.public_bytes().len(), 32);
        assert_eq!(keypair.secret_bytes().len(), 32);
    }

    #[test]
    fn test_keypair_from_bytes() {
        let keypair = VibeKeypair::generate();
        let secret_bytes = keypair.secret_bytes();

        let restored = VibeKeypair::from_bytes(&secret_bytes).unwrap();
        assert_eq!(keypair.public_bytes(), restored.public_bytes());
    }

    #[test]
    fn test_address_derivation() {
        let keypair = VibeKeypair::generate();
        let address = keypair.address();

        // Address should be 32 bytes (SHA-256 hash)
        assert_eq!(address.len(), 32);

        // Address should be deterministic
        let address2 = address_from_pubkey(&keypair.public);
        assert_eq!(address, address2);
    }
}
