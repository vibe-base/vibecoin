use serde::{Serialize, Deserialize};
use std::fmt;

use crate::storage::block_store::Block;
use crate::storage::tx_store::TransactionRecord;
use crate::network::types::node_info::NodeInfo;

/// Network message types for peer-to-peer communication
#[derive(Serialize, Deserialize, Clone)]
pub enum NetMessage {
    /// Initial handshake message with node information
    Handshake(NodeInfo),
    
    /// Broadcast a new block to peers
    NewBlock(Block),
    
    /// Broadcast a new transaction to peers
    NewTransaction(TransactionRecord),
    
    /// Request a block by height
    RequestBlock(u64),
    
    /// Response to a block request
    ResponseBlock(Option<Block>),
    
    /// Request a range of blocks
    RequestBlockRange {
        start_height: u64,
        end_height: u64,
    },
    
    /// Response to a block range request
    ResponseBlockRange(Vec<Block>),
    
    /// Ping message to check connection
    Ping(u64), // Nonce
    
    /// Pong response to a ping
    Pong(u64), // Same nonce as ping
    
    /// Disconnect message
    Disconnect(DisconnectReason),
}

/// Reasons for disconnecting from a peer
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DisconnectReason {
    /// Normal shutdown
    Shutdown,
    
    /// Protocol violation
    ProtocolViolation,
    
    /// Incompatible version
    IncompatibleVersion,
    
    /// Too many connections
    TooManyConnections,
    
    /// Duplicate connection
    DuplicateConnection,
    
    /// Timeout
    Timeout,
}

impl fmt::Debug for NetMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetMessage::Handshake(info) => write!(f, "Handshake({:?})", info),
            NetMessage::NewBlock(block) => write!(f, "NewBlock(height: {})", block.height),
            NetMessage::NewTransaction(tx) => write!(f, "NewTransaction(id: {:?})", tx.tx_id),
            NetMessage::RequestBlock(height) => write!(f, "RequestBlock({})", height),
            NetMessage::ResponseBlock(maybe_block) => {
                match maybe_block {
                    Some(block) => write!(f, "ResponseBlock(height: {})", block.height),
                    None => write!(f, "ResponseBlock(None)"),
                }
            },
            NetMessage::RequestBlockRange { start_height, end_height } => {
                write!(f, "RequestBlockRange({} to {})", start_height, end_height)
            },
            NetMessage::ResponseBlockRange(blocks) => {
                write!(f, "ResponseBlockRange(count: {})", blocks.len())
            },
            NetMessage::Ping(nonce) => write!(f, "Ping({})", nonce),
            NetMessage::Pong(nonce) => write!(f, "Pong({})", nonce),
            NetMessage::Disconnect(reason) => write!(f, "Disconnect({:?})", reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode;
    
    #[test]
    fn test_message_serialization() {
        // Create a sample NodeInfo
        let node_info = NodeInfo {
            version: "0.1.0".to_string(),
            node_id: "test-node".to_string(),
            listen_addr: "127.0.0.1:8765".parse().unwrap(),
        };
        
        // Create a handshake message
        let message = NetMessage::Handshake(node_info);
        
        // Serialize the message
        let serialized = bincode::serialize(&message).unwrap();
        
        // Deserialize the message
        let deserialized: NetMessage = bincode::deserialize(&serialized).unwrap();
        
        // Check that we got a handshake message
        match deserialized {
            NetMessage::Handshake(info) => {
                assert_eq!(info.version, "0.1.0");
                assert_eq!(info.node_id, "test-node");
                assert_eq!(info.listen_addr.to_string(), "127.0.0.1:8765");
            },
            _ => panic!("Expected Handshake message"),
        }
    }
}
