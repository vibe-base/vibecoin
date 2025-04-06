// VibeCoin Transaction Pool (Mempool) Module
//
// This module provides a thread-safe, prioritized pool for transactions:
// - Accepts verified transactions from users/network
// - Queues them for block producers (miners/validators)
// - Prevents spam and invalid transaction propagation

pub mod pool;
pub mod types;

pub use pool::{Mempool, MempoolError};
pub use types::{TransactionRecord, TransactionStatus};
