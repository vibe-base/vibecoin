# VibeCoin Cryptography Module

## Overview

The cryptography module provides essential cryptographic primitives for the VibeCoin blockchain, including key generation, digital signatures, message verification, and hashing functions.

## Components

### Keys (keys.rs)

Handles key generation and management:

- `VibeKeypair`: Wrapper around Ed25519 keypairs for signing and verification
- `address_from_pubkey`: Derives an address from a public key
- `VibePublicKey`: Serializable wrapper for public keys

### Hashing (hash.rs)

Provides hashing utilities:

- `sha256`: Computes SHA-256 hash of data
- `double_sha256`: Computes double SHA-256 hash (SHA-256 of SHA-256)
- `Hash`: A serializable wrapper for 32-byte hash values

### Signing (signer.rs)

Handles digital signatures:

- `sign_message`: Signs a message using a keypair
- `verify_signature`: Verifies a signature against a message and public key
- `VibeSignature`: Serializable wrapper for Ed25519 signatures

## Usage

```rust
use vibecoin::crypto::{VibeKeypair, sign_message, verify_signature, sha256};

// Generate a keypair
let keypair = VibeKeypair::generate();

// Get the address
let address = keypair.address();

// Sign a message
let message = b"Transfer 100 VibeCoin to Alice";
let signature = sign_message(&keypair, message);

// Verify the signature
let is_valid = verify_signature(message, &signature, &keypair.public);
assert!(is_valid);

// Hash data
let hash = sha256(message);
```

## Security Considerations

- Secret keys should be handled with care and never exposed
- Use `OsRng` for secure randomness
- Avoid logging sensitive information
- Consider implementing zeroization for sensitive data in memory

## Testing

Each component includes unit tests that can be run with:

```bash
cargo test
```

Integration tests are available in the `tests` directory and demonstrate how the different components work together.
