use vibecoin::crypto::{
    VibeKeypair,
    address_from_pubkey,
    sha256,
    double_sha256,
    sign_message,
    verify_signature
};

#[test]
fn test_crypto_workflow() {
    // Generate a keypair
    let keypair = VibeKeypair::generate();
    
    // Derive an address
    let address = keypair.address();
    let address2 = address_from_pubkey(&keypair.public);
    assert_eq!(address, address2);
    
    // Create a message
    let message = b"Transfer 100 VibeCoin to Alice";
    
    // Hash the message
    let hash = sha256(message);
    assert_eq!(hash.len(), 32);
    
    // Double hash for extra security
    let double_hash = double_sha256(message);
    assert_ne!(hash, double_hash);
    
    // Sign the message
    let signature = sign_message(&keypair, message);
    
    // Verify the signature
    let is_valid = verify_signature(message, &signature, &keypair.public);
    assert!(is_valid);
    
    // Tamper with the message
    let tampered_message = b"Transfer 1000 VibeCoin to Alice";
    let is_valid = verify_signature(tampered_message, &signature, &keypair.public);
    assert!(!is_valid);
}

#[test]
fn test_keypair_persistence() {
    // Generate a keypair
    let original = VibeKeypair::generate();
    
    // Export the secret key
    let secret_bytes = original.secret_bytes();
    
    // Recreate the keypair from the secret
    let restored = VibeKeypair::from_bytes(&secret_bytes).unwrap();
    
    // Public keys should match
    assert_eq!(original.public_bytes(), restored.public_bytes());
    
    // Addresses should match
    assert_eq!(original.address(), restored.address());
    
    // Should be able to sign and verify
    let message = b"Test message";
    let signature = sign_message(&restored, message);
    let is_valid = verify_signature(message, &signature, &original.public);
    assert!(is_valid);
}

#[test]
fn test_known_hash_vectors() {
    // Test vectors from https://www.di-mgt.com.au/sha_testvectors.html
    
    // Test 1
    let input1 = b"abc";
    let expected1 = hex::decode("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad").unwrap();
    assert_eq!(sha256(input1).to_vec(), expected1);
    
    // Test 2
    let input2 = b"";
    let expected2 = hex::decode("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").unwrap();
    assert_eq!(sha256(input2).to_vec(), expected2);
    
    // Test 3
    let input3 = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    let expected3 = hex::decode("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1").unwrap();
    assert_eq!(sha256(input3).to_vec(), expected3);
}

#[test]
fn test_address_uniqueness() {
    // Generate multiple keypairs
    let keypair1 = VibeKeypair::generate();
    let keypair2 = VibeKeypair::generate();
    let keypair3 = VibeKeypair::generate();
    
    // Derive addresses
    let address1 = keypair1.address();
    let address2 = keypair2.address();
    let address3 = keypair3.address();
    
    // Addresses should be unique
    assert_ne!(address1, address2);
    assert_ne!(address1, address3);
    assert_ne!(address2, address3);
}
