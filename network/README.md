# VibeCoin Network Module

## Overview

The network module provides the peer-to-peer networking layer for the VibeCoin blockchain. It handles peer discovery, connection management, message broadcasting, and blockchain data synchronization.

## Architecture

```
                  ┌─────────────────┐
                  │  NetworkService │
                  └────────┬────────┘
                           │
                           ▼
          ┌────────────────────────────────┐
          │      AdvancedPeerRegistry      │
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

### Types (types/)

- **message.rs**: Defines the network message types for peer-to-peer communication
- **node_info.rs**: Contains information about nodes in the network

### Peer Management (peer/)

- **handler.rs**: Handles individual peer connections
- **state.rs**: Manages peer state and connection information
- **manager.rs**: Coordinates peer connections and message broadcasting
- **registry.rs**: Tracks peer state and metadata
- **broadcaster.rs**: Broadcasts messages to peers
- **advanced_registry.rs**: Enhanced peer tracking with performance metrics
- **performance.rs**: Tracks peer performance metrics
- **reputation.rs**: Manages peer reputation scores

### Network Service (service/)

- **listener.rs**: Listens for incoming connections
- **dialer.rs**: Establishes outbound connections
- **router.rs**: Routes messages to the appropriate handlers
- **advanced_router.rs**: Enhanced router with priority and statistics
- **system_router.rs**: Connects network messages to subsystems
- **mod.rs**: Main network service implementation

### Message Handlers (handlers/)

- **transaction_handler.rs**: Handles transaction messages
- **block_handler.rs**: Handles block messages
- **sync_handler.rs**: Handles synchronization messages

### Subsystem Integration (integration/)

- **mempool_integration.rs**: Connects network to the mempool
- **storage_integration.rs**: Connects network to the storage module
- **consensus_integration.rs**: Connects network to the consensus engine

### Event System (events/)

- **event_bus.rs**: Provides event-based communication
- **event_types.rs**: Defines network event types

### Synchronization (sync/)

- **sync_service.rs**: Handles blockchain synchronization
- **sync_manager.rs**: Coordinates synchronization strategies

### Message Codec (codec/)

- **frame.rs**: Handles message framing and serialization/deserialization

## Usage

### Basic Network Service

```rust
use vibecoin::network::{NetworkConfig, start_network};

// Create a network configuration
let config = NetworkConfig {
    bind_addr: "127.0.0.1:8765".parse().unwrap(),
    seed_peers: vec!["127.0.0.1:8766".parse().unwrap()],
    max_outbound: 8,
    max_inbound: 32,
    node_id: "my-node".to_string(),
};

// Start the network service
let service = start_network(config).await;

// The service is now running and will:
// - Listen for incoming connections
// - Connect to seed peers
// - Handle message broadcasting
// - Manage peer connections
```

### Enhanced Network Service

```rust
use vibecoin::network::{NetworkConfig, start_enhanced_network};

// Create a network configuration
let config = NetworkConfig {
    bind_addr: "127.0.0.1:8765".parse().unwrap(),
    seed_peers: vec!["127.0.0.1:8766".parse().unwrap()],
    max_outbound: 8,
    max_inbound: 32,
    node_id: "my-node".to_string(),
};

// Start the enhanced network service with subsystems
let service = start_enhanced_network(
    config,
    Some(block_store),
    Some(tx_store),
    Some(mempool),
    Some(consensus),
).await;

// The enhanced service provides:
// - Advanced peer management with reputation system
// - Message routing to appropriate subsystems
// - Blockchain synchronization
// - Event-based communication between components
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

## Advanced Features

### Peer Performance Tracking

The network module tracks peer performance metrics:

- Message statistics (sent, received, bytes)
- Latency measurements (ping, response time)
- Reliability (successful responses, timeouts)
- Misbehavior (invalid messages, protocol violations)

### Reputation System

Peers are assigned reputation scores based on their behavior:

- Good blocks/transactions increase reputation
- Invalid blocks/transactions decrease reputation
- Protocol violations significantly decrease reputation
- Peers with low reputation are banned

### Synchronization Strategies

Multiple synchronization strategies are supported:

- **Full Sync**: Sync from genesis to the latest block
- **Fast Sync**: Sync from a trusted checkpoint
- **Incremental Sync**: Sync only new blocks

### Event-Based Communication

The EventBus provides a publish-subscribe mechanism for components:

- Peer events (connected, disconnected)
- Block events (received, validated)
- Transaction events (received, validated)
- Sync events (requested, completed)

## Design Considerations

- **Modularity**: Each component has a clear responsibility
- **Async/Await**: Uses Tokio for asynchronous networking
- **Error Handling**: Robust error handling for network failures
- **Reconnection**: Automatic reconnection with exponential backoff
- **Message Routing**: Flexible message routing system
- **Testing**: Comprehensive unit and integration tests
- **Thread Safety**: Concurrent access with DashMap and RwLock
- **Performance**: Optimized message handling with prioritization
- **Extensibility**: Pluggable components for easy extension

## Future Improvements

- WebSocket support for browser clients
- NAT traversal for better connectivity
- Peer discovery via DNS seeds
- Bandwidth throttling and rate limiting
- Connection encryption (TLS/noise protocol)
- DHT-based peer discovery
