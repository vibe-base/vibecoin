use log::{info, warn};

use crate::consensus::types::Target;
use crate::consensus::config::ConsensusConfig;
use crate::storage::block_store::BlockStore;

/// Calculate the next target difficulty
pub fn calculate_next_target(
    config: &ConsensusConfig,
    block_store: &BlockStore<'_>,
    current_height: u64,
) -> Target {
    // If we're at the genesis block or below the adjustment window, use the initial difficulty
    if current_height < config.difficulty_adjustment_window {
        return Target::from_difficulty(config.initial_difficulty);
    }

    // Get the block at the start of the window
    let start_height = current_height - config.difficulty_adjustment_window;
    let start_block = match block_store.get_block_by_height(start_height) {
        Ok(Some(block)) => block,
        Ok(None) => {
            warn!("Could not find block at height {}, using initial difficulty", start_height);
            return Target::from_difficulty(config.initial_difficulty);
        },
        Err(_e) => {
            warn!("Could not find block at height {}, using initial difficulty", start_height);
            return Target::from_difficulty(config.initial_difficulty);
        }
    };

    // Get the latest block
    let latest_block = match block_store.get_block_by_height(current_height) {
        Ok(Some(block)) => block,
        Ok(None) => {
            warn!("Could not find block at height {}, using initial difficulty", current_height);
            return Target::from_difficulty(config.initial_difficulty);
        },
        Err(_e) => {
            warn!("Could not find block at height {}, using initial difficulty", current_height);
            return Target::from_difficulty(config.initial_difficulty);
        }
    };

    // Calculate the time taken for the window
    let time_taken = latest_block.timestamp - start_block.timestamp;

    // Expected time for the window
    let expected_time = config.target_block_time * config.difficulty_adjustment_window;

    // If time_taken is zero, something is wrong, use the initial difficulty
    if time_taken == 0 {
        warn!("Time taken for difficulty window is zero, using initial difficulty");
        return Target::from_difficulty(config.initial_difficulty);
    }

    // Calculate the adjustment factor
    let adjustment_factor = expected_time as f64 / time_taken as f64;

    // Clamp the adjustment factor to prevent too rapid changes
    let clamped_factor = adjustment_factor.max(0.25).min(4.0);

    // Get the current difficulty
    let current_target = get_target_at_height(block_store, current_height);
    let current_difficulty = current_target.to_difficulty();

    // Calculate the new difficulty
    let new_difficulty = (current_difficulty as f64 * clamped_factor) as u64;

    // Ensure the difficulty doesn't go below the initial difficulty
    let new_difficulty = new_difficulty.max(config.initial_difficulty);

    info!(
        "Difficulty adjustment: {} -> {} (factor: {:.2})",
        current_difficulty, new_difficulty, clamped_factor
    );

    Target::from_difficulty(new_difficulty)
}

/// Get the target at a specific height
pub fn get_target_at_height(block_store: &BlockStore<'_>, height: u64) -> Target {
    // Get the block at the specified height
    let _block = match block_store.get_block_by_height(height) {
        Ok(Some(block)) => block,
        Ok(None) => {
            warn!("Could not find block at height {}, using default target", height);
            return Target::from_difficulty(1);
        },
        Err(_e) => {
            warn!("Could not find block at height {}, using default target", height);
            return Target::from_difficulty(1);
        }
    };

    // In a real implementation, the target would be stored in the block
    // For now, we'll just use a placeholder
    Target::from_difficulty(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::{KVStore, RocksDBStore};
    use tempfile::tempdir;

    #[test]
    fn test_difficulty_adjustment() {
        // Create a temporary directory for the database
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = BlockStore::new(&kv_store);

        // Create a config
        let config = ConsensusConfig::default()
            .with_target_block_time(10)
            .with_difficulty_adjustment_window(2);

        // Create some test blocks
        // Block 0 (genesis) at time 0
        let block0 = Block {
            height: 0,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        // Block 1 at time 10 (on target)
        let block1 = Block {
            height: 1,
            hash: [1u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 10,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        // Block 2 at time 30 (slower than target)
        let block2 = Block {
            height: 2,
            hash: [2u8; 32],
            prev_hash: [1u8; 32],
            timestamp: 30,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        // Store the blocks
        block_store.put_block(&block0);
        block_store.put_block(&block1);
        block_store.put_block(&block2);

        // Calculate the next target
        let target = calculate_next_target(&config, &block_store, 2);

        // The blocks took longer than expected, so difficulty should decrease
        // Expected time: 10 * 2 = 20
        // Actual time: 30 - 0 = 30
        // Adjustment factor: 20 / 30 = 0.67
        // New difficulty: initial_difficulty * 0.67
        let expected_difficulty = (config.initial_difficulty as f64 * 0.67) as u64;

        // Allow for some floating point imprecision
        let actual_difficulty = target.to_difficulty();
        let ratio = if actual_difficulty > expected_difficulty {
            actual_difficulty as f64 / expected_difficulty as f64
        } else {
            expected_difficulty as f64 / actual_difficulty as f64
        };

        assert!(ratio < 1.01, "Difficulty adjustment error too large");
    }
}
