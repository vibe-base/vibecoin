# Vibecoin Storage Module

## Overview

The storage module provides persistent storage for the Vibecoin blockchain using RocksDB as the backend. It implements storage for blocks, transactions, account states, and Proof of History (PoH) entries.

## Components

### KVStore (kv_store.rs)

A key-value store abstraction with a RocksDB implementation:

- `KVStore` trait defines the interface for storage operations
- `RocksDBStore` implements the KVStore trait using RocksDB

### BlockStore (block_store.rs)

Stores and retrieves blockchain blocks:

- `Block` structure represents a block in the blockchain
- `BlockStore` provides methods to store and retrieve blocks by height or hash

### TxStore (tx_store.rs)

Manages transaction records:

- `TransactionRecord` structure represents a transaction
- `TxStore` provides methods to store and retrieve transactions by ID, sender, recipient, or block

### StateStore (state_store.rs)

Manages account states:

- `AccountState` structure represents the state of an account
- `StateStore` provides methods to get and set account states, update balances, and manage nonces

### PoHStore (poh_store.rs)

Stores Proof of History entries:

- `PoHEntry` structure represents a PoH entry
- `PoHStore` provides methods to append and retrieve PoH entries

## Usage

```rust
use vibecoin::storage::{RocksDBStore, BlockStore, TxStore, StateStore, PoHStore};

// Initialize the KV store
let kv_store = RocksDBStore::new("path/to/db");

// Initialize specialized stores
let block_store = BlockStore::new(&kv_store);
let tx_store = TxStore::new(&kv_store);
let state_store = StateStore::new(&kv_store);
let poh_store = PoHStore::new(&kv_store);

// Use the stores to interact with the blockchain data
```

## Testing

Each component includes unit tests that can be run with:

```bash
cargo test
```

Integration tests are available in the `tests` directory and demonstrate how the different stores work together.

## Design Considerations

- **Modularity**: Each store is independent but can work together
- **Indexing**: Data is indexed in multiple ways for efficient retrieval
- **Serialization**: Uses bincode for efficient binary serialization
- **Error Handling**: Provides clear error messages for common failure cases
