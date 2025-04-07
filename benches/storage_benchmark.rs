#![feature(test)]
extern crate test;

use std::sync::Arc;
use test::Bencher;
use tempfile::tempdir;

use vibecoin::storage::{
    RocksDBStore, BlockStore, TxStore, StateStore, StateManager, BatchOperationManager,
    Block, TransactionRecord, TransactionStatus, AccountState, AccountType, Schema,
    RocksDBManager, DatabaseStats,
};

/// Benchmark for storing a block
#[bench]
fn bench_store_block(b: &mut Bencher) {
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

    // Benchmark storing the block
    b.iter(|| {
        // Create a new block with a different height for each iteration
        let mut new_block = block.clone();
        new_block.height = test::black_box(new_block.height + 1);
        new_block.hash[0] = test::black_box(new_block.hash[0].wrapping_add(1));

        block_store.put_block(&new_block).unwrap();
    });
}

/// Benchmark for retrieving a block by height
#[bench]
fn bench_get_block_by_height(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let block_store = Arc::new(BlockStore::new(&kv_store));

    // Create and store a test block
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

    block_store.put_block(&block).unwrap();

    // Benchmark retrieving the block
    b.iter(|| {
        let height = test::black_box(1);
        block_store.get_block_by_height(height).unwrap();
    });
}

/// Benchmark for storing a transaction
#[bench]
fn bench_store_transaction(b: &mut Bencher) {
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

    // Benchmark storing the transaction
    b.iter(|| {
        // Create a new transaction with a different ID for each iteration
        let mut new_tx = tx.clone();
        new_tx.tx_id[0] = test::black_box(new_tx.tx_id[0].wrapping_add(1));

        tx_store.put_transaction(&new_tx).unwrap();
    });
}

/// Benchmark for retrieving a transaction
#[bench]
fn bench_get_transaction(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));

    // Create and store a test transaction
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

    tx_store.put_transaction(&tx).unwrap();

    // Benchmark retrieving the transaction
    b.iter(|| {
        let tx_id = test::black_box([1; 32]);
        tx_store.get_transaction(&tx_id).unwrap();
    });
}

/// Benchmark for updating account state
#[bench]
fn bench_update_account_state(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

    // Create a test account
    let address = [1; 32];
    state_store.create_account(&address, 1000, AccountType::User).unwrap();

    // Benchmark updating the account balance
    b.iter(|| {
        let balance = test::black_box(1500);
        state_store.update_balance(&address, balance).unwrap();
    });
}

/// Benchmark for calculating state root
#[bench]
fn bench_calculate_state_root(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

    // Create some test accounts
    for i in 0..100 {
        let mut address = [0; 32];
        address[0] = i;
        state_store.create_account(&address, i as u64 * 100, AccountType::User).unwrap();
    }

    // Benchmark calculating the state root
    b.iter(|| {
        let height = test::black_box(1);
        let timestamp = test::black_box(12345);
        state_store.calculate_state_root(height, timestamp).unwrap();
    });
}

/// Benchmark for state manager get_account_state
#[bench]
fn bench_state_manager_get_account(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let state_manager = StateManager::new(kv_store.as_ref());

    // Create a test account
    let address = [1; 32];
    state_manager.create_account(&address, 1000, AccountType::User).unwrap();

    // Benchmark getting the account state
    b.iter(|| {
        let addr = test::black_box(address);
        state_manager.get_account_state(&addr).unwrap();
    });
}

/// Benchmark for state manager set_storage_value
#[bench]
fn bench_state_manager_set_storage(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let state_manager = StateManager::new(kv_store.as_ref());

    // Create a test account
    let address = [1; 32];
    state_manager.create_account(&address, 1000, AccountType::Contract).unwrap();

    // Benchmark setting a storage value
    b.iter(|| {
        let key = test::black_box([2; 32]);
        let value = test::black_box(vec![3; 32]);
        state_manager.set_storage_value(&address, &key, value).unwrap();
    });
}

/// Benchmark for state manager apply_block
#[bench]
fn bench_state_manager_apply_block(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let state_manager = StateManager::new(kv_store.as_ref());

    // Create sender and recipient accounts
    let sender = [1; 32];
    let recipient = [2; 32];

    state_manager.create_account(&sender, 1000, AccountType::User).unwrap();

    // Create a transaction
    let tx = TransactionRecord {
        tx_id: [3; 32],
        sender,
        recipient,
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
        hash: [4; 32],
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

    // Benchmark applying the block
    b.iter(|| {
        let blk = test::black_box(block.clone());
        state_manager.apply_block(&blk).unwrap();
    });
}

/// Benchmark for atomic batch operations
#[bench]
fn bench_commit_block(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = Arc::new(RocksDBStore::new(temp_dir.path()).unwrap());
    let block_store = Arc::new(BlockStore::new(&kv_store));
    let tx_store = Arc::new(TxStore::new(kv_store.as_ref()));
    let state_store = Arc::new(StateStore::new(kv_store.as_ref()));

    let batch_manager = BatchOperationManager::new(
        kv_store.clone(),
        block_store.clone(),
        tx_store.clone(),
        state_store.clone(),
    );

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

    // Benchmark committing a block with transactions and state changes
    b.iter(|| {
        // Create a new block with a different height for each iteration
        let mut new_block = block.clone();
        new_block.height = test::black_box(new_block.height + 1);
        new_block.hash[0] = test::black_box(new_block.hash[0].wrapping_add(1));

        // Create new transactions with different IDs
        let mut new_tx1 = tx1.clone();
        new_tx1.tx_id[0] = test::black_box(new_tx1.tx_id[0].wrapping_add(1));
        new_tx1.block_height = new_block.height;

        let mut new_tx2 = tx2.clone();
        new_tx2.tx_id[0] = test::black_box(new_tx2.tx_id[0].wrapping_add(1));
        new_tx2.block_height = new_block.height;

        batch_manager.commit_block(&new_block, &[new_tx1, new_tx2], &state_changes).unwrap();
    });
}

/// Benchmark for RocksDBManager get_stats
#[bench]
fn bench_rocksdb_manager_get_stats(b: &mut Bencher) {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let store = RocksDBStore::new(temp_dir.path()).unwrap();
    let manager = RocksDBManager::new(&store);

    // Add some data
    store.put(Schema::block_by_height_key(1).as_bytes(), &[1]).unwrap();
    store.put(Schema::block_by_height_key(2).as_bytes(), &[2]).unwrap();
    store.put(Schema::tx_by_hash_key(&[1u8; 32]).as_bytes(), &[3]).unwrap();
    store.put(Schema::account_state_key(&[2u8; 32]).as_bytes(), &[4]).unwrap();

    // Benchmark getting database stats
    b.iter(|| {
        manager.get_stats().unwrap();
    });
}
