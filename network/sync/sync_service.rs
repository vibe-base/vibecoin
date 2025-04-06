use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;
use crate::network::peer::advanced_registry::AdvancedPeerRegistry;
use crate::network::peer::reputation::{ReputationSystem, ReputationEvent};
use crate::network::events::event_bus::EventBus;
use crate::network::events::event_types::{NetworkEvent, SyncResult, EventType};
use crate::network::service::advanced_router::SyncRequest;

/// Sync state
#[derive(Debug, Clone)]
pub struct SyncState {
    /// Whether a sync is in progress
    pub in_progress: bool,
    
    /// The current sync target height
    pub target_height: u64,
    
    /// The current sync height
    pub current_height: u64,
    
    /// The peer we're syncing from
    pub sync_peer: Option<String>,
    
    /// When the sync started
    pub start_time: Option<Instant>,
    
    /// The number of blocks synced
    pub blocks_synced: u64,
    
    /// The number of failed block requests
    pub failed_requests: u64,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            in_progress: false,
            target_height: 0,
            current_height: 0,
            sync_peer: None,
            start_time: None,
            blocks_synced: 0,
            failed_requests: 0,
        }
    }
}

/// Configuration for the sync service
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Maximum number of blocks to request at once
    pub max_blocks_per_request: u64,
    
    /// Timeout for sync requests
    pub request_timeout: Duration,
    
    /// Maximum number of retries for a request
    pub max_retries: u32,
    
    /// Interval between sync attempts
    pub sync_interval: Duration,
    
    /// Whether to automatically sync on startup
    pub auto_sync: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            max_blocks_per_request: 100,
            request_timeout: Duration::from_secs(30),
            max_retries: 3,
            sync_interval: Duration::from_secs(60),
            auto_sync: true,
        }
    }
}

/// Sync service for blockchain synchronization
pub struct SyncService {
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
    
    /// Sync state
    sync_state: Arc<RwLock<SyncState>>,
    
    /// Configuration
    config: SyncConfig,
    
    /// Channel for block responses
    block_rx: Option<mpsc::Receiver<(Block, String)>>,
    
    /// Channel for block range responses
    block_range_rx: Option<mpsc::Receiver<(Vec<Block>, String)>>,
    
    /// Whether the service is running
    running: Arc<RwLock<bool>>,
}

impl SyncService {
    /// Create a new sync service
    pub fn new(
        block_store: Arc<BlockStore<'static>>,
        peer_registry: Arc<PeerRegistry>,
        broadcaster: Arc<PeerBroadcaster>,
    ) -> Self {
        Self {
            block_store,
            peer_registry,
            advanced_registry: None,
            broadcaster,
            event_bus: None,
            reputation: None,
            sync_state: Arc::new(RwLock::new(SyncState::default())),
            config: SyncConfig::default(),
            block_rx: None,
            block_range_rx: None,
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
    
    /// Set the configuration
    pub fn with_config(mut self, config: SyncConfig) -> Self {
        self.config = config;
        self
    }
    
    /// Set the block response channel
    pub fn with_block_channel(mut self, rx: mpsc::Receiver<(Block, String)>) -> Self {
        self.block_rx = Some(rx);
        self
    }
    
    /// Set the block range response channel
    pub fn with_block_range_channel(mut self, rx: mpsc::Receiver<(Vec<Block>, String)>) -> Self {
        self.block_range_rx = Some(rx);
        self
    }
    
    /// Start the sync service
    pub async fn start(&self) -> Result<(), String> {
        // Check if we're already running
        {
            let running = self.running.read().await;
            if *running {
                return Err("Sync service already running".to_string());
            }
        }
        
        // Set running flag
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        
        // Start the sync loop
        let block_store = self.block_store.clone();
        let peer_registry = self.peer_registry.clone();
        let broadcaster = self.broadcaster.clone();
        let sync_state = self.sync_state.clone();
        let config = self.config.clone();
        let running = self.running.clone();
        let event_bus = self.event_bus.clone();
        let reputation = self.reputation.clone();
        let advanced_registry = self.advanced_registry.clone();
        
        tokio::spawn(async move {
            info!("Starting sync service");
            
            // Auto-sync on startup if enabled
            if config.auto_sync {
                let _ = Self::sync_with_network(
                    block_store.clone(),
                    peer_registry.clone(),
                    broadcaster.clone(),
                    sync_state.clone(),
                    event_bus.clone(),
                    reputation.clone(),
                    advanced_registry.clone(),
                    &config,
                ).await;
            }
            
            // Main sync loop
            let mut interval = tokio::time::interval(config.sync_interval);
            
            while {
                let is_running = *running.read().await;
                is_running
            } {
                interval.tick().await;
                
                // Check if we're already syncing
                let is_syncing = {
                    let state = sync_state.read().await;
                    state.in_progress
                };
                
                if !is_syncing {
                    let _ = Self::sync_with_network(
                        block_store.clone(),
                        peer_registry.clone(),
                        broadcaster.clone(),
                        sync_state.clone(),
                        event_bus.clone(),
                        reputation.clone(),
                        advanced_registry.clone(),
                        &config,
                    ).await;
                }
            }
            
            info!("Sync service stopped");
        });
        
        // Start the block response handler
        if let Some(mut block_rx) = self.block_rx.clone() {
            let block_store = self.block_store.clone();
            let sync_state = self.sync_state.clone();
            let running = self.running.clone();
            let event_bus = self.event_bus.clone();
            let reputation = self.reputation.clone();
            
            tokio::spawn(async move {
                info!("Starting block response handler");
                
                while {
                    let is_running = *running.read().await;
                    is_running
                } {
                    match block_rx.recv().await {
                        Some((block, peer_id)) => {
                            debug!("Received block from peer {}: height={}", peer_id, block.height);
                            
                            // Store the block
                            block_store.put_block(&block);
                            
                            // Update sync state
                            {
                                let mut state = sync_state.write().await;
                                if state.in_progress {
                                    state.blocks_synced += 1;
                                    
                                    // Check if we've reached the target height
                                    if block.height >= state.target_height {
                                        state.in_progress = false;
                                        
                                        // Publish sync completed event
                                        if let Some(event_bus) = &event_bus {
                                            let result = SyncResult {
                                                success: true,
                                                blocks_synced: state.blocks_synced,
                                                start_height: state.current_height,
                                                end_height: state.target_height,
                                                error: None,
                                            };
                                            
                                            let _ = event_bus.publish(NetworkEvent::SyncCompleted(result)).await;
                                        }
                                    }
                                }
                            }
                            
                            // Update peer reputation (good block)
                            if let Some(reputation) = &reputation {
                                reputation.update_score(&peer_id, ReputationEvent::GoodBlock);
                            }
                        },
                        None => {
                            warn!("Block response channel closed");
                            break;
                        }
                    }
                }
                
                info!("Block response handler stopped");
            });
        }
        
        // Start the block range response handler
        if let Some(mut block_range_rx) = self.block_range_rx.clone() {
            let block_store = self.block_store.clone();
            let sync_state = self.sync_state.clone();
            let running = self.running.clone();
            let event_bus = self.event_bus.clone();
            let reputation = self.reputation.clone();
            
            tokio::spawn(async move {
                info!("Starting block range response handler");
                
                while {
                    let is_running = *running.read().await;
                    is_running
                } {
                    match block_range_rx.recv().await {
                        Some((blocks, peer_id)) => {
                            debug!("Received {} blocks from peer {}", blocks.len(), peer_id);
                            
                            // Store the blocks
                            for block in &blocks {
                                block_store.put_block(block);
                            }
                            
                            // Update sync state
                            {
                                let mut state = sync_state.write().await;
                                if state.in_progress {
                                    state.blocks_synced += blocks.len() as u64;
                                    
                                    // Check if we've reached the target height
                                    if let Some(last_block) = blocks.last() {
                                        if last_block.height >= state.target_height {
                                            state.in_progress = false;
                                            
                                            // Publish sync completed event
                                            if let Some(event_bus) = &event_bus {
                                                let result = SyncResult {
                                                    success: true,
                                                    blocks_synced: state.blocks_synced,
                                                    start_height: state.current_height,
                                                    end_height: state.target_height,
                                                    error: None,
                                                };
                                                
                                                let _ = event_bus.publish(NetworkEvent::SyncCompleted(result)).await;
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Update peer reputation (good blocks)
                            if let Some(reputation) = &reputation {
                                reputation.update_score(&peer_id, ReputationEvent::GoodBlock);
                            }
                        },
                        None => {
                            warn!("Block range response channel closed");
                            break;
                        }
                    }
                }
                
                info!("Block range response handler stopped");
            });
        }
        
        Ok(())
    }
    
    /// Stop the sync service
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }
    
    /// Sync with the network
    async fn sync_with_network(
        block_store: Arc<BlockStore<'static>>,
        peer_registry: Arc<PeerRegistry>,
        broadcaster: Arc<PeerBroadcaster>,
        sync_state: Arc<RwLock<SyncState>>,
        event_bus: Option<Arc<EventBus>>,
        reputation: Option<Arc<ReputationSystem>>,
        advanced_registry: Option<Arc<AdvancedPeerRegistry>>,
        config: &SyncConfig,
    ) -> Result<(), String> {
        // Get our current height
        let current_height = block_store.get_latest_height().unwrap_or(0);
        
        // Find the best peer to sync from
        let sync_peer = if let Some(registry) = &advanced_registry {
            // Use the advanced registry to find the best sync peer
            let best_peers = registry.get_best_sync_peers(1);
            best_peers.first().cloned()
        } else {
            // Use the basic registry to find an active peer
            let active_peers = peer_registry.get_active_peers();
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
        match broadcaster.send_to_peer(
            &sync_peer,
            NetMessage::RequestBlock(u64::MAX), // Special value to request the latest block
        ).await {
            Ok(_) => {
                debug!("Requested latest block from peer {}", sync_peer);
                
                // Update sync state
                {
                    let mut state = sync_state.write().await;
                    state.in_progress = true;
                    state.current_height = current_height;
                    state.sync_peer = Some(sync_peer.clone());
                    state.start_time = Some(Instant::now());
                    state.blocks_synced = 0;
                    state.failed_requests = 0;
                }
                
                // Publish sync requested event
                if let Some(event_bus) = &event_bus {
                    let _ = event_bus.publish(NetworkEvent::SyncRequested(
                        SyncRequest::GetLatestBlock,
                        sync_peer.clone(),
                    )).await;
                }
                
                Ok(())
            },
            Err(e) => {
                error!("Failed to request latest block from peer {}: {:?}", sync_peer, e);
                
                // Update peer reputation (timeout)
                if let Some(reputation) = &reputation {
                    reputation.update_score(&sync_peer, ReputationEvent::Timeout);
                }
                
                Err(format!("Failed to request latest block: {}", e))
            }
        }
    }
    
    /// Sync to a specific height
    pub async fn sync_to_height(&self, target_height: u64) -> Result<(), String> {
        // Get our current height
        let current_height = self.block_store.get_latest_height().unwrap_or(0);
        
        // Check if we're already at or beyond the target height
        if current_height >= target_height {
            debug!("Already at or beyond target height: {} >= {}", current_height, target_height);
            return Ok(());
        }
        
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
        
        // Request blocks from the peer
        let start_height = current_height + 1;
        let end_height = target_height;
        
        match self.broadcaster.send_to_peer(
            &sync_peer,
            NetMessage::RequestBlockRange { start_height, end_height },
        ).await {
            Ok(_) => {
                info!("Requested blocks {}..{} from peer {}", start_height, end_height, sync_peer);
                
                // Update sync state
                {
                    let mut state = self.sync_state.write().await;
                    state.in_progress = true;
                    state.target_height = target_height;
                    state.current_height = current_height;
                    state.sync_peer = Some(sync_peer.clone());
                    state.start_time = Some(Instant::now());
                    state.blocks_synced = 0;
                    state.failed_requests = 0;
                }
                
                // Publish sync requested event
                if let Some(event_bus) = &self.event_bus {
                    let _ = event_bus.publish(NetworkEvent::SyncRequested(
                        SyncRequest::GetBlocks(start_height, end_height),
                        sync_peer.clone(),
                    )).await;
                }
                
                Ok(())
            },
            Err(e) => {
                error!("Failed to request blocks from peer {}: {:?}", sync_peer, e);
                
                // Update peer reputation (timeout)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(&sync_peer, ReputationEvent::Timeout);
                }
                
                Err(format!("Failed to request blocks: {}", e))
            }
        }
    }
    
    /// Get the current sync state
    pub async fn get_sync_state(&self) -> SyncState {
        let state = self.sync_state.read().await;
        state.clone()
    }
    
    /// Check if a sync is in progress
    pub async fn is_syncing(&self) -> bool {
        let state = self.sync_state.read().await;
        state.in_progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_sync_service() {
        // Create dependencies
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let peer_registry = Arc::new(PeerRegistry::new());
        let broadcaster = Arc::new(PeerBroadcaster::new());
        
        // Create sync service
        let service = SyncService::new(
            block_store.clone(),
            peer_registry.clone(),
            broadcaster.clone(),
        );
        
        // Check initial state
        let state = service.get_sync_state().await;
        assert!(!state.in_progress);
        assert_eq!(state.blocks_synced, 0);
        
        // Register a peer
        let addr: std::net::SocketAddr = "127.0.0.1:8000".parse().unwrap();
        peer_registry.register_peer("peer1", addr, true);
        peer_registry.update_peer_state("peer1", ConnectionState::Ready);
        
        // We can't fully test syncing without a network, but we can check that
        // the service starts and stops correctly
        let result = service.start().await;
        assert!(result.is_ok());
        
        // Stop the service
        service.stop().await;
    }
}
