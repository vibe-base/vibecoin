// VibeCoin Network Module
//
// This module provides the peer-to-peer networking layer for the VibeCoin blockchain:
// - Peer discovery and connection management
// - Message broadcasting (blocks, transactions)
// - Blockchain data synchronization
// - Support for distributed consensus

pub mod types;
pub mod peer;
pub mod service;
pub mod codec;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::network::service::NetworkService;
use crate::network::types::message::NetMessage;

/// Configuration for the network module
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    /// Local address to bind to
    pub bind_addr: SocketAddr,

    /// List of seed peers to connect to
    pub seed_peers: Vec<SocketAddr>,

    /// Maximum number of outbound connections
    pub max_outbound: usize,

    /// Maximum number of inbound connections
    pub max_inbound: usize,

    /// Node ID (derived from public key)
    pub node_id: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8765".parse().unwrap(),
            seed_peers: vec![],
            max_outbound: 8,
            max_inbound: 32,
            node_id: "unknown".to_string(),
        }
    }
}

/// Start the network service with the given configuration
pub async fn start_network(config: NetworkConfig) -> Arc<NetworkService> {
    let (message_tx, message_rx) = mpsc::channel(100);

    let service = NetworkService::new(config, message_rx);
    let service_arc = Arc::new(service);

    // Clone the Arc for the spawned task
    let service_clone = service_arc.clone();

    // Start the network service in a separate task
    tokio::spawn(async move {
        service_clone.run().await;
    });

    service_arc
}

/// Create a network message sender
pub fn create_message_sender() -> mpsc::Sender<NetMessage> {
    let (tx, _rx) = mpsc::channel(100);
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_config() {
        let config = NetworkConfig::default();
        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:8765");
        assert_eq!(config.max_outbound, 8);
        assert_eq!(config.max_inbound, 32);
    }
}