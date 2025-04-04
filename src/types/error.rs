use std::fmt;

#[derive(Debug)]
pub enum VibecoinError {
    InvalidTransaction,
    HashMismatch,
    InvalidBlockStructure,
    InvalidProofOfWork,
    InvalidTimestamp,
    InvalidSignature,
    ChainValidationError,
    DatabaseError(String),
    IoError(std::io::Error),
    MiningInterrupted,
    MiningTimeout,
    InvalidHash,
    InvalidPreviousHash,
    BlockchainError(String),
    NetworkError(String),
    SerializationError(String),
    InvalidBlock,
    InvalidState,
    NotSlotLeader,
    InvalidSlot,
    SlotExpired,
}

impl std::error::Error for VibecoinError {}

impl fmt::Display for VibecoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VibecoinError::InvalidTransaction => write!(f, "Invalid transaction"),
            VibecoinError::HashMismatch => write!(f, "Hash mismatch"),
            VibecoinError::InvalidBlockStructure => write!(f, "Invalid block structure"),
            VibecoinError::InvalidProofOfWork => write!(f, "Invalid proof of work"),
            VibecoinError::InvalidTimestamp => write!(f, "Invalid timestamp"),
            VibecoinError::InvalidSignature => write!(f, "Invalid signature"),
            VibecoinError::ChainValidationError => write!(f, "Chain validation failed"),
            VibecoinError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            VibecoinError::IoError(err) => write!(f, "IO error: {}", err),
            VibecoinError::MiningInterrupted => write!(f, "Mining was interrupted"),
            VibecoinError::MiningTimeout => write!(f, "Mining timed out"),
            VibecoinError::InvalidHash => write!(f, "Invalid block hash"),
            VibecoinError::InvalidPreviousHash => write!(f, "Invalid previous hash"),
            VibecoinError::BlockchainError(msg) => write!(f, "Blockchain error: {}", msg),
            VibecoinError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            VibecoinError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            VibecoinError::InvalidBlock => write!(f, "Invalid block"),
            VibecoinError::InvalidState => write!(f, "Invalid blockchain state"),
            VibecoinError::NotSlotLeader => write!(f, "Not the slot leader"),
            VibecoinError::InvalidSlot => write!(f, "Invalid slot"),
            VibecoinError::SlotExpired => write!(f, "Slot has expired"),
        }
    }
}