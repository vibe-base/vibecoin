//! Consensus types module
//!
//! This module defines the types used by the consensus mechanism.

mod block_header;
mod target;
mod errors;
mod config;

pub use block_header::BlockHeader;
pub use target::Target;
pub use errors::ConsensusError;
pub use config::ConsensusConfig;
