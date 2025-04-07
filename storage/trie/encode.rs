

/// Nibble is a 4-bit value (0-15)
pub type Nibble = u8;

/// Encode a byte slice into a vector of nibbles
///
/// Each byte is split into two nibbles (4-bit values).
/// For example, the byte 0xAB becomes two nibbles: 0xA and 0xB.
pub fn bytes_to_nibbles(bytes: &[u8]) -> Vec<Nibble> {
    let mut nibbles = Vec::with_capacity(bytes.len() * 2);

    for &byte in bytes {
        // Extract the high nibble (first 4 bits)
        nibbles.push(byte >> 4);
        // Extract the low nibble (last 4 bits)
        nibbles.push(byte & 0x0F);
    }

    nibbles
}

/// Encode a hex string into a vector of nibbles
///
/// Each character in the hex string becomes a nibble.
/// For example, the string "AB" becomes two nibbles: 0xA and 0xB.
pub fn hex_to_nibbles(hex: &str) -> Result<Vec<Nibble>, String> {
    let mut nibbles = Vec::with_capacity(hex.len());

    for c in hex.chars() {
        let nibble = match c {
            '0'..='9' => c as u8 - b'0',
            'a'..='f' => c as u8 - b'a' + 10,
            'A'..='F' => c as u8 - b'A' + 10,
            _ => return Err(format!("Invalid hex character: {}", c)),
        };

        nibbles.push(nibble);
    }

    Ok(nibbles)
}

/// Convert a vector of nibbles back to bytes
///
/// Every two nibbles are combined into a single byte.
/// For example, the nibbles [0xA, 0xB] become the byte 0xAB.
/// If there's an odd number of nibbles, the last nibble is ignored.
pub fn nibbles_to_bytes(nibbles: &[Nibble]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((nibbles.len() + 1) / 2);

    // Process pairs of nibbles
    for chunk in nibbles.chunks(2) {
        if chunk.len() == 2 {
            // Combine two nibbles into a byte
            let byte = (chunk[0] << 4) | chunk[1];
            bytes.push(byte);
        } else {
            // Handle odd number of nibbles (last nibble)
            let byte = chunk[0] << 4;
            bytes.push(byte);
        }
    }

    bytes
}

/// Convert a vector of nibbles to a hex string
///
/// Each nibble becomes a hex character.
/// For example, the nibbles [0xA, 0xB] become the string "AB".
pub fn nibbles_to_hex(nibbles: &[Nibble]) -> String {
    let mut hex = String::with_capacity(nibbles.len());

    for &nibble in nibbles {
        let c = match nibble {
            0..=9 => (b'0' + nibble) as char,
            10..=15 => (b'a' + (nibble - 10)) as char,
            _ => panic!("Invalid nibble: {}", nibble),
        };

        hex.push(c);
    }

    hex
}

/// Compact encoding for keys in extension and leaf nodes
///
/// The first nibble of the compact encoding contains flags:
/// - The lowest bit (bit 0) contains the parity of the length of the key in nibbles
/// - The second-lowest bit (bit 1) contains the flag indicating whether the node is a leaf
///
/// For example:
/// - [0, 1, 2, 3, 4, 5] as an extension node becomes [0x00, 0x01, 0x23, 0x45]
/// - [0, 1, 2, 3, 4, 5] as a leaf node becomes [0x20, 0x01, 0x23, 0x45]
/// - [1, 2, 3, 4, 5] as an extension node becomes [0x11, 0x23, 0x45]
/// - [1, 2, 3, 4, 5] as a leaf node becomes [0x31, 0x23, 0x45]
pub fn compact_encode(nibbles: &[Nibble], is_leaf: bool) -> Vec<u8> {
    let mut compact = Vec::with_capacity((nibbles.len() + 2) / 2);
    let is_odd = nibbles.len() % 2 != 0;

    // Create the first byte with flags
    let mut first_byte = 0;

    if is_leaf {
        first_byte |= 0x20; // Set the leaf flag (bit 1)
    }

    if is_odd {
        first_byte |= 0x10; // Set the odd flag (bit 0)
        compact.push(first_byte | nibbles[0]); // Include the first nibble in the first byte

        // Process the rest of the nibbles
        for i in (1..nibbles.len()).step_by(2) {
            if i + 1 < nibbles.len() {
                compact.push((nibbles[i] << 4) | nibbles[i + 1]);
            } else {
                compact.push(nibbles[i] << 4);
            }
        }
    } else {
        compact.push(first_byte); // First byte only contains flags

        // Process all nibbles
        for i in (0..nibbles.len()).step_by(2) {
            compact.push((nibbles[i] << 4) | nibbles[i + 1]);
        }
    }

    compact
}

/// Decode a compact encoding back to nibbles
///
/// This is the reverse of compact_encode.
pub fn compact_decode(compact: &[u8]) -> (Vec<Nibble>, bool) {
    if compact.is_empty() {
        return (Vec::new(), false);
    }

    let first_byte = compact[0];
    let is_leaf = (first_byte & 0x20) != 0;
    let is_odd = (first_byte & 0x10) != 0;

    let mut nibbles = Vec::with_capacity(compact.len() * 2);

    if is_odd {
        // First byte contains a nibble
        nibbles.push(first_byte & 0x0F);

        // Process the rest of the bytes
        for &byte in &compact[1..] {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }
    } else {
        // Process all bytes except the first
        for &byte in &compact[1..] {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }
    }

    (nibbles, is_leaf)
}

/// Helper function to format a slice of nibbles
pub fn format_nibbles(nibbles: &[Nibble]) -> String {
    let mut s = String::new();
    s.push('[');

    for (i, &nibble) in nibbles.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&format!("{:x}", nibble));
    }

    s.push(']');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_nibbles() {
        let bytes = vec![0x12, 0x34, 0xAB, 0xCD];
        let nibbles = bytes_to_nibbles(&bytes);
        assert_eq!(nibbles, vec![1, 2, 3, 4, 10, 11, 12, 13]);
    }

    #[test]
    fn test_hex_to_nibbles() {
        let hex = "1234ABCD";
        let nibbles = hex_to_nibbles(hex).unwrap();
        assert_eq!(nibbles, vec![1, 2, 3, 4, 10, 11, 12, 13]);

        // Test with lowercase
        let hex = "1234abcd";
        let nibbles = hex_to_nibbles(hex).unwrap();
        assert_eq!(nibbles, vec![1, 2, 3, 4, 10, 11, 12, 13]);

        // Test with invalid character
        let hex = "1234ABGD";
        assert!(hex_to_nibbles(hex).is_err());
    }

    #[test]
    fn test_nibbles_to_bytes() {
        let nibbles = vec![1, 2, 3, 4, 10, 11, 12, 13];
        let bytes = nibbles_to_bytes(&nibbles);
        assert_eq!(bytes, vec![0x12, 0x34, 0xAB, 0xCD]);

        // Test with odd number of nibbles
        let nibbles = vec![1, 2, 3, 4, 10, 11, 12];
        let bytes = nibbles_to_bytes(&nibbles);
        assert_eq!(bytes, vec![0x12, 0x34, 0xAB, 0xC0]);
    }

    #[test]
    fn test_nibbles_to_hex() {
        let nibbles = vec![1, 2, 3, 4, 10, 11, 12, 13];
        let hex = nibbles_to_hex(&nibbles);
        assert_eq!(hex, "1234abcd");
    }

    #[test]
    fn test_compact_encode() {
        // Test even length, extension node
        let nibbles = vec![0, 1, 2, 3, 4, 5];
        let compact = compact_encode(&nibbles, false);
        assert_eq!(compact, vec![0x00, 0x01, 0x23, 0x45]);

        // Test even length, leaf node
        let compact = compact_encode(&nibbles, true);
        assert_eq!(compact, vec![0x20, 0x01, 0x23, 0x45]);

        // Test odd length, extension node
        let nibbles = vec![1, 2, 3, 4, 5];
        let compact = compact_encode(&nibbles, false);
        assert_eq!(compact, vec![0x11, 0x23, 0x45]);

        // Test odd length, leaf node
        let compact = compact_encode(&nibbles, true);
        assert_eq!(compact, vec![0x31, 0x23, 0x45]);
    }

    #[test]
    fn test_compact_decode() {
        // Test even length, extension node
        let compact = vec![0x00, 0x01, 0x23, 0x45];
        let (nibbles, is_leaf) = compact_decode(&compact);
        assert_eq!(nibbles, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(is_leaf, false);

        // Test even length, leaf node
        let compact = vec![0x20, 0x01, 0x23, 0x45];
        let (nibbles, is_leaf) = compact_decode(&compact);
        assert_eq!(nibbles, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(is_leaf, true);

        // Test odd length, extension node
        let compact = vec![0x11, 0x23, 0x45];
        let (nibbles, is_leaf) = compact_decode(&compact);
        assert_eq!(nibbles, vec![1, 2, 3, 4, 5]);
        assert_eq!(is_leaf, false);

        // Test odd length, leaf node
        let compact = vec![0x31, 0x23, 0x45];
        let (nibbles, is_leaf) = compact_decode(&compact);
        assert_eq!(nibbles, vec![1, 2, 3, 4, 5]);
        assert_eq!(is_leaf, true);
    }

    #[test]
    fn test_roundtrip() {
        // Test even length
        let original = vec![0, 1, 2, 3, 4, 5];
        let compact = compact_encode(&original, false);
        let (decoded, _) = compact_decode(&compact);
        assert_eq!(original, decoded);

        // Test odd length
        let original = vec![1, 2, 3, 4, 5];
        let compact = compact_encode(&original, true);
        let (decoded, is_leaf) = compact_decode(&compact);
        assert_eq!(original, decoded);
        assert_eq!(is_leaf, true);
    }
}
