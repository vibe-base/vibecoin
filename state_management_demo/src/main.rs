use state_management_demo::storage::{
    RocksDBStore, BlockStore, StateStore,
    AccountState, AccountType, StateRoot,
    StateValidator, ValidationError, ValidationResult,
    StatePruner, PruningMode, PrunerConfig, PruningResult,
    StateShardingManager, ShardingStrategy, ShardConfig, ShardingError,
    StateIndexingManager, IndexType, IndexConfig, IndexingStatus,
};

fn main() {
    println!("VibeCoin State Management Demo");
    println!("------------------------------");
    
    // Create a temporary directory for the database
    let temp_dir = tempfile::tempdir().unwrap();
    println!("Created temporary database at: {}", temp_dir.path().display());
    
    // Create a RocksDB store
    let kv_store = RocksDBStore::new(temp_dir.path()).unwrap();
    println!("Initialized RocksDB store");
    
    // Create block and state stores
    let block_store = BlockStore::new(&kv_store);
    let state_store = StateStore::new(&kv_store);
    println!("Initialized block and state stores");
    
    // Create test accounts
    let height = 100;
    println!("Creating test accounts at block height {}", height);
    
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
        println!("Created {} account with balance {}", account_type, account.balance);
    }
    
    // Create a state validator
    println!("\nInitializing state validator");
    let validator = StateValidator::new(Default::default());
    
    // Create a state pruner
    println!("\nInitializing state pruner");
    let pruner_config = PrunerConfig {
        mode: PruningMode::KeepLastNBlocks(5),
        max_batch_size: 100,
        compact_after_pruning: true,
    };
    
    let pruner = StatePruner::new(&kv_store, &state_store, &block_store, pruner_config);
    
    // Create a state sharding manager
    println!("\nInitializing state sharding manager");
    let sharding_config = ShardConfig {
        shard_count: 4,
        strategy: ShardingStrategy::AddressPrefix,
        separate_databases: false,
        base_dir: None,
        custom_shard_fn: None,
    };
    
    let sharding_manager = StateShardingManager::new(&kv_store, &state_store, &block_store, sharding_config);
    sharding_manager.init().unwrap();
    
    // Create a state indexing manager
    println!("\nInitializing state indexing manager");
    let indexing_manager = StateIndexingManager::new(&kv_store, &state_store, &block_store);
    
    // Create a balance index
    let balance_config = IndexConfig {
        name: "balance_index".to_string(),
        index_type: IndexType::Balance,
        historical: true,
        custom_index_fn: None,
    };
    
    indexing_manager.create_index("balance_index", balance_config).unwrap();
    println!("Created balance index");
    
    // Create an account type index
    let account_type_config = IndexConfig {
        name: "account_type_index".to_string(),
        index_type: IndexType::AccountType,
        historical: true,
        custom_index_fn: None,
    };
    
    indexing_manager.create_index("account_type_index", account_type_config).unwrap();
    println!("Created account type index");
    
    // Index all accounts
    println!("\nIndexing accounts");
    for i in 0..10 {
        let mut address = [0; 32];
        address[0] = i;
        
        if let Some(account) = state_store.get_account(&address, height).unwrap() {
            indexing_manager.index_account(&address, &account, height).unwrap();
            println!("Indexed account with address: {}", hex::encode([i]));
        }
    }
    
    // Query accounts by balance range
    println!("\nQuerying accounts by balance range (3000-5000)");
    let balance_index = indexing_manager.get_index("balance_index").unwrap();
    let results = balance_index.query_by_balance_range(3000, 5000, 10, None).unwrap();
    
    for (address, account) in results {
        println!("Found account with address {} and balance {}", hex::encode(&address[0..1]), account.balance);
    }
    
    // Query accounts by account type
    println!("\nQuerying accounts by type (User)");
    let account_type_index = indexing_manager.get_index("account_type_index").unwrap();
    let results = account_type_index.query_by_account_type("User", 10, None).unwrap();
    
    for (address, account) in results {
        println!("Found User account with address {} and balance {}", hex::encode(&address[0..1]), account.balance);
    }
    
    println!("\nState management demo completed successfully!");
}
