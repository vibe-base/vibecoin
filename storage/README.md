# VibeCoin Storage Module

## Overview

The VibeCoin Storage Module provides a comprehensive, production-ready storage layer for the blockchain. It is responsible for persisting all blockchain data, including blocks, transactions, account states, and Proof of History entries. Built on top of RocksDB, it offers high performance, durability, and efficient data retrieval.

## Architecture

The storage module is built on top of RocksDB, a high-performance key-value store. It provides a modular design with separate components for different types of data:

```
                  ┌─────────────────┐
                  │   Application   │
                  └────────┬────────┘
                           │
                           ▼
          ┌────────────────────────────────┐
          │        Storage Module          │
          └────────┬─────────────┬─────────┘
                   │             │
        ┌──────────┴──────────┐ │ ┌─────────────────┐
        │    Data Stores      │ │ │   State Trie    │
        └┬─────┬──────┬──────┬┘ │ └────────┬────────┘
         │     │      │      │  │          │
         ▼     ▼      ▼      ▼  │          ▼
┌──────────┐ ┌───┐ ┌─────┐ ┌───┐ ┌───────┐│  ┌───────────────┐
│BlockStore│ │Tx │ │State│ │PoH │ │Object ││  │ Merkle Proofs │
│          │ │Store│ │Store│ │Store│ │Store ││  │               │
└────┬─────┘ └─┬─┘ └──┬──┘ └─┬─┘ └───┬───┘│  └───────┬───────┘
     │         │      │      │      │    │          │
     └─────────┴──────┴──────┴──────┴────┴──────────┘
                           │
                           ▼
                  ┌─────────────────┐
                  │     RocksDB     │
                  └─────────────────┘
```

## Components

### KVStore (kv_store.rs)

The KVStore trait defines the interface for key-value storage operations. The RocksDBStore implementation provides a high-performance, persistent key-value store using RocksDB.

Key features:
- Atomic batch operations
- Prefix scanning
- Error handling
- Compression
- Optimized RocksDB configuration

### BlockStore (block_store.rs)

The BlockStore is responsible for storing and retrieving blockchain blocks. It indexes blocks by both height and hash for efficient retrieval.

Key features:
- Block storage and retrieval by height or hash
- Latest block tracking
- Block range retrieval
- Efficient serialization

### TxStore (tx_store.rs)

The TxStore manages transaction records. It provides multiple indexes for efficient transaction lookup by ID, sender, recipient, or block.

Key features:
- Transaction storage and retrieval by ID
- Transaction lookup by sender or recipient
- Transaction lookup by block
- Transaction status tracking
- Nonce-based transaction lookup

### StateStore (state_store.rs)

The StateStore maintains the current state of all accounts in the blockchain. It supports account balances, nonces, and smart contract storage.

Key features:
- Account state storage and retrieval
- Balance transfers
- Nonce management
- Smart contract storage
- State root calculation
- Account caching for performance

### ObjectStore (object_store.rs)

The ObjectStore implements a Sui-style object ID system, extending the account-based storage model into a more object-centric architecture.

Key features:
- Object creation, retrieval, and deletion
- Different ownership models (Address-owned, Shared, Immutable)
- Object versioning for tracking mutations
- Efficient querying by owner and type
- Object metadata management

### PoHStore (poh_store.rs)

The PoHStore persists Proof of History entries, which are used for consensus.

Key features:
- PoH entry storage and retrieval
- Sequence verification
- Efficient lookup

### Merkle Patricia Trie (trie.rs)

The Merkle Patricia Trie provides a cryptographically verifiable data structure for the state. It enables efficient verification of account states without downloading the entire state.

Key features:
- Efficient key-value storage
- Cryptographic verification
- Merkle proofs
- State root calculation

## Data Schema

The storage module uses a prefix-based schema for keys in RocksDB:

| Prefix | Description | Example |
|--------|-------------|--------|
| `block:height:` | Block by height | `block:height:1000` |
| `block:hash:` | Block by hash | `block:hash:abcdef...` |
| `tx:id:` | Transaction by ID | `tx:id:abcdef...` |
| `tx:sender:` | Transaction by sender | `tx:sender:abcdef...` |
| `tx:recipient:` | Transaction by recipient | `tx:recipient:abcdef...` |
| `tx:block:` | Transaction by block | `tx:block:1000:abcdef...` |
| `state:account:` | Account state | `state:account:abcdef...` |
| `state:root:` | State root | `state:root:latest` |
| `objects:` | Object by ID | `objects:abcdef...` |
| `objects_by_owner:` | Objects by owner | `objects_by_owner:abcdef...` |
| `objects_by_type:` | Objects by type | `objects_by_type:Coin:abcdef...` |
| `poh:seq:` | PoH entry by sequence | `poh:seq:1000` |

## Usage Examples

### Storing and Retrieving Blocks

```rust
// Create a block store
let kv_store = RocksDBStore::new(path);
let block_store = BlockStore::new(&kv_store);

// Store a block
let block = Block {
    height: 1,
    hash: [1; 32],
    prev_hash: [0; 32],
    timestamp: 12345,
    transactions: vec![[2; 32], [3; 32]],
    state_root: [4; 32],
    nonce: 42,
    poh_seq: 100,
};
block_store.put_block(&block).unwrap();

// Retrieve by height
let retrieved = block_store.get_block_by_height(1).unwrap();

// Retrieve by hash
let retrieved = block_store.get_block_by_hash(&[1; 32]).unwrap();

// Get the latest block
let latest = block_store.get_latest_block().unwrap();
```

### Managing Account States

```rust
// Create a state store
let kv_store = RocksDBStore::new(path);
let state_store = StateStore::new(&kv_store);

// Create an account
let address = [1; 32];
state_store.create_account(&address, 1000, AccountType::User).unwrap();

// Transfer balance
let recipient = [2; 32];
state_store.transfer_balance(&address, &recipient, 500, 1).unwrap();

// Update account state
state_store.update_balance(&address, 2000).unwrap();
state_store.increment_nonce(&address).unwrap();

// Set contract code
let contract_code = vec![1, 2, 3, 4];
state_store.set_contract_code(&address, contract_code).unwrap();

// Set contract storage
let storage_key = b"key";
let storage_value = b"value".to_vec();
state_store.set_storage_value(&address, storage_key, storage_value).unwrap();

// Calculate state root
let state_root = state_store.calculate_state_root(1, 12345).unwrap();
```

### Working with Transactions

```rust
// Create a transaction store
let kv_store = RocksDBStore::new(path);
let tx_store = TxStore::new(&kv_store);

// Store a transaction
let tx = TransactionRecord {
    tx_id: [1; 32],
    sender: [2; 32],
    recipient: [3; 32],
    value: 100,
    gas_price: 5,
    gas_limit: 21000,
    gas_used: 10,
    nonce: 1,
    timestamp: 12345,
    block_height: 5,
    data: None,
    status: TransactionStatus::Included,
};
tx_store.put_transaction(&tx).unwrap();

// Retrieve by ID
let retrieved = tx_store.get_transaction(&[1; 32]).unwrap();

// Get transactions by sender
let sender_txs = tx_store.get_transactions_by_sender(&[2; 32]);

// Get transactions by block
let block_txs = tx_store.get_transactions_by_block(5);

// Update transaction status
tx_store.update_transaction_status(&[1; 32], TransactionStatus::Confirmed).unwrap();
```

### Working with Objects

```rust
// Create an object store
let kv_store = RocksDBStore::new(path);
let object_store = ObjectStore::new(&kv_store);

// Create an object
let owner = Ownership::Address([1; 32]);
let object = object_store.create_object(
    owner,
    "Coin".to_string(),
    vec![1, 2, 3, 4],
    None,
).unwrap();

// Retrieve an object by ID
let retrieved = object_store.get_object(&object.id).unwrap();

// Get objects by owner
let owner_objects = object_store.get_objects_by_owner(&[1; 32]).unwrap();

// Get objects by type
let coins = object_store.get_objects_by_type("Coin").unwrap();

// Update an object
let updated = object_store.update_object(&object.id, |obj| {
    obj.update_contents(vec![5, 6, 7, 8], obj.updated_at + 1, 1)
}).unwrap();

// Transfer ownership
let recipient = [2; 32];
let transferred = object_store.update_object(&object.id, |obj| {
    obj.transfer_ownership(Ownership::Address(recipient), obj.updated_at + 1, 1)
}).unwrap();

// Delete an object
object_store.delete_object(&object.id).unwrap();
```

## Performance Considerations

The storage module is optimized for performance:

- **RocksDB Configuration**: Optimized for blockchain workloads with appropriate buffer sizes, compression, and compaction settings.
- **Batch Operations**: Uses atomic batch operations for consistency and performance.
- **Caching**: Implements in-memory caching for frequently accessed data.
- **Efficient Serialization**: Uses bincode for fast and compact serialization.
- **Indexing**: Multiple indexes for efficient data retrieval.
- **Concurrency**: Thread-safe design for concurrent access.

## Resilience and Durability

The storage module ensures data integrity and durability:

- **Atomic Writes**: All related writes are performed atomically using batch operations.
- **Crash Recovery**: RocksDB provides WAL (Write-Ahead Logging) for crash recovery.
- **Checksumming**: Data integrity is verified using checksums.
- **Compaction**: Regular compaction to reclaim space and optimize performance.
- **Backups**: Support for creating consistent backups.

## Testing

Each component includes comprehensive unit tests that can be run with:

```bash
cargo test
```

Integration tests are available in the `tests` directory and demonstrate how the different stores work together.

## Future Improvements

- **Pruning**: Implement state pruning to reduce storage requirements.
- **Sharding**: Shard the database for better scalability.
- **Compression**: Add more advanced compression options.
- **Encryption**: Add support for encrypted storage.
- **Remote Storage**: Support for remote storage backends.
- **Snapshot/Rollback**: Enhanced support for state snapshots and rollbacks.
