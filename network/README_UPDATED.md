# VibeCoin Network Module (Updated)

## Overview

The network module provides the peer-to-peer communication layer for the VibeCoin blockchain. It handles peer discovery, connection management, message broadcasting, and blockchain data synchronization.

## Architecture

```
                  ┌─────────────────┐
                  │  NetworkService │
                  └────────┬────────┘
                           │
                           ▼
          ┌────────────────────────────────┐
          │         PeerRegistry           │
          └────────────────┬───────────────┘
                           │
                           ▼
┌──────────────────────────────────────────────────┐
│                  PeerHandler                     │
└──────────┬─────────────────┬────────────┬────────┘
           │                 │            │
           ▼                 ▼            ▼
┌─────────────────┐ ┌───────────────┐ ┌──────────────┐
│ PeerBroadcaster │ │ MessageRouter │ │ SystemRouter │
└────────┬────────┘ └───────┬───────┘ └──────┬───────┘
         │                  │                │
         ▼                  ▼                ▼
┌─────────────────┐ ┌───────────────┐ ┌──────────────┐
│ Network Messages│ │ Message Types │ │ Subsystems   │
└─────────────────┘ └───────────────┘ └──────────────┘
```

## Components

### Peer Management

- **PeerRegistry**: Tracks peer state and metadata
- **PeerHandler**: Manages individual peer connections
- **PeerBroadcaster**: Broadcasts messages to peers

### Message Routing

- **MessageRouter**: Routes messages to appropriate handlers
- **SystemRouter**: Connects network messages to subsystems (mempool, storage, consensus)

### Network Service

- **NetworkService**: Coordinates all network activities
- **Listener**: Accepts incoming connections
- **Dialer**: Establishes outbound connections

### Message Codec

- **FramedReader/Writer**: Handles message framing and serialization

## Key Features

### Enhanced Peer Management

- Comprehensive peer state tracking
- Connection health monitoring with ping/pong
- Automatic reconnection with exponential backoff
- Peer banning for misbehaving nodes

### Efficient Message Broadcasting

- Thread-safe message broadcasting to all peers
- Targeted message sending to specific peers
- Retry mechanism for reliable delivery

### Flexible Message Routing

- Pluggable message handlers
- Integration with blockchain subsystems
- Support for custom message types

### System Integration

- Direct integration with mempool for transaction processing
- Block validation through consensus engine
- State synchronization with storage module

## Usage

```rust
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

// Create a system router
let system_router = SystemRouter::new(service.router().clone())
    .with_mempool(mempool)
    .with_block_store(block_store)
    .with_consensus(consensus)
    .with_broadcaster(broadcaster);

// Initialize the system router
system_router.initialize().await;
```

## Future Improvements

- WebSocket support for browser clients
- NAT traversal for better connectivity
- Peer discovery via DNS seeds
- Connection encryption (TLS/noise protocol)
- DHT-based peer discovery
