use crate::types::error::VibecoinError;
use std::net::SocketAddr;

/// Simple message types
pub enum SimpleMessageType {
    /// Ping message
    Ping,
    /// Pong message
    Pong,
    /// New block announcement
    NewBlock,
    /// New transaction announcement
    NewTransaction,
    /// Peer discovery request
    GetPeers,
    /// Peer list response
    Peers,
}

/// Simple network message
pub struct SimpleMessage {
    /// Message type
    pub message_type: SimpleMessageType,
    /// Message content
    pub content: String,
}

impl SimpleMessage {
    /// Create a new message
    pub fn new(message_type: SimpleMessageType, content: &str) -> Self {
        SimpleMessage {
            message_type,
            content: content.to_string(),
        }
    }

    /// Serialize the message to a string
    pub fn serialize(&self) -> String {
        let type_str = match self.message_type {
            SimpleMessageType::Ping => "PING",
            SimpleMessageType::Pong => "PONG",
            SimpleMessageType::NewBlock => "BLOCK",
            SimpleMessageType::NewTransaction => "TX",
            SimpleMessageType::GetPeers => "GETPEERS",
            SimpleMessageType::Peers => "PEERS",
        };

        format!("{}: {}", type_str, self.content)
    }

    /// Parse a message from a string
    pub fn parse(message: &str) -> Result<Self, VibecoinError> {
        let parts: Vec<&str> = message.splitn(2, ":").collect();

        if parts.len() != 2 {
            return Err(VibecoinError::NetworkError("Invalid message format".to_string()));
        }

        let message_type = match parts[0] {
            "PING" => SimpleMessageType::Ping,
            "PONG" => SimpleMessageType::Pong,
            "BLOCK" => SimpleMessageType::NewBlock,
            "TX" => SimpleMessageType::NewTransaction,
            "GETPEERS" => SimpleMessageType::GetPeers,
            "PEERS" => SimpleMessageType::Peers,
            _ => return Err(VibecoinError::NetworkError(format!("Unknown message type: {}", parts[0]))),
        };

        Ok(SimpleMessage {
            message_type,
            content: parts[1].to_string(),
        })
    }
}

/// Network address
pub struct NetworkAddress {
    /// IP address and port
    pub address: String,
    /// Timestamp
    pub timestamp: u64,
}

impl NetworkAddress {
    /// Convert to a SocketAddr
    pub fn to_socket_addr(&self) -> Option<SocketAddr> {
        self.address.parse().ok()
    }

    /// Create from a SocketAddr
    pub fn from_socket_addr(addr: SocketAddr, timestamp: u64) -> Self {
        NetworkAddress {
            address: addr.to_string(),
            timestamp,
        }
    }
}
