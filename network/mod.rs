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
pub mod handlers;
pub mod integration;
pub mod events;
pub mod sync;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use log::{debug, error, info, warn};

use crate::network::service::NetworkService;
use crate::network::types::message::NetMessage;
use crate::network::service::advanced_router::AdvancedMessageRouter;

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

/// Start the enhanced network service with the given configuration
pub async fn start_enhanced_network<'a>(
    config: NetworkConfig,
    block_store: Option<Arc<crate::storage::block_store::BlockStore<'a>>>,
    tx_store: Option<Arc<crate::storage::tx_store::TxStore<'a>>>,
    mempool: Option<Arc<crate::mempool::Mempool>>,
    consensus: Option<Arc<crate::consensus::engine::ConsensusEngine<'a>>>,
) -> Arc<NetworkService> {
    // Create the basic network service
    let (message_tx, message_rx) = mpsc::channel(100);
    let service = NetworkService::new(config, message_rx);
    let service_arc = Arc::new(service);

    // Create the peer registry and broadcaster
    let peer_registry = Arc::new(peer::registry::PeerRegistry::new());
    let advanced_registry = Arc::new(peer::advanced_registry::AdvancedPeerRegistry::new());
    let broadcaster = Arc::new(peer::broadcaster::PeerBroadcaster::new());

    // Create the event bus
    let event_bus = Arc::new(events::event_bus::EventBus::new());

    // Create the reputation system
    let reputation = Arc::new(peer::reputation::ReputationSystem::new());

    // Create the advanced router
    let router = Arc::new(AdvancedMessageRouter::new(
        peer_registry.clone(),
        broadcaster.clone(),
    ));

    // Add subsystems to the router
    let router = if let Some(mempool) = mempool.clone() {
        router.with_mempool(mempool)
    } else {
        router
    };

    let router = if let Some(block_store) = block_store.clone() {
        router.with_block_store(block_store)
    } else {
        router
    };

    let router = if let Some(tx_store) = tx_store.clone() {
        router.with_tx_store(tx_store)
    } else {
        router
    };

    let router = if let Some(consensus) = consensus.clone() {
        router.with_consensus(consensus)
    } else {
        router
    };

    // Create the system router
    let system_router = service::system_router::SystemRouter::new(service_arc.router().clone())
        .with_broadcaster(broadcaster.clone());

    let system_router = if let Some(mempool) = mempool.clone() {
        system_router.with_mempool(mempool)
    } else {
        system_router
    };

    let system_router = if let Some(block_store) = block_store.clone() {
        system_router.with_block_store(block_store)
    } else {
        system_router
    };

    let system_router = if let Some(tx_store) = tx_store.clone() {
        system_router.with_tx_store(tx_store)
    } else {
        system_router
    };

    let system_router = if let Some(consensus) = consensus.clone() {
        system_router.with_consensus(consensus)
    } else {
        system_router
    };

    // Initialize the system router
    tokio::spawn(async move {
        system_router.initialize().await;
    });

    // Create integrations if subsystems are provided
    if let (Some(mempool), Some(block_store)) = (mempool.clone(), block_store.clone()) {
        // Create mempool integration
        let mempool_integration = integration::mempool_integration::MempoolIntegration::new(
            mempool,
            broadcaster.clone(),
            peer_registry.clone(),
        ).with_reputation(reputation.clone());

        // Create storage integration
        let storage_integration = integration::storage_integration::StorageIntegration::new(
            block_store,
            broadcaster.clone(),
            peer_registry.clone(),
        ).with_reputation(reputation.clone());

        // Create sync service
        let sync_service = Arc::new(sync::sync_service::SyncService::new(
            block_store.clone(),
            peer_registry.clone(),
            broadcaster.clone(),
        ).with_advanced_registry(advanced_registry.clone())
         .with_event_bus(event_bus.clone())
         .with_reputation(reputation.clone()));

        // Create sync manager
        let sync_manager = sync::sync_manager::SyncManager::new(
            sync_service,
            block_store.clone(),
            peer_registry.clone(),
            broadcaster.clone(),
        ).with_advanced_registry(advanced_registry.clone())
         .with_event_bus(event_bus.clone())
         .with_reputation(reputation.clone());

        // Start the sync manager
        tokio::spawn(async move {
            if let Err(e) = sync_manager.start().await {
                error!("Failed to start sync manager: {}", e);
            }
        });
    }

    // Start the network service
    let service_clone = service_arc.clone();
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