use serde::{Serialize, Deserialize};

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Initial difficulty
    pub initial_difficulty: u64,
    
    /// Difficulty adjustment interval (in blocks)
    pub difficulty_adjustment_interval: u64,
    
    /// Target block time (in seconds)
    pub target_block_time: u64,
    
    /// Maximum future time (in seconds)
    pub max_future_time: u64,
    
    /// PoH tick interval (in milliseconds)
    pub poh_tick_interval: u64,
    
    /// Number of confirmations required for finality
    pub confirmations_for_finality: u64,
    
    /// Maximum number of transactions per block
    pub max_transactions_per_block: usize,
    
    /// Mining thread count (0 = use all available cores)
    pub mining_threads: usize,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            initial_difficulty: 1000,
            difficulty_adjustment_interval: 2016, // ~2 weeks with 10-minute blocks
            target_block_time: 600,               // 10 minutes
            max_future_time: 7200,                // 2 hours
            poh_tick_interval: 50,                // 50 milliseconds
            confirmations_for_finality: 6,
            max_transactions_per_block: 5000,
            mining_threads: 0,                    // Use all available cores
        }
    }
}
