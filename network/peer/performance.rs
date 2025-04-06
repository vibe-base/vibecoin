use std::collections::HashMap;
use std::time::Instant;
use std::sync::atomic::{AtomicU64, Ordering};

/// Type alias for block hash
pub type BlockHash = [u8; 32];

/// Type alias for transaction hash
pub type TxHash = [u8; 32];

/// Counter for atomic operations
#[derive(Debug, Default)]
pub struct Counter {
    /// The counter value
    value: AtomicU64,
}

impl Counter {
    /// Create a new counter
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }
    
    /// Increment the counter
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::Relaxed) + 1
    }
    
    /// Get the counter value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
    
    /// Reset the counter
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

impl Clone for Counter {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU64::new(self.get()),
        }
    }
}

/// Exponential moving average
#[derive(Debug, Clone)]
pub struct ExponentialMovingAverage {
    /// The current average
    value: f64,
    
    /// The weight for new values
    alpha: f64,
    
    /// The number of samples
    samples: u64,
}

impl ExponentialMovingAverage {
    /// Create a new exponential moving average
    pub fn new(alpha: f64) -> Self {
        Self {
            value: 0.0,
            alpha,
            samples: 0,
        }
    }
    
    /// Update the average with a new value
    pub fn update(&mut self, value: f64) {
        if self.samples == 0 {
            self.value = value;
        } else {
            self.value = self.alpha * value + (1.0 - self.alpha) * self.value;
        }
        self.samples += 1;
    }
    
    /// Get the current average
    pub fn get(&self) -> f64 {
        self.value
    }
    
    /// Get the number of samples
    pub fn samples(&self) -> u64 {
        self.samples
    }
}

impl Default for ExponentialMovingAverage {
    fn default() -> Self {
        Self::new(0.2) // 20% weight for new values
    }
}

/// Peer performance tracking
#[derive(Debug, Clone)]
pub struct PeerPerformance {
    /// Message statistics
    pub messages_received: Counter,
    pub messages_sent: Counter,
    pub bytes_received: Counter,
    pub bytes_sent: Counter,
    
    /// Latency tracking
    pub ping_latency: ExponentialMovingAverage,
    pub response_latency: ExponentialMovingAverage,
    
    /// Reliability
    pub successful_responses: Counter,
    pub failed_responses: Counter,
    pub timeouts: Counter,
    
    /// Block/transaction propagation
    pub block_announcement_time: HashMap<BlockHash, Instant>,
    pub transaction_announcement_time: HashMap<TxHash, Instant>,
    
    /// Misbehavior
    pub invalid_messages: Counter,
    pub duplicate_messages: Counter,
    pub protocol_violations: Counter,
}

impl PeerPerformance {
    /// Create new peer performance tracking
    pub fn new() -> Self {
        Self {
            messages_received: Counter::new(),
            messages_sent: Counter::new(),
            bytes_received: Counter::new(),
            bytes_sent: Counter::new(),
            ping_latency: ExponentialMovingAverage::default(),
            response_latency: ExponentialMovingAverage::default(),
            successful_responses: Counter::new(),
            failed_responses: Counter::new(),
            timeouts: Counter::new(),
            block_announcement_time: HashMap::new(),
            transaction_announcement_time: HashMap::new(),
            invalid_messages: Counter::new(),
            duplicate_messages: Counter::new(),
            protocol_violations: Counter::new(),
        }
    }
    
    /// Record a received message
    pub fn record_message_received(&self, bytes: u64) {
        self.messages_received.increment();
        self.bytes_received.fetch_add(bytes);
    }
    
    /// Record a sent message
    pub fn record_message_sent(&self, bytes: u64) {
        self.messages_sent.increment();
        self.bytes_sent.fetch_add(bytes);
    }
    
    /// Update ping latency
    pub fn update_ping_latency(&mut self, latency: u64) {
        self.ping_latency.update(latency as f64);
    }
    
    /// Update response latency
    pub fn update_response_latency(&mut self, latency: u64) {
        self.response_latency.update(latency as f64);
    }
    
    /// Record a successful response
    pub fn record_successful_response(&self) {
        self.successful_responses.increment();
    }
    
    /// Record a failed response
    pub fn record_failed_response(&self) {
        self.failed_responses.increment();
    }
    
    /// Record a timeout
    pub fn record_timeout(&self) {
        self.timeouts.increment();
    }
    
    /// Record a block announcement
    pub fn record_block_announcement(&mut self, block_hash: BlockHash) {
        self.block_announcement_time.insert(block_hash, Instant::now());
    }
    
    /// Record a transaction announcement
    pub fn record_transaction_announcement(&mut self, tx_hash: TxHash) {
        self.transaction_announcement_time.insert(tx_hash, Instant::now());
    }
    
    /// Record an invalid message
    pub fn record_invalid_message(&self) {
        self.invalid_messages.increment();
    }
    
    /// Record a duplicate message
    pub fn record_duplicate_message(&self) {
        self.duplicate_messages.increment();
    }
    
    /// Record a protocol violation
    pub fn record_protocol_violation(&self) {
        self.protocol_violations.increment();
    }
    
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        let successful = self.successful_responses.get();
        let failed = self.failed_responses.get();
        let timeouts = self.timeouts.get();
        
        let total = successful + failed + timeouts;
        if total == 0 {
            return 0.0;
        }
        
        successful as f64 / total as f64
    }
    
    /// Calculate misbehavior rate
    pub fn misbehavior_rate(&self) -> f64 {
        let invalid = self.invalid_messages.get();
        let duplicate = self.duplicate_messages.get();
        let violations = self.protocol_violations.get();
        
        let total = self.messages_received.get();
        if total == 0 {
            return 0.0;
        }
        
        (invalid + duplicate + violations) as f64 / total as f64
    }
    
    /// Get average ping latency
    pub fn average_ping_latency(&self) -> Option<f64> {
        if self.ping_latency.samples() > 0 {
            Some(self.ping_latency.get())
        } else {
            None
        }
    }
    
    /// Get average response latency
    pub fn average_response_latency(&self) -> Option<f64> {
        if self.response_latency.samples() > 0 {
            Some(self.response_latency.get())
        } else {
            None
        }
    }
    
    /// Calculate a performance score (higher is better)
    pub fn performance_score(&self) -> f64 {
        let success_rate = self.success_rate();
        let misbehavior_rate = self.misbehavior_rate();
        let ping_latency = self.average_ping_latency().unwrap_or(1000.0);
        
        // Normalize ping latency (lower is better)
        let normalized_latency = 1.0 - (ping_latency.min(1000.0) / 1000.0);
        
        // Calculate score (higher is better)
        let score = success_rate * 0.5 + normalized_latency * 0.3 - misbehavior_rate * 0.2;
        
        // Ensure score is between 0 and 1
        score.max(0.0).min(1.0)
    }
}

impl Default for PeerPerformance {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for Counter
pub trait CounterExt {
    /// Add a value to the counter
    fn fetch_add(&self, value: u64) -> u64;
}

impl CounterExt for Counter {
    fn fetch_add(&self, value: u64) -> u64 {
        self.value.fetch_add(value, Ordering::Relaxed) + value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_counter() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);
        
        counter.increment();
        assert_eq!(counter.get(), 1);
        
        counter.fetch_add(5);
        assert_eq!(counter.get(), 6);
        
        counter.reset();
        assert_eq!(counter.get(), 0);
    }
    
    #[test]
    fn test_exponential_moving_average() {
        let mut ema = ExponentialMovingAverage::new(0.5); // 50% weight
        
        // First value is set directly
        ema.update(100.0);
        assert_eq!(ema.get(), 100.0);
        assert_eq!(ema.samples(), 1);
        
        // Second value is weighted average
        ema.update(200.0);
        assert_eq!(ema.get(), 150.0); // 0.5 * 200 + 0.5 * 100
        assert_eq!(ema.samples(), 2);
        
        // Third value is weighted average
        ema.update(300.0);
        assert_eq!(ema.get(), 225.0); // 0.5 * 300 + 0.5 * 150
        assert_eq!(ema.samples(), 3);
    }
    
    #[test]
    fn test_peer_performance() {
        let mut perf = PeerPerformance::new();
        
        // Test message statistics
        perf.record_message_received(100);
        perf.record_message_sent(200);
        
        assert_eq!(perf.messages_received.get(), 1);
        assert_eq!(perf.messages_sent.get(), 1);
        assert_eq!(perf.bytes_received.get(), 100);
        assert_eq!(perf.bytes_sent.get(), 200);
        
        // Test latency tracking
        perf.update_ping_latency(50);
        assert_eq!(perf.average_ping_latency().unwrap(), 50.0);
        
        // Test reliability
        perf.record_successful_response();
        perf.record_successful_response();
        perf.record_failed_response();
        
        assert_eq!(perf.success_rate(), 2.0 / 3.0);
        
        // Test misbehavior
        perf.record_invalid_message();
        perf.record_duplicate_message();
        
        assert_eq!(perf.misbehavior_rate(), 2.0 / 1.0);
        
        // Test performance score
        let score = perf.performance_score();
        assert!(score >= 0.0 && score <= 1.0);
    }
}
