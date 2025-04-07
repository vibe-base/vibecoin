# VibeCoin Storage System

This document provides a comprehensive overview of the VibeCoin storage system, including its architecture, components, and usage patterns.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Key Components](#key-components)
3. [Storage Schema](#storage-schema)
4. [State Management](#state-management)
5. [Atomic Operations](#atomic-operations)
6. [Merkle Patricia Trie](#merkle-patricia-trie)
7. [Performance Considerations](#performance-considerations)
8. [Error Handling](#error-handling)
9. [Testing](#testing)
10. [Examples](#examples)

## Architecture Overview

The VibeCoin storage system is designed to provide a robust, efficient, and scalable persistence layer for the blockchain. It is built on top of RocksDB, a high-performance key-value store, and provides abstractions for storing and retrieving blockchain data such as blocks, transactions, and account states.

The storage system follows a layered architecture:

1. **Low-level Storage**: RocksDB integration with error handling and schema management
2. **Domain-specific Stores**: Specialized stores for blocks, transactions, and account states
3. **State Management**: Efficient state management with caching and atomic operations
4. **Batch Operations**: Support for atomic batch operations across multiple stores
5. **Merkle Patricia Trie**: Support for state verification and light clients

## Key Components

### KVStore

The `KVStore` trait defines the interface for the low-level key-value store operations:

```rust
pub trait KVStore: Send + Sync {
    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError>;
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError>;
    fn delete(&self, key: &[u8]) -> Result<(), KVStoreError>;
    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError>;
    fn write_batch(&self, operations: Vec<WriteBatchOperation>) -> Result<(), KVStoreError>;
    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError>;
    fn flush(&self) -> Result<(), KVStoreError>;
}
```

### RocksDBStore

The `RocksDBStore` implements the `KVStore` trait using RocksDB as the underlying storage engine:

```rust
pub struct RocksDBStore {
    db: DB,
}
```

### BlockStore

The `BlockStore` provides methods for storing and retrieving blocks:

```rust
pub struct BlockStore<'a> {
    store: &'a dyn KVStore,
    latest_height: std::sync::RwLock<Option<u64>>,
}
```

### TxStore

The `TxStore` provides methods for storing and retrieving transactions:

```rust
pub struct TxStore<'a> {
    store: &'a dyn KVStore,
}
```

### StateStore

The `StateStore` provides methods for storing and retrieving account states:

```rust
pub struct StateStore<'a> {
    store: &'a dyn KVStore,
}
```

### StateManager

The `StateManager` provides advanced state management capabilities with caching and atomic operations:

```rust
pub struct StateManager<'a> {
    store: &'a dyn KVStore,
    state_root: RwLock<Option<StateRoot>>,
    account_cache: DashMap<String, AccountState>,
    storage_cache: DashMap<String, DashMap<String, Vec<u8>>>,
    max_account_cache_size: usize,
    max_storage_cache_size: usize,
    trie: RwLock<Option<MerklePatriciaTrie>>,
}
```

### BatchOperationManager

The `BatchOperationManager` provides support for atomic batch operations across multiple stores:

```rust
pub struct BatchOperationManager {
    store: Arc<dyn KVStore>,
    block_store: Arc<BlockStore>,
    tx_store: Arc<TxStore>,
    state_store: Arc<StateStore>,
}
```

### RocksDBManager

The `RocksDBManager` provides utilities for managing the RocksDB database:

```rust
pub struct RocksDBManager<'a> {
    store: &'a dyn KVStore,
}
```

## Storage Schema

The storage schema defines the structure of keys and values in the key-value store. The `Schema` struct provides methods for creating and parsing keys:

### Block Keys

- `block:<height>` - Block by height
- `block_hash:<hash>` - Block hash to height mapping
- `meta:latest_block_height` - Latest block height

### Transaction Keys

- `tx:<hash>` - Transaction by hash
- `tx_block:<height>:<tx_hash>` - Transactions by block
- `tx_sender:<sender>:<tx_hash>` - Transactions by sender
- `tx_recipient:<recipient>:<tx_hash>` - Transactions by recipient
- `tx_sender_nonce:<sender>` - Latest nonce for a sender

### Account State Keys

- `state:<address>` - Account state
- `state_storage:<address>:<slot>` - Contract storage
- `state_root:<height>` - State root by block height
- `meta:latest_state_root` - Latest state root

### PoH Keys

- `poh:<seq>` - PoH entry by sequence number
- `meta:latest_poh_seq` - Latest PoH sequence number

### MPT Keys

- `mpt_node:<hash>` - MPT node by hash

## State Management

The state management system provides efficient access to account states and contract storage with caching and atomic operations.

### Account State

The `AccountState` struct represents the state of an account:

```rust
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub account_type: AccountType,
}
```

### State Root

The `StateRoot` struct represents the root of the state trie:

```rust
pub struct StateRoot {
    pub root_hash: Hash,
    pub block_height: u64,
    pub timestamp: u64,
}
```

### State Caching

The `StateManager` implements a two-level caching system:

1. **Account Cache**: Caches account states to reduce database reads
2. **Storage Cache**: Caches contract storage values to reduce database reads

### State Updates

The `StateManager` provides methods for updating account states and contract storage:

```rust
pub fn update_balance(&self, address: &Hash, new_balance: u64) -> Result<(), StateStoreError>;
pub fn update_nonce(&self, address: &Hash, new_nonce: u64) -> Result<(), StateStoreError>;
pub fn set_storage_value(&self, address: &Hash, key: &[u8], value: Vec<u8>) -> Result<(), StateStoreError>;
```

### State Root Calculation

The `StateManager` provides a method for calculating the state root:

```rust
pub fn calculate_state_root(&self, block_height: u64, timestamp: u64) -> Result<StateRoot, StateStoreError>;
```

## Atomic Operations

The storage system supports atomic operations to ensure data consistency:

### Write Batch

The `WriteBatchOperation` enum represents a single operation in a batch:

```rust
pub enum WriteBatchOperation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}
```

### Batch Manager

The `BatchOperationManager` provides methods for atomic batch operations:

```rust
pub fn commit_block(
    &self,
    block: &Block,
    transactions: &[TransactionRecord],
    state_changes: &[(Hash, AccountState)],
) -> Result<(), BatchOperationError>;

pub fn rollback_block(&self, height: u64) -> Result<(), BatchOperationError>;
```

## Merkle Patricia Trie

The storage system includes a Merkle Patricia Trie implementation for state verification:

### MPT Structure

The `MerklePatriciaTrie` struct represents a Merkle Patricia Trie:

```rust
pub struct MerklePatriciaTrie {
    root: Option<Node>,
    nodes: HashMap<Hash, Node>,
}
```

### Node Types

The `Node` enum represents different types of nodes in the trie:

```rust
pub enum Node {
    Empty,
    Leaf { key: Vec<u8>, value: Vec<u8> },
    Extension { prefix: Vec<u8>, next: Hash },
    Branch { children: [Option<Hash>; 16], value: Option<Vec<u8>> },
}
```

### Proof Generation

The `MerklePatriciaTrie` provides methods for generating and verifying proofs:

```rust
pub fn generate_proof(&self, key: &[u8]) -> Proof;
pub fn verify_proof(proof: &Proof, root_hash: &Hash) -> bool;
```

## Performance Considerations

### Caching

The storage system implements caching at multiple levels:

1. **RocksDB Cache**: RocksDB's internal block cache and write buffer
2. **Account Cache**: In-memory cache for account states
3. **Storage Cache**: In-memory cache for contract storage values
4. **State Root Cache**: Caches the calculated state root

### Batch Operations

The storage system uses batch operations to improve performance:

1. **Write Batching**: Multiple writes are batched into a single atomic operation
2. **Read Batching**: Multiple reads are batched when possible

### Prefix Scanning

The storage system uses prefix scanning for efficient range queries:

```rust
fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError>;
```

### Database Management

The `RocksDBManager` provides utilities for managing the database:

```rust
pub fn compact(&self) -> Result<(), KVStoreError>;
pub fn backup(&self, backup_path: &str) -> Result<(), KVStoreError>;
pub fn restore(&self, backup_path: &str) -> Result<(), KVStoreError>;
pub fn prune(&self, keep_blocks: u64) -> Result<(), KVStoreError>;
```

## Error Handling

The storage system uses custom error types for comprehensive error handling:

### KVStoreError

```rust
pub enum KVStoreError {
    IoError(Error),
    RocksDBError(String),
    SerializationError(String),
    DeserializationError(String),
    KeyNotFound(String),
    InvalidDataFormat(String),
    BatchOperationFailed(String),
    DatabaseAlreadyExists(String),
    DatabaseNotFound(String),
    ColumnFamilyNotFound(String),
    Other(String),
}
```

### StateStoreError

```rust
pub enum StateStoreError {
    KVStoreError(KVStoreError),
    SerializationError(String),
    DeserializationError(String),
    AccountNotFound(String),
    AccountAlreadyExists(String),
    InsufficientBalance(String),
    BalanceOverflow(String),
    InvalidNonce(String),
    InvalidAccountType(String),
    InvalidStateRoot(String),
    Other(String),
}
```

### BatchOperationError

```rust
pub enum BatchOperationError {
    BlockStoreError(BlockStoreError),
    TxStoreError(TxStoreError),
    StateStoreError(StateStoreError),
    KVStoreError(KVStoreError),
    Other(String),
}
```

## Testing

The storage system includes comprehensive tests:

### Unit Tests

Each component includes unit tests to verify its functionality:

```rust
#[test]
fn test_rocksdb_integration() {
    // ...
}

#[test]
fn test_block_store() {
    // ...
}

#[test]
fn test_transaction_store() {
    // ...
}

#[test]
fn test_state_store() {
    // ...
}

#[test]
fn test_state_manager() {
    // ...
}

#[test]
fn test_batch_operations() {
    // ...
}

#[test]
fn test_rocksdb_schema() {
    // ...
}

#[test]
fn test_rocksdb_manager() {
    // ...
}
```

### Integration Tests

The storage system includes integration tests to verify the interaction between components:

```rust
#[test]
fn test_apply_block() {
    // ...
}
```

### Memory Store

The storage system includes a memory-based implementation of the `KVStore` trait for testing:

```rust
pub struct MemoryStore {
    data: std::sync::RwLock<std::collections::HashMap<Vec<u8>, Vec<u8>>>,
}
```

## Examples

### Initializing the Storage System

```rust
// Create the RocksDB store
let kv_store = Arc::new(RocksDBStore::new("./data/vibecoin").unwrap());

// Create the specialized stores
let block_store = Arc::new(BlockStore::new(&kv_store));
let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

// Create the state manager
let state_manager = StateManager::new(kv_store.as_ref());

// Create the batch operation manager
let batch_manager = Arc::new(BatchOperationManager::new(
    kv_store.clone(),
    block_store.clone(),
    tx_store.clone(),
    state_store.clone(),
));
```

### Storing and Retrieving a Block

```rust
// Create a block
let block = Block {
    height: 1,
    hash: [1; 32],
    prev_hash: [0; 32],
    timestamp: 12345,
    transactions: vec![[2; 32], [3; 32]],
    state_root: [4; 32],
    tx_root: [5; 32],
    nonce: 42,
    poh_seq: 100,
    poh_hash: [6; 32],
    difficulty: 1000,
    total_difficulty: 1000,
};

// Store the block
block_store.put_block(&block).unwrap();

// Retrieve the block by height
let retrieved_block = block_store.get_block_by_height(1).unwrap().unwrap();
```

### Managing Account State

```rust
// Create an account
let address = [1; 32];
state_store.create_account(&address, 1000, AccountType::User).unwrap();

// Update the account balance
state_store.update_balance(&address, 2000).unwrap();

// Update the account nonce
state_store.update_nonce(&address, 1).unwrap();

// Calculate the state root
let state_root = state_store.calculate_state_root(1, 12345).unwrap();
```

### Using the State Manager

```rust
// Create an account
let address = [1; 32];
state_manager.create_account(&address, 1000, AccountType::User).unwrap();

// Set a storage value
let key = [2; 32];
let value = vec![3; 32];
state_manager.set_storage_value(&address, &key, value.clone()).unwrap();

// Get a storage value
let retrieved_value = state_manager.get_storage_value(&address, &key).unwrap().unwrap();

// Apply a block
state_manager.apply_block(&block).unwrap();

// Generate a proof
let proof = state_manager.generate_account_proof(&address).unwrap();

// Verify a proof
let is_valid = StateManager::verify_account_proof(&proof, &state_root.root_hash).unwrap();
```

### Using Batch Operations

```rust
// Create a block
let block = Block { /* ... */ };

// Create transactions
let tx1 = TransactionRecord { /* ... */ };
let tx2 = TransactionRecord { /* ... */ };

// Create state changes
let state1 = AccountState { /* ... */ };
let state2 = AccountState { /* ... */ };

let state_changes = vec![
    ([10; 32], state1),
    ([11; 32], state2),
];

// Commit the block
batch_manager.commit_block(&block, &[tx1, tx2], &state_changes).unwrap();

// Rollback the block
batch_manager.rollback_block(1).unwrap();
```

### Managing the Database

```rust
// Create the RocksDB manager
let manager = RocksDBManager::new(&store);

// Get database statistics
let stats = manager.get_stats().unwrap();
println!("Block count: {}", stats.block_count);
println!("Transaction count: {}", stats.transaction_count);
println!("Account count: {}", stats.account_count);

// Compact the database
manager.compact().unwrap();

// Backup the database
manager.backup("./backup").unwrap();

// Restore the database
manager.restore("./backup").unwrap();

// Prune old data
manager.prune(1000).unwrap();
```
