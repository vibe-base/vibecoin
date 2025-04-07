use std::sync::Arc;
use tempfile::tempdir;

use vibecoin::storage::{
    RocksDBStore, BlockStore, StateStore, StateManager,
    Block, AccountState, AccountType, StateRoot,
    StateValidator, ValidationError, ValidationResult,
    StatePruner, PruningMode, PrunerConfig, PruningError, PruningResult,
    StateShardingManager, ShardingStrategy, ShardConfig, ShardingError,
    StateSynchronizer, SyncMode, SyncConfig, SyncStatus,
    StateIndexingManager, IndexType, IndexConfig, IndexingStatus,
};

#[test]
fn test_state_validation() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
    let state_store = StateStore::new(&kv_store);
    
    // Create a state validator with default chain parameters
    let validator = StateValidator::new(Default::default());
    
    // Create test accounts
    let address1 = [1; 32];
    let address2 = [2; 32];
    
    // Create a valid user account
    let account1 = AccountState::new_user(1000, 100);
    state_store.put_account(&address1, &account1, 100).unwrap().unwrap();
    
    // Create a valid contract account
    let code = vec![1, 2, 3, 4]; // Dummy code
    let account2 = AccountState::new_contract(2000, code, 100);
    state_store.put_account(&address2, &account2, 100).unwrap().unwrap();
    
    // Create a state transition with valid account changes
    let prev_state_root = [1; 32];
    let new_state_root = [2; 32];
    let block_height = 100;
    
    // Create a valid account creation change
    let address3 = [3; 32];
    let account3 = AccountState::new_user(500, block_height);
    let creation_change = vibecoin::storage::AccountChange {
        address: address3,
        prev_state: None,
        new_state: Some(account3),
        change_type: vibecoin::storage::AccountChangeType::Created,
    };
    
    // Create a valid account update change
    let mut updated_account1 = account1.clone();
    updated_account1.balance = 900;
    updated_account1.nonce = 1;
    updated_account1.last_updated = 101;
    let update_change = vibecoin::storage::AccountChange {
        address: address1,
        prev_state: Some(account1),
        new_state: Some(updated_account1),
        change_type: vibecoin::storage::AccountChangeType::Updated,
    };
    
    // Create a state transition
    let transition = vibecoin::storage::StateTransition {
        prev_state_root,
        new_state_root,
        block_height,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        account_changes: vec![creation_change, update_change],
    };
    
    // Validate the transition
    let result = validator.validate_transition(&transition);
    assert!(result.is_ok());
}

#[test]
fn test_state_pruning() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
    let block_store = BlockStore::new(&kv_store);
    let state_store = StateStore::new(&kv_store);
    
    // Create test blocks and state
    for i in 0..10 {
        let block = Block {
            height: i,
            hash: [i as u8; 32],
            prev_hash: if i == 0 { [0; 32] } else { [(i-1) as u8; 32] },
            timestamp: 12345 + i * 10,
            transactions: vec![[i as u8 + 100; 32]],
            state_root: [i as u8 + 50; 32],
            tx_root: [i as u8 + 60; 32],
            nonce: 42,
            poh_seq: 100 + i,
            poh_hash: [i as u8 + 70; 32],
            difficulty: 1000,
            total_difficulty: 1000 + i * 100,
        };
        
        block_store.put_block(&block).unwrap();
        
        // Create a state root for this block
        let state_root = StateRoot::new([i as u8 + 50; 32], i, 12345 + i * 10);
        state_store.put_state_root(&state_root).unwrap().unwrap();
        
        // Create some accounts for this block
        let address1 = [i as u8; 32];
        let account1 = AccountState::new_user(1000 + i * 100, i);
        state_store.put_account(&address1, &account1, i).unwrap().unwrap();
        
        let address2 = [i as u8 + 10; 32];
        let account2 = AccountState::new_user(2000 + i * 100, i);
        state_store.put_account(&address2, &account2, i).unwrap().unwrap();
    }
    
    // Create a pruner with KeepLastNBlocks mode
    let config = PrunerConfig {
        mode: PruningMode::KeepLastNBlocks(5),
        max_batch_size: 100,
        compact_after_pruning: true,
    };
    
    let pruner = StatePruner::new(&kv_store, &state_store, &block_store, config);
    
    // Prune the state
    let stats = pruner.prune().unwrap();
    
    // Verify pruning results
    assert_eq!(stats.total_heights, 10);
    assert_eq!(stats.heights_kept, 6); // 0 (genesis) and 5-9
    assert_eq!(stats.heights_pruned, 4); // 1-4
    
    // Verify state roots were pruned
    for i in 1..5 {
        let root = state_store.get_state_root_at_height(i).unwrap();
        assert!(root.is_none());
    }
    
    // Verify state roots were kept
    for i in 5..10 {
        let root = state_store.get_state_root_at_height(i).unwrap();
        assert!(root.is_some());
    }
    
    // Verify genesis state root was kept
    let root = state_store.get_state_root_at_height(0).unwrap();
    assert!(root.is_some());
}

#[test]
fn test_state_sharding() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
    let block_store = BlockStore::new(&kv_store);
    let state_store = StateStore::new(&kv_store);
    
    // Create a sharding manager with address prefix strategy
    let config = ShardConfig {
        shard_count: 4,
        strategy: ShardingStrategy::AddressPrefix,
        separate_databases: false,
        base_dir: None,
        custom_shard_fn: None,
    };
    
    let manager = StateShardingManager::new(&kv_store, &state_store, &block_store, config);
    
    // Initialize sharding
    manager.init().unwrap();
    
    // Create test accounts with different address prefixes
    for i in 0..8 {
        let mut address = [0; 32];
        address[0] = i;
        
        let account = AccountState::new_user(1000 + i * 100, 100);
        
        // Put account in the appropriate shard
        manager.put_account(&address, &account).unwrap();
    }
    
    // Verify accounts were stored in the correct shards
    for i in 0..8 {
        let mut address = [0; 32];
        address[0] = i;
        
        // Get account from the sharding manager
        let account = manager.get_account(&address).unwrap().unwrap();
        assert_eq!(account.balance, 1000 + i * 100);
        
        // Get the shard for this address
        let shard_id = i % 4;
        let shard = manager.get_shard(shard_id).unwrap();
        
        // Verify the account is in this shard
        let shard_account = shard.get_account(&address).unwrap().unwrap();
        assert_eq!(shard_account.balance, 1000 + i * 100);
    }
    
    // Get shard info
    let info = manager.get_all_shard_info().unwrap();
    assert_eq!(info.len(), 4);
    
    // Each shard should have 2 accounts
    for shard_info in info {
        assert_eq!(shard_info.account_count, 2);
    }
}

#[test]
fn test_state_indexing() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
    let block_store = BlockStore::new(&kv_store);
    let state_store = StateStore::new(&kv_store);
    
    // Create an indexing manager
    let manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);
    
    // Create a balance index
    let balance_config = IndexConfig {
        name: "balance_index".to_string(),
        index_type: IndexType::Balance,
        historical: true,
        custom_index_fn: None,
    };
    
    manager.create_index("balance_index", balance_config).unwrap();
    
    // Create an account type index
    let account_type_config = IndexConfig {
        name: "account_type_index".to_string(),
        index_type: IndexType::AccountType,
        historical: true,
        custom_index_fn: None,
    };
    
    manager.create_index("account_type_index", account_type_config).unwrap();
    
    // Create test accounts
    let height = 100;
    
    // Create accounts with different balances and types
    for i in 0..10 {
        let mut address = [0; 32];
        address[0] = i;
        
        let account_type = match i % 3 {
            0 => AccountType::User,
            1 => AccountType::Contract,
            _ => AccountType::Validator,
        };
        
        let account = match account_type {
            AccountType::User => AccountState::new_user(1000 * (i + 1), height),
            AccountType::Contract => {
                let code = vec![1, 2, 3, 4]; // Dummy code
                AccountState::new_contract(1000 * (i + 1), code, height)
            },
            AccountType::Validator => AccountState::new_validator(1000 * (i + 1), 1000, height),
            _ => AccountState::new_user(1000 * (i + 1), height),
        };
        
        // Store the account
        state_store.put_account(&address, &account, height).unwrap().unwrap();
        
        // Index the account
        manager.index_account(&address, &account, height).unwrap();
    }
    
    // Query by balance range
    let balance_index = manager.get_index("balance_index").unwrap();
    let results = balance_index.query_by_balance_range(3000, 5000, 10, None).unwrap();
    assert_eq!(results.len(), 3); // Accounts with balances 3000, 4000, 5000
    
    // Query by account type
    let account_type_index = manager.get_index("account_type_index").unwrap();
    let results = account_type_index.query_by_account_type("User", 10, None).unwrap();
    assert_eq!(results.len(), 4); // 4 user accounts (i = 0, 3, 6, 9)
    
    let results = account_type_index.query_by_account_type("Contract", 10, None).unwrap();
    assert_eq!(results.len(), 3); // 3 contract accounts (i = 1, 4, 7)
    
    let results = account_type_index.query_by_account_type("Validator", 10, None).unwrap();
    assert_eq!(results.len(), 3); // 3 validator accounts (i = 2, 5, 8)
}
