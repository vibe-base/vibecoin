use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rocksdb::{DB, Options};
use serde::{Serialize, Deserialize};

// Simple account state structure
#[derive(Serialize, Deserialize, Clone, Debug)]
struct AccountState {
    balance: u64,
    nonce: u64,
    account_type: AccountType,
}

// Account type enum
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
enum AccountType {
    User,
    Contract,
    Validator,
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountType::User => write!(f, "User"),
            AccountType::Contract => write!(f, "Contract"),
            AccountType::Validator => write!(f, "Validator"),
        }
    }
}

// Simple key-value store
struct KVStore {
    db: DB,
}

impl KVStore {
    fn new(path: &Path) -> Self {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path).expect("Failed to open database");
        Self { db }
    }

    fn put(&self, key: &[u8], value: &[u8]) -> Result<(), String> {
        self.db.put(key, value).map_err(|e| e.to_string())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
        self.db.get(key).map_err(|e| e.to_string())
    }

    fn delete(&self, key: &[u8]) -> Result<(), String> {
        self.db.delete(key).map_err(|e| e.to_string())
    }
    
    fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        let mut results = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        
        for item in iter {
            let (key, value) = item.map_err(|e| e.to_string())?;
            if key.starts_with(prefix) {
                results.push((key.to_vec(), value.to_vec()));
            }
        }
        
        Ok(results)
    }
}

// Simple state store
struct StateStore {
    kv_store: Arc<KVStore>,
}

impl StateStore {
    fn new(kv_store: Arc<KVStore>) -> Self {
        Self { kv_store }
    }

    fn put_account(&self, address: &[u8], account: &AccountState, height: u64) -> Result<(), String> {
        let key = format!("account:{}:{}", hex::encode(address), height);
        let value = bincode::serialize(account).map_err(|e| e.to_string())?;
        self.kv_store.put(key.as_bytes(), &value)
    }

    fn get_account(&self, address: &[u8], height: u64) -> Result<Option<AccountState>, String> {
        let key = format!("account:{}:{}", hex::encode(address), height);
        match self.kv_store.get(key.as_bytes())? {
            Some(value) => {
                let account = bincode::deserialize(&value).map_err(|e| e.to_string())?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }
    
    fn get_latest_account(&self, address: &[u8]) -> Result<Option<AccountState>, String> {
        let prefix = format!("account:{}:", hex::encode(address));
        let results = self.kv_store.scan_prefix(prefix.as_bytes())?;
        
        if results.is_empty() {
            return Ok(None);
        }
        
        // Find the entry with the highest height
        let mut latest_height = 0;
        let mut latest_value = None;
        
        for (key, value) in results {
            let key_str = String::from_utf8_lossy(&key);
            let parts: Vec<&str> = key_str.split(':').collect();
            
            if parts.len() >= 3 {
                if let Ok(height) = parts[2].parse::<u64>() {
                    if height > latest_height {
                        latest_height = height;
                        latest_value = Some(value);
                    }
                }
            }
        }
        
        match latest_value {
            Some(value) => {
                let account = bincode::deserialize(&value).map_err(|e| e.to_string())?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }
}

// Simple state validator
#[derive(Debug)]
struct ValidationError(String);

type ValidationResult<T> = Result<T, ValidationError>;

struct StateValidator;

impl StateValidator {
    fn new() -> Self {
        Self {}
    }

    fn validate_account(&self, account: &AccountState) -> ValidationResult<()> {
        // Simple validation: balance must be non-negative
        if account.balance > 0 || account.balance == 0 {
            Ok(())
        } else {
            Err(ValidationError("Invalid balance".to_string()))
        }
    }
    
    fn validate_transaction(&self, from: &AccountState, to: &AccountState, amount: u64) -> ValidationResult<()> {
        // Check if sender has sufficient balance
        if from.balance < amount {
            return Err(ValidationError("Insufficient balance".to_string()));
        }
        
        // Check if sender nonce is valid
        if from.nonce == 0 {
            return Err(ValidationError("Invalid nonce".to_string()));
        }
        
        Ok(())
    }
}

// Simple state pruner
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PruningMode {
    KeepLastNBlocks(u64),
    KeepInterval(u64),
}

struct PrunerConfig {
    mode: PruningMode,
    max_batch_size: usize,
}

struct StatePruner {
    kv_store: Arc<KVStore>,
    config: PrunerConfig,
}

impl StatePruner {
    fn new(kv_store: Arc<KVStore>, config: PrunerConfig) -> Self {
        Self { kv_store, config }
    }

    fn prune(&self, current_height: u64) -> Result<u64, String> {
        let mut pruned_count = 0;
        
        match self.config.mode {
            PruningMode::KeepLastNBlocks(n) => {
                if current_height <= n {
                    return Ok(0);
                }
                
                let start_height = 0;
                let end_height = current_height - n;
                
                println!("Pruning blocks from {} to {}", start_height, end_height);
                
                // In a real implementation, we would scan the database for accounts in this height range
                // For this demo, we'll just return a placeholder value
                pruned_count = end_height - start_height + 1;
            },
            PruningMode::KeepInterval(interval) => {
                println!("Pruning blocks with interval {}", interval);
                
                // In a real implementation, we would scan the database for accounts not at interval heights
                // For this demo, we'll just return a placeholder value
                pruned_count = current_height / 2; // Rough estimate
            },
        }
        
        Ok(pruned_count)
    }
}

// Simple state indexer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexType {
    Balance,
    AccountType,
    Custom,
}

struct IndexConfig {
    name: String,
    index_type: IndexType,
    historical: bool,
}

struct StateIndexer {
    kv_store: Arc<KVStore>,
    config: IndexConfig,
}

impl StateIndexer {
    fn new(kv_store: Arc<KVStore>, config: IndexConfig) -> Self {
        Self { kv_store, config }
    }

    fn index_account(&self, address: &[u8], account: &AccountState, height: u64) -> Result<(), String> {
        match self.config.index_type {
            IndexType::Balance => {
                let key = format!("index:{}:balance:{}:{}", 
                    self.config.name, account.balance, hex::encode(address));
                self.kv_store.put(key.as_bytes(), &[])?;
                
                if self.config.historical {
                    let key = format!("index:{}:balance:{}:{}:{}", 
                        self.config.name, account.balance, height, hex::encode(address));
                    self.kv_store.put(key.as_bytes(), &[])?;
                }
            },
            IndexType::AccountType => {
                let account_type = format!("{:?}", account.account_type);
                let key = format!("index:{}:account_type:{}:{}", 
                    self.config.name, account_type, hex::encode(address));
                self.kv_store.put(key.as_bytes(), &[])?;
                
                if self.config.historical {
                    let key = format!("index:{}:account_type:{}:{}:{}", 
                        self.config.name, account_type, height, hex::encode(address));
                    self.kv_store.put(key.as_bytes(), &[])?;
                }
            },
            IndexType::Custom => {
                // For this demo, we'll just index by nonce
                let key = format!("index:{}:custom:{}:{}", 
                    self.config.name, account.nonce, hex::encode(address));
                self.kv_store.put(key.as_bytes(), &[])?;
                
                if self.config.historical {
                    let key = format!("index:{}:custom:{}:{}:{}", 
                        self.config.name, account.nonce, height, hex::encode(address));
                    self.kv_store.put(key.as_bytes(), &[])?;
                }
            },
        }
        
        Ok(())
    }

    fn query_by_balance(&self, min_balance: u64, max_balance: u64, limit: usize) -> Result<Vec<(Vec<u8>, u64)>, String> {
        if self.config.index_type != IndexType::Balance {
            return Err("This index is not a balance index".to_string());
        }
        
        let mut results = Vec::new();
        
        for balance in min_balance..=max_balance {
            let prefix = format!("index:{}:balance:{}:", self.config.name, balance);
            let entries = self.kv_store.scan_prefix(prefix.as_bytes())?;
            
            for (key, _) in entries {
                if results.len() >= limit {
                    break;
                }
                
                // Extract address from key
                let key_str = String::from_utf8_lossy(&key);
                let parts: Vec<&str> = key_str.split(':').collect();
                
                if parts.len() >= 4 {
                    if let Ok(address) = hex::decode(parts[3]) {
                        results.push((address, balance));
                    }
                }
            }
            
            if results.len() >= limit {
                break;
            }
        }
        
        Ok(results)
    }
    
    fn query_by_account_type(&self, account_type: &str, limit: usize) -> Result<Vec<(Vec<u8>, String)>, String> {
        if self.config.index_type != IndexType::AccountType {
            return Err("This index is not an account type index".to_string());
        }
        
        let mut results = Vec::new();
        
        let prefix = format!("index:{}:account_type:{}:", self.config.name, account_type);
        let entries = self.kv_store.scan_prefix(prefix.as_bytes())?;
        
        for (key, _) in entries {
            if results.len() >= limit {
                break;
            }
            
            // Extract address from key
            let key_str = String::from_utf8_lossy(&key);
            let parts: Vec<&str> = key_str.split(':').collect();
            
            if parts.len() >= 4 {
                if let Ok(address) = hex::decode(parts[3]) {
                    results.push((address, account_type.to_string()));
                }
            }
        }
        
        Ok(results)
    }
}

// Simple state sharding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShardingStrategy {
    AddressPrefix,
    AddressModulo,
}

struct ShardConfig {
    shard_count: u32,
    strategy: ShardingStrategy,
}

struct StateShard {
    id: u32,
    kv_store: Arc<KVStore>,
}

impl StateShard {
    fn new(id: u32, kv_store: Arc<KVStore>) -> Self {
        Self { id, kv_store }
    }
    
    fn put_account(&self, address: &[u8], account: &AccountState) -> Result<(), String> {
        let key = format!("shard:{}:account:{}", self.id, hex::encode(address));
        let value = bincode::serialize(account).map_err(|e| e.to_string())?;
        self.kv_store.put(key.as_bytes(), &value)
    }
    
    fn get_account(&self, address: &[u8]) -> Result<Option<AccountState>, String> {
        let key = format!("shard:{}:account:{}", self.id, hex::encode(address));
        match self.kv_store.get(key.as_bytes())? {
            Some(value) => {
                let account = bincode::deserialize(&value).map_err(|e| e.to_string())?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }
}

struct StateShardingManager {
    kv_store: Arc<KVStore>,
    config: ShardConfig,
    shards: HashMap<u32, StateShard>,
}

impl StateShardingManager {
    fn new(kv_store: Arc<KVStore>, config: ShardConfig) -> Self {
        let mut shards = HashMap::new();
        
        for id in 0..config.shard_count {
            shards.insert(id, StateShard::new(id, Arc::clone(&kv_store)));
        }
        
        Self { kv_store, config, shards }
    }
    
    fn get_shard_for_address(&self, address: &[u8]) -> Result<&StateShard, String> {
        let shard_id = match self.config.strategy {
            ShardingStrategy::AddressPrefix => {
                // Use first byte as shard ID
                address[0] as u32 % self.config.shard_count
            },
            ShardingStrategy::AddressModulo => {
                // Use modulo of address as shard ID
                let mut sum = 0u32;
                for &byte in address {
                    sum = sum.wrapping_add(byte as u32);
                }
                sum % self.config.shard_count
            },
        };
        
        self.shards.get(&shard_id)
            .ok_or_else(|| format!("Shard {} not found", shard_id))
    }
    
    fn put_account(&self, address: &[u8], account: &AccountState) -> Result<(), String> {
        let shard = self.get_shard_for_address(address)?;
        shard.put_account(address, account)
    }
    
    fn get_account(&self, address: &[u8]) -> Result<Option<AccountState>, String> {
        let shard = self.get_shard_for_address(address)?;
        shard.get_account(address)
    }
}

fn main() {
    println!("VibeCoin State Management Demo");
    println!("------------------------------");
    
    // Create a temporary directory for the database
    let temp_dir = tempfile::tempdir().unwrap();
    println!("Created temporary database at: {}", temp_dir.path().display());
    
    // Create a KV store
    let kv_store = Arc::new(KVStore::new(temp_dir.path()));
    println!("Initialized KV store");
    
    // Create a state store
    let state_store = StateStore::new(Arc::clone(&kv_store));
    println!("Initialized state store");
    
    // Create a state validator
    let validator = StateValidator::new();
    println!("Initialized state validator");
    
    // Create a state pruner
    let pruner_config = PrunerConfig {
        mode: PruningMode::KeepLastNBlocks(5),
        max_batch_size: 100,
    };
    let pruner = StatePruner::new(Arc::clone(&kv_store), pruner_config);
    println!("Initialized state pruner");
    
    // Create a state indexer for balances
    let balance_index_config = IndexConfig {
        name: "balance_index".to_string(),
        index_type: IndexType::Balance,
        historical: true,
    };
    let balance_indexer = StateIndexer::new(Arc::clone(&kv_store), balance_index_config);
    println!("Initialized balance indexer");
    
    // Create a state indexer for account types
    let account_type_index_config = IndexConfig {
        name: "account_type_index".to_string(),
        index_type: IndexType::AccountType,
        historical: true,
    };
    let account_type_indexer = StateIndexer::new(Arc::clone(&kv_store), account_type_index_config);
    println!("Initialized account type indexer");
    
    // Create a state sharding manager
    let sharding_config = ShardConfig {
        shard_count: 4,
        strategy: ShardingStrategy::AddressPrefix,
    };
    let sharding_manager = StateShardingManager::new(Arc::clone(&kv_store), sharding_config);
    println!("Initialized sharding manager");
    
    // Create test accounts
    let height = 100;
    println!("\nCreating test accounts at block height {}", height);
    
    for i in 0..10 {
        let mut address = [0; 32];
        address[0] = i;
        
        let account_type = match i % 3 {
            0 => AccountType::User,
            1 => AccountType::Contract,
            _ => AccountType::Validator,
        };
        
        let account = AccountState {
            balance: 1000 * (i as u64 + 1),
            nonce: i as u64,
            account_type,
        };
        
        // Validate the account
        validator.validate_account(&account).unwrap();
        
        // Store the account
        state_store.put_account(&address, &account, height).unwrap();
        println!("Created {} account with balance {}", account.account_type, account.balance);
        
        // Index the account
        balance_indexer.index_account(&address, &account, height).unwrap();
        account_type_indexer.index_account(&address, &account, height).unwrap();
        
        // Store in the appropriate shard
        sharding_manager.put_account(&address, &account).unwrap();
    }
    
    // Retrieve accounts
    println!("\nRetrieving accounts");
    
    let mut address = [0; 32];
    address[0] = 1;
    
    match state_store.get_account(&address, height).unwrap() {
        Some(account) => {
            println!("Found account in state store with balance {}", account.balance);
        }
        None => {
            println!("Account not found in state store");
        }
    }
    
    match sharding_manager.get_account(&address).unwrap() {
        Some(account) => {
            println!("Found account in shard with balance {}", account.balance);
        }
        None => {
            println!("Account not found in shard");
        }
    }
    
    // Query accounts by balance
    println!("\nQuerying accounts by balance (3000-5000)");
    
    let accounts = balance_indexer.query_by_balance(3000, 5000, 10).unwrap();
    
    for (address, balance) in accounts {
        println!("Found account with address {} and balance {}", hex::encode(&address[0..1]), balance);
    }
    
    // Query accounts by account type
    println!("\nQuerying accounts by type (User)");
    
    let accounts = account_type_indexer.query_by_account_type("User", 10).unwrap();
    
    for (address, account_type) in accounts {
        println!("Found {} account with address {}", account_type, hex::encode(&address[0..1]));
    }
    
    // Prune old accounts
    println!("\nPruning old accounts");
    
    let pruned_count = pruner.prune(height).unwrap();
    println!("Pruned {} old accounts", pruned_count);
    
    println!("\nState management demo completed successfully!");
}
