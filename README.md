<div align="center">

# 🌊 Vibecoin

**A next-generation blockchain platform for the decentralized future**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/vibecoin/vibecoin)

</div>

## 🚀 Overview

Vibecoin is a revolutionary blockchain platform that combines the security of Proof-of-Work with the speed of Solana-style Proof of History. Built with a gas-based architecture, Vibecoin enables highly scalable, efficient, and truly decentralized applications for the future.

Developed using **Vobe Code**, our proprietary development framework, Vibecoin represents the cutting edge of blockchain technology, offering unparalleled performance without compromising on security or decentralization.

## ✨ Key Features

- **Hybrid Consensus**: Combines PoW security with PoH speed
- **High Throughput**: Process up to 50,000 transactions per second
- **Low Latency**: Achieve finality in seconds, not minutes
- **Gas-Based Economy**: Efficient resource allocation and fair pricing
- **Smart Contract Support**: Build powerful decentralized applications
- **Developer-Friendly**: Comprehensive SDKs and documentation

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
# Initialize a new node
./target/release/vibecoin init --network mainnet --data-dir ~/.vibecoin

# Start the node
./target/release/vibecoin start --rpc-port 8899 --p2p-port 8900
```

### Configuration Options

| Option | Description | Default |
|--------|-------------|--------|
| `--network` | Network to connect to (mainnet, testnet, devnet) | `mainnet` |
| `--data-dir` | Directory for blockchain data | `~/.vibecoin` |
| `--rpc-port` | Port for JSON-RPC server | `8899` |
| `--p2p-port` | Port for P2P communication | `8900` |
| `--log-level` | Logging verbosity (error, warn, info, debug, trace) | `info` |

## 📚 Documentation

Comprehensive documentation is available in the [docs](./docs) directory:

- [Architecture Overview](./docs/architecture.md)
- [Developer Guide](./docs/development.md)
- [API Reference](./docs/api/README.md)
- [Module Documentation](./docs/modules/README.md)

## 🧩 Core Modules

### Storage Module

The [Storage Module](./storage/README.md) provides persistent storage for the blockchain using RocksDB:

- **Key-Value Store**: Abstraction over RocksDB for data persistence
- **Block Store**: Storage and retrieval of blockchain blocks
- **Transaction Store**: Management of transaction records
- **Account State**: Handling of account balances and state
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

## 🔍 Vobe Code

Vibecoin was developed using **Vobe Code**, our proprietary development framework that enables rapid blockchain development with a focus on performance, security, and maintainability. Vobe Code provides:

- Advanced static analysis tools
- Performance optimization frameworks
- Security-first development practices
- Automated testing and verification

The use of Vobe Code ensures that Vibecoin maintains the highest standards of code quality, with strict enforcement of best practices such as our 1000-line limit for Rust files.

## 🤝 Contributing

We welcome contributions from the community! Please see our [Contributing Guide](./CONTRIBUTING.md) for details on how to get involved.

## 📄 License

Vibecoin is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

