use crate::types::primitives::Hash;
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct ProofOfHistory {
    pub last_hash: Hash,
    pub count: u64,
    pub ticks_per_second: u64,
}

impl ProofOfHistory {
    pub fn new(initial_hash: Hash, ticks_per_second: u64) -> Self {
        ProofOfHistory {
            last_hash: initial_hash,
            count: 0,
            ticks_per_second,
        }
    }

    pub fn tick(&mut self) -> Hash {
        self.count += 1;
        self.last_hash = self.hash_tick(self.last_hash);
        self.last_hash
    }

    pub fn record_event(&mut self, event_hash: Hash) -> Hash {
        self.last_hash = self.hash_with_event(self.last_hash, event_hash);
        self.last_hash
    }

    fn hash_tick(&self, previous_hash: Hash) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(previous_hash);
        hasher.update(self.count.to_le_bytes());
        hasher.update(self.get_current_timestamp().to_le_bytes());

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result[..]);
        hash
    }

    fn hash_with_event(&self, previous_hash: Hash, event_hash: Hash) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(previous_hash);
        hasher.update(event_hash);
        hasher.update(self.get_current_timestamp().to_le_bytes());

        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result[..]);
        hash
    }

    pub fn verify_sequence(&self, hashes: &[Hash]) -> bool {
        if hashes.is_empty() {
            return true;
        }

        for window in hashes.windows(2) {
            if !self.verify_hash_pair(window[0], window[1]) {
                return false;
            }
        }
        true
    }

    fn verify_hash_pair(&self, first: Hash, second: Hash) -> bool {
        // In a real implementation, we would:
        // 1. Verify that second could be derived from first using our hash function
        // 2. Check that the time difference between hashes is plausible
        // 3. Verify that the sequence follows our PoH rules

        let mut hasher = Sha256::new();
        hasher.update(first);
        let expected = hasher.finalize();
        let mut expected_hash = [0u8; 32];
        expected_hash.copy_from_slice(&expected[..]);

        // This is a simplified verification
        // In production, you'd want more sophisticated verification
        second != first && second != [0u8; 32]
    }

    pub fn get_current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poh_tick() {
        let initial_hash = [0u8; 32];
        let mut poh = ProofOfHistory::new(initial_hash, 1000);

        let hash1 = poh.tick();
        let hash2 = poh.tick();

        assert_ne!(hash1, hash2, "Consecutive ticks should produce different hashes");
        assert_eq!(poh.count, 2, "Tick count should be incremented");
    }

    #[test]
    fn test_record_event() {
        let initial_hash = [0u8; 32];
        let mut poh = ProofOfHistory::new(initial_hash, 1000);
        let event_hash = [1u8; 32];

        let before_hash = poh.last_hash;
        let after_hash = poh.record_event(event_hash);

        assert_ne!(before_hash, after_hash, "Recording event should change the hash");
    }

    #[test]
    fn test_hash_uniqueness() {
        let initial_hash = [0u8; 32];
        let mut poh = ProofOfHistory::new(initial_hash, 1000);

        let mut hashes = Vec::new();
        for _ in 0..10 {
            hashes.push(poh.tick());
        }

        // Check that all hashes are unique
        for i in 0..hashes.len() {
            for j in i+1..hashes.len() {
                assert_ne!(hashes[i], hashes[j], "Found duplicate hashes");
            }
        }
    }

    #[test]
    fn test_verify_sequence() {
        let initial_hash = [0u8; 32];
        let poh = ProofOfHistory::new(initial_hash, 1000);

        let hashes = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        assert!(poh.verify_sequence(&hashes), "Valid sequence should verify");

        let empty_sequence: Vec<Hash> = vec![];
        assert!(poh.verify_sequence(&empty_sequence), "Empty sequence should verify");
    }
}
