use std::time::Duration;

/// Configuration for the consensus module
#[derive(Clone, Debug)]
pub struct ConsensusConfig {
    /// Target time between blocks in seconds
    pub target_block_time: u64,
    
    /// Initial mining difficulty
    pub initial_difficulty: u64,
    
    /// Number of blocks to consider for difficulty adjustment
    pub difficulty_adjustment_window: u64,
    
    /// Maximum number of transactions per block
    pub max_transactions_per_block: usize,
    
    /// PoH tick rate (hashes per second)
    pub poh_tick_rate: u64,
    
    /// Whether to enable mining
    pub enable_mining: bool,
    
    /// Number of mining threads to use
    pub mining_threads: usize,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            target_block_time: 15,
            initial_difficulty: 100,
            difficulty_adjustment_window: 10,
            max_transactions_per_block: 1000,
            poh_tick_rate: 400_000, // 400k hashes per second
            enable_mining: false,
            mining_threads: num_cpus::get().max(1),
        }
    }
}

impl ConsensusConfig {
    /// Create a new consensus configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the target block time
    pub fn with_target_block_time(mut self, seconds: u64) -> Self {
        self.target_block_time = seconds;
        self
    }
    
    /// Set the initial difficulty
    pub fn with_initial_difficulty(mut self, difficulty: u64) -> Self {
        self.initial_difficulty = difficulty;
        self
    }
    
    /// Set the difficulty adjustment window
    pub fn with_difficulty_adjustment_window(mut self, blocks: u64) -> Self {
        self.difficulty_adjustment_window = blocks;
        self
    }
    
    /// Set the maximum transactions per block
    pub fn with_max_transactions_per_block(mut self, max: usize) -> Self {
        self.max_transactions_per_block = max;
        self
    }
    
    /// Set the PoH tick rate
    pub fn with_poh_tick_rate(mut self, rate: u64) -> Self {
        self.poh_tick_rate = rate;
        self
    }
    
    /// Enable or disable mining
    pub fn with_mining_enabled(mut self, enabled: bool) -> Self {
        self.enable_mining = enabled;
        self
    }
    
    /// Set the number of mining threads
    pub fn with_mining_threads(mut self, threads: usize) -> Self {
        self.mining_threads = threads;
        self
    }
    
    /// Get the target block time as a Duration
    pub fn target_block_time_duration(&self) -> Duration {
        Duration::from_secs(self.target_block_time)
    }
    
    /// Get the PoH tick interval
    pub fn poh_tick_interval(&self) -> Duration {
        if self.poh_tick_rate == 0 {
            Duration::from_micros(1) // Avoid division by zero
        } else {
            Duration::from_secs(1) / self.poh_tick_rate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = ConsensusConfig::default();
        assert_eq!(config.target_block_time, 15);
        assert_eq!(config.initial_difficulty, 100);
        assert_eq!(config.difficulty_adjustment_window, 10);
        assert_eq!(config.max_transactions_per_block, 1000);
        assert_eq!(config.poh_tick_rate, 400_000);
        assert_eq!(config.enable_mining, false);
        assert!(config.mining_threads > 0);
    }
    
    #[test]
    fn test_config_builder() {
        let config = ConsensusConfig::new()
            .with_target_block_time(30)
            .with_initial_difficulty(200)
            .with_difficulty_adjustment_window(20)
            .with_max_transactions_per_block(500)
            .with_poh_tick_rate(200_000)
            .with_mining_enabled(true)
            .with_mining_threads(4);
        
        assert_eq!(config.target_block_time, 30);
        assert_eq!(config.initial_difficulty, 200);
        assert_eq!(config.difficulty_adjustment_window, 20);
        assert_eq!(config.max_transactions_per_block, 500);
        assert_eq!(config.poh_tick_rate, 200_000);
        assert_eq!(config.enable_mining, true);
        assert_eq!(config.mining_threads, 4);
    }
    
    #[test]
    fn test_time_conversions() {
        let config = ConsensusConfig::default();
        
        // Test target block time
        let duration = config.target_block_time_duration();
        assert_eq!(duration, Duration::from_secs(15));
        
        // Test PoH tick interval
        let interval = config.poh_tick_interval();
        let expected = Duration::from_secs(1) / 400_000;
        assert_eq!(interval, expected);
    }
}
