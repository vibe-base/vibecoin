use vibecoin::crypto::keys::{VibeKeypair, address_from_pubkey, VibePublicKey};
use vibecoin::crypto::signer::{sign_message, verify_signature, VibeSignature};
use vibecoin::crypto::hash::{sha256, double_sha256, Hash};
use vibecoin::mempool::types::TransactionRecord;

#[test]
fn test_keypair_generation() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Verify that the public key is derived from the secret key
    let public_from_secret = keypair.public_from_secret();
    assert_eq!(keypair.public, public_from_secret);
    
    // Test address derivation
    let address1 = keypair.address();
    let address2 = address_from_pubkey(&keypair.public);
    assert_eq!(address1, address2);
}

#[test]
fn test_keypair_serialization() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Serialize the keypair
    let serialized = keypair.to_bytes();
    
    // Deserialize the keypair
    let deserialized = VibeKeypair::from_bytes(&serialized).unwrap();
    
    // Verify that the deserialized keypair matches the original
    assert_eq!(keypair.public.as_bytes(), deserialized.public.as_bytes());
    
    // Sign a message with both keypairs and verify they produce the same signature
    let message = b"Test message";
    let sig1 = sign_message(&keypair, message);
    let sig2 = sign_message(&deserialized, message);
    assert_eq!(sig1.as_bytes(), sig2.as_bytes());
}

#[test]
fn test_signature_verification() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Create a message
    let message = b"This is a test message";
    
    // Sign the message
    let signature = sign_message(&keypair, message);
    
    // Verify the signature
    let result = verify_signature(message, &signature, &keypair.public);
    assert!(result);
    
    // Try to verify with a different message
    let different_message = b"This is a different message";
    let result = verify_signature(different_message, &signature, &keypair.public);
    assert!(!result);
    
    // Try to verify with a different keypair
    let different_keypair = VibeKeypair::generate();
    let result = verify_signature(message, &signature, &different_keypair.public);
    assert!(!result);
}

#[test]
fn test_signature_malleability() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Create a message
    let message = b"Test message for malleability";
    
    // Sign the message
    let signature = sign_message(&keypair, message);
    
    // Try to modify the signature
    let mut modified_sig = signature.clone();
    let sig_bytes = modified_sig.as_bytes().clone();
    let mut modified_bytes = [0u8; 64];
    modified_bytes.copy_from_slice(sig_bytes);
    
    // Flip some bits in the signature
    modified_bytes[0] ^= 0x01;
    modified_bytes[32] ^= 0x01;
    
    let modified_sig = VibeSignature::new(modified_bytes);
    
    // Verify the modified signature (should fail)
    let result = verify_signature(message, &modified_sig, &keypair.public);
    assert!(!result);
}

#[test]
fn test_hash_functions() {
    // Test vector for SHA-256
    let input = b"abc";
    let expected = hex::decode("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").unwrap();
    
    let result = sha256(input);
    assert_eq!(result.to_vec(), expected);
    
    // Test double SHA-256
    let single_hash = sha256(input);
    let double_hash = double_sha256(input);
    
    // Double hash should be different from single hash
    assert_ne!(single_hash, double_hash);
    
    // Double hash should equal SHA-256 of single hash
    assert_eq!(double_hash, sha256(&single_hash));
}

#[test]
fn test_hash_struct() {
    // Create a Hash from bytes
    let bytes = [1u8; 32];
    let hash = Hash::new(bytes);
    
    // Test as_bytes
    assert_eq!(hash.as_bytes(), &bytes);
    
    // Test from_data
    let data = b"Test data";
    let hash_from_data = Hash::from_data(data);
    assert_eq!(hash_from_data.as_bytes(), &sha256(data));
    
    // Test from_data_double
    let hash_from_data_double = Hash::from_data_double(data);
    assert_eq!(hash_from_data_double.as_bytes(), &double_sha256(data));
    
    // Test zero hash
    let zero_hash = Hash::zero();
    assert_eq!(zero_hash.as_bytes(), &[0u8; 32]);
}

#[test]
fn test_transaction_signing() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Create a transaction
    let recipient = [2u8; 32];
    let value = 100;
    let gas_price = 10;
    let gas_limit = 21000;
    let nonce = 1;
    let data = Some(b"Test transaction data".to_vec());
    
    // Create a signed transaction
    let tx = TransactionRecord::create_signed(
        &keypair,
        recipient,
        value,
        gas_price,
        gas_limit,
        nonce,
        data.clone(),
    );
    
    // Verify the transaction fields
    assert_eq!(tx.sender, keypair.address());
    assert_eq!(tx.recipient, recipient);
    assert_eq!(tx.value, value);
    assert_eq!(tx.gas_price, gas_price);
    assert_eq!(tx.gas_limit, gas_limit);
    assert_eq!(tx.nonce, nonce);
    assert_eq!(tx.data, data);
    
    // Verify the transaction signature
    let tx_data = tx.serialize_for_signing();
    let result = verify_signature(&tx_data, &tx.signature, &keypair.public);
    assert!(result);
    
    // Modify the transaction and verify that the signature is invalid
    let mut modified_tx = tx.clone();
    modified_tx.value = 200;
    let modified_tx_data = modified_tx.serialize_for_signing();
    let result = verify_signature(&modified_tx_data, &tx.signature, &keypair.public);
    assert!(!result);
}

#[test]
fn test_transaction_id_consistency() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Create two identical transactions
    let recipient = [2u8; 32];
    let value = 100;
    let gas_price = 10;
    let gas_limit = 21000;
    let nonce = 1;
    let data = Some(b"Test transaction data".to_vec());
    
    let tx1 = TransactionRecord::create_signed(
        &keypair,
        recipient,
        value,
        gas_price,
        gas_limit,
        nonce,
        data.clone(),
    );
    
    let tx2 = TransactionRecord::create_signed(
        &keypair,
        recipient,
        value,
        gas_price,
        gas_limit,
        nonce,
        data.clone(),
    );
    
    // The transaction IDs should be the same
    assert_eq!(tx1.tx_id, tx2.tx_id);
    
    // The signatures might be different due to randomness in the signing process,
    // but they should both verify correctly
    let tx_data = tx1.serialize_for_signing();
    let result1 = verify_signature(&tx_data, &tx1.signature, &keypair.public);
    let result2 = verify_signature(&tx_data, &tx2.signature, &keypair.public);
    assert!(result1);
    assert!(result2);
}

#[test]
fn test_public_key_serialization() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Convert to VibePublicKey
    let pubkey = VibePublicKey::from(keypair.public);
    
    // Convert back to ed25519_dalek PublicKey
    let dalek_pubkey = pubkey.to_dalek_pubkey().unwrap();
    
    // Verify that the conversion is lossless
    assert_eq!(keypair.public.as_bytes(), dalek_pubkey.as_bytes());
    
    // Verify address derivation
    let address1 = address_from_pubkey(&keypair.public);
    let address2 = pubkey.address();
    assert_eq!(address1, address2);
}
