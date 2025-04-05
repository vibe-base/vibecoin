use crate::types::primitives::Hash;
use crate::types::error::VibecoinError;
use crate::ledger::block::Block;
use crate::ledger::transaction::Transaction;
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
    /// Response with block data
    BlockData(Vec<u8>),
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
        "block" => {
            if parts.len() < 5 {
                return Err(VibecoinError::NetworkError("Invalid block message".to_string()));
            }

            // This is a simplified block message - for full blocks, we use blockdata
            let index = parts[1].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid block index".to_string()))?;

            let hash_bytes = hex::decode(parts[2])
                .map_err(|_| VibecoinError::NetworkError("Invalid hash".to_string()))?;

            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 {
                hash.copy_from_slice(&hash_bytes);
            } else {
                return Err(VibecoinError::NetworkError("Invalid hash length".to_string()));
            }

            let slot_number = parts[3].parse::<u64>()
                .map_err(|_| VibecoinError::NetworkError("Invalid slot number".to_string()))?;

            let tx_count = parts[4].parse::<usize>()
                .map_err(|_| VibecoinError::NetworkError("Invalid transaction count".to_string()))?;

            // Create a minimal block with just the essential information
            let block = Block {
                index,
                hash,
                previous_hash: [0u8; 32], // Will be filled in later
                timestamp: 0,             // Will be filled in later
                nonce: 0,                 // Will be filled in later
                transactions: Vec::with_capacity(tx_count),
                slot_number,
                slot_leader: None,        // Will be filled in later
                poh_proof: None,          // Will be filled in later
            };

            Ok(Message::Block(block))
        },
        "blockdata" => {
            if parts.len() < 2 {
                return Err(VibecoinError::NetworkError("Invalid blockdata message".to_string()));
            }

            let data = hex::decode(parts[1])
                .map_err(|_| VibecoinError::NetworkError("Invalid block data".to_string()))?;

            Ok(Message::BlockData(data))
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
        Message::Block(block) => {
            // Serialize the block to a string representation for simple cases
            format!("block:{}:{}:{}:{}\n",
                   block.index,
                   hex::encode(block.hash),
                   block.slot_number,
                   block.transactions.len())
        },
        Message::BlockData(data) => {
            // For binary data, we use a special format
            format!("blockdata:{}\n", hex::encode(data))
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
    // Use a simple binary format for serialization
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
        data.extend_from_slice(&tx.sender);
        data.extend_from_slice(&tx.recipient);
        data.extend_from_slice(&tx.amount.to_le_bytes());
        data.extend_from_slice(&tx.timestamp.to_le_bytes());

        // Add signature if present
        if let Some(sig) = tx.signature {
            data.push(1); // Indicator that signature is present
            data.extend_from_slice(&sig);
        } else {
            data.push(0); // Indicator that signature is not present
        }
    }

    // Add block hash
    data.extend_from_slice(&block.hash);

    data
}

/// Deserialize a block from bytes
pub fn deserialize_block(data: &[u8]) -> Result<Block, VibecoinError> {
    if data.len() < 8 + 32 + 8 + 8 + 8 + 1 { // Minimum size for a block with no transactions
        return Err(VibecoinError::NetworkError("Data too short for block".to_string()));
    }

    let mut pos = 0;

    // Read block index
    let mut index_bytes = [0u8; 8];
    index_bytes.copy_from_slice(&data[pos..pos+8]);
    let index = u64::from_le_bytes(index_bytes);
    pos += 8;

    // Read previous hash
    let mut previous_hash = [0u8; 32];
    previous_hash.copy_from_slice(&data[pos..pos+32]);
    pos += 32;

    // Read timestamp
    let mut timestamp_bytes = [0u8; 8];
    timestamp_bytes.copy_from_slice(&data[pos..pos+8]);
    let timestamp = u64::from_le_bytes(timestamp_bytes);
    pos += 8;

    // Read nonce
    let mut nonce_bytes = [0u8; 8];
    nonce_bytes.copy_from_slice(&data[pos..pos+8]);
    let nonce = u64::from_le_bytes(nonce_bytes);
    pos += 8;

    // Read slot number
    let mut slot_bytes = [0u8; 8];
    slot_bytes.copy_from_slice(&data[pos..pos+8]);
    let slot_number = u64::from_le_bytes(slot_bytes);
    pos += 8;

    // Read slot leader
    let has_leader = data[pos] == 1;
    pos += 1;

    let slot_leader = if has_leader {
        if pos + 32 > data.len() {
            return Err(VibecoinError::NetworkError("Data too short for slot leader".to_string()));
        }

        let mut leader = [0u8; 32];
        leader.copy_from_slice(&data[pos..pos+32]);
        pos += 32;

        Some(leader)
    } else {
        None
    };

    // Read PoH proof
    let has_proof = data[pos] == 1;
    pos += 1;

    let poh_proof = if has_proof {
        if pos + 32 > data.len() {
            return Err(VibecoinError::NetworkError("Data too short for PoH proof".to_string()));
        }

        let mut proof = [0u8; 32];
        proof.copy_from_slice(&data[pos..pos+32]);
        pos += 32;

        Some(proof)
    } else {
        None
    };

    // Read transactions
    if pos + 4 > data.len() {
        return Err(VibecoinError::NetworkError("Data too short for transaction count".to_string()));
    }

    let mut tx_count_bytes = [0u8; 4];
    tx_count_bytes.copy_from_slice(&data[pos..pos+4]);
    let tx_count = u32::from_le_bytes(tx_count_bytes) as usize;
    pos += 4;

    let mut transactions = Vec::with_capacity(tx_count);

    for _ in 0..tx_count {
        if pos + 32 + 32 + 32 + 8 + 8 + 1 > data.len() {
            return Err(VibecoinError::NetworkError("Data too short for transaction".to_string()));
        }

        // Read transaction hash
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&data[pos..pos+32]);
        pos += 32;

        // Read sender
        let mut sender = [0u8; 32];
        sender.copy_from_slice(&data[pos..pos+32]);
        pos += 32;

        // Read recipient
        let mut recipient = [0u8; 32];
        recipient.copy_from_slice(&data[pos..pos+32]);
        pos += 32;

        // Read amount
        let mut amount_bytes = [0u8; 8];
        amount_bytes.copy_from_slice(&data[pos..pos+8]);
        let amount = u64::from_le_bytes(amount_bytes);
        pos += 8;

        // Read timestamp
        let mut tx_timestamp_bytes = [0u8; 8];
        tx_timestamp_bytes.copy_from_slice(&data[pos..pos+8]);
        let tx_timestamp = u64::from_le_bytes(tx_timestamp_bytes);
        pos += 8;

        // Read signature
        let has_signature = data[pos] == 1;
        pos += 1;

        let signature = if has_signature {
            if pos + 64 > data.len() {
                return Err(VibecoinError::NetworkError("Data too short for signature".to_string()));
            }

            let mut sig = [0u8; 64];
            sig.copy_from_slice(&data[pos..pos+64]);
            pos += 64;

            Some(sig)
        } else {
            None
        };

        // Create transaction
        let tx = Transaction {
            hash,
            sender,
            recipient,
            amount,
            timestamp: tx_timestamp,
            signature,
        };

        transactions.push(tx);
    }

    // Read block hash
    if pos + 32 > data.len() {
        return Err(VibecoinError::NetworkError("Data too short for block hash".to_string()));
    }

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&data[pos..pos+32]);

    // Create block
    let block = Block {
        index,
        previous_hash,
        timestamp,
        hash,
        nonce,
        transactions,
        slot_number,
        slot_leader,
        poh_proof,
    };

    Ok(block)
}
