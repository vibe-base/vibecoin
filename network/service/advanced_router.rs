use std::sync::Arc;
use tokio::sync::mpsc;
use log::{debug, error, info, warn};
use dashmap::DashMap;
use std::collections::HashMap;
use std::time::Instant;

use crate::network::types::message::NetMessage;
use crate::storage::block_store::{Block, BlockStore};
use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::mempool::{Mempool, MempoolError};
use crate::consensus::engine::ConsensusEngine;
use crate::network::peer::registry::PeerRegistry;
use crate::network::peer::broadcaster::PeerBroadcaster;

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessagePriority {
    /// High priority messages (blocks, consensus)
    High,
    
    /// Medium priority messages (transactions)
    Medium,
    
    /// Low priority messages (peer discovery, stats)
    Low,
}

/// Message type categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    /// Handshake messages
    Handshake,
    
    /// Block-related messages
    Block,
    
    /// Transaction-related messages
    Transaction,
    
    /// Sync-related messages
    Sync,
    
    /// Ping/pong messages
    Ping,
    
    /// Disconnect messages
    Disconnect,
    
    /// Other messages
    Other,
}

/// Message counters for statistics
#[derive(Debug, Default)]
pub struct MessageCounters {
    /// Counters by message type
    counters: DashMap<MessageType, u64>,
    
    /// Bytes received by message type
    bytes: DashMap<MessageType, u64>,
    
    /// Processing time by message type (in microseconds)
    processing_time: DashMap<MessageType, u64>,
    
    /// Total messages processed
    total_messages: std::sync::atomic::AtomicU64,
    
    /// Total bytes processed
    total_bytes: std::sync::atomic::AtomicU64,
}

impl MessageCounters {
    /// Create new message counters
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Increment counter for a message type
    pub fn increment(&self, msg_type: MessageType) {
        self.counters.entry(msg_type).and_modify(|c| *c += 1).or_insert(1);
        self.total_messages.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Add bytes for a message type
    pub fn add_bytes(&self, msg_type: MessageType, bytes: u64) {
        self.bytes.entry(msg_type).and_modify(|b| *b += bytes).or_insert(bytes);
        self.total_bytes.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
    }
    
    /// Record processing time for a message type
    pub fn record_processing_time(&self, msg_type: MessageType, micros: u64) {
        self.processing_time.entry(msg_type).and_modify(|t| *t += micros).or_insert(micros);
    }
    
    /// Get counter for a message type
    pub fn get_count(&self, msg_type: MessageType) -> u64 {
        self.counters.get(&msg_type).map(|c| *c).unwrap_or(0)
    }
    
    /// Get total messages processed
    pub fn total_messages(&self) -> u64 {
        self.total_messages.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    /// Get total bytes processed
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes.load(std::sync::atomic::Ordering::Relaxed)
    }
    
    /// Get statistics for all message types
    pub fn get_stats(&self) -> HashMap<MessageType, (u64, u64, u64)> {
        let mut stats = HashMap::new();
        
        for msg_type in [
            MessageType::Handshake,
            MessageType::Block,
            MessageType::Transaction,
            MessageType::Sync,
            MessageType::Ping,
            MessageType::Disconnect,
            MessageType::Other,
        ] {
            let count = self.get_count(msg_type);
            let bytes = self.bytes.get(&msg_type).map(|b| *b).unwrap_or(0);
            let time = self.processing_time.get(&msg_type).map(|t| *t).unwrap_or(0);
            
            stats.insert(msg_type, (count, bytes, time));
        }
        
        stats
    }
}

/// Configuration for the advanced router
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Maximum number of pending messages
    pub max_pending_messages: usize,
    
    /// Maximum message size in bytes
    pub max_message_size: usize,
    
    /// Enable message prioritization
    pub enable_prioritization: bool,
    
    /// Enable message rate limiting
    pub enable_rate_limiting: bool,
    
    /// Maximum messages per second per peer
    pub max_messages_per_second: u32,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            max_pending_messages: 10000,
            max_message_size: 10 * 1024 * 1024, // 10 MB
            enable_prioritization: true,
            enable_rate_limiting: true,
            max_messages_per_second: 100,
        }
    }
}

/// Sync request types
#[derive(Debug, Clone)]
pub enum SyncRequest {
    /// Request a single block by height
    GetBlock(u64),
    
    /// Request a range of blocks
    GetBlocks(u64, u64),
    
    /// Request the latest block
    GetLatestBlock,
    
    /// Request the chain state
    GetChainState,
}

/// Advanced message router
pub struct AdvancedMessageRouter {
    /// Core subsystems
    mempool: Option<Arc<Mempool>>,
    block_store: Option<Arc<BlockStore<'static>>>,
    tx_store: Option<Arc<TxStore<'static>>>,
    consensus: Option<Arc<ConsensusEngine>>,
    
    /// Routing channels
    transaction_tx: Option<mpsc::Sender<(TransactionRecord, String)>>,
    block_tx: Option<mpsc::Sender<(Block, String)>>,
    sync_request_tx: Option<mpsc::Sender<(SyncRequest, String)>>,
    
    /// Peer management
    peer_registry: Arc<PeerRegistry>,
    broadcaster: Arc<PeerBroadcaster>,
    
    /// Statistics and monitoring
    message_counters: Arc<MessageCounters>,
    
    /// Configuration
    config: RouterConfig,
}

impl AdvancedMessageRouter {
    /// Create a new advanced message router
    pub fn new(
        peer_registry: Arc<PeerRegistry>,
        broadcaster: Arc<PeerBroadcaster>,
    ) -> Self {
        Self {
            mempool: None,
            block_store: None,
            tx_store: None,
            consensus: None,
            transaction_tx: None,
            block_tx: None,
            sync_request_tx: None,
            peer_registry,
            broadcaster,
            message_counters: Arc::new(MessageCounters::new()),
            config: RouterConfig::default(),
        }
    }
    
    /// Set the mempool
    pub fn with_mempool(mut self, mempool: Arc<Mempool>) -> Self {
        self.mempool = Some(mempool);
        self
    }
    
    /// Set the block store
    pub fn with_block_store(mut self, block_store: Arc<BlockStore<'static>>) -> Self {
        self.block_store = Some(block_store);
        self
    }
    
    /// Set the transaction store
    pub fn with_tx_store(mut self, tx_store: Arc<TxStore<'static>>) -> Self {
        self.tx_store = Some(tx_store);
        self
    }
    
    /// Set the consensus engine
    pub fn with_consensus(mut self, consensus: Arc<ConsensusEngine>) -> Self {
        self.consensus = Some(consensus);
        self
    }
    
    /// Set the transaction channel
    pub fn with_transaction_channel(mut self, tx: mpsc::Sender<(TransactionRecord, String)>) -> Self {
        self.transaction_tx = Some(tx);
        self
    }
    
    /// Set the block channel
    pub fn with_block_channel(mut self, tx: mpsc::Sender<(Block, String)>) -> Self {
        self.block_tx = Some(tx);
        self
    }
    
    /// Set the sync request channel
    pub fn with_sync_request_channel(mut self, tx: mpsc::Sender<(SyncRequest, String)>) -> Self {
        self.sync_request_tx = Some(tx);
        self
    }
    
    /// Set the router configuration
    pub fn with_config(mut self, config: RouterConfig) -> Self {
        self.config = config;
        self
    }
    
    /// Get the message priority for a message type
    fn get_message_priority(&self, message: &NetMessage) -> MessagePriority {
        match message {
            NetMessage::NewBlock(_) => MessagePriority::High,
            NetMessage::ResponseBlock(_) => MessagePriority::High,
            NetMessage::ResponseBlockRange(_) => MessagePriority::High,
            NetMessage::NewTransaction(_) => MessagePriority::Medium,
            NetMessage::RequestBlock(_) => MessagePriority::Medium,
            NetMessage::RequestBlockRange { .. } => MessagePriority::Medium,
            NetMessage::Handshake(_) => MessagePriority::Low,
            NetMessage::Ping(_) => MessagePriority::Low,
            NetMessage::Pong(_) => MessagePriority::Low,
            NetMessage::Disconnect(_) => MessagePriority::Low,
        }
    }
    
    /// Get the message type for a message
    fn get_message_type(&self, message: &NetMessage) -> MessageType {
        match message {
            NetMessage::Handshake(_) => MessageType::Handshake,
            NetMessage::NewBlock(_) => MessageType::Block,
            NetMessage::ResponseBlock(_) => MessageType::Block,
            NetMessage::ResponseBlockRange(_) => MessageType::Block,
            NetMessage::NewTransaction(_) => MessageType::Transaction,
            NetMessage::RequestBlock(_) => MessageType::Sync,
            NetMessage::RequestBlockRange { .. } => MessageType::Sync,
            NetMessage::Ping(_) => MessageType::Ping,
            NetMessage::Pong(_) => MessageType::Ping,
            NetMessage::Disconnect(_) => MessageType::Disconnect,
        }
    }
    
    /// Route a message to the appropriate handler
    pub async fn route_message(&self, message: NetMessage, source_peer: String) {
        let msg_type = self.get_message_type(&message);
        let priority = self.get_message_priority(&message);
        
        // Update statistics
        self.message_counters.increment(msg_type);
        
        // Record processing time
        let start_time = Instant::now();
        
        // Route based on message type
        match message {
            NetMessage::NewTransaction(tx) => {
                self.handle_transaction(tx, source_peer).await;
            },
            NetMessage::NewBlock(block) => {
                self.handle_block(block, source_peer).await;
            },
            NetMessage::RequestBlock(height) => {
                self.handle_block_request(height, source_peer).await;
            },
            NetMessage::RequestBlockRange { start_height, end_height } => {
                self.handle_block_range_request(start_height, end_height, source_peer).await;
            },
            NetMessage::Ping(nonce) => {
                self.handle_ping(nonce, source_peer).await;
            },
            NetMessage::Pong(nonce) => {
                self.handle_pong(nonce, source_peer).await;
            },
            NetMessage::Handshake(node_info) => {
                debug!("Handshake from peer {}: {:?}", source_peer, node_info);
                // Handshake is handled by the peer handler
            },
            NetMessage::Disconnect(reason) => {
                info!("Disconnect from peer {}: {:?}", source_peer, reason);
                // Disconnect is handled by the peer handler
            },
            _ => {
                warn!("Unhandled message type from peer {}", source_peer);
            }
        }
        
        // Record processing time
        let elapsed = start_time.elapsed();
        self.message_counters.record_processing_time(msg_type, elapsed.as_micros() as u64);
    }
    
    /// Handle a transaction message
    async fn handle_transaction(&self, tx: TransactionRecord, source_peer: String) {
        debug!("Handling transaction from peer {}: {:?}", source_peer, tx.tx_id);
        
        // Send to transaction channel if available
        if let Some(tx_channel) = &self.transaction_tx {
            if let Err(e) = tx_channel.send((tx.clone(), source_peer.clone())).await {
                error!("Failed to send transaction to channel: {:?}", e);
            }
            return;
        }
        
        // Otherwise, process directly
        if let Some(mempool) = &self.mempool {
            match mempool.insert(tx.clone()).await {
                Ok(_) => {
                    info!("Transaction added to mempool: {:?}", tx.tx_id);
                    
                    // Broadcast to other peers
                    self.broadcaster.broadcast_except(
                        NetMessage::NewTransaction(tx),
                        &source_peer
                    ).await;
                },
                Err(MempoolError::DuplicateTransaction) => {
                    debug!("Duplicate transaction from peer {}: {:?}", source_peer, tx.tx_id);
                },
                Err(e) => {
                    warn!("Failed to add transaction to mempool: {:?}", e);
                    
                    // TODO: Update peer reputation
                }
            }
        } else {
            warn!("No mempool available to handle transaction");
        }
    }
    
    /// Handle a block message
    async fn handle_block(&self, block: Block, source_peer: String) {
        info!("Handling block from peer {}: height={}", source_peer, block.height);
        
        // Send to block channel if available
        if let Some(block_channel) = &self.block_tx {
            if let Err(e) = block_channel.send((block.clone(), source_peer.clone())).await {
                error!("Failed to send block to channel: {:?}", e);
            }
            return;
        }
        
        // Otherwise, process directly
        if let Some(consensus) = &self.consensus {
            // The consensus engine will handle validation and storage
            let block_rx = consensus.block_channel();
            if let Err(e) = block_rx.send(block.clone()).await {
                error!("Failed to send block to consensus engine: {:?}", e);
            }
        } else if let Some(block_store) = &self.block_store {
            // If we don't have a consensus engine, just store the block
            block_store.put_block(&block);
            
            // Broadcast to other peers
            self.broadcaster.broadcast_except(
                NetMessage::NewBlock(block),
                &source_peer
            ).await;
        } else {
            warn!("No block store or consensus engine available to handle block");
        }
    }
    
    /// Handle a block request
    async fn handle_block_request(&self, height: u64, source_peer: String) {
        debug!("Handling block request from peer {}: height={}", source_peer, height);
        
        // Send to sync channel if available
        if let Some(sync_channel) = &self.sync_request_tx {
            if let Err(e) = sync_channel.send((SyncRequest::GetBlock(height), source_peer.clone())).await {
                error!("Failed to send sync request to channel: {:?}", e);
            }
            return;
        }
        
        // Otherwise, process directly
        if let Some(block_store) = &self.block_store {
            // Get the block from the store
            let block = block_store.get_block_by_height(height);
            
            // Send the response
            self.broadcaster.send_to_peer(
                &source_peer,
                NetMessage::ResponseBlock(block),
            ).await;
        } else {
            warn!("No block store available to handle block request");
        }
    }
    
    /// Handle a block range request
    async fn handle_block_range_request(&self, start_height: u64, end_height: u64, source_peer: String) {
        debug!("Handling block range request from peer {}: {}..{}", source_peer, start_height, end_height);
        
        // Send to sync channel if available
        if let Some(sync_channel) = &self.sync_request_tx {
            if let Err(e) = sync_channel.send((SyncRequest::GetBlocks(start_height, end_height), source_peer.clone())).await {
                error!("Failed to send sync request to channel: {:?}", e);
            }
            return;
        }
        
        // Otherwise, process directly
        if let Some(block_store) = &self.block_store {
            // Get the blocks from the store
            let mut blocks = Vec::new();
            for height in start_height..=end_height {
                if let Some(block) = block_store.get_block_by_height(height) {
                    blocks.push(block);
                }
            }
            
            // Send the response
            self.broadcaster.send_to_peer(
                &source_peer,
                NetMessage::ResponseBlockRange(blocks),
            ).await;
        } else {
            warn!("No block store available to handle block range request");
        }
    }
    
    /// Handle a ping message
    async fn handle_ping(&self, nonce: u64, source_peer: String) {
        debug!("Handling ping from peer {}: nonce={}", source_peer, nonce);
        
        // Send pong response
        self.broadcaster.send_to_peer(
            &source_peer,
            NetMessage::Pong(nonce),
        ).await;
    }
    
    /// Handle a pong message
    async fn handle_pong(&self, nonce: u64, source_peer: String) {
        debug!("Handling pong from peer {}: nonce={}", source_peer, nonce);
        
        // Update peer latency (if we were tracking the ping)
        // For now, just log it
    }
    
    /// Get message statistics
    pub fn get_stats(&self) -> HashMap<MessageType, (u64, u64, u64)> {
        self.message_counters.get_stats()
    }
    
    /// Get total messages processed
    pub fn total_messages(&self) -> u64 {
        self.message_counters.total_messages()
    }
    
    /// Get total bytes processed
    pub fn total_bytes(&self) -> u64 {
        self.message_counters.total_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::types::node_info::NodeInfo;
    use std::net::SocketAddr;
    
    #[tokio::test]
    async fn test_message_priority() {
        // Create a router
        let peer_registry = Arc::new(PeerRegistry::new());
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let router = AdvancedMessageRouter::new(peer_registry, broadcaster);
        
        // Test message priorities
        let block_msg = NetMessage::NewBlock(Block {
            height: 1,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
        });
        
        let tx_msg = NetMessage::NewTransaction(TransactionRecord {
            tx_id: [0u8; 32],
            sender: [0u8; 32],
            recipient: [0u8; 32],
            value: 0,
            gas_used: 0,
            block_height: 0,
        });
        
        let ping_msg = NetMessage::Ping(0);
        
        assert_eq!(router.get_message_priority(&block_msg), MessagePriority::High);
        assert_eq!(router.get_message_priority(&tx_msg), MessagePriority::Medium);
        assert_eq!(router.get_message_priority(&ping_msg), MessagePriority::Low);
    }
    
    #[tokio::test]
    async fn test_message_type() {
        // Create a router
        let peer_registry = Arc::new(PeerRegistry::new());
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let router = AdvancedMessageRouter::new(peer_registry, broadcaster);
        
        // Test message types
        let block_msg = NetMessage::NewBlock(Block {
            height: 1,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
        });
        
        let tx_msg = NetMessage::NewTransaction(TransactionRecord {
            tx_id: [0u8; 32],
            sender: [0u8; 32],
            recipient: [0u8; 32],
            value: 0,
            gas_used: 0,
            block_height: 0,
        });
        
        let handshake_msg = NetMessage::Handshake(NodeInfo::new(
            "1.0.0".to_string(),
            "test-node".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        ));
        
        assert_eq!(router.get_message_type(&block_msg), MessageType::Block);
        assert_eq!(router.get_message_type(&tx_msg), MessageType::Transaction);
        assert_eq!(router.get_message_type(&handshake_msg), MessageType::Handshake);
    }
}
