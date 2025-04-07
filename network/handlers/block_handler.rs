use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::consensus::engine::ConsensusEngine;
use crate::consensus::validation::block_validator::{BlockValidator, BlockValidationResult};
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;
use crate::mempool::Mempool;

/// Error types for block handler
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("Block already exists")]
    DuplicateBlock,

    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    #[error("Unknown parent block")]
    UnknownParent,

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Consensus error: {0}")]
    ConsensusError(String),
}

/// Handler for block messages
pub struct BlockHandler {
    /// Block store
    block_store: Arc<BlockStore<'static>>,

    /// Consensus engine
    consensus: Option<Arc<ConsensusEngine>>,

    /// Block validator
    validator: Option<Arc<BlockValidator<'static>>>,

    /// Mempool for transaction processing
    mempool: Option<Arc<Mempool>>,

    /// Broadcaster for sending messages to peers
    broadcaster: Arc<PeerBroadcaster>,

    /// Peer registry for tracking peers
    peer_registry: Arc<PeerRegistry>,

    /// Whether to validate blocks before adding to store
    validate_blocks: bool,
}

impl BlockHandler {
    /// Create a new block handler
    pub fn new(
        block_store: Arc<BlockStore<'static>>,
        broadcaster: Arc<PeerBroadcaster>,
        peer_registry: Arc<PeerRegistry>,
    ) -> Self {
        Self {
            block_store,
            consensus: None,
            validator: None,
            mempool: None,
            broadcaster,
            peer_registry,
            validate_blocks: true,
        }
    }

    /// Set the consensus engine
    pub fn with_consensus(mut self, consensus: Arc<ConsensusEngine>) -> Self {
        self.consensus = Some(consensus);
        self
    }

    /// Set the block validator
    pub fn with_validator(mut self, validator: Arc<BlockValidator<'static>>) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Set the mempool
    pub fn with_mempool(mut self, mempool: Arc<Mempool>) -> Self {
        self.mempool = Some(mempool);
        self
    }

    /// Set whether to validate blocks
    pub fn with_validation(mut self, validate: bool) -> Self {
        self.validate_blocks = validate;
        self
    }

    /// Handle a block message
    pub async fn handle(&self, block: Block, source_peer: &str) -> Result<(), HandlerError> {
        info!("Handling block from peer {}: height={}", source_peer, block.height);

        // If we have a consensus engine, let it handle the block
        if let Some(consensus) = &self.consensus {
            debug!("Forwarding block to consensus engine");
            let block_rx = consensus.block_channel();
            if let Err(e) = block_rx.send(block.clone()).await {
                error!("Failed to send block to consensus engine: {:?}", e);
                return Err(HandlerError::ConsensusError(format!("Failed to send block: {}", e)));
            }
            return Ok(());
        }

        // Otherwise, validate and process the block ourselves
        if self.validate_blocks {
            if let Some(validator) = &self.validator {
                match validator.validate_block(&block, &Default::default()) {
                    BlockValidationResult::Valid => {
                        debug!("Block is valid");
                    },
                    BlockValidationResult::AlreadyKnown => {
                        debug!("Block already known: height={}", block.height);
                        return Err(HandlerError::DuplicateBlock);
                    },
                    BlockValidationResult::UnknownParent => {
                        warn!("Block has unknown parent: height={}", block.height);
                        return Err(HandlerError::UnknownParent);
                    },
                    BlockValidationResult::Invalid(reason) => {
                        warn!("Invalid block from peer {}: {}", source_peer, reason);

                        // Update peer reputation (bad block)
                        self.update_peer_reputation(source_peer, false).await;

                        return Err(HandlerError::InvalidBlock(reason));
                    }
                }
            } else {
                // Basic validation if no validator is available
                if let Ok(Some(_)) = self.block_store.get_block_by_hash(&block.hash) {
                    debug!("Block already known: height={}", block.height);
                    return Err(HandlerError::DuplicateBlock);
                }

                if block.height > 0 && matches!(self.block_store.get_block_by_hash(&block.prev_hash), Ok(None) | Err(_)) {
                    warn!("Block has unknown parent: height={}", block.height);
                    return Err(HandlerError::UnknownParent);
                }
            }
        }

        // Store the block
        let _ = self.block_store.put_block(&block);
        info!("Block added to store: height={}", block.height);

        // Update mempool to remove included transactions
        if let Some(mempool) = &self.mempool {
            self.update_mempool(&block, mempool).await;
        }

        // Broadcast to other peers
        self.broadcast_block(block.clone(), source_peer).await?;

        // Update peer reputation (good block)
        self.update_peer_reputation(source_peer, true).await;

        Ok(())
    }

    /// Broadcast a block to other peers
    async fn broadcast_block(&self, block: Block, source_peer: &str) -> Result<(), HandlerError> {
        match self.broadcaster.broadcast_except(
            NetMessage::NewBlock(block),
            source_peer,
        ).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to broadcast block: {:?}", e);
                Err(HandlerError::NetworkError(format!("Broadcast failed: {}", e)))
            }
        }
    }

    /// Update mempool to remove transactions included in the block
    async fn update_mempool(&self, block: &Block, mempool: &Mempool) {
        // Mark transactions as included in a block
        mempool.mark_included(&block.transactions, block.height).await;
    }

    /// Update peer reputation based on block validity
    async fn update_peer_reputation(&self, peer_id: &str, is_valid: bool) {
        // In a real implementation, we would update the peer's reputation score
        // For now, just log it
        if is_valid {
            debug!("Peer {} provided a valid block", peer_id);
        } else {
            warn!("Peer {} provided an invalid block", peer_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_block_handler() {
        // Create dependencies
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let peer_registry = Arc::new(PeerRegistry::new());

        // Create handler
        let handler = BlockHandler::new(
            block_store.clone(),
            broadcaster,
            peer_registry,
        ).with_validation(false); // Disable validation for testing

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
        let result = handler.handle(genesis.clone(), "peer1").await;
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
        let result = handler.handle(block1.clone(), "peer1").await;
        assert!(result.is_ok());

        // Try to handle the same block again
        let result = handler.handle(block1.clone(), "peer2").await;
        assert!(matches!(result, Err(HandlerError::DuplicateBlock)));
    }
}
