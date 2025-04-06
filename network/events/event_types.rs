use crate::storage::block_store::Block;
use crate::storage::tx_store::TransactionRecord;
use crate::network::peer::advanced_registry::PeerId;
use crate::network::service::advanced_router::SyncRequest;

/// Network event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// Peer events
    Peer,
    
    /// Block events
    Block,
    
    /// Transaction events
    Transaction,
    
    /// Sync events
    Sync,
    
    /// Network events
    Network,
    
    /// All events
    All,
}

/// Result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Whether the sync was successful
    pub success: bool,
    
    /// The number of blocks synced
    pub blocks_synced: u64,
    
    /// The start height
    pub start_height: u64,
    
    /// The end height
    pub end_height: u64,
    
    /// Error message if any
    pub error: Option<String>,
}

/// Network events
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Peer connected
    PeerConnected(PeerId),
    
    /// Peer disconnected
    PeerDisconnected(PeerId, Option<String>),
    
    /// Block received
    BlockReceived(Block, PeerId),
    
    /// Transaction received
    TransactionReceived(TransactionRecord, PeerId),
    
    /// Sync requested
    SyncRequested(SyncRequest, PeerId),
    
    /// Sync completed
    SyncCompleted(SyncResult),
    
    /// Network started
    NetworkStarted,
    
    /// Network stopped
    NetworkStopped,
    
    /// Peer banned
    PeerBanned(PeerId, String),
    
    /// Peer unbanned
    PeerUnbanned(PeerId),
}

impl NetworkEvent {
    /// Get the event type
    pub fn get_type(&self) -> EventType {
        match self {
            NetworkEvent::PeerConnected(_) => EventType::Peer,
            NetworkEvent::PeerDisconnected(_, _) => EventType::Peer,
            NetworkEvent::BlockReceived(_, _) => EventType::Block,
            NetworkEvent::TransactionReceived(_, _) => EventType::Transaction,
            NetworkEvent::SyncRequested(_, _) => EventType::Sync,
            NetworkEvent::SyncCompleted(_) => EventType::Sync,
            NetworkEvent::NetworkStarted => EventType::Network,
            NetworkEvent::NetworkStopped => EventType::Network,
            NetworkEvent::PeerBanned(_, _) => EventType::Peer,
            NetworkEvent::PeerUnbanned(_) => EventType::Peer,
        }
    }
}
