use vibecoin::network::{NetworkConfig, start_network};
use vibecoin::network::types::message::NetMessage;
use vibecoin::network::types::node_info::NodeInfo;
use vibecoin::storage::block_store::Block;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[tokio::test]
async fn test_network_connection() {
    // Create two network configs
    let node1_addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    let node2_addr: SocketAddr = "127.0.0.1:9002".parse().unwrap();
    
    let config1 = NetworkConfig {
        bind_addr: node1_addr,
        seed_peers: vec![node2_addr],
        max_outbound: 8,
        max_inbound: 32,
        node_id: "node1".to_string(),
    };
    
    let config2 = NetworkConfig {
        bind_addr: node2_addr,
        seed_peers: vec![node1_addr],
        max_outbound: 8,
        max_inbound: 32,
        node_id: "node2".to_string(),
    };
    
    // Create message channels
    let (tx1, rx1) = mpsc::channel(100);
    let (tx2, rx2) = mpsc::channel(100);
    
    // Start the networks
    let service1 = start_network(config1, rx1).await;
    let service2 = start_network(config2, rx2).await;
    
    // Wait for the networks to connect
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Check that the networks are connected
    let connected1 = service1.peer_manager().connected_peer_count().await;
    let connected2 = service2.peer_manager().connected_peer_count().await;
    
    assert!(connected1 > 0 || connected2 > 0, "Networks should be connected");
}

#[tokio::test]
async fn test_message_broadcast() {
    // Create two network configs
    let node1_addr: SocketAddr = "127.0.0.1:9003".parse().unwrap();
    let node2_addr: SocketAddr = "127.0.0.1:9004".parse().unwrap();
    
    let config1 = NetworkConfig {
        bind_addr: node1_addr,
        seed_peers: vec![node2_addr],
        max_outbound: 8,
        max_inbound: 32,
        node_id: "node1".to_string(),
    };
    
    let config2 = NetworkConfig {
        bind_addr: node2_addr,
        seed_peers: vec![node1_addr],
        max_outbound: 8,
        max_inbound: 32,
        node_id: "node2".to_string(),
    };
    
    // Create message channels
    let (tx1, rx1) = mpsc::channel(100);
    let (tx2, rx2) = mpsc::channel(100);
    
    // Start the networks
    let service1 = start_network(config1, rx1).await;
    let service2 = start_network(config2, rx2).await;
    
    // Wait for the networks to connect
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Create a channel for new block messages on node2
    let mut new_block_rx = service2.router().create_channel("new_block").await;
    
    // Create a test block
    let block = Block {
        height: 1,
        hash: [1; 32],
        prev_hash: [0; 32],
        timestamp: 12345,
        transactions: vec![[2; 32], [3; 32]],
        state_root: [4; 32],
    };
    
    // Broadcast the block from node1
    let message = NetMessage::NewBlock(block.clone());
    service1.peer_manager().broadcast(message).await;
    
    // Wait for node2 to receive the block
    let result = timeout(Duration::from_secs(2), new_block_rx.recv()).await;
    
    assert!(result.is_ok(), "Should receive the block");
    
    if let Ok(Some((node_id, NetMessage::NewBlock(received_block)))) = result {
        assert_eq!(node_id, "node1");
        assert_eq!(received_block.height, block.height);
        assert_eq!(received_block.hash, block.hash);
    } else {
        panic!("Expected NewBlock message");
    }
}
