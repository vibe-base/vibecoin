// VibeCoin Consensus Module
//
// This module implements the hybrid Proof-of-Work (PoW) and Proof-of-History (PoH)
// consensus mechanism for the VibeCoin blockchain.

pub mod pow;
pub mod poh;
pub mod validation;
pub mod mining;
pub mod types;
pub mod config;
pub mod engine;
pub mod block_processor;
pub mod engine_runner;

use std::sync::Arc;
use tokio::sync::mpsc;
use log::info;

use crate::storage::block_store::BlockStore;
use crate::storage::tx_store::TxStore;
use crate::storage::state_store::StateStore;
use crate::storage::kv_store::KVStore;
use crate::network::types::message::NetMessage;
use crate::consensus::config::ConsensusConfig;
use crate::consensus::engine::ConsensusEngine;
use crate::consensus::engine_runner::ConsensusEngineRunner;

/// Start the consensus engine
pub async fn start_consensus<S: KVStore + 'static>(
    config: ConsensusConfig,
    kv_store: Arc<S>,
    block_store: Arc<BlockStore<'static>>,
    tx_store: Arc<TxStore<'static>>,
    state_store: Arc<StateStore<'static>>,
    network_tx: mpsc::Sender<NetMessage>,
) -> Arc<ConsensusEngine> {
    // Create the consensus engine
    let engine = ConsensusEngine::new(
        config.clone(),
        kv_store.clone(),
        block_store.clone(),
        tx_store.clone(),
        state_store.clone(),
        network_tx.clone(),
    );

    // Create a reference to the engine for returning
    let engine_arc = Arc::new(engine);

    // Create a clone of the engine for the runner
    let engine_for_runner = ConsensusEngine::new(
        config,
        kv_store,
        block_store,
        tx_store,
        state_store,
        network_tx,
    );

    // Create the engine runner
    let runner = ConsensusEngineRunner::new(engine_for_runner);

    // Start the engine in a separate task
    tokio::spawn(async move {
        info!("Starting consensus engine...");
        runner.run().await;
    });

    // Return a reference to the engine
    engine_arc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::config::ConsensusConfig;

    #[test]
    fn test_consensus_config() {
        let config = ConsensusConfig::default();
        assert!(config.target_block_time > 0);
        assert!(config.initial_difficulty > 0);
        assert!(config.difficulty_adjustment_window > 0);
    }
}
