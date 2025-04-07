use std::path::PathBuf;
use structopt::StructOpt;
use vibecoin::config::{Config, NetworkConfig, ConsensusConfig, StorageConfig, NodeConfig};
use vibecoin::init_logger;
use log::{info, warn, error};

#[derive(Debug, StructOpt)]
#[structopt(name = "vibecoin-config", about = "VibeCoin configuration tool")]
struct Opt {
    /// Generate a default configuration
    #[structopt(long)]
    generate: bool,
    
    /// Output file
    #[structopt(long, parse(from_os_str))]
    output: Option<PathBuf>,
    
    /// Input file
    #[structopt(long, parse(from_os_str))]
    input: Option<PathBuf>,
    
    /// Node name
    #[structopt(long)]
    node_name: Option<String>,
    
    /// Data directory
    #[structopt(long)]
    data_dir: Option<String>,
    
    /// Network (dev, testnet, mainnet)
    #[structopt(long)]
    network: Option<String>,
    
    /// Listen address
    #[structopt(long)]
    listen_addr: Option<String>,
    
    /// Listen port
    #[structopt(long)]
    listen_port: Option<u16>,
    
    /// API port
    #[structopt(long)]
    api_port: Option<u16>,
    
    /// API host
    #[structopt(long)]
    api_host: Option<String>,
    
    /// Metrics port
    #[structopt(long)]
    metrics_port: Option<u16>,
    
    /// Enable metrics
    #[structopt(long)]
    enable_metrics: Option<bool>,
    
    /// Enable API
    #[structopt(long)]
    enable_api: Option<bool>,
    
    /// Enable mining
    #[structopt(long)]
    enable_mining: Option<bool>,
    
    /// Mining threads
    #[structopt(long)]
    mining_threads: Option<usize>,
    
    /// Bootstrap nodes
    #[structopt(long)]
    bootstrap: Option<String>,
    
    /// Chain ID
    #[structopt(long)]
    chain_id: Option<u64>,
}

fn main() {
    // Initialize logger
    init_logger();
    
    // Parse command line arguments
    let opt = Opt::from_args();
    
    // Generate a default configuration
    if opt.generate {
        let mut config = Config::default();
        
        // Update config with command line arguments
        if let Some(node_name) = opt.node_name {
            config.node.node_name = node_name;
        }
        
        if let Some(data_dir) = opt.data_dir {
            config.node.data_dir = data_dir;
        }
        
        if let Some(network) = opt.network {
            match network.as_str() {
                "dev" => {
                    config.consensus.chain_id = 1337;
                    config.consensus.target_block_time = 5;
                    config.consensus.initial_difficulty = 100;
                    config.consensus.enable_mining = true;
                    config.network.bootstrap_nodes = vec![];
                },
                "testnet" => {
                    config.consensus.chain_id = 2;
                    config.consensus.target_block_time = 10;
                    config.consensus.initial_difficulty = 1000;
                    config.network.bootstrap_nodes = vec![
                        "/dns4/bootstrap1.vibecoin.network/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp".to_string(),
                        "/dns4/bootstrap2.vibecoin.network/tcp/30333/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9".to_string(),
                    ];
                },
                "mainnet" => {
                    config.consensus.chain_id = 1;
                    config.consensus.target_block_time = 10;
                    config.consensus.initial_difficulty = 10000;
                    config.network.bootstrap_nodes = vec![
                        "/dns4/bootstrap1.vibecoin.network/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp".to_string(),
                        "/dns4/bootstrap2.vibecoin.network/tcp/30333/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9".to_string(),
                        "/dns4/bootstrap3.vibecoin.network/tcp/30333/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9".to_string(),
                        "/dns4/bootstrap4.vibecoin.network/tcp/30333/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9".to_string(),
                    ];
                },
                _ => {
                    warn!("Unknown network: {}, using default", network);
                }
            }
        }
        
        if let Some(listen_addr) = opt.listen_addr {
            config.network.listen_addr = listen_addr;
        }
        
        if let Some(listen_port) = opt.listen_port {
            config.network.listen_port = listen_port;
        }
        
        if let Some(api_port) = opt.api_port {
            config.node.api_port = api_port;
        }
        
        if let Some(api_host) = opt.api_host {
            config.node.api_host = api_host;
        }
        
        if let Some(metrics_port) = opt.metrics_port {
            config.node.metrics_port = metrics_port;
        }
        
        if let Some(enable_metrics) = opt.enable_metrics {
            config.node.enable_metrics = enable_metrics;
        }
        
        if let Some(enable_api) = opt.enable_api {
            config.node.enable_api = enable_api;
        }
        
        if let Some(enable_mining) = opt.enable_mining {
            config.consensus.enable_mining = enable_mining;
        }
        
        if let Some(mining_threads) = opt.mining_threads {
            config.consensus.mining_threads = mining_threads;
        }
        
        if let Some(bootstrap) = opt.bootstrap {
            config.network.bootstrap_nodes = bootstrap.split(',')
                .map(|s| format!("/dns4/{}/tcp/30333", s))
                .collect();
        }
        
        if let Some(chain_id) = opt.chain_id {
            config.consensus.chain_id = chain_id;
        }
        
        // Save the configuration
        if let Some(output) = opt.output {
            match config.save(&output) {
                Ok(_) => {
                    info!("Configuration saved to {:?}", output);
                },
                Err(e) => {
                    error!("Failed to save configuration: {}", e);
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
                    error!("Failed to serialize configuration: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else if let Some(input) = opt.input {
        // Load the configuration
        match Config::load(&input) {
            Ok(config) => {
                // Print the configuration to stdout
                match toml::to_string_pretty(&config) {
                    Ok(config_str) => {
                        println!("{}", config_str);
                    },
                    Err(e) => {
                        error!("Failed to serialize configuration: {}", e);
                        std::process::exit(1);
                    }
                }
            },
            Err(e) => {
                error!("Failed to load configuration: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Print help
        println!("{}", Opt::clap().get_help());
    }
}
