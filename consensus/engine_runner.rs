use std::sync::Arc;
use tokio::sync::Mutex;

use crate::consensus::engine::ConsensusEngine;

/// A wrapper for running the consensus engine without requiring &mut self
pub struct ConsensusEngineRunner {
    /// The consensus engine
    engine: Arc<Mutex<ConsensusEngine>>,
}

impl ConsensusEngineRunner {
    /// Create a new consensus engine runner
    pub fn new(engine: ConsensusEngine) -> Self {
        Self {
            engine: Arc::new(Mutex::new(engine)),
        }
    }

    /// Run the consensus engine
    pub async fn run(&self) {
        // Get a mutable reference to the engine
        let mut engine = self.engine.lock().await;

        // Run the engine
        engine.run().await;
    }

    /// Get a reference to the underlying engine
    pub fn engine(&self) -> Arc<Mutex<ConsensusEngine>> {
        self.engine.clone()
    }
}
