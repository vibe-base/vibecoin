use crate::types::primitives::Hash;
use crate::types::error::VibecoinError;
use crate::ledger::block::Block;
use crate::ledger::state::Blockchain;
use crate::consensus::pow::{self, Difficulty};
use crate::consensus::poh::ProofOfHistory;

// Weight factor for PoH quality in fork resolution
const POH_QUALITY_WEIGHT: f64 = 0.01;

// Fork resolution strategy
pub enum ForkStrategy {
    // Choose the chain with the highest cumulative PoW
    HighestWork,
    // Choose the chain with the best PoH quality if work is equal
    BestPoHQuality,
    // Choose the chain with the lowest hash if work and PoH quality are equal
    LowestHash,
}

// Calculate the total work (cumulative difficulty) of a chain
pub fn calculate_total_work(chain: &[Block], difficulty: Difficulty) -> u64 {
    // In a simple implementation, each block contributes its difficulty to the total work
    // In a more sophisticated implementation, we would calculate the actual work based on
    // the difficulty target and the actual hash
    
    // For now, we'll use a simple model: work = difficulty * number of blocks
    (difficulty as u64) * (chain.len() as u64)
}

// Calculate the PoH quality factor (0.0 to 1.0)
pub fn calculate_poh_quality(chain: &[Block], poh: &ProofOfHistory) -> f64 {
    // In a real implementation, we would analyze the PoH stream for:
    // 1. Continuity (no gaps)
    // 2. Correct timing
    // 3. Proper sequencing
    
    // For this implementation, we'll use a simple heuristic:
    // - Check what percentage of blocks have valid PoH proofs
    
    let mut valid_proofs = 0;
    
    for block in chain {
        if let Some(proof) = block.poh_proof {
            if proof != [0u8; 32] {
                // In a real implementation, we would verify the proof more thoroughly
                valid_proofs += 1;
            }
        }
    }
    
    if chain.is_empty() {
        return 1.0;
    }
    
    valid_proofs as f64 / chain.len() as f64
}

// Calculate a fork score that combines work and PoH quality
pub fn calculate_fork_score(chain: &[Block], difficulty: Difficulty, poh: &ProofOfHistory) -> f64 {
    let total_work = calculate_total_work(chain, difficulty) as f64;
    let poh_quality = calculate_poh_quality(chain, poh);
    
    // Combine the scores, with work being the dominant factor
    total_work + (poh_quality * POH_QUALITY_WEIGHT * total_work)
}

// Compare two chains and determine which one is better
pub fn compare_chains(
    chain_a: &[Block],
    chain_b: &[Block],
    difficulty: Difficulty,
    poh: &ProofOfHistory,
    strategy: ForkStrategy,
) -> ChainComparison {
    // First, compare by total work (primary criterion)
    let work_a = calculate_total_work(chain_a, difficulty);
    let work_b = calculate_total_work(chain_b, difficulty);
    
    if work_a > work_b {
        return ChainComparison::FirstChainBetter;
    } else if work_b > work_a {
        return ChainComparison::SecondChainBetter;
    }
    
    // If work is equal, use the specified strategy
    match strategy {
        ForkStrategy::HighestWork => {
            // We already compared work, so they're equal
            ChainComparison::Equal
        },
        ForkStrategy::BestPoHQuality => {
            // Compare PoH quality
            let quality_a = calculate_poh_quality(chain_a, poh);
            let quality_b = calculate_poh_quality(chain_b, poh);
            
            if quality_a > quality_b {
                ChainComparison::FirstChainBetter
            } else if quality_b > quality_a {
                ChainComparison::SecondChainBetter
            } else {
                ChainComparison::Equal
            }
        },
        ForkStrategy::LowestHash => {
            // Compare the hash of the last block
            if chain_a.is_empty() && chain_b.is_empty() {
                return ChainComparison::Equal;
            } else if chain_a.is_empty() {
                return ChainComparison::SecondChainBetter;
            } else if chain_b.is_empty() {
                return ChainComparison::FirstChainBetter;
            }
            
            let last_hash_a = chain_a.last().unwrap().hash;
            let last_hash_b = chain_b.last().unwrap().hash;
            
            // Compare hashes lexicographically
            for i in 0..32 {
                if last_hash_a[i] < last_hash_b[i] {
                    return ChainComparison::FirstChainBetter;
                } else if last_hash_b[i] < last_hash_a[i] {
                    return ChainComparison::SecondChainBetter;
                }
            }
            
            ChainComparison::Equal
        }
    }
}

// Result of comparing two chains
#[derive(Debug, PartialEq)]
pub enum ChainComparison {
    FirstChainBetter,
    SecondChainBetter,
    Equal,
}

// Resolve a fork between two blockchains
pub fn resolve_fork(
    chain_a: &Blockchain,
    chain_b: &Blockchain,
) -> ChainComparison {
    // First try highest work strategy
    let result = compare_chains(
        &chain_a.chain,
        &chain_b.chain,
        chain_a.difficulty, // Assuming both chains have the same difficulty
        &chain_a.poh,       // Using chain_a's PoH for quality calculation
        ForkStrategy::HighestWork
    );
    
    if result != ChainComparison::Equal {
        return result;
    }
    
    // If equal work, try PoH quality
    let result = compare_chains(
        &chain_a.chain,
        &chain_b.chain,
        chain_a.difficulty,
        &chain_a.poh,
        ForkStrategy::BestPoHQuality
    );
    
    if result != ChainComparison::Equal {
        return result;
    }
    
    // If still equal, use lowest hash
    compare_chains(
        &chain_a.chain,
        &chain_b.chain,
        chain_a.difficulty,
        &chain_a.poh,
        ForkStrategy::LowestHash
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::block::Block;
    use std::time::Duration;
    
    fn create_test_block(index: u64, previous_hash: Hash) -> Result<Block, VibecoinError> {
        let transactions = Vec::new();
        Block::new(index, previous_hash, transactions)
    }
    
    #[test]
    fn test_calculate_total_work() -> Result<(), VibecoinError> {
        let mut blocks = Vec::new();
        let previous_hash = [0u8; 32];
        
        // Create 5 blocks
        for i in 0..5 {
            let prev_hash = if i == 0 { previous_hash } else { blocks[i-1].hash };
            let block = create_test_block(i as u64, prev_hash)?;
            blocks.push(block);
        }
        
        let difficulty = 2;
        let total_work = calculate_total_work(&blocks, difficulty);
        
        // Expected work = difficulty * number of blocks
        assert_eq!(total_work, 2 * 5);
        
        Ok(())
    }
    
    #[test]
    fn test_calculate_poh_quality() -> Result<(), VibecoinError> {
        let mut blocks = Vec::new();
        let previous_hash = [0u8; 32];
        
        // Create 4 blocks
        for i in 0..4 {
            let prev_hash = if i == 0 { previous_hash } else { blocks[i-1].hash };
            let mut block = create_test_block(i as u64, prev_hash)?;
            
            // Set PoH proof for some blocks
            if i % 2 == 0 {
                block.poh_proof = Some([1u8; 32]);
            }
            
            blocks.push(block);
        }
        
        let poh = ProofOfHistory::new([0u8; 32], 1000);
        let quality = calculate_poh_quality(&blocks, &poh);
        
        // 2 out of 4 blocks have valid proofs
        assert_eq!(quality, 0.5);
        
        Ok(())
    }
    
    #[test]
    fn test_compare_chains() -> Result<(), VibecoinError> {
        // Create two chains with different lengths
        let mut chain_a = Vec::new();
        let mut chain_b = Vec::new();
        let previous_hash = [0u8; 32];
        
        // Chain A: 5 blocks
        for i in 0..5 {
            let prev_hash = if i == 0 { previous_hash } else { chain_a[i-1].hash };
            let block = create_test_block(i as u64, prev_hash)?;
            chain_a.push(block);
        }
        
        // Chain B: 3 blocks
        for i in 0..3 {
            let prev_hash = if i == 0 { previous_hash } else { chain_b[i-1].hash };
            let block = create_test_block(i as u64, prev_hash)?;
            chain_b.push(block);
        }
        
        let poh = ProofOfHistory::new([0u8; 32], 1000);
        let difficulty = 2;
        
        // Compare by work
        let result = compare_chains(&chain_a, &chain_b, difficulty, &poh, ForkStrategy::HighestWork);
        assert_eq!(result, ChainComparison::FirstChainBetter);
        
        // Create two chains with equal length but different PoH quality
        let mut chain_c = Vec::new();
        let mut chain_d = Vec::new();
        
        // Chain C: 3 blocks, all with PoH proofs
        for i in 0..3 {
            let prev_hash = if i == 0 { previous_hash } else { chain_c[i-1].hash };
            let mut block = create_test_block(i as u64, prev_hash)?;
            block.poh_proof = Some([1u8; 32]);
            chain_c.push(block);
        }
        
        // Chain D: 3 blocks, only 1 with PoH proof
        for i in 0..3 {
            let prev_hash = if i == 0 { previous_hash } else { chain_d[i-1].hash };
            let mut block = create_test_block(i as u64, prev_hash)?;
            if i == 0 {
                block.poh_proof = Some([1u8; 32]);
            }
            chain_d.push(block);
        }
        
        // Compare by PoH quality
        let result = compare_chains(&chain_c, &chain_d, difficulty, &poh, ForkStrategy::BestPoHQuality);
        assert_eq!(result, ChainComparison::FirstChainBetter);
        
        Ok(())
    }
    
    #[test]
    fn test_resolve_fork() -> Result<(), VibecoinError> {
        // Create genesis block
        let genesis = Block::new(0, [0u8; 32], Vec::new())?;
        
        // Create initial PoH
        let initial_hash = [0u8; 32];
        let poh = ProofOfHistory::new(initial_hash, 1000);
        
        // Create two blockchains
        let mut chain_a = Blockchain::new(genesis.clone(), 2, poh.clone());
        let mut chain_b = Blockchain::new(genesis.clone(), 2, poh.clone());
        
        // Add more blocks to chain A
        let mut prev_hash = genesis.hash;
        for i in 1..4 {
            let mut block = create_test_block(i, prev_hash)?;
            block.poh_proof = Some([i as u8; 32]);
            chain_a.add_block(block.clone())?;
            prev_hash = block.hash;
        }
        
        // Add fewer blocks to chain B
        prev_hash = genesis.hash;
        for i in 1..3 {
            let mut block = create_test_block(i, prev_hash)?;
            block.poh_proof = Some([i as u8; 32]);
            chain_b.add_block(block.clone())?;
            prev_hash = block.hash;
        }
        
        // Resolve fork
        let result = resolve_fork(&chain_a, &chain_b);
        assert_eq!(result, ChainComparison::FirstChainBetter);
        
        Ok(())
    }
}
