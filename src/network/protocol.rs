use crate::types::primitives::Hash;
use crate::types::error::VibecoinError;
use crate::ledger::block::Block;
use crate::ledger::state::BlockchainState;
use std::net::SocketAddr;

/// Network protocol message types
#[derive(Debug, Clone)]
pub enum Message {
    /// Version message for handshake
    Version {
        version: String,
        node_id: u32,
    },
    /// Request the latest blockchain state
    GetState,
    /// Response with blockchain state
    State(BlockchainState),
    /// Request a specific block by hash
    GetBlock(Hash),
    /// Response with a block
    Block(Block),
    /// Request blocks in a range
    GetBlocks {
        start_height: u64,
        end_height: u64,
    },
    /// Announce a new block
    NewBlock(Hash),
    /// Announce a new transaction
    NewTransaction(Hash),
    /// Ping message to keep connection alive
    Ping(u64),
    /// Pong response to ping
    Pong(u64),
}

/// Parse a message from a string
pub fn parse_message(msg: &str) -> Result<Message, VibecoinError> {
    let parts: Vec<&str> = msg.trim().split(':').collect();

    if parts.is_empty() {
        return Err(VibecoinError::NetworkError("Empty message".to_string()));
    }

    match parts[0] {
        "version" => {
            if parts.len() < 3 {
                return Err(VibecoinError::NetworkError("Invalid version message".to_string()));
            }

            let version = parts[1].to_string();
            // More lenient parsing of node_id - handle both u32 and process IDs
            let node_id = match parts[2].parse::<u32>() {
                Ok(id) => id,
                Err(_) => {
                    // If we can't parse it as u32, just use a default value
                    println!("Could not parse node ID '{}', using default", parts[2]);
                    0
                }
            };

            Ok(Message::Version { version, node_id })
        },
        "getstate" => {
            Ok(Message::GetState)
        },
        "state" => {
            if parts.len() < 5 {
                return Err(VibecoinError::NetworkError("Invalid state message".to_string()));
            }

            let height = parts[1].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid height".to_string()))?;

            let latest_hash = hex::decode(parts[2])
                .map_err(|_| VibecoinError::NetworkError("Invalid hash".to_string()))?;

            let mut hash = [0u8; 32];
            if latest_hash.len() == 32 {
                hash.copy_from_slice(&latest_hash);
            } else {
                return Err(VibecoinError::NetworkError("Invalid hash length".to_string()));
            }

            let difficulty = parts[3].parse::<u8>()
                .map_err(|_| VibecoinError::NetworkError("Invalid difficulty".to_string()))?;

            let poh_count = parts[4].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid PoH count".to_string()))?;

            let current_slot = if parts.len() > 5 {
                parts[5].parse::<u64>()
                    .map_err(|_| VibecoinError::NetworkError("Invalid slot".to_string()))?
            } else {
                0
            };

            Ok(Message::State(BlockchainState {
                height,
                latest_hash: hash,
                difficulty,
                poh_count,
                current_slot,
            }))
        },
        "getblock" => {
            if parts.len() < 2 {
                return Err(VibecoinError::NetworkError("Invalid getblock message".to_string()));
            }

            let hash_bytes = hex::decode(parts[1])
                .map_err(|_| VibecoinError::NetworkError("Invalid hash".to_string()))?;

            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 {
                hash.copy_from_slice(&hash_bytes);
            } else {
                return Err(VibecoinError::NetworkError("Invalid hash length".to_string()));
            }

            Ok(Message::GetBlock(hash))
        },
        "getblocks" => {
            if parts.len() < 3 {
                return Err(VibecoinError::NetworkError("Invalid getblocks message".to_string()));
            }

            let start_height = parts[1].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid start height".to_string()))?;

            let end_height = parts[2].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid end height".to_string()))?;

            Ok(Message::GetBlocks {
                start_height,
                end_height,
            })
        },
        "newblock" => {
            if parts.len() < 2 {
                return Err(VibecoinError::NetworkError("Invalid newblock message".to_string()));
            }

            let hash_bytes = hex::decode(parts[1])
                .map_err(|_| VibecoinError::NetworkError("Invalid hash".to_string()))?;

            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 {
                hash.copy_from_slice(&hash_bytes);
            } else {
                return Err(VibecoinError::NetworkError("Invalid hash length".to_string()));
            }

            Ok(Message::NewBlock(hash))
        },
        "newtx" => {
            if parts.len() < 2 {
                return Err(VibecoinError::NetworkError("Invalid newtx message".to_string()));
            }

            let hash_bytes = hex::decode(parts[1])
                .map_err(|_| VibecoinError::NetworkError("Invalid hash".to_string()))?;

            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 {
                hash.copy_from_slice(&hash_bytes);
            } else {
                return Err(VibecoinError::NetworkError("Invalid hash length".to_string()));
            }

            Ok(Message::NewTransaction(hash))
        },
        "ping" => {
            if parts.len() < 2 {
                return Err(VibecoinError::NetworkError("Invalid ping message".to_string()));
            }

            let nonce = parts[1].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid nonce".to_string()))?;

            Ok(Message::Ping(nonce))
        },
        "pong" => {
            if parts.len() < 2 {
                return Err(VibecoinError::NetworkError("Invalid pong message".to_string()));
            }

            let nonce = parts[1].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid nonce".to_string()))?;

            Ok(Message::Pong(nonce))
        },
        _ => {
            Err(VibecoinError::NetworkError(format!("Unknown message type: {}", parts[0])))
        }
    }
}

/// Format a message to a string
pub fn format_message(msg: &Message) -> String {
    match msg {
        Message::Version { version, node_id } => {
            format!("version:{}:{}\n", version, node_id)
        },
        Message::GetState => {
            "getstate\n".to_string()
        },
        Message::State(state) => {
            format!("state:{}:{}:{}:{}:{}\n",
                state.height,
                hex::encode(state.latest_hash),
                state.difficulty,
                state.poh_count,
                state.current_slot)
        },
        Message::GetBlock(hash) => {
            format!("getblock:{}\n", hex::encode(hash))
        },
        Message::Block(_) => {
            // Block serialization is more complex and would be handled separately
            "block:...\n".to_string()
        },
        Message::GetBlocks { start_height, end_height } => {
            format!("getblocks:{}:{}\n", start_height, end_height)
        },
        Message::NewBlock(hash) => {
            format!("newblock:{}\n", hex::encode(hash))
        },
        Message::NewTransaction(hash) => {
            format!("newtx:{}\n", hex::encode(hash))
        },
        Message::Ping(nonce) => {
            format!("ping:{}\n", nonce)
        },
        Message::Pong(nonce) => {
            format!("pong:{}\n", nonce)
        },
    }
}

/// Serialize a block to bytes
pub fn serialize_block(block: &Block) -> Vec<u8> {
    // In a real implementation, we would use a proper serialization format
    // For now, we'll use a simple string representation
    let mut data = Vec::new();

    // Add block header fields
    data.extend_from_slice(&block.index.to_le_bytes());
    data.extend_from_slice(&block.previous_hash);
    data.extend_from_slice(&block.timestamp.to_le_bytes());
    data.extend_from_slice(&block.nonce.to_le_bytes());
    data.extend_from_slice(&block.slot_number.to_le_bytes());

    // Add slot leader if present
    if let Some(leader) = block.slot_leader {
        data.push(1); // Indicator that leader is present
        data.extend_from_slice(&leader);
    } else {
        data.push(0); // Indicator that leader is not present
    }

    // Add PoH proof if present
    if let Some(proof) = block.poh_proof {
        data.push(1); // Indicator that proof is present
        data.extend_from_slice(&proof);
    } else {
        data.push(0); // Indicator that proof is not present
    }

    // Add transactions
    data.extend_from_slice(&(block.transactions.len() as u32).to_le_bytes());
    for tx in block.transactions.iter() {
        data.extend_from_slice(&tx.hash);
    }

    // Add block hash
    data.extend_from_slice(&block.hash);

    data
}

/// Deserialize a block from bytes
pub fn deserialize_block(data: &[u8]) -> Result<Block, VibecoinError> {
    // In a real implementation, we would use a proper deserialization format
    // For now, we'll use a simple string representation

    // This is a placeholder - in a real implementation, we would parse the bytes
    // and construct a Block object

    Err(VibecoinError::NetworkError("Block deserialization not implemented".to_string()))
}
