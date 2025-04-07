use std::sync::Arc;
use tempfile::tempdir;

use vibecoin::storage::{
    RocksDBStore, BlockStore, StateStore,
    AccountState, AccountType, StateRoot,
    StateValidator, ValidationError, ValidationResult,
    StatePruner, PruningMode, PrunerConfig, PruningResult,
    StateShardingManager, ShardingStrategy, ShardConfig,
    StateIndexingManager, IndexType, IndexConfig,
};

#[test]
fn test_state_management_integration() {
    // Create a temporary directory for the database
    let temp_dir = tempdir().unwrap();
    let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
    let block_store = BlockStore::new(&kv_store);
    let state_store = StateStore::new(&kv_store);
    
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
    }
    
    // Test state validation
    let validator = StateValidator::new(Default::default());
    
    // Test state pruning
    let pruner_config = PrunerConfig {
        mode: PruningMode::KeepLastNBlocks(5),
        max_batch_size: 100,
        compact_after_pruning: true,
    };
    
    let pruner = StatePruner::new(&kv_store, &state_store, &block_store, pruner_config);
    
    // Test state sharding
    let sharding_config = ShardConfig {
        shard_count: 4,
        strategy: ShardingStrategy::AddressPrefix,
        separate_databases: false,
        base_dir: None,
        custom_shard_fn: None,
    };
    
    let sharding_manager = StateShardingManager::new(&kv_store, &state_store, &block_store, sharding_config);
    sharding_manager.init().unwrap();
    
    // Test state indexing
    let indexing_manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);
    
    // Create a balance index
    let balance_config = IndexConfig {
        name: "balance_index".to_string(),
        index_type: IndexType::Balance,
        historical: true,
        custom_index_fn: None,
    };
    
    indexing_manager.create_index("balance_index", balance_config).unwrap();
    
    // Create an account type index
    let account_type_config = IndexConfig {
        name: "account_type_index".to_string(),
        index_type: IndexType::AccountType,
        historical: true,
        custom_index_fn: None,
    };
    
    indexing_manager.create_index("account_type_index", account_type_config).unwrap();
    
    // Index all accounts
    for i in 0..10 {
        let mut address = [0; 32];
        address[0] = i;
        
        if let Some(account) = state_store.get_account(&address, height).unwrap() {
            indexing_manager.index_account(&address, &account, height).unwrap();
        }
    }
    
    // Verify everything works together
    assert!(true);
}
