use crate::types::error::VibecoinError;
use crate::network::bootstrap::get_bootstrap_addresses;
use std::net::{TcpListener, TcpStream, SocketAddr};
use std::io::{Read, Write};
use std::thread;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;

/// Simple network configuration
pub struct SimpleNetworkConfig {
    /// Address to listen on
    pub listen_addr: SocketAddr,
    /// Seed nodes to connect to
    pub seed_nodes: Vec<SocketAddr>,
}

impl Default for SimpleNetworkConfig {
    fn default() -> Self {
        SimpleNetworkConfig {
            listen_addr: "127.0.0.1:8333".parse().unwrap(),
            seed_nodes: Vec::new(),
        }
    }
}

/// Simple network service
pub struct SimpleNetwork {
    /// Configuration
    config: SimpleNetworkConfig,
    /// Connected peers
    peers: Arc<Mutex<HashSet<SocketAddr>>>,
    /// Running flag
    running: Arc<Mutex<bool>>,
}

impl SimpleNetwork {
    /// Create a new simple network
    pub fn new(config: SimpleNetworkConfig) -> Self {
        SimpleNetwork {
            config,
            peers: Arc::new(Mutex::new(HashSet::new())),
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Start the network service
    pub fn start(&self) -> Result<(), VibecoinError> {
        let mut running = self.running.lock().unwrap();
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        // Connect to seed nodes if provided
        if !self.config.seed_nodes.is_empty() {
            println!("Connecting to seed nodes:");
            for seed in &self.config.seed_nodes {
                println!("  - {}", seed);
                self.connect_to_peer(*seed);
            }
        } else {
            // Use bootstrap peers if no seed nodes are provided
            let bootstrap_peers = get_bootstrap_addresses();
            if !bootstrap_peers.is_empty() {
                println!("Connecting to bootstrap peers:");
                for peer in &bootstrap_peers {
                    println!("  - {}", peer);
                    self.connect_to_peer(*peer);
                }
            }
        }

        // Start listener
        let peers = Arc::clone(&self.peers);
        let running = Arc::clone(&self.running);
        let listen_addr = self.config.listen_addr;

        thread::spawn(move || {
            if let Ok(listener) = TcpListener::bind(listen_addr) {
                println!("Listening for connections on {}", listen_addr);

                // Set non-blocking mode
                if let Err(e) = listener.set_nonblocking(true) {
                    println!("Error setting non-blocking mode: {}", e);
                    return;
                }

                while *running.lock().unwrap() {
                    match listener.accept() {
                        Ok((stream, addr)) => {
                            println!("New connection from {}", addr);

                            // Add to peers
                            peers.lock().unwrap().insert(addr);

                            // Handle connection in a new thread
                            let peers_clone = Arc::clone(&peers);
                            thread::spawn(move || {
                                if let Err(e) = handle_connection(stream, addr, peers_clone) {
                                    println!("Error handling connection: {}", e);
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
            } else {
                println!("Failed to bind to {}", listen_addr);
            }
        });

        Ok(())
    }

    /// Stop the network service
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    /// Connect to a peer
    pub fn connect_to_peer(&self, addr: SocketAddr) {
        let peers = Arc::clone(&self.peers);

        thread::spawn(move || {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
                Ok(stream) => {
                    println!("Connected to {}", addr);

                    // Add to peers
                    peers.lock().unwrap().insert(addr);

                    // Handle connection
                    if let Err(e) = handle_connection(stream, addr, peers) {
                        println!("Error handling connection to {}: {}", addr, e);
                    }
                },
                Err(e) => {
                    println!("Failed to connect to {}: {}", addr, e);
                }
            }
        });
    }

    /// Broadcast a message to all peers
    pub fn broadcast_message(&self, message: &str) {
        let peers = self.peers.lock().unwrap();
        println!("Broadcasting message '{}' to {} peers", message, peers.len());
        // In a real implementation, we would send the message to all peers
    }

    /// Get the number of connected peers
    pub fn peer_count(&self) -> usize {
        self.peers.lock().unwrap().len()
    }

    /// Get a list of connected peers
    pub fn get_peers(&self) -> Vec<SocketAddr> {
        let peers = self.peers.lock().unwrap();
        peers.iter().cloned().collect()
    }
}

/// Handle a connection
fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    peers: Arc<Mutex<HashSet<SocketAddr>>>,
) -> Result<(), VibecoinError> {
    // Set timeouts
    stream.set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| VibecoinError::IoError(e))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| VibecoinError::IoError(e))?;

    // Simple message loop
    let mut buffer = [0; 1024];

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => {
                // Connection closed
                println!("Connection closed by {}", addr);
                break;
            },
            Ok(n) => {
                // Process message
                let message = String::from_utf8_lossy(&buffer[0..n]);
                println!("Received from {}: {}", addr, message);

                // Echo back
                if let Err(e) = stream.write_all(&buffer[0..n]) {
                    println!("Error writing to {}: {}", addr, e);
                    break;
                }
            },
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available, sleep for a bit
                thread::sleep(Duration::from_millis(100));
            },
            Err(e) => {
                println!("Error reading from {}: {}", addr, e);
                break;
            }
        }
    }

    // Remove from peers
    peers.lock().unwrap().remove(&addr);

    Ok(())
}
