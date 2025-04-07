# VibeCoin Storage Schema

This document defines the storage schema used by VibeCoin for persistent blockchain data storage using RocksDB.

## Key Format

All keys in the database follow a consistent format to enable efficient lookups and range queries:
- Keys are prefixed with a type identifier followed by a colon
- This allows for logical separation of different data types within a single RocksDB instance

## Schema Definition

### Block Storage
- **Key Format**: `block:<block_height>`
- **Value**: Serialized Block struct using bincode
- **Example**: `block:1000` → [serialized block data]
- **Secondary Index**: `block_hash:<block_hash>` → `<block_height>` (for hash-based lookups)

### Transaction Storage
- **Key Format**: `tx:<tx_hash>`
- **Value**: Serialized TransactionRecord struct using bincode
- **Example**: `tx:a1b2c3...` → [serialized transaction data]
- **Secondary Index**: `tx_block:<block_height>:<tx_index>` → `<tx_hash>` (for block-based lookups)

### Account State Storage
- **Key Format**: `state:<account_address>`
- **Value**: Serialized AccountState struct using bincode
- **Example**: `state:d4e5f6...` → [serialized account state]

### State Root Storage
- **Key Format**: `state_root:<block_height>`
- **Value**: Serialized StateRoot struct using bincode
- **Example**: `state_root:1000` → [serialized state root]

### Merkle Patricia Trie Nodes
- **Key Format**: `mpt_node:<node_hash>`
- **Value**: Serialized Node struct using bincode
- **Example**: `mpt_node:g7h8i9...` → [serialized MPT node]

### Metadata Storage
- **Key Format**: `meta:<metadata_key>`
- **Value**: Serialized metadata value using bincode
- **Example**: `meta:latest_block_height` → [serialized u64 value]

## Serialization

All data is serialized using bincode for:
- Compact binary representation
- Fast serialization/deserialization
- Type safety

## Atomic Operations

All related operations (e.g., adding a block, its transactions, and updating state) are performed atomically using RocksDB's WriteBatch functionality to ensure data consistency.

## Optimizations

- Short key prefixes to minimize storage overhead
- Secondary indices for efficient lookups
- Careful key design to enable range scans for related data
