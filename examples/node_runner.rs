use vibecoin::node::runner::{Node, NodeConfig};
use std::{thread, time::Duration};

fn main() {
    println!("Starting VibeCoin Node Runner Example...\n");

    // Create node configuration with low difficulty for faster mining
    let config = NodeConfig {
        difficulty: 2,                // Very low difficulty for demo
        mining_enabled: true,
        target_block_time: 10,        // Mine a block every 10 seconds
        mine_empty_blocks: true,      // Mine even if there are no transactions
        max_tx_age: 3600,             // 1 hour max age for transactions
        poh_ticks_per_second: 1000,
        wallet_path: "config/example_miner_wallet.json".to_string(),
        wallet_password: "example_password".to_string(),
    };

    // Create the node
    let mut node = match Node::new(config) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("Failed to create node: {}", e);
            return;
        }
    };

    println!("Node created successfully!");
    println!("Genesis block hash: {}", hex::encode(node.blockchain.latest_block().hash));

    // Create and add some test transactions
    println!("\nCreating test transactions...");
    for i in 1..=3 {
        let amount = i * 100;
        match node.create_test_transaction(amount) {
            Ok(tx) => {
                println!("Created transaction with amount {} and hash {}", amount, hex::encode(tx.hash));
                if let Err(e) = node.add_transaction(tx) {
                    println!("Failed to add transaction: {}", e);
                } else {
                    println!("Transaction added to mempool");
                }
            },
            Err(e) => {
                println!("Failed to create transaction: {}", e);
            }
        }
    }

    // Mine a block
    println!("\nMining a block...");
    match node.mine_new_block() {
        Ok(_) => {
            println!("Block mined successfully!");
            let state = node.blockchain.get_state_summary();
            println!("Blockchain height: {}", state.height);
            println!("Latest block hash: {}", hex::encode(state.latest_hash));
        },
        Err(e) => {
            println!("Mining failed: {}", e);
        }
    }

    // Run the node for a short time to mine more blocks
    println!("\nRunning node for 30 seconds to mine more blocks...");
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < Duration::from_secs(30) {
        // Update PoH
        node.poh_generator.tick();

        // Check if it's time to mine a new block
        if node.last_block_time.elapsed() >= Duration::from_secs(node.config.target_block_time) {
            match node.mine_new_block() {
                Ok(_) => {
                    let state = node.blockchain.get_state_summary();
                    println!("Mined block at height {}", state.height);
                },
                Err(e) => {
                    println!("Mining failed: {}", e);
                }
            }
            node.last_block_time = std::time::Instant::now();
        }

        // Sleep to prevent CPU hogging
        thread::sleep(Duration::from_millis(100));
    }

    // Show final blockchain state
    let state = node.blockchain.get_state_summary();
    println!("\nFinal blockchain state:");
    println!("Height: {}", state.height);
    println!("Latest block hash: {}", hex::encode(state.latest_hash));
    println!("Difficulty: {}", state.difficulty);
    println!("PoH count: {}", state.poh_count);

    println!("\nNode runner example complete!");
}
