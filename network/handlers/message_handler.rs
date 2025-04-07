//! Message handler registry for VibeCoin blockchain
//!
//! This module provides a registry for message handlers.

use std::sync::Arc;
use std::collections::HashMap;
use log::{debug, error};

use crate::network::types::message::NetMessage;
use crate::network::service::advanced_router::MessageType;

/// Handler error type
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    
    /// Unsupported message type
    #[error("Unsupported message type: {0}")]
    UnsupportedType(String),
    
    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),
    
    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),
    
    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Message handler trait
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle a message
    async fn handle(&self, peer_id: String, message: NetMessage) -> Result<(), HandlerError>;
    
    /// Get the message type
    fn get_type(&self) -> MessageType;
}

/// Handler registry for message handlers
pub struct HandlerRegistry {
    /// Handlers by message type
    handlers: HashMap<MessageType, Arc<dyn MessageHandler>>,
}

impl HandlerRegistry {
    /// Create a new handler registry
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
    
    /// Register a handler
    pub fn register(&mut self, handler: Arc<dyn MessageHandler>) {
        let message_type = handler.get_type();
        self.handlers.insert(message_type, handler);
        debug!("Registered handler for message type: {:?}", message_type);
    }
    
    /// Get a handler by message type
    pub fn get(&self, message_type: &MessageType) -> Option<Arc<dyn MessageHandler>> {
        self.handlers.get(message_type).cloned()
    }
    
    /// Remove a handler
    pub fn remove(&mut self, message_type: &MessageType) -> Option<Arc<dyn MessageHandler>> {
        let handler = self.handlers.remove(message_type);
        if handler.is_some() {
            debug!("Removed handler for message type: {:?}", message_type);
        }
        handler
    }
    
    /// Check if a handler exists
    pub fn has_handler(&self, message_type: &MessageType) -> bool {
        self.handlers.contains_key(message_type)
    }
    
    /// Get the number of registered handlers
    pub fn count(&self) -> usize {
        self.handlers.len()
    }
    
    /// Get all registered message types
    pub fn message_types(&self) -> Vec<MessageType> {
        self.handlers.keys().cloned().collect()
    }
}
