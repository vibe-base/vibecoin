use std::sync::Arc;
use tokio::sync::mpsc;
use vibecoin::network::types::message::NetMessage;
use vibecoin::network::peer::broadcaster::PeerBroadcaster;
use vibecoin::network::peer::registry::PeerRegistry;

#[tokio::main]
async fn main() {
    // Create a peer registry
    let peer_registry = Arc::new(PeerRegistry::new());
    
    // Register the peer
    let peer_id = "peer-108.61.193.45:30334";
    let peer_addr = "108.61.193.45:30334".parse().unwrap();
    peer_registry.register_peer(peer_id, peer_addr, true);
    
    // Create a broadcaster
    let broadcaster = Arc::new(PeerBroadcaster::new());
    
    // Send a request for the latest block
    match broadcaster.send_to_peer(
        peer_id,
        NetMessage::RequestBlock(u64::MAX), // Special value to request the latest block
    ).await {
        Ok(true) => println!("Successfully sent request for latest block to peer {}", peer_id),
        Ok(false) => println!("Failed to send request to peer {}", peer_id),
        Err(e) => println!("Error sending request to peer {}: {}", peer_id, e),
    }
}
