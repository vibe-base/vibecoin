//! Development tools for VibeCoin blockchain
//!
//! This module provides development tools for the blockchain,
//! including testing utilities, simulation tools, and benchmarking.

use std::sync::Arc;
use std::time::{Duration, Instant};
use log::{error, info};

use crate::storage::{
    KVStore, BlockStore, StateStore, TxStore,
    Block, AccountState, TransactionRecord,
};
use crate::crypto::keys::VibeKeypair;
use crate::crypto::signer::sign_message;

/// Development tools for blockchain testing and simulation
pub struct DevelopmentTools<'a> {
    /// Block store
    block_store: Arc<BlockStore<'a>>,

    /// State store
    state_store: Arc<StateStore<'a>>,

    /// Transaction store
    tx_store: Arc<TxStore<'a>>,

    /// Key-value store
    kv_store: Arc<dyn KVStore + 'a>,
}

impl<'a> DevelopmentTools<'a> {
    /// Create a new instance of DevelopmentTools
    pub fn new(
        block_store: Arc<BlockStore<'a>>,
        state_store: Arc<StateStore<'a>>,
        tx_store: Arc<TxStore<'a>>,
        kv_store: Arc<dyn KVStore + 'a>,
    ) -> Self {
        Self {
            block_store,
            state_store,
            tx_store,
            kv_store,
        }
    }

    /// Generate a test account
    pub fn generate_test_account(&self, balance: u64) -> (VibeKeypair, [u8; 32]) {
        // Generate a new key pair
        let keypair = VibeKeypair::generate();

        // Derive the address from the public key
        let address = keypair.address();

        // Create a new account state
        let account = AccountState::new_user(balance, 0);

        // Store the account
        match self.state_store.set_account_state(&address, &account) {
            Ok(_) => {
                info!("Created test account with address: {:?}", hex::encode(&address));
            }
            Err(e) => {
                error!("Failed to create test account: {}", e);
            }
        }

        (keypair, address)
    }

    /// Generate a test transaction
    pub fn generate_test_transaction(
        &self,
        sender_keypair: &VibeKeypair,
        sender_address: &[u8; 32],
        receiver_address: &[u8; 32],
        amount: u64,
        fee: u64,
    ) -> Option<TransactionRecord> {
        // Get the sender account
        let sender_account = match self.state_store.get_account_state(sender_address) {
            Some(account) => account,
            None => {
                error!("Sender account not found");
                return None;
            }
        };

        // Check if the sender has sufficient balance
        if sender_account.balance < amount + fee {
            error!("Insufficient balance: {} < {}", sender_account.balance, amount + fee);
            return None;
        }

        // Calculate gas price
        let gas_limit = 21000; // Default gas limit
        let gas_price = fee / gas_limit;

        // Create a transaction record
        let tx_id = [0u8; 32]; // Will be set after signing
        let mut tx = TransactionRecord {
            tx_id,
            sender: *sender_address,
            recipient: *receiver_address,
            value: amount,
            gas_price,
            gas_limit,
            gas_used: 0, // Will be set after execution
            nonce: sender_account.nonce,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            block_height: 0, // Will be set when included in a block
            data: None, // No data for simple transfers
            status: crate::storage::tx_store::TransactionStatus::Pending,
        };

        // Create a message to sign
        let message = format!("{}{}{}{}{}{}",
            hex::encode(&tx.sender),
            hex::encode(&tx.recipient),
            tx.value,
            tx.nonce,
            tx.gas_price,
            tx.gas_limit
        );

        // Sign the message
        let _signature = sign_message(sender_keypair, message.as_bytes());

        // Calculate the transaction ID (hash)
        let tx_hash = crate::crypto::hash::sha256(message.as_bytes());
        tx.tx_id.copy_from_slice(&tx_hash);

        info!("Generated test transaction: {:?}", hex::encode(&tx.tx_id));
        Some(tx)
    }

    /// Generate a test block
    pub fn generate_test_block(&self, transactions: Vec<TransactionRecord>, _miner: &[u8; 32]) -> Option<Block> {
        // Get the latest block
        let latest_block = match self.block_store.get_latest_block() {
            Ok(Some(block)) => block,
            Ok(None) => {
                error!("No blocks found, cannot generate a new block");
                return None;
            }
            Err(e) => {
                error!("Failed to get latest block: {}", e);
                return None;
            }
        };

        // Create a new block
        let mut block = Block {
            height: latest_block.height + 1,
            hash: [0; 32], // Will be calculated later
            prev_hash: latest_block.hash,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            transactions: transactions.iter().map(|tx| tx.tx_id).collect(),
            state_root: [0; 32], // Will be calculated later
            tx_root: [0; 32], // Will be calculated later
            nonce: 0, // Will be set during mining
            poh_seq: latest_block.poh_seq + 1,
            poh_hash: [0; 32], // Will be calculated later
            difficulty: latest_block.difficulty, // Use the same difficulty for testing
            total_difficulty: latest_block.total_difficulty + latest_block.difficulty as u128,
        };

        // Calculate the block hash (simplified for testing)
        let block_data = bincode::serialize(&block).unwrap();
        let hash = crate::crypto::hash::sha256(&block_data);
        block.hash.copy_from_slice(&hash);

        info!("Generated test block: height={}, hash={:?}", block.height, hex::encode(&block.hash));
        Some(block)
    }

    /// Run a benchmark
    pub fn run_benchmark(&self, operation: &str, iterations: u32) -> Duration {
        info!("Running benchmark for {} ({} iterations)", operation, iterations);

        let start = Instant::now();

        match operation {
            "block_creation" => {
                for _ in 0..iterations {
                    // Generate a test block with no transactions
                    let miner = [0; 32];
                    self.generate_test_block(Vec::new(), &miner);
                }
            }
            "account_creation" => {
                for _ in 0..iterations {
                    // Generate a test account
                    self.generate_test_account(1000);
                }
            }
            "transaction_creation" => {
                // Generate a sender account
                let (sender_keypair, sender_address) = self.generate_test_account(1000 * iterations as u64);

                // Generate a receiver account
                let (_, receiver_address) = self.generate_test_account(0);

                for _ in 0..iterations {
                    // Generate a test transaction
                    self.generate_test_transaction(
                        &sender_keypair,
                        &sender_address,
                        &receiver_address,
                        10,
                        1,
                    );
                }
            }
            _ => {
                error!("Unknown benchmark operation: {}", operation);
            }
        }

        let duration = start.elapsed();
        info!("Benchmark completed in {:?}", duration);

        duration
    }
}
