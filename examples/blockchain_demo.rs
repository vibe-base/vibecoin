use vibecoin::consensus::poh::ProofOfHistory;
use vibecoin::consensus::pow;
use vibecoin::ledger::block::Block;
use vibecoin::ledger::transaction::Transaction;
use vibecoin::ledger::state::Blockchain;
use vibecoin::types::primitives::PublicKey;
use std::{thread, time::Duration};
use std::sync::Arc;

fn main() {
    println!("Starting VibeCoin Blockchain Demo");

    // Initialize PoH with genesis hash
    let initial_hash = [0u8; 32];
    let mut poh = ProofOfHistory::new(initial_hash, 1000);

    // Create genesis block
    println!("\nCreating genesis block...");
    let genesis = Blockchain::create_genesis_block().expect("Failed to create genesis block");
    println!("Genesis block hash: {}", hex::encode(genesis.hash));

    // Create blockchain with low difficulty for faster mining
    let difficulty = 4; // Low difficulty for demo
    let mut blockchain = Blockchain::new(genesis, difficulty, poh.clone());

    // Create a transaction
    println!("\nCreating a transaction...");
    let from: PublicKey = [1u8; 32];
    let to: PublicKey = [2u8; 32];
    let amount = 100;

    let mut tx = Transaction::new(from, to, amount).expect("Failed to create transaction");
    tx.signature = Arc::new([1u8; 64]); // Set a dummy signature

    // Create a block
    println!("Creating a block...");
    let previous_hash = blockchain.latest_block().hash;
    let transactions = vec![tx];
    let mut block = Block::new(1, previous_hash, transactions).expect("Failed to create block");

    // Mine the block
    println!("Mining the block...");
    let max_time = Some(Duration::from_secs(10));

    if let Ok(_) = pow::mine_block(&mut block, difficulty, &mut poh, None, max_time) {
        println!("Block mined successfully!");
        println!("Block hash: {}", hex::encode(block.hash));

        // Add block to blockchain
        match blockchain.add_block(block) {
            Ok(_) => {
                println!("Block added to blockchain!");

                // Show blockchain state
                let state = blockchain.get_state_summary();
                println!("\nBlockchain state:");
                println!("Height: {}", state.height);
                println!("Latest hash: {}", hex::encode(state.latest_hash));
            },
            Err(e) => {
                println!("Failed to add block to blockchain: {}", e);
            }
        }
    } else {
        println!("Mining failed or timed out");
    }

    println!("\nDemo complete!");
}
