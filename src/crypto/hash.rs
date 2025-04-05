use sha2::{Sha256, Digest};
use crate::types::primitives::Hash;

pub fn sha256_bytes(data: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

pub fn sha256(data: &str) -> Hash {
    sha256_bytes(data.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_empty_string() {
        let hash = sha256(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_hello_world() {
        let hash = sha256(b"Hello, world!");
        assert_eq!(
            hash,
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_sha256_bytes() {
        let hash = sha256_bytes(b"Hello, world!");
        assert_eq!(
            hex::encode(hash),
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_sha256_consistency() {
        let data = b"test data";
        let hash1 = sha256_bytes(data);
        let hash2 = sha256_bytes(data);
        assert_eq!(hash1, hash2, "Hash function should be deterministic");
    }
}
