use std::sync::Arc;
use tokio::sync::mpsc;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;
use crate::network::peer::reputation::{ReputationSystem, ReputationEvent};
use crate::network::service::advanced_router::SyncRequest;

/// Error types for storage integration
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Block not found: height={0}")]
    BlockNotFound(u64),

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Integration between the network module and the storage module
pub struct StorageIntegration {
    /// Block store
    block_store: Arc<BlockStore<'static>>,

    /// Broadcaster for sending messages to peers
    broadcaster: Arc<PeerBroadcaster>,

    /// Peer registry for tracking peers
    peer_registry: Arc<PeerRegistry>,

    /// Reputation system for tracking peer behavior
    reputation: Option<Arc<ReputationSystem>>,

    /// Maximum number of blocks to return in a range request
    max_blocks_per_request: u64,
}

impl StorageIntegration {
    /// Create a new storage integration
    pub fn new(
        block_store: Arc<BlockStore<'static>>,
        broadcaster: Arc<PeerBroadcaster>,
        peer_registry: Arc<PeerRegistry>,
    ) -> Self {
        Self {
            block_store,
            broadcaster,
            peer_registry,
            reputation: None,
            max_blocks_per_request: 100,
        }
    }

    /// Set the reputation system
    pub fn with_reputation(mut self, reputation: Arc<ReputationSystem>) -> Self {
        self.reputation = Some(reputation);
        self
    }

    /// Set the maximum blocks per request
    pub fn with_max_blocks_per_request(mut self, max_blocks: u64) -> Self {
        self.max_blocks_per_request = max_blocks;
        self
    }

    /// Handle a block from the network
    pub async fn handle_block(&self, block: Block, source_peer: &str) -> Result<(), StorageError> {
        info!("Handling block from peer {}: height={}", source_peer, block.height);

        // Check if the block already exists
        if let Ok(Some(_)) = self.block_store.get_block_by_hash(&block.hash) {
            debug!("Block already exists: height={}", block.height);

            // Update peer reputation (duplicate data)
            if let Some(reputation) = &self.reputation {
                reputation.update_score(source_peer, ReputationEvent::DuplicateData);
            }

            return Ok(());
        }

        // Check if the parent block exists
        if block.height > 0 && matches!(self.block_store.get_block_by_hash(&block.prev_hash), Ok(None) | Err(_)) {
            warn!("Block has unknown parent: height={}", block.height);

            // Request the parent block
            self.request_block(block.height - 1, source_peer).await?;

            return Err(StorageError::BlockNotFound(block.height - 1));
        }

        // Store the block
        self.block_store.put_block(&block);
        info!("Block added to store: height={}", block.height);

        // Update peer reputation (good block)
        if let Some(reputation) = &self.reputation {
            reputation.update_score(source_peer, ReputationEvent::GoodBlock);
        }

        // Broadcast to other peers
        self.broadcast_block(block, source_peer).await?;

        Ok(())
    }

    /// Broadcast a block to other peers
    async fn broadcast_block(&self, block: Block, source_peer: &str) -> Result<(), StorageError> {
        match self.broadcaster.broadcast_except(
            NetMessage::NewBlock(block),
            source_peer,
        ).await {
            Ok(_) => {
                debug!("Block broadcast to peers");
                Ok(())
            },
            Err(e) => {
                error!("Failed to broadcast block: {:?}", e);
                Err(StorageError::NetworkError(format!("Broadcast failed: {}", e)))
            }
        }
    }

    /// Handle a sync request
    pub async fn handle_sync_request(&self, request: SyncRequest, source_peer: &str) -> Result<(), StorageError> {
        match request {
            SyncRequest::GetBlock(height) => {
                self.handle_get_block(height, source_peer).await
            },
            SyncRequest::GetBlocks(start_height, end_height) => {
                self.handle_get_blocks(start_height, end_height, source_peer).await
            },
            SyncRequest::GetLatestBlock => {
                self.handle_get_latest_block(source_peer).await
            },
            SyncRequest::GetChainState => {
                self.handle_get_chain_state(source_peer).await
            },
            SyncRequest::Legacy { .. } => {
                // Legacy request not supported
                warn!("Legacy sync request not supported from peer {}", source_peer);
                Err(StorageError::NetworkError(format!("Unsupported request")))
            },
        }
    }

    /// Handle a request for a single block
    async fn handle_get_block(&self, height: u64, source_peer: &str) -> Result<(), StorageError> {
        debug!("Handling GetBlock request from peer {}: height={}", source_peer, height);

        // Get the block from the store
        let block = self.block_store.get_block_by_height(height);

        // Send the response
        match self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlock(block.ok().flatten()),
        ).await {
            Ok(true) => {
                debug!("Block response sent to peer {}", source_peer);

                // Update peer reputation (useful request)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(source_peer, ReputationEvent::UsefulData);
                }

                Ok(())
            },
            Ok(false) => {
                error!("Failed to send block response to peer {}", source_peer);
                Err(StorageError::NetworkError(format!("Send failed")))
            },
            Err(e) => {
                error!("Error sending block response to peer {}: {}", source_peer, e);
                Err(StorageError::NetworkError(format!("Send error: {}", e)))
            }
        }
    }

    /// Handle a request for a range of blocks
    async fn handle_get_blocks(&self, start_height: u64, end_height: u64, source_peer: &str) -> Result<(), StorageError> {
        debug!("Handling GetBlocks request from peer {}: {}..{}", source_peer, start_height, end_height);

        // Validate the range
        if start_height > end_height {
            return Err(StorageError::InvalidRange(format!("Start height {} is greater than end height {}", start_height, end_height)));
        }

        // Limit the range to avoid excessive responses
        let limited_end = start_height + (end_height - start_height).min(self.max_blocks_per_request);

        // Get the blocks from the store
        let mut blocks = Vec::new();
        for height in start_height..=limited_end {
            if let Ok(Some(block)) = self.block_store.get_block_by_height(height) {
                blocks.push(block);
            }
        }

        // Send the response
        match self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlockRange(blocks.clone()),
        ).await {
            Ok(true) => {
                debug!("Block range response sent to peer {}: {} blocks", source_peer, blocks.len());

                // Update peer reputation (useful request)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(source_peer, ReputationEvent::UsefulData);
                }

                Ok(())
            },
            Ok(false) => {
                error!("Failed to send block range response to peer {}", source_peer);
                Err(StorageError::NetworkError(format!("Send failed")))
            },
            Err(e) => {
                error!("Error sending block range response to peer {}: {}", source_peer, e);
                Err(StorageError::NetworkError(format!("Send error: {}", e)))
            }
        }
    }

    /// Handle a request for the latest block
    async fn handle_get_latest_block(&self, source_peer: &str) -> Result<(), StorageError> {
        debug!("Handling GetLatestBlock request from peer {}", source_peer);

        // Get the latest height
        let latest_height = self.block_store.get_latest_height().unwrap_or(0);

        // Get the latest block
        let block = self.block_store.get_block_by_height(latest_height);

        // Send the response
        match self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlock(block.ok().flatten()),
        ).await {
            Ok(true) => {
                debug!("Latest block response sent to peer {}", source_peer);

                // Update peer reputation (useful request)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(source_peer, ReputationEvent::UsefulData);
                }

                Ok(())
            },
            Ok(false) => {
                error!("Failed to send latest block response to peer {}", source_peer);
                Err(StorageError::NetworkError(format!("Send failed")))
            },
            Err(e) => {
                error!("Error sending latest block response to peer {}: {}", source_peer, e);
                Err(StorageError::NetworkError(format!("Send error: {}", e)))
            }
        }
    }

    /// Handle a request for the chain state
    async fn handle_get_chain_state(&self, source_peer: &str) -> Result<(), StorageError> {
        debug!("Handling GetChainState request from peer {}", source_peer);

        // Get the latest height
        let latest_height = self.block_store.get_latest_height().unwrap_or(0);

        // In a real implementation, we would send the chain state
        // For now, just send the latest block
        let block = self.block_store.get_block_by_height(latest_height);

        // Send the response
        match self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlock(block.ok().flatten()),
        ).await {
            Ok(true) => {
                debug!("Chain state response sent to peer {}", source_peer);

                // Update peer reputation (useful request)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(source_peer, ReputationEvent::UsefulData);
                }

                Ok(())
            },
            Ok(false) => {
                error!("Failed to send chain state response to peer {}", source_peer);
                Err(StorageError::NetworkError(format!("Send failed")))
            },
            Err(e) => {
                error!("Error sending chain state response to peer {}: {}", source_peer, e);
                Err(StorageError::NetworkError(format!("Send error: {}", e)))
            }
        }
    }

    /// Request a block from a peer
    async fn request_block(&self, height: u64, peer_id: &str) -> Result<(), StorageError> {
        debug!("Requesting block from peer {}: height={}", peer_id, height);

        match self.broadcaster.send_to_peer(
            peer_id,
            NetMessage::RequestBlock(height),
        ).await {
            Ok(true) => {
                debug!("Block request sent to peer {}", peer_id);
                Ok(())
            },
            Ok(false) => {
                error!("Failed to send block request to peer {}", peer_id);
                Err(StorageError::NetworkError(format!("Send failed")))
            },
            Err(e) => {
                error!("Error sending block request to peer {}: {}", peer_id, e);
                Err(StorageError::NetworkError(format!("Send error: {}", e)))
            }
        }
    }

    /// Request a range of blocks from a peer
    pub async fn request_block_range(&self, start_height: u64, end_height: u64, peer_id: &str) -> Result<(), StorageError> {
        debug!("Requesting block range from peer {}: {}..{}", peer_id, start_height, end_height);

        // Validate the range
        if start_height > end_height {
            return Err(StorageError::InvalidRange(format!("Start height {} is greater than end height {}", start_height, end_height)));
        }

        match self.broadcaster.send_to_peer(
            peer_id,
            NetMessage::RequestBlockRange { start_height, end_height },
        ).await {
            Ok(true) => {
                debug!("Block range request sent to peer {}", peer_id);
                Ok(())
            },
            Ok(false) => {
                error!("Failed to send block range request to peer {}", peer_id);
                Err(StorageError::NetworkError(format!("Send failed")))
            },
            Err(e) => {
                error!("Error sending block range request to peer {}: {}", peer_id, e);
                Err(StorageError::NetworkError(format!("Send error: {}", e)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_storage_integration() {
        // Create dependencies
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let peer_registry = Arc::new(PeerRegistry::new());

        // Create integration
        let integration = StorageIntegration::new(
            block_store.clone(),
            broadcaster,
            peer_registry,
        );

        // Create a genesis block
        let genesis = Block {
            height: 0,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        // Handle the genesis block
        let result = integration.handle_block(genesis.clone(), "peer1").await;
        assert!(result.is_ok());

        // Create a child block
        let block1 = Block {
            height: 1,
            hash: [1u8; 32],
            prev_hash: [0u8; 32], // Points to genesis
            timestamp: 10,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        // Handle the child block
        let result = integration.handle_block(block1.clone(), "peer1").await;
        assert!(result.is_ok());
    }
}
