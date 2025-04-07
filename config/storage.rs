use serde::{Serialize, Deserialize};

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Database path
    pub db_path: String,
    
    /// Cache size in MB
    pub cache_size: usize,
    
    /// Maximum open files
    pub max_open_files: i32,
    
    /// Write buffer size in MB
    pub write_buffer_size: usize,
    
    /// Maximum write buffer number
    pub max_write_buffer_number: i32,
    
    /// Enable WAL (Write Ahead Log)
    pub enable_wal: bool,
    
    /// Enable statistics
    pub enable_statistics: bool,
    
    /// Enable compression
    pub enable_compression: bool,
    
    /// Compression type
    pub compression_type: String,
    
    /// Enable bloom filters
    pub enable_bloom_filters: bool,
    
    /// Bloom filter bits per key
    pub bloom_filter_bits_per_key: i32,
    
    /// Enable automatic compaction
    pub enable_auto_compaction: bool,
    
    /// Compaction style
    pub compaction_style: String,
    
    /// Enable pruning
    pub enable_pruning: bool,
    
    /// Pruning keep recent blocks
    pub pruning_keep_recent: u64,
    
    /// Pruning interval in blocks
    pub pruning_interval: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "./data/vibecoin/db".to_string(),
            cache_size: 512, // 512MB
            max_open_files: 1000,
            write_buffer_size: 64, // 64MB
            max_write_buffer_number: 3,
            enable_wal: true,
            enable_statistics: false,
            enable_compression: true,
            compression_type: "lz4".to_string(),
            enable_bloom_filters: true,
            bloom_filter_bits_per_key: 10,
            enable_auto_compaction: true,
            compaction_style: "level".to_string(),
            enable_pruning: false,
            pruning_keep_recent: 10000, // Keep last 10000 blocks
            pruning_interval: 100, // Run pruning every 100 blocks
        }
    }
}
