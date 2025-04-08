use std::sync::Arc;
use std::path::Path;
use vibecoin::network::peer::broadcaster::PeerBroadcaster;
use vibecoin::network::peer::registry::PeerRegistry;
use vibecoin::network::sync::sync_service::SyncService;
use vibecoin::storage::RocksDBStore;
use vibecoin::storage::block_store::BlockStore;

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

    // Create a RocksDB store with 'static lifetime
    let kv_store = Box::leak(Box::new(RocksDBStore::new(Path::new("./data/vibecoin/db")).expect("Failed to open RocksDB")));

    // Create a block store
    let block_store = Arc::new(BlockStore::new(kv_store));

    // Create a sync service
    let sync_service = SyncService::new(
        block_store,
        peer_registry.clone(),
        broadcaster.clone(),
    );

    // Start the sync service
    match sync_service.start().await {
        Ok(_) => println!("Successfully started sync service"),
        Err(e) => {
            println!("Failed to start sync service: {}", e);
            return;
        }
    }

    // Try to sync to a specific height (e.g., 1000)
    match sync_service.sync_to_height(1000).await {
        Ok(_) => println!("Successfully triggered sync to height 1000"),
        Err(e) => println!("Failed to trigger sync to height 1000: {}", e),
    }
}
