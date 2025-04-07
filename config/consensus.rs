use serde::{Serialize, Deserialize};

/// Consensus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Chain ID
    pub chain_id: u64,
    
    /// Genesis block hash
    pub genesis_hash: Option<String>,
    
    /// Enable mining
    pub enable_mining: bool,
    
    /// Mining threads
    pub mining_threads: usize,
    
    /// Target block time in seconds
    pub target_block_time: u64,
    
    /// Initial difficulty
    pub initial_difficulty: u64,
    
    /// Difficulty adjustment interval (in blocks)
    pub difficulty_adjustment_interval: u64,
    
    /// Maximum transactions per block
    pub max_transactions_per_block: usize,
    
    /// Maximum block size in bytes
    pub max_block_size: usize,
    
    /// Maximum gas per block
    pub max_gas_per_block: u64,
    
    /// Gas price minimum
    pub gas_price_minimum: u64,
    
    /// Enable Proof of History
    pub enable_poh: bool,
    
    /// PoH tick interval in milliseconds
    pub poh_tick_interval: u64,
    
    /// PoH ticks per block
    pub poh_ticks_per_block: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            chain_id: 1,
            genesis_hash: None,
            enable_mining: true,
            mining_threads: 4,
            target_block_time: 10,
            initial_difficulty: 1000,
            difficulty_adjustment_interval: 2016, // ~2 weeks with 10s blocks
            max_transactions_per_block: 10000,
            max_block_size: 1024 * 1024, // 1MB
            max_gas_per_block: 10_000_000,
            gas_price_minimum: 1,
            enable_poh: true,
            poh_tick_interval: 10, // 10ms
            poh_ticks_per_block: 1000, // 10s worth of ticks
        }
    }
}
