//! Block template for mining
//!
//! This module defines the block template used for mining.

use crate::storage::block_store::{Block, Hash};
use crate::storage::tx_store::TransactionRecord;
use crate::consensus::types::Target;

/// Block template for mining
#[derive(Debug, Clone)]
pub struct BlockTemplate {
    /// Block height
    pub height: u64,

    /// Previous block hash
    pub prev_hash: Hash,

    /// Timestamp
    pub timestamp: u64,

    /// Transactions to include
    pub transactions: Vec<TransactionRecord>,

    /// State root
    pub state_root: Hash,

    /// Transaction root
    pub tx_root: Hash,

    /// PoH sequence number
    pub poh_seq: u64,

    /// Previous block's PoH sequence number
    pub prev_poh_seq: u64,

    /// PoH hash
    pub poh_hash: Hash,

    /// Mining target
    pub target: Target,

    /// Total difficulty
    pub total_difficulty: u128,

    /// Miner address
    pub miner: Hash,
}

impl BlockTemplate {
    /// Create a new block template
    pub fn new(
        height: u64,
        prev_hash: Hash,
        timestamp: u64,
        transactions: Vec<TransactionRecord>,
        state_root: Hash,
        tx_root: Hash,
        poh_seq: u64,
        prev_poh_seq: u64,
        poh_hash: Hash,
        target: Target,
        total_difficulty: u64,
        miner: Hash,
    ) -> Self {
        Self {
            height,
            prev_hash,
            timestamp,
            transactions,
            state_root,
            tx_root,
            poh_seq,
            prev_poh_seq,
            poh_hash,
            target,
            total_difficulty: total_difficulty.into(),
            miner,
        }
    }

    /// Convert to a block
    pub fn to_block(&self, nonce: u64, hash: Hash) -> Block {
        // Ensure the PoH sequence is increasing
        let poh_seq = if self.height <= 1 {
            // For genesis block or the first block after genesis, use the PoH sequence from the template
            self.poh_seq
        } else {
            // For blocks after the first block, ensure the sequence is increasing
            // Use max to ensure it's always greater than the previous block's sequence
            std::cmp::max(self.poh_seq, self.prev_poh_seq + 1)
        };

        // Generate the correct PoH hash for the block
        // For a valid PoH entry with an event, the hash should be the SHA-256 hash of
        // the concatenation of prev_poh_hash and the sequence difference
        let poh_hash = if self.height <= 1 {
            // For genesis block or the first block after genesis, we use the PoH hash from the template
            self.poh_hash
        } else {
            // For blocks after the first block, we need to generate the PoH hash
            let seq_diff = poh_seq - self.prev_poh_seq;
            let event_data = seq_diff.to_be_bytes();
            let combined = [&self.prev_hash[..], &event_data[..]].concat();
            crate::crypto::hash::sha256(&combined)
        };

        Block {
            height: self.height,
            hash,
            prev_hash: self.prev_hash,
            timestamp: self.timestamp,
            transactions: self.transactions.iter().map(|tx| tx.tx_id.clone()).collect(),
            state_root: self.state_root,
            tx_root: self.tx_root,
            nonce,
            poh_seq,
            poh_hash,
            difficulty: self.target.to_difficulty(),
            total_difficulty: self.total_difficulty,
        }
    }
}
