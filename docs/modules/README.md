# Vibecoin Modules Documentation

This directory contains documentation for the various modules that make up the Vibecoin blockchain.

## Core Modules

### Consensus Module

The [Consensus Module](consensus.md) implements the hybrid Proof-of-Work and Proof-of-History consensus mechanism.

### Network Module

The [Network Module](network.md) handles peer-to-peer communication and network synchronization.

### Storage Module

The [Storage Module](storage.md) manages blockchain data storage and state management.

### Cryptography Module

The [Cryptography Module](crypto.md) provides cryptographic primitives and functions.

### Transaction Module

The [Transaction Module](transaction.md) handles transaction validation, execution, and mempool management.

## Additional Modules

### Smart Contract Module

The [Smart Contract Module](smart_contract.md) provides support for executing smart contracts.

### Wallet Module

The [Wallet Module](wallet.md) implements wallet functionality for key management and transaction signing.

### API Module

The [API Module](api.md) exposes interfaces for interacting with the blockchain.

### CLI Module

The [CLI Module](cli.md) provides a command-line interface for interacting with the blockchain.

## Module Dependencies

```
Consensus → Network → Storage
    ↑          ↑         ↑
    |          |         |
Transaction ← Crypto → Wallet
    ↑          ↑         ↑
    |          |         |
Smart Contract → API ← CLI
```

## Module Design Principles

- **Modularity**: Each module has a clear responsibility
- **Testability**: Modules are designed to be easily testable
- **Performance**: Modules are optimized for performance
- **Security**: Modules are designed with security in mind
