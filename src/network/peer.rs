use crate::types::error::VibecoinError;
use std::net::{SocketAddr, TcpStream};
use std::io::{Read, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

/// Status of a peer connection
#[derive(Debug, Clone, PartialEq)]
pub enum PeerStatus {
    /// Peer is connected and active
    Connected,
    /// Peer is disconnected
    Disconnected,
    /// Peer is banned
    Banned,
    /// Peer is pending connection
    Pending,
}

/// Information about a peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Address of the peer
    pub address: SocketAddr,
    /// Status of the peer
    pub status: PeerStatus,
    /// Last time we received a message from this peer
    pub last_seen: u64,
    /// Protocol version
    pub version: u32,
    /// User agent string
    pub user_agent: String,
    /// Current height of the peer's blockchain
    pub height: u64,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(address: SocketAddr) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        PeerInfo {
            address,
            status: PeerStatus::Pending,
            last_seen: now,
            version: 1,
            user_agent: "VibeCoin/0.1.0".to_string(),
            height: 0,
        }
    }

    /// Update the last seen time
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Check if the peer is active
    pub fn is_active(&self) -> bool {
        self.status == PeerStatus::Connected
    }

    /// Check if the peer is stale (no activity for a while)
    pub fn is_stale(&self, timeout_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now - self.last_seen > timeout_secs
    }
}

/// A connection to a peer
pub struct PeerConnection {
    /// Information about the peer
    pub info: PeerInfo,
    /// TCP stream for communication
    stream: TcpStream,
}

impl PeerConnection {
    /// Create a new peer connection
    pub fn new(address: SocketAddr) -> Result<Self, VibecoinError> {
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))
            .map_err(|e| VibecoinError::IoError(e))?;

        // Set timeouts
        stream.set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| VibecoinError::IoError(e))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| VibecoinError::IoError(e))?;

        let info = PeerInfo::new(address);

        Ok(PeerConnection {
            info,
            stream,
        })
    }

    /// Send data to the peer
    pub fn send(&mut self, data: &[u8]) -> Result<(), VibecoinError> {
        // First send the length as a 4-byte prefix
        let len = data.len() as u32;
        let len_bytes = len.to_be_bytes();

        self.stream.write_all(&len_bytes)
            .map_err(|e| VibecoinError::IoError(e))?;

        // Then send the actual data
        self.stream.write_all(data)
            .map_err(|e| VibecoinError::IoError(e))?;

        self.stream.flush()
            .map_err(|e| VibecoinError::IoError(e))?;

        self.info.update_last_seen();

        Ok(())
    }

    /// Receive data from the peer
    pub fn receive(&mut self) -> Result<Vec<u8>, VibecoinError> {
        // First read the length prefix
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes)
            .map_err(|e| VibecoinError::IoError(e))?;

        let len = u32::from_be_bytes(len_bytes) as usize;

        // Sanity check to prevent memory exhaustion attacks
        if len > 1024 * 1024 * 10 { // 10 MB max message size
            return Err(VibecoinError::NetworkError("Message too large".to_string()));
        }

        // Then read the actual data
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data)
            .map_err(|e| VibecoinError::IoError(e))?;

        self.info.update_last_seen();

        Ok(data)
    }

    /// Close the connection
    pub fn close(&mut self) -> Result<(), VibecoinError> {
        self.info.status = PeerStatus::Disconnected;
        self.stream.shutdown(std::net::Shutdown::Both)
            .map_err(|e| VibecoinError::IoError(e))?;

        Ok(())
    }
}

/// Manager for peer connections
pub struct PeerManager {
    /// Known peers
    peers: Arc<Mutex<HashMap<SocketAddr, PeerInfo>>>,
    /// Active connections
    connections: Arc<Mutex<HashMap<SocketAddr, Arc<Mutex<PeerConnection>>>>>,
    /// Maximum number of outbound connections
    max_outbound: usize,
    /// Timeout for stale peers (in seconds)
    stale_timeout: u64,
    /// Whether the manager is running
    running: Arc<Mutex<bool>>,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(max_outbound: usize, stale_timeout: u64) -> Self {
        PeerManager {
            peers: Arc::new(Mutex::new(HashMap::new())),
            connections: Arc::new(Mutex::new(HashMap::new())),
            max_outbound,
            stale_timeout,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Add a peer to the known peers list
    pub fn add_peer(&self, address: SocketAddr) {
        let mut peers = self.peers.lock().unwrap();
        if !peers.contains_key(&address) {
            peers.insert(address, PeerInfo::new(address));
        }
    }

    /// Remove a peer from the known peers list
    pub fn remove_peer(&self, address: &SocketAddr) {
        let mut peers = self.peers.lock().unwrap();
        peers.remove(address);

        // Also remove any active connection
        let mut connections = self.connections.lock().unwrap();
        if let Some(conn) = connections.remove(address) {
            let mut conn = conn.lock().unwrap();
            let _ = conn.close(); // Ignore errors
        }
    }

    /// Ban a peer
    pub fn ban_peer(&self, address: &SocketAddr) {
        let mut peers = self.peers.lock().unwrap();
        if let Some(peer) = peers.get_mut(address) {
            peer.status = PeerStatus::Banned;
        }

        // Also close any active connection
        let mut connections = self.connections.lock().unwrap();
        if let Some(conn) = connections.remove(address) {
            let mut conn = conn.lock().unwrap();
            let _ = conn.close(); // Ignore errors
        }
    }

    /// Connect to a peer
    pub fn connect(&self, address: SocketAddr) -> Result<(), VibecoinError> {
        // Check if we're already connected
        let connections = self.connections.lock().unwrap();
        if connections.contains_key(&address) {
            return Ok(());
        }

        // Check if we've reached the maximum number of outbound connections
        if connections.len() >= self.max_outbound {
            return Err(VibecoinError::NetworkError("Too many connections".to_string()));
        }
        drop(connections);

        // Check if the peer is banned
        let peers = self.peers.lock().unwrap();
        if let Some(peer) = peers.get(&address) {
            if peer.status == PeerStatus::Banned {
                return Err(VibecoinError::NetworkError("Peer is banned".to_string()));
            }
        }
        drop(peers);

        // Connect to the peer
        let connection = PeerConnection::new(address)?;

        // Update peer info
        let mut peers = self.peers.lock().unwrap();
        if let Some(peer) = peers.get_mut(&address) {
            peer.status = PeerStatus::Connected;
            peer.update_last_seen();
        } else {
            let mut info = PeerInfo::new(address);
            info.status = PeerStatus::Connected;
            peers.insert(address, info);
        }
        drop(peers);

        // Add to active connections
        let mut connections = self.connections.lock().unwrap();
        connections.insert(address, Arc::new(Mutex::new(connection)));

        Ok(())
    }

    /// Disconnect from a peer
    pub fn disconnect(&self, address: &SocketAddr) -> Result<(), VibecoinError> {
        let mut connections = self.connections.lock().unwrap();
        if let Some(conn) = connections.remove(address) {
            let mut conn = conn.lock().unwrap();
            conn.close()?;

            // Update peer info
            let mut peers = self.peers.lock().unwrap();
            if let Some(peer) = peers.get_mut(address) {
                peer.status = PeerStatus::Disconnected;
            }
        }

        Ok(())
    }

    /// Get a list of all known peers
    pub fn get_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.lock().unwrap();
        peers.values().cloned().collect()
    }

    /// Get a list of all active connections
    pub fn get_active_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.lock().unwrap();
        peers.values()
            .filter(|p| p.status == PeerStatus::Connected)
            .cloned()
            .collect()
    }

    /// Get all connections
    pub fn get_connections(&self) -> HashMap<SocketAddr, Arc<Mutex<PeerConnection>>> {
        let connections = self.connections.lock().unwrap();
        connections.clone()
    }

    /// Update peer info
    pub fn update_peer_info<F>(&self, address: &SocketAddr, update_fn: F)
    where
        F: FnOnce(&mut PeerInfo),
    {
        let mut peers = self.peers.lock().unwrap();
        if let Some(peer) = peers.get_mut(address) {
            update_fn(peer);
        }
    }

    /// Add a connection
    pub fn add_connection(&self, address: SocketAddr, connection: PeerConnection) {
        let mut connections = self.connections.lock().unwrap();
        connections.insert(address, Arc::new(Mutex::new(connection)));
    }

    /// Send data to all connected peers
    pub fn broadcast(&self, data: &[u8]) -> Result<usize, VibecoinError> {
        let connections = self.connections.lock().unwrap();
        let mut success_count = 0;

        for conn in connections.values() {
            let mut conn = conn.lock().unwrap();
            if let Ok(()) = conn.send(data) {
                success_count += 1;
            }
        }

        Ok(success_count)
    }

    /// Send data to a specific peer
    pub fn send_to_peer(&self, address: &SocketAddr, data: &[u8]) -> Result<(), VibecoinError> {
        let connections = self.connections.lock().unwrap();
        if let Some(conn) = connections.get(address) {
            let mut conn = conn.lock().unwrap();
            conn.send(data)?;
            Ok(())
        } else {
            Err(VibecoinError::NetworkError("Peer not connected".to_string()))
        }
    }

    /// Start the peer manager
    pub fn start(&self) {
        let mut running = self.running.lock().unwrap();
        if *running {
            return;
        }
        *running = true;
        drop(running);

        // Clone the necessary Arc references for the background thread
        let peers = Arc::clone(&self.peers);
        let connections = Arc::clone(&self.connections);
        let running = Arc::clone(&self.running);
        let stale_timeout = self.stale_timeout;

        // Start a background thread to manage connections
        thread::spawn(move || {
            while *running.lock().unwrap() {
                // Clean up stale connections
                let mut to_disconnect = Vec::new();

                {
                    let peers_lock = peers.lock().unwrap();
                    for (addr, peer) in peers_lock.iter() {
                        if peer.status == PeerStatus::Connected && peer.is_stale(stale_timeout) {
                            to_disconnect.push(*addr);
                        }
                    }
                }

                // Disconnect stale peers
                let connections_lock = connections.lock().unwrap();
                for addr in to_disconnect {
                    if let Some(conn) = connections_lock.get(&addr) {
                        let mut conn = conn.lock().unwrap();
                        let _ = conn.close(); // Ignore errors
                    }

                    // Update peer status
                    let mut peers_lock = peers.lock().unwrap();
                    if let Some(peer) = peers_lock.get_mut(&addr) {
                        peer.status = PeerStatus::Disconnected;
                    }
                }

                // Sleep for a bit
                thread::sleep(Duration::from_secs(30));
            }
        });
    }

    /// Stop the peer manager
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
        drop(running);

        // Disconnect all peers
        let connections = self.connections.lock().unwrap();
        for conn in connections.values() {
            let mut conn = conn.lock().unwrap();
            let _ = conn.close(); // Ignore errors
        }
    }

    /// Broadcast a new block to all peers
    pub fn broadcast_block(&self, block_hash: &[u8; 32]) -> Result<usize, VibecoinError> {
        // Create a simple message with the block hash
        let block_hash_hex = hex::encode(block_hash);

        // Broadcast the message
        let mut success_count = 0;
        let connections = self.connections.lock().unwrap();

        for conn in connections.values() {
            let mut conn = conn.lock().unwrap();
            if let Ok(()) = conn.send(format!("BLOCK: {}", block_hash_hex).as_bytes()) {
                success_count += 1;
            }
        }

        Ok(success_count)
    }

    /// Broadcast a new transaction to all peers
    pub fn broadcast_transaction(&self, tx_hash: &[u8; 32]) -> Result<usize, VibecoinError> {
        // Create a simple message with the transaction hash
        let tx_hash_hex = hex::encode(tx_hash);

        // Broadcast the message
        let mut success_count = 0;
        let connections = self.connections.lock().unwrap();

        for conn in connections.values() {
            let mut conn = conn.lock().unwrap();
            if let Ok(()) = conn.send(format!("TX: {}", tx_hash_hex).as_bytes()) {
                success_count += 1;
            }
        }

        Ok(success_count)
    }
}

/// Simple message type for basic network communication
pub enum SimpleMessageType {
    /// Ping message
    Ping,
    /// Pong message
    Pong,
    /// Block announcement
    Block,
    /// Transaction announcement
    Transaction,
    /// Get peers request
    GetPeers,
    /// Peers response
    Peers,
}

/// Simple network message
pub struct SimpleMessage {
    /// Type of message
    pub message_type: SimpleMessageType,
    /// Content of the message
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

    /// Convert to a string
    pub fn to_string(&self) -> String {
        let type_str = match self.message_type {
            SimpleMessageType::Ping => "PING",
            SimpleMessageType::Pong => "PONG",
            SimpleMessageType::Block => "BLOCK",
            SimpleMessageType::Transaction => "TX",
            SimpleMessageType::GetPeers => "GETPEERS",
            SimpleMessageType::Peers => "PEERS",
        };

        format!("{}: {}", type_str, self.content)
    }

    /// Parse from a string
    pub fn from_string(s: &str) -> Result<Self, VibecoinError> {
        let parts: Vec<&str> = s.splitn(2, ":").collect();

        if parts.len() != 2 {
            return Err(VibecoinError::NetworkError("Invalid message format".to_string()));
        }

        let message_type = match parts[0] {
            "PING" => SimpleMessageType::Ping,
            "PONG" => SimpleMessageType::Pong,
            "BLOCK" => SimpleMessageType::Block,
            "TX" => SimpleMessageType::Transaction,
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
