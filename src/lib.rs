// Vibecoin - A next-generation blockchain combining Proof-of-Work with Solana-style Proof of History

// Export modules
pub mod storage;
pub mod crypto;
pub mod network;
pub mod consensus;
pub mod mempool;

// Initialize logging
pub fn init_logger() {
    env_logger::init();
}
