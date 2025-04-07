use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time;
use log::{debug, error, info, trace, warn};
use rayon::prelude::*;

use crate::crypto::hash::{sha256, Hash};
use crate::storage::block_store::Block;
use crate::consensus::types::{Target, BlockTemplate};
use crate::consensus::config::ConsensusConfig;

/// Result of mining
#[derive(Clone)]
pub struct MiningResult {
    /// The mined block
    pub block: Block,

    /// The nonce that solved the block
    pub nonce: u64,

    /// The time it took to mine the block
    pub mining_time: Duration,
}

/// Miner for Proof of Work
pub struct PoWMiner {
    /// Mining configuration
    config: ConsensusConfig,

    /// Flag to indicate if mining should stop
    stop_flag: Arc<AtomicBool>,

    /// Channel for mining results
    result_tx: Option<mpsc::Sender<MiningResult>>,
}

impl PoWMiner {
    /// Create a new PoW miner
    pub fn new(config: ConsensusConfig) -> Self {
        Self {
            config,
            stop_flag: Arc::new(AtomicBool::new(false)),
            result_tx: None,
        }
    }

    /// Set the channel for mining results
    pub fn with_result_channel(mut self, tx: mpsc::Sender<MiningResult>) -> Self {
        self.result_tx = Some(tx);
        self
    }

    /// Mine a block with the given template
    pub async fn mine_block(&self, template: BlockTemplate) -> Option<MiningResult> {
        // Reset the stop flag
        self.stop_flag.store(false, Ordering::SeqCst);

        info!("Starting mining for block at height {}", template.height);
        let start_time = Instant::now();

        // Create a channel for the mining result
        let (tx, mut rx) = mpsc::channel(1);

        // Clone the stop flag for the mining task
        let stop_flag = self.stop_flag.clone();

        // Spawn a task to mine the block
        let num_threads = self.config.mining_threads;
        let template_clone = template.clone();

        tokio::task::spawn_blocking(move || {
            // Divide the nonce space among threads
            let nonce_range_per_thread = u64::MAX / num_threads as u64;

            // Use Rayon for parallel mining
            let result = (0..num_threads).into_par_iter().find_map_any(|thread_id| {
                let start_nonce = thread_id as u64 * nonce_range_per_thread;
                let end_nonce = if thread_id == num_threads - 1 {
                    u64::MAX
                } else {
                    (thread_id + 1) as u64 * nonce_range_per_thread - 1
                };

                Self::mine_range(
                    &template_clone,
                    start_nonce,
                    end_nonce,
                    &stop_flag,
                )
            });

            // Send the result if mining was successful
            if let Some((block, nonce)) = result {
                let mining_time = start_time.elapsed();
                let result = MiningResult {
                    block,
                    nonce,
                    mining_time,
                };

                let _ = tx.blocking_send(result);
            }
        });

        // Wait for the mining result or stop signal
        tokio::select! {
            result = rx.recv() => {
                if let Some(result) = result {
                    info!(
                        "Block mined at height {} with nonce {} in {:.2}s",
                        template.height,
                        result.nonce,
                        result.mining_time.as_secs_f64()
                    );

                    // Send the result if we have a channel
                    if let Some(tx) = &self.result_tx {
                        if let Err(e) = tx.send(result.clone()).await {
                            error!("Failed to send mining result: {}", e);
                        }
                    }

                    Some(result)
                } else {
                    warn!("Mining task exited without result");
                    None
                }
            }
            _ = time::sleep(Duration::from_secs(60)) => {
                warn!("Mining timed out after 60 seconds");
                self.stop_mining();
                None
            }
        }
    }

    /// Mine a range of nonces
    fn mine_range(
        template: &BlockTemplate,
        start_nonce: u64,
        end_nonce: u64,
        stop_flag: &AtomicBool,
    ) -> Option<(Block, u64)> {
        let target = &template.target;

        for nonce in start_nonce..=end_nonce {
            // Check if we should stop
            if stop_flag.load(Ordering::Relaxed) {
                return None;
            }

            // Every 1M nonces, log progress
            if nonce % 1_000_000 == 0 && nonce > 0 {
                trace!("Mining progress: tried {} nonces", nonce);
            }

            // Create a block with this nonce
            // In a real implementation, we would compute the block hash here
            // For now, we'll just use a placeholder
            let block_data = format!(
                "{}:{}:{}:{}:{}:{}",
                template.height,
                hex::encode(&template.prev_hash),
                template.timestamp,
                hex::encode(&template.state_root),
                hex::encode(&template.tx_root),
                nonce
            );

            let hash = sha256(block_data.as_bytes());

            // Check if the hash meets the target
            if target.is_met_by(&hash) {
                // Get the current PoH hash
                let poh_hash = hash; // In a real implementation, we would get this from the PoH generator

                // Create the block
                let block = template.to_block(nonce, poh_hash);
                return Some((block, nonce));
            }
        }

        None
    }

    /// Stop mining
    pub fn stop_mining(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::types::Target;

    #[tokio::test]
    async fn test_mining_with_easy_target() {
        // Create a config
        let config = ConsensusConfig::default()
            .with_mining_threads(1);

        // Create a miner
        let miner = PoWMiner::new(config);

        // Create a block template with a very easy target
        let template = BlockTemplate {
            height: 1,
            prev_hash: [0u8; 32],
            timestamp: 12345,
            transactions: vec![],
            state_root: [0u8; 32],
            tx_root: None, // Will be calculated when needed
            target: Target::from_difficulty(1), // Easiest possible difficulty
            poh_sequence_start: 0,
        };

        // Mine the block
        let result = miner.mine_block(template).await;

        // Check that we got a result
        assert!(result.is_some());

        let result = result.unwrap();

        // Check that the mining time is reasonable
        assert!(result.mining_time < Duration::from_secs(1));
    }
}
