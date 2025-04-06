# Vibecoin Development Guide

## Development Environment Setup

### Prerequisites

- Rust (latest stable version)
- Cargo (comes with Rust)
- Git
- A Unix-like environment (Linux, macOS, or WSL for Windows)

### Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/your-username/vibecoin.git
   cd vibecoin
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run tests:
   ```bash
   cargo test
   ```

## Project Structure

The Vibecoin project follows a modular architecture:

- `src/` - Main source code directory
  - `lib.rs` - Library entry point
  - `main.rs` - CLI entry point

- `consensus/` - Consensus mechanism implementation
  - `pow/` - Proof of Work implementation
  - `poh/` - Proof of History implementation
  - `validation/` - Block and transaction validation
  - `mining/` - Block production and mempool
  - `engine.rs` - Main consensus engine

- `network/` - Networking and P2P communication
  - `types/` - Network message types
  - `peer/` - Peer connection management
  - `service/` - Network service and message routing
  - `codec/` - Message serialization and framing

- `storage/` - Blockchain storage and state management
  - `kv_store.rs` - Key-value store abstraction
  - `block_store.rs` - Block storage
  - `tx_store.rs` - Transaction storage
  - `state_store.rs` - Account state storage
  - `poh_store.rs` - Proof of History storage

- `crypto/` - Cryptographic primitives
  - `keys.rs` - Key generation and management
  - `hash.rs` - Hashing functions
  - `signer.rs` - Digital signatures

- `docs/` - Project documentation
  - `modules/` - Module-specific documentation
  - `api/` - API documentation

## Contribution Guidelines

### Code Style

- Follow the Rust style guide
- Use meaningful variable and function names
- Write comprehensive comments
- Keep files under 1000 lines (enforced by pre-commit hook)

### Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Write or update tests
5. Submit a pull request

### Commit Messages

Follow the conventional commits format:

```
type(scope): description

[optional body]

[optional footer]
```

Types: feat, fix, docs, style, refactor, test, chore

### Code Review

All code must be reviewed by at least one maintainer before being merged.

## Testing

- Write unit tests for all new functionality
- Integration tests for complex features
- Benchmarks for performance-critical code

## Documentation

- Document all public APIs
- Update relevant documentation when making changes
- Use Rust doc comments (`///`) for API documentation

## Continuous Integration

The project uses GitHub Actions for CI/CD:

- Automated testing on multiple platforms
- Code coverage reporting
- Linting and formatting checks
