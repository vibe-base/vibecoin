# Vibecoin Network Troubleshooting Guide

This guide provides steps to diagnose and resolve common network connectivity issues with Vibecoin nodes.

## Checking Node Connectivity

### 1. Verify Seed Server Availability

First, check if the seed server is reachable:

```bash
nc -zv 155.138.221.38 30334
```

If this fails, the seed server might be down or there might be a firewall blocking the connection.

### 2. Check Node Logs

Look for network-related log messages:

```bash
grep "network\|peer\|connection" vibecoin.log
```

Common error messages and their solutions:

- **"Failed to connect to peer"**: The peer might be offline or unreachable.
- **"Handshake timeout"**: The connection was established but the handshake protocol failed.
- **"Connection refused"**: The peer is not accepting connections on that port.

### 3. Check Firewall Settings

Make sure your firewall allows incoming and outgoing connections on the node's port:

```bash
# Check if the port is open
sudo ufw status | grep 30334

# Allow the port if needed
sudo ufw allow 30334/tcp
```

### 4. Check Network Configuration

Verify your network configuration in `config.toml`:

```toml
[network]
listen_addr = "0.0.0.0"  # Should be 0.0.0.0 to accept connections from any IP
listen_port = 30334
bootstrap_nodes = [
    "/dns4/155.138.221.38/tcp/30334/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
]
```

### 5. Test Local Network

Run two nodes on the same machine with different ports to test local networking:

```bash
# Node 1
./vibecoin --config config.toml --genesis genesis.toml

# Node 2
./vibecoin --config config_node2.toml --genesis genesis.toml
```

## Advanced Troubleshooting

### 1. Network Packet Capture

Use `tcpdump` to capture network traffic:

```bash
sudo tcpdump -i any port 30334 -w vibecoin_network.pcap
```

Analyze the capture file with Wireshark to see the actual network communication.

### 2. Check Node ID

Each node has a unique ID derived from its public key. Make sure your node ID is correctly generated:

```bash
# Look for the node ID in the logs
grep "node_id" vibecoin.log
```

### 3. Test NAT Traversal

If your node is behind NAT, test if UPnP or NAT-PMP is working:

```bash
# Check if UPnP is enabled in your config
grep "enable_upnp" config.toml

# Look for UPnP-related messages in the logs
grep "UPnP\|NAT" vibecoin.log
```

### 4. Check DHT Functionality

The Distributed Hash Table (DHT) is used for peer discovery:

```bash
# Check if DHT is enabled
grep "enable_dht" config.toml

# Look for DHT-related messages in the logs
grep "DHT" vibecoin.log
```

## Common Issues and Solutions

### 1. No Peers Connecting

**Symptoms**: Node starts but doesn't connect to any peers.

**Solutions**:
- Verify that at least one bootstrap node is reachable
- Check your firewall settings
- Ensure your node is publicly reachable if you're running a seed node
- Try adding more bootstrap nodes

### 2. Peers Connect but Disconnect Immediately

**Symptoms**: Peers connect but disconnect after a few seconds.

**Solutions**:
- Check for version mismatches between nodes
- Verify that both nodes are on the same network (mainnet, testnet, or devnet)
- Look for validation errors in the logs

### 3. Slow Synchronization

**Symptoms**: Node connects to peers but synchronizes very slowly.

**Solutions**:
- Connect to more peers
- Check your internet bandwidth
- Increase the `max_peers` setting
- Optimize your storage settings

## Getting Help

If you're still experiencing issues after trying these troubleshooting steps, please:

1. Collect your node logs
2. Note your node configuration (with sensitive information removed)
3. Describe the steps you've already taken
4. Open an issue on the Vibecoin GitHub repository or reach out on Discord

## Network Monitoring Tools

Consider using these tools to monitor your node's network activity:

- **Prometheus + Grafana**: For real-time metrics visualization
- **Netdata**: For system and network monitoring
- **Wireshark**: For detailed packet analysis
- **mtr**: For network path analysis
