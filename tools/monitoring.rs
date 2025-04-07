//! Monitoring tools for VibeCoin blockchain
//!
//! This module provides monitoring tools for the blockchain,
//! including performance metrics, resource usage, and health checks.

use std::sync::Arc;
use std::time::{Duration, Instant};
use log::{debug, error, info, warn};

use crate::storage::{
    KVStore, BlockStore, StateStore, TxStore,
    Block, AccountState, TransactionRecord, DatabaseStats,
};

/// Monitoring tools for blockchain performance
pub struct MonitoringTools<'a> {
    /// Block store
    block_store: Arc<BlockStore<'a>>,

    /// State store
    state_store: Arc<StateStore<'a>>,

    /// Transaction store
    tx_store: Arc<TxStore<'a>>,

    /// Key-value store
    kv_store: Arc<dyn KVStore + 'a>,

    /// Start time
    start_time: Instant,

    /// Performance metrics
    metrics: Arc<std::sync::RwLock<PerformanceMetrics>>,
}

/// Performance metrics
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    /// Number of blocks processed
    pub blocks_processed: u64,

    /// Number of transactions processed
    pub transactions_processed: u64,

    /// Average block processing time (in milliseconds)
    pub avg_block_processing_time: f64,

    /// Average transaction processing time (in milliseconds)
    pub avg_tx_processing_time: f64,

    /// Database size (in bytes)
    pub database_size: u64,

    /// Memory usage (in bytes)
    pub memory_usage: u64,

    /// CPU usage (percentage)
    pub cpu_usage: f64,
}

impl<'a> MonitoringTools<'a> {
    /// Create a new instance of MonitoringTools
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
            start_time: Instant::now(),
            metrics: Arc::new(std::sync::RwLock::new(PerformanceMetrics::default())),
        }
    }

    /// Get the current performance metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().unwrap().clone()
    }

    /// Update block processing metrics
    pub fn update_block_metrics(&self, processing_time: Duration) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.blocks_processed += 1;

        // Update average block processing time
        let current_avg = metrics.avg_block_processing_time;
        let current_count = metrics.blocks_processed;

        metrics.avg_block_processing_time = (current_avg * (current_count - 1) as f64 + processing_time.as_millis() as f64) / current_count as f64;
    }

    /// Update transaction processing metrics
    pub fn update_transaction_metrics(&self, processing_time: Duration) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.transactions_processed += 1;

        // Update average transaction processing time
        let current_avg = metrics.avg_tx_processing_time;
        let current_count = metrics.transactions_processed;

        metrics.avg_tx_processing_time = (current_avg * (current_count - 1) as f64 + processing_time.as_millis() as f64) / current_count as f64;
    }

    /// Update resource usage metrics
    pub fn update_resource_metrics(&self, db_stats: DatabaseStats) {
        let mut metrics = self.metrics.write().unwrap();
        metrics.database_size = db_stats.total_size_bytes;

        // Update memory and CPU usage (placeholder implementation)
        metrics.memory_usage = 0; // TODO: Implement memory usage tracking
        metrics.cpu_usage = 0.0; // TODO: Implement CPU usage tracking
    }

    /// Get the uptime of the node
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Perform a health check
    pub fn health_check(&self) -> bool {
        // Check if the database is accessible
        let db_accessible = match self.kv_store.get(b"health_check") {
            Ok(_) => true,
            Err(e) => {
                error!("Database health check failed: {}", e);
                false
            }
        };

        // Check if the block store is accessible
        let block_store_accessible = match self.block_store.get_latest_block() {
            Ok(_) => true,
            Err(e) => {
                error!("Block store health check failed: {}", e);
                false
            }
        };

        // Check if the state store is accessible
        let state_store_accessible = match self.state_store.get_state_root() {
            Some(_) => true,
            None => {
                error!("State store health check failed: state root not found");
                false
            }
        };

        db_accessible && block_store_accessible && state_store_accessible
    }
}

impl Clone for PerformanceMetrics {
    fn clone(&self) -> Self {
        Self {
            blocks_processed: self.blocks_processed,
            transactions_processed: self.transactions_processed,
            avg_block_processing_time: self.avg_block_processing_time,
            avg_tx_processing_time: self.avg_tx_processing_time,
            database_size: self.database_size,
            memory_usage: self.memory_usage,
            cpu_usage: self.cpu_usage,
        }
    }
}
