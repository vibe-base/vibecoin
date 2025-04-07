//! Fork choice rule for consensus
//!
//! This module defines the fork choice rule used by the consensus mechanism.

use crate::storage::block_store::Block;

/// Fork choice rule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkChoice {
    /// Choose the chain with the highest total difficulty
    HighestTotalDifficulty,

    /// Choose the chain with the most recent block
    MostRecent,

    /// Choose the chain with the most blocks
    MostBlocks,

    /// Choose the chain with the most PoH entries
    MostPoH,
}

impl Default for ForkChoice {
    fn default() -> Self {
        ForkChoice::HighestTotalDifficulty
    }
}

impl ForkChoice {
    /// Compare two blocks according to the fork choice rule
    pub fn compare(&self, block1: &Block, block2: &Block) -> std::cmp::Ordering {
        match self {
            ForkChoice::HighestTotalDifficulty => {
                block1.total_difficulty.cmp(&block2.total_difficulty)
            }
            ForkChoice::MostRecent => {
                block1.timestamp.cmp(&block2.timestamp)
            }
            ForkChoice::MostBlocks => {
                block1.height.cmp(&block2.height)
            }
            ForkChoice::MostPoH => {
                block1.poh_seq.cmp(&block2.poh_seq)
            }
        }
    }

    /// Choose the best block from a list of blocks
    pub fn choose_best<'a>(&self, blocks: &'a [Block]) -> Option<&'a Block> {
        if blocks.is_empty() {
            return None;
        }

        let mut best_block = &blocks[0];

        for block in blocks.iter().skip(1) {
            if self.compare(block, best_block) == std::cmp::Ordering::Greater {
                best_block = block;
            }
        }

        Some(best_block)
    }
}
