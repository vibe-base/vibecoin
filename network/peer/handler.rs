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
use crate::network::service::router::MessageRouter;

/// Timeout for handshake in seconds
const HANDSHAKE_TIMEOUT: u64 = 10;

/// Timeout for idle connections in seconds
const IDLE_TIMEOUT: u64 = 60;

/// Handler for a single peer connection
pub struct PeerHandler {
    /// TCP stream for the connection
    stream: TcpStream,
    
    /// Peer information
    peer_info: Arc<PeerInfo>,
    
    /// Local node information
    local_node_info: NodeInfo,
    
    /// Message router
    router: Arc<MessageRouter>,
    
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
        peer_info: Arc<PeerInfo>,
        local_node_info: NodeInfo,
        router: Arc<MessageRouter>,
        incoming_tx: mpsc::Sender<(String, NetMessage)>,
    ) -> Self {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(100);
        let is_outbound = peer_info.is_outbound;
        
        let handler = Self {
            stream,
            peer_info,
            local_node_info,
            router,
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
        let peer_addr = self.peer_info.addr;
        info!("Handling connection with peer {}", peer_addr);
        
        // Split the TCP stream
        let (read_half, write_half) = self.stream.split();
        let mut reader = FramedReader::new(read_half);
        let mut writer = FramedWriter::new(write_half);
        
        // Set up message channels
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(100);
        self.outgoing_tx = outgoing_tx;
        
        // Perform handshake
        if !self.perform_handshake(&mut reader, &mut writer).await {
            return;
        }
        
        // Main message loop
        loop {
            // Check for idle timeout
            if self.last_activity.elapsed() > Duration::from_secs(IDLE_TIMEOUT) {
                warn!("Connection with peer {} timed out", peer_addr);
                let _ = self.send_disconnect(DisconnectReason::Timeout, &mut writer).await;
                break;
            }
            
            tokio::select! {
                // Handle outgoing messages
                Some(message) = outgoing_rx.recv() => {
                    if let Err(e) = writer.write_message(&message).await {
                        error!("Failed to send message to peer {}: {:?}", peer_addr, e);
                        break;
                    }
                    self.last_activity = Instant::now();
                }
                
                // Handle incoming messages
                result = reader.read_message::<NetMessage>() => {
                    match result {
                        Ok(Some(message)) => {
                            self.last_activity = Instant::now();
                            
                            // Handle disconnect message
                            if let NetMessage::Disconnect(reason) = &message {
                                info!("Peer {} disconnected: {:?}", peer_addr, reason);
                                break;
                            }
                            
                            // Handle ping message
                            if let NetMessage::Ping(nonce) = &message {
                                let _ = self.outgoing_tx.send(NetMessage::Pong(*nonce)).await;
                                continue;
                            }
                            
                            // Route the message
                            if let Some(node_id) = self.peer_info.node_id() {
                                if let Err(e) = self.incoming_tx.send((node_id.to_string(), message)).await {
                                    error!("Failed to route message from peer {}: {:?}", peer_addr, e);
                                    break;
                                }
                            }
                        }
                        Ok(None) => {
                            info!("Peer {} closed the connection", peer_addr);
                            break;
                        }
                        Err(e) => {
                            error!("Error reading message from peer {}: {:?}", peer_addr, e);
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
        
        // Update peer state to disconnected
        info!("Connection with peer {} closed", peer_addr);
    }
    
    /// Perform the initial handshake
    async fn perform_handshake(
        &mut self,
        reader: &mut FramedReader<tokio::net::tcp::ReadHalf<'_>>,
        writer: &mut FramedWriter<tokio::net::tcp::WriteHalf<'_>>,
    ) -> bool {
        let peer_addr = self.peer_info.addr;
        
        // If this is an outbound connection, send handshake first
        if self.is_outbound {
            debug!("Sending handshake to peer {}", peer_addr);
            
            // Send our handshake
            let handshake = NetMessage::Handshake(self.local_node_info.clone());
            if let Err(e) = writer.write_message(&handshake).await {
                error!("Failed to send handshake to peer {}: {:?}", peer_addr, e);
                return false;
            }
        }
        
        // Wait for handshake from peer
        debug!("Waiting for handshake from peer {}", peer_addr);
        let handshake_result = timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT),
            reader.read_message::<NetMessage>(),
        ).await;
        
        match handshake_result {
            Ok(Ok(Some(NetMessage::Handshake(node_info)))) => {
                debug!("Received handshake from peer {}: {:?}", peer_addr, node_info);
                
                // Check version compatibility
                if !self.local_node_info.is_compatible(&node_info) {
                    warn!("Incompatible version with peer {}: {:?}", peer_addr, node_info);
                    let _ = self.send_disconnect(DisconnectReason::IncompatibleVersion, writer).await;
                    return false;
                }
                
                // Update peer info
                self.peer_info.set_node_info(node_info.clone());
                self.peer_info.update_state(ConnectionState::Ready);
                
                // If this is an inbound connection, send our handshake
                if !self.is_outbound {
                    debug!("Sending handshake to peer {}", peer_addr);
                    
                    // Send our handshake
                    let handshake = NetMessage::Handshake(self.local_node_info.clone());
                    if let Err(e) = writer.write_message(&handshake).await {
                        error!("Failed to send handshake to peer {}: {:?}", peer_addr, e);
                        return false;
                    }
                }
                
                info!("Handshake completed with peer {}", peer_addr);
                return true;
            }
            Ok(Ok(Some(other))) => {
                warn!("Expected handshake from peer {}, got {:?}", peer_addr, other);
                let _ = self.send_disconnect(DisconnectReason::ProtocolViolation, writer).await;
            }
            Ok(Ok(None)) => {
                warn!("Peer {} closed connection during handshake", peer_addr);
            }
            Ok(Err(e)) => {
                error!("Error reading handshake from peer {}: {:?}", peer_addr, e);
            }
            Err(_) => {
                warn!("Handshake with peer {} timed out", peer_addr);
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
        writer.write_message(&message).await
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
