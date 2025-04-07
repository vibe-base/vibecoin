use serde::{Serialize, Deserialize};

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Listen address
    pub listen_addr: String,
    
    /// Listen port
    pub listen_port: u16,
    
    /// External address (for NAT traversal)
    pub external_addr: Option<String>,
    
    /// External port (for NAT traversal)
    pub external_port: Option<u16>,
    
    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,
    
    /// Maximum number of peers
    pub max_peers: usize,
    
    /// Minimum number of peers
    pub min_peers: usize,
    
    /// Peer discovery interval in seconds
    pub discovery_interval: u64,
    
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    
    /// Handshake timeout in seconds
    pub handshake_timeout: u64,
    
    /// Enable UPnP
    pub enable_upnp: bool,
    
    /// Enable NAT-PMP
    pub enable_natpmp: bool,
    
    /// Enable DHT
    pub enable_dht: bool,
    
    /// DHT bootstrap nodes
    pub dht_bootstrap_nodes: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0".to_string(),
            listen_port: 30333,
            external_addr: None,
            external_port: None,
            bootstrap_nodes: vec![
                "/dns4/bootstrap1.vibecoin.network/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp".to_string(),
                "/dns4/bootstrap2.vibecoin.network/tcp/30333/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9".to_string(),
            ],
            max_peers: 50,
            min_peers: 10,
            discovery_interval: 30,
            connection_timeout: 10,
            handshake_timeout: 5,
            enable_upnp: true,
            enable_natpmp: true,
            enable_dht: true,
            dht_bootstrap_nodes: vec![
                "/dns4/dht1.vibecoin.network/tcp/30334/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp".to_string(),
                "/dns4/dht2.vibecoin.network/tcp/30334/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9".to_string(),
            ],
        }
    }
}
