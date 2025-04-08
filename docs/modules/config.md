# Configuration Module

## Overview

The Configuration Module is responsible for managing VibeCoin node settings and configuration. It provides a flexible and extensible way to configure all aspects of a VibeCoin node, including network settings, consensus parameters, storage options, and more.

## Components

### Configuration Manager

The Configuration Manager is the central component of the Configuration Module. It is responsible for:

- Loading configuration from various sources (files, environment variables, command-line arguments)
- Validating configuration values
- Providing access to configuration values to other modules
- Handling configuration changes at runtime (when supported)

### Configuration Sources

The Configuration Module supports multiple configuration sources with the following precedence (highest to lowest):

1. **Command-Line Arguments**: Provided when starting the node
2. **Environment Variables**: Set in the node's environment
3. **Configuration File**: TOML file specified with the `--config` flag
4. **Default Values**: Hardcoded defaults for all settings

### Configuration Schema

The configuration schema defines the structure and validation rules for all configuration options. It includes:

- **Option Names**: The names of configuration options
- **Data Types**: The expected data types for each option
- **Validation Rules**: Rules for validating option values
- **Default Values**: Default values for options
- **Documentation**: Description and usage information for each option

## Interfaces

### Public API

```rust
/// Configuration manager for VibeCoin node
pub struct Config {
    /// Node configuration
    pub node: NodeConfig,
    /// Network configuration
    pub network: NetworkConfig,
    /// Consensus configuration
    pub consensus: ConsensusConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Mempool configuration
    pub mempool: MempoolConfig,
}

impl Config {
    /// Load configuration from a file
    pub fn from_file(path: &Path) -> Result<Self, ConfigError>;

    /// Load configuration from command-line arguments
    pub fn from_args() -> Result<Self, ConfigError>;

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError>;

    /// Load configuration from multiple sources with proper precedence
    pub fn load() -> Result<Self, ConfigError>;

    /// Save configuration to a file
    pub fn save(&self, path: &Path) -> Result<(), ConfigError>;

    /// Generate a default configuration
    pub fn default() -> Self;

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError>;
}
```

### Configuration Structures

```rust
/// Node configuration
pub struct NodeConfig {
    /// Node name
    pub name: String,
    /// Data directory
    pub data_dir: String,
    /// API settings
    pub api_host: String,
    pub api_port: u16,
    /// Metrics settings
    pub metrics_enabled: bool,
    pub metrics_host: String,
    pub metrics_port: u16,
}

/// Network configuration
pub struct NetworkConfig {
    /// P2P settings
    pub listen_addr: String,
    pub listen_port: u16,
    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,
    /// Maximum number of peers
    pub max_peers: u32,
    /// Peer reputation settings
    pub min_reputation: i32,
}

/// Consensus configuration
pub struct ConsensusConfig {
    /// Chain ID
    pub chain_id: u64,
    /// Block time target in seconds
    pub target_block_time: u64,
    /// Initial mining difficulty
    pub initial_difficulty: u64,
    /// Enable mining
    pub enable_mining: bool,
    /// Number of mining threads
    pub mining_threads: u32,
}

/// Storage configuration
pub struct StorageConfig {
    /// Database path
    pub db_path: String,
    /// Cache size in MB
    pub cache_size: u64,
    /// Enable compression
    pub compression: bool,
    /// Pruning mode (archive, 1000, 10000)
    pub pruning: String,
}

/// Mempool configuration
pub struct MempoolConfig {
    /// Maximum number of transactions
    pub max_transactions: u32,
    /// Maximum number of transactions per sender
    pub max_transactions_per_sender: u32,
    /// Minimum gas price
    pub min_gas_price: u64,
    /// Transaction timeout in seconds
    pub transaction_timeout: u64,
}
```

## Implementation Details

### Configuration File Format

VibeCoin uses TOML for configuration files. Here's an example configuration file:

```toml
[node]
# Node identity
name = "my-vibecoin-node"
# Data directory
data_dir = "./data/vibecoin"
# API settings
api_host = "127.0.0.1"
api_port = 8545
# Metrics settings
metrics_enabled = true
metrics_host = "127.0.0.1"
metrics_port = 9100

[network]
# P2P settings
listen_addr = "0.0.0.0"
listen_port = 30334
# Bootstrap nodes
bootstrap_nodes = [
    "/dns4/bootstrap1.vibecoin.network/tcp/30334/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp",
    "/dns4/bootstrap2.vibecoin.network/tcp/30334/p2p/12D3KooWHdiAxVd8uMQR1hGWXccidmfCwLqcMpGwR6QcTP6QRMq9"
]
# Maximum number of peers
max_peers = 50
# Peer reputation settings
min_reputation = -100

[consensus]
# Chain ID
chain_id = 1
# Block time target in seconds
target_block_time = 10
# Initial mining difficulty
initial_difficulty = 10000
# Enable mining
enable_mining = true
# Number of mining threads
mining_threads = 4

[storage]
# Database path
db_path = "./data/vibecoin/db"
# Cache size in MB
cache_size = 512
# Enable compression
compression = true
# Pruning mode (archive, 1000, 10000)
pruning = "archive"

[mempool]
# Maximum number of transactions
max_transactions = 10000
# Maximum number of transactions per sender
max_transactions_per_sender = 100
# Minimum gas price
min_gas_price = 1
# Transaction timeout in seconds
transaction_timeout = 3600
```

### Environment Variables

Configuration options can also be set using environment variables. The naming convention is:

```
VIBECOIN_<SECTION>_<OPTION>
```

For example:
- `VIBECOIN_NODE_NAME` sets the node name
- `VIBECOIN_NETWORK_LISTEN_PORT` sets the network listen port
- `VIBECOIN_CONSENSUS_ENABLE_MINING` sets whether mining is enabled

### Command-Line Arguments

Command-line arguments use the following format:

```
--<section>-<option> <value>
```

For example:
- `--node-name my-node` sets the node name
- `--network-listen-port 30334` sets the network listen port
- `--consensus-enable-mining true` sets whether mining is enabled

Some common options have shortcuts:
- `-c, --config <FILE>` specifies the configuration file
- `-n, --network <NETWORK>` specifies the network (dev, testnet, mainnet)
- `-d, --data-dir <DIRECTORY>` specifies the data directory

### Configuration Validation

The Configuration Module validates all configuration options to ensure they are valid and consistent. Validation includes:

- **Type Checking**: Ensuring values have the correct data type
- **Range Checking**: Ensuring numeric values are within valid ranges
- **Format Checking**: Ensuring strings have the correct format (e.g., addresses, URLs)
- **Consistency Checking**: Ensuring related options are consistent with each other

### Network-Specific Configurations

VibeCoin supports different network configurations:

- **Development**: For local development and testing
- **Testnet**: For testing in a shared environment
- **Mainnet**: For production use

Each network has its own default configuration values, which can be overridden by the user.

## Configuration

The Configuration Module itself has minimal configuration options:

- **Config File Path**: Path to the configuration file
- **Config Format**: Format of the configuration file (currently only TOML is supported)
- **Auto-Save**: Whether to automatically save changes to the configuration file

## Performance Considerations

The Configuration Module is designed to be lightweight and efficient:

- **Lazy Loading**: Configuration is loaded only when needed
- **Caching**: Configuration values are cached to avoid repeated parsing
- **Minimal Dependencies**: The module has minimal dependencies to reduce overhead

## Security Considerations

The Configuration Module implements several security measures:

- **Validation**: All configuration values are validated to prevent security issues
- **Sanitization**: Sensitive configuration values are sanitized in logs
- **Access Control**: Configuration access is controlled to prevent unauthorized changes
- **Secure Defaults**: Default values are chosen with security in mind

## Future Improvements

Planned improvements for the Configuration Module include:

- **Dynamic Reconfiguration**: Support for changing configuration at runtime
- **Configuration Profiles**: Support for different configuration profiles
- **Configuration Versioning**: Support for versioning configuration files
- **Configuration UI**: A web-based UI for configuring the node
- **Configuration Encryption**: Support for encrypting sensitive configuration values
- **Configuration Synchronization**: Support for synchronizing configuration across nodes
- **Configuration Validation API**: An API for validating configuration without starting the node

## Example Usage

### Loading Configuration

```rust
use vibecoin::config::Config;

// Load configuration from all sources
let config = Config::load().expect("Failed to load configuration");

// Access configuration values
let node_name = &config.node.name;
let listen_port = config.network.listen_port;
let enable_mining = config.consensus.enable_mining;

println!("Node name: {}", node_name);
println!("Listen port: {}", listen_port);
println!("Mining enabled: {}", enable_mining);
```

### Generating Default Configuration

```rust
use vibecoin::config::Config;
use std::path::Path;

// Generate default configuration
let config = Config::default();

// Save configuration to a file
config.save(Path::new("config.toml")).expect("Failed to save configuration");
```

### Validating Configuration

```rust
use vibecoin::config::Config;
use std::path::Path;

// Load configuration from a file
let config = Config::from_file(Path::new("config.toml")).expect("Failed to load configuration");

// Validate configuration
match config.validate() {
    Ok(_) => println!("Configuration is valid"),
    Err(e) => println!("Configuration is invalid: {}", e),
}
```
