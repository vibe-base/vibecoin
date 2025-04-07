# VibeCoin Storage Usage Guide

This document provides examples and best practices for using the VibeCoin storage module.

## Table of Contents

1. [Initializing Storage](#initializing-storage)
2. [Working with Blocks](#working-with-blocks)
3. [Working with Transactions](#working-with-transactions)
4. [Working with Account State](#working-with-account-state)
5. [Atomic Batch Operations](#atomic-batch-operations)
6. [Error Handling](#error-handling)
7. [Performance Considerations](#performance-considerations)

## Initializing Storage

To initialize the storage system, you need to create a RocksDB instance and then initialize the various stores:

```rust
use std::sync::Arc;
use vibecoin::storage::{RocksDBStore, BlockStore, TxStore, StateStore, BatchOperationManager};

// Create the RocksDB store
let kv_store = Arc::new(RocksDBStore::new("./data/vibecoin").unwrap());

// Create the specialized stores
let block_store = Arc::new(BlockStore::new(&kv_store));
let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

// Create the batch operation manager for atomic operations
let batch_manager = BatchOperationManager::new(
    kv_store.clone(),
    block_store.clone(),
    tx_store.clone(),
    state_store.clone(),
);
```

## Working with Blocks

### Storing a Block

```rust
use vibecoin::storage::Block;

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
match block_store.put_block(&block) {
    Ok(_) => println!("Block stored successfully"),
    Err(e) => eprintln!("Failed to store block: {}", e),
}
```

### Retrieving a Block

```rust
// By height
match block_store.get_block_by_height(1) {
    Ok(Some(block)) => println!("Found block: {:?}", block),
    Ok(None) => println!("Block not found"),
    Err(e) => eprintln!("Error retrieving block: {}", e),
}

// By hash
let hash = [1; 32];
match block_store.get_block_by_hash(&hash) {
    Ok(Some(block)) => println!("Found block: {:?}", block),
    Ok(None) => println!("Block not found"),
    Err(e) => eprintln!("Error retrieving block: {}", e),
}

// Get the latest block
match block_store.get_latest_block() {
    Ok(Some(block)) => println!("Latest block: {:?}", block),
    Ok(None) => println!("No blocks found"),
    Err(e) => eprintln!("Error retrieving latest block: {}", e),
}
```

## Working with Transactions

### Storing a Transaction

```rust
use vibecoin::storage::{TransactionRecord, TransactionStatus};

// Create a transaction
let tx = TransactionRecord {
    tx_id: [1; 32],
    sender: [2; 32],
    recipient: [3; 32],
    value: 100,
    gas_price: 1,
    gas_limit: 21000,
    gas_used: 21000,
    nonce: 0,
    timestamp: 12345,
    block_height: 1,
    data: None,
    status: TransactionStatus::Confirmed,
};

// Store the transaction
match tx_store.put_transaction(&tx) {
    Ok(_) => println!("Transaction stored successfully"),
    Err(e) => eprintln!("Failed to store transaction: {}", e),
}
```

### Retrieving Transactions

```rust
// By transaction ID
let tx_id = [1; 32];
match tx_store.get_transaction(&tx_id) {
    Ok(Some(tx)) => println!("Found transaction: {:?}", tx),
    Ok(None) => println!("Transaction not found"),
    Err(e) => eprintln!("Error retrieving transaction: {}", e),
}

// By sender
let sender = [2; 32];
match tx_store.get_transactions_by_sender(&sender) {
    Ok(txs) => println!("Found {} transactions from sender", txs.len()),
    Err(e) => eprintln!("Error retrieving transactions: {}", e),
}

// By recipient
let recipient = [3; 32];
match tx_store.get_transactions_by_recipient(&recipient) {
    Ok(txs) => println!("Found {} transactions to recipient", txs.len()),
    Err(e) => eprintln!("Error retrieving transactions: {}", e),
}

// By block
match tx_store.get_transactions_by_block(1) {
    Ok(txs) => println!("Found {} transactions in block 1", txs.len()),
    Err(e) => eprintln!("Error retrieving transactions: {}", e),
}
```

### Updating Transaction Status

```rust
let tx_id = [1; 32];
match tx_store.update_transaction_status(&tx_id, TransactionStatus::Confirmed) {
    Ok(_) => println!("Transaction status updated successfully"),
    Err(e) => eprintln!("Failed to update transaction status: {}", e),
}
```

## Working with Account State

### Creating an Account

```rust
use vibecoin::storage::{AccountState, AccountType};

let address = [1; 32];
let state = AccountState {
    balance: 1000,
    nonce: 0,
    account_type: AccountType::User,
};

match state_store.create_account(&address, 1000, AccountType::User) {
    Ok(_) => println!("Account created successfully"),
    Err(e) => eprintln!("Failed to create account: {}", e),
}
```

### Retrieving Account State

```rust
let address = [1; 32];
match state_store.get_account_state(&address) {
    Ok(Some(state)) => println!("Account balance: {}", state.balance),
    Ok(None) => println!("Account not found"),
    Err(e) => eprintln!("Error retrieving account: {}", e),
}
```

### Updating Account Balance

```rust
let address = [1; 32];
match state_store.update_balance(&address, 1500) {
    Ok(_) => println!("Balance updated successfully"),
    Err(e) => eprintln!("Failed to update balance: {}", e),
}
```

### Calculating State Root

```rust
match state_store.calculate_state_root(1, 12345) {
    Ok(root) => println!("State root: {:?}", root),
    Err(e) => eprintln!("Failed to calculate state root: {}", e),
}
```

## Atomic Batch Operations

For operations that need to be atomic (all succeed or all fail), use the BatchOperationManager:

```rust
// Commit a block with all its transactions and state changes atomically
match batch_manager.commit_block(&block, &transactions, &state_changes) {
    Ok(_) => println!("Block committed successfully"),
    Err(e) => eprintln!("Failed to commit block: {}", e),
}

// Rollback a block if needed
match batch_manager.rollback_block(1) {
    Ok(_) => println!("Block rolled back successfully"),
    Err(e) => eprintln!("Failed to rollback block: {}", e),
}
```

## Error Handling

All storage operations return a `Result` type with appropriate error variants. It's important to handle these errors properly:

```rust
match block_store.get_block_by_height(1) {
    Ok(Some(block)) => {
        // Process the block
    },
    Ok(None) => {
        // Block not found, handle this case
        println!("Block not found");
    },
    Err(e) => {
        // Handle different error types
        match e {
            BlockStoreError::KVStoreError(kv_err) => {
                eprintln!("Database error: {}", kv_err);
                // Maybe try to reconnect to the database
            },
            BlockStoreError::SerializationError(ser_err) => {
                eprintln!("Serialization error: {}", ser_err);
                // Maybe the block format has changed
            },
            _ => {
                eprintln!("Unknown error: {}", e);
            }
        }
    }
}
```

## Performance Considerations

### Batch Operations

For bulk operations, always use batch operations instead of individual puts:

```rust
// Bad: Individual puts
for tx in transactions {
    tx_store.put_transaction(&tx).unwrap();
}

// Good: Atomic batch operation
batch_manager.commit_block(&block, &transactions, &state_changes).unwrap();
```

### Caching

The stores implement caching for frequently accessed data. You can control the cache size:

```rust
// Create RocksDB with custom options
let mut opts = Options::default();
opts.set_max_open_files(1000);
opts.set_keep_log_file_num(10);
opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
opts.set_max_write_buffer_number(3);
opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB

let kv_store = Arc::new(RocksDBStore::with_options("./data/vibecoin", opts).unwrap());
```

### Flushing

For critical operations, you can force a flush to disk:

```rust
match tx_store.flush() {
    Ok(_) => println!("Data flushed to disk"),
    Err(e) => eprintln!("Failed to flush data: {}", e),
}
```
