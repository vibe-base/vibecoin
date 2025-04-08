#!/bin/bash

# Script to create a Vibecoin configuration file

# Default values
NODE_NAME="vibecoin-node"
DATA_DIR="./data/vibecoin"
LOG_LEVEL="info"
API_HOST="127.0.0.1"
API_PORT=8545
LISTEN_ADDR="0.0.0.0"
LISTEN_PORT=30334
BOOTSTRAP_IP="155.138.221.38"
BOOTSTRAP_PORT=30334
BOOTSTRAP_PEER_ID="12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
CUSTOM_BOOTSTRAP_IP=""
MINING_ENABLED=true
MINING_THREADS=4
CHAIN_ID=1337

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --node-name)
      NODE_NAME="$2"
      shift 2
      ;;
    --data-dir)
      DATA_DIR="$2"
      shift 2
      ;;
    --log-level)
      LOG_LEVEL="$2"
      shift 2
      ;;
    --api-host)
      API_HOST="$2"
      shift 2
      ;;
    --api-port)
      API_PORT="$2"
      shift 2
      ;;
    --listen-addr)
      LISTEN_ADDR="$2"
      shift 2
      ;;
    --listen-port)
      LISTEN_PORT="$2"
      shift 2
      ;;
    --bootstrap-ip)
      BOOTSTRAP_IP="$2"
      shift 2
      ;;
    --bootstrap-port)
      BOOTSTRAP_PORT="$2"
      shift 2
      ;;
    --bootstrap-peer-id)
      BOOTSTRAP_PEER_ID="$2"
      shift 2
      ;;
    --custom-bootstrap-ip)
      CUSTOM_BOOTSTRAP_IP="$2"
      shift 2
      ;;
    --mining-enabled)
      MINING_ENABLED="$2"
      shift 2
      ;;
    --mining-threads)
      MINING_THREADS="$2"
      shift 2
      ;;
    --chain-id)
      CHAIN_ID="$2"
      shift 2
      ;;
    --help)
      echo "Usage: $0 [options]"
      echo "Options:"
      echo "  --node-name NAME           Set the node name (default: vibecoin-node)"
      echo "  --data-dir DIR             Set the data directory (default: ./data/vibecoin)"
      echo "  --log-level LEVEL          Set the log level (default: info)"
      echo "  --api-host HOST            Set the API host (default: 127.0.0.1)"
      echo "  --api-port PORT            Set the API port (default: 8545)"
      echo "  --listen-addr ADDR         Set the listen address (default: 0.0.0.0)"
      echo "  --listen-port PORT         Set the listen port (default: 30334)"
      echo "  --bootstrap-ip IP          Set the bootstrap node IP (default: 155.138.221.38)"
      echo "  --bootstrap-port PORT      Set the bootstrap node port (default: 30334)"
      echo "  --bootstrap-peer-id ID     Set the bootstrap node peer ID"
      echo "  --custom-bootstrap-ip IP   Add a custom bootstrap node IP"
      echo "  --mining-enabled BOOL      Enable or disable mining (default: true)"
      echo "  --mining-threads NUM       Set the number of mining threads (default: 4)"
      echo "  --chain-id ID              Set the chain ID (default: 1337)"
      echo "  --help                     Show this help message"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Use --help to see available options"
      exit 1
      ;;
  esac
done

# Create the config.toml file
cat > config.toml << EOF
[node]
node_name = "$NODE_NAME"
data_dir = "$DATA_DIR"
log_level = "$LOG_LEVEL"
enable_metrics = false
metrics_port = 9100
enable_api = true
api_port = $API_PORT
api_host = "$API_HOST"

[network]
listen_addr = "$LISTEN_ADDR"
listen_port = $LISTEN_PORT
bootstrap_nodes = [
    "/dns4/$BOOTSTRAP_IP/tcp/$BOOTSTRAP_PORT/p2p/$BOOTSTRAP_PEER_ID"
EOF

# Add custom bootstrap node if provided
if [ -n "$CUSTOM_BOOTSTRAP_IP" ]; then
  cat >> config.toml << EOF
,
    "/ip4/$CUSTOM_BOOTSTRAP_IP/tcp/$LISTEN_PORT"
EOF
fi

# Close the bootstrap_nodes array and add the rest of the config
cat >> config.toml << EOF
]
max_peers = 50
min_peers = 10
discovery_interval = 30
connection_timeout = 10
handshake_timeout = 5
enable_upnp = true
enable_natpmp = true
enable_dht = true
dht_bootstrap_nodes = [
    "/dns4/dht1.vibecoin.network/tcp/30334/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp",
    "/dns4/dht2.vibecoin.network/tcp/30334/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9"
EOF

# Add custom bootstrap node to DHT if provided
if [ -n "$CUSTOM_BOOTSTRAP_IP" ]; then
  cat >> config.toml << EOF
,
    "/ip4/$CUSTOM_BOOTSTRAP_IP/tcp/$LISTEN_PORT"
EOF
fi

# Close the dht_bootstrap_nodes array and add the rest of the config
cat >> config.toml << EOF
]

[consensus]
chain_id = $CHAIN_ID
enable_mining = $MINING_ENABLED
mining_threads = $MINING_THREADS
target_block_time = 5
initial_difficulty = 100
difficulty_adjustment_interval = 2016
max_transactions_per_block = 10000
max_block_size = 1048576
max_gas_per_block = 10000000
gas_price_minimum = 1
enable_poh = true
poh_tick_interval = 10
poh_ticks_per_block = 1000

[storage]
db_path = "$DATA_DIR/db"
cache_size = 512
max_open_files = 1000
write_buffer_size = 64
max_write_buffer_number = 3
enable_wal = true
enable_statistics = false
enable_compression = true
compression_type = "lz4"
enable_bloom_filters = true
bloom_filter_bits_per_key = 10
enable_auto_compaction = true
compaction_style = "level"
enable_pruning = false
pruning_keep_recent = 10000
pruning_interval = 100
EOF

echo "Created config.toml with the following settings:"
echo "Node name: $NODE_NAME"
echo "Data directory: $DATA_DIR"
echo "Log level: $LOG_LEVEL"
echo "API host: $API_HOST"
echo "API port: $API_PORT"
echo "Listen address: $LISTEN_ADDR"
echo "Listen port: $LISTEN_PORT"
echo "Bootstrap node: $BOOTSTRAP_IP:$BOOTSTRAP_PORT"
if [ -n "$CUSTOM_BOOTSTRAP_IP" ]; then
  echo "Custom bootstrap node: $CUSTOM_BOOTSTRAP_IP:$LISTEN_PORT"
fi
echo "Mining enabled: $MINING_ENABLED"
echo "Mining threads: $MINING_THREADS"
echo "Chain ID: $CHAIN_ID"

# Create data directory if it doesn't exist
mkdir -p "$DATA_DIR"

# Create a basic genesis.toml file if it doesn't exist
if [ ! -f "$DATA_DIR/genesis.toml" ]; then
  cat > "$DATA_DIR/genesis.toml" << EOF
# Genesis Block Configuration
chain_id = 1
timestamp = $(date +%s)
version = 1
nonce = 0
difficulty = 1
initial_difficulty = 1
initial_accounts = {}

# Network bootstrap nodes
[[bootstrap_nodes]]
address = "$BOOTSTRAP_IP:$BOOTSTRAP_PORT"
EOF

  # Add custom bootstrap node to genesis if provided
  if [ -n "$CUSTOM_BOOTSTRAP_IP" ]; then
    cat >> "$DATA_DIR/genesis.toml" << EOF

[[bootstrap_nodes]]
address = "$CUSTOM_BOOTSTRAP_IP:$LISTEN_PORT"
EOF
  fi

  echo "Created genesis.toml in $DATA_DIR"
fi

echo "Configuration complete. You can now start the node with:"
echo "./target/release/vibecoin --config config.toml"
