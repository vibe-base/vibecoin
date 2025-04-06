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

// Re-export common types
pub use block_store::{Block, Hash, BlockStore};
pub use tx_store::{TransactionRecord, TransactionStatus, TransactionError, TxStore};
pub use state_store::{AccountState, AccountType, StateRoot, StateStore};
pub use kv_store::{KVStore, KVStoreError, RocksDBStore, WriteBatchOperation};
pub use poh_store::{PoHEntry, PoHStore};