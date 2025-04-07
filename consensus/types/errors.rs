use thiserror::Error;

/// Consensus error types
#[derive(Debug, Error)]
pub enum ConsensusError {
    /// Invalid Proof of Work
    #[error("Invalid Proof of Work: hash does not meet target")]
    InvalidPoW,
    
    /// Invalid Proof of History
    #[error("Invalid Proof of History: {0}")]
    InvalidPoH(String),
    
    /// Invalid previous hash
    #[error("Invalid previous hash: expected {expected}, got {actual}")]
    InvalidPrevHash {
        expected: String,
        actual: String,
    },
    
    /// Invalid state root
    #[error("Invalid state root: expected {expected}, got {actual}")]
    InvalidStateRoot {
        expected: String,
        actual: String,
    },
    
    /// Invalid transaction root
    #[error("Invalid transaction root: expected {expected}, got {actual}")]
    InvalidTxRoot {
        expected: String,
        actual: String,
    },
    
    /// Invalid block height
    #[error("Invalid block height: expected {expected}, got {actual}")]
    InvalidHeight {
        expected: u64,
        actual: u64,
    },
    
    /// Invalid timestamp
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),
    
    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),
    
    /// Other error
    #[error("Consensus error: {0}")]
    Other(String),
}
