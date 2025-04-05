use vibecoin::node::runner::{Node, NodeConfig};

use std::{thread, time::Duration};
use std::net::SocketAddr;

fn main() {
    println!("Starting VibeCoin Network Demo...\n");

    // Create two nodes with networking enabled
    let node1 = create_node(8333, vec![], true);
    let node2 = create_node(8334, vec!["127.0.0.1:8333".parse().unwrap()], false);

    // Start both nodes
    println!("Starting nodes...");
    let mut node1 = node1.expect("Failed to create node 1");
    let mut node2 = node2.expect("Failed to create node 2");

    thread::spawn(move || {
        if let Err(e) = node1.start() {
            eprintln!("Node 1 error: {}", e);
        }
    });

    thread::spawn(move || {
        if let Err(e) = node2.start() {
            eprintln!("Node 2 error: {}", e);
        }
    });

    // Wait for nodes to connect
    println!("Waiting for nodes to connect...");
    thread::sleep(Duration::from_secs(5));

    println!("Network demo complete!");
    println!("In a real implementation, the nodes would now sync their blockchains");
    println!("and propagate transactions and blocks between them.");

    // In a real implementation, we would:
    // 1. Create a transaction on node 1
    // 2. Wait for it to propagate to node 2
    // 3. Mine a block on node 2
    // 4. Wait for it to propagate to node 1
    // 5. Verify that both nodes have the same blockchain state
}

fn create_node(port: u16, seed_nodes: Vec<SocketAddr>, mining_enabled: bool) -> Result<Node, String> {
    println!("Creating node on port {}...", port);

    // Create node configuration
    let config = NodeConfig {
        difficulty: 2,                // Very low difficulty for demo
        mining_enabled,
        target_block_time: 10,        // Mine a block every 10 seconds
        mine_empty_blocks: true,      // Mine even if there are no transactions
        max_tx_age: 3600,             // 1 hour max age for transactions
        poh_ticks_per_second: 1000,
        wallet_path: format!("config/node_{}_wallet.json", port),
        wallet_password: "demo_password".to_string(),
        enable_networking: true,
        p2p_listen_addr: Some(format!("127.0.0.1:{}", port).parse().unwrap()),
        seed_nodes,
        max_inbound: 125,
        max_outbound: 8,
    };

    // Create the node
    Node::new(config).map_err(|e| format!("Failed to create node: {}", e))
}
