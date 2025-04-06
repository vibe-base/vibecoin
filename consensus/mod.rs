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

use std::sync::Arc;
use tokio::sync::mpsc;
use log::{debug, error, info, warn};

use crate::storage::block_store::{Block, BlockStore};
use crate::storage::tx_store::{TransactionRecord, TxStore};
use crate::storage::state_store::StateStore;
use crate::storage::kv_store::KVStore;
use crate::network::types::message::NetMessage;
use crate::consensus::config::ConsensusConfig;
use crate::consensus::engine::ConsensusEngine;

/// Start the consensus engine
pub async fn start_consensus<S: KVStore + 'static>(
    config: ConsensusConfig,
    block_store: Arc<BlockStore<'static>>,
    tx_store: Arc<TxStore<'static>>,
    state_store: Arc<StateStore<'static>>,
    network_tx: mpsc::Sender<NetMessage>,
) -> Arc<ConsensusEngine> {
    // Create the consensus engine
    let engine = ConsensusEngine::new(
        config,
        block_store,
        tx_store,
        state_store,
        network_tx,
    );
    
    // Start the engine
    let engine_arc = Arc::new(engine);
    let engine_clone = engine_arc.clone();
    
    tokio::spawn(async move {
        engine_clone.run().await;
    });
    
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
