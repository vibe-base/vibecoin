use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::cmp::Ordering;

use crate::crypto::signer::VibeSignature;

/// Type alias for address (public key hash)
pub type Address = [u8; 32];

/// Type alias for transaction hash
pub type Hash = [u8; 32];

/// Transaction record structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionRecord {
    /// Unique transaction ID (hash)
    pub tx_id: Hash,
    
    /// Sender address
    pub sender: Address,
    
    /// Recipient address
    pub recipient: Address,
    
    /// Transaction value
    pub value: u64,
    
    /// Gas price (fee per gas unit)
    pub gas_price: u64,
    
    /// Gas limit (maximum gas units)
    pub gas_limit: u64,
    
    /// Account nonce (prevents replay attacks)
    pub nonce: u64,
    
    /// Timestamp when the transaction was created
    pub timestamp: u64,
    
    /// Transaction signature
    pub signature: VibeSignature,
    
    /// Optional data payload
    pub data: Option<Vec<u8>>,
}

impl TransactionRecord {
    /// Create a new transaction record
    pub fn new(
        sender: Address,
        recipient: Address,
        value: u64,
        gas_price: u64,
        gas_limit: u64,
        nonce: u64,
        data: Option<Vec<u8>>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // In a real implementation, we would compute the tx_id based on the transaction contents
        // For now, we'll just use a placeholder
        let tx_id = [0u8; 32];
        
        // In a real implementation, the signature would be created by the sender
        // For now, we'll just use a placeholder
        let signature = VibeSignature::new([0u8; 64]);
        
        Self {
            tx_id,
            sender,
            recipient,
            value,
            gas_price,
            gas_limit,
            nonce,
            timestamp,
            signature,
            data,
        }
    }
    
    /// Get the total gas cost (gas_price * gas_limit)
    pub fn gas_cost(&self) -> u64 {
        self.gas_price * self.gas_limit
    }
    
    /// Get the total transaction cost (value + gas_cost)
    pub fn total_cost(&self) -> u64 {
        self.value + self.gas_cost()
    }
    
    /// Check if the transaction is expired
    pub fn is_expired(&self, max_age_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.timestamp > max_age_secs
    }
}

/// Transaction status in the mempool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    /// Transaction is pending in the mempool
    Pending,
    
    /// Transaction is included in a block
    Included,
    
    /// Transaction is rejected
    Rejected,
    
    /// Transaction is expired
    Expired,
}

/// Implement Ord for TransactionRecord to enable priority queue
impl Ord for TransactionRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by gas price (higher gas price has higher priority)
        other.gas_price.cmp(&self.gas_price)
            // Then compare by timestamp (older transactions have higher priority)
            .then(self.timestamp.cmp(&other.timestamp))
    }
}

impl PartialOrd for TransactionRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TransactionRecord {
    fn eq(&self, other: &Self) -> bool {
        self.tx_id == other.tx_id
    }
}

impl Eq for TransactionRecord {}
