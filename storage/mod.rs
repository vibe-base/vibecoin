//! Storage module for VibeCoin blockchain
//!
//! This module provides a comprehensive storage layer for the blockchain,
//! including block storage, transaction storage, account state storage, and PoH entries.
//!
//! The storage module is built on top of RocksDB, a high-performance key-value store.
//! It provides a modular design with separate components for different types of data:
//!
//! - `kv_store`: Low-level key-value store interface and RocksDB implementation
//! - `block_store`: Storage for blockchain blocks
//! - `tx_store`: Storage for transactions
//! - `state_store`: Storage for account states
//! - `poh_store`: Storage for Proof of History entries
//! - `trie`: Merkle Patricia Trie implementation for state verification (optional)

pub mod kv_store;
pub mod block_store;
pub mod tx_store;
pub mod state_store;
pub mod poh_store;
pub mod trie;
pub mod batch_operations;
pub mod rocksdb_schema;
pub mod state_manager;
pub mod mempool;
pub mod snapshot;
pub mod state;
pub mod state_validation;
pub mod state_pruning;
pub mod state_sharding;
pub mod state_sync;

// Re-export common types
pub use block_store::{Block, Hash, BlockStore};
pub use tx_store::{TransactionRecord, TransactionStatus, TransactionError, TxStore};
pub use state_store::StateStore;
pub use kv_store::{KVStore, KVStoreError, RocksDBStore, WriteBatchOperation};
pub use poh_store::{PoHEntry, PoHStore};
pub use trie::mpt::{MerklePatriciaTrie, Proof, ProofItem};
pub use trie::node::Node;
pub use batch_operations::{BatchOperationManager, BatchOperationError};
pub use rocksdb_schema::{Schema, KeyType, RocksDBManager, DatabaseStats};
pub use state_manager::StateManager;
pub use mempool::{MempoolStore, MempoolTransactionMetadata, MempoolStorageError};
pub use snapshot::{SnapshotManager, SnapshotConfig, SnapshotMetadata, SnapshotType, CompressionType, SnapshotError};
pub use state::{AccountState, AccountType, StateRoot, StateError, StateResult, GlobalState, ChainParameters, StateTransition, AccountChange, AccountChangeType};
pub use state_validation::{StateValidator, ValidationError, ValidationResult};
pub use state_pruning::{StatePruner, PruningMode, PrunerConfig, PruningError, PruningResult, PruningStats};
pub use state_sharding::{StateShardingManager, StateShard, ShardingStrategy, ShardConfig, ShardingError, ShardingResult};
pub use state_sync::{StateSynchronizer, SyncMode, SyncConfig, SyncStatus, SyncProgress, SyncError, SyncResult, NetworkClient};