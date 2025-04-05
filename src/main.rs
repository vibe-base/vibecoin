use vibecoin::node::runner::{Node, NodeConfig};
use std::{thread, time::Duration};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};

// Commands that can be sent to the node
enum NodeCommand {
    CreateTransaction(u64),
    GetState,
    GetNetworkStatus,
    Stop,
}

// Responses from the node
enum NodeResponse {
    TransactionCreated(String),
    State(String),
    NetworkStatus(String),
    Error(String),
}

fn main() {
    println!("Starting VibeCoin Node Runner...\n");

    // Create node configuration
    let config = NodeConfig {
        difficulty: 4,                // Lower difficulty for faster mining
        mining_enabled: true,
        target_block_time: 20,        // Target 20 seconds between blocks
        mine_empty_blocks: true,      // Mine even if there are no transactions
        max_tx_age: 3600,             // 1 hour max age for transactions
        poh_ticks_per_second: 1000,
        wallet_path: "config/node_wallet.json".to_string(),
        wallet_password: "password".to_string(),
        enable_networking: true,      // Enable networking
        p2p_listen_addr: Some("0.0.0.0:8333".parse().unwrap()),
        seed_nodes: Vec::new(),
        max_inbound: 125,
        max_outbound: 8,
    };

    // Create channels for communication with the node
    let (cmd_tx, cmd_rx) = mpsc::channel();
    let (resp_tx, resp_rx) = mpsc::channel();

    // Start the node in a separate thread
    let node_thread = thread::spawn(move || {
        run_node(config, cmd_rx, resp_tx);
    });

    // Wait a bit for the node to initialize
    thread::sleep(Duration::from_secs(1));

    // Interactive command loop
    println!("\nVibeCoin Node is running!");
    println!("Commands:\n  tx <amount> - Create a test transaction\n  state - Show blockchain state\n  network - Show network status\n  exit - Exit the node\n");

    let mut input = String::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("vibecoin> ");
        stdout.flush().unwrap();

        input.clear();
        if stdin.read_line(&mut input).is_err() {
            eprintln!("Error reading input");
            continue;
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        match parts[0] {
            "tx" => {
                if parts.len() < 2 {
                    println!("Usage: tx <amount>");
                    continue;
                }

                let amount = match parts[1].parse::<u64>() {
                    Ok(amount) => amount,
                    Err(_) => {
                        println!("Invalid amount");
                        continue;
                    }
                };

                // Send command to create transaction
                if let Err(e) = cmd_tx.send(NodeCommand::CreateTransaction(amount)) {
                    eprintln!("Failed to send command: {}", e);
                    continue;
                }

                // Wait for response
                match resp_rx.recv() {
                    Ok(NodeResponse::TransactionCreated(hash)) => {
                        println!("Transaction created with hash: {}", hash);
                    },
                    Ok(NodeResponse::Error(msg)) => {
                        println!("Error: {}", msg);
                    },
                    _ => {
                        println!("Unexpected response");
                    }
                }
            },
            "state" => {
                // Send command to get state
                if let Err(e) = cmd_tx.send(NodeCommand::GetState) {
                    eprintln!("Failed to send command: {}", e);
                    continue;
                }

                // Wait for response
                match resp_rx.recv() {
                    Ok(NodeResponse::State(state)) => {
                        println!("Blockchain state:\n{}", state);
                    },
                    Ok(NodeResponse::Error(msg)) => {
                        println!("Error: {}", msg);
                    },
                    _ => {
                        println!("Unexpected response");
                    }
                }
            },
            "exit" => {
                println!("Exiting VibeCoin Node");

                // Send command to stop the node
                if let Err(e) = cmd_tx.send(NodeCommand::Stop) {
                    eprintln!("Failed to send stop command: {}", e);
                }

                break;
            },
            "network" => {
                // Send command to get network status
                if let Err(e) = cmd_tx.send(NodeCommand::GetNetworkStatus) {
                    eprintln!("Failed to send command: {}", e);
                    continue;
                }

                // Wait for response
                match resp_rx.recv() {
                    Ok(NodeResponse::NetworkStatus(status)) => {
                        println!("Network status:\n{}", status);
                    },
                    Ok(NodeResponse::Error(msg)) => {
                        println!("Error: {}", msg);
                    },
                    _ => {
                        println!("Unexpected response");
                    }
                }
            },
            _ => {
                println!("Unknown command: {}", parts[0]);
                println!("Commands:\n  tx <amount> - Create a test transaction\n  state - Show blockchain state\n  network - Show network status\n  exit - Exit the node");
            }
        }
    }

    // Wait for the node thread to finish
    if let Err(e) = node_thread.join() {
        eprintln!("Error joining node thread: {:?}", e);
    }

    println!("Node runner demonstration complete!");
}

// Run the node with command handling
fn run_node(config: NodeConfig, cmd_rx: Receiver<NodeCommand>, resp_tx: Sender<NodeResponse>) {
    // Create the node
    let node = match Node::new(config) {
        Ok(node) => node,
        Err(e) => {
            let _ = resp_tx.send(NodeResponse::Error(format!("Failed to create node: {}", e)));
            return;
        }
    };

    // Flag to control the main loop
    let mut running = true;

    // Start the mining loop in a separate thread
    let node_arc = Arc::new(Mutex::new(node));
    let node_arc_clone = node_arc.clone();

    let mining_thread = thread::spawn(move || {
        let mut last_mine_time = std::time::Instant::now();

        while running {
            // Check if it's time to mine a new block
            let node_config = {
                let node = node_arc_clone.lock().unwrap();
                (node.config.mining_enabled, node.config.target_block_time)
            };

            if node_config.0 && last_mine_time.elapsed() >= Duration::from_secs(node_config.1) {
                // Mine a block
                let mut node = node_arc_clone.lock().unwrap();
                if let Err(e) = node.mine_new_block() {
                    eprintln!("Mining error: {}", e);
                }
                last_mine_time = std::time::Instant::now();
            }

            // Sleep to prevent CPU hogging
            thread::sleep(Duration::from_millis(100));
        }
    });

    // Main command processing loop
    while running {
        // Check for commands
        match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(NodeCommand::CreateTransaction(amount)) => {
                let mut node = node_arc.lock().unwrap();
                match node.create_test_transaction(amount) {
                    Ok(tx) => {
                        let hash = hex::encode(tx.hash);
                        if let Err(e) = node.add_transaction(tx) {
                            let _ = resp_tx.send(NodeResponse::Error(format!("Failed to add transaction: {}", e)));
                        } else {
                            let _ = resp_tx.send(NodeResponse::TransactionCreated(hash));
                        }
                    },
                    Err(e) => {
                        let _ = resp_tx.send(NodeResponse::Error(format!("Failed to create transaction: {}", e)));
                    }
                }
            },
            Ok(NodeCommand::GetState) => {
                let node = node_arc.lock().unwrap();
                let state = node.get_blockchain_state();
                let _ = resp_tx.send(NodeResponse::State(state));
            },
            Ok(NodeCommand::GetNetworkStatus) => {
                let node = node_arc.lock().unwrap();
                let status = node.get_network_status();
                let _ = resp_tx.send(NodeResponse::NetworkStatus(status));
            },
            Ok(NodeCommand::Stop) => {
                running = false;
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // No command received, continue
            },
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Channel disconnected, stop the node
                running = false;
            }
        }

        // Update PoH
        let mut node = node_arc.lock().unwrap();
        node.poh_generator.tick();
    }

    // Wait for the mining thread to finish
    if let Err(e) = mining_thread.join() {
        eprintln!("Error joining mining thread: {:?}", e);
    }

    println!("Node stopped");
}
