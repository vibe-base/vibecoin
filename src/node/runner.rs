use crate::consensus::poh::ProofOfHistory;
use crate::consensus::pow::{self, Difficulty};
use crate::ledger::block::Block;
use crate::ledger::state::Blockchain;
use crate::ledger::transaction::Transaction;
use crate::node::mempool::{TransactionPool, MAX_TRANSACTIONS_PER_BLOCK};
use crate::types::error::VibecoinError;
use crate::types::primitives::PublicKey;
use crate::wallet::keys::Wallet;
use crate::network::server::{NetworkServer, NetworkConfig};
use crate::network::bootstrap::get_bootstrap_addresses;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::net::SocketAddr;
use std::thread;

/// Configuration for the node
pub struct NodeConfig {
    /// Initial difficulty for mining
    pub difficulty: Difficulty,
    /// Whether mining is enabled
    pub mining_enabled: bool,
    /// Target time between blocks (in seconds)
    pub target_block_time: u64,
    /// Whether to mine empty blocks (with no transactions)
    pub mine_empty_blocks: bool,
    /// Maximum age of transactions in mempool (in seconds)
    pub max_tx_age: u64,
    /// PoH ticks per second
    pub poh_ticks_per_second: u64,
    /// Path to the wallet file
    pub wallet_path: String,
    /// Password for the wallet
    pub wallet_password: String,
    /// Whether to enable networking
    pub enable_networking: bool,
    /// Address to listen on for P2P connections
    pub p2p_listen_addr: Option<SocketAddr>,
    /// Seed nodes to connect to
    pub seed_nodes: Vec<SocketAddr>,
    /// Maximum number of inbound connections
    pub max_inbound: usize,
    /// Maximum number of outbound connections
    pub max_outbound: usize,
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
            difficulty: 4,
            mining_enabled: true,
            target_block_time: 10,
            mine_empty_blocks: false,
            max_tx_age: 3600, // 1 hour
            poh_ticks_per_second: 1000,
            wallet_path: "config/miner_wallet.json".to_string(),
            wallet_password: "password".to_string(), // In a real app, this would be securely provided
            enable_networking: false, // Disabled by default for safety
            p2p_listen_addr: Some("127.0.0.1:8333".parse().unwrap()),
            seed_nodes: Vec::new(),
            max_inbound: 125,
            max_outbound: 8,
        }
    }
}

/// Node runner for the blockchain
pub struct Node {
    /// The blockchain
    pub blockchain: Blockchain,
    /// Proof of History generator
    pub poh_generator: ProofOfHistory,
    /// Transaction pool
    pub mempool: TransactionPool,
    /// Node configuration
    pub config: NodeConfig,
    /// Miner wallet for receiving rewards
    pub miner_wallet: Wallet,
    /// Network server for P2P communication
    pub network: Option<NetworkServer>,
    /// Last time a block was mined
    pub last_block_time: Instant,
    /// Whether the node is running
    running: bool,
}

impl Node {
    /// Create a new node with the given configuration
    pub fn new(config: NodeConfig) -> Result<Self, VibecoinError> {
        // Initialize PoH
        let initial_hash = [0u8; 32];
        let poh = ProofOfHistory::new(initial_hash, config.poh_ticks_per_second);

        // Create genesis block
        let genesis = Blockchain::create_genesis_block()?;

        // Load or create the miner wallet
        let miner_wallet = Wallet::load_or_create(&config.wallet_path, &config.wallet_password)?;
        println!("Miner address: {}", hex::encode(miner_wallet.get_address()));

        // Create blockchain
        let blockchain = Blockchain::new(
            genesis,
            config.difficulty,
            poh.clone(),
            miner_wallet.get_address()
        );

        // Create transaction pool
        let mempool = TransactionPool::new();

        // Initialize network server if networking is enabled
        let mut network = if config.enable_networking {
            // Determine seed nodes - use provided ones or bootstrap peers
            let seed_nodes = if config.seed_nodes.is_empty() {
                // Use bootstrap peers if no seed nodes are provided
                get_bootstrap_addresses()
            } else {
                config.seed_nodes.clone()
            };

            println!("Initializing network with {} seed nodes", seed_nodes.len());
            for seed in &seed_nodes {
                println!("  - {}", seed);
            }

            // Create network configuration
            let network_config = NetworkConfig {
                listen_addr: config.p2p_listen_addr.unwrap_or_else(|| "0.0.0.0:8333".parse().unwrap()),
                seed_nodes,
                user_agent: format!("VibeCoin/0.1.0 ({})", config.wallet_path),
            };

            // Create the network server
            let mut server = NetworkServer::new(network_config);
            println!("Network server initialized, listening on {}",
                     config.p2p_listen_addr.unwrap_or_else(|| "0.0.0.0:8333".parse().unwrap()));

            // Set the blockchain reference
            let blockchain_ref = Arc::new(Mutex::new(blockchain.clone()));
            server.set_blockchain(blockchain_ref);

            // Print bootstrap peers
            println!("Bootstrap peers available:");
            for peer in get_bootstrap_addresses() {
                println!("  - {}", peer);
            }

            // Start the network server
            if let Err(e) = server.start() {
                println!("Error starting network server: {}", e);
            } else {
                println!("Network server started successfully");
            }

            Some(server)
        } else {
            println!("Networking disabled");
            None
        };

        Ok(Node {
            blockchain,
            poh_generator: poh,
            mempool,
            config,
            miner_wallet,
            network,
            last_block_time: Instant::now(),
            running: false,
        })
    }

    /// Start the node
    pub fn start(&mut self) -> Result<(), VibecoinError> {
        println!("Starting VibeCoin node...");
        println!("Genesis block: {}", hex::encode(self.blockchain.latest_block().hash));
        println!("Difficulty: {}", self.config.difficulty);
        println!("Mining enabled: {}", self.config.mining_enabled);

        // Network server is already started in the constructor
        if let Some(_) = &self.network {
            println!("Network is enabled");
        }

        self.running = true;
        self.run_main_loop()
    }

    /// Stop the node
    pub fn stop(&mut self) {
        println!("Stopping VibeCoin node...");

        // Stop the network server if enabled
        if let Some(network) = &self.network {
            println!("Stopping network server...");
            network.stop();
        }

        self.running = false;
    }

    /// Main loop for the node
    fn run_main_loop(&mut self) -> Result<(), VibecoinError> {
        while self.running {
            // Update PoH
            self.poh_generator.tick();

            // Check if it's time to mine a new block
            if self.config.mining_enabled &&
               self.last_block_time.elapsed() >= Duration::from_secs(self.config.target_block_time) {
                self.mine_new_block()?;
                self.last_block_time = Instant::now();
            }

            // Clean up old transactions
            let removed = self.mempool.cleanup_old_transactions(self.config.max_tx_age);
            if removed > 0 {
                println!("Removed {} old transactions from mempool", removed);
            }

            // Small sleep to prevent CPU hogging
            thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    }

    /// Mine a new block
    pub fn mine_new_block(&mut self) -> Result<(), VibecoinError> {
        // Select transactions from mempool
        let transactions = self.mempool.select_transactions(MAX_TRANSACTIONS_PER_BLOCK);

        if transactions.is_empty() {
            if !self.config.mine_empty_blocks {
                println!("No transactions to mine and empty blocks disabled");
                return Ok(());
            }

            // If we're mining an empty block, we need to add a coinbase transaction
            // to satisfy the validation rule that blocks can't be empty
            println!("No transactions in mempool, adding a coinbase transaction");
            let coinbase_tx = self.create_coinbase_transaction()?;
            let transactions_with_coinbase = vec![coinbase_tx];
            return self.mine_block_with_transactions(transactions_with_coinbase);
        }

        // Mine block with the selected transactions
        self.mine_block_with_transactions(transactions)
    }

    /// Create a coinbase transaction (reward for mining)
    fn create_coinbase_transaction(&self) -> Result<Transaction, VibecoinError> {
        // Get the miner's address from the wallet
        let miner_address = self.miner_wallet.get_address();
        let reward_amount = 50; // Block reward

        // Create a coinbase transaction (from address is all zeros)
        let mut tx = Transaction::new([0u8; 32], miner_address, reward_amount)?;

        // Use the special signature for coinbase transactions
        tx.signature = Arc::new([2u8; 64]);

        println!("Created coinbase transaction with reward {} to {}",
                 reward_amount, hex::encode(miner_address));

        Ok(tx)
    }

    /// Mine a block with the given transactions
    fn mine_block_with_transactions(&mut self, transactions: Vec<Transaction>) -> Result<(), VibecoinError> {
        // Create a new block
        let latest_block = self.blockchain.latest_block();
        let previous_hash = latest_block.hash;
        let next_index = latest_block.index + 1;

        println!("Creating block {} with previous hash: {}", next_index, hex::encode(previous_hash));
        let mut block = Block::new(next_index, previous_hash, transactions.clone())?;

        println!("Mining block {} with {} transactions...", block.index, transactions.len());
        let start_time = Instant::now();

        // Get the current slot number
        let slot_number = 0; // Default to slot 0

        // Get the miner key
        let miner_key = self.miner_wallet.get_address();

        // Mine the block
        pow::mine_block(
            &mut block,
            self.config.difficulty,
            &mut self.poh_generator,
            slot_number,
            miner_key,
            None,
            Some(Duration::from_secs(60))
        )?;

        let elapsed = start_time.elapsed();
        println!("Block {} mined in {:.2}s!", block.index, elapsed.as_secs_f64());
        println!("Block hash: {}", hex::encode(block.hash));
        println!("Nonce: {}", block.nonce);

        // Add the block to our blockchain
        self.blockchain.add_block(block.clone())?;

        // Remove mined transactions from mempool
        self.mempool.remove_included_transactions(&transactions);

        // Broadcast the block to the network if networking is enabled
        if let Some(network) = &self.network {
            if let Ok(peer_count) = network.broadcast_block(&block.hash) {
                println!("Block broadcast to {} peers", peer_count);
            }
        }

        Ok(())
    }

    /// Add a transaction to the mempool
    pub fn add_transaction(&mut self, transaction: Transaction) -> Result<(), VibecoinError> {
        // Add to mempool
        self.mempool.add_transaction(transaction.clone())?;
        println!("Transaction added to mempool. Pool size: {}", self.mempool.size());

        // Broadcast the transaction to the network if networking is enabled
        if let Some(network) = &self.network {
            if let Ok(peer_count) = network.broadcast_transaction(&transaction.hash) {
                println!("Transaction broadcast to {} peers", peer_count);
            }
        }

        Ok(())
    }

    /// Create a test transaction
    pub fn create_test_transaction(&self, amount: u64) -> Result<Transaction, VibecoinError> {
        let from: PublicKey = [1u8; 32];
        let to: PublicKey = [2u8; 32];

        let mut tx = Transaction::new(from, to, amount)?;
        tx.signature = Arc::new([1u8; 64]); // Set a dummy signature

        Ok(tx)
    }

    /// Get blockchain state summary
    pub fn get_blockchain_state(&self) -> String {
        let state = self.blockchain.get_state_summary();
        format!(
            "Height: {}, Latest hash: {}, Difficulty: {}, PoH count: {}",
            state.height,
            hex::encode(state.latest_hash),
            state.difficulty,
            state.poh_count
        )
    }

    /// Get network status
    pub fn get_network_status(&self) -> String {
        if let Some(network) = &self.network {
            let peer_count = network.get_peer_count();
            let peers = network.get_peer_info();

            let mut status = format!("Connected to {} peers\n", peer_count);

            if !peers.is_empty() {
                status.push_str("Peers:\n");
                for (i, peer) in peers.iter().enumerate() {
                    status.push_str(&format!("  {}. {}\n", i + 1, peer));
                }
            }

            status
        } else {
            "Networking disabled".to_string()
        }
    }
}
