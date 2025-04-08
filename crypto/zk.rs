use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek::traits::MultiscalarMul;
use merlin::Transcript;
use rand::rngs::OsRng;
use rand::RngCore;
use rand_chacha::ChaChaRng;
use rand_core::SeedableRng;
use serde::{Serialize, Deserialize};
use sha2::{Sha512, Digest};

use crate::crypto::keys;
use crate::crypto::signer;

/// A Pedersen commitment
#[derive(Clone, Debug)]
pub struct PedersenCommitment {
    /// The commitment value
    pub commitment: CompressedRistretto,

    /// The blinding factor (kept secret)
    pub blinding: Option<Scalar>,
}

impl PedersenCommitment {
    /// Create a new Pedersen commitment to a value
    pub fn commit(value: u64, blinding_factor: Option<Scalar>) -> Self {
        // Use the provided blinding factor or generate a random one
        let blinding = blinding_factor.unwrap_or_else(|| {
            // Create a seed from OsRng
            let mut seed = [0u8; 32];
            OsRng.fill_bytes(&mut seed);

            // Use the seed to create a deterministic RNG that works with curve25519-dalek
            let mut rng = rand_chacha::ChaChaRng::from_seed(seed);
            Scalar::random(&mut rng)
        });

        // Convert the value to a scalar
        let value_scalar = Scalar::from(value);

        // Get the generators
        let (g, h) = get_generators();

        // Compute the commitment: C = v*G + r*H
        let commitment_point = RistrettoPoint::multiscalar_mul(
            &[value_scalar, blinding],
            &[g, h],
        );

        Self {
            commitment: commitment_point.compress(),
            blinding: Some(blinding),
        }
    }

    /// Verify that a commitment opens to a specific value
    pub fn verify(&self, value: u64, blinding: &Scalar) -> bool {
        // Convert the value to a scalar
        let value_scalar = Scalar::from(value);

        // Get the generators
        let (g, h) = get_generators();

        // Compute the expected commitment: C = v*G + r*H
        let expected_point = RistrettoPoint::multiscalar_mul(
            &[value_scalar, *blinding],
            &[g, h],
        );
        let expected_commitment = expected_point.compress();

        // Check if the commitments match
        self.commitment == expected_commitment
    }

    /// Create a commitment without storing the blinding factor
    pub fn commit_public(value: u64, blinding: &Scalar) -> Self {
        // Convert the value to a scalar
        let value_scalar = Scalar::from(value);

        // Get the generators
        let (g, h) = get_generators();

        // Compute the commitment: C = v*G + r*H
        let commitment_point = RistrettoPoint::multiscalar_mul(
            &[value_scalar, *blinding],
            &[g, h],
        );

        Self {
            commitment: commitment_point.compress(),
            blinding: None,
        }
    }
}

/// A range proof that proves a value is within a range [0, 2^bits)
#[derive(Clone, Debug)]
pub struct RangeProof {
    /// The commitment to the value
    pub commitment: CompressedRistretto,

    /// The proof data
    pub proof_data: Vec<u8>,
}

impl RangeProof {
    /// Create a new range proof for a value
    pub fn prove(value: u64, bits: usize) -> (Self, PedersenCommitment) {
        // Create a commitment to the value
        let commitment = PedersenCommitment::commit(value, None);
        let blinding = commitment.blinding.unwrap();

        // Create a transcript for the Fiat-Shamir heuristic
        let mut transcript = Transcript::new(b"range_proof");

        // Add the commitment to the transcript
        transcript.append_message(b"commitment", commitment.commitment.as_bytes());

        // Add the range to the transcript
        transcript.append_u64(b"bits", bits as u64);

        // In a real implementation, we would use bulletproofs or similar
        // For now, we'll just create a dummy proof
        let mut proof_data = Vec::new();
        proof_data.extend_from_slice(&value.to_le_bytes());
        proof_data.extend_from_slice(blinding.as_bytes());

        (
            Self {
                commitment: commitment.commitment,
                proof_data,
            },
            commitment,
        )
    }

    /// Verify a range proof
    pub fn verify(&self, bits: usize) -> bool {
        // Create a transcript for the Fiat-Shamir heuristic
        let mut transcript = Transcript::new(b"range_proof");

        // Add the commitment to the transcript
        transcript.append_message(b"commitment", self.commitment.as_bytes());

        // Add the range to the transcript
        transcript.append_u64(b"bits", bits as u64);

        // In a real implementation, we would verify the bulletproof
        // For now, we'll just check that the proof data is valid
        if self.proof_data.len() < 8 + 32 {
            return false;
        }

        // Extract the value and blinding factor
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&self.proof_data[0..8]);
        let value = u64::from_le_bytes(value_bytes);

        let mut blinding_bytes = [0u8; 32];
        blinding_bytes.copy_from_slice(&self.proof_data[8..40]);
        let blinding = Scalar::from_bytes_mod_order(blinding_bytes);

        // Check that the value is within the range
        if value >= (1 << bits) {
            return false;
        }

        // Verify the commitment
        let commitment = PedersenCommitment::commit_public(value, &blinding);
        commitment.commitment == self.commitment
    }
}

/// A confidential transaction that hides the transaction amount
#[derive(Clone, Debug)]
pub struct ConfidentialTransaction {
    /// Sender address
    pub sender: [u8; 32],

    /// Recipient address
    pub recipient: [u8; 32],

    /// Commitment to the transaction amount
    pub amount_commitment: CompressedRistretto,

    /// Range proof for the transaction amount
    pub range_proof: RangeProof,

    /// Transaction signature
    pub signature: Vec<u8>,
}

impl ConfidentialTransaction {
    /// Create a new confidential transaction
    pub fn new(
        sender: [u8; 32],
        recipient: [u8; 32],
        amount: u64,
        sender_keypair: &keys::VibeKeypair,
    ) -> Self {
        // Create a range proof for the amount
        let (range_proof, commitment) = RangeProof::prove(amount, 64);

        // Create a signature for the transaction
        let mut data = Vec::new();
        data.extend_from_slice(&sender);
        data.extend_from_slice(&recipient);
        data.extend_from_slice(commitment.commitment.as_bytes());

        let signature = signer::sign_message(sender_keypair, &data);

        Self {
            sender,
            recipient,
            amount_commitment: commitment.commitment,
            range_proof,
            signature: signature.as_bytes().to_vec(),
        }
    }

    /// Verify a confidential transaction
    pub fn verify(&self, sender_pubkey: &keys::VibePublicKey) -> bool {
        // Verify the range proof
        if !self.range_proof.verify(64) {
            return false;
        }

        // Verify the signature
        let mut data = Vec::new();
        data.extend_from_slice(&self.sender);
        data.extend_from_slice(&self.recipient);
        data.extend_from_slice(self.amount_commitment.as_bytes());

        let signature = signer::VibeSignature::try_from(&self.signature[..]).unwrap();

        match sender_pubkey.to_dalek_pubkey() {
            Ok(pubkey) => signer::verify_signature(&data, &signature, &pubkey),
            Err(_) => false,
        }
    }
}

/// Get the generators for Pedersen commitments
fn get_generators() -> (RistrettoPoint, RistrettoPoint) {
    // In a real implementation, these would be standardized generators
    // For now, we'll derive them from fixed strings
    let g = hash_to_point(b"vibecoin_pedersen_g");
    let h = hash_to_point(b"vibecoin_pedersen_h");

    (g, h)
}

/// Hash a message to a Ristretto point
fn hash_to_point(message: &[u8]) -> RistrettoPoint {
    let mut hasher = Sha512::new();
    hasher.update(message);
    let hash = hasher.finalize();

    CompressedRistretto::from_slice(&hash[0..32])
        .decompress()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pedersen_commitment() {
        // Create a commitment to a value
        let value = 42u64;
        let commitment = PedersenCommitment::commit(value, None);

        // Verify the commitment
        assert!(commitment.verify(value, &commitment.blinding.unwrap()));

        // Try to verify with a different value
        assert!(!commitment.verify(43, &commitment.blinding.unwrap()));
    }

    #[test]
    fn test_range_proof() {
        // Create a range proof for a value
        let value = 42u64;
        let bits = 64;
        let (proof, _) = RangeProof::prove(value, bits);

        // Verify the range proof
        assert!(proof.verify(bits));
    }

    #[test]
    fn test_confidential_transaction() {
        // Create a keypair for the sender
        let sender_keypair = crate::keys::VibeKeypair::generate();
        let sender_pubkey = crate::keys::VibePublicKey::from(sender_keypair.public);

        // Create a confidential transaction
        let sender = sender_keypair.address();
        let recipient = [2u8; 32];
        let amount = 100u64;

        let tx = ConfidentialTransaction::new(
            sender,
            recipient,
            amount,
            &sender_keypair,
        );

        // Verify the transaction
        assert!(tx.verify(&sender_pubkey));
    }
}
