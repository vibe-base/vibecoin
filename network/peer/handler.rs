use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
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
        };

        handler
    }

    /// Get the outgoing message sender
    pub fn get_sender(&self) -> mpsc::Sender<NetMessage> {
        self.outgoing_tx.clone()
    }

    /// Handle the peer connection
    pub async fn handle(mut self) {
        info!("Handling connection with peer {} ({})", self.peer_id, self.peer_addr);

        // Register with the peer registry
        self.peer_registry.register_peer(&self.peer_id, self.peer_addr, self.is_outbound);
        self.peer_registry.update_peer_state(&self.peer_id, ConnectionState::Connected);

        // Split the TCP stream
        let (read_half, write_half) = self.stream.split();
        let mut reader = FramedReader::new(read_half);
        let mut writer = FramedWriter::new(write_half);

        // Set up message channels
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(100);
        self.outgoing_tx = outgoing_tx.clone();

        // Register with the broadcaster
        self.broadcaster.register_peer(&self.peer_id, outgoing_tx).await;

        // Perform handshake
        if !self.perform_handshake(&mut reader, &mut writer).await {
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
                let _ = self.send_disconnect(DisconnectReason::Timeout, &mut writer).await;
                break;
            }

            tokio::select! {
                // Handle outgoing messages
                Some(message) = outgoing_rx.recv() => {
                    if let Err(e) = writer.write_message(&message).await {
                        error!("Failed to send message to peer {}: {:?}", self.peer_addr, e);
                        break;
                    }
                    self.last_activity = Instant::now();
                    self.peer_registry.update_peer_last_seen(&self.peer_id);
                }

                // Handle incoming messages
                result = reader.read_message::<NetMessage>() => {
                    match result {
                        Ok(Some(message)) => {
                            self.last_activity = Instant::now();
                            self.peer_registry.update_peer_last_seen(&self.peer_id);

                            // Handle disconnect message
                            if let NetMessage::Disconnect(reason) = &message {
                                info!("Peer {} disconnected: {:?}", self.peer_addr, reason);
                                break;
                            }

                            // Handle ping message
                            if let NetMessage::Ping(nonce) = &message {
                                let start_time = self.last_activity;
                                let _ = self.outgoing_tx.send(NetMessage::Pong(*nonce)).await;

                                // Calculate ping latency
                                let latency = start_time.elapsed().as_millis() as u64;
                                self.peer_registry.update_peer_ping_latency(&self.peer_id, latency);
                                continue;
                            }

                            // Handle pong message
                            if let NetMessage::Pong(nonce) = &message {
                                // Calculate ping latency (if we were tracking the ping)
                                // For now, just log it
                                debug!("Received pong from peer {}: nonce={}", self.peer_addr, nonce);
                                continue;
                            }

                            // Route the message
                            if let Err(e) = self.incoming_tx.send((self.peer_id.clone(), message)).await {
                                error!("Failed to route message from peer {}: {:?}", self.peer_addr, e);
                                break;
                            }
                        }
                        Ok(None) => {
                            info!("Peer {} closed the connection", self.peer_addr);
                            break;
                        }
                        Err(e) => {
                            error!("Error reading message from peer {}: {:?}", self.peer_addr, e);
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
    async fn perform_handshake(
        &mut self,
        reader: &mut FramedReader<tokio::net::tcp::ReadHalf<'_>>,
        writer: &mut FramedWriter<tokio::net::tcp::WriteHalf<'_>>,
    ) -> bool {
        // If this is an outbound connection, send handshake first
        if self.is_outbound {
            debug!("Sending handshake to peer {}", self.peer_addr);

            // Send our handshake
            let handshake = NetMessage::Handshake(self.local_node_info.clone());
            if let Err(e) = writer.write_message(&handshake).await {
                error!("Failed to send handshake to peer {}: {:?}", self.peer_addr, e);
                return false;
            }
        }

        // Wait for handshake from peer
        debug!("Waiting for handshake from peer {}", self.peer_addr);
        let handshake_result = timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT),
            reader.read_message::<NetMessage>(),
        ).await;

        match handshake_result {
            Ok(Ok(Some(NetMessage::Handshake(node_info)))) => {
                debug!("Received handshake from peer {}: {:?}", self.peer_addr, node_info);

                // Check version compatibility
                if !self.local_node_info.is_compatible(&node_info) {
                    warn!("Incompatible version with peer {}: {:?}", self.peer_addr, node_info);
                    let _ = self.send_disconnect(DisconnectReason::IncompatibleVersion, writer).await;
                    return false;
                }

                // Update peer info in registry
                self.peer_registry.update_peer_info(&self.peer_id, node_info.clone());

                // If this is an inbound connection, send our handshake
                if !self.is_outbound {
                    debug!("Sending handshake to peer {}", self.peer_addr);

                    // Send our handshake
                    let handshake = NetMessage::Handshake(self.local_node_info.clone());
                    if let Err(e) = writer.write_message(&handshake).await {
                        error!("Failed to send handshake to peer {}: {:?}", self.peer_addr, e);
                        return false;
                    }
                }

                info!("Handshake completed with peer {}", self.peer_addr);
                return true;
            }
            Ok(Ok(Some(other))) => {
                warn!("Expected handshake from peer {}, got {:?}", self.peer_addr, other);
                let _ = self.send_disconnect(DisconnectReason::ProtocolViolation, writer).await;
            }
            Ok(Ok(None)) => {
                warn!("Peer {} closed connection during handshake", self.peer_addr);
            }
            Ok(Err(e)) => {
                error!("Error reading handshake from peer {}: {:?}", self.peer_addr, e);
            }
            Err(_) => {
                warn!("Handshake with peer {} timed out", self.peer_addr);
                let _ = self.send_disconnect(DisconnectReason::Timeout, writer).await;
            }
        }

        false
    }

    /// Send a disconnect message
    async fn send_disconnect(
        &self,
        reason: DisconnectReason,
        writer: &mut FramedWriter<tokio::net::tcp::WriteHalf<'_>>,
    ) -> Result<(), bincode::Error> {
        let message = NetMessage::Disconnect(reason);
        let result = writer.write_message(&message).await;

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
}
