use std::net::SocketAddr;

/// Peer information for bootstrap nodes
#[derive(Debug, Clone)]
pub struct BootstrapPeer {
    /// Network address of the peer
    pub address: SocketAddr,
}

/// Returns a hardcoded list of known-good Vibecoin bootstrap peers
pub fn get_bootstrap_peers() -> Vec<BootstrapPeer> {
    let raw_peers = vec![
        "155.138.225.82:8333",
        "45.76.65.28:8333",
        "127.0.0.1:9001",         // Optional: local testing
        "127.0.0.1:9002",         // Optional: local testing
    ];

    raw_peers
        .into_iter()
        .filter_map(|addr| addr.parse::<SocketAddr>().ok())
        .map(|address| BootstrapPeer { address })
        .collect()
}

/// Returns a list of bootstrap peers as socket addresses
pub fn get_bootstrap_addresses() -> Vec<SocketAddr> {
    get_bootstrap_peers()
        .into_iter()
        .map(|peer| peer.address)
        .collect()
}
