use std::fs;
use std::path::Path;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use log::{info, warn, error};

use crate::storage::block_store::{Block, Hash};
use crate::consensus::types::BlockHeader;
use crate::storage::state::{AccountState, AccountType};
use crate::crypto::hash::sha256;

/// Genesis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    /// Chain ID
    pub chain_id: u64,

    /// Genesis timestamp
    pub timestamp: u64,

    /// Initial difficulty
    pub initial_difficulty: u64,

    /// Initial accounts
    pub initial_accounts: HashMap<String, GenesisAccount>,
}

/// Genesis account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisAccount {
    /// Initial balance
    pub balance: u64,

    /// Account type
    pub account_type: String,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        let mut initial_accounts = HashMap::new();

        // Add some initial accounts
        initial_accounts.insert(
            "0000000000000000000000000000000000000000000000000000000000000001".to_string(),
            GenesisAccount {
                balance: 1_000_000_000,
                account_type: "User".to_string(),
            },
        );

        initial_accounts.insert(
            "0000000000000000000000000000000000000000000000000000000000000002".to_string(),
            GenesisAccount {
                balance: 1_000_000_000,
                account_type: "User".to_string(),
            },
        );

        Self {
            chain_id: 1,
            timestamp: 1609459200, // 2021-01-01 00:00:00 UTC
            initial_difficulty: 1000,
            initial_accounts,
        }
    }
}

impl GenesisConfig {
    /// Load genesis configuration from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let config_str = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read genesis config file: {}", e))?;

        let config: GenesisConfig = toml::from_str(&config_str)
            .map_err(|e| format!("Failed to parse genesis config file: {}", e))?;

        Ok(config)
    }

    /// Save genesis configuration to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let config_str = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize genesis config: {}", e))?;

        fs::write(path, config_str)
            .map_err(|e| format!("Failed to write genesis config file: {}", e))?;

        Ok(())
    }

    /// Generate a default genesis configuration file if it doesn't exist
    pub fn generate_default<P: AsRef<Path>>(path: P) -> Result<(), String> {
        let path = path.as_ref();

        if path.exists() {
            info!("Genesis config file already exists at {:?}", path);
            return Ok(());
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create genesis config directory: {}", e))?;
            }
        }

        // Create default config
        let config = GenesisConfig::default();

        // Save config
        config.save(path)?;

        info!("Generated default genesis config at {:?}", path);
        Ok(())
    }

    /// Generate a genesis block from the configuration
    pub fn generate_block(&self) -> Block {
        // Create an empty block
        let mut block = Block {
            height: 0,
            hash: [0; 32],
            prev_hash: [0; 32],
            timestamp: self.timestamp,
            transactions: vec![],
            state_root: [0; 32], // Will be calculated later
            tx_root: [0; 32],    // Empty transaction root
            nonce: 0,
            poh_seq: 0,
            poh_hash: [0; 32],
            difficulty: self.initial_difficulty,
            total_difficulty: self.initial_difficulty as u128,
        };

        // Calculate the block hash
        let block_data = format!(
            "{}:{}:{}:{}:{}:{}",
            block.height,
            hex::encode(&block.prev_hash),
            block.timestamp,
            hex::encode(&block.state_root),
            hex::encode(&block.tx_root),
            block.nonce
        );

        block.hash = sha256(block_data.as_bytes());

        block
    }

    /// Generate initial account states from the configuration
    pub fn generate_account_states(&self) -> Vec<(Hash, AccountState)> {
        let mut account_states = Vec::new();

        for (address_hex, account) in &self.initial_accounts {
            // Parse the address
            let address_bytes = hex::decode(address_hex)
                .expect("Invalid address hex in genesis config");

            let mut address = [0u8; 32];
            address.copy_from_slice(&address_bytes);

            // Parse the account type
            let account_type = match account.account_type.as_str() {
                "User" => AccountType::User,
                "Contract" => AccountType::Contract,
                "System" => AccountType::System,
                _ => {
                    warn!("Unknown account type: {}, defaulting to User", account.account_type);
                    AccountType::User
                }
            };

            // Create the account state
            let state = match account_type {
                AccountType::User => AccountState::new_user(account.balance, 0),
                AccountType::Contract => AccountState::new_contract(account.balance, Vec::new(), 0),
                AccountType::System => AccountState::new_system(account.balance, 0),
                AccountType::Validator => AccountState::new_validator(account.balance, account.balance, 0),
            };

            account_states.push((address, state));
        }

        account_states
    }
}

/// Generate a genesis block and initial account states
pub fn generate_genesis<P: AsRef<Path>>(config_path: P) -> Result<(Block, Vec<(Hash, AccountState)>), String> {
    // Load the genesis configuration
    let config = GenesisConfig::load(config_path)?;

    // Generate the genesis block
    let block = config.generate_block();

    // Generate the initial account states
    let account_states = config.generate_account_states();

    Ok((block, account_states))
}
