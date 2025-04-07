# VibeCoin Consensus Module

## Overview

The consensus module implements VibeCoin's hybrid Proof-of-Work (PoW) and Proof-of-History (PoH) consensus mechanism. This approach combines the security and fair distribution of PoW with the high throughput and verifiable timestamps of PoH.

The module is responsible for:

- Mining new blocks with a PoW nonce
- Embedding verifiable PoH sequences for time-ordering
- Validating blocks and applying them to the chain
- Resolving forks based on cumulative work

## Architecture

```
                  ┌─────────────────┐
                  │  ConsensusEngine│
                  └────────┬────────┘
                           │
                           ▼
          ┌────────────────────────────────┐
          │        Block Validation        │
          └────────┬─────────────┬─────────┘
                   │             │
        ┌──────────┴──────────┐ │ ┌─────────────────┐
        │    Block Mining     │ │ │   Fork Choice   │
        └┬─────┬──────┬──────┬┘ │ └────────┬────────┘
         │     │      │      │  │          │
         ▼     ▼      ▼      ▼  │          ▼
┌──────────┐ ┌───┐ ┌─────┐ ┌───┐│  ┌───────────────┐
│PoW Mining│ │PoH│ │State│ │Diff││  │ Chain Selection│
│          │ │Gen │ │Root │ │Adj ││  │               │
└────┬─────┘ └─┬─┘ └──┬──┘ └─┬─┘│  └───────┬───────┘
     │         │      │      │  │          │
     └─────────┴──────┴──────┴──┴──────────┘
                           │
                           ▼
                  ┌─────────────────┐
                  │  Storage Module │
                  └─────────────────┘
```

## Components

### Proof of Work (pow/)

- **miner.rs**: Implements the mining algorithm for finding valid blocks
- **difficulty.rs**: Handles difficulty adjustment based on block times

### Proof of History (poh/)

- **generator.rs**: Generates the sequential hash chain for PoH
- **verifier.rs**: Verifies the correctness of PoH entries

### Validation (validation/)

- **block_validator.rs**: Validates blocks against consensus rules
- **transaction_validator.rs**: Validates transactions
- **fork_choice.rs**: Implements the fork choice rule for selecting the canonical chain

### Mining (mining/)

- **block_producer.rs**: Creates new block templates and mines blocks
- **mempool.rs**: Manages the pool of pending transactions

### Core Components

- **types.rs**: Defines core consensus types like Target and BlockTemplate
- **config.rs**: Configuration for the consensus module
- **engine.rs**: Main consensus engine that ties everything together

## Usage

```rust
use vibecoin::consensus::{start_consensus, ConsensusConfig};
use vibecoin::storage::{BlockStore, TxStore, StateStore};
use vibecoin::network::types::message::NetMessage;
use std::sync::Arc;
use tokio::sync::mpsc;

// Create the stores
let block_store = Arc::new(BlockStore::new(&kv_store));
let tx_store = Arc::new(TxStore::new(&kv_store));
let state_store = Arc::new(StateStore::new(&kv_store));

// Create a network channel
let (network_tx, _) = mpsc::channel(100);

// Create a config
let config = ConsensusConfig::default()
    .with_target_block_time(15)
    .with_initial_difficulty(100)
    .with_mining_enabled(true);

// Start the consensus engine
let engine = start_consensus(
    config,
    block_store,
    tx_store,
    state_store,
    network_tx,
).await;
```

## Consensus Flow

1. **Block Production**:
   - Create a block template with pending transactions
   - Generate PoH entries to establish a verifiable timestamp
   - Mine the block using PoW to find a valid nonce
   - Broadcast the block to the network

2. **Block Validation**:
   - Verify the PoW nonce meets the difficulty target
   - Verify the PoH sequence is valid
   - Validate all transactions in the block
   - Apply the transactions to the state

3. **Fork Resolution**:
   - Use the longest chain rule with most accumulated work
   - Resolve forks by comparing cumulative work
   - Reorganize the chain if necessary

## Design Considerations

- **Modularity**: Each component has a clear responsibility
- **Testability**: Comprehensive unit tests for each component
- **Performance**: Optimized mining with parallel nonce search
- **Security**: Robust validation of all consensus rules

## Block Structure

Each block contains:

- `height`: Block height
- `prev_hash`: Hash of the previous block
- `state_root`: Merkle root of the state trie
- `tx_root`: Merkle root of the transactions
- `timestamp`: Block timestamp
- `poh_seq`: PoH sequence number
- `poh_hash`: PoH hash
- `nonce`: PoW nonce
- `difficulty`: Block difficulty
- `total_difficulty`: Cumulative difficulty of the chain

## Future Improvements

- Sharding for parallel transaction processing
- Stake-weighted PoH leader selection
- Improved fork choice rules with finality gadgets
- More sophisticated difficulty adjustment algorithm
- Parallel transaction validation
- Memory pool prioritization
- ASIC-resistant PoW algorithm
- Optimized PoH verification
- Checkpointing for faster sync
- Light client support
