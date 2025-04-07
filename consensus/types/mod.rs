//! Consensus types module
//!
//! This module defines the types used by the consensus mechanism.

pub mod block_header;
pub mod target;
pub mod block_template;
pub mod chain_state;
pub mod fork_choice;
pub mod errors;
pub mod config;

// Re-export main types
pub use block_header::BlockHeader;
pub use target::Target;
pub use block_template::BlockTemplate;
pub use chain_state::ChainState;
pub use fork_choice::ForkChoice;
pub use errors::ConsensusError;
pub use config::ConsensusConfig;
