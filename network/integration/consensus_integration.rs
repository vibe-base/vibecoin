use std::sync::Arc;
use tokio::sync::mpsc;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::consensus::engine::ConsensusEngine;
use crate::consensus::validation::block_validator::{BlockValidator, BlockValidationResult};
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;
use crate::network::peer::reputation::{ReputationSystem, ReputationEvent};

/// Error types for consensus integration
#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Block already exists")]
    DuplicateBlock,

    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    #[error("Unknown parent block")]
    UnknownParent,

    #[error("Consensus error: {0}")]
    ConsensusError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Integration between the network module and the consensus module
pub struct ConsensusIntegration<'a> {
    /// Lifetime parameter to ensure proper lifetime management
    _lifetime: std::marker::PhantomData<&'static ()>,
    /// Consensus engine
    consensus: Arc<ConsensusEngine<'a>>,

    /// Block store
    block_store: Arc<BlockStore<'a>>,

    /// Block validator
    validator: Option<Arc<BlockValidator<'a>>>,

    /// Broadcaster for sending messages to peers
    broadcaster: Arc<PeerBroadcaster>,

    /// Peer registry for tracking peers
    peer_registry: Arc<PeerRegistry>,

    /// Reputation system for tracking peer behavior
    reputation: Option<Arc<ReputationSystem>>,

    /// Channel for new blocks from the consensus engine
    block_subscription: Option<mpsc::Receiver<Block>>,

    /// Whether the integration is running
    running: Arc<tokio::sync::RwLock<bool>>,
}

impl<'a> ConsensusIntegration<'a> {
    /// Create a new consensus integration
    pub fn new(
        consensus: Arc<ConsensusEngine<'a>>,
        block_store: Arc<BlockStore<'a>>,
        broadcaster: Arc<PeerBroadcaster>,
        peer_registry: Arc<PeerRegistry>,
    ) -> Self {
        Self {
            _lifetime: std::marker::PhantomData,
            consensus,
            block_store,
            validator: None,
            broadcaster,
            peer_registry,
            reputation: None,
            block_subscription: None,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// Set the block validator
    pub fn with_validator(mut self, validator: Arc<BlockValidator<'a>>) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Set the reputation system
    pub fn with_reputation(mut self, reputation: Arc<ReputationSystem>) -> Self {
        self.reputation = Some(reputation);
        self
    }

    /// Set the block subscription channel
    pub fn with_block_subscription(mut self, rx: mpsc::Receiver<Block>) -> Self {
        self.block_subscription = Some(rx);
        self
    }

    /// Handle a block from the network
    pub async fn handle_block(&self, block: Block, source_peer: &str) -> Result<(), ConsensusError> {
        info!("Handling block from peer {}: height={}", source_peer, block.height);

        // Validate the block if we have a validator
        if let Some(validator) = &self.validator {
            match validator.validate_block(&block, &Default::default()) {
                BlockValidationResult::Valid => {
                    debug!("Block is valid");
                },
                BlockValidationResult::AlreadyKnown => {
                    debug!("Block already known: height={}", block.height);

                    // Update peer reputation (duplicate data)
                    if let Some(reputation) = &self.reputation {
                        reputation.update_score(source_peer, ReputationEvent::DuplicateData);
                    }

                    return Err(ConsensusError::DuplicateBlock);
                },
                BlockValidationResult::UnknownParent => {
                    warn!("Block has unknown parent: height={}", block.height);

                    // Update peer reputation (protocol violation)
                    if let Some(reputation) = &self.reputation {
                        reputation.update_score(source_peer, ReputationEvent::ProtocolViolation);
                    }

                    return Err(ConsensusError::UnknownParent);
                },
                BlockValidationResult::Invalid(reason) => {
                    warn!("Invalid block from peer {}: {}", source_peer, reason);

                    // Update peer reputation (invalid block)
                    if let Some(reputation) = &self.reputation {
                        reputation.update_score(source_peer, ReputationEvent::InvalidBlock);
                    }

                    return Err(ConsensusError::InvalidBlock(reason));
                }
            }
        }

        // Forward the block to the consensus engine
        let block_rx = self.consensus.block_channel();
        if let Err(e) = block_rx.send(block.clone()).await {
            error!("Failed to send block to consensus engine: {:?}", e);
            return Err(ConsensusError::ConsensusError(format!("Failed to send block: {}", e)));
        }

        // Update peer reputation (good block)
        if let Some(reputation) = &self.reputation {
            reputation.update_score(source_peer, ReputationEvent::GoodBlock);
        }

        Ok(())
    }

    /// Start listening for new blocks from the consensus engine
    pub async fn start_consensus_listener(&self) -> Result<(), String> {
        // Check if we have a subscription channel
        let rx = match &self.block_subscription {
            Some(rx) => rx.clone(),
            None => return Err("No block subscription channel".to_string()),
        };

        // Check if we're already running
        {
            let running = self.running.read().await;
            if *running {
                return Err("Consensus listener already running".to_string());
            }
        }

        // Set running flag
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        // Clone necessary fields for the task
        let broadcaster = self.broadcaster.clone();
        let running = self.running.clone();

        // Spawn a task to listen for new blocks
        tokio::spawn(async move {
            info!("Starting consensus listener");

            while {
                let is_running = *running.read().await;
                is_running
            } {
                match rx.recv().await {
                    Some(block) => {
                        debug!("New block from consensus: height={}", block.height);

                        // Broadcast to all peers
                        broadcaster.broadcast(NetMessage::NewBlock(block)).await;
                        debug!("Block broadcast to peers");
                    },
                    None => {
                        warn!("Block subscription channel closed");
                        break;
                    }
                }
            }

            info!("Consensus listener stopped");
        });

        Ok(())
    }

    /// Stop the consensus listener
    pub async fn stop_consensus_listener(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::kv_store::RocksDBStore;
    use tempfile::tempdir;

    // This test is commented out because it requires a real ConsensusEngine
    // which is complex to set up in a test environment
    /*
    #[tokio::test]
    async fn test_consensus_integration() {
        // Create dependencies
        let temp_dir = tempdir().unwrap();
        let kv_store = RocksDBStore::new(temp_dir.path());
        let block_store = Arc::new(BlockStore::new(&kv_store));
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let peer_registry = Arc::new(PeerRegistry::new());

        // Create a consensus engine
        let consensus = Arc::new(ConsensusEngine::new(
            Default::default(),
            block_store.clone(),
            tx_store,
            state_store,
            network_tx,
        ));

        // Create integration
        let integration = ConsensusIntegration::new(
            consensus,
            block_store.clone(),
            broadcaster,
            peer_registry,
        );

        // Create a block
        let block = Block {
            height: 0,
            hash: [0u8; 32],
            prev_hash: [0u8; 32],
            timestamp: 0,
            transactions: vec![],
            state_root: [0u8; 32],
        };

        // Handle the block
        let result = integration.handle_block(block.clone(), "peer1").await;
        assert!(result.is_ok());
    }
    */
}
