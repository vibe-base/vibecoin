use serde::{Serialize, Deserialize};
use crate::storage::block_store::Hash;

/// Target difficulty for mining
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Target([u8; 32]);

impl Target {
    /// Create a new target from a byte array
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    
    /// Create a target from a u64 difficulty
    pub fn from_difficulty(difficulty: u64) -> Self {
        // Convert a difficulty value to a target threshold
        // The higher the difficulty, the lower the target
        let max_target = [0xFF; 32];
        let mut bytes = [0u8; 32];
        
        if difficulty == 0 {
            return Self(max_target);
        }
        
        // Calculate target = max_target / difficulty
        let mut remainder = 0u128;
        for i in 0..32 {
            let dividend = (remainder << 8) + max_target[i] as u128;
            bytes[i] = (dividend / difficulty as u128) as u8;
            remainder = dividend % difficulty as u128;
        }
        
        Self(bytes)
    }
    
    /// Get the target as a byte array
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    /// Check if a hash meets the target
    pub fn is_met_by(&self, hash: &Hash) -> bool {
        // A hash meets the target if it is less than or equal to the target
        for i in 0..32 {
            if hash[i] < self.0[i] {
                return true;
            } else if hash[i] > self.0[i] {
                return false;
            }
        }
        true
    }
    
    /// Convert target back to difficulty
    pub fn to_difficulty(&self) -> u64 {
        // Simple approximation: count leading zeros and calculate 2^(leading_zeros)
        let mut leading_zeros = 0;
        for &byte in self.0.iter() {
            if byte == 0 {
                leading_zeros += 8;
            } else {
                leading_zeros += byte.leading_zeros() as u64;
                break;
            }
        }
        
        if leading_zeros >= 64 {
            return u64::MAX;
        }
        
        1u64 << leading_zeros
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_target_difficulty_conversion() {
        let difficulties = vec![1, 10, 100, 1000, 10000, 100000];
        
        for difficulty in difficulties {
            let target = Target::from_difficulty(difficulty);
            let converted = target.to_difficulty();
            
            // Due to integer division, there might be small differences
            // We'll allow a small margin of error
            let ratio = if converted > difficulty {
                converted as f64 / difficulty as f64
            } else {
                difficulty as f64 / converted as f64
            };
            
            assert!(ratio < 1.1, "Conversion error too large: {} -> {}", difficulty, converted);
        }
    }
    
    #[test]
    fn test_target_hash_check() {
        // Create a target with difficulty 16
        let target = Target::from_difficulty(16);
        
        // A hash with all zeros should meet any reasonable target
        let zero_hash = [0u8; 32];
        assert!(target.is_met_by(&zero_hash));
        
        // A hash with the first byte as 0x10 (16) should not meet the target
        // because 16 * 2^248 is greater than 2^256 / 16
        let mut hard_hash = [0u8; 32];
        hard_hash[0] = 0x10;
        assert!(!target.is_met_by(&hard_hash));
    }
}
