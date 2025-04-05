use crate::types::primitives::Hash;
use crate::types::error::VibecoinError;
use crate::ledger::block::Block;
use crate::consensus::poh::ProofOfHistory;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// Difficulty is represented as the number of leading zeros required in the hash
pub type Difficulty = u8;

// Check if a hash meets the difficulty requirement
pub fn meets_difficulty(hash: &Hash, difficulty: Difficulty) -> bool {
    // Check that the first 'difficulty' bits are zero
    for i in 0..difficulty as usize / 8 {
        if hash[i] != 0 {
            return false;
        }
    }

    // Check remaining bits if difficulty is not a multiple of 8
    if difficulty as usize % 8 != 0 {
        let mask = 0xff << (8 - difficulty as usize % 8);
        if hash[difficulty as usize / 8] & mask != 0 {
            return false;
        }
    }

    true
}

// Mine a block with PoW and include a PoH segment during a slot
pub fn mine_block(
    block: &mut Block,
    difficulty: Difficulty,
    poh: &mut ProofOfHistory,
    slot_number: u64,
    slot_leader: Hash,
    stop_signal: Option<Arc<AtomicBool>>,
    max_time: Option<Duration>,
) -> Result<(), VibecoinError> {
    let start_time = Instant::now();
    let mut nonce: u64 = 0;
    let poh_start_hash = poh.last_hash;

    // Set slot information in the block
    block.slot_number = slot_number;
    block.slot_leader = Some(slot_leader);

    // Record the block's pre-mining hash in the PoH sequence
    // We use calculate_hash_without_nonce to get the state before mining
    let block_data_hash = block.calculate_hash_without_nonce();
    poh.record_event(block_data_hash);

    // Generate a few PoH ticks to create a time segment
    for _ in 0..5 {
        poh.tick();
    }

    // Store the PoH hash after the segment
    let poh_end_hash = poh.last_hash;

    // Include the PoH proof in the block
    block.poh_proof = Some(poh_end_hash);

    println!("Mining block {} in slot {} with difficulty {}", block.index, slot_number, difficulty);
    println!("Slot leader: {}", hex::encode(slot_leader));
    println!("PoH segment: {} -> {}", hex::encode(poh_start_hash), hex::encode(poh_end_hash));

    // Start mining loop
    loop {
        // Check if we should stop
        if let Some(signal) = &stop_signal {
            if signal.load(Ordering::Relaxed) {
                return Err(VibecoinError::MiningInterrupted);
            }
        }

        // Check if we've exceeded max time
        if let Some(max_duration) = max_time {
            if start_time.elapsed() > max_duration {
                return Err(VibecoinError::MiningTimeout);
            }
        }

        // Try a new nonce
        block.nonce = nonce;
        let hash = block.calculate_hash();

        // Check if the hash meets the difficulty requirement
        if meets_difficulty(&hash, difficulty) {
            block.hash = hash;
            println!("Block mined! Nonce: {}, Hash: {}", nonce, hex::encode(hash));
            return Ok(());
        }

        // Increment nonce and continue
        nonce += 1;

        // Periodically yield to other threads
        if nonce % 100_000 == 0 {
            thread::yield_now();

            // Print progress every million attempts
            if nonce % 1_000_000 == 0 {
                println!("Mining... Tried {} nonces", nonce);
            }
        }
    }
}

// Verify that a block meets the PoW difficulty requirement
pub fn verify_pow(block: &Block, difficulty: Difficulty) -> Result<(), VibecoinError> {
    if !meets_difficulty(&block.hash, difficulty) {
        return Err(VibecoinError::InvalidProofOfWork);
    }
    Ok(())
}

// Verify that the PoH proof in the block is valid
pub fn verify_poh_proof(block: &Block, poh_state: &ProofOfHistory) -> Result<(), VibecoinError> {
    if let Some(proof) = block.poh_proof {
        // Check that the proof is not empty
        if proof == [0u8; 32] {
            return Err(VibecoinError::InvalidProofOfWork);
        }

        // Check that the block has a slot leader
        if block.slot_leader.is_none() {
            println!("PoH verification failed: Block has no slot leader");
            return Err(VibecoinError::InvalidProofOfWork);
        }

        // In a full implementation, we would:
        // 1. Verify the block's data was recorded in the PoH sequence
        // 2. Replay the PoH ticks to reach the expected proof hash
        // 3. Verify the timing of the PoH segment
        // 4. Verify the slot leader was correctly selected

        // For demonstration, let's implement a simplified version of this verification
        let block_data_hash = block.calculate_hash_without_nonce(); // Hash without nonce to get pre-mining state

        // Create a temporary PoH instance to replay the segment
        let mut temp_poh = ProofOfHistory::new(poh_state.last_hash, poh_state.ticks_per_second);

        // Record the block's data in the PoH sequence
        temp_poh.record_event(block_data_hash);

        // Generate the expected number of ticks (5 in our implementation)
        for _ in 0..5 {
            temp_poh.tick();
        }

        // Verify the final hash matches the proof in the block
        if temp_poh.last_hash != proof {
            println!("PoH verification failed: Expected proof {}, got {}",
                     hex::encode(temp_poh.last_hash), hex::encode(proof));
            return Err(VibecoinError::InvalidProofOfWork);
        }

        println!("PoH verification for block in slot {} PASSED", block.slot_number);
        Ok(())
    } else {
        // Genesis block doesn't need PoH proof
        if block.index == 0 {
            return Ok(());
        }
        Err(VibecoinError::InvalidProofOfWork)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::transaction::Transaction;

    #[test]
    fn test_meets_difficulty() {
        // Test with zero difficulty (always meets)
        let hash = [0xff; 32];
        assert!(meets_difficulty(&hash, 0));

        // Test with difficulty 8 (first byte must be 0)
        let hash1 = [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert!(meets_difficulty(&hash1, 8));

        // Test with difficulty 8 (fails)
        let hash2 = [1, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert!(!meets_difficulty(&hash2, 8));

        // Test with difficulty 12 (first byte and half of second byte must be 0)
        let hash3 = [0, 0x0f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert!(meets_difficulty(&hash3, 12));

        // Test with difficulty 12 (fails)
        let hash4 = [0, 0x10, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        assert!(!meets_difficulty(&hash4, 12));
    }

    #[test]
    fn test_mine_block_with_low_difficulty() -> Result<(), VibecoinError> {
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
        let slot_number = 1;
        let slot_leader = [1u8; 32];

        // Mine the block
        mine_block(&mut block, difficulty, &mut poh, slot_number, slot_leader, None, max_time)?;

        // Verify the block meets the difficulty
        verify_pow(&block, difficulty)?;

        // Verify the block has a PoH proof
        assert!(block.poh_proof.is_some());

        // Verify slot information
        assert_eq!(block.slot_number, slot_number);
        assert_eq!(block.slot_leader, Some(slot_leader));

        Ok(())
    }
}
