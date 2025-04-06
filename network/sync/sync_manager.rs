use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;
use crate::network::peer::advanced_registry::{AdvancedPeerRegistry, PeerId};
use crate::network::peer::reputation::{ReputationSystem, ReputationEvent};
use crate::network::events::event_bus::EventBus;
use crate::network::events::event_types::{NetworkEvent, SyncResult, EventType};
use crate::network::service::advanced_router::SyncRequest;
use crate::network::sync::sync_service::{SyncService, SyncConfig, SyncState};

/// Sync strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStrategy {
    /// Full sync from genesis
    FullSync,
    
    /// Fast sync from a trusted checkpoint
    FastSync,
    
    /// Incremental sync from the current height
    IncrementalSync,
}

/// Sync manager for coordinating blockchain synchronization
pub struct SyncManager {
    /// Sync service
    sync_service: Arc<SyncService>,
    
    /// Block store
    block_store: Arc<BlockStore<'static>>,
    
    /// Peer registry
    peer_registry: Arc<PeerRegistry>,
    
    /// Advanced peer registry
    advanced_registry: Option<Arc<AdvancedPeerRegistry>>,
    
    /// Broadcaster for sending messages to peers
    broadcaster: Arc<PeerBroadcaster>,
    
    /// Event bus for publishing events
    event_bus: Option<Arc<EventBus>>,
    
    /// Reputation system for tracking peer behavior
    reputation: Option<Arc<ReputationSystem>>,
    
    /// Sync strategy
    strategy: SyncStrategy,
    
    /// Trusted checkpoints (height -> hash)
    checkpoints: Arc<RwLock<Vec<(u64, [u8; 32])>>>,
    
    /// Whether the manager is running
    running: Arc<RwLock<bool>>,
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new(
        sync_service: Arc<SyncService>,
        block_store: Arc<BlockStore<'static>>,
        peer_registry: Arc<PeerRegistry>,
        broadcaster: Arc<PeerBroadcaster>,
    ) -> Self {
        Self {
            sync_service,
            block_store,
            peer_registry,
            advanced_registry: None,
            broadcaster,
            event_bus: None,
            reputation: None,
            strategy: SyncStrategy::IncrementalSync,
            checkpoints: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Set the advanced peer registry
    pub fn with_advanced_registry(mut self, registry: Arc<AdvancedPeerRegistry>) -> Self {
        self.advanced_registry = Some(registry);
        self
    }
    
    /// Set the event bus
    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }
    
    /// Set the reputation system
    pub fn with_reputation(mut self, reputation: Arc<ReputationSystem>) -> Self {
        self.reputation = Some(reputation);
        self
    }
    
    /// Set the sync strategy
    pub fn with_strategy(mut self, strategy: SyncStrategy) -> Self {
        self.strategy = strategy;
        self
    }
    
    /// Add a checkpoint
    pub async fn add_checkpoint(&self, height: u64, hash: [u8; 32]) {
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.push((height, hash));
        
        // Sort by height
        checkpoints.sort_by_key(|(h, _)| *h);
    }
    
    /// Start the sync manager
    pub async fn start(&self) -> Result<(), String> {
        // Check if we're already running
        {
            let running = self.running.read().await;
            if *running {
                return Err("Sync manager already running".to_string());
            }
        }
        
        // Set running flag
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        
        // Start the sync service
        self.sync_service.start().await?;
        
        // Subscribe to sync events
        if let Some(event_bus) = &self.event_bus {
            let sync_rx = event_bus.subscribe(EventType::Sync).await;
            
            // Clone necessary fields for the task
            let running = self.running.clone();
            let sync_service = self.sync_service.clone();
            let strategy = self.strategy;
            let checkpoints = self.checkpoints.clone();
            
            tokio::spawn(async move {
                Self::handle_sync_events(
                    sync_rx,
                    running,
                    sync_service,
                    strategy,
                    checkpoints,
                ).await;
            });
        }
        
        // Start the sync based on the strategy
        match self.strategy {
            SyncStrategy::FullSync => {
                self.start_full_sync().await?;
            },
            SyncStrategy::FastSync => {
                self.start_fast_sync().await?;
            },
            SyncStrategy::IncrementalSync => {
                // Incremental sync is handled by the sync service
            },
        }
        
        Ok(())
    }
    
    /// Stop the sync manager
    pub async fn stop(&self) {
        // Stop the sync service
        self.sync_service.stop().await;
        
        // Set running flag
        let mut running = self.running.write().await;
        *running = false;
    }
    
    /// Start a full sync from genesis
    async fn start_full_sync(&self) -> Result<(), String> {
        info!("Starting full sync from genesis");
        
        // Get the latest height from the network
        let latest_height = self.get_network_height().await?;
        
        // Sync from genesis to the latest height
        self.sync_service.sync_to_height(latest_height).await
    }
    
    /// Start a fast sync from a checkpoint
    async fn start_fast_sync(&self) -> Result<(), String> {
        info!("Starting fast sync from checkpoint");
        
        // Get the latest checkpoint
        let checkpoint = {
            let checkpoints = self.checkpoints.read().await;
            checkpoints.last().cloned()
        };
        
        let checkpoint = match checkpoint {
            Some(cp) => cp,
            None => {
                warn!("No checkpoints available for fast sync");
                return Err("No checkpoints available for fast sync".to_string());
            }
        };
        
        // Get the latest height from the network
        let latest_height = self.get_network_height().await?;
        
        // Sync from the checkpoint to the latest height
        self.sync_service.sync_to_height(latest_height).await
    }
    
    /// Get the latest height from the network
    async fn get_network_height(&self) -> Result<u64, String> {
        // Find the best peer to sync from
        let sync_peer = if let Some(registry) = &self.advanced_registry {
            // Use the advanced registry to find the best sync peer
            let best_peers = registry.get_best_sync_peers(1);
            best_peers.first().cloned()
        } else {
            // Use the basic registry to find an active peer
            let active_peers = self.peer_registry.get_active_peers();
            active_peers.first().map(|p| p.node_id.clone())
        };
        
        let sync_peer = match sync_peer {
            Some(peer) => peer,
            None => {
                warn!("No peers available for sync");
                return Err("No peers available for sync".to_string());
            }
        };
        
        // Request the latest block from the peer
        match self.broadcaster.send_to_peer(
            &sync_peer,
            NetMessage::RequestBlock(u64::MAX), // Special value to request the latest block
        ).await {
            Ok(_) => {
                debug!("Requested latest block from peer {}", sync_peer);
                
                // In a real implementation, we would wait for the response
                // For now, just return a placeholder value
                Ok(1000)
            },
            Err(e) => {
                error!("Failed to request latest block from peer {}: {:?}", sync_peer, e);
                Err(format!("Failed to request latest block: {}", e))
            }
        }
    }
    
    /// Handle sync events
    async fn handle_sync_events(
        mut sync_rx: mpsc::Receiver<NetworkEvent>,
        running: Arc<RwLock<bool>>,
        sync_service: Arc<SyncService>,
        strategy: SyncStrategy,
        checkpoints: Arc<RwLock<Vec<(u64, [u8; 32])>>>,
    ) {
        info!("Starting sync event handler");
        
        while {
            let is_running = *running.read().await;
            is_running
        } {
            match sync_rx.recv().await {
                Some(event) => {
                    match event {
                        NetworkEvent::SyncRequested(request, peer_id) => {
                            debug!("Sync requested: {:?} from peer {}", request, peer_id);
                        },
                        NetworkEvent::SyncCompleted(result) => {
                            info!("Sync completed: {:?}", result);
                            
                            // If we're doing a full or fast sync, we might need to continue
                            if strategy == SyncStrategy::FullSync || strategy == SyncStrategy::FastSync {
                                if result.success {
                                    // Check if we've reached the target height
                                    // In a real implementation, we would check if we've reached
                                    // the latest network height
                                }
                            }
                        },
                        _ => {
                            // Ignore other events
                        }
                    }
                },
                None => {
                    warn!("Sync event channel closed");
                    break;
                }
            }
        }
        
        info!("Sync event handler stopped");
    }
    
    /// Get the current sync state
    pub async fn get_sync_state(&self) -> SyncState {
        self.sync_service.get_sync_state().await
    }
    
    /// Check if a sync is in progress
    pub async fn is_syncing(&self) -> bool {
        self.sync_service.is_syncing().await
    }
    
    /// Get the sync strategy
    pub fn get_strategy(&self) -> SyncStrategy {
        self.strategy
    }
    
    /// Set the sync strategy
    pub fn set_strategy(&mut self, strategy: SyncStrategy) {
        self.strategy = strategy;
    }
    
    /// Get the checkpoints
    pub async fn get_checkpoints(&self) -> Vec<(u64, [u8; 32])> {
        let checkpoints = self.checkpoints.read().await;
        checkpoints.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_sync_manager() {
        // Create dependencies
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let peer_registry = Arc::new(PeerRegistry::new());
        let broadcaster = Arc::new(PeerBroadcaster::new());
        
        // Create sync service
        let sync_service = Arc::new(SyncService::new(
            block_store.clone(),
            peer_registry.clone(),
            broadcaster.clone(),
        ));
        
        // Create sync manager
        let manager = SyncManager::new(
            sync_service,
            block_store.clone(),
            peer_registry.clone(),
            broadcaster.clone(),
        );
        
        // Add a checkpoint
        manager.add_checkpoint(100, [1u8; 32]).await;
        
        // Check that the checkpoint was added
        let checkpoints = manager.get_checkpoints().await;
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].0, 100);
        
        // Register a peer
        let addr: std::net::SocketAddr = "127.0.0.1:8000".parse().unwrap();
        peer_registry.register_peer("peer1", addr, true);
        peer_registry.update_peer_state("peer1", ConnectionState::Ready);
        
        // We can't fully test syncing without a network, but we can check that
        // the manager starts and stops correctly
        let result = manager.start().await;
        assert!(result.is_ok());
        
        // Stop the manager
        manager.stop().await;
    }
}
