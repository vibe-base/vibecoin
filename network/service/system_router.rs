use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::network::types::message::NetMessage;
use crate::network::service::router::MessageRouter;
use crate::storage::block_store::{Block, BlockStore};
use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::mempool::{Mempool, MempoolError};
use crate::consensus::engine::ConsensusEngine;
use crate::network::peer::broadcaster::PeerBroadcaster;

/// System router that connects network messages to the appropriate subsystems
pub struct SystemRouter {
    /// Message router for network messages
    router: Arc<MessageRouter>,

    /// Mempool for transaction processing
    mempool: Option<Arc<Mempool>>,

    /// Block store for block storage
    block_store: Option<Arc<BlockStore<'static>>>,

    /// Transaction store for transaction storage
    tx_store: Option<Arc<TxStore<'static>>>,

    /// Consensus engine for block validation
    consensus: Option<Arc<ConsensusEngine>>,

    /// Peer broadcaster for sending messages to peers
    broadcaster: Option<Arc<PeerBroadcaster>>,
}

impl SystemRouter {
    /// Create a new system router
    pub fn new(router: Arc<MessageRouter>) -> Self {
        Self {
            router,
            mempool: None,
            block_store: None,
            tx_store: None,
            consensus: None,
            broadcaster: None,
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

    /// Set the peer broadcaster
    pub fn with_broadcaster(mut self, broadcaster: Arc<PeerBroadcaster>) -> Self {
        self.broadcaster = Some(broadcaster);
        self
    }

    /// Initialize the system router by registering handlers
    pub async fn initialize(&self) -> Result<(), String> {
        // Register handlers for different message types
        self.register_transaction_handler().await?;
        self.register_block_handler().await?;
        self.register_block_request_handler().await?;
        self.register_block_range_request_handler().await?;

        Ok(())
    }

    /// Register handler for transaction messages
    async fn register_transaction_handler(&self) -> Result<(), String> {
        let mempool = self.mempool.clone();
        let broadcaster = self.broadcaster.clone();

        if let Some(mempool) = mempool {
            let router = self.router.clone();

            router.register_handler("new_transaction", move |node_id, message| {
                if let NetMessage::NewTransaction(tx) = message {
                    debug!("Received transaction from {}: {:?}", node_id, tx.tx_id);

                    // Process the transaction asynchronously
                    let mempool_clone = mempool.clone();
                    let broadcaster_clone = broadcaster.clone();
                    let tx_clone = tx.clone();

                    // Convert from storage TransactionRecord to mempool TransactionRecord
                    let mempool_tx = crate::mempool::types::TransactionRecord {
                        tx_id: tx_clone.tx_id,
                        sender: tx_clone.sender,
                        recipient: tx_clone.recipient,
                        value: tx_clone.value,
                        gas_price: tx_clone.gas_price,
                        gas_limit: tx_clone.gas_limit,
                        nonce: tx_clone.nonce,
                        timestamp: tx_clone.timestamp,
                        signature: crate::crypto::signer::VibeSignature::new([0; 64]), // Placeholder
                        data: tx_clone.data.clone(),
                    };

                    tokio::spawn(async move {
                        // Try to insert the transaction into the mempool
                        match mempool_clone.insert(mempool_tx).await {
                            Ok(_) => {
                                info!("Transaction added to mempool: {:?}", tx_clone.tx_id);

                                // Broadcast the transaction to other peers
                                if let Some(broadcaster) = broadcaster_clone {
                                    broadcaster.broadcast(NetMessage::NewTransaction(tx_clone)).await;
                                }
                            }
                            Err(MempoolError::DuplicateTransaction) => {
                                debug!("Duplicate transaction: {:?}", tx_clone.tx_id);
                            }
                            Err(e) => {
                                warn!("Failed to add transaction to mempool: {:?}", e);
                            }
                        }
                    });

                    true
                } else {
                    false
                }
            }).await;
        }

        Ok(())
    }

    /// Register handler for block messages
    async fn register_block_handler(&self) -> Result<(), String> {
        let block_store = self.block_store.clone();
        let consensus = self.consensus.clone();
        let broadcaster = self.broadcaster.clone();

        if let Some(block_store) = block_store {
            let router = self.router.clone();

            router.register_handler("new_block", move |node_id, message| {
                if let NetMessage::NewBlock(block) = message {
                    info!("Received block from {}: height={}", node_id, block.height);

                    // Process the block asynchronously
                    let block_store_clone = block_store.clone();
                    let consensus_clone = consensus.clone();
                    let broadcaster_clone = broadcaster.clone();
                    let block_clone = block.clone();

                    tokio::spawn(async move {
                        // If we have a consensus engine, validate and process the block
                        if let Some(consensus) = consensus_clone {
                            // The consensus engine will handle validation and storage
                            // It will also update the mempool to remove included transactions
                            let block_rx = consensus.block_channel();
                            if let Err(e) = block_rx.send(block_clone.clone()).await {
                                error!("Failed to send block to consensus engine: {:?}", e);
                            }
                        } else {
                            // If we don't have a consensus engine, just store the block
                            block_store_clone.put_block(&block_clone);

                            // Broadcast the block to other peers
                            if let Some(broadcaster) = broadcaster_clone {
                                broadcaster.broadcast(NetMessage::NewBlock(block_clone)).await;
                            }
                        }
                    });

                    true
                } else {
                    false
                }
            }).await;
        }

        Ok(())
    }

    /// Register handler for block request messages
    async fn register_block_request_handler(&self) -> Result<(), String> {
        let block_store = self.block_store.clone();
        let broadcaster = self.broadcaster.clone();

        if let Some(block_store) = block_store {
            let router = self.router.clone();

            router.register_handler("request_block", move |node_id, message| {
                if let NetMessage::RequestBlock(height) = message {
                    debug!("Received block request from {}: height={}", node_id, height);

                    // Create static clones of the resources we need
                    let block_store_clone = block_store.clone();
                    let broadcaster_clone = broadcaster.clone();
                    let node_id_clone = node_id.to_string();

                    // Process the request in a separate task to avoid lifetime issues
                    tokio::task::spawn(async move {
                        // Get the block from the store
                        let block_result = block_store_clone.get_block_by_height(height);

                        // Send the response
                        if let Some(ref _broadcaster) = broadcaster_clone {
                            let response = match block_result {
                                Ok(block_option) => NetMessage::ResponseBlock(block_option),
                                Err(e) => {
                                    error!("Error retrieving block: {:?}", e);
                                    NetMessage::ResponseBlock(None)
                                }
                            };
                            if let Some(ref broadcaster) = broadcaster_clone {
                                match broadcaster.send_to_peer(&node_id_clone, response).await {
                                    Ok(_) => {},
                                    Err(e) => error!("Failed to send block response to {}: {}", node_id_clone, e)
                                }
                            }
                        }
                    });

                    true
                } else {
                    false
                }
            }).await;
        }

        Ok(())
    }

    /// Register handler for block range request messages
    async fn register_block_range_request_handler(&self) -> Result<(), String> {
        let block_store = self.block_store.clone();
        let broadcaster = self.broadcaster.clone();

        if let Some(block_store) = block_store {
            let router = self.router.clone();

            router.register_handler("request_block_range", move |node_id, message| {
                if let NetMessage::RequestBlockRange { start_height, end_height } = message {
                    debug!("Received block range request from {}: {}..{}", node_id, start_height, end_height);

                    // Create static clones of the resources we need
                    let block_store_clone = block_store.clone();
                    let broadcaster_clone = broadcaster.clone();
                    let node_id_clone = node_id.to_string();

                    // Process the request in a separate task to avoid lifetime issues
                    tokio::task::spawn(async move {
                        // Get the blocks from the store
                        let mut blocks = Vec::new();
                        for height in start_height..=end_height {
                            match block_store_clone.get_block_by_height(height) {
                                Ok(Some(block)) => blocks.push(block),
                                Ok(None) => {}, // Block not found at this height
                                Err(e) => {
                                    error!("Error retrieving block at height {}: {:?}", height, e);
                                }
                            }
                        }

                        // Send the response
                        if let Some(broadcaster) = broadcaster_clone {
                            let response = NetMessage::ResponseBlockRange(blocks);
                            match broadcaster.send_to_peer(&node_id_clone, response).await {
                                Ok(_) => {},
                                Err(e) => error!("Failed to send block range response to {}: {}", node_id_clone, e)
                            }
                        }
                    });

                    true
                } else {
                    false
                }
            }).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::types::message::NetMessage;
    use crate::storage::tx_store::TransactionRecord;
    use crate::storage::block_store::Block;
    use crate::network::service::router::MessageRouter;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_system_router_creation() {
        // Create a message router
        let router = Arc::new(MessageRouter::new());

        // Create a system router
        let system_router = SystemRouter::new(router);

        // The system router should be created successfully
        assert!(system_router.mempool.is_none());
        assert!(system_router.block_store.is_none());
        assert!(system_router.consensus.is_none());
    }
}
