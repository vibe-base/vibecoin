use serde::{Serialize, Deserialize};
use std::fs;
use std::path::Path;
use log::{info, warn, error};

mod network;
mod consensus;
mod storage;
mod node;

pub use network::NetworkConfig;
pub use consensus::ConsensusConfig;
pub use storage::StorageConfig;
pub use node::NodeConfig;

/// Main configuration for the VibeCoin node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Node configuration
    pub node: NodeConfig,
    
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
}

impl Config {
    /// Create a new configuration with default values
    pub fn default() -> Self {
        Self {
            node: NodeConfig::default(),
            network: NetworkConfig::default(),
            consensus: ConsensusConfig::default(),
            storage: StorageConfig::default(),
        }
    }
    
    /// Load configuration from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let config_str = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        
        let config: Config = toml::from_str(&config_str)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;
        
        Ok(config)
    }
    
    /// Save configuration to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let config_str = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        
        fs::write(path, config_str)
            .map_err(|e| format!("Failed to write config file: {}", e))?;
        
        Ok(())
    }
    
    /// Generate a default configuration file if it doesn't exist
    pub fn generate_default<P: AsRef<Path>>(path: P) -> Result<(), String> {
        let path = path.as_ref();
        
        if path.exists() {
            info!("Config file already exists at {:?}", path);
            return Ok(());
        }
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create config directory: {}", e))?;
            }
        }
        
        // Create default config
        let config = Config::default();
        
        // Save config
        config.save(path)?;
        
        info!("Generated default config at {:?}", path);
        Ok(())
    }
}
