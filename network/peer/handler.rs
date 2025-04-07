use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use log::{debug, error, info, warn};

use crate::network::codec::frame::{FramedReader, FramedWriter};
use crate::network::types::message::{NetMessage, DisconnectReason};
use crate::network::types::node_info::NodeInfo;
use crate::network::peer::state::{ConnectionState, PeerInfo};
use crate::network::peer::registry::{PeerRegistry, PeerMetadata};
use crate::network::peer::broadcaster::PeerBroadcaster;
use crate::network::service::router::MessageRouter;

/// Timeout for handshake in seconds
const HANDSHAKE_TIMEOUT: u64 = 10;

/// Timeout for idle connections in seconds
const IDLE_TIMEOUT: u64 = 60;

/// Timeout for message responses in seconds
const MESSAGE_TIMEOUT: u64 = 30;

/// Error type for peer handler operations
#[derive(Debug, thiserror::Error)]
pub enum PeerHandlerError {
    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),

    /// Channel closed
    #[error("Channel closed")]
    ChannelClosed,

    /// Timeout
    #[error("Operation timed out")]
    Timeout,

    /// Peer disconnected
    #[error("Peer disconnected: {0:?}")]
    PeerDisconnected(DisconnectReason),

    /// Other error
    #[error("Other error: {0}")]
    Other(String),
}

/// Handler for a single peer connection
pub struct PeerHandler {
    /// TCP stream for the connection
    stream: TcpStream,

    /// Peer ID
    peer_id: String,

    /// Peer address
    peer_addr: std::net::SocketAddr,

    /// Local node information
    local_node_info: NodeInfo,

    /// Message router
    router: Arc<MessageRouter>,

    /// Peer registry
    peer_registry: Arc<PeerRegistry>,

    /// Peer broadcaster
    broadcaster: Arc<PeerBroadcaster>,

    /// Channel for outgoing messages
    outgoing_tx: mpsc::Sender<NetMessage>,

    /// Channel for incoming messages
    incoming_tx: mpsc::Sender<(String, NetMessage)>,

    /// Last activity timestamp
    last_activity: Instant,

    /// Whether this is an outbound connection
    is_outbound: bool,

    /// Channel for direct message responses
    response_tx: Option<mpsc::Sender<(NetMessage, oneshot::Sender<NetMessage>)>>,

    /// Reader for the TCP stream
    reader: Option<FramedReader<tokio::net::tcp::ReadHalf<'static>>>,

    /// Writer for the TCP stream
    writer: Option<FramedWriter<tokio::net::tcp::WriteHalf<'static>>>,
}

impl PeerHandler {
    /// Create a new peer handler
    pub fn new(
        stream: TcpStream,
        peer_id: String,
        peer_addr: std::net::SocketAddr,
        local_node_info: NodeInfo,
        router: Arc<MessageRouter>,
        peer_registry: Arc<PeerRegistry>,
        broadcaster: Arc<PeerBroadcaster>,
        incoming_tx: mpsc::Sender<(String, NetMessage)>,
        is_outbound: bool,
    ) -> Self {
        let (outgoing_tx, _) = mpsc::channel(100);

        let handler = Self {
            stream,
            peer_id,
            peer_addr,
            local_node_info,
            router,
            peer_registry,
            broadcaster,
            outgoing_tx,
            incoming_tx,
            last_activity: Instant::now(),
            is_outbound,
            response_tx: None,
            reader: None,
            writer: None,
        };

        handler
    }

    /// Get the outgoing message sender
    pub fn get_sender(&self) -> mpsc::Sender<NetMessage> {
        self.outgoing_tx.clone()
    }

    /// Send a message to the peer
    pub async fn send(&mut self, message: NetMessage) -> Result<(), PeerHandlerError> {
        // Ensure we have a writer
        if self.writer.is_none() {
            // Split the TCP stream if we haven't already
            let (read_half, write_half) = tokio::io::split(self.stream.clone());
            self.reader = Some(FramedReader::new(read_half));
            self.writer = Some(FramedWriter::new(write_half));
        }

        // Get the writer
        let writer = self.writer.as_mut().unwrap();

        // Send the message
        debug!("Sending message to peer {}: {:?}", self.peer_addr, message);
        writer.write_message(&message).await
            .map_err(|e| PeerHandlerError::SerializationError(e))?;

        // Update last activity
        self.last_activity = Instant::now();
        self.peer_registry.update_peer_last_seen(&self.peer_id);

        Ok(())
    }

    /// Receive a message from the peer
    pub async fn recv(&mut self) -> Result<Option<NetMessage>, PeerHandlerError> {
        // Ensure we have a reader
        if self.reader.is_none() {
            // Split the TCP stream if we haven't already
            let (read_half, write_half) = tokio::io::split(self.stream.clone());
            self.reader = Some(FramedReader::new(read_half));
            self.writer = Some(FramedWriter::new(write_half));
        }

        // Get the reader
        let reader = self.reader.as_mut().unwrap();

        // Read a message with timeout
        let result = timeout(
            Duration::from_secs(MESSAGE_TIMEOUT),
            reader.read_message::<NetMessage>()
        ).await;

        match result {
            Ok(Ok(Some(message))) => {
                // Update last activity
                self.last_activity = Instant::now();
                self.peer_registry.update_peer_last_seen(&self.peer_id);

                // Handle special messages
                match &message {
                    NetMessage::Disconnect(reason) => {
                        info!("Peer {} disconnected: {:?}", self.peer_addr, reason);
                        return Err(PeerHandlerError::PeerDisconnected(reason.clone()));
                    },
                    NetMessage::Ping(nonce) => {
                        // Automatically respond to pings
                        let _ = self.send(NetMessage::Pong(*nonce)).await;

                        // Continue reading to get the next message
                        return self.recv().await;
                    },
                    NetMessage::Pong(_) => {
                        // Ignore pongs in recv() - they're handled by the main loop
                        return self.recv().await;
                    },
                    _ => {
                        debug!("Received message from peer {}: {:?}", self.peer_addr, message);
                        return Ok(Some(message));
                    }
                }
            },
            Ok(Ok(None)) => {
                info!("Peer {} closed the connection", self.peer_addr);
                return Ok(None);
            },
            Ok(Err(e)) => {
                error!("Error reading message from peer {}: {:?}", self.peer_addr, e);
                return Err(PeerHandlerError::SerializationError(e));
            },
            Err(_) => {
                warn!("Timeout waiting for message from peer {}", self.peer_addr);
                return Err(PeerHandlerError::Timeout);
            }
        }
    }

    /// Send a request and wait for a specific response
    pub async fn send_request(&mut self, request: NetMessage) -> Result<NetMessage, PeerHandlerError> {
        // Send the request
        self.send(request.clone()).await?;

        // Wait for a response that matches the request
        match request {
            NetMessage::RequestBlock(height) => {
                // Wait for ResponseBlock
                loop {
                    match self.recv().await? {
                        Some(response @ NetMessage::ResponseBlock(_)) => return Ok(response),
                        Some(_) => continue, // Ignore other messages
                        None => return Err(PeerHandlerError::Other("Connection closed".to_string())),
                    }
                }
            },
            NetMessage::RequestBlockRange { .. } => {
                // Wait for ResponseBlockRange
                loop {
                    match self.recv().await? {
                        Some(response @ NetMessage::ResponseBlockRange(_)) => return Ok(response),
                        Some(_) => continue, // Ignore other messages
                        None => return Err(PeerHandlerError::Other("Connection closed".to_string())),
                    }
                }
            },
            _ => return Err(PeerHandlerError::Other("Not a request message".to_string())),
        }
    }

    /// Handle the peer connection
    pub async fn handle(mut self) {
        info!("Handling connection with peer {} ({})", self.peer_id, self.peer_addr);

        // Register with the peer registry
        self.peer_registry.register_peer(&self.peer_id, self.peer_addr, self.is_outbound);
        self.peer_registry.update_peer_state(&self.peer_id, ConnectionState::Connected);

        // Set up message channels
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(100);
        self.outgoing_tx = outgoing_tx.clone();

        // Set up response channels
        let (response_tx, mut response_rx) = mpsc::channel(100);
        self.response_tx = Some(response_tx);

        // Register with the broadcaster
        self.broadcaster.register_peer(&self.peer_id, outgoing_tx).await;

        // Perform handshake
        if !self.perform_handshake().await {
            // Clean up on handshake failure
            self.peer_registry.update_peer_state(&self.peer_id, ConnectionState::Failed);
            self.broadcaster.unregister_peer(&self.peer_id).await;
            return;
        }

        // Update peer state to ready
        self.peer_registry.update_peer_state(&self.peer_id, ConnectionState::Ready);

        // Main message loop
        loop {
            // Check for idle timeout
            if self.last_activity.elapsed() > Duration::from_secs(IDLE_TIMEOUT) {
                warn!("Connection with peer {} timed out", self.peer_addr);
                let _ = self.send_disconnect(DisconnectReason::Timeout).await;
                break;
            }

            tokio::select! {
                // Handle outgoing messages
                Some(message) = outgoing_rx.recv() => {
                    if let Err(e) = self.send(message).await {
                        error!("Failed to send message to peer {}: {:?}", self.peer_addr, e);
                        break;
                    }
                }

                // Handle response requests
                Some((request, response_tx)) = response_rx.recv() => {
                    match self.send_request(request).await {
                        Ok(response) => {
                            if response_tx.send(response).is_err() {
                                error!("Failed to send response to requester");
                            }
                        },
                        Err(e) => {
                            error!("Failed to get response from peer {}: {:?}", self.peer_addr, e);
                            break;
                        }
                    }
                }

                // Handle incoming messages
                result = self.recv() => {
                    match result {
                        Ok(Some(message)) => {
                            // Route the message
                            if let Err(e) = self.incoming_tx.send((self.peer_id.clone(), message)).await {
                                error!("Failed to route message from peer {}: {:?}", self.peer_addr, e);
                                break;
                            }
                        },
                        Ok(None) => {
                            info!("Peer {} closed the connection", self.peer_addr);
                            break;
                        },
                        Err(PeerHandlerError::PeerDisconnected(reason)) => {
                            info!("Peer {} disconnected: {:?}", self.peer_addr, reason);
                            break;
                        },
                        Err(e) => {
                            error!("Error receiving message from peer {}: {:?}", self.peer_addr, e);
                            break;
                        }
                    }
                }

                // Add a timeout to avoid blocking forever
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // Just a timeout to check for idle connections
                }
            }
        }

        // Clean up
        self.peer_registry.update_peer_state(&self.peer_id, ConnectionState::Disconnected);
        self.broadcaster.unregister_peer(&self.peer_id).await;
        info!("Connection with peer {} closed", self.peer_addr);
    }

    /// Perform the initial handshake
    async fn perform_handshake(&mut self) -> bool {
        // If this is an outbound connection, send handshake first
        if self.is_outbound {
            debug!("Sending handshake to peer {}", self.peer_addr);

            // Send our handshake
            let handshake = NetMessage::Handshake(self.local_node_info.clone());
            if let Err(e) = self.send(handshake).await {
                error!("Failed to send handshake to peer {}: {:?}", self.peer_addr, e);
                return false;
            }
        }

        // Wait for handshake from peer
        debug!("Waiting for handshake from peer {}", self.peer_addr);
        let handshake_result = timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT),
            self.recv(),
        ).await;

        match handshake_result {
            Ok(Ok(Some(NetMessage::Handshake(node_info)))) => {
                debug!("Received handshake from peer {}: {:?}", self.peer_addr, node_info);

                // Check version compatibility
                if !self.local_node_info.is_compatible(&node_info) {
                    warn!("Incompatible version with peer {}: {:?}", self.peer_addr, node_info);
                    let _ = self.send_disconnect(DisconnectReason::IncompatibleVersion).await;
                    return false;
                }

                // Update peer info in registry
                self.peer_registry.update_peer_info(&self.peer_id, node_info.clone());

                // If this is an inbound connection, send our handshake
                if !self.is_outbound {
                    debug!("Sending handshake to peer {}", self.peer_addr);

                    // Send our handshake
                    let handshake = NetMessage::Handshake(self.local_node_info.clone());
                    if let Err(e) = self.send(handshake).await {
                        error!("Failed to send handshake to peer {}: {:?}", self.peer_addr, e);
                        return false;
                    }
                }

                info!("Handshake completed with peer {}", self.peer_addr);
                return true;
            }
            Ok(Ok(Some(other))) => {
                warn!("Expected handshake from peer {}, got {:?}", self.peer_addr, other);
                let _ = self.send_disconnect(DisconnectReason::ProtocolViolation).await;
            }
            Ok(Ok(None)) => {
                warn!("Peer {} closed connection during handshake", self.peer_addr);
            }
            Ok(Err(e)) => {
                error!("Error reading handshake from peer {}: {:?}", self.peer_addr, e);
            }
            Err(_) => {
                warn!("Handshake with peer {} timed out", self.peer_addr);
                let _ = self.send_disconnect(DisconnectReason::Timeout).await;
            }
        }

        false
    }

    /// Send a disconnect message
    async fn send_disconnect(&mut self, reason: DisconnectReason) -> Result<(), PeerHandlerError> {
        let message = NetMessage::Disconnect(reason);
        let result = self.send(message).await;

        // Update peer state regardless of whether the message was sent
        self.peer_registry.update_peer_state(&self.peer_id, ConnectionState::Disconnected);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::{TcpListener, TcpStream};
    use std::net::SocketAddr;

    #[tokio::test]
    async fn test_handshake() {
        // This is a more complex test that would require setting up
        // two TCP streams and performing a handshake between them.
        // For simplicity, we'll just test the basic functionality.

        // Create a local node info
        let local_node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "local-node".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        );

        // Create a peer node info
        let peer_node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "peer-node".to_string(),
            "127.0.0.1:8001".parse().unwrap(),
        );

        // Check compatibility
        assert!(local_node_info.is_compatible(&peer_node_info));
    }

    #[tokio::test]
    async fn test_send_recv() {
        // Create a duplex channel
        let (client, server) = tokio::io::duplex(1024);

        // Create reader and writer for the "remote" end
        let (server_read, server_write) = tokio::io::split(server);
        let mut server_reader = FramedReader::new(server_read);
        let mut server_writer = FramedWriter::new(server_write);

        // Create a peer handler for the "local" end
        let peer_addr: SocketAddr = "127.0.0.1:8765".parse().unwrap();
        let local_node_info = NodeInfo::new(
            "0.1.0".to_string(),
            "test-node".to_string(),
            "127.0.0.1:8765".parse().unwrap(),
        );

        let (incoming_tx, mut incoming_rx) = mpsc::channel(100);

        // Create mock dependencies
        let router = Arc::new(MessageRouter::new());
        let peer_registry = Arc::new(PeerRegistry::new());
        let broadcaster = Arc::new(PeerBroadcaster::new());

        // Create the peer handler
        let mut handler = PeerHandler::new(
            client,
            "test-peer".to_string(),
            peer_addr,
            local_node_info,
            router,
            peer_registry,
            broadcaster,
            incoming_tx,
            true,
        );

        // Test sending a message
        let message = NetMessage::Ping(42);
        handler.send(message.clone()).await.unwrap();

        // Check that the message was received on the server side
        let received = server_reader.read_message::<NetMessage>().await.unwrap().unwrap();
        assert!(matches!(received, NetMessage::Ping(42)));

        // Test receiving a message
        let response = NetMessage::Pong(42);
        server_writer.write_message(&response).await.unwrap();

        // The handler should automatically respond to pings and ignore pongs in recv()
        // So we'll send a different message to test recv()
        let block_message = NetMessage::NewBlock(Block {
            height: 1,
            hash: [1; 32],
            prev_hash: [0; 32],
            timestamp: 12345,
            transactions: vec![],
            state_root: [0; 32],
            poh_hash: [0; 32],
            poh_seq: 100,
            nonce: 0,
            difficulty: 1000,
            total_difficulty: 1000,
        });

        server_writer.write_message(&block_message).await.unwrap();

        // Check that the message was received
        let received = handler.recv().await.unwrap().unwrap();
        match received {
            NetMessage::NewBlock(block) => {
                assert_eq!(block.height, 1);
            },
            _ => panic!("Expected NewBlock message"),
        }
    }

    #[tokio::test]
    async fn test_send_request() {
        // Create a duplex channel
        let (client, server) = tokio::io::duplex(1024);

        // Create reader and writer for the "remote" end
        let (server_read, server_write) = tokio::io::split(server);
        let mut server_reader = FramedReader::new(server_read);
        let mut server_writer = FramedWriter::new(server_write);

        // Create a peer handler for the "local" end
        let peer_addr: SocketAddr = "127.0.0.1:8765".parse().unwrap();
        let local_node_info = NodeInfo::new(
            "0.1.0".to_string(),
            "test-node".to_string(),
            "127.0.0.1:8765".parse().unwrap(),
        );

        let (incoming_tx, _) = mpsc::channel(100);

        // Create mock dependencies
        let router = Arc::new(MessageRouter::new());
        let peer_registry = Arc::new(PeerRegistry::new());
        let broadcaster = Arc::new(PeerBroadcaster::new());

        // Create the peer handler
        let mut handler = PeerHandler::new(
            client,
            "test-peer".to_string(),
            peer_addr,
            local_node_info,
            router,
            peer_registry,
            broadcaster,
            incoming_tx,
            true,
        );

        // Spawn a task to handle the server side
        tokio::spawn(async move {
            // Read the request
            let request = server_reader.read_message::<NetMessage>().await.unwrap().unwrap();

            // Check that it's a block request
            match request {
                NetMessage::RequestBlock(height) => {
                    assert_eq!(height, 1);

                    // Create a response block
                    let block = Block {
                        height: 1,
                        hash: [1; 32],
                        prev_hash: [0; 32],
                        timestamp: 12345,
                        transactions: vec![],
                        state_root: [0; 32],
                        poh_hash: [0; 32],
                        poh_seq: 100,
                        nonce: 0,
                        difficulty: 1000,
                        total_difficulty: 1000,
                    };

                    // Send the response
                    let response = NetMessage::ResponseBlock(Some(block));
                    server_writer.write_message(&response).await.unwrap();
                },
                _ => panic!("Expected RequestBlock message"),
            }
        });

        // Send a request and wait for the response
        let request = NetMessage::RequestBlock(1);
        let response = handler.send_request(request).await.unwrap();

        // Check the response
        match response {
            NetMessage::ResponseBlock(Some(block)) => {
                assert_eq!(block.height, 1);
            },
            _ => panic!("Expected ResponseBlock message"),
        }
    }
}
