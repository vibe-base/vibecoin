# Vibecoin Architecture

## Overview

Vibecoin is a next-generation blockchain that combines Proof-of-Work (PoW) with Solana-style Proof of History (PoH) to achieve a balance of speed, fairness, and security. The architecture is designed to power scalable, efficient, and decentralized applications.

## Core Components

### Consensus Mechanism

Vibecoin implements a hybrid consensus mechanism:

- **Proof of Work (PoW)**: Provides security and fair distribution of tokens
- **Proof of History (PoH)**: Enables high throughput and low latency by creating a historical record of events

Our implementation includes:
- Dynamic difficulty adjustment based on block times
- Parallel mining using Rayon for efficient nonce search
- PoH generator that creates a verifiable sequence of hashes
- Fork choice rules for selecting the canonical chain

### Block Structure

Each block in the Vibecoin blockchain contains:

- Block height and hash
- Previous block hash for chain linking
- Timestamp for chronological ordering
- Transaction list (as transaction IDs)
- State root hash for account state verification
- PoW solution (nonce and hash meeting difficulty target)
- PoH sequence data for verifiable timestamps

### Gas-Based Architecture

Vibecoin uses a gas-based system similar to Ethereum:

- Each operation costs a certain amount of gas
- Users specify gas price and gas limit
- Miners prioritize transactions with higher gas prices

## Network Architecture

The Vibecoin network consists of several node types:

- **Validator Nodes**: Participate in consensus and validate transactions
- **Full Nodes**: Store the complete blockchain and relay transactions
- **Light Clients**: Connect to full nodes to interact with the blockchain

## Security Features

- **51% Attack Resistance**: The hybrid PoW/PoH mechanism increases the cost of attacks
- **Sybil Attack Protection**: PoW component prevents Sybil attacks
- **Double-Spending Prevention**: Transactions are confirmed through consensus

## Scalability Solutions

- **Sharding**: Divides the network into smaller partitions to process transactions in parallel
- **Layer 2 Solutions**: Supports off-chain scaling solutions
- **Optimized Transaction Processing**: Efficient transaction validation and propagation

## File Size Limitations

To maintain code quality and readability, Vibecoin enforces a strict policy that no Rust file should exceed 1000 lines. This is enforced through a pre-commit Git hook.
