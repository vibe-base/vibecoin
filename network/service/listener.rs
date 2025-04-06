use std::net::SocketAddr;
use tokio::net::TcpListener;
use log::{error, info};

use crate::network::peer::manager::PeerManager;

/// Start a TCP listener for incoming connections
pub async fn start_listener(
    bind_addr: SocketAddr,
    peer_manager: PeerManager,
) -> Result<(), std::io::Error> {
    // Bind to the address
    let listener = TcpListener::bind(bind_addr).await?;
    info!("Listening for connections on {}", bind_addr);
    
    // Accept connections
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("Accepted connection from {}", addr);
                
                // Clone the peer manager for the task
                let peer_manager = peer_manager.clone();
                
                // Handle the connection in a separate task
                tokio::spawn(async move {
                    peer_manager.handle_inbound_connection(stream, addr).await;
                });
            }
            Err(e) => {
                error!("Error accepting connection: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;
    use std::time::Duration;
    use crate::network::service::router::MessageRouter;
    use crate::network::types::node_info::NodeInfo;
    use tokio::sync::mpsc;
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_listener_binds() {
        // This test just checks that the listener can bind to a port
        // We'll use a random high port to avoid conflicts
        let bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        
        // Create a message router
        let router = Arc::new(MessageRouter::new());
        
        // Create channels for incoming messages
        let (incoming_tx, _incoming_rx) = mpsc::channel(100);
        
        // Create a local node info
        let local_node_info = NodeInfo::new(
            "1.0.0".to_string(),
            "local-node".to_string(),
            bind_addr,
        );
        
        // Create a peer manager
        let peer_manager = PeerManager::new(
            local_node_info,
            router,
            incoming_tx,
            8,
            32,
        );
        
        // Start the listener in a separate task
        let listener_task = tokio::spawn(async move {
            let _ = start_listener(bind_addr, peer_manager).await;
        });
        
        // Give it a moment to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Cancel the task
        listener_task.abort();
    }
}
