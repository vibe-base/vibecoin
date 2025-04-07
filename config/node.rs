use serde::{Serialize, Deserialize};

/// Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node ID (public key)
    pub node_id: Option<String>,
    
    /// Node name
    pub node_name: String,
    
    /// Data directory
    pub data_dir: String,
    
    /// Log level
    pub log_level: String,
    
    /// Enable metrics
    pub enable_metrics: bool,
    
    /// Metrics port
    pub metrics_port: u16,
    
    /// Enable API
    pub enable_api: bool,
    
    /// API port
    pub api_port: u16,
    
    /// API host
    pub api_host: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            node_id: None,
            node_name: "vibecoin-node".to_string(),
            data_dir: "./data/vibecoin".to_string(),
            log_level: "info".to_string(),
            enable_metrics: false,
            metrics_port: 9100,
            enable_api: true,
            api_port: 8545,
            api_host: "127.0.0.1".to_string(),
        }
    }
}
