use std::sync::Arc;
use tokio::sync::mpsc;
use log::{debug, error, info, warn};

use crate::mempool::{Mempool, MempoolError};
use crate::storage::tx_store::TransactionRecord;
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;
use crate::network::peer::reputation::{ReputationSystem, ReputationEvent};
use crate::crypto::signer::VibeSignature;

/// Integration between the network module and the mempool
pub struct MempoolIntegration {
    /// Lifetime parameter to ensure proper lifetime management
    _lifetime: std::marker::PhantomData<&'static ()>,
    /// Mempool for transaction processing
    mempool: Arc<Mempool>,

    /// Broadcaster for sending messages to peers
    broadcaster: Arc<PeerBroadcaster>,

    /// Peer registry for tracking peers
    peer_registry: Arc<PeerRegistry>,

    /// Reputation system for tracking peer behavior
    reputation: Option<Arc<ReputationSystem>>,

    /// Channel for new transactions from the mempool
    tx_subscription: Option<mpsc::Receiver<TransactionRecord>>,

    /// Whether the integration is running
    running: Arc<tokio::sync::RwLock<bool>>,
}

impl MempoolIntegration {
    /// Create a new mempool integration
    pub fn new(
        mempool: Arc<Mempool>,
        broadcaster: Arc<PeerBroadcaster>,
        peer_registry: Arc<PeerRegistry>,
    ) -> Self {
        Self {
            _lifetime: std::marker::PhantomData,
            mempool,
            broadcaster,
            peer_registry,
            reputation: None,
            tx_subscription: None,
            running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// Set the reputation system
    pub fn with_reputation(mut self, reputation: Arc<ReputationSystem>) -> Self {
        self.reputation = Some(reputation);
        self
    }

    /// Set the transaction subscription channel
    pub fn with_tx_subscription(mut self, rx: mpsc::Receiver<TransactionRecord>) -> Self {
        self.tx_subscription = Some(rx);
        self
    }

    /// Handle a transaction from the network
    pub async fn handle_transaction(&self, tx: crate::storage::tx_store::TransactionRecord, source_peer: &str) -> Result<(), MempoolError> {
        debug!("Handling transaction from peer {}: {:?}", source_peer, tx.tx_id);

        // Convert to mempool transaction record
        let mempool_tx = crate::mempool::types::TransactionRecord {
            tx_id: tx.tx_id,
            sender: tx.sender,
            recipient: tx.recipient,
            value: tx.value,
            gas_price: tx.gas_price,
            gas_limit: tx.gas_limit,
            nonce: tx.nonce,
            timestamp: tx.timestamp,
            data: tx.data.clone(),
            signature: VibeSignature::new([0; 64]), // Placeholder signature
        };

        // Add to mempool
        match self.mempool.insert(mempool_tx).await {
            Ok(_) => {
                info!("Transaction added to mempool: {:?}", tx.tx_id);

                // Update peer reputation (good transaction)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(source_peer, ReputationEvent::GoodTransaction);
                }

                // Broadcast to other peers
                self.broadcast_transaction(tx, source_peer).await;

                Ok(())
            },
            Err(e @ MempoolError::DuplicateTransaction) => {
                debug!("Duplicate transaction from peer {}: {:?}", source_peer, tx.tx_id);

                // Update peer reputation (duplicate data)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(source_peer, ReputationEvent::DuplicateData);
                }

                Err(e)
            },
            Err(e) => {
                warn!("Failed to add transaction to mempool: {:?}", e);

                // Update peer reputation (invalid transaction)
                if let Some(reputation) = &self.reputation {
                    reputation.update_score(source_peer, ReputationEvent::InvalidTransaction);
                }

                Err(e)
            }
        }
    }

    /// Broadcast a transaction to other peers
    async fn broadcast_transaction(&self, tx: crate::storage::tx_store::TransactionRecord, source_peer: &str) {
        match self.broadcaster.broadcast_except(
            NetMessage::NewTransaction(tx),
            source_peer,
        ).await {
            Ok(_) => {
                debug!("Transaction broadcast to peers");
            },
            Err(e) => {
                error!("Failed to broadcast transaction: {:?}", e);
            }
        }
    }

    /// Start listening for new transactions from the mempool
    pub async fn start_mempool_listener(&self) -> Result<(), String> {
        // Check if we have a subscription channel
        let rx = match &self.tx_subscription {
            Some(rx) => rx.clone(),
            None => return Err("No transaction subscription channel".to_string()),
        };

        // Check if we're already running
        {
            let running = self.running.read().await;
            if *running {
                return Err("Mempool listener already running".to_string());
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

        // Spawn a task to listen for new transactions
        tokio::spawn(async move {
            info!("Starting mempool listener");

            while {
                let is_running = *running.read().await;
                is_running
            } {
                match rx.recv().await {
                    Some(tx) => {
                        debug!("New transaction from mempool: {:?}", tx.tx_id);

                        // Broadcast to all peers
                        broadcaster.broadcast(NetMessage::NewTransaction(tx)).await;
                        debug!("Transaction broadcast to peers");
                    },
                    None => {
                        warn!("Transaction subscription channel closed");
                        break;
                    }
                }
            }

            info!("Mempool listener stopped");
        });

        Ok(())
    }

    /// Stop the mempool listener
    pub async fn stop_mempool_listener(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mempool::pool::MempoolConfig;

    #[tokio::test]
    async fn test_mempool_integration() {
        // Create dependencies
        let mempool = Arc::new(Mempool::with_config(MempoolConfig::default()));
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let peer_registry = Arc::new(PeerRegistry::new());

        // Create integration
        let integration = MempoolIntegration::new(
            mempool.clone(),
            broadcaster,
            peer_registry,
        );

        // Create a transaction
        let tx = TransactionRecord {
            tx_id: [1u8; 32],
            sender: [2u8; 32],
            recipient: [3u8; 32],
            value: 100,
            gas_used: 10,
            block_height: 0,
        };

        // Handle the transaction
        let result = integration.handle_transaction(tx.clone(), "peer1").await;
        assert!(result.is_ok());

        // Try to handle the same transaction again
        let result = integration.handle_transaction(tx.clone(), "peer2").await;
        assert!(matches!(result, Err(MempoolError::DuplicateTransaction)));
    }
}
