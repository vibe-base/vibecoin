use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use vibecoin::storage::RocksDBStore;
use vibecoin::storage::object::{Object, Ownership};
use vibecoin::storage::object_store::ObjectStore;

fn main() {
    // Create a RocksDB store
    let db_path = Path::new("./data/vibecoin/test_objects");
    let kv_store = RocksDBStore::new(db_path).expect("Failed to open RocksDB");
    
    // Create an object store
    let object_store = ObjectStore::new(&kv_store);
    
    // Create a test address
    let owner_address = [1; 32];
    
    println!("=== Creating Objects ===");
    
    // Create a coin object
    let mut metadata = HashMap::new();
    metadata.insert("name".to_string(), "VibeCoin".to_string());
    metadata.insert("symbol".to_string(), "VIBE".to_string());
    metadata.insert("decimals".to_string(), "18".to_string());
    
    let coin = object_store.create_object(
        Ownership::Address(owner_address),
        "Coin".to_string(),
        vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100], // 100 tokens
        Some(metadata),
    ).expect("Failed to create coin object");
    
    println!("Created coin object: {}", coin.id_hex());
    println!("  Type: {}", coin.type_tag);
    println!("  Version: {}", coin.version);
    println!("  Metadata: {:?}", coin.metadata);
    
    // Create an NFT object
    let mut nft_metadata = HashMap::new();
    nft_metadata.insert("name".to_string(), "Vibe NFT #1".to_string());
    nft_metadata.insert("description".to_string(), "A unique digital collectible".to_string());
    nft_metadata.insert("image".to_string(), "https://example.com/nft1.png".to_string());
    
    let nft = object_store.create_object(
        Ownership::Address(owner_address),
        "NFT".to_string(),
        vec![1, 2, 3, 4], // Some arbitrary data
        Some(nft_metadata),
    ).expect("Failed to create NFT object");
    
    println!("Created NFT object: {}", nft.id_hex());
    println!("  Type: {}", nft.type_tag);
    println!("  Version: {}", nft.version);
    println!("  Metadata: {:?}", nft.metadata);
    
    // Create a shared object
    let shared_object = object_store.create_object(
        Ownership::Shared,
        "SharedResource".to_string(),
        vec![5, 6, 7, 8], // Some arbitrary data
        None,
    ).expect("Failed to create shared object");
    
    println!("Created shared object: {}", shared_object.id_hex());
    println!("  Type: {}", shared_object.type_tag);
    println!("  Version: {}", shared_object.version);
    
    // Create an immutable object
    let immutable_object = object_store.create_object(
        Ownership::Immutable,
        "ImmutableResource".to_string(),
        vec![9, 10, 11, 12], // Some arbitrary data
        None,
    ).expect("Failed to create immutable object");
    
    println!("Created immutable object: {}", immutable_object.id_hex());
    println!("  Type: {}", immutable_object.type_tag);
    println!("  Version: {}", immutable_object.version);
    
    println!("\n=== Querying Objects ===");
    
    // Get objects by owner
    let owner_objects = object_store.get_objects_by_owner(&owner_address)
        .expect("Failed to get objects by owner");
    
    println!("Objects owned by address {}:", hex::encode(owner_address));
    for obj in &owner_objects {
        println!("  {} ({})", obj.id_hex(), obj.type_tag);
    }
    
    // Get objects by type
    let coins = object_store.get_objects_by_type("Coin")
        .expect("Failed to get objects by type");
    
    println!("Coin objects:");
    for coin in &coins {
        println!("  {} (owned by: {:?})", coin.id_hex(), coin.owner);
    }
    
    println!("\n=== Updating Objects ===");
    
    // Update a coin object (increase balance)
    if let Some(coin) = coins.first() {
        let updated_coin = object_store.update_object(&coin.id, |obj| {
            // Update the balance (last 8 bytes represent the amount)
            let mut new_contents = obj.contents.clone();
            let amount = 200u64.to_be_bytes();
            new_contents[12..20].copy_from_slice(&amount);
            
            obj.update_contents(new_contents, obj.updated_at + 1, 1)
        }).expect("Failed to update coin object");
        
        println!("Updated coin object: {}", updated_coin.id_hex());
        println!("  New version: {}", updated_coin.version);
        println!("  New contents: {:?}", updated_coin.contents);
    }
    
    println!("\n=== Transferring Objects ===");
    
    // Transfer an NFT to another address
    let recipient = [2; 32];
    
    if let Some(nft) = object_store.get_objects_by_type("NFT").expect("Failed to get NFTs").first() {
        let transferred_nft = object_store.update_object(&nft.id, |obj| {
            obj.transfer_ownership(Ownership::Address(recipient), obj.updated_at + 1, 2)
        }).expect("Failed to transfer NFT object");
        
        println!("Transferred NFT object: {}", transferred_nft.id_hex());
        println!("  New owner: {:?}", transferred_nft.owner);
        println!("  New version: {}", transferred_nft.version);
    }
    
    // Check the recipient's objects
    let recipient_objects = object_store.get_objects_by_owner(&recipient)
        .expect("Failed to get objects by recipient");
    
    println!("Objects owned by recipient {}:", hex::encode(recipient));
    for obj in &recipient_objects {
        println!("  {} ({})", obj.id_hex(), obj.type_tag);
    }
    
    println!("\n=== Testing Ownership Rules ===");
    
    // Try to update an immutable object (should fail)
    let result = object_store.update_object(&immutable_object.id, |obj| {
        obj.update_contents(vec![13, 14, 15, 16], obj.updated_at + 1, 3)
    });
    
    match result {
        Ok(_) => println!("Unexpectedly updated immutable object!"),
        Err(e) => println!("Correctly failed to update immutable object: {}", e),
    }
    
    // Try to transfer a shared object (should fail)
    let result = object_store.update_object(&shared_object.id, |obj| {
        obj.transfer_ownership(Ownership::Address(recipient), obj.updated_at + 1, 4)
    });
    
    match result {
        Ok(_) => println!("Unexpectedly transferred shared object!"),
        Err(e) => println!("Correctly failed to transfer shared object: {}", e),
    }
    
    println!("\n=== Deleting Objects ===");
    
    // Delete a coin object
    if let Some(coin) = coins.first() {
        object_store.delete_object(&coin.id).expect("Failed to delete coin object");
        println!("Deleted coin object: {}", coin.id_hex());
    }
    
    // Verify the coin is deleted
    let remaining_coins = object_store.get_objects_by_type("Coin")
        .expect("Failed to get objects by type");
    
    println!("Remaining coin objects: {}", remaining_coins.len());
    
    println!("\n=== All Objects ===");
    
    // Get all objects
    let all_objects = object_store.get_all_objects().expect("Failed to get all objects");
    
    println!("All objects in the store:");
    for obj in &all_objects {
        println!("  {} ({}, owner: {:?})", obj.id_hex(), obj.type_tag, obj.owner);
    }
}
