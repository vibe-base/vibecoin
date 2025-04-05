use crate::types::primitives::{PublicKey, Signature, Amount, Hash};
use crate::types::error::VibecoinError;
use crate::crypto::hash::sha256_bytes;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Transaction {
    pub from: PublicKey,
    pub to: PublicKey,
    pub amount: Amount,
    pub timestamp: u64,
    pub signature: Arc<Signature>, // Use Arc for large signature data
    pub hash: Hash,
}

impl Transaction {
    pub fn new(from: PublicKey, to: PublicKey, amount: Amount) -> Result<Self, VibecoinError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VibecoinError::InvalidTimestamp)?
            .as_secs();

        if amount == 0 {
            return Err(VibecoinError::InvalidTransaction);
        }

        let mut tx = Transaction {
            from,
            to,
            amount,
            timestamp,
            signature: Arc::new([0u8; 64]),
            hash: [0u8; 32],
        };

        tx.hash = tx.calculate_hash();
        Ok(tx)
    }

    pub fn validate(&self) -> Result<(), VibecoinError> {
        // Verify timestamp
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VibecoinError::InvalidTimestamp)?
            .as_secs();

        if self.timestamp > current_time {
            return Err(VibecoinError::InvalidTimestamp);
        }

        // Verify amount
        if self.amount == 0 {
            return Err(VibecoinError::InvalidTransaction);
        }

        // Verify signature (placeholder)
        if self.signature == Arc::new([0u8; 64]) {
            return Err(VibecoinError::InvalidSignature);
        }

        Ok(())
    }

    pub fn is_valid(&self) -> Result<(), VibecoinError> {
        // Special case for coinbase transactions (from address is all zeros)
        let is_coinbase = self.from == [0u8; 32];

        if is_coinbase {
            // Coinbase transactions must have a valid destination
            if self.to == [0u8; 32] {
                return Err(VibecoinError::InvalidTransaction);
            }

            // Coinbase transactions should have a special signature
            if self.signature != Arc::new([2u8; 64]) {
                return Err(VibecoinError::InvalidSignature);
            }
        } else {
            // Regular transactions need valid addresses
            if self.from == [0u8; 32] || self.to == [0u8; 32] {
                return Err(VibecoinError::InvalidTransaction);
            }

            // Prevent self-transfers
            if self.from == self.to {
                return Err(VibecoinError::InvalidTransaction);
            }
        }

        // Validate amount
        if self.amount == 0 {
            return Err(VibecoinError::InvalidTransaction);
        }

        // Validate timestamp
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| VibecoinError::InvalidTimestamp)?
            .as_secs();

        if self.timestamp > current_time {
            return Err(VibecoinError::InvalidTimestamp);
        }

        // Validate hash
        if self.calculate_hash() != self.hash {
            return Err(VibecoinError::HashMismatch);
        }

        // Verify cryptographic signature
        self.verify_signature()?;

        Ok(())
    }

    fn verify_signature(&self) -> Result<(), VibecoinError> {
        // Special case for coinbase transactions
        if self.from == [0u8; 32] {
            // Coinbase transactions should have a special signature
            if *self.signature != [2u8; 64] {
                return Err(VibecoinError::InvalidSignature);
            }
            return Ok(());
        }

        // For regular transactions, check that the signature is not empty
        if *self.signature == [0u8; 64] {
            return Err(VibecoinError::InvalidSignature);
        }

        // In a real implementation, we would verify the signature
        // against the transaction data and the sender's public key
        Ok(())
    }

    // Helper method to create message for signature verification
    #[allow(dead_code)]
    fn to_signing_message(&self) -> Vec<u8> {
        let mut message = Vec::with_capacity(72); // 32 + 32 + 8 bytes
        message.extend_from_slice(&self.from);
        message.extend_from_slice(&self.to);
        message.extend_from_slice(&self.amount.to_le_bytes());
        message.extend_from_slice(&self.timestamp.to_le_bytes());
        message
    }

    // Optimize hash calculation
    pub fn calculate_hash(&self) -> Hash {
        let mut data = Vec::with_capacity(104); // Pre-allocate exact size

        data.extend_from_slice(&self.from);
        data.extend_from_slice(&self.to);
        data.extend_from_slice(&self.amount.to_le_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());

        sha256_bytes(&data)
    }

    // Implement Clone manually for efficiency
    pub fn clone(&self) -> Self {
        Transaction {
            from: self.from,
            to: self.to,
            amount: self.amount,
            timestamp: self.timestamp,
            signature: Arc::clone(&self.signature),
            hash: self.hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_transaction() -> Result<Transaction, VibecoinError> {
        let from = [1u8; 32];
        let to = [2u8; 32];
        let amount = 100;
        let mut tx = Transaction::new(from, to, amount)?;
        tx.signature = Arc::new([1u8; 64]); // Set non-zero signature for test
        Ok(tx)
    }

    #[test]
    fn test_valid_transaction() -> Result<(), VibecoinError> {
        let tx = create_test_transaction()?;
        tx.is_valid()?;
        Ok(())
    }

    #[test]
    fn test_invalid_self_transfer() -> Result<(), VibecoinError> {
        let address = [1u8; 32];
        let amount = 100;

        let mut tx = Transaction::new(address, address, amount)?;
        tx.signature = Arc::new([1u8; 64]);

        assert!(matches!(
            tx.is_valid(),
            Err(VibecoinError::InvalidTransaction)
        ));
        Ok(())
    }

    #[test]
    fn test_invalid_zero_amount() -> Result<(), VibecoinError> {
        let from = [1u8; 32];
        let to = [2u8; 32];
        let amount = 0;

        let result = Transaction::new(from, to, amount);
        assert!(matches!(result, Err(VibecoinError::InvalidTransaction)));
        Ok(())
    }

    #[test]
    fn test_invalid_signature() -> Result<(), VibecoinError> {
        let mut tx = create_test_transaction()?;
        tx.signature = Arc::new([0u8; 64]);

        assert!(matches!(tx.is_valid(), Err(VibecoinError::InvalidSignature)));
        Ok(())
    }

    #[test]
    fn test_hash_calculation() -> Result<(), VibecoinError> {
        let tx = create_test_transaction()?;
        let hash1 = tx.calculate_hash();
        let hash2 = tx.calculate_hash();

        assert_eq!(hash1, hash2, "Hash calculation should be deterministic");
        assert_ne!(hash1, [0u8; 32], "Hash should not be zero");
        Ok(())
    }

    #[test]
    fn test_future_timestamp() -> Result<(), VibecoinError> {
        let mut tx = create_test_transaction()?;
        tx.timestamp = u64::MAX; // Far future timestamp

        assert!(matches!(tx.is_valid(), Err(VibecoinError::InvalidTimestamp)));
        Ok(())
    }
}
