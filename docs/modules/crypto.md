# Cryptography Module

## Overview

The Cryptography Module provides essential cryptographic primitives for the VibeCoin blockchain. It handles key generation, digital signatures, message verification, and hashing functions.

## Components

### Key Management

The key management component handles cryptographic keys:

- **VibeKeypair**: Wrapper around Ed25519 keypairs for signing and verification
- **Key Generation**: Secure random key generation using OsRng
- **Key Import/Export**: Methods for serializing and deserializing keys
- **Address Derivation**: Deriving account addresses from public keys

### Digital Signatures

The digital signatures component handles message signing and verification:

- **Signing**: Creating signatures for transactions and other data
- **Verification**: Verifying signatures against public keys
- **Signature Format**: Ed25519 signatures (64 bytes)

### Hashing

The hashing component provides cryptographic hash functions:

- **SHA-256**: Standard SHA-256 hash function
- **Double SHA-256**: SHA-256 applied twice for extra security
- **Hash Wrapper**: Serializable wrapper for hash values

## Implementation Details

```rust
// Key Management
pub struct VibeKeypair {
    pub public: PublicKey,
    pub secret: SecretKey,
}

impl VibeKeypair {
    pub fn generate() -> Self;
    pub fn from_bytes(secret_bytes: &[u8]) -> Result<Self, Error>;
    pub fn public_bytes(&self) -> [u8; 32];
    pub fn secret_bytes(&self) -> [u8; 32];
    pub fn address(&self) -> [u8; 32];
}

// Address Derivation
pub fn address_from_pubkey(pubkey: &PublicKey) -> [u8; 32];

// Digital Signatures
pub fn sign_message(keypair: &VibeKeypair, message: &[u8]) -> VibeSignature;
pub fn verify_signature(message: &[u8], signature: &VibeSignature, public_key: &PublicKey) -> bool;

// Hashing
pub fn sha256(data: &[u8]) -> [u8; 32];
pub fn double_sha256(data: &[u8]) -> [u8; 32];
```

## Security Considerations

### Key Security

- **Secure Generation**: Uses cryptographically secure random number generation
- **Key Protection**: Secret keys are never exposed or logged
- **Memory Safety**: Careful handling of sensitive data in memory

### Signature Security

- **Ed25519**: Uses the Ed25519 signature scheme, which is resistant to side-channel attacks
- **Deterministic Signatures**: Signatures are deterministic to prevent nonce reuse
- **Malleability**: Ed25519 signatures are non-malleable

### Hash Security

- **Collision Resistance**: SHA-256 is collision-resistant
- **Preimage Resistance**: SHA-256 is preimage-resistant
- **Double Hashing**: Double SHA-256 provides additional security

## Usage Examples

### Key Generation and Signing

```rust
// Generate a new keypair
let keypair = VibeKeypair::generate();

// Get the address
let address = keypair.address();

// Sign a message
let message = b"Transfer 100 VibeCoin to Alice";
let signature = sign_message(&keypair, message);

// Verify the signature
let is_valid = verify_signature(message, &signature, &keypair.public);
assert!(is_valid);
```

### Hashing

```rust
// Hash some data
let data = b"Block data";
let hash = sha256(data);

// Double hash for extra security
let double_hash = double_sha256(data);
```

## Dependencies

- **ed25519-dalek**: For Ed25519 key generation and signatures
- **sha2**: For SHA-256 hashing
- **rand_core**: For secure random number generation
- **hex**: For hex encoding/decoding

## Future Improvements

- **BLS Signatures**: Add support for BLS signatures for aggregation
- **Zero-Knowledge Proofs**: Integrate zero-knowledge proof systems
- **Threshold Signatures**: Implement threshold signature schemes
- **Key Recovery**: Add support for key recovery from signatures
