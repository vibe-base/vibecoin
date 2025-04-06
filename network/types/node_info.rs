use serde::{Serialize, Deserialize};
use std::net::SocketAddr;
use std::fmt;

/// Information about a node in the network
#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct NodeInfo {
    /// Protocol version
    pub version: String,
    
    /// Unique node identifier
    pub node_id: String,
    
    /// Address the node is listening on
    pub listen_addr: SocketAddr,
}

impl NodeInfo {
    /// Create a new NodeInfo
    pub fn new(version: String, node_id: String, listen_addr: SocketAddr) -> Self {
        Self {
            version,
            node_id,
            listen_addr,
        }
    }
    
    /// Check if the version is compatible
    pub fn is_compatible(&self, other: &NodeInfo) -> bool {
        // For now, just check if major version matches
        let self_version = self.version.split('.').next().unwrap_or("0");
        let other_version = other.version.split('.').next().unwrap_or("0");
        
        self_version == other_version
    }
}

impl fmt::Debug for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeInfo {{ id: {}, version: {}, addr: {} }}", 
               self.node_id, self.version, self.listen_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_node_info_compatibility() {
        let node1 = NodeInfo::new(
            "1.0.0".to_string(),
            "node1".to_string(),
            "127.0.0.1:8000".parse().unwrap(),
        );
        
        let node2 = NodeInfo::new(
            "1.2.3".to_string(),
            "node2".to_string(),
            "127.0.0.1:8001".parse().unwrap(),
        );
        
        let node3 = NodeInfo::new(
            "2.0.0".to_string(),
            "node3".to_string(),
            "127.0.0.1:8002".parse().unwrap(),
        );
        
        // Same major version should be compatible
        assert!(node1.is_compatible(&node2));
        assert!(node2.is_compatible(&node1));
        
        // Different major version should not be compatible
        assert!(!node1.is_compatible(&node3));
        assert!(!node3.is_compatible(&node1));
    }
}
