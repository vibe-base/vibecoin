<div align="center">

# 🌊 Vibecoin

**A next-generation blockchain platform for the decentralized future**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/vibecoin/vibecoin)

</div>

## 🚀 Overview

Vibecoin is a revolutionary blockchain platform that combines the security of Proof-of-Work with the speed of Solana-style Proof of History. Built on a gas-based architecture similar to Ethereum, and enhanced with Sui-style parallel processing, Vibecoin enables highly scalable, efficient, and truly decentralized applications. Its ability to execute transactions in parallel dramatically increases throughput and reduces latency, making Vibecoin ideal for next-generation dApps and real-world adoption.

protocol level mev protection

## ✨ Key Features

- **Hybrid Consensus**: Combines PoW security with PoH speed
- **High Throughput**: Process up to 50,000 transactions per second
- **Low Latency**: Achieve finality in seconds, not minutes
- **Gas-Based Economy**: Efficient resource allocation and fair pricing
- **Object-Centric Architecture**: Sui-style object ID system for flexible data modeling
- **Smart Contract Support**: Build powerful decentralized applications
- **Developer-Friendly**: Comprehensive SDKs and documentation
- **Parallel Execution**: Sui-style architecture enables simultaneous transaction processing, boosting scalability and minimizing congestion.


## 🛠️ Getting Started

### Prerequisites

- Rust 1.70 or higher
- Cargo (comes with Rust)
- 8GB+ RAM
- 100GB+ free disk space
- Linux, macOS, or Windows with WSL

### Installation

```bash
# Clone the repository
git clone https://github.com/vibecoin/vibecoin.git
cd vibecoin

# Build from source
cargo build --release

# Run tests to verify installation
cargo test
```

### Running a Node

```bash
# Generate a default configuration
./target/release/vibecoin-config --generate --output config.toml

# Generate a genesis block
./target/release/vibecoin-genesis --generate --output genesis.toml

# Start the node
./target/release/vibecoin --config config.toml --genesis genesis.toml
```

### Using Docker

```bash
# Start a local development network
./scripts/bootstrap_devnet.sh
docker-compose -f docker-compose.dev.yml up -d

# Start a testnet node
./scripts/bootstrap_testnet.sh
docker-compose -f docker-compose.testnet.yml up -d
```

### Configuration Options

| Option | Description | Default |
|--------|-------------|--------|
| `-c, --config` | Configuration file | None |
| `-g, --genesis` | Genesis file | None |
| `-n, --network` | Network to connect to (dev, testnet, mainnet) | None |
| `-d, --data-dir` | Directory for blockchain data | `./data/vibecoin` |
| `-b, --bootstrap` | Bootstrap nodes (comma-separated) | None |
| `--enable-mining` | Enable mining | `true` |
| `--mining-threads` | Mining threads | `4` |
| `--api-port` | Port for JSON-RPC server | `8545` |
| `--api-host` | Host for JSON-RPC server | `127.0.0.1` |
| `--listen-port` | Port for P2P communication | `30334` |
| `--listen-addr` | Address for P2P communication | `0.0.0.0` |

## 📚 Documentation

Comprehensive documentation is available in the [docs](./docs) directory:

### System Documentation
- [System Architecture](./docs/architecture/system_architecture.md)
- [Architecture Overview](./docs/architecture.md)

### Developer Documentation
- [Developer Guide](./docs/development.md)
- [Developer Workflow](./docs/guides/developer_workflow.md)
- [Installation Guide](./docs/guides/installation_guide.md)
- [Code Standards](./docs/code_standards.md)
- [File Size Policy](./docs/file_size_policy.md)

### API Documentation
- [API Reference](./docs/api/README.md)

### Module Documentation
- [Module Overview](./docs/modules/README.md)
- [Storage Module](./docs/modules/storage.md)
- [Object System Module](./docs/modules/object_system.md)
- [Consensus Module](./docs/modules/consensus.md)
- [Network Module](./docs/modules/network.md)
- [Cryptography Module](./docs/modules/crypto.md)
- [Mempool Module](./docs/modules/mempool.md)
- [Configuration Module](./docs/modules/config.md)

### Storage Documentation
- [Storage System](./docs/storage_system.md)
- [Storage Schema](./docs/storage_schema.md)
- [Storage Integration](./docs/storage_integration.md)
- [Storage Usage](./docs/storage_usage.md)

## 🧩 Core Modules

### Deployment and Configuration

VibeCoin provides comprehensive deployment and configuration tools:

- **Configuration System**: TOML-based configuration for all node settings
- **Genesis Generator**: Tool for creating custom genesis blocks
- **Docker Support**: Containerized deployment for easy setup
- **Network Scripts**: Bootstrap scripts for different network types
- **Monitoring**: Prometheus metrics and Grafana dashboards

### Storage Module

The [Storage Module](./storage/README.md) provides persistent storage for the blockchain using RocksDB:

- **Key-Value Store**: Abstraction over RocksDB for data persistence
- **Block Store**: Storage and retrieval of blockchain blocks
- **Transaction Store**: Management of transaction records
- **Account State**: Handling of account balances and state
- **Object Store**: Sui-style object ID system for object-centric architecture
- **PoH Store**: Storage for Proof of History entries

### Cryptography Module

The [Cryptography Module](./crypto/README.md) implements essential cryptographic primitives:

- **Key Generation**: Ed25519 keypair generation and management
- **Digital Signatures**: Secure transaction signing and verification
- **Hashing**: SHA-256 and double SHA-256 for various blockchain components
- **Address Derivation**: Generation of account addresses from public keys

### Network Module

The [Network Module](./network/README.md) provides peer-to-peer communication:

- **Peer Discovery**: Finding and connecting to other nodes
- **Message Broadcasting**: Distributing blocks and transactions
- **Connection Management**: Handling peer connections and reconnections
- **Protocol Implementation**: Framed message protocol with serialization

### Consensus Module

The [Consensus Module](./consensus/README.md) implements our hybrid PoW/PoH mechanism:

- **Proof of Work**: Mining algorithm with difficulty adjustment
- **Proof of History**: Sequential hash chain for verifiable timestamps
- **Block Validation**: Verification of blocks against consensus rules
- **Fork Choice**: Rules for selecting the canonical blockchain
- **Mining Engine**: Block production and transaction inclusion

### Mempool Module

The [Mempool Module](./mempool/README.md) provides a thread-safe transaction pool:

- **Transaction Validation**: Verification of transaction validity
- **Prioritization**: Ordering transactions by gas price and timestamp
- **Spam Prevention**: Limiting transactions per sender
- **Memory Management**: Handling transaction lifecycle and expiration
- **Thread Safety**: Concurrent access for high throughput

## 🔍 Vibecode

Vibecoin was developed through vibecoding, a process guided by flow, intuition, and mastery. Vibecode is the natural output when experienced software engineers build with rhythm, intention, and deep technical skill. The result is a blockchain that embodies:

- Clean, performant architecture

- Battle-tested security practices

- Thoughtful design for long-term maintainability

- Code that feels as good as it runs

The use of Vobecode ensures that Vibecoin maintains the highest standards of code quality, with strict enforcement of best practices such as our 1000-line limit for Rust files.

## 🤝 Contributing

We welcome contributions from the community! Please see our [Contributing Guide](./CONTRIBUTING.md) for details on how to get involved.

## 📄 License

Vibecoin is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

