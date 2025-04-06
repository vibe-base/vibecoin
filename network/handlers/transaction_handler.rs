use std::sync::Arc;
use log::{debug, error, info, warn};

use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::mempool::{Mempool, MempoolError};
use crate::network::types::message::NetMessage;
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::peer::registry::PeerRegistry;

/// Error types for transaction handler
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("Transaction already exists")]
    DuplicateTransaction,
    
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    
    #[error("Mempool error: {0}")]
    MempoolError(#[from] MempoolError),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Handler for transaction messages
pub struct TransactionHandler {
    /// Mempool for transaction processing
    mempool: Arc<Mempool>,
    
    /// Transaction store
    tx_store: Option<Arc<TxStore<'static>>>,
    
    /// Broadcaster for sending messages to peers
    broadcaster: Arc<PeerBroadcaster>,
    
    /// Peer registry for tracking peers
    peer_registry: Arc<PeerRegistry>,
    
    /// Whether to validate transactions before adding to mempool
    validate_transactions: bool,
}

impl TransactionHandler {
    /// Create a new transaction handler
    pub fn new(
        mempool: Arc<Mempool>,
        broadcaster: Arc<PeerBroadcaster>,
        peer_registry: Arc<PeerRegistry>,
    ) -> Self {
        Self {
            mempool,
            tx_store: None,
            broadcaster,
            peer_registry,
            validate_transactions: true,
        }
    }
    
    /// Set the transaction store
    pub fn with_tx_store(mut self, tx_store: Arc<TxStore<'static>>) -> Self {
        self.tx_store = Some(tx_store);
        self
    }
    
    /// Set whether to validate transactions
    pub fn with_validation(mut self, validate: bool) -> Self {
        self.validate_transactions = validate;
        self
    }
    
    /// Handle a transaction message
    pub async fn handle(&self, tx: TransactionRecord, source_peer: &str) -> Result<(), HandlerError> {
        debug!("Handling transaction from peer {}: {:?}", source_peer, tx.tx_id);
        
        // Check if transaction already exists in the store
        if let Some(tx_store) = &self.tx_store {
            if tx_store.get_transaction(&tx.tx_id).is_some() {
                debug!("Transaction already in store: {:?}", tx.tx_id);
                return Err(HandlerError::DuplicateTransaction);
            }
        }
        
        // Add to mempool
        match self.mempool.insert(tx.clone()).await {
            Ok(_) => {
                info!("Transaction added to mempool: {:?}", tx.tx_id);
                
                // Broadcast to other peers
                self.broadcast_transaction(tx, source_peer).await?;
                
                // Update peer reputation (good transaction)
                self.update_peer_reputation(source_peer, true).await;
                
                Ok(())
            },
            Err(MempoolError::DuplicateTransaction) => {
                debug!("Duplicate transaction: {:?}", tx.tx_id);
                Err(HandlerError::DuplicateTransaction)
            },
            Err(e) => {
                warn!("Failed to add transaction to mempool: {:?}", e);
                
                // Update peer reputation (bad transaction)
                self.update_peer_reputation(source_peer, false).await;
                
                Err(HandlerError::MempoolError(e))
            }
        }
    }
    
    /// Broadcast a transaction to other peers
    async fn broadcast_transaction(&self, tx: TransactionRecord, source_peer: &str) -> Result<(), HandlerError> {
        match self.broadcaster.broadcast_except(
            NetMessage::NewTransaction(tx),
            source_peer,
        ).await {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to broadcast transaction: {:?}", e);
                Err(HandlerError::NetworkError(format!("Broadcast failed: {}", e)))
            }
        }
    }
    
    /// Update peer reputation based on transaction validity
    async fn update_peer_reputation(&self, peer_id: &str, is_valid: bool) {
        // In a real implementation, we would update the peer's reputation score
        // For now, just log it
        if is_valid {
            debug!("Peer {} provided a valid transaction", peer_id);
        } else {
            warn!("Peer {} provided an invalid transaction", peer_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mempool::pool::MempoolConfig;
    
    #[tokio::test]
    async fn test_transaction_handler() {
        // Create dependencies
        let mempool = Arc::new(Mempool::with_config(MempoolConfig::default()));
        let broadcaster = Arc::new(PeerBroadcaster::new());
        let peer_registry = Arc::new(PeerRegistry::new());
        
        // Create handler
        let handler = TransactionHandler::new(
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
        let result = handler.handle(tx.clone(), "peer1").await;
        assert!(result.is_ok());
        
        // Try to handle the same transaction again
        let result = handler.handle(tx.clone(), "peer2").await;
        assert!(matches!(result, Err(HandlerError::DuplicateTransaction)));
    }
}
