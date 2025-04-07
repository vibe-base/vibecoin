use std::sync::Arc;
use tempfile::tempdir;

use vibecoin::storage::{
    RocksDBStore, BlockStore, TxStore, StateStore, StateManager, BatchOperationManager,
    Block, TransactionRecord, TransactionStatus, AccountState, AccountType, Schema,
    RocksDBManager, DatabaseStats,
};

#[test]
fn test_rocksdb_integration() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let store = RocksDBStore::new(temp_dir.path()).unwrap();
    
    // Test basic operations
    store.put(b"test_key", b"test_value").unwrap();
    let value = store.get(b"test_key").unwrap().unwrap();
    assert_eq!(value, b"test_value");
    
    store.delete(b"test_key").unwrap();
    let value = store.get(b"test_key").unwrap();
    assert!(value.is_none());
    
    // Test batch operations
    let mut batch = Vec::new();
    batch.push(vibecoin::storage::WriteBatchOperation::Put {
        key: b"batch_key1".to_vec(),
        value: b"batch_value1".to_vec(),
    });
    batch.push(vibecoin::storage::WriteBatchOperation::Put {
        key: b"batch_key2".to_vec(),
        value: b"batch_value2".to_vec(),
    });
    store.write_batch(batch).unwrap();
    
    let value1 = store.get(b"batch_key1").unwrap().unwrap();
    let value2 = store.get(b"batch_key2").unwrap().unwrap();
    assert_eq!(value1, b"batch_value1");
    assert_eq!(value2, b"batch_value2");
    
    // Test prefix scan
    let prefix = b"batch_";
    let results = store.scan_prefix(prefix).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_block_store() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let block_store = Arc::new(BlockStore::new(&kv_store));
    
    // Create a test block
    let block = Block {
        height: 1,
        hash: [1; 32],
        prev_hash: [0; 32],
        timestamp: 12345,
        transactions: vec![[2; 32], [3; 32]],
        state_root: [4; 32],
        tx_root: [5; 32],
        nonce: 42,
        poh_seq: 100,
        poh_hash: [6; 32],
        difficulty: 1000,
        total_difficulty: 1000,
    };
    
    // Store the block
    block_store.put_block(&block).unwrap();
    
    // Retrieve the block by height
    let retrieved_block = block_store.get_block_by_height(1).unwrap().unwrap();
    assert_eq!(retrieved_block.hash, block.hash);
    
    // Retrieve the block by hash
    let retrieved_block = block_store.get_block_by_hash(&block.hash).unwrap().unwrap();
    assert_eq!(retrieved_block.height, block.height);
    
    // Get the latest block
    let latest_block = block_store.get_latest_block().unwrap().unwrap();
    assert_eq!(latest_block.hash, block.hash);
}

#[test]
fn test_transaction_store() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
    
    // Create a test transaction
    let tx = TransactionRecord {
        tx_id: [1; 32],
        sender: [2; 32],
        recipient: [3; 32],
        value: 100,
        gas_price: 1,
        gas_limit: 21000,
        gas_used: 21000,
        nonce: 0,
        timestamp: 12345,
        block_height: 1,
        data: None,
        status: TransactionStatus::Confirmed,
    };
    
    // Store the transaction
    tx_store.put_transaction(&tx).unwrap();
    
    // Retrieve the transaction
    let retrieved_tx = tx_store.get_transaction(&tx.tx_id).unwrap().unwrap();
    assert_eq!(retrieved_tx.value, tx.value);
    
    // Check if the transaction exists
    let exists = tx_store.has_transaction(&tx.tx_id).unwrap();
    assert!(exists);
    
    // Get transactions by sender
    let sender_txs = tx_store.get_transactions_by_sender(&tx.sender).unwrap();
    assert_eq!(sender_txs.len(), 1);
    assert_eq!(sender_txs[0].tx_id, tx.tx_id);
    
    // Get transactions by recipient
    let recipient_txs = tx_store.get_transactions_by_recipient(&tx.recipient).unwrap();
    assert_eq!(recipient_txs.len(), 1);
    assert_eq!(recipient_txs[0].tx_id, tx.tx_id);
    
    // Get transactions by block
    let block_txs = tx_store.get_transactions_by_block(1).unwrap();
    assert_eq!(block_txs.len(), 1);
    assert_eq!(block_txs[0].tx_id, tx.tx_id);
    
    // Update transaction status
    tx_store.update_transaction_status(&tx.tx_id, TransactionStatus::Failed).unwrap();
    let updated_tx = tx_store.get_transaction(&tx.tx_id).unwrap().unwrap();
    assert_eq!(updated_tx.status, TransactionStatus::Failed);
    
    // Get latest nonce
    let latest_nonce = tx_store.get_latest_nonce(&tx.sender).unwrap();
    assert_eq!(latest_nonce, Some(0));
}

#[test]
fn test_state_store() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let state_store = Arc::new(StateStore::new(kv_store.as_ref()));
    
    // Create a test account
    let address = [1; 32];
    state_store.create_account(&address, 1000, AccountType::User).unwrap();
    
    // Retrieve the account
    let account = state_store.get_account_state(&address).unwrap().unwrap();
    assert_eq!(account.balance, 1000);
    assert_eq!(account.nonce, 0);
    assert_eq!(account.account_type, AccountType::User);
    
    // Update the account balance
    state_store.update_balance(&address, 2000).unwrap();
    let updated_account = state_store.get_account_state(&address).unwrap().unwrap();
    assert_eq!(updated_account.balance, 2000);
    
    // Update the account nonce
    state_store.update_nonce(&address, 1).unwrap();
    let updated_account = state_store.get_account_state(&address).unwrap().unwrap();
    assert_eq!(updated_account.nonce, 1);
    
    // Calculate the state root
    let state_root = state_store.calculate_state_root(1, 12345).unwrap();
    assert_eq!(state_root.block_height, 1);
    assert_eq!(state_root.timestamp, 12345);
    
    // Get the state root
    let retrieved_root = state_store.get_state_root(1).unwrap().unwrap();
    assert_eq!(retrieved_root.root_hash, state_root.root_hash);
}

#[test]
fn test_state_manager() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let state_manager = StateManager::new(kv_store.as_ref());
    
    // Create test accounts
    let addr1 = [1; 32];
    let addr2 = [2; 32];
    
    state_manager.create_account(&addr1, 1000, AccountType::User).unwrap();
    state_manager.create_account(&addr2, 0, AccountType::User).unwrap();
    
    // Set storage values
    let key = [3; 32];
    let value = vec![4; 32];
    state_manager.set_storage_value(&addr1, &key, value.clone()).unwrap();
    
    // Get storage values
    let retrieved_value = state_manager.get_storage_value(&addr1, &key).unwrap().unwrap();
    assert_eq!(retrieved_value, value);
    
    // Create a transaction
    let tx = TransactionRecord {
        tx_id: [5; 32],
        sender: addr1,
        recipient: addr2,
        value: 500,
        gas_price: 1,
        gas_limit: 21000,
        gas_used: 21000,
        nonce: 0,
        timestamp: 12345,
        block_height: 1,
        data: None,
        status: TransactionStatus::Confirmed,
    };
    
    // Store the transaction
    let tx_key = Schema::tx_by_hash_key(&tx.tx_id);
    let tx_value = bincode::serialize(&tx).unwrap();
    kv_store.put(tx_key.as_bytes(), &tx_value).unwrap();
    
    // Create a block index entry
    let tx_block_key = Schema::tx_by_block_key(1, &tx.tx_id);
    kv_store.put(tx_block_key.as_bytes(), &tx.tx_id).unwrap();
    
    // Create a block
    let block = Block {
        height: 1,
        hash: [6; 32],
        prev_hash: [0; 32],
        timestamp: 12345,
        transactions: vec![tx.tx_id],
        state_root: [0; 32], // Will be calculated
        tx_root: [0; 32],
        nonce: 0,
        poh_seq: 0,
        poh_hash: [0; 32],
        difficulty: 0,
        total_difficulty: 0,
    };
    
    // Apply the block
    state_manager.apply_block(&block).unwrap();
    
    // Check the sender's balance
    let sender_state = state_manager.get_account_state(&addr1).unwrap().unwrap();
    assert_eq!(sender_state.balance, 479); // 1000 - 500 - 21
    assert_eq!(sender_state.nonce, 1);
    
    // Check the recipient's balance
    let recipient_state = state_manager.get_account_state(&addr2).unwrap().unwrap();
    assert_eq!(recipient_state.balance, 500);
    
    // Calculate the state root
    let state_root = state_manager.calculate_state_root(1, 12345).unwrap();
    
    // Generate a proof
    let proof = state_manager.generate_account_proof(&addr1).unwrap();
    
    // Verify the proof
    let is_valid = StateManager::verify_account_proof(&proof, &state_root.root_hash).unwrap();
    assert!(is_valid);
}

#[test]
fn test_batch_operations() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let block_store = Arc::new(BlockStore::new(&kv_store));
    let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
    let state_store = Arc::new(StateStore::new(kv_store.as_ref()));
    
    let batch_manager = Arc::new(BatchOperationManager::new(
        kv_store.clone(),
        block_store.clone(),
        tx_store.clone(),
        state_store.clone(),
    ));
    
    // Create a test block
    let block = Block {
        height: 1,
        hash: [1; 32],
        prev_hash: [0; 32],
        timestamp: 12345,
        transactions: vec![[2; 32], [3; 32]],
        state_root: [4; 32],
        tx_root: [5; 32],
        nonce: 42,
        poh_seq: 100,
        poh_hash: [6; 32],
        difficulty: 1000,
        total_difficulty: 1000,
    };
    
    // Create test transactions
    let tx1 = TransactionRecord {
        tx_id: [2; 32],
        sender: [10; 32],
        recipient: [11; 32],
        value: 100,
        gas_price: 1,
        gas_limit: 21000,
        gas_used: 21000,
        nonce: 0,
        timestamp: 12345,
        block_height: 1,
        data: None,
        status: TransactionStatus::Confirmed,
    };
    
    let tx2 = TransactionRecord {
        tx_id: [3; 32],
        sender: [12; 32],
        recipient: [13; 32],
        value: 200,
        gas_price: 1,
        gas_limit: 21000,
        gas_used: 21000,
        nonce: 0,
        timestamp: 12345,
        block_height: 1,
        data: None,
        status: TransactionStatus::Confirmed,
    };
    
    // Create test state changes
    let state1 = AccountState {
        balance: 900,
        nonce: 1,
        account_type: AccountType::User,
    };
    
    let state2 = AccountState {
        balance: 200,
        nonce: 0,
        account_type: AccountType::User,
    };
    
    let state_changes = vec![
        ([10; 32], state1),
        ([11; 32], state2),
    ];
    
    // Commit the block
    batch_manager.commit_block(&block, &[tx1, tx2], &state_changes).unwrap();
    
    // Verify the block was stored
    let stored_block = block_store.get_block_by_height(1).unwrap().unwrap();
    assert_eq!(stored_block.hash, block.hash);
    
    // Verify transactions were stored
    let stored_tx1 = tx_store.get_transaction(&[2; 32]).unwrap().unwrap();
    assert_eq!(stored_tx1.value, 100);
    
    let stored_tx2 = tx_store.get_transaction(&[3; 32]).unwrap().unwrap();
    assert_eq!(stored_tx2.value, 200);
    
    // Verify state changes were stored
    let stored_state1 = state_store.get_account_state(&[10; 32]).unwrap().unwrap();
    assert_eq!(stored_state1.balance, 900);
    
    let stored_state2 = state_store.get_account_state(&[11; 32]).unwrap().unwrap();
    assert_eq!(stored_state2.balance, 200);
    
    // Rollback the block
    batch_manager.rollback_block(1).unwrap();
    
    // Verify the block was removed
    assert!(block_store.get_block_by_height(1).unwrap().is_none());
    
    // Verify transactions were removed
    assert!(tx_store.get_transaction(&[2; 32]).unwrap().is_none());
    assert!(tx_store.get_transaction(&[3; 32]).unwrap().is_none());
}

#[test]
fn test_rocksdb_schema() {
    // Test key generation
    let block_key = Schema::block_by_height_key(123);
    assert_eq!(block_key, "block:123");
    
    let hash = [1u8; 32];
    let block_hash_key = Schema::block_by_hash_key(&hash);
    assert_eq!(block_hash_key, "block_hash:0101010101010101010101010101010101010101010101010101010101010101");
    
    let tx_key = Schema::tx_by_hash_key(&hash);
    assert_eq!(tx_key, "tx:0101010101010101010101010101010101010101010101010101010101010101");
    
    // Test key parsing
    let key_type = Schema::parse_key(block_key.as_bytes());
    assert_eq!(key_type, vibecoin::storage::KeyType::BlockByHeight);
    
    let height = Schema::extract_block_height(&block_key);
    assert_eq!(height, Some(123));
    
    let tx_hash = Schema::extract_tx_hash(&tx_key);
    assert_eq!(tx_hash, Some(hash));
}

#[test]
fn test_rocksdb_manager() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let store = RocksDBStore::new(temp_dir.path()).unwrap();
    let manager = RocksDBManager::new(&store);
    
    // Add some data
    store.put(Schema::block_by_height_key(1).as_bytes(), &[1]).unwrap();
    store.put(Schema::block_by_height_key(2).as_bytes(), &[2]).unwrap();
    store.put(Schema::tx_by_hash_key(&[1u8; 32]).as_bytes(), &[3]).unwrap();
    store.put(Schema::account_state_key(&[2u8; 32]).as_bytes(), &[4]).unwrap();
    
    // Get database stats
    let stats = manager.get_stats().unwrap();
    assert_eq!(stats.block_count, 2);
    assert_eq!(stats.transaction_count, 1);
    assert_eq!(stats.account_count, 1);
    
    // Test compaction
    manager.compact().unwrap();
    
    // Test pruning
    store.put(Schema::latest_block_height_key().as_bytes(), &10u64.to_be_bytes()).unwrap();
    manager.prune(5).unwrap();
}
