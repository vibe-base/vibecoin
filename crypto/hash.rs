use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use std::fmt;

/// Compute SHA-256 hash of data
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Compute double SHA-256 hash (SHA-256 of SHA-256)
pub fn double_sha256(data: &[u8]) -> [u8; 32] {
    sha256(&sha256(data))
}

/// A 32-byte hash value
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Create a new hash from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    /// Create a hash by hashing the provided data
    pub fn from_data(data: &[u8]) -> Self {
        Self(sha256(data))
    }
    
    /// Create a hash by double-hashing the provided data
    pub fn from_data_double(data: &[u8]) -> Self {
        Self(double_sha256(data))
    }
    
    /// Get the hash as bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    /// Create a zero hash (all zeros)
    pub fn zero() -> Self {
        Self([0u8; 32])
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 32]> for Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", hex::encode(&self.0))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sha256() {
        // Test vector from https://www.di-mgt.com.au/sha_testvectors.html
        let input = b"abc";
        let expected = hex::decode("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").unwrap();
        
        let result = sha256(input);
        assert_eq!(result.to_vec(), expected);
    }
    
    #[test]
    fn test_double_sha256() {
        let input = b"vibecoin";
        let single_hash = sha256(input);
        let double_hash = double_sha256(input);
        
        // Double hash should be different from single hash
        assert_ne!(single_hash, double_hash);
        
        // Double hash should equal SHA-256 of single hash
        assert_eq!(double_hash, sha256(&single_hash));
    }
    
    #[test]
    fn test_hash_struct() {
        let data = b"test data";
        let hash = Hash::from_data(data);
        
        // Hash should be 32 bytes
        assert_eq!(hash.as_bytes().len(), 32);
        
        // Hash should be deterministic
        let hash2 = Hash::from_data(data);
        assert_eq!(hash, hash2);
        
        // Different data should produce different hashes
        let hash3 = Hash::from_data(b"different data");
        assert_ne!(hash, hash3);
    }
}
