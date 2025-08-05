//! Consensus monitoring and health checks for MICC consensus.
//!
//! This module provides comprehensive monitoring capabilities for MICC consensus,
//! including health metrics, security anomaly detection, and performance tracking.

use sp_consensus_slots::Slot;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use log::{debug, info, warn, error};

/// Consensus health metrics
#[derive(Debug, Clone)]
pub struct ConsensusMetrics {
    /// Total blocks produced
    pub blocks_produced: u64,
    /// Total slots seen
    pub slots_seen: u64,
    /// Empty slots (no block produced)
    pub empty_slots: u64,
    /// Average block production time
    pub avg_block_time: Duration,
    /// Number of forks detected
    pub forks_detected: u32,
    /// Number of consensus anomalies
    pub anomalies_detected: u32,
    /// Last block production time
    pub last_block_time: Option<Instant>,
    /// Current session authorities count
    pub authorities_count: u32,
    /// Network health status
    pub network_health: NetworkHealth,
}

impl Default for ConsensusMetrics {
    fn default() -> Self {
        Self {
            blocks_produced: 0,
            slots_seen: 0,
            empty_slots: 0,
            avg_block_time: Duration::from_millis(500), // Default 500ms
            forks_detected: 0,
            anomalies_detected: 0,
            last_block_time: None,
            authorities_count: 0,
            network_health: NetworkHealth::Unknown,
        }
    }
}

/// Network health status
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkHealth {
    /// Network health is unknown
    Unknown,
    /// Network is healthy
    Healthy,
    /// Network is degraded but functional
    Degraded,
    /// Network has serious issues
    Critical,
}

/// Security anomaly types
#[derive(Debug, Clone)]
pub enum SecurityAnomaly {
    /// Unexpected empty slots
    EmptySlotSpike {
        /// Number of consecutive empty slots detected
        consecutive_empty: u32,
        /// Threshold that was exceeded
        threshold: u32,
    },
    /// Block time variance anomaly
    BlockTimeAnomaly {
        /// Actual observed block time
        observed_time: Duration,
        /// Expected block time
        expected_time: Duration,
        /// Variance threshold that was exceeded
        variance_threshold: f64,
    },
    /// Fork detection
    ForkDetected {
        /// Slot where fork was detected
        slot: Slot,
        /// Number of conflicting blocks
        block_count: u32,
    },
    /// Authority set change anomaly
    AuthorityAnomaly {
        /// Expected number of authorities
        expected_count: u32,
        /// Actual number of authorities
        actual_count: u32,
    },
    /// Consensus stall detected
    ConsensusStall {
        /// Duration of the stall
        stall_duration: Duration,
        /// Last slot that produced a block
        last_block_slot: Slot,
        /// Current slot
        current_slot: Slot,
    },
}

/// Configuration for consensus monitoring
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Maximum consecutive empty slots before anomaly
    pub max_empty_slots: u32,
    /// Block time variance threshold (percentage)
    pub block_time_variance_threshold: f64,
    /// Stall detection threshold (duration)
    pub stall_threshold: Duration,
    /// Enable detailed logging
    pub enable_detailed_logging: bool,
    /// Metrics collection interval
    pub metrics_interval: Duration,
    /// Maximum metrics history to keep
    pub max_metrics_history: usize,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            max_empty_slots: 10,              // Alert after 10 consecutive empty slots
            block_time_variance_threshold: 0.5, // 50% variance threshold
            stall_threshold: Duration::from_secs(30), // 30 second stall threshold
            enable_detailed_logging: true,
            metrics_interval: Duration::from_secs(60), // Collect metrics every minute
            max_metrics_history: 1440,        // Keep 24 hours of minute-level metrics
        }
    }
}

/// Slot tracking information
#[derive(Debug, Clone)]
struct SlotInfo {
    slot: Slot,
    block_produced: bool,
    timestamp: Instant,
    authority_index: Option<u32>,
}

/// Consensus monitoring system
pub struct ConsensusMonitor<Block: BlockT> {
    /// Current metrics
    metrics: Arc<Mutex<ConsensusMetrics>>,
    /// Monitoring configuration
    config: MonitoringConfig,
    /// Recent slot information
    recent_slots: Mutex<VecDeque<SlotInfo>>,
    /// Metrics history
    metrics_history: Mutex<VecDeque<ConsensusMetrics>>,
    /// Detected anomalies
    anomalies: Mutex<Vec<SecurityAnomaly>>,
    /// Block hash to slot mapping
    block_slots: Mutex<BTreeMap<Block::Hash, Slot>>,
    /// Last metrics collection time
    last_metrics_time: Mutex<Option<Instant>>,
}

impl<Block: BlockT> ConsensusMonitor<Block> {
    /// Create a new consensus monitor
    pub fn new(config: MonitoringConfig) -> Self {
        Self {
            metrics: Arc::new(Mutex::new(ConsensusMetrics::default())),
            config,
            recent_slots: Mutex::new(VecDeque::new()),
            metrics_history: Mutex::new(VecDeque::new()),
            anomalies: Mutex::new(Vec::new()),
            block_slots: Mutex::new(BTreeMap::new()),
            last_metrics_time: Mutex::new(None),
        }
    }

    /// Record a new slot
    pub fn record_slot(&self, slot: Slot, authority_index: Option<u32>) {
        let slot_info = SlotInfo {
            slot,
            block_produced: false,
            timestamp: Instant::now(),
            authority_index,
        };

        {
            let mut recent_slots = self.recent_slots.lock().unwrap();
            recent_slots.push_back(slot_info);
            
            // Keep only recent slots (last 100)
            if recent_slots.len() > 100 {
                recent_slots.pop_front();
            }
        }

        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.slots_seen += 1;
        }

        // Check for empty slot anomalies
        self.check_empty_slot_anomalies();
        
        if self.config.enable_detailed_logging {
            debug!(target: "micc-monitor", "📊 Recorded slot {:?} with authority {:?}", slot, authority_index);
        }
    }

    /// Record a block production
    pub fn record_block_production<Header: HeaderT>(
        &self,
        header: &Header,
        slot: Slot,
        authority_index: Option<u32>,
    ) {
        let now = Instant::now();
        
        // Update slot information
        {
            let mut recent_slots = self.recent_slots.lock().unwrap();
            if let Some(slot_info) = recent_slots.iter_mut().find(|s| s.slot == slot) {
                slot_info.block_produced = true;
            }
        }

        // Store block-slot mapping (convert header hash to block hash)
        // Note: In a real implementation, we'd need the actual block hash
        // For now, we'll skip this mapping since we don't have access to the block hash

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.blocks_produced += 1;
            
            if let Some(last_time) = metrics.last_block_time {
                let block_time = now.duration_since(last_time);
                
                // Update average block time (exponential moving average)
                let alpha = 0.1; // Smoothing factor
                let new_avg = Duration::from_secs_f64(
                    alpha * block_time.as_secs_f64() + 
                    (1.0 - alpha) * metrics.avg_block_time.as_secs_f64()
                );
                metrics.avg_block_time = new_avg;
                
                // Check for block time anomalies
                self.check_block_time_anomaly(block_time);
            }
            
            metrics.last_block_time = Some(now);
        }

        // Check for forks
        self.check_for_forks(header, slot);

        if self.config.enable_detailed_logging {
            info!(target: "micc-monitor", "✅ Block produced for slot {:?} by authority {:?}", slot, authority_index);
        }

        // Collect metrics if interval has passed
        self.maybe_collect_metrics();
    }

    /// Check for empty slot anomalies
    fn check_empty_slot_anomalies(&self) {
        let recent_slots = self.recent_slots.lock().unwrap();
        let recent_count = recent_slots.len().min(self.config.max_empty_slots as usize);
        
        if recent_count < self.config.max_empty_slots as usize {
            return;
        }

        // Check last N slots for consecutive empty slots
        let consecutive_empty = recent_slots
            .iter()
            .rev()
            .take(recent_count)
            .take_while(|slot| !slot.block_produced)
            .count() as u32;

        if consecutive_empty >= self.config.max_empty_slots {
            let anomaly = SecurityAnomaly::EmptySlotSpike {
                consecutive_empty,
                threshold: self.config.max_empty_slots,
            };
            
            self.record_anomaly(anomaly);
            
            {
                let mut metrics = self.metrics.lock().unwrap();
                metrics.empty_slots += consecutive_empty as u64;
                metrics.network_health = NetworkHealth::Degraded;
            }
        }
    }

    /// Check for block time anomalies
    fn check_block_time_anomaly(&self, observed_time: Duration) {
        let expected_time = Duration::from_millis(500); // Expected 500ms block time
        let variance = (observed_time.as_secs_f64() - expected_time.as_secs_f64()).abs() 
                      / expected_time.as_secs_f64();

        if variance > self.config.block_time_variance_threshold {
            let anomaly = SecurityAnomaly::BlockTimeAnomaly {
                observed_time,
                expected_time,
                variance_threshold: self.config.block_time_variance_threshold,
            };
            
            self.record_anomaly(anomaly);
            
            warn!(target: "micc-monitor", 
                "⚠️ Block time anomaly: expected {:?}, observed {:?} (variance: {:.2}%)", 
                expected_time, observed_time, variance * 100.0
            );
        }
    }

    /// Check for potential forks
    fn check_for_forks<Header: HeaderT>(&self, _header: &Header, slot: Slot) {
        // This is a simplified fork detection
        // In a full implementation, we would track multiple blocks per slot
        // and detect when we see conflicting blocks
        
        // For now, we'll just count if we see multiple blocks for the same slot number
        // in a short time window (this would need more sophisticated logic)
        
        if self.config.enable_detailed_logging {
            debug!(target: "micc-monitor", "🔍 Checking for forks at slot {:?}", slot);
        }
    }

    /// Record a security anomaly
    fn record_anomaly(&self, anomaly: SecurityAnomaly) {
        error!(target: "micc-monitor", "🚨 Security anomaly detected: {:?}", anomaly);
        
        {
            let mut anomalies = self.anomalies.lock().unwrap();
            anomalies.push(anomaly);
            
            // Keep only recent anomalies (last 100)
            if anomalies.len() > 100 {
                anomalies.remove(0);
            }
        }
        
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.anomalies_detected += 1;
        }
    }

    /// Maybe collect metrics based on interval
    fn maybe_collect_metrics(&self) {
        let now = Instant::now();
        let should_collect = {
            let mut last_time = self.last_metrics_time.lock().unwrap();
            match *last_time {
                Some(last) if now.duration_since(last) >= self.config.metrics_interval => {
                    *last_time = Some(now);
                    true
                },
                None => {
                    *last_time = Some(now);
                    true
                },
                _ => false,
            }
        };

        if should_collect {
            self.collect_metrics_snapshot();
        }
    }

    /// Collect a snapshot of metrics
    fn collect_metrics_snapshot(&self) {
        let current_metrics = {
            let metrics = self.metrics.lock().unwrap();
            metrics.clone()
        };

        {
            let mut history = self.metrics_history.lock().unwrap();
            history.push_back(current_metrics.clone());
            
            if history.len() > self.config.max_metrics_history {
                history.pop_front();
            }
        }

        info!(target: "micc-monitor", 
            "📈 Metrics snapshot: blocks={}, slots={}, empty={}, health={:?}", 
            current_metrics.blocks_produced,
            current_metrics.slots_seen,
            current_metrics.empty_slots,
            current_metrics.network_health
        );
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> ConsensusMetrics {
        let metrics = self.metrics.lock().unwrap();
        metrics.clone()
    }

    /// Get recent anomalies
    pub fn get_recent_anomalies(&self) -> Vec<SecurityAnomaly> {
        let anomalies = self.anomalies.lock().unwrap();
        anomalies.clone()
    }

    /// Update authority count
    pub fn update_authority_count(&self, count: u32) {
        let mut metrics = self.metrics.lock().unwrap();
        
        if metrics.authorities_count > 0 && metrics.authorities_count != count {
            // Authority set changed
            let anomaly = SecurityAnomaly::AuthorityAnomaly {
                expected_count: metrics.authorities_count,
                actual_count: count,
            };
            drop(metrics); // Release lock before calling record_anomaly
            self.record_anomaly(anomaly);
            
            let mut metrics = self.metrics.lock().unwrap();
            metrics.authorities_count = count;
        } else {
            metrics.authorities_count = count;
        }
    }

    /// Check for consensus stalls
    pub fn check_consensus_stall(&self, current_slot: Slot) {
        let metrics = self.metrics.lock().unwrap();
        
        if let Some(last_time) = metrics.last_block_time {
            let stall_duration = Instant::now().duration_since(last_time);
            
            if stall_duration > self.config.stall_threshold {
                let anomaly = SecurityAnomaly::ConsensusStall {
                    stall_duration,
                    last_block_slot: Slot::from(0), // Would need to track this properly
                    current_slot,
                };
                
                drop(metrics); // Release lock
                self.record_anomaly(anomaly);
                
                let mut metrics = self.metrics.lock().unwrap();
                metrics.network_health = NetworkHealth::Critical;
            }
        }
    }

    /// Reset monitoring state for new session
    pub fn new_session(&self) {
        info!(target: "micc-monitor", "🔄 Starting new monitoring session");
        
        {
            let mut recent_slots = self.recent_slots.lock().unwrap();
            recent_slots.clear();
        }
        
        {
            let mut block_slots = self.block_slots.lock().unwrap();
            block_slots.clear();
        }
        
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.network_health = NetworkHealth::Healthy;
            // Don't reset counters, as they're cumulative
        }
    }

    /// Get metrics summary for health checks
    pub fn get_health_summary(&self) -> String {
        let metrics = self.get_metrics();
        let anomalies = self.get_recent_anomalies();
        
        format!(
            "MICC Consensus Health Summary:\n\
             - Blocks Produced: {}\n\
             - Slots Seen: {}\n\
             - Empty Slots: {}\n\
             - Average Block Time: {:?}\n\
             - Network Health: {:?}\n\
             - Recent Anomalies: {}\n\
             - Authorities: {}",
            metrics.blocks_produced,
            metrics.slots_seen,
            metrics.empty_slots,
            metrics.avg_block_time,
            metrics.network_health,
            anomalies.len(),
            metrics.authorities_count
        )
    }
}

/// Helper functions for monitoring
pub mod utils {
    use super::*;

    /// Calculate consensus efficiency (blocks produced / slots seen)
    pub fn calculate_efficiency(metrics: &ConsensusMetrics) -> f64 {
        if metrics.slots_seen == 0 {
            0.0
        } else {
            metrics.blocks_produced as f64 / metrics.slots_seen as f64
        }
    }

    /// Determine if the network is healthy based on metrics
    pub fn assess_network_health(metrics: &ConsensusMetrics) -> NetworkHealth {
        let efficiency = calculate_efficiency(metrics);
        
        if efficiency < 0.5 {
            NetworkHealth::Critical
        } else if efficiency < 0.8 || metrics.anomalies_detected > 10 {
            NetworkHealth::Degraded
        } else {
            NetworkHealth::Healthy
        }
    }

    /// Format duration for human-readable output
    pub fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();
        
        if secs > 0 {
            format!("{}.{:03}s", secs, millis)
        } else {
            format!("{}ms", millis)
        }
    }
}