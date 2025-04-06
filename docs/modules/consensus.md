# Consensus Module

## Overview

The Consensus Module implements Vibecoin's hybrid consensus mechanism, combining Proof-of-Work (PoW) with Proof of History (PoH). This approach provides the security benefits of PoW while achieving the high throughput and low latency of PoH.

## Components

### Proof of Work (PoW)

The PoW component requires miners to solve a computational puzzle to create new blocks. This provides:

- Security against Sybil attacks
- Fair distribution of new coins
- Protection against 51% attacks

#### Implementation Details

- Uses SHA-256 for block hash verification
- Parallel mining with Rayon for efficient nonce search
- Difficulty adjusts dynamically based on block times
- Target block time: 15 seconds (configurable)
- Implements longest chain rule with cumulative work comparison

### Proof of History (PoH)

The PoH component creates a historical record of events, providing:

- A verifiable passage of time
- Ordering of transactions without global consensus
- Reduced communication overhead

#### Implementation Details

- Uses a sequential hash function (SHA-256)
- Creates a verifiable sequence of timestamps
- Generates approximately 400,000 hashes per second (configurable)
- Supports recording events within the hash chain
- Includes verification of hash sequence integrity

### Hybrid Consensus

The hybrid approach combines PoW and PoH:

1. PoH leader generates a sequence of hashes
2. Transactions are inserted into the sequence
3. PoW miners solve puzzles to validate blocks
4. Network reaches consensus on the canonical chain

## Interfaces

### Public Functions

```rust
/// Initialize the consensus module
pub fn init(config: ConsensusConfig) -> Result<Consensus, Error>;

/// Process a new block
pub fn process_block(block: Block) -> Result<(), Error>;

/// Generate a new block
pub fn generate_block(transactions: Vec<Transaction>) -> Result<Block, Error>;

/// Verify a block
pub fn verify_block(block: Block) -> Result<bool, Error>;
```

### Configuration

```rust
pub struct ConsensusConfig {
    /// Target block time in seconds
    pub target_block_time: u64,

    /// Initial difficulty
    pub initial_difficulty: u64,

    /// Number of blocks to consider for difficulty adjustment
    pub difficulty_adjustment_window: u64,

    /// PoH tick rate (hashes per second)
    pub poh_tick_rate: u64,

    /// Maximum number of transactions per block
    pub max_transactions_per_block: usize,

    /// Whether to enable mining
    pub enable_mining: bool,

    /// Number of mining threads to use
    pub mining_threads: usize,
}
```

## Performance Characteristics

- **Throughput**: Up to 50,000 transactions per second
- **Latency**: Block confirmation in 1-2 seconds
- **Finality**: Probabilistic finality after 6 confirmations

## Security Considerations

- **51% Attack Resistance**: Requires controlling both PoW hash power and PoH generation
- **Long-Range Attack Protection**: Uses checkpoints and social consensus
- **Nothing-at-Stake Problem**: Mitigated by PoW component

## Future Improvements

- **Sharding**: Divide the network into shards for parallel processing
- **VDF Integration**: Replace PoH with Verifiable Delay Functions
- **Stake-Weighted PoH**: Incorporate stake-weighting into PoH leader selection
