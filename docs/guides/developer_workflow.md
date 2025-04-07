# VibeCoin Developer Workflow Guide

This guide provides a comprehensive overview of the development workflow for contributing to the VibeCoin project. It covers everything from setting up your development environment to submitting pull requests and getting your code merged.

## Table of Contents

1. [Development Environment Setup](#development-environment-setup)
2. [Project Structure](#project-structure)
3. [Development Workflow](#development-workflow)
4. [Coding Standards](#coding-standards)
5. [Testing](#testing)
6. [Documentation](#documentation)
7. [Code Review Process](#code-review-process)
8. [Continuous Integration](#continuous-integration)
9. [Release Process](#release-process)
10. [Troubleshooting](#troubleshooting)

## Development Environment Setup

### Prerequisites

- **Rust**: VibeCoin is written in Rust. Install the latest stable version using [rustup](https://rustup.rs/).
- **Git**: Version control system for tracking changes.
- **IDE/Editor**: We recommend [VS Code](https://code.visualstudio.com/) with the [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extension.
- **Docker** (optional): For containerized development and testing.

### Setting Up Your Development Environment

1. **Fork the Repository**

   Go to the [VibeCoin GitHub repository](https://github.com/vibecoin/vibecoin) and click the "Fork" button in the top-right corner.

2. **Clone Your Fork**

   ```bash
   git clone https://github.com/YOUR_USERNAME/vibecoin.git
   cd vibecoin
   ```

3. **Add the Upstream Remote**

   ```bash
   git remote add upstream https://github.com/vibecoin/vibecoin.git
   ```

4. **Install Development Dependencies**

   ```bash
   # Install additional Rust components
   rustup component add rustfmt clippy
   
   # Install pre-commit hooks
   cargo install cargo-husky
   cargo husky install
   ```

5. **Build the Project**

   ```bash
   cargo build
   ```

6. **Run Tests**

   ```bash
   cargo test
   ```

### IDE Setup

#### VS Code

1. Install the following extensions:
   - rust-analyzer
   - Better TOML
   - CodeLLDB (for debugging)
   - GitLens (for Git integration)

2. Configure settings.json:

   ```json
   {
     "rust-analyzer.checkOnSave.command": "clippy",
     "rust-analyzer.cargo.allFeatures": true,
     "editor.formatOnSave": true,
     "editor.rulers": [100],
     "files.insertFinalNewline": true,
     "files.trimTrailingWhitespace": true
   }
   ```

#### IntelliJ IDEA / CLion

1. Install the Rust plugin
2. Configure the Rust toolchain in Settings > Languages & Frameworks > Rust

## Project Structure

VibeCoin follows a modular architecture with the following directory structure:

```
vibecoin/
├── benches/            # Benchmarks
├── config/             # Configuration module
├── consensus/          # Consensus module
│   ├── pow/            # Proof of Work implementation
│   ├── poh/            # Proof of History implementation
│   ├── validation/     # Block and transaction validation
│   └── mining/         # Block production
├── crypto/             # Cryptography module
│   ├── keys.rs         # Key generation and management
│   ├── hash.rs         # Hashing functions
│   └── signer.rs       # Digital signatures
├── docs/               # Documentation
├── mempool/            # Transaction pool
├── network/            # Networking and P2P communication
│   ├── types/          # Network message types
│   ├── peer/           # Peer connection management
│   ├── service/        # Network service
│   └── codec/          # Message serialization
├── scripts/            # Utility scripts
├── src/                # Main source code
│   ├── lib.rs          # Library entry point
│   └── main.rs         # CLI entry point
├── storage/            # Blockchain storage
│   ├── kv_store.rs     # Key-value store abstraction
│   ├── block_store.rs  # Block storage
│   ├── tx_store.rs     # Transaction storage
│   ├── state_store.rs  # Account state storage
│   └── trie/           # Merkle Patricia Trie implementation
├── tests/              # Integration tests
├── tools/              # Development tools
├── Cargo.toml          # Rust package manifest
├── Cargo.lock          # Dependency lock file
├── .github/            # GitHub configuration
├── .gitignore          # Git ignore file
└── README.md           # Project README
```

### Module Responsibilities

- **config**: Configuration handling and parsing
- **consensus**: Consensus mechanism implementation
- **crypto**: Cryptographic primitives
- **mempool**: Transaction pool management
- **network**: Peer-to-peer communication
- **storage**: Blockchain data persistence
- **tools**: Development and maintenance tools

## Development Workflow

### Feature Development Process

1. **Create a Feature Branch**

   ```bash
   # Ensure you're on the main branch
   git checkout main
   
   # Pull the latest changes
   git pull upstream main
   
   # Create a feature branch
   git checkout -b feature/your-feature-name
   ```

2. **Implement Your Feature**

   - Write code that follows the [Coding Standards](#coding-standards)
   - Add tests for your feature
   - Update documentation as needed

3. **Commit Your Changes**

   ```bash
   # Stage your changes
   git add .
   
   # Commit with a descriptive message
   git commit -m "feat(module): add your feature description"
   ```

4. **Keep Your Branch Updated**

   ```bash
   # Fetch upstream changes
   git fetch upstream
   
   # Rebase your branch on the latest main
   git rebase upstream/main
   ```

5. **Push Your Changes**

   ```bash
   git push origin feature/your-feature-name
   ```

6. **Create a Pull Request**

   - Go to the [VibeCoin GitHub repository](https://github.com/vibecoin/vibecoin)
   - Click "New Pull Request"
   - Select your branch and provide a detailed description
   - Link any related issues

7. **Address Review Feedback**

   - Make requested changes
   - Push additional commits
   - Respond to reviewer comments

8. **Merge Your Pull Request**

   Once approved, a maintainer will merge your pull request.

### Bug Fix Process

1. **Create a Bug Fix Branch**

   ```bash
   git checkout main
   git pull upstream main
   git checkout -b fix/bug-description
   ```

2. **Implement Your Fix**

   - Write a test that reproduces the bug
   - Fix the bug
   - Ensure all tests pass

3. **Commit Your Changes**

   ```bash
   git add .
   git commit -m "fix(module): fix bug description"
   ```

4. **Follow Steps 4-8 from the Feature Development Process**

### Hotfix Process

For critical bugs that need immediate attention:

1. **Create a Hotfix Branch from the Latest Release**

   ```bash
   git checkout v1.2.3  # Latest release tag
   git checkout -b hotfix/critical-bug
   ```

2. **Implement Your Fix**

   - Fix the bug with minimal changes
   - Add tests

3. **Commit Your Changes**

   ```bash
   git add .
   git commit -m "fix(module): fix critical bug description"
   ```

4. **Create a Pull Request Against Both Main and the Release Branch**

   - Create a PR to merge into the release branch
   - Create a PR to merge into main

## Coding Standards

### Code Style

VibeCoin follows the Rust style guide with some additional conventions:

- Use 4 spaces for indentation
- Maximum line length of 100 characters
- No file should exceed 1000 lines (enforced by pre-commit hook)
- Use meaningful variable and function names
- Write comprehensive comments

### Commit Message Format

We follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- **feat**: A new feature
- **fix**: A bug fix
- **docs**: Documentation changes
- **style**: Code style changes (formatting, etc.)
- **refactor**: Code changes that neither fix bugs nor add features
- **perf**: Performance improvements
- **test**: Adding or updating tests
- **chore**: Changes to the build process or auxiliary tools

Examples:
- `feat(consensus): add dynamic difficulty adjustment`
- `fix(storage): resolve race condition in block store`
- `docs(api): update RPC documentation`

### Pull Request Guidelines

- Keep PRs focused on a single change
- Include tests for new features and bug fixes
- Update documentation as needed
- Link related issues
- Ensure CI passes before requesting review
- Respond to review comments promptly

## Testing

### Types of Tests

- **Unit Tests**: Test individual functions and methods
- **Integration Tests**: Test interactions between components
- **End-to-End Tests**: Test the entire system
- **Benchmarks**: Measure performance

### Writing Tests

Unit tests are typically placed in the same file as the code they test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Test code
        assert_eq!(function_name(input), expected_output);
    }
}
```

Integration tests are placed in the `tests/` directory:

```rust
// tests/integration_test.rs
use vibecoin::module::function;

#[test]
fn test_integration() {
    // Test code
}
```

Benchmarks are placed in the `benches/` directory:

```rust
// benches/benchmark.rs
#![feature(test)]
extern crate test;

use test::Bencher;
use vibecoin::module::function;

#[bench]
fn bench_function(b: &mut Bencher) {
    b.iter(|| {
        // Code to benchmark
        function(input)
    });
}
```

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific module
cargo test --lib storage

# Run a specific test
cargo test test_function_name

# Run benchmarks
cargo bench
```

### Test Coverage

We aim for high test coverage. You can check coverage using:

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

## Documentation

### Code Documentation

- Use doc comments (`///`) for public API documentation
- Use regular comments (`//`) for implementation details
- Document all public functions, structs, and traits
- Include examples where appropriate

Example:

```rust
/// Calculates the hash of a block.
///
/// # Arguments
///
/// * `block` - The block to hash
///
/// # Returns
///
/// A 32-byte hash of the block
///
/// # Examples
///
/// ```
/// use vibecoin::crypto::hash_block;
/// use vibecoin::storage::Block;
///
/// let block = Block::new();
/// let hash = hash_block(&block);
/// ```
pub fn hash_block(block: &Block) -> [u8; 32] {
    // Implementation
}
```

### Project Documentation

- Update the README.md with significant changes
- Update the documentation in the `docs/` directory
- Create or update module-specific documentation

### Generating Documentation

```bash
# Generate and open documentation
cargo doc --open
```

## Code Review Process

### Review Criteria

- **Functionality**: Does the code work as expected?
- **Performance**: Is the code efficient?
- **Security**: Are there any security concerns?
- **Maintainability**: Is the code easy to understand and maintain?
- **Testing**: Are there sufficient tests?
- **Documentation**: Is the code well-documented?
- **Style**: Does the code follow our coding standards?

### Review Process

1. **Automated Checks**: CI runs tests, linting, and formatting checks
2. **Peer Review**: At least one maintainer reviews the code
3. **Feedback**: Reviewers provide feedback and request changes if needed
4. **Iteration**: The author addresses feedback and pushes additional commits
5. **Approval**: Reviewers approve the changes
6. **Merge**: A maintainer merges the pull request

### Review Etiquette

- Be respectful and constructive
- Focus on the code, not the person
- Explain your reasoning
- Suggest alternatives when possible
- Respond to feedback promptly

## Continuous Integration

VibeCoin uses GitHub Actions for CI/CD:

### CI Workflow

1. **Build**: Compile the code
2. **Test**: Run unit and integration tests
3. **Lint**: Check code style with rustfmt and clippy
4. **Coverage**: Generate test coverage report
5. **Benchmarks**: Run performance benchmarks

### CI Configuration

The CI configuration is defined in `.github/workflows/`:

- `ci.yml`: Main CI workflow
- `release.yml`: Release workflow
- `docs.yml`: Documentation workflow

### CI Status

You can check the CI status on the GitHub repository:

- Green check: All checks passed
- Red X: One or more checks failed
- Yellow circle: Checks are in progress

## Release Process

### Version Numbering

VibeCoin follows [Semantic Versioning](https://semver.org/):

- **Major**: Incompatible API changes
- **Minor**: Backward-compatible new features
- **Patch**: Backward-compatible bug fixes

### Release Steps

1. **Create a Release Branch**

   ```bash
   git checkout main
   git pull upstream main
   git checkout -b release/v1.2.3
   ```

2. **Update Version Numbers**

   - Update version in Cargo.toml
   - Update version in other relevant files

3. **Create a Changelog Entry**

   Update `CHANGELOG.md` with the changes in this release.

4. **Create a Pull Request**

   Create a PR from the release branch to main.

5. **Review and Merge**

   After review and approval, merge the PR.

6. **Create a Release Tag**

   ```bash
   git checkout main
   git pull upstream main
   git tag -a v1.2.3 -m "Release v1.2.3"
   git push upstream v1.2.3
   ```

7. **Create a GitHub Release**

   - Go to the [Releases page](https://github.com/vibecoin/vibecoin/releases)
   - Click "Draft a new release"
   - Select the tag
   - Add release notes
   - Attach binaries

## Troubleshooting

### Common Development Issues

#### Build Errors

- Check that you have the latest Rust version
- Check that you have all required dependencies
- Try cleaning the build: `cargo clean`

#### Test Failures

- Check that you have the latest code: `git pull upstream main`
- Check for environment-specific issues
- Try running tests with more verbose output: `cargo test -- --nocapture`

#### Git Issues

- **Merge Conflicts**: Resolve conflicts manually and continue the rebase
- **Detached HEAD**: Checkout a branch: `git checkout -b new-branch`
- **Accidental Commits**: Use `git reset --soft HEAD~1` to undo the last commit

### Getting Help

- **GitHub Issues**: Search existing issues or create a new one
- **Discord Channel**: Join our developer Discord for real-time help
- **Mailing List**: Subscribe to our developer mailing list

## Conclusion

Following this development workflow will help ensure a smooth contribution process. If you have any questions or suggestions for improving this guide, please open an issue or pull request.

Happy coding!
