use std::path::Path;
use std::env;

use vibecoin::storage::RocksDBStore;
use vibecoin::storage::BlockStore;
use vibecoin::storage::KVStore;

fn main() {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <command> [args]", args[0]);
        println!("Commands:");
        println!("  latest - Get the latest block");
        println!("  height <height> - Get a block by height");
        println!("  hash <hash> - Get a block by hash");
        println!("  range <start> <end> - Get blocks in a height range");
        return;
    }

    // Initialize the database
    let db_path = Path::new("./data/vibecoin/db");
    let kv_store = match RocksDBStore::new(db_path) {
        Ok(store) => store,
        Err(e) => {
            println!("Error opening database: {}", e);
            return;
        }
    };
    let block_store = BlockStore::new(&kv_store);

    // Process the command
    match args[1].as_str() {
        "latest" => {
            match block_store.get_latest_height() {
                Some(height) => {
                    match block_store.get_block_by_height(height) {
                        Ok(Some(block)) => {
                            println!("Latest block:");
                            println!("  Height: {}", block.height);
                            println!("  Hash: {}", hex::encode(&block.hash));
                            println!("  Previous Hash: {}", hex::encode(&block.prev_hash));
                            println!("  Timestamp: {}", block.timestamp);
                            println!("  Transactions: {}", block.transactions.len());
                            println!("  State Root: {}", hex::encode(&block.state_root));
                            println!("  Transaction Root: {}", hex::encode(&block.tx_root));
                            println!("  Nonce: {}", block.nonce);
                            println!("  PoH Sequence: {}", block.poh_seq);
                            println!("  PoH Hash: {}", hex::encode(&block.poh_hash));
                            println!("  Difficulty: {}", block.difficulty);
                            println!("  Total Difficulty: {}", block.total_difficulty);
                        }
                        Ok(None) => println!("Block not found at height {}", height),
                        Err(e) => println!("Error retrieving block: {}", e),
                    }
                }
                None => println!("No blocks found"),
            }
        }
        "height" => {
            if args.len() < 3 {
                println!("Missing height argument");
                return;
            }
            let height = match args[2].parse::<u64>() {
                Ok(h) => h,
                Err(_) => {
                    println!("Invalid height: {}", args[2]);
                    return;
                }
            };
            match block_store.get_block_by_height(height) {
                Ok(Some(block)) => {
                    println!("Block at height {}:", height);
                    println!("  Hash: {}", hex::encode(&block.hash));
                    println!("  Previous Hash: {}", hex::encode(&block.prev_hash));
                    println!("  Timestamp: {}", block.timestamp);
                    println!("  Transactions: {}", block.transactions.len());
                    println!("  State Root: {}", hex::encode(&block.state_root));
                    println!("  Transaction Root: {}", hex::encode(&block.tx_root));
                    println!("  Nonce: {}", block.nonce);
                    println!("  PoH Sequence: {}", block.poh_seq);
                    println!("  PoH Hash: {}", hex::encode(&block.poh_hash));
                    println!("  Difficulty: {}", block.difficulty);
                    println!("  Total Difficulty: {}", block.total_difficulty);
                }
                Ok(None) => println!("Block not found at height {}", height),
                Err(e) => println!("Error retrieving block: {}", e),
            }
        }
        "hash" => {
            if args.len() < 3 {
                println!("Missing hash argument");
                return;
            }
            let hash_str = &args[2];
            let hash = match hex::decode(hash_str) {
                Ok(h) => {
                    if h.len() != 32 {
                        println!("Invalid hash length: {}", h.len());
                        return;
                    }
                    let mut hash_array = [0u8; 32];
                    hash_array.copy_from_slice(&h);
                    hash_array
                }
                Err(_) => {
                    println!("Invalid hash: {}", hash_str);
                    return;
                }
            };
            match block_store.get_block_by_hash(&hash) {
                Ok(Some(block)) => {
                    println!("Block with hash {}:", hash_str);
                    println!("  Height: {}", block.height);
                    println!("  Previous Hash: {}", hex::encode(&block.prev_hash));
                    println!("  Timestamp: {}", block.timestamp);
                    println!("  Transactions: {}", block.transactions.len());
                    println!("  State Root: {}", hex::encode(&block.state_root));
                    println!("  Transaction Root: {}", hex::encode(&block.tx_root));
                    println!("  Nonce: {}", block.nonce);
                    println!("  PoH Sequence: {}", block.poh_seq);
                    println!("  PoH Hash: {}", hex::encode(&block.poh_hash));
                    println!("  Difficulty: {}", block.difficulty);
                    println!("  Total Difficulty: {}", block.total_difficulty);
                }
                Ok(None) => println!("Block not found with hash {}", hash_str),
                Err(e) => println!("Error retrieving block: {}", e),
            }
        }
        "range" => {
            if args.len() < 4 {
                println!("Missing start or end height arguments");
                return;
            }
            let start = match args[2].parse::<u64>() {
                Ok(h) => h,
                Err(_) => {
                    println!("Invalid start height: {}", args[2]);
                    return;
                }
            };
            let end = match args[3].parse::<u64>() {
                Ok(h) => h,
                Err(_) => {
                    println!("Invalid end height: {}", args[3]);
                    return;
                }
            };
            match block_store.get_blocks_by_height_range(start, end) {
                Ok(blocks) => {
                    println!("Found {} blocks in range {}..{}", blocks.len(), start, end);
                    for block in blocks {
                        println!("Block at height {}:", block.height);
                        println!("  Hash: {}", hex::encode(&block.hash));
                        println!("  Previous Hash: {}", hex::encode(&block.prev_hash));
                        println!("  Timestamp: {}", block.timestamp);
                        println!("  Transactions: {}", block.transactions.len());
                        println!("  Nonce: {}", block.nonce);
                        println!("  PoH Sequence: {}", block.poh_seq);
                        println!("  Difficulty: {}", block.difficulty);
                        println!("  Total Difficulty: {}", block.total_difficulty);
                        println!("");
                    }
                }
                Err(e) => println!("Error retrieving blocks: {}", e),
            }
        }
        _ => {
            println!("Unknown command: {}", args[1]);
        }
    }
}
