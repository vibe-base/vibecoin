# Mempool Module

## Overview

The Mempool Module provides a thread-safe, prioritized transaction pool for the VibeCoin blockchain. It serves as the interface between the network and consensus layers, accepting transactions from users and the network, validating them, and providing them to block producers for inclusion in blocks.

## Components

### Transaction Record

The transaction record structure represents a transaction in the mempool:

```rust
pub struct TransactionRecord {
    pub tx_id: Hash,
    pub sender: Address,
    pub recipient: Address,
    pub value: u64,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub nonce: u64,
    pub timestamp: u64,
    pub signature: VibeSignature,
    pub data: Option<Vec<u8>>,
}
```

### Mempool

The mempool class provides the core functionality:

```rust
pub struct Mempool {
    // Add a transaction to the mempool
    pub async fn insert(&self, tx: TransactionRecord) -> Result<(), MempoolError>;
    
    // Get transactions for inclusion in a block
    pub async fn get_pending(&self, max_count: usize) -> Vec<TransactionRecord>;
    
    // Remove transactions from the mempool
    pub async fn remove(&self, tx_ids: &[Hash]);
    
    // Mark transactions as included in a block
    pub async fn mark_included(&self, tx_ids: &[Hash], block_height: u64);
    
    // Clean up expired transactions
    pub async fn cleanup_expired(&self);
}
```

## Transaction Lifecycle

1. **Submission**: Transactions are submitted by users or received from the network
2. **Validation**: The mempool validates transactions against the current state
3. **Prioritization**: Valid transactions are prioritized based on gas price and timestamp
4. **Selection**: Block producers select transactions from the mempool for inclusion in blocks
5. **Inclusion**: Once included in a block, transactions are removed from the pending pool
6. **Expiration**: Transactions that remain in the mempool too long are removed

## Validation Rules

The mempool validates transactions before accepting them:

- **Signature Verification**: Ensures the transaction is properly signed by the sender
- **Nonce Validation**: Prevents replay attacks and ensures transaction ordering
- **Balance Check**: Ensures the sender has sufficient funds for the transaction
- **Duplicate Check**: Prevents the same transaction from being included multiple times
- **Spam Prevention**: Limits the number of transactions per sender

## Prioritization

Transactions are prioritized based on:

1. **Gas Price**: Higher gas price transactions are prioritized
2. **Timestamp**: Older transactions are prioritized over newer ones with the same gas price

This ensures that transactions offering higher fees are processed first, while still maintaining fairness for older transactions.

## Configuration

The mempool can be configured with:

```rust
pub struct MempoolConfig {
    pub max_size: usize,
    pub max_per_sender: usize,
    pub max_transaction_age: u64,
    pub max_included_size: usize,
}
```

## Thread Safety

The mempool is designed to be thread-safe using:

- **DashMap**: For concurrent access to transaction maps
- **RwLock**: For the priority queue
- **Atomic Operations**: For counters

## Integration with Other Modules

- **Network Module**: Receives transactions from the network
- **Consensus Module**: Provides transactions for block production
- **Storage Module**: Validates transactions against the current state
- **Cryptography Module**: Verifies transaction signatures
