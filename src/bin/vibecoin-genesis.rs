use std::path::PathBuf;
use structopt::StructOpt;
use vibecoin::tools::genesis::{GenesisConfig, generate_genesis};
use vibecoin::init_logger;
use log::{info, warn, error};

#[derive(Debug, StructOpt)]
#[structopt(name = "vibecoin-genesis", about = "VibeCoin genesis block generator")]
struct Opt {
    /// Generate a default genesis configuration
    #[structopt(long)]
    generate: bool,
    
    /// Output file
    #[structopt(long, parse(from_os_str))]
    output: Option<PathBuf>,
    
    /// Input file
    #[structopt(long, parse(from_os_str))]
    input: Option<PathBuf>,
    
    /// Network (dev, testnet, mainnet)
    #[structopt(long)]
    network: Option<String>,
    
    /// Chain ID
    #[structopt(long)]
    chain_id: Option<u64>,
    
    /// Initial difficulty
    #[structopt(long)]
    initial_difficulty: Option<u64>,
    
    /// Genesis timestamp
    #[structopt(long)]
    timestamp: Option<u64>,
}

fn main() {
    // Initialize logger
    init_logger();
    
    // Parse command line arguments
    let opt = Opt::from_args();
    
    // Generate a default genesis configuration
    if opt.generate {
        let mut config = GenesisConfig::default();
        
        // Update config with command line arguments
        if let Some(network) = opt.network {
            match network.as_str() {
                "dev" => {
                    config.chain_id = 1337;
                    config.initial_difficulty = 100;
                    config.timestamp = chrono::Utc::now().timestamp() as u64;
                },
                "testnet" => {
                    config.chain_id = 2;
                    config.initial_difficulty = 1000;
                    config.timestamp = 1609459200; // 2021-01-01 00:00:00 UTC
                },
                "mainnet" => {
                    config.chain_id = 1;
                    config.initial_difficulty = 10000;
                    config.timestamp = 1609459200; // 2021-01-01 00:00:00 UTC
                },
                _ => {
                    warn!("Unknown network: {}, using default", network);
                }
            }
        }
        
        if let Some(chain_id) = opt.chain_id {
            config.chain_id = chain_id;
        }
        
        if let Some(initial_difficulty) = opt.initial_difficulty {
            config.initial_difficulty = initial_difficulty;
        }
        
        if let Some(timestamp) = opt.timestamp {
            config.timestamp = timestamp;
        }
        
        // Save the configuration
        if let Some(output) = opt.output {
            match config.save(&output) {
                Ok(_) => {
                    info!("Genesis configuration saved to {:?}", output);
                    
                    // Generate the genesis block
                    match generate_genesis(&output) {
                        Ok((block, account_states)) => {
                            info!("Genesis block generated with hash: {}", hex::encode(&block.hash));
                            info!("Initial accounts: {}", account_states.len());
                        },
                        Err(e) => {
                            error!("Failed to generate genesis block: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to save genesis configuration: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            // Print the configuration to stdout
            match toml::to_string_pretty(&config) {
                Ok(config_str) => {
                    println!("{}", config_str);
                },
                Err(e) => {
                    error!("Failed to serialize genesis configuration: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else if let Some(input) = opt.input {
        // Generate the genesis block from the configuration
        match generate_genesis(&input) {
            Ok((block, account_states)) => {
                info!("Genesis block generated with hash: {}", hex::encode(&block.hash));
                info!("Initial accounts: {}", account_states.len());
                
                // Print the block details
                println!("Genesis Block:");
                println!("  Hash: {}", hex::encode(&block.hash));
                println!("  Height: {}", block.height);
                println!("  Timestamp: {}", block.timestamp);
                println!("  Difficulty: {}", block.difficulty);
                println!("  State Root: {}", hex::encode(&block.state_root));
                println!("  Transaction Root: {}", hex::encode(&block.tx_root));
                println!("  Initial Accounts: {}", account_states.len());
                
                for (address, state) in &account_states {
                    println!("    {}: {} VIBE", hex::encode(address), state.balance);
                }
            },
            Err(e) => {
                error!("Failed to generate genesis block: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Print help
        println!("{}", Opt::clap().get_help());
    }
}
