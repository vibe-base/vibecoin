use blst::min_pk::{SecretKey, PublicKey, Signature};
use blst::{BLST_ERROR, Pairing};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Serialize, Deserialize};
use std::fmt;
use std::convert::TryFrom;

/// BLS keypair for signing and verification
pub struct BlsKeypair {
    /// Public key for verification
    pub public: PublicKey,

    /// Secret key for signing
    pub secret: SecretKey,
}

impl BlsKeypair {
    /// Generate a new random keypair
    pub fn generate() -> Self {
        let mut ikm = [0u8; 32];
        OsRng.fill_bytes(&mut ikm);

        let secret = SecretKey::key_gen(&ikm, &[]).unwrap();
        let public = secret.sk_to_pk();

        Self { public, secret }
    }

    /// Create a keypair from a secret key
    pub fn from_secret(secret: SecretKey) -> Self {
        let public = secret.sk_to_pk();
        Self { public, secret }
    }

    /// Get the public key
    pub fn public_key(&self) -> &PublicKey {
        &self.public
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> BlsSignature {
        let dst = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";
        let sig = self.secret.sign(message, dst, &[]);
        BlsSignature(sig)
    }
}

/// A BLS signature
#[derive(Clone)]
pub struct BlsSignature(pub Signature);

impl BlsSignature {
    /// Verify the signature against a message and public key
    pub fn verify(&self, message: &[u8], public_key: &PublicKey) -> bool {
        let dst = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";
        // The verify method has changed in newer versions of blst
        let result = self.0.verify(false, message, dst, &[], public_key, false);
        result == BLST_ERROR::BLST_SUCCESS
    }

    /// Serialize the signature to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_bytes().to_vec()
    }

    /// Deserialize the signature from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        match Signature::from_bytes(bytes) {
            Ok(sig) => Ok(BlsSignature(sig)),
            Err(_) => Err("Invalid signature bytes".to_string()),
        }
    }
}

/// A serializable BLS public key
#[derive(Clone, Serialize, Deserialize)]
pub struct BlsPublicKey(pub Vec<u8>);

impl BlsPublicKey {
    /// Create a new BLS public key from a PublicKey
    pub fn new(public_key: &PublicKey) -> Self {
        Self(public_key.to_bytes().to_vec())
    }

    /// Convert to a PublicKey
    pub fn to_public_key(&self) -> Result<PublicKey, String> {
        match PublicKey::from_bytes(&self.0) {
            Ok(pk) => Ok(pk),
            Err(_) => Err("Invalid public key bytes".to_string()),
        }
    }
}

impl fmt::Debug for BlsPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlsPublicKey({})", hex::encode(&self.0))
    }
}

/// Aggregate multiple BLS signatures into a single signature
pub fn aggregate_signatures(signatures: &[BlsSignature]) -> Option<BlsSignature> {
    if signatures.is_empty() {
        return None;
    }

    // In newer blst versions, we need to use the Signature::aggregate method
    let sig_refs: Vec<&Signature> = signatures.iter().map(|s| &s.0).collect();
    let agg_sig = Signature::aggregate(&sig_refs, false);

    match agg_sig {
        Ok(sig) => Some(BlsSignature(sig)),
        Err(_) => None,
    }
}

/// Aggregate multiple BLS public keys into a single public key
pub fn aggregate_public_keys(public_keys: &[PublicKey]) -> Option<PublicKey> {
    if public_keys.is_empty() {
        return None;
    }

    // In newer blst versions, we need to use the PublicKey::aggregate method
    let pk_refs: Vec<&PublicKey> = public_keys.iter().collect();
    let agg_pk = PublicKey::aggregate(&pk_refs, false);

    match agg_pk {
        Ok(pk) => Some(pk),
        Err(_) => None,
    }
}

/// Verify an aggregate signature against multiple messages and public keys
pub fn verify_aggregate_signature(
    signature: &BlsSignature,
    messages: &[&[u8]],
    public_keys: &[PublicKey],
) -> bool {
    if messages.len() != public_keys.len() || messages.is_empty() {
        return false;
    }

    let dst = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

    // Create a pairing context for verification
    let mut ctx = Pairing::new(false, dst);

    // Add each message and public key to the context
    for (i, (msg, pk)) in messages.iter().zip(public_keys.iter()).enumerate() {
        if let Err(_) = ctx.aggregate(&pk, false, *msg, &[]) {
            return false;
        }
    }

    // Verify the aggregate signature
    let result = ctx.verify(false, &signature.0);
    result == BLST_ERROR::BLST_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = BlsKeypair::generate();
        assert_ne!(keypair.public.to_bytes(), [0u8; 48]);
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = BlsKeypair::generate();
        let message = b"Test message";

        // Sign the message
        let signature = keypair.sign(message);

        // Verify the signature
        assert!(signature.verify(message, &keypair.public));

        // Try with a different message
        let different_message = b"Different message";
        assert!(!signature.verify(different_message, &keypair.public));

        // Try with a different keypair
        let different_keypair = BlsKeypair::generate();
        assert!(!signature.verify(message, &different_keypair.public));
    }

    #[test]
    fn test_signature_serialization() {
        let keypair = BlsKeypair::generate();
        let message = b"Test message";

        // Sign the message
        let signature = keypair.sign(message);

        // Serialize and deserialize
        let bytes = signature.to_bytes();
        let deserialized = BlsSignature::from_bytes(&bytes).unwrap();

        // Verify the deserialized signature
        assert!(deserialized.verify(message, &keypair.public));
    }

    #[test]
    fn test_public_key_serialization() {
        let keypair = BlsKeypair::generate();

        // Serialize and deserialize
        let bls_pk = BlsPublicKey::new(&keypair.public);
        let deserialized = bls_pk.to_public_key().unwrap();

        // Verify they're the same
        assert_eq!(keypair.public.to_bytes(), deserialized.to_bytes());
    }

    #[test]
    fn test_signature_aggregation() {
        // Create multiple keypairs
        let keypair1 = BlsKeypair::generate();
        let keypair2 = BlsKeypair::generate();
        let keypair3 = BlsKeypair::generate();

        // Create a single message
        let message = b"Test message";

        // Sign the message with each keypair
        let sig1 = keypair1.sign(message);
        let sig2 = keypair2.sign(message);
        let sig3 = keypair3.sign(message);

        // Aggregate the signatures
        let agg_sig = aggregate_signatures(&[sig1, sig2, sig3]).unwrap();

        // Aggregate the public keys
        let agg_pk = aggregate_public_keys(&[
            keypair1.public.clone(),
            keypair2.public.clone(),
            keypair3.public.clone(),
        ]).unwrap();

        // Verify the aggregate signature with the aggregate public key
        assert!(agg_sig.verify(message, &agg_pk));
    }

    #[test]
    fn test_multi_signature_verification() {
        // Create multiple keypairs
        let keypair1 = BlsKeypair::generate();
        let keypair2 = BlsKeypair::generate();
        let keypair3 = BlsKeypair::generate();

        // Create different messages for each keypair
        let message1 = b"Message 1";
        let message2 = b"Message 2";
        let message3 = b"Message 3";

        // Sign each message with the corresponding keypair
        let sig1 = keypair1.sign(message1);
        let sig2 = keypair2.sign(message2);
        let sig3 = keypair3.sign(message3);

        // Aggregate the signatures
        let agg_sig = aggregate_signatures(&[sig1, sig2, sig3]).unwrap();

        // Verify the aggregate signature
        let result = verify_aggregate_signature(
            &agg_sig,
            &[message1, message2, message3],
            &[
                keypair1.public.clone(),
                keypair2.public.clone(),
                keypair3.public.clone(),
            ],
        );

        assert!(result);

        // Try with a wrong message
        let wrong_message = b"Wrong message";
        let result = verify_aggregate_signature(
            &agg_sig,
            &[message1, wrong_message, message3],
            &[
                keypair1.public.clone(),
                keypair2.public.clone(),
                keypair3.public.clone(),
            ],
        );

        assert!(!result);
    }
}
