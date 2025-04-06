# Storage Module

## Overview

The Storage Module provides persistent storage for the VibeCoin blockchain data. It uses RocksDB as the underlying key-value store and implements specialized stores for different types of blockchain data.

## Components

### Key-Value Store

The key-value store provides a generic interface for data storage:

- **KVStore Trait**: Defines the interface for storage operations
- **RocksDBStore**: Implements the KVStore trait using RocksDB
- **Operations**: put, get, delete, scan_prefix

### Block Store

The block store manages blockchain blocks:

- **Block Structure**: Represents a block in the blockchain
- **Storage**: Indexed by both height and hash for efficient retrieval
- **Operations**: put_block, get_block_by_height, get_block_by_hash, get_latest_height

### Transaction Store

The transaction store manages transaction records:

- **TransactionRecord Structure**: Represents a transaction
- **Indexing**: By ID, sender, recipient, and block
- **Operations**: put_transaction, get_transaction, get_transactions_by_sender, get_transactions_by_recipient, get_transactions_by_block

### State Store

The state store manages account states:

- **AccountState Structure**: Represents the state of an account
- **Operations**: get_account_state, set_account_state, update_balance, increment_nonce, create_account, set_contract_data

### PoH Store

The PoH store manages Proof of History entries:

- **PoHEntry Structure**: Represents a PoH entry
- **Operations**: append_entry, get_entry, get_entry_by_hash, get_latest_sequence, get_entry_range

## Implementation Details

```rust
// Key-Value Store
pub trait KVStore {
    fn put(&self, key: &[u8], value: &[u8]);
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn delete(&self, key: &[u8]);
    fn scan_prefix(&self, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)>;
}

// Block Structure
pub struct Block {
    pub height: u64,
    pub hash: [u8; 32],
    pub prev_hash: [u8; 32],
    pub timestamp: u64,
    pub transactions: Vec<[u8; 32]>, // tx hashes
    pub state_root: [u8; 32],
}

// Account State
pub struct AccountState {
    pub balance: u64,
    pub nonce: u64,
    pub contract_data: Option<Vec<u8>>,
}

// PoH Entry
pub struct PoHEntry {
    pub hash: [u8; 32],
    pub sequence: u64,
    pub timestamp: u64,
}
```

## Data Organization

The storage module uses prefixed keys to organize different types of data:

- **Blocks by Height**: `block:height:{height}`
- **Blocks by Hash**: `block:hash:{hash}`
- **Transactions by ID**: `tx:id:{tx_id}`
- **Transactions by Sender**: `tx:sender:{sender}:{tx_id}`
- **Transactions by Recipient**: `tx:recipient:{recipient}:{tx_id}`
- **Transactions by Block**: `tx:block:{block_height}:{tx_id}`
- **Account States**: `state:addr:{address}`
- **PoH Entries by Sequence**: `poh:seq:{sequence}`
- **PoH Entries by Hash**: `poh:hash:{hash}`

## Performance Considerations

- **Efficient Indexing**: Multiple indexes for fast lookups
- **Batch Operations**: Support for batched writes for better performance
- **Serialization**: Uses bincode for efficient binary serialization
- **Caching**: Future improvement to add in-memory caching

## Security Considerations

- **Data Integrity**: Ensures data consistency through atomic operations
- **Backup and Recovery**: Supports RocksDB's backup mechanisms
- **Error Handling**: Robust error handling for storage operations

## Future Improvements

- **Pruning**: Implement state pruning for older data
- **Sharding**: Support for sharded storage across multiple nodes
- **Merkle Trees**: Implement Merkle trees for efficient state verification
- **Compression**: Add data compression for reduced storage requirements
