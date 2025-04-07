use crate::storage::block_store::Hash;
use crate::consensus::poh::PoHGenerator;

/// Verifier for Proof of History
pub struct PoHVerifier {
    // Add any state needed for verification
}

impl PoHVerifier {
    /// Create a new PoH verifier
    pub fn new() -> Self {
        Self {}
    }
    
    /// Verify a PoH sequence
    pub fn verify_sequence(&self, start_hash: &Hash, end_hash: &Hash, steps: u64) -> bool {
        // Use the PoHGenerator to verify the sequence
        PoHGenerator::verify_sequence(start_hash, end_hash, steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Sha256, Digest};
    
    #[test]
    fn test_verify_sequence() {
        let verifier = PoHVerifier::new();
        
        // Create a start hash
        let start_hash = [1u8; 32];
        
        // Generate a sequence of 10 hashes
        let mut hash = start_hash;
        for _ in 0..10 {
            let mut hasher = Sha256::new();
            hasher.update(&hash);
            let result = hasher.finalize();
            hash.copy_from_slice(&result);
        }
        
        // Verify the sequence
        assert!(verifier.verify_sequence(&start_hash, &hash, 10));
        
        // Verify that an incorrect sequence fails
        let wrong_hash = [2u8; 32];
        assert!(!verifier.verify_sequence(&start_hash, &wrong_hash, 10));
        
        // Verify that an incorrect step count fails
        assert!(!verifier.verify_sequence(&start_hash, &hash, 9));
        assert!(!verifier.verify_sequence(&start_hash, &hash, 11));
    }
}
