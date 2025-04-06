# VibeCoin Network Module

## Overview

The network module provides the peer-to-peer networking layer for the VibeCoin blockchain. It handles peer discovery, connection management, message broadcasting, and blockchain data synchronization.

## Components

### Types (types/)

- **message.rs**: Defines the network message types for peer-to-peer communication
- **node_info.rs**: Contains information about nodes in the network

### Peer Management (peer/)

- **handler.rs**: Handles individual peer connections
- **state.rs**: Manages peer state and connection information
- **manager.rs**: Coordinates peer connections and message broadcasting

### Network Service (service/)

- **listener.rs**: Listens for incoming connections
- **dialer.rs**: Establishes outbound connections
- **router.rs**: Routes messages to the appropriate handlers
- **mod.rs**: Main network service implementation

### Message Codec (codec/)

- **frame.rs**: Handles message framing and serialization/deserialization

## Usage

```rust
use vibecoin::network::{NetworkConfig, start_network};
use tokio::sync::mpsc;

// Create a network configuration
let config = NetworkConfig {
    bind_addr: "127.0.0.1:8765".parse().unwrap(),
    seed_peers: vec!["127.0.0.1:8766".parse().unwrap()],
    max_outbound: 8,
    max_inbound: 32,
    node_id: "my-node".to_string(),
};

// Create a message channel
let (tx, rx) = mpsc::channel(100);

// Start the network service
let service = start_network(config, rx).await;

// The service is now running and will:
// - Listen for incoming connections
// - Connect to seed peers
// - Handle message broadcasting
// - Manage peer connections
```

## Message Types

The network module supports the following message types:

- **Handshake**: Initial connection handshake with node information
- **NewBlock**: Broadcast a new block to peers
- **NewTransaction**: Broadcast a new transaction to peers
- **RequestBlock**: Request a block by height
- **ResponseBlock**: Response to a block request
- **RequestBlockRange**: Request a range of blocks
- **ResponseBlockRange**: Response to a block range request
- **Ping/Pong**: Connection keepalive messages
- **Disconnect**: Graceful disconnection with reason

## Design Considerations

- **Modularity**: Each component has a clear responsibility
- **Async/Await**: Uses Tokio for asynchronous networking
- **Error Handling**: Robust error handling for network failures
- **Reconnection**: Automatic reconnection with exponential backoff
- **Message Routing**: Flexible message routing system
- **Testing**: Comprehensive unit and integration tests

## Future Improvements

- WebSocket support for browser clients
- NAT traversal for better connectivity
- Peer discovery via DNS seeds
- Bandwidth throttling and rate limiting
- Connection encryption (TLS/noise protocol)
- DHT-based peer discovery
