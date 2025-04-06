# VibeCoin Transaction Pool (Mempool)

## Overview

The Transaction Pool (Mempool) is a thread-safe, prioritized pool that:

- Accepts verified transactions from users/network
- Queues them for block producers (miners/validators)
- Prevents spam and invalid transaction propagation

## Architecture

```
     ┌──────────────────────────────┐
     │        Network Module        │
     └────────────┬────────────────┘
                  ↓
       Incoming transactions
                  ↓
          ┌───────────────┐
          │   Mempool     │
          └─────┬─────────┘
                ↓
      ┌─────────────────────┐
      │ Consensus / Miner   │
      └─────────────────────┘
```

## Components

### Transaction Record

The `TransactionRecord` structure represents a transaction in the mempool:

- `tx_id`: Unique transaction ID (hash)
- `sender`: Sender address
- `recipient`: Recipient address
- `value`: Transaction value
- `gas_price`: Fee per gas unit
- `gas_limit`: Maximum gas units
- `nonce`: Account nonce (prevents replay attacks)
- `timestamp`: Creation timestamp
- `signature`: Transaction signature
- `data`: Optional data payload

### Mempool

The `Mempool` class provides the core functionality:

- `insert`: Add a transaction to the mempool
- `get_pending`: Get transactions for inclusion in a block
- `remove`: Remove transactions from the mempool
- `mark_included`: Mark transactions as included in a block
- `cleanup_expired`: Remove expired transactions
- `prune_included`: Clean up old included transactions

## Validation Rules

The mempool validates transactions before accepting them:

- Signature is valid (using Crypto module)
- Nonce is equal to or one above current account state
- Sender balance >= value + gas cost (using StateStore)
- Transaction is not already in the mempool
- Sender has not exceeded the maximum number of transactions
- Transaction is not expired

## Prioritization

Transactions are prioritized based on:

1. Gas price (higher gas price = higher priority)
2. Timestamp (older transactions = higher priority)

This ensures that transactions offering higher fees are processed first, while still maintaining fairness for older transactions.

## Configuration

The mempool can be configured with:

- `max_size`: Maximum number of transactions in the mempool
- `max_per_sender`: Maximum number of transactions per sender
- `max_transaction_age`: Maximum age of transactions in seconds
- `max_included_size`: Maximum size of the included transaction set

## Thread Safety

The mempool is designed to be thread-safe using:

- `DashMap` for concurrent access to transaction maps
- `RwLock` for the priority queue
- Atomic operations for counters

## Usage Example

```rust
// Create a mempool
let mempool = Mempool::new();

// Create a transaction
let tx = TransactionRecord::new(
    sender,
    recipient,
    value,
    gas_price,
    gas_limit,
    nonce,
    data,
);

// Insert the transaction
mempool.insert(tx).await?;

// Get pending transactions for a block
let transactions = mempool.get_pending(max_count).await;

// Mark transactions as included in a block
mempool.mark_included(&tx_ids, block_height).await;
```
