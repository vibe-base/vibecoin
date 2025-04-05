// This file is a placeholder for a more complete network implementation
// For now, we're using the simple network implementation in src/network/simple.rs

use crate::types::error::VibecoinError;
use crate::network::bootstrap::get_bootstrap_addresses;
use std::net::{SocketAddr, TcpListener, TcpStream, IpAddr};
use std::io::{Read, Write};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crate::network::protocol::{Message, parse_message, format_message};
use crate::ledger::state::Blockchain;
use rand::thread_rng;
use rand::Rng;

/// Get all local IP addresses
fn get_local_ips() -> HashSet<IpAddr> {
    let mut ips = HashSet::new();

    // Add localhost
    ips.insert("127.0.0.1".parse().unwrap());
    ips.insert("::1".parse().unwrap());

    // Try to get all network interfaces
    if let Ok(interfaces) = get_if_addrs::get_if_addrs() {
        for interface in interfaces {
            ips.insert(interface.ip());
        }
    }

    ips
}

/// Network server configuration
#[derive(Debug)]
pub struct NetworkConfig {
    /// Address to listen on
    pub listen_addr: SocketAddr,
    /// Seed nodes to connect to
    pub seed_nodes: Vec<SocketAddr>,
    /// User agent string
    pub user_agent: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            listen_addr: "0.0.0.0:8333".parse().unwrap(),
            seed_nodes: Vec::new(),
            user_agent: "VibeCoin/0.1.0".to_string(),
        }
    }
}

/// Network server
pub struct NetworkServer {
    /// Configuration
    config: NetworkConfig,
    /// Connected peers
    peers: Arc<Mutex<HashSet<SocketAddr>>>,
    /// Blockchain reference
    blockchain: Option<Arc<Mutex<Blockchain>>>,
}

impl NetworkServer {
    /// Create a new network server
    pub fn new(config: NetworkConfig) -> Self {
        NetworkServer {
            config,
            peers: Arc::new(Mutex::new(HashSet::new())),
            blockchain: None,
        }
    }

    /// Set the blockchain reference
    pub fn set_blockchain(&mut self, blockchain: Arc<Mutex<Blockchain>>) {
        self.blockchain = Some(blockchain);
    }

    /// Start the network server
    pub fn start(&self) -> Result<(), VibecoinError> {
        println!("[NETWORK] Network server starting...");
        println!("[NETWORK] Listening on {}", self.config.listen_addr);
        println!("[NETWORK] Server config: {:?}", self.config);

        // Get our local IP addresses to prevent self-connections
        let local_ips = get_local_ips();
        println!("[NETWORK] Local IPs: {:?}", local_ips);

        // Start listener
        let listener = TcpListener::bind(self.config.listen_addr)
            .map_err(|e| VibecoinError::IoError(e))?;

        // Set non-blocking mode
        listener.set_nonblocking(true)
            .map_err(|e| VibecoinError::IoError(e))?;

        // Start listener thread
        let listen_addr = self.config.listen_addr;
        let peers = Arc::clone(&self.peers);
        let blockchain = self.blockchain.clone();
        thread::spawn(move || {
            println!("Listener thread started on {}", listen_addr);

            loop {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        println!("Accepted connection from {}", addr);

                        // Add to peers
                        peers.lock().unwrap().insert(addr);

                        // Handle connection in a new thread
                        let peers_clone = Arc::clone(&peers);
                        let blockchain_clone = blockchain.clone();
                        thread::spawn(move || {
                            if let Err(e) = handle_connection(stream, addr, peers_clone, blockchain_clone) {
                                println!("Error handling connection from {}: {}", addr, e);
                            }
                        });
                    },
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // No new connections, sleep for a bit
                        thread::sleep(Duration::from_millis(100));
                    },
                    Err(e) => {
                        println!("Error accepting connection: {}", e);
                        break;
                    }
                }
            }
        });

        // Get local IPs to prevent self-connections
        let local_ips = get_local_ips();

        // Connect to seed nodes if provided
        if !self.config.seed_nodes.is_empty() {
            println!("[NETWORK] Connecting to seed nodes:");
            for seed in &self.config.seed_nodes {
                println!("[NETWORK] Seed node: {}", seed);

                // Check if this is a self-connection
                if local_ips.contains(&seed.ip()) {
                    println!("[NETWORK] Skipping seed node {} (self-connection)", seed);
                    continue;
                }

                // Try a direct connection first to test connectivity
                println!("[NETWORK] Testing direct connection to {}", seed);
                match std::net::TcpStream::connect_timeout(seed, std::time::Duration::from_secs(1)) {
                    Ok(_) => println!("[NETWORK] Direct connection to {} succeeded", seed),
                    Err(e) => println!("[NETWORK] Direct connection to {} failed: {} ({})", seed, e, e.kind()),
                }

                // Connect using our network implementation
                println!("[NETWORK] Initiating connection to seed node {}", seed);
                self.connect_to_peer(*seed);
            }
        } else {
            // Use bootstrap peers if no seed nodes are provided
            let bootstrap_peers = get_bootstrap_addresses();
            if !bootstrap_peers.is_empty() {
                println!("[NETWORK] Connecting to bootstrap peers:");
                for peer in &bootstrap_peers {
                    println!("[NETWORK] Bootstrap peer: {}", peer);

                    // Check if this is a self-connection
                    if local_ips.contains(&peer.ip()) {
                        println!("[NETWORK] Skipping bootstrap peer {} (self-connection)", peer);
                        continue;
                    }

                    // Try a direct connection first to test connectivity
                    println!("[NETWORK] Testing direct connection to {}", peer);
                    match std::net::TcpStream::connect_timeout(peer, std::time::Duration::from_secs(1)) {
                        Ok(_) => println!("[NETWORK] Direct connection to {} succeeded", peer),
                        Err(e) => println!("[NETWORK] Direct connection to {} failed: {} ({})", peer, e, e.kind()),
                    }

                    // Connect using our network implementation
                    println!("[NETWORK] Initiating connection to bootstrap peer {}", peer);
                    self.connect_to_peer(*peer);
                }
            } else {
                println!("[NETWORK] No bootstrap peers available");
            }
        }

        println!("[NETWORK] Network server started");
        Ok(())
    }

    /// Stop the network server
    pub fn stop(&self) {
        println!("[NETWORK] Stopping network server...");

        // In a real implementation, we would gracefully close all connections
        // For now, we just clear the peers list
        let mut peers = self.peers.lock().unwrap();
        let peer_count = peers.len();

        if peer_count > 0 {
            println!("[NETWORK] Disconnecting from {} peers", peer_count);
            for peer in peers.iter() {
                println!("[NETWORK] Disconnecting from {}", peer);
            }
            peers.clear();
            println!("[NETWORK] All peers disconnected");
        } else {
            println!("[NETWORK] No peers to disconnect from");
        }

        println!("[NETWORK] Network server stopped");
    }

    /// Broadcast a block to all peers
    pub fn broadcast_block(&self, block_hash: &[u8; 32]) -> Result<usize, VibecoinError> {
        let peers = self.peers.lock().unwrap();
        let peer_count = peers.len();

        if peer_count == 0 {
            println!("[NETWORK] No peers to broadcast block {}", hex::encode(block_hash));
            return Ok(0);
        }

        println!("[NETWORK] Broadcasting block {} to {} peers", hex::encode(block_hash), peer_count);

        // Create the newblock message
        let newblock_msg = format!("newblock:{}\n", hex::encode(block_hash));

        // Send the message to all peers
        let mut success_count = 0;
        for peer in peers.iter() {
            println!("[NETWORK] Sending block {} to {}", hex::encode(block_hash), peer);

            // Try to connect to the peer
            match TcpStream::connect_timeout(peer, Duration::from_secs(5)) {
                Ok(mut stream) => {
                    // Send the newblock message
                    match stream.write_all(newblock_msg.as_bytes()) {
                        Ok(_) => {
                            println!("[NETWORK] Successfully sent block {} to {}", hex::encode(block_hash), peer);
                            success_count += 1;
                        },
                        Err(e) => {
                            println!("[NETWORK] Error sending block to {}: {}", peer, e);
                        }
                    }
                },
                Err(e) => {
                    println!("[NETWORK] Failed to connect to {} for block broadcast: {}", peer, e);
                }
            }
        }

        println!("[NETWORK] Block {} broadcast complete: {} of {} peers received",
                 hex::encode(block_hash), success_count, peer_count);
        Ok(success_count)
    }

    /// Broadcast a transaction to all peers
    pub fn broadcast_transaction(&self, tx_hash: &[u8; 32]) -> Result<usize, VibecoinError> {
        let peers = self.peers.lock().unwrap();
        let peer_count = peers.len();

        if peer_count == 0 {
            println!("[NETWORK] No peers to broadcast transaction {}", hex::encode(tx_hash));
            return Ok(0);
        }

        println!("[NETWORK] Broadcasting transaction {} to {} peers", hex::encode(tx_hash), peer_count);

        // Create the newtx message
        let newtx_msg = format!("newtx:{}\n", hex::encode(tx_hash));

        // Send the message to all peers
        let mut success_count = 0;
        for peer in peers.iter() {
            println!("[NETWORK] Sending transaction {} to {}", hex::encode(tx_hash), peer);

            // Try to connect to the peer
            match TcpStream::connect_timeout(peer, Duration::from_secs(5)) {
                Ok(mut stream) => {
                    // Send the newtx message
                    match stream.write_all(newtx_msg.as_bytes()) {
                        Ok(_) => {
                            println!("[NETWORK] Successfully sent transaction {} to {}", hex::encode(tx_hash), peer);
                            success_count += 1;
                        },
                        Err(e) => {
                            println!("[NETWORK] Error sending transaction to {}: {}", peer, e);
                        }
                    }
                },
                Err(e) => {
                    println!("[NETWORK] Failed to connect to {} for transaction broadcast: {}", peer, e);
                }
            }
        }

        println!("[NETWORK] Transaction {} broadcast complete: {} of {} peers received",
                 hex::encode(tx_hash), success_count, peer_count);
        Ok(success_count)
    }

    /// Get the number of connected peers
    pub fn get_peer_count(&self) -> usize {
        let count = self.peers.lock().unwrap().len();
        println!("[NETWORK] Current peer count: {}", count);
        count
    }

    /// Get information about all connected peers
    pub fn get_peer_info(&self) -> Vec<String> {
        let peers = self.peers.lock().unwrap();
        println!("[NETWORK] Getting info for {} peers", peers.len());

        let peer_info: Vec<String> = peers.iter().map(|addr| addr.to_string()).collect();

        if !peer_info.is_empty() {
            println!("[NETWORK] Connected peers:");
            for peer in &peer_info {
                println!("[NETWORK]   - {}", peer);
            }
        } else {
            println!("[NETWORK] No connected peers");
        }

        peer_info
    }
}

impl NetworkServer {
    /// Connect to a peer
    pub fn connect_to_peer(&self, addr: SocketAddr) {
        let peers = Arc::clone(&self.peers);
        let blockchain = self.blockchain.clone();

        // Check if we're already connected to this peer
        if peers.lock().unwrap().contains(&addr) {
            println!("[NETWORK] Already connected to {}, skipping", addr);
            return;
        }

        // Check if this is a self-connection
        let local_ips = get_local_ips();
        let listen_port = self.config.listen_addr.port();
        if local_ips.contains(&addr.ip()) && addr.port() == listen_port {
            println!("[NETWORK] Skipping connection to self: {}", addr);
            return;
        }

        println!("[NETWORK] Attempting to connect to peer: {}", addr);

        thread::spawn(move || {
            println!("[NETWORK] Spawned connection thread for {}", addr);
            match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
                Ok(mut stream) => {
                    println!("[NETWORK] Connected to {}", addr);

                    // Set TCP options
                    if let Err(e) = stream.set_nodelay(true) {
                        println!("[NETWORK] Warning: Failed to set TCP_NODELAY for {}: {}", addr, e);
                    }

                    // Add to peers
                    peers.lock().unwrap().insert(addr);
                    println!("[NETWORK] Added {} to peers list", addr);

                    // Send initial version message
                    let version_msg = format!("version:VibeCoin/0.1.0:{}\n", std::process::id());
                    println!("[NETWORK] Sending version message to {}: {}", addr, version_msg.trim());
                    match stream.write_all(version_msg.as_bytes()) {
                        Ok(_) => {
                            println!("[NETWORK] Sent initial version message to {}", addr);

                            // Try to read response
                            let mut response = [0; 1024];
                            match stream.read(&mut response) {
                                Ok(n) => {
                                    let response_str = String::from_utf8_lossy(&response[0..n]);
                                    println!("[NETWORK] Received response from {}: {}", addr, response_str.trim());
                                },
                                Err(e) => {
                                    println!("[NETWORK] Error reading response from {}: {}", addr, e);
                                }
                            }

                            // Handle connection
                            println!("[NETWORK] Starting connection handler for {}", addr);
                            if let Err(e) = handle_connection(stream, addr, peers, blockchain) {
                                println!("[NETWORK] Error handling connection to {}: {}", addr, e);
                            }
                        },
                        Err(e) => {
                            println!("[NETWORK] Error sending initial version to {}: {}", addr, e);
                            println!("[NETWORK] Error kind: {:?}", e.kind());
                            peers.lock().unwrap().remove(&addr);
                            println!("[NETWORK] Removed {} from peers list", addr);
                        }
                    }
                },
                Err(e) => {
                    println!("[NETWORK] Failed to connect to {}: {}", addr, e);
                    println!("[NETWORK] Error kind: {:?}", e.kind());
                    println!("[NETWORK] Error details: {:?}", e);
                }
            }
        });
    }
}

/// Handle a connection
fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    peers: Arc<Mutex<HashSet<SocketAddr>>>,
    blockchain: Option<Arc<Mutex<Blockchain>>>,
) -> Result<(), VibecoinError> {
    println!("[NETWORK] Setting up connection handler for {}", addr);

    // Check if this is a self-connection
    let local_ips = get_local_ips();
    let local_addr = match stream.local_addr() {
        Ok(addr) => addr,
        Err(e) => {
            println!("[NETWORK] Error getting local address: {}", e);
            return Err(VibecoinError::IoError(e));
        }
    };

    if local_ips.contains(&addr.ip()) && local_addr.port() == addr.port() {
        println!("[NETWORK] Detected self-connection from {}, closing", addr);
        return Ok(());
    }

    // Set timeouts
    match stream.set_read_timeout(Some(Duration::from_secs(30))) {
        Ok(_) => println!("[NETWORK] Set read timeout for {}", addr),
        Err(e) => return Err(VibecoinError::IoError(e)),
    }

    match stream.set_write_timeout(Some(Duration::from_secs(30))) {
        Ok(_) => println!("[NETWORK] Set write timeout for {}", addr),
        Err(e) => return Err(VibecoinError::IoError(e)),
    }

    // Simple message loop
    let mut buffer = [0; 1024];
    println!("[NETWORK] Starting message loop for {}", addr);

    // Send initial version message
    let version_msg = format!("version:VibeCoin/0.1.0:{}\n", std::process::id());
    println!("[NETWORK] Sending initial version message to {}: {}", addr, version_msg.trim());

    match stream.write_all(version_msg.as_bytes()) {
        Ok(_) => println!("[NETWORK] Sent initial version message to {}", addr),
        Err(e) => {
            println!("[NETWORK] Error sending initial version to {}: {}", addr, e);
            println!("[NETWORK] Error kind: {:?}", e.kind());
            return Err(VibecoinError::IoError(e));
        }
    }

    // After connecting, check if the peer supports the new protocol
    // For backward compatibility, we'll use a simple version check
    let version_check_msg = "version_check\n";
    println!("[NETWORK] Checking protocol version with {}", addr);

    match stream.write_all(version_check_msg.as_bytes()) {
        Ok(_) => {
            println!("[NETWORK] Sent version check to {}", addr);

            // Read response to see if peer supports new protocol
            let mut response = [0; 1024];
            let supports_new_protocol = match stream.read(&mut response) {
                Ok(n) => {
                    let response_str = String::from_utf8_lossy(&response[0..n]);
                    response_str.contains("version_check_ok")
                },
                Err(_) => false
            };

            // If peer supports new protocol, request blockchain state
            if supports_new_protocol {
                println!("[NETWORK] Peer {} supports new protocol", addr);

                let getstate_msg = "getstate\n";
                println!("[NETWORK] Requesting blockchain state from {}", addr);

                if let Err(e) = stream.write_all(getstate_msg.as_bytes()) {
                    println!("[NETWORK] Error sending getstate to {}: {}", addr, e);
                    println!("[NETWORK] Error kind: {:?}", e.kind());
                }
            } else {
                println!("[NETWORK] Peer {} using legacy protocol", addr);
                // For legacy protocol, we'll just continue with the connection
                // and rely on block announcements
            }
        },
        Err(e) => {
            println!("[NETWORK] Error sending version check to {}: {}", addr, e);
            println!("[NETWORK] Error kind: {:?}", e.kind());
            // Continue anyway, assuming legacy protocol
            println!("[NETWORK] Assuming legacy protocol for {}", addr);
        }
    }

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                // Connection closed
                println!("[NETWORK] Connection closed by {}", addr);
                // Remove from peers
                peers.lock().unwrap().remove(&addr);
                println!("[NETWORK] Removed {} from peers list", addr);
                break;
            },
            Ok(n) => {
                // Process message
                let message_str = String::from_utf8_lossy(&buffer[0..n]);
                println!("[NETWORK] Received from {}: {}", addr, message_str.trim());

                // Handle special case for version_check message
                if message_str.trim() == "version_check" {
                    println!("[NETWORK] Received version check from {}", addr);
                    let response = "version_check_ok\n";
                    if let Err(e) = stream.write_all(response.as_bytes()) {
                        println!("[NETWORK] Error sending version check response to {}: {}", addr, e);
                        break;
                    }
                    continue;
                }

                // Parse the message
                match parse_message(&message_str) {
                    Ok(message) => {
                        match message {
                            Message::Version { version, node_id } => {
                                println!("[NETWORK] Received version message from {}: {} (node ID: {})", addr, version, node_id);

                                // Send our version in response
                                let version_msg = format!("version:VibeCoin/0.1.0:{}\n", std::process::id());
                                if let Err(e) = stream.write_all(version_msg.as_bytes()) {
                                    println!("[NETWORK] Error sending version to {}: {}", addr, e);
                                    break;
                                }
                            },
                            Message::GetState => {
                                println!("[NETWORK] Received getstate request from {}", addr);

                                // Send our blockchain state
                                if let Some(blockchain_ref) = &blockchain {
                                    if let Ok(blockchain) = blockchain_ref.lock() {
                                        let state = blockchain.get_state_summary();
                                        let state_msg = format_message(&Message::State(state.clone()));

                                        println!("[NETWORK] Sending blockchain state to {}: height={}, hash={}",
                                                 addr, state.height, hex::encode(state.latest_hash));

                                        if let Err(e) = stream.write_all(state_msg.as_bytes()) {
                                            println!("[NETWORK] Error sending state to {}: {}", addr, e);
                                            break;
                                        }
                                    }
                                }
                            },
                            Message::State(state) => {
                                println!("[NETWORK] Received blockchain state from {}: height={}, hash={}",
                                         addr, state.height, hex::encode(state.latest_hash));

                                // Compare with our blockchain state
                                if let Some(blockchain_ref) = &blockchain {
                                    if let Ok(blockchain) = blockchain_ref.lock() {
                                        let our_state = blockchain.get_state_summary();

                                        if state.height > our_state.height {
                                            println!("[NETWORK] Peer {} has higher blockchain: {} vs our {}",
                                                     addr, state.height, our_state.height);

                                            // Request missing blocks
                                            let start_height = our_state.height + 1;
                                            let end_height = state.height;

                                            let getblocks_msg = format!("getblocks:{}:{}\n", start_height, end_height);
                                            println!("[NETWORK] Requesting blocks {} to {} from {}", start_height, end_height, addr);

                                            if let Err(e) = stream.write_all(getblocks_msg.as_bytes()) {
                                                println!("[NETWORK] Error sending getblocks to {}: {}", addr, e);
                                                break;
                                            }
                                        } else if state.height < our_state.height {
                                            println!("[NETWORK] Peer {} has lower blockchain: {} vs our {}",
                                                     addr, state.height, our_state.height);
                                        } else {
                                            println!("[NETWORK] Peer {} has same blockchain height: {}", addr, state.height);
                                        }
                                    }
                                }
                            },
                            Message::GetBlocks { start_height, end_height } => {
                                println!("[NETWORK] Received getblocks request from {}: {} to {}", addr, start_height, end_height);

                                // Send requested blocks
                                if let Some(blockchain_ref) = &blockchain {
                                    if let Ok(blockchain) = blockchain_ref.lock() {
                                        // Get the blocks in the requested range
                                        let current_height = blockchain.chain.len() as u64 - 1;
                                        let end = std::cmp::min(end_height, current_height);

                                        if start_height <= end {
                                            for height in start_height..=end {
                                                if height < blockchain.chain.len() as u64 {
                                                    let block = &blockchain.chain[height as usize];

                                                    // Serialize the block
                                                    let block_data = crate::network::protocol::serialize_block(block);

                                                    // Send the block data
                                                    let block_msg = format_message(&Message::BlockData(block_data));
                                                    println!("[NETWORK] Sending block {} to {}", height, addr);

                                                    if let Err(e) = stream.write_all(block_msg.as_bytes()) {
                                                        println!("[NETWORK] Error sending block to {}: {}", addr, e);
                                                        break;
                                                    }
                                                }
                                            }
                                        } else {
                                            println!("[NETWORK] Invalid block range: {} to {}", start_height, end_height);
                                        }
                                    }
                                }
                            },
                            Message::GetBlock(hash) => {
                                println!("[NETWORK] Received getblock request from {}: {}", addr, hex::encode(hash));

                                // Send requested block
                                if let Some(blockchain_ref) = &blockchain {
                                    if let Ok(blockchain) = blockchain_ref.lock() {
                                        // Find the block with the requested hash
                                        let block = blockchain.chain.iter().find(|b| b.hash == hash);

                                        if let Some(block) = block {
                                            // Serialize the block
                                            let block_data = crate::network::protocol::serialize_block(block);

                                            // Send the block data
                                            let block_msg = format_message(&Message::BlockData(block_data));
                                            println!("[NETWORK] Sending block {} to {}", block.index, addr);

                                            if let Err(e) = stream.write_all(block_msg.as_bytes()) {
                                                println!("[NETWORK] Error sending block to {}: {}", addr, e);
                                                break;
                                            }
                                        } else {
                                            println!("[NETWORK] Block {} not found", hex::encode(hash));
                                        }
                                    }
                                }
                            },
                            Message::Block(block) => {
                                println!("[NETWORK] Received block {} from {}", block.index, addr);

                                // Request the full block data if we don't have it
                                if let Some(blockchain_ref) = &blockchain {
                                    if let Ok(blockchain) = blockchain_ref.lock() {
                                        let current_height = blockchain.chain.len() as u64 - 1;

                                        if block.index > current_height {
                                            // We need this block, request the full data
                                            let getblock_msg = format_message(&Message::GetBlock(block.hash));
                                            println!("[NETWORK] Requesting full block {} from {}", block.index, addr);

                                            if let Err(e) = stream.write_all(getblock_msg.as_bytes()) {
                                                println!("[NETWORK] Error requesting block from {}: {}", addr, e);
                                                break;
                                            }
                                        } else {
                                            println!("[NETWORK] Already have block {}", block.index);
                                        }
                                    }
                                }
                            },
                            Message::BlockData(data) => {
                                println!("[NETWORK] Received block data from {}", addr);

                                // Deserialize the block
                                match crate::network::protocol::deserialize_block(&data) {
                                    Ok(block) => {
                                        println!("[NETWORK] Deserialized block {} from {}", block.index, addr);

                                        // Add the block to our chain if we don't have it
                                        if let Some(blockchain_ref) = &blockchain {
                                            if let Ok(mut blockchain) = blockchain_ref.lock() {
                                                let current_height = blockchain.chain.len() as u64 - 1;

                                                if block.index > current_height {
                                                    // Validate and add the block
                                                    match blockchain.add_block(block.clone()) {
                                                        Ok(_) => {
                                                            println!("[NETWORK] Added block {} to chain", block.index);
                                                        },
                                                        Err(e) => {
                                                            println!("[NETWORK] Error adding block {}: {}", block.index, e);
                                                        }
                                                    }
                                                } else {
                                                    println!("[NETWORK] Already have block {}", block.index);
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        println!("[NETWORK] Error deserializing block data: {}", e);
                                    }
                                }
                            },
                            Message::NewBlock(hash) => {
                                println!("[NETWORK] Received newblock announcement from {}: {}", addr, hex::encode(hash));

                                // Request the block if we don't have it
                                // For now, we'll just send a message that we received the announcement
                                // In a real implementation, we would check if we have the block and request it if not
                                let msg = format!("Received newblock announcement for {}\n", hex::encode(hash));
                                if let Err(e) = stream.write_all(msg.as_bytes()) {
                                    println!("[NETWORK] Error sending newblock response to {}: {}", addr, e);
                                    break;
                                }
                            },
                            Message::NewTransaction(hash) => {
                                println!("[NETWORK] Received newtx announcement from {}: {}", addr, hex::encode(hash));

                                // For now, we'll just send a message that we received the announcement
                                // In a real implementation, we would check if we have the transaction and request it if not
                                let msg = format!("Received newtx announcement for {}\n", hex::encode(hash));
                                if let Err(e) = stream.write_all(msg.as_bytes()) {
                                    println!("[NETWORK] Error sending newtx response to {}: {}", addr, e);
                                    break;
                                }
                            },
                            Message::Ping(nonce) => {
                                println!("[NETWORK] Received ping from {}: {}", addr, nonce);

                                // Send pong response
                                let pong_msg = format!("pong:{}\n", nonce);
                                if let Err(e) = stream.write_all(pong_msg.as_bytes()) {
                                    println!("[NETWORK] Error sending pong to {}: {}", addr, e);
                                    break;
                                }
                            },
                            Message::Pong(nonce) => {
                                println!("[NETWORK] Received pong from {}: {}", addr, nonce);
                                // Nothing to do here, just log it
                            },
                            _ => {
                                println!("[NETWORK] Received unknown message type from {}", addr);
                            }
                        }
                    },
                    Err(e) => {
                        println!("[NETWORK] Error parsing message from {}: {}", addr, e);

                        // For backward compatibility, handle legacy protocol messages
                        if message_str.contains("block:") {
                            println!("[NETWORK] Received legacy block message from {}", addr);
                            // Handle legacy block message
                            let response = "Received legacy block message\n";
                            if let Err(e) = stream.write_all(response.as_bytes()) {
                                println!("[NETWORK] Error sending legacy response to {}: {}", addr, e);
                                break;
                            }
                        } else if message_str.contains("tx:") {
                            println!("[NETWORK] Received legacy transaction message from {}", addr);
                            // Handle legacy transaction message
                            let response = "Received legacy transaction message\n";
                            if let Err(e) = stream.write_all(response.as_bytes()) {
                                println!("[NETWORK] Error sending legacy response to {}: {}", addr, e);
                                break;
                            }
                        } else {
                            // Echo back the message for backward compatibility
                            println!("[NETWORK] Echoing unknown message back to {}", addr);
                            if let Err(e) = stream.write_all(&buffer[0..n]) {
                                println!("[NETWORK] Error echoing message to {}: {}", addr, e);
                                break;
                            }
                        }
                    }
                }
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available, sleep for a bit
                thread::sleep(Duration::from_millis(100));

                // Periodically send ping to keep the connection alive
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                if now % 30 == 0 { // Send ping every 30 seconds
                    // Generate a random nonce
                    let nonce: u64 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64;
                    let ping_msg = format!("ping:{}\n", nonce);

                    match stream.write_all(ping_msg.as_bytes()) {
                        Ok(_) => println!("[NETWORK] Sent ping to {}: {}", addr, nonce),
                        Err(e) => {
                            println!("[NETWORK] Error sending ping to {}: {}", addr, e);
                            break;
                        }
                    }
                }
            },
            Err(e) => {
                println!("[NETWORK] Error reading from {}: {}", addr, e);
                println!("[NETWORK] Error kind: {:?}", e.kind());
                // Remove from peers
                peers.lock().unwrap().remove(&addr);
                println!("[NETWORK] Removed {} from peers list", addr);
                break;
            }
        }
    }

    println!("[NETWORK] Connection handler for {} exiting", addr);
    Ok(())
}
