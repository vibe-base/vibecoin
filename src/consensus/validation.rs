use crate::ledger::block::Block;
use crate::types::error::VibecoinError;
use crate::consensus::poh::ProofOfHistory;
use crate::consensus::pow::{self, Difficulty};

/// Comprehensive block validation that includes:
/// 1. Basic block structure validation
/// 2. PoW validation (hash meets difficulty)
/// 3. PoH validation (replay ticks to verify)
/// 4. Transaction validation
/// 5. Block sequence validation (if previous block provided)
pub fn validate_block(
    block: &Block,
    difficulty: Difficulty,
    poh_state: &ProofOfHistory,
    previous_block: Option<&Block>,
) -> Result<(), VibecoinError> {
    println!("Validating block {}...", block.index);

    // Step 1: Validate basic block structure and transactions
    block.is_valid()?;
    println!("✓ Basic block structure and transactions valid");

    // Step 2: Validate PoW (hash meets difficulty)
    pow::verify_pow(block, difficulty)?;
    println!("✓ Proof of Work valid (difficulty {})", difficulty);

    // Step 3: Validate PoH segment
    pow::verify_poh_proof(block, poh_state)?;
    println!("✓ Proof of History segment valid");

    // Step 4: Validate block sequence if previous block provided
    if let Some(prev_block) = previous_block {
        block.verify_sequence(prev_block)?;
        println!("✓ Block sequence valid");
    } else {
        println!("ℹ️ No previous block provided for sequence validation");
    }

    println!("Block {} fully validated!", block.index);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::transaction::Transaction;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn test_validate_mined_block() -> Result<(), VibecoinError> {
        // Create a test block
        let previous_hash = [0u8; 32];
        let transactions = Vec::new();
        let mut block = Block::new(1, previous_hash, transactions)?;

        // Create a PoH instance
        let initial_hash = [0u8; 32];
        let mut poh = ProofOfHistory::new(initial_hash, 1000);

        // Mine with very low difficulty (should be fast)
        let difficulty = 1;
        let max_time = Some(Duration::from_secs(1));

        // Mine the block
        pow::mine_block(&mut block, difficulty, &mut poh, None, max_time)?;

        // Validate the block
        validate_block(&block, difficulty, &poh, None)?;

        Ok(())
    }
}
