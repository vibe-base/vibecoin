use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::time::timeout;
use std::time::Duration;
use log::{debug, error};

/// Connect to a peer with timeout
pub async fn connect_to_peer(
    addr: SocketAddr,
    timeout_secs: u64,
) -> Result<TcpStream, std::io::Error> {
    debug!("Connecting to peer {}", addr);
    
    // Connect with timeout
    match timeout(
        Duration::from_secs(timeout_secs),
        TcpStream::connect(addr),
    ).await {
        Ok(Ok(stream)) => {
            debug!("Connected to peer {}", addr);
            Ok(stream)
        }
        Ok(Err(e)) => {
            error!("Failed to connect to peer {}: {}", addr, e);
            Err(e)
        }
        Err(_) => {
            error!("Connection to peer {} timed out", addr);
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Connection timed out",
            ))
        }
    }
}

/// Connect to multiple peers in parallel
pub async fn connect_to_peers(
    addrs: &[SocketAddr],
    timeout_secs: u64,
) -> Vec<(SocketAddr, Result<TcpStream, std::io::Error>)> {
    let mut tasks = Vec::new();
    
    // Start connection tasks
    for &addr in addrs {
        let task = tokio::spawn(async move {
            let result = connect_to_peer(addr, timeout_secs).await;
            (addr, result)
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let mut results = Vec::new();
    for task in tasks {
        if let Ok(result) = task.await {
            results.push(result);
        }
    }
    
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    
    #[tokio::test]
    async fn test_connect_to_peer() {
        // Start a listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        // Accept connections in a separate task
        tokio::spawn(async move {
            let (_, _) = listener.accept().await.unwrap();
        });
        
        // Connect to the listener
        let result = connect_to_peer(addr, 5).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_connect_timeout() {
        // Try to connect to a non-existent server
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        
        // Use a short timeout
        let result = connect_to_peer(addr, 1).await;
        assert!(result.is_err());
    }
}
