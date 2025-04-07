// Proof of History module

pub mod generator;
pub mod verifier;

// Re-export main types
pub use generator::PoHGenerator;
pub use verifier::PoHVerifier;
