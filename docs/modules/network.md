# Network Module

## Overview

The Network Module provides the peer-to-peer communication layer for the VibeCoin blockchain. It handles peer discovery, connection management, message broadcasting, and blockchain data synchronization.

## Components

### Peer Management

The peer management component handles peer connections:

- **PeerHandler**: Manages individual peer connections
- **PeerInfo**: Tracks peer state and metadata
- **PeerManager**: Coordinates peer connections and message broadcasting
- **Connection States**: Disconnected, Connecting, Connected, Ready, Failed, Banned

### Message Types

The network module supports various message types:

- **Handshake**: Initial connection handshake with node information
- **NewBlock**: Broadcast a new block to peers
- **NewTransaction**: Broadcast a new transaction to peers
- **RequestBlock/ResponseBlock**: Request and respond with specific blocks
- **RequestBlockRange/ResponseBlockRange**: Request and respond with ranges of blocks
- **Ping/Pong**: Connection keepalive messages
- **Disconnect**: Graceful disconnection with reason

### Network Service

The network service coordinates all network activities:

- **Listener**: Accepts incoming connections
- **Dialer**: Establishes outbound connections
- **Router**: Routes messages to appropriate handlers
- **Message Processing**: Handles incoming and outgoing messages

### Message Codec

The message codec handles serialization and framing:

- **FramedReader**: Reads length-prefixed messages
- **FramedWriter**: Writes length-prefixed messages
- **Serialization**: Uses bincode for efficient binary serialization

## Implementation Details

```rust
// Network Configuration
pub struct NetworkConfig {
    pub bind_addr: SocketAddr,
    pub seed_peers: Vec<SocketAddr>,
    pub max_outbound: usize,
    pub max_inbound: usize,
    pub node_id: String,
}

// Message Types
pub enum NetMessage {
    Handshake(NodeInfo),
    NewBlock(Block),
    NewTransaction(TransactionRecord),
    RequestBlock(u64),
    ResponseBlock(Option<Block>),
    RequestBlockRange { start_height: u64, end_height: u64 },
    ResponseBlockRange(Vec<Block>),
    Ping(u64),
    Pong(u64),
    Disconnect(DisconnectReason),
}

// Peer Connection States
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Ready,
    Failed,
    Banned,
}
```

## Network Flow

### Connection Establishment

1. **Outbound Connection**:
   - Connect to a peer address
   - Send handshake message
   - Wait for handshake response
   - Verify compatibility
   - Mark connection as ready

2. **Inbound Connection**:
   - Accept incoming connection
   - Wait for handshake message
   - Verify compatibility
   - Send handshake response
   - Mark connection as ready

### Message Handling

1. **Sending Messages**:
   - Serialize the message
   - Write the message length
   - Write the message data
   - Flush the connection

2. **Receiving Messages**:
   - Read the message length
   - Read the message data
   - Deserialize the message
   - Route to appropriate handler

### Peer Discovery

1. **Initial Peers**:
   - Connect to seed peers from configuration
   - Exchange peer information during handshake
   - Add new peers to the peer list

2. **Reconnection**:
   - Track failed connections
   - Implement exponential backoff for retries
   - Periodically attempt to reconnect

## Security Considerations

- **Peer Validation**: Verify peer identity and protocol version
- **Message Validation**: Validate all incoming messages
- **Rate Limiting**: Prevent DoS attacks through message flooding
- **Banning**: Ban peers that violate protocol rules

## Performance Considerations

- **Asynchronous I/O**: Uses Tokio for efficient async networking
- **Connection Pooling**: Maintains a pool of connections
- **Message Batching**: Batches messages for efficient transmission
- **Backpressure**: Implements backpressure for message handling

## Future Improvements

- **WebSocket Support**: Add WebSocket transport for browser clients
- **NAT Traversal**: Implement techniques for NAT traversal
- **Encryption**: Add transport-level encryption
- **Peer Discovery**: Implement DHT-based peer discovery
- **Bandwidth Throttling**: Add bandwidth management
