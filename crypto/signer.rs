use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use serde::{Serialize, Deserialize};
use std::convert::TryFrom;
use std::fmt;

use crate::crypto::keys::VibeKeypair;

/// Sign a message using the provided keypair
pub fn sign_message(keypair: &VibeKeypair, message: &[u8]) -> VibeSignature {
    let dalek_keypair = keypair.as_dalek_keypair();
    let signature = dalek_keypair.sign(message);
    VibeSignature::new(*signature.as_bytes())
}

/// Verify a signature against a message and public key
pub fn verify_signature(
    message: &[u8],
    signature: &VibeSignature,
    public_key: &PublicKey,
) -> bool {
    match Signature::from_bytes(&signature.0) {
        Ok(sig) => public_key.verify(message, &sig).is_ok(),
        Err(_) => false,
    }
}

/// A serializable signature wrapper
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VibeSignature(pub [u8; 64]);

impl VibeSignature {
    /// Create a new signature from bytes
    pub fn new(bytes: [u8; 64]) -> Self {
        Self(bytes)
    }
    
    /// Convert to ed25519_dalek Signature
    pub fn to_dalek_signature(&self) -> Result<Signature, ed25519_dalek::SignatureError> {
        Signature::from_bytes(&self.0)
    }
    
    /// Get the signature as bytes
    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }
}

impl TryFrom<&[u8]> for VibeSignature {
    type Error = ed25519_dalek::SignatureError;
    
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != 64 {
            return Err(ed25519_dalek::SignatureError::new());
        }
        
        let mut sig_bytes = [0u8; 64];
        sig_bytes.copy_from_slice(bytes);
        Ok(Self(sig_bytes))
    }
}

impl fmt::Debug for VibeSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VibeSignature({})", hex::encode(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::VibeKeypair;
    
    #[test]
    fn test_sign_and_verify() {
        let keypair = VibeKeypair::generate();
        let message = b"This is a test message";
        
        // Sign the message
        let signature = sign_message(&keypair, message);
        
        // Verify the signature
        let result = verify_signature(message, &signature, &keypair.public);
        assert!(result);
    }
    
    #[test]
    fn test_invalid_signature() {
        let keypair = VibeKeypair::generate();
        let message = b"This is a test message";
        
        // Sign the message
        let signature = sign_message(&keypair, message);
        
        // Try to verify with a different message
        let different_message = b"This is a different message";
        let result = verify_signature(different_message, &signature, &keypair.public);
        assert!(!result);
        
        // Try to verify with a different keypair
        let different_keypair = VibeKeypair::generate();
        let result = verify_signature(message, &signature, &different_keypair.public);
        assert!(!result);
    }
    
    #[test]
    fn test_signature_serialization() {
        let keypair = VibeKeypair::generate();
        let message = b"This is a test message";
        
        // Sign the message
        let signature = sign_message(&keypair, message);
        
        // Convert to bytes and back
        let sig_bytes = signature.as_bytes();
        let restored = VibeSignature::try_from(&sig_bytes[..]).unwrap();
        
        // Should be the same signature
        assert_eq!(signature, restored);
        
        // Should still verify
        let result = verify_signature(message, &restored, &keypair.public);
        assert!(result);
    }
}
