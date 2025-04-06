// Vibecoin - A next-generation blockchain combining Proof-of-Work with Solana-style Proof of History

// Re-export storage module
pub mod storage;

// Initialize logging
pub fn init_logger() {
    env_logger::init();
}
