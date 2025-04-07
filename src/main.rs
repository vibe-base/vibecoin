use std::sync::Arc;
use std::path::Path;
use tokio::sync::mpsc;
use log::{info, error, warn};

use vibecoin::init_logger;
use vibecoin::storage::{RocksDBStore, BlockStore, TxStore, StateStore, BatchOperationManager};
use vibecoin::consensus::{start_consensus, config::ConsensusConfig};
use vibecoin::network::{start_network, config::NetworkConfig};

#[tokio::main]
async fn main() {
    // Initialize logger
    init_logger();

    info!("Starting Vibecoin node...");

    // Create data directory if it doesn't exist
    let data_dir = Path::new("./data/vibecoin");
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir).expect("Failed to create data directory");
    }

    // Initialize storage
    info!("Initializing storage...");
    let kv_store = Arc::new(RocksDBStore::new(data_dir).expect("Failed to initialize RocksDB"));
    let block_store = Arc::new(BlockStore::new(&kv_store));
    let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
    let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

    // Create batch operation manager
    let batch_manager = Arc::new(BatchOperationManager::new(
        kv_store.clone(),
        block_store.clone(),
        tx_store.clone(),
        state_store.clone(),
    ));

    // Initialize network
    info!("Initializing network...");
    let network_config = NetworkConfig::default();
    let (network_tx, network_rx) = mpsc::channel(100);
    let (block_tx, block_rx) = mpsc::channel(100);
    let (tx_tx, tx_rx) = mpsc::channel(1000);

    let network = start_network(
        network_config,
        network_rx,
        block_tx,
        tx_tx,
    ).await;

    // Initialize consensus
    info!("Initializing consensus...");
    let consensus_config = ConsensusConfig::default();
    let consensus = start_consensus(
        consensus_config,
        kv_store.clone(),
        block_store.clone(),
        tx_store.clone(),
        state_store.clone(),
        network_tx,
    ).await;

    info!("Vibecoin node started successfully");

    // Keep the main thread alive
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl-c");
    info!("Shutting down Vibecoin node...");
}
