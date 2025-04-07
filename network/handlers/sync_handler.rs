use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;
use crate::network::service::advanced_router::SyncRequest;

/// Error types for sync handler
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("Block not found: height={0}")]
    BlockNotFound(u64),

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Unsupported request")]
    UnsupportedRequest,

    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Handler for sync messages
pub struct SyncHandler {
    /// Block store
    block_store: Arc<BlockStore<'static>>,

    /// Broadcaster for sending messages to peers
    broadcaster: Arc<PeerBroadcaster>,

    /// Peer registry for tracking peers
    peer_registry: Arc<PeerRegistry>,

    /// Maximum number of blocks to return in a range request
    max_blocks_per_request: u64,
}

impl SyncHandler {
    /// Create a new sync handler
    pub fn new(
        block_store: Arc<BlockStore<'static>>,
        broadcaster: Arc<PeerBroadcaster>,
        peer_registry: Arc<PeerRegistry>,
    ) -> Self {
        Self {
            block_store,
            broadcaster,
            peer_registry,
            max_blocks_per_request: 100,
        }
    }

    /// Set the maximum blocks per request
    pub fn with_max_blocks_per_request(mut self, max_blocks: u64) -> Self {
        self.max_blocks_per_request = max_blocks;
        self
    }

    /// Handle a sync request
    pub async fn handle(&self, request: SyncRequest, source_peer: &str) -> Result<(), HandlerError> {
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
                Err(HandlerError::UnsupportedRequest)
            },
        }
    }

    /// Handle a request for a single block
    async fn handle_get_block(&self, height: u64, source_peer: &str) -> Result<(), HandlerError> {
        debug!("Handling GetBlock request from peer {}: height={}", source_peer, height);

        // Get the block from the store
        let block = self.block_store.get_block_by_height(height);

        // Send the response
        if self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlock(block.ok().flatten()),
        ).await {
            Ok(())
        } else {
            error!("Failed to send block response to peer {}", source_peer);
            Err(HandlerError::NetworkError(format!("Send failed")))
        }
    }

    /// Handle a request for a range of blocks
    async fn handle_get_blocks(&self, start_height: u64, end_height: u64, source_peer: &str) -> Result<(), HandlerError> {
        debug!("Handling GetBlocks request from peer {}: {}..{}", source_peer, start_height, end_height);

        // Validate the range
        if start_height > end_height {
            return Err(HandlerError::InvalidRange(format!("Start height {} is greater than end height {}", start_height, end_height)));
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
        if self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlockRange(blocks),
        ).await {
            Ok(())
        } else {
            error!("Failed to send block range response to peer {}", source_peer);
            Err(HandlerError::NetworkError(format!("Send failed")))
        }
    }

    /// Handle a request for the latest block
    async fn handle_get_latest_block(&self, source_peer: &str) -> Result<(), HandlerError> {
        debug!("Handling GetLatestBlock request from peer {}", source_peer);

        // Get the latest height
        let latest_height = self.block_store.get_latest_height().unwrap_or(0);

        // Get the latest block
        let block = self.block_store.get_block_by_height(latest_height);

        // Send the response
        if self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlock(block.ok().flatten()),
        ).await {
            Ok(())
        } else {
            error!("Failed to send latest block response to peer {}", source_peer);
            Err(HandlerError::NetworkError(format!("Send failed")))
        }
    }

    /// Handle a request for the chain state
    async fn handle_get_chain_state(&self, source_peer: &str) -> Result<(), HandlerError> {
        debug!("Handling GetChainState request from peer {}", source_peer);

        // Get the latest height
        let latest_height = self.block_store.get_latest_height().unwrap_or(0);

        // In a real implementation, we would send the chain state
        // For now, just send the latest block
        let block = self.block_store.get_block_by_height(latest_height);

        // Send the response
        if self.broadcaster.send_to_peer(
            source_peer,
            NetMessage::ResponseBlock(block.ok().flatten()),
        ).await {
            Ok(())
        } else {
            error!("Failed to send chain state response to peer {}", source_peer);
            Err(HandlerError::NetworkError(format!("Send failed")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sync_handler() {
        // Create dependencies
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let peer_registry = Arc::new(PeerRegistry::new());

        // Create handler
        let handler = SyncHandler::new(
            block_store.clone(),
            broadcaster,
            peer_registry,
        );

        // Create some blocks
        let genesis = Block {
            height: 0,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        let block1 = Block {
            height: 1,
            hash: [1u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 10,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        // Store the blocks
        block_store.put_block(&genesis);
        block_store.put_block(&block1);

        // Test GetBlock request
        let result = handler.handle(SyncRequest::GetBlock(0), "peer1").await;
        assert!(result.is_ok());

        // Test GetBlocks request
        let result = handler.handle(SyncRequest::GetBlocks(0, 1), "peer1").await;
        assert!(result.is_ok());

        // Test GetLatestBlock request
        let result = handler.handle(SyncRequest::GetLatestBlock, "peer1").await;
        assert!(result.is_ok());

        // Test GetChainState request
        let result = handler.handle(SyncRequest::GetChainState, "peer1").await;
        assert!(result.is_ok());
    }
}
