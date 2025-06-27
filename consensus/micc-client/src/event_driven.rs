// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Event-driven transaction pool watcher for MICC consensus.
//!
//! This module provides event-driven block production by monitoring transaction pool
//! events instead of polling at regular intervals.

use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    stream::{Stream, StreamExt},
    Future,
};
use sc_transaction_pool_api::{InPoolTransaction, TransactionPool, TransactionPriority};
use sp_runtime::traits::Block as BlockT;
use std::{
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use log::{debug, info, warn};
use tokio::time::{sleep, Sleep};
use sc_consensus_slots::SlotTrigger;

const LOG_TARGET: &str = "micc::event_driven";

/// Transaction pool events for event-driven block production.
#[derive(Debug, Clone)]
pub enum PoolEvent<Hash = TransactionHash> {
    /// A new transaction was added to the ready queue.
    TransactionAdded(Hash),
    /// A high-priority transaction was added to the ready queue.
    HighPriorityTransactionAdded(Hash, TransactionPriority),
    /// A transaction was removed from the pool.
    TransactionRemoved(Hash),
    /// Pool status update with number of ready transactions and highest priority.
    PoolReady(usize, Option<TransactionPriority>),
    /// Pool became empty.
    PoolEmpty,
    /// Network load indicator (transactions per second over recent history).
    NetworkLoad(f64),
}

/// Transaction hash type alias.
pub type TransactionHash = sp_core::H256;

/// Smart collection window configuration for optimized block production.
#[derive(Clone, Debug)]
pub struct CollectionConfig {
    /// Minimum collection time before producing a block.
    pub min_collection_time: Duration,
    /// Maximum collection time to wait for more transactions.
    pub max_collection_time: Duration,
    /// Maximum number of transactions per block.
    pub max_batch_size: usize,
    /// Priority threshold for immediate block production.
    pub priority_threshold: TransactionPriority,
    /// Network load factor for dynamic timing (0.1 to 2.0).
    pub network_load_factor: f64,
    /// Enable adaptive collection windows based on transaction flow.
    pub enable_adaptive_timing: bool,
}

impl Default for CollectionConfig {
    fn default() -> Self {
        Self {
            min_collection_time: Duration::from_millis(100),
            max_collection_time: Duration::from_millis(400),
            max_batch_size: 1000,
            priority_threshold: TransactionPriority::MAX / 2, // High priority threshold for immediate production
            network_load_factor: 1.0,
            enable_adaptive_timing: true,
        }
    }
}

/// Configuration for event-driven block production with smart collection.
#[derive(Debug, Clone)]
pub struct EventDrivenConfig {
    /// Smart collection window configuration.
    pub collection: CollectionConfig,
    /// Interval for empty block production (None = no empty blocks).
    pub empty_block_interval_ms: Option<u64>,
    /// Enable priority fast-track for high-priority transactions.
    pub enable_priority_fast_track: bool,
    /// Historical transaction rate for adaptive timing.
    pub transaction_rate_history_size: usize,
}

impl Default for EventDrivenConfig {
    fn default() -> Self {
        Self {
            collection: CollectionConfig::default(),
            empty_block_interval_ms: Some(3600000), // 1 hour
            enable_priority_fast_track: true,
            transaction_rate_history_size: 10,
        }
    }
}

/// Transaction pool watcher for event-driven block production.
pub struct TransactionPoolWatcher<Block, Client, Pool> {
    pool: Arc<Pool>,
    event_sender: UnboundedSender<PoolEvent>,
    config: EventDrivenConfig,
    last_pool_status: std::sync::Mutex<usize>,
    _phantom: PhantomData<(Block, Client)>,
}

impl<Block, Client, Pool> TransactionPoolWatcher<Block, Client, Pool>
where
    Block: BlockT,
    Pool: TransactionPool<Block = Block> + 'static,
    <Pool as TransactionPool>::Hash: Default,
{
    /// Create a new transaction pool watcher.
    pub fn new(pool: Arc<Pool>, config: EventDrivenConfig) -> Self {
        let (event_sender, _) = unbounded();
        
        Self {
            pool,
            event_sender,
            config,
            last_pool_status: std::sync::Mutex::new(0),
            _phantom: PhantomData,
        }
    }

    /// Start watching the transaction pool for events.
    /// Returns a receiver for pool events.
    pub fn start_watching(&self) -> UnboundedReceiver<PoolEvent<<Pool as TransactionPool>::Hash>> {
        let (sender, receiver) = unbounded();
        
        // Clone the data we need for the background task
        let pool = self.pool.clone();
        let config = self.config.clone();
        let initial_status = self.pool.status().ready;
        let last_status = Arc::new(std::sync::Mutex::new(initial_status));
        
        // Spawn background task to monitor transaction pool
        tokio::spawn(async move {
            Self::monitor_pool_events(pool, sender, config, last_status).await;
        });
        
        receiver
    }

    /// Monitor transaction pool events in the background using true event-driven approach.
    async fn monitor_pool_events(
        pool: Arc<Pool>,
        sender: UnboundedSender<PoolEvent<<Pool as TransactionPool>::Hash>>,
        _config: EventDrivenConfig,
        last_status: Arc<std::sync::Mutex<usize>>,
    ) {
        info!(target: LOG_TARGET, "Starting true event-driven transaction pool monitoring");
        
        // Get the import notification stream for immediate transaction notifications
        let mut import_stream = pool.import_notification_stream();
        let mut status_check_interval = tokio::time::interval(Duration::from_millis(500)); // Backup polling at 500ms
        
        info!(target: LOG_TARGET, "Subscribing to transaction pool import events");
        
        loop {
            tokio::select! {
                // PRIMARY: Immediate response to transaction import events
                tx_hash = import_stream.next() => {
                    match tx_hash {
                        Some(hash) => {
                            info!(target: LOG_TARGET, "Transaction imported to pool: {:?}", hash);
                            
                            // Send immediate notification of transaction added
                            if sender.unbounded_send(PoolEvent::TransactionAdded(hash)).is_err() {
                                warn!(target: LOG_TARGET, "Event sender closed, stopping pool monitoring");
                                break;
                            }
                            
                            // Get current pool status and send update
                            let status = pool.status();
                            let ready_count = status.ready;
                            
                            // Update last status
                            {
                                let mut last = last_status.lock().unwrap();
                                *last = ready_count;
                            }
                            
                            // Send pool ready event with current transaction count
                            let event = if ready_count > 0 {
                                PoolEvent::PoolReady(ready_count, None) // Priority detection would need deeper integration
                            } else {
                                PoolEvent::PoolEmpty
                            };
                            
                            if sender.unbounded_send(event).is_err() {
                                warn!(target: LOG_TARGET, "Event sender closed, stopping pool monitoring");
                                break;
                            }
                        }
                        None => {
                            warn!(target: LOG_TARGET, "Transaction import stream ended, falling back to polling");
                            break;
                        }
                    }
                }
                
                // BACKUP: Periodic status check to catch any missed events
                _ = status_check_interval.tick() => {
                    let status = pool.status();
                    let ready_count = status.ready;
                    
                    let last_count = {
                        let mut last = last_status.lock().unwrap();
                        let prev = *last;
                        *last = ready_count;
                        prev
                    };
                    
                    // Only send update if status changed and we missed it
                    if ready_count != last_count {
                        debug!(target: LOG_TARGET, "Backup status check detected change: {} -> {} ready transactions", last_count, ready_count);
                        
                        let event = if ready_count > 0 {
                            PoolEvent::PoolReady(ready_count, None)
                        } else {
                            PoolEvent::PoolEmpty
                        };
                        
                        if sender.unbounded_send(event).is_err() {
                            warn!(target: LOG_TARGET, "Event sender closed, stopping pool monitoring");
                            break;
                        }
                    }
                }
            }
        }
        
        // Fallback to polling-only mode if import stream fails
        warn!(target: LOG_TARGET, "Falling back to polling-only transaction pool monitoring");
        let mut polling_interval = tokio::time::interval(Duration::from_millis(100));
        
        loop {
            polling_interval.tick().await;
            
            let status = pool.status();
            let ready_count = status.ready;
            
            let last_count = {
                let mut last = last_status.lock().unwrap();
                let prev = *last;
                *last = ready_count;
                prev
            };
            
            if ready_count != last_count {
                info!(target: LOG_TARGET, "Polling detected pool status change: {} -> {} ready transactions", last_count, ready_count);
                
                if ready_count > last_count {
                    // Create a default hash for the transaction added event when polling
                    let default_hash = <Pool as TransactionPool>::Hash::default();
                    if sender.unbounded_send(PoolEvent::TransactionAdded(default_hash)).is_err() {
                        break;
                    }
                }
                
                let event = if ready_count > 0 {
                    PoolEvent::PoolReady(ready_count, None)
                } else {
                    PoolEvent::PoolEmpty
                };
                
                if sender.unbounded_send(event).is_err() {
                    break;
                }
            }
        }
        
        info!(target: LOG_TARGET, "Transaction pool event monitoring stopped");
    }
}

/// Smart collection window manager with adaptive timing.
#[derive(Debug)]
pub struct SmartCollectionWindow {
    /// Start time of the collection window.
    pub started_at: Instant,
    /// Adaptive duration based on network conditions.
    pub duration: Duration,
    /// Number of transactions when window started.
    pub initial_tx_count: usize,
    /// Highest priority transaction in this window.
    pub highest_priority: Option<TransactionPriority>,
    /// Network load factor at window start.
    pub network_load: f64,
    /// Timer for the collection window.
    pub timer: Pin<Box<Sleep>>,
    /// Whether this window was triggered by a high-priority transaction.
    pub priority_triggered: bool,
}

/// Network load tracker for adaptive collection windows.
#[derive(Debug)]
pub struct NetworkLoadTracker {
    /// Transaction timestamps for rate calculation.
    transaction_history: Vec<Instant>,
    /// Maximum history size.
    max_history_size: usize,
    /// Current transactions per second.
    current_tps: f64,
    /// Last calculation time.
    last_calculation: Instant,
}

impl NetworkLoadTracker {
    /// Create a new network load tracker with specified history size.
    pub fn new(max_history_size: usize) -> Self {
        Self {
            transaction_history: Vec::new(),
            max_history_size,
            current_tps: 0.0,
            last_calculation: Instant::now(),
        }
    }

    /// Record a new transaction and calculate current TPS.
    pub fn record_transaction(&mut self) -> f64 {
        let now = Instant::now();
        self.transaction_history.push(now);

        // Keep only recent transactions within the last 10 seconds
        let cutoff = now - Duration::from_secs(10);
        self.transaction_history.retain(|&timestamp| timestamp > cutoff);

        // Limit history size
        if self.transaction_history.len() > self.max_history_size {
            self.transaction_history.drain(0..self.transaction_history.len() - self.max_history_size);
        }

        // Calculate TPS if enough time has passed
        if now.duration_since(self.last_calculation) > Duration::from_millis(500) {
            self.current_tps = self.calculate_tps();
            self.last_calculation = now;
        }

        self.current_tps
    }

    /// Get current transactions per second.
    pub fn get_tps(&self) -> f64 {
        self.current_tps
    }

    /// Calculate TPS from transaction history.
    fn calculate_tps(&self) -> f64 {
        if self.transaction_history.len() < 2 {
            return 0.0;
        }

        let now = Instant::now();
        let oldest = self.transaction_history[0];
        let duration = now.duration_since(oldest).as_secs_f64();

        if duration > 0.0 {
            self.transaction_history.len() as f64 / duration
        } else {
            0.0
        }
    }
}

/// Legacy collection window for backwards compatibility.
#[derive(Debug)]
pub struct CollectionWindow {
    /// Start time of the collection window.
    pub started_at: Instant,
    /// Duration of the collection window.
    pub duration: Duration,
    /// Number of transactions when window started.
    pub initial_tx_count: usize,
    /// Timer for the collection window.
    pub timer: Pin<Box<Sleep>>,
}

impl SmartCollectionWindow {
    /// Create a new smart collection window with adaptive timing.
    pub fn new(
        config: &CollectionConfig,
        initial_tx_count: usize,
        highest_priority: Option<TransactionPriority>,
        network_load: f64,
        priority_triggered: bool,
    ) -> Self {
        let duration = Self::calculate_adaptive_duration(config, network_load, highest_priority, priority_triggered);
        
        info!(target: LOG_TARGET, 
            "Creating smart collection window: duration={:?}, tx_count={}, priority={:?}, load={:.2}, priority_triggered={}", 
            duration, initial_tx_count, highest_priority, network_load, priority_triggered
        );

        Self {
            started_at: Instant::now(),
            duration,
            initial_tx_count,
            highest_priority,
            network_load,
            timer: Box::pin(sleep(duration)),
            priority_triggered,
        }
    }

    /// Calculate adaptive duration based on network conditions and priority.
    fn calculate_adaptive_duration(
        config: &CollectionConfig,
        network_load: f64,
        highest_priority: Option<TransactionPriority>,
        priority_triggered: bool,
    ) -> Duration {
        let base_duration = if priority_triggered {
            // Shorter window for priority transactions
            config.min_collection_time
        } else {
            // Standard window duration
            Duration::from_millis(
                ((config.min_collection_time.as_millis() + config.max_collection_time.as_millis()) / 2) as u64
            )
        };

        if !config.enable_adaptive_timing {
            return base_duration.clamp(config.min_collection_time, config.max_collection_time);
        }

        // Adjust based on network load
        let load_factor = if network_load > 5.0 {
            // High load: shorter windows to process transactions faster
            0.7
        } else if network_load > 2.0 {
            // Medium load: slightly shorter windows
            0.85
        } else if network_load < 0.5 {
            // Low load: longer windows to batch more transactions
            1.3
        } else {
            // Normal load
            1.0
        };

        // Adjust for transaction priority
        let priority_factor = if let Some(priority) = highest_priority {
            if priority >= config.priority_threshold {
                // Very high priority: immediate production
                0.1
            } else if priority > config.priority_threshold / 2 {
                // High priority: shorter window
                0.5
            } else {
                // Normal priority
                1.0
            }
        } else {
            1.0
        };

        let adjusted_duration = Duration::from_nanos(
            (base_duration.as_nanos() as f64 * load_factor * priority_factor) as u64
        );

        adjusted_duration.clamp(config.min_collection_time, config.max_collection_time)
    }

    /// Check if the collection window has expired.
    pub fn is_expired(&mut self) -> bool {
        matches!(self.timer.as_mut().poll(&mut Context::from_waker(&futures::task::noop_waker())), Poll::Ready(()))
    }
    
    /// Get elapsed time since window started.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Update window with new high-priority transaction.
    pub fn update_priority(&mut self, priority: TransactionPriority, config: &CollectionConfig) {
        if let Some(current_priority) = self.highest_priority {
            if priority > current_priority {
                self.highest_priority = Some(priority);
                
                // If this is a very high priority transaction, shorten the window
                if priority >= config.priority_threshold {
                    let remaining = self.duration.saturating_sub(self.elapsed());
                    let new_remaining = remaining.min(Duration::from_millis(100));
                    self.timer = Box::pin(sleep(new_remaining));
                    
                    info!(target: LOG_TARGET, 
                        "Updated collection window for high priority transaction: new_duration={:?}", 
                        new_remaining
                    );
                }
            }
        } else {
            self.highest_priority = Some(priority);
        }
    }
}

impl CollectionWindow {
    /// Create a new collection window.
    pub fn new(duration: Duration, initial_tx_count: usize) -> Self {
        Self {
            started_at: Instant::now(),
            duration,
            initial_tx_count,
            timer: Box::pin(sleep(duration)),
        }
    }
    
    /// Check if the collection window has expired.
    pub fn is_expired(&mut self) -> bool {
        matches!(self.timer.as_mut().poll(&mut Context::from_waker(&futures::task::noop_waker())), Poll::Ready(()))
    }
    
    /// Get elapsed time since window started.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

/// Smart event-driven block production controller with adaptive collection.
pub struct SmartEventDrivenController<Block: BlockT> {
    config: EventDrivenConfig,
    collection_window: Option<SmartCollectionWindow>,
    empty_block_timer: Option<Pin<Box<Sleep>>>,
    pending_transactions: usize,
    last_block_time: Instant,
    network_load_tracker: NetworkLoadTracker,
    _phantom: PhantomData<Block>,
}

/// Legacy event-driven block production controller.
pub struct EventDrivenController<Block: BlockT> {
    config: EventDrivenConfig,
    collection_window: Option<CollectionWindow>,
    empty_block_timer: Option<Pin<Box<Sleep>>>,
    pending_transactions: usize,
    last_block_time: Instant,
    _phantom: PhantomData<Block>,
}

impl<Block: BlockT> SmartEventDrivenController<Block> {
    /// Create a new smart event-driven controller.
    pub fn new(config: EventDrivenConfig) -> Self {
        let empty_block_timer = config.empty_block_interval_ms.map(|interval_ms| {
            Box::pin(sleep(Duration::from_millis(interval_ms)))
        });
        
        Self {
            config: config.clone(),
            collection_window: None,
            empty_block_timer,
            pending_transactions: 0,
            last_block_time: Instant::now(),
            network_load_tracker: NetworkLoadTracker::new(config.transaction_rate_history_size),
            _phantom: PhantomData,
        }
    }
    
    /// Handle a pool event and determine if block production should be triggered.
    pub fn handle_pool_event(&mut self, event: PoolEvent) -> BlockProductionTrigger {
        match event {
            PoolEvent::TransactionAdded(_) => {
                let tps = self.network_load_tracker.record_transaction();
                debug!(target: LOG_TARGET, "Transaction added, current TPS: {:.2}", tps);
                BlockProductionTrigger::None
            },
            
            PoolEvent::HighPriorityTransactionAdded(_, priority) => {
                let tps = self.network_load_tracker.record_transaction();
                info!(target: LOG_TARGET, "High priority transaction added: priority={}, TPS={:.2}", priority, tps);
                
                // Update existing window or trigger immediate production
                if let Some(ref mut window) = self.collection_window {
                    window.update_priority(priority, &self.config.collection);
                    
                    // Check if we should produce immediately for very high priority
                    if priority >= self.config.collection.priority_threshold {
                        self.collection_window = None;
                        return BlockProductionTrigger::ProduceImmediately;
                    }
                } else {
                    // Start new priority window
                    return self.start_collection_window_for_priority(priority, tps);
                }
                
                BlockProductionTrigger::None
            },
            
            PoolEvent::PoolReady(count, highest_priority) => {
                let tps = self.network_load_tracker.record_transaction();
                self.pending_transactions = count;
                
                // Start collection window if not already active
                if self.collection_window.is_none() && count > 0 {
                    info!(target: LOG_TARGET, "Starting collection window for {} transactions (TPS: {:.2})", count, tps);
                    
                    let priority_triggered = highest_priority
                        .map(|p| p >= self.config.collection.priority_threshold / 2)
                        .unwrap_or(false);
                    
                    self.collection_window = Some(SmartCollectionWindow::new(
                        &self.config.collection,
                        count,
                        highest_priority,
                        tps,
                        priority_triggered,
                    ));
                    
                    return BlockProductionTrigger::StartCollectionWindow;
                }
                
                // Check if we should produce immediately for large batches or high priority
                if self.should_produce_immediately(count, highest_priority) {
                    self.collection_window = None;
                    return BlockProductionTrigger::ProduceImmediately;
                }
                
                BlockProductionTrigger::None
            }
            
            PoolEvent::PoolEmpty => {
                self.pending_transactions = 0;
                if self.collection_window.is_some() {
                    warn!(target: LOG_TARGET, "Pool became empty, canceling collection window");
                    self.collection_window = None;
                }
                BlockProductionTrigger::None
            }
            
            PoolEvent::TransactionRemoved(_) => {
                BlockProductionTrigger::None
            }
            
            PoolEvent::NetworkLoad(tps) => {
                debug!(target: LOG_TARGET, "Network load update: {:.2} TPS", tps);
                BlockProductionTrigger::None
            }
        }
    }
    
    /// Start a new collection window for a high-priority transaction.
    fn start_collection_window_for_priority(
        &mut self, 
        priority: TransactionPriority, 
        network_load: f64
    ) -> BlockProductionTrigger {
        // Immediate production for very high priority
        if priority >= self.config.collection.priority_threshold {
            info!(target: LOG_TARGET, "Immediate production for very high priority transaction: {}", priority);
            return BlockProductionTrigger::ProduceImmediately;
        }
        
        // Start priority collection window
        self.collection_window = Some(SmartCollectionWindow::new(
            &self.config.collection,
            self.pending_transactions,
            Some(priority),
            network_load,
            true, // priority_triggered
        ));
        
        BlockProductionTrigger::StartCollectionWindow
    }
    
    /// Check if collection window has expired.
    pub fn check_collection_window(&mut self) -> BlockProductionTrigger {
        if let Some(ref mut window) = self.collection_window {
            if window.is_expired() {
                let elapsed = window.elapsed();
                info!(target: LOG_TARGET, 
                    "Smart collection window expired after {:?} with {} transactions (priority: {:?})", 
                    elapsed, self.pending_transactions, window.highest_priority
                );
                
                self.collection_window = None;
                
                if self.pending_transactions > 0 {
                    return BlockProductionTrigger::CollectionWindowExpired;
                }
            }
        }
        
        BlockProductionTrigger::None
    }
    
    /// Check if empty block timer has expired.
    pub fn check_empty_block_timer(&mut self) -> BlockProductionTrigger {
        if let Some(ref mut timer) = self.empty_block_timer {
            if matches!(timer.as_mut().poll(&mut Context::from_waker(&futures::task::noop_waker())), Poll::Ready(())) {
                info!(target: LOG_TARGET, "Empty block timer expired, producing empty block");
                
                // Reset timer
                if let Some(interval_ms) = self.config.empty_block_interval_ms {
                    self.empty_block_timer = Some(Box::pin(sleep(Duration::from_millis(interval_ms))));
                }
                
                return BlockProductionTrigger::EmptyBlockTimer;
            }
        }
        
        BlockProductionTrigger::None
    }
    
    /// Record that a block was produced.
    pub fn record_block_produced(&mut self) {
        self.last_block_time = Instant::now();
        self.collection_window = None;
        
        // Reset empty block timer
        if let Some(interval_ms) = self.config.empty_block_interval_ms {
            self.empty_block_timer = Some(Box::pin(sleep(Duration::from_millis(interval_ms))));
        }
        
        info!(target: LOG_TARGET, "Block produced, resetting collection state");
    }
    
    /// Check if we should produce a block immediately.
    fn should_produce_immediately(&self, tx_count: usize, highest_priority: Option<TransactionPriority>) -> bool {
        // Produce immediately if we have a large batch
        if tx_count >= self.config.collection.max_batch_size {
            info!(target: LOG_TARGET, "Large batch detected: {} >= {}", tx_count, self.config.collection.max_batch_size);
            return true;
        }
        
        // Produce immediately for very high priority transactions
        if let Some(priority) = highest_priority {
            if priority >= self.config.collection.priority_threshold {
                info!(target: LOG_TARGET, "Very high priority transaction detected: {}", priority);
                return true;
            }
        }
        
        // Check if collection window is taking too long
        if let Some(ref window) = self.collection_window {
            if window.elapsed() >= self.config.collection.max_collection_time {
                info!(target: LOG_TARGET, "Collection window timeout");
                return true;
            }
        }
        
        false
    }
    
    /// Get current network load (TPS).
    pub fn get_network_load(&self) -> f64 {
        self.network_load_tracker.get_tps()
    }
}

impl<Block: BlockT> EventDrivenController<Block> {
    /// Create a new event-driven controller.
    pub fn new(config: EventDrivenConfig) -> Self {
        let empty_block_timer = config.empty_block_interval_ms.map(|interval_ms| {
            Box::pin(sleep(Duration::from_millis(interval_ms)))
        });
        
        Self {
            config,
            collection_window: None,
            empty_block_timer,
            pending_transactions: 0,
            last_block_time: Instant::now(),
            _phantom: PhantomData,
        }
    }
    
    /// Handle a pool event and determine if block production should be triggered.
    pub fn handle_pool_event(&mut self, event: PoolEvent) -> BlockProductionTrigger {
        match event {
            PoolEvent::TransactionAdded(_) => {
                // We need to get current count from status
                BlockProductionTrigger::None
            },
            PoolEvent::HighPriorityTransactionAdded(_, priority) => {
                // For legacy controller, treat high priority transactions as immediate triggers
                if priority >= TransactionPriority::MAX / 2 {
                    info!(target: LOG_TARGET, "High priority transaction detected: {}, producing immediately", priority);
                    self.collection_window = None;
                    return BlockProductionTrigger::ProduceImmediately;
                }
                BlockProductionTrigger::None
            },
            PoolEvent::PoolReady(count, _priority) => {
                self.pending_transactions = count;
                
                // Start collection window if not already active
                if self.collection_window.is_none() && count > 0 {
                    let duration = self.calculate_collection_window_duration(count);
                    info!(target: LOG_TARGET, "Starting collection window for {} transactions (duration: {:?})", count, duration);
                    
                    self.collection_window = Some(CollectionWindow::new(duration, count));
                    return BlockProductionTrigger::StartCollectionWindow;
                }
                
                // Check if we should produce immediately for high-priority or large batches
                if self.should_produce_immediately(count) {
                    self.collection_window = None;
                    return BlockProductionTrigger::ProduceImmediately;
                }
                
                BlockProductionTrigger::None
            }
            
            PoolEvent::PoolEmpty => {
                self.pending_transactions = 0;
                // Cancel collection window if pool becomes empty
                if self.collection_window.is_some() {
                    warn!(target: LOG_TARGET, "Pool became empty, canceling collection window");
                    self.collection_window = None;
                }
                BlockProductionTrigger::None
            }
            
            PoolEvent::TransactionRemoved(_) => {
                // Pool status will be updated by the next PoolReady/PoolEmpty event
                BlockProductionTrigger::None
            }
            
            PoolEvent::NetworkLoad(_tps) => {
                // Legacy controller ignores network load
                BlockProductionTrigger::None
            }
        }
    }
    
    /// Check if collection window has expired.
    pub fn check_collection_window(&mut self) -> BlockProductionTrigger {
        if let Some(ref mut window) = self.collection_window {
            if window.is_expired() {
                let elapsed = window.elapsed();
                info!(target: LOG_TARGET, "Collection window expired after {:?} with {} transactions", elapsed, self.pending_transactions);
                
                self.collection_window = None;
                
                if self.pending_transactions > 0 {
                    return BlockProductionTrigger::CollectionWindowExpired;
                }
            }
        }
        
        BlockProductionTrigger::None
    }
    
    /// Check if empty block timer has expired.
    pub fn check_empty_block_timer(&mut self) -> BlockProductionTrigger {
        if let Some(ref mut timer) = self.empty_block_timer {
            if matches!(timer.as_mut().poll(&mut Context::from_waker(&futures::task::noop_waker())), Poll::Ready(())) {
                info!(target: LOG_TARGET, "Empty block timer expired, producing empty block");
                
                // Reset timer
                if let Some(interval_ms) = self.config.empty_block_interval_ms {
                    self.empty_block_timer = Some(Box::pin(sleep(Duration::from_millis(interval_ms))));
                }
                
                return BlockProductionTrigger::EmptyBlockTimer;
            }
        }
        
        BlockProductionTrigger::None
    }
    
    /// Record that a block was produced.
    pub fn record_block_produced(&mut self) {
        self.last_block_time = Instant::now();
        self.collection_window = None;
        
        // Reset empty block timer
        if let Some(interval_ms) = self.config.empty_block_interval_ms {
            self.empty_block_timer = Some(Box::pin(sleep(Duration::from_millis(interval_ms))));
        }
    }
    
    /// Calculate optimal collection window duration based on transaction count and network load.
    fn calculate_collection_window_duration(&self, tx_count: usize) -> Duration {
        let base_duration = Duration::from_millis(1000); // 1 second
        let min_duration = self.config.collection.min_collection_time;
        let max_duration = self.config.collection.max_collection_time;
        
        // Adjust duration based on transaction count
        // More transactions = shorter window (assuming more are coming)
        // Fewer transactions = longer window (wait for more)
        let duration = if tx_count > 50 {
            min_duration
        } else if tx_count > 10 {
            base_duration
        } else {
            max_duration
        };
        
        duration.clamp(min_duration, max_duration)
    }
    
    /// Check if we should produce a block immediately without waiting for collection window.
    fn should_produce_immediately(&self, tx_count: usize) -> bool {
        // Produce immediately if we have a large batch
        if tx_count >= self.config.collection.max_batch_size {
            return true;
        }
        
        // Check if collection window is taking too long
        if let Some(ref window) = self.collection_window {
            if window.elapsed() >= self.config.collection.max_collection_time {
                return true;
            }
        }
        
        false
    }
}

/// Trigger for block production decisions.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockProductionTrigger {
    /// No action needed.
    None,
    /// Start collection window for incoming transactions.
    StartCollectionWindow,
    /// Produce block immediately (high priority or large batch).
    ProduceImmediately,
    /// Collection window expired, produce block with current transactions.
    CollectionWindowExpired,
    /// Empty block timer expired, produce empty block.
    EmptyBlockTimer,
}

impl BlockProductionTrigger {
    /// Check if this trigger should result in block production.
    pub fn should_produce_block(&self) -> bool {
        matches!(
            self,
            BlockProductionTrigger::ProduceImmediately |
            BlockProductionTrigger::CollectionWindowExpired |
            BlockProductionTrigger::EmptyBlockTimer
        )
    }
}

/// Create a true event-driven stream using transaction pool import notifications.
/// This replaces polling with immediate response to transaction imports.
pub fn create_true_event_driven_stream<Block, Pool>(
    pool: Arc<Pool>,
    config: EventDrivenConfig,
) -> Pin<Box<dyn Stream<Item = SlotTrigger> + Send + 'static>>
where
    Block: sp_runtime::traits::Block,
    Pool: sc_transaction_pool_api::TransactionPool<Block = Block> + 'static,
{
    const LOG_TARGET: &str = "micc::true_event_driven";
    
    info!(target: LOG_TARGET, "True event-driven stream initialized with config: {:?}", config);
    
    let pool_clone = pool.clone();
    let config_clone = config.clone();
    
    info!(target: LOG_TARGET, "Starting true event-driven transaction monitoring with import notifications");
    
    // Use unfold to create a true event-driven monitoring stream
    let initial_status = pool.status().ready; // Initialize with current pool status to avoid false positives
    Box::pin(futures::stream::unfold(
        (pool.import_notification_stream(), initial_status, None::<Instant>, tokio::time::interval(Duration::from_millis(500))),
        move |(mut import_stream, mut last_status, mut collection_timer, mut backup_interval)| {
            let pool = pool_clone.clone();
            let config = config_clone.clone();
            
            async move {
                tokio::select! {
                    // PRIMARY: Immediate response to transaction imports
                    tx_hash = import_stream.next() => {
                        match tx_hash {
                            Some(hash) => {
                                info!(target: LOG_TARGET, "Transaction import detected: {:?}", hash);
                                
                                let status = pool.status();
                                let ready_count = status.ready;
                                last_status = ready_count;
                                
                                // Check priority of ready transactions for immediate production
                                if ready_count > 0 {
                                    let ready_transactions = pool.ready();
                                    let highest_priority = ready_transactions
                                        .map(|tx| *tx.priority())
                                        .max()
                                        .unwrap_or(0);
                                    
                                    // Check if high-priority transaction should trigger immediate block production
                                    if highest_priority >= config.collection.priority_threshold {
                                        info!(target: LOG_TARGET, "High-priority transaction detected (priority: {}), producing block immediately", highest_priority);
                                        collection_timer = None;
                                        return Some((SlotTrigger::CreateBlock, (import_stream, last_status, collection_timer, backup_interval)));
                                    }
                                    
                                    // Start collection window if not already active (for non-high-priority transactions)
                                    if collection_timer.is_none() {
                                        let collection_duration = calculate_collection_duration(&config, ready_count);
                                        collection_timer = Some(Instant::now() + collection_duration);
                                        info!(target: LOG_TARGET, "Starting collection window for {}ms with {} ready transactions (highest priority: {})", 
                                            collection_duration.as_millis(), ready_count, highest_priority);
                                        
                                        // Check if we should produce immediately for large batches
                                        if ready_count >= config.collection.max_batch_size {
                                            info!(target: LOG_TARGET, "Large batch detected ({} transactions), producing block immediately", ready_count);
                                            collection_timer = None;
                                            return Some((SlotTrigger::CreateBlock, (import_stream, last_status, collection_timer, backup_interval)));
                                        }
                                    }
                                }
                                
                                return Some((SlotTrigger::NoAction, (import_stream, last_status, collection_timer, backup_interval)));
                            }
                            None => {
                                warn!(target: LOG_TARGET, "Import stream ended, falling back to polling");
                                // Fall through to backup polling
                            }
                        }
                    }
                    
                    // BACKUP: Periodic status check to catch any missed events
                    _ = backup_interval.tick() => {
                        let status = pool.status();
                        let ready_count = status.ready;
                        
                        // Check if collection window has expired
                        if let Some(expire_time) = collection_timer {
                            if Instant::now() >= expire_time {
                                if ready_count > 0 {
                                    info!(target: LOG_TARGET, "Collection window expired, producing block with {} transactions", ready_count);
                                    collection_timer = None;
                                    last_status = ready_count;
                                    return Some((SlotTrigger::CreateBlock, (import_stream, last_status, collection_timer, backup_interval)));
                                } else {
                                    collection_timer = None;
                                }
                            }
                        }
                        
                        // Detect status changes that might have been missed
                        if ready_count != last_status {
                            debug!(target: LOG_TARGET, "Backup check detected pool status change: {} -> {} ready transactions", last_status, ready_count);
                            last_status = ready_count;
                            
                            if ready_count > 0 && collection_timer.is_none() {
                                // Check priority for immediate production
                                let ready_transactions = pool.ready();
                                let highest_priority = ready_transactions
                                    .map(|tx| *tx.priority())
                                    .max()
                                    .unwrap_or(0);
                                
                                if highest_priority >= config.collection.priority_threshold {
                                    info!(target: LOG_TARGET, "Backup check: High-priority transaction detected (priority: {}), producing block immediately", highest_priority);
                                    return Some((SlotTrigger::CreateBlock, (import_stream, last_status, collection_timer, backup_interval)));
                                }
                                
                                let collection_duration = calculate_collection_duration(&config, ready_count);
                                collection_timer = Some(Instant::now() + collection_duration);
                                info!(target: LOG_TARGET, "Backup check started collection window for {}ms (highest priority: {})", collection_duration.as_millis(), highest_priority);
                            } else if ready_count == 0 && collection_timer.is_some() {
                                info!(target: LOG_TARGET, "Backup check: pool became empty, canceling collection window");
                                collection_timer = None;
                            }
                        }
                        
                        return Some((SlotTrigger::NoAction, (import_stream, last_status, collection_timer, backup_interval)));
                    }
                }
                
                // This should not be reached, but included for completeness
                None
            }
        }
    ).filter_map(|trigger| async move {
        match trigger {
            SlotTrigger::CreateBlock => {
                info!(target: LOG_TARGET, "Emitting CreateBlock trigger from true event-driven stream");
                Some(SlotTrigger::CreateBlock)
            }
            SlotTrigger::NoAction => None,
        }
    }))
}

/// Create an event-driven stream that can be used with the existing slot worker system.
/// This provides a clean separation between event detection and slot management.
pub fn create_event_driven_stream<Block, Pool>(
    pool: Arc<Pool>,
    config: EventDrivenConfig,
) -> Pin<Box<dyn Stream<Item = SlotTrigger> + Send + 'static>>
where
    Block: sp_runtime::traits::Block,
    Pool: sc_transaction_pool_api::TransactionPool<Block = Block> + 'static,
{
    
    const LOG_TARGET: &str = "micc::event_driven";
    
    info!(target: LOG_TARGET, "Event-driven stream initialized with config: {:?}", config);
    
    // Create an interval stream that emits every 100ms to check pool status
    let interval = tokio::time::interval(Duration::from_millis(100));
    let pool_clone = pool.clone();
    let config_clone = config.clone();
    
    info!(target: LOG_TARGET, "Starting persistent transaction pool monitoring");
    
    // Use unfold to avoid lifetime issues with scan
    let initial_status = pool.status().ready;
    Box::pin(futures::stream::unfold(
        (initial_status, None::<Instant>, interval), // (last_status, collection_timer, interval)
        move |(mut last_status, mut collection_timer, mut interval)| {
            let pool = pool_clone.clone();
            let config = config_clone.clone();
            
            async move {
                // Wait for next tick
                interval.tick().await;
                
                // Check transaction pool status
                let status = pool.status();
                let ready_count = status.ready;
                
                // Detect status changes
                if ready_count != last_status {
                    info!(target: LOG_TARGET, "Pool status changed: {} -> {} ready transactions", last_status, ready_count);
                    last_status = ready_count;
                    
                    // Start collection window if we now have transactions and no active window
                    if ready_count > 0 && collection_timer.is_none() {
                        // Check priority for immediate production
                        let ready_transactions = pool.ready();
                        let highest_priority = ready_transactions
                            .map(|tx| *tx.priority())
                            .max()
                            .unwrap_or(0);
                        
                        if highest_priority >= config.collection.priority_threshold {
                            info!(target: LOG_TARGET, "High-priority transaction detected (priority: {}), producing block immediately", highest_priority);
                            collection_timer = None;
                            return Some((SlotTrigger::CreateBlock, (last_status, collection_timer, interval)));
                        }
                        
                        let collection_duration = calculate_collection_duration(&config, ready_count);
                        collection_timer = Some(Instant::now() + collection_duration);
                        info!(target: LOG_TARGET, "Starting collection window for {}ms with {} ready transactions (highest priority: {})", 
                            collection_duration.as_millis(), ready_count, highest_priority);
                        
                        // Check if we should produce immediately for large batches
                        if ready_count >= config.collection.max_batch_size {
                            info!(target: LOG_TARGET, "Large batch detected ({} transactions), producing block immediately", ready_count);
                            collection_timer = None;
                            return Some((SlotTrigger::CreateBlock, (last_status, collection_timer, interval)));
                        }
                    } else if ready_count == 0 && collection_timer.is_some() {
                        info!(target: LOG_TARGET, "Pool became empty, canceling collection window");
                        collection_timer = None;
                    }
                }
                
                // Check if collection window has expired
                if let Some(expire_time) = collection_timer {
                    if Instant::now() >= expire_time {
                        if ready_count > 0 {
                            info!(target: LOG_TARGET, "Collection window expired, producing block with {} transactions", ready_count);
                            collection_timer = None;
                            return Some((SlotTrigger::CreateBlock, (last_status, collection_timer, interval)));
                        } else {
                            collection_timer = None;
                        }
                    }
                }
                
                // Continue monitoring
                Some((SlotTrigger::NoAction, (last_status, collection_timer, interval)))
            }
        }
    ).filter_map(|trigger| async move {
        match trigger {
            SlotTrigger::CreateBlock => {
                info!(target: LOG_TARGET, "Emitting CreateBlock trigger");
                Some(SlotTrigger::CreateBlock)
            }
            SlotTrigger::NoAction => None,
        }
    }))
}

/// Calculate optimal collection window duration based on transaction count and configuration.
fn calculate_collection_duration(config: &EventDrivenConfig, tx_count: usize) -> Duration {
    let base_duration = Duration::from_millis(1000); // 1 second
    let min_duration = config.collection.min_collection_time;
    let max_duration = config.collection.max_collection_time;
    
    // Adjust duration based on transaction count
    // More transactions = shorter window (assuming more are coming)
    // Fewer transactions = longer window (wait for more)
    let duration = if tx_count > 50 {
        min_duration
    } else if tx_count > 10 {
        base_duration
    } else {
        max_duration
    };
    
    duration.clamp(min_duration, max_duration)
}

/// Create a smart event-driven stream with adaptive collection windows.
pub fn create_smart_event_driven_stream<Block, Pool>(
    pool: Arc<Pool>,
    config: EventDrivenConfig,
) -> Pin<Box<dyn Stream<Item = SlotTrigger> + Send + 'static>>
where
    Block: sp_runtime::traits::Block,
    Pool: sc_transaction_pool_api::TransactionPool<Block = Block> + 'static,
{
    const LOG_TARGET: &str = "micc::smart_event_driven";
    
    info!(target: LOG_TARGET, "Smart event-driven stream initialized with config: {:?}", config);
    
    // Create an interval stream that emits every 100ms to check pool status
    let interval = tokio::time::interval(Duration::from_millis(100));
    let pool_clone = pool.clone();
    let config_clone = config.clone();
    
    info!(target: LOG_TARGET, "Starting smart transaction pool monitoring with adaptive collection");
    
    // Use unfold to create a smart monitoring stream
    let initial_status = pool.status().ready;
    Box::pin(futures::stream::unfold(
        (initial_status, None::<SmartCollectionWindow>, interval, NetworkLoadTracker::new(config.transaction_rate_history_size)),
        move |(mut last_status, mut collection_window, mut interval, mut load_tracker)| {
            let pool = pool_clone.clone();
            let config = config_clone.clone();
            
            async move {
                // Wait for next tick
                interval.tick().await;
                
                // Check transaction pool status
                let status = pool.status();
                let ready_count = status.ready;
                
                // Record transaction activity and calculate load
                let current_tps = if ready_count != last_status {
                    load_tracker.record_transaction()
                } else {
                    load_tracker.get_tps()
                };
                
                // Detect status changes
                if ready_count != last_status {
                    info!(target: LOG_TARGET, "Pool status changed: {} -> {} ready transactions (TPS: {:.2})", 
                        last_status, ready_count, current_tps);
                    last_status = ready_count;
                    
                    // Start smart collection window if we now have transactions and no active window
                    if ready_count > 0 && collection_window.is_none() {
                        // Check priority for immediate production
                        let ready_transactions = pool.ready();
                        let highest_priority = ready_transactions
                            .map(|tx| *tx.priority())
                            .max();
                        
                        if let Some(priority) = highest_priority {
                            if priority >= config.collection.priority_threshold {
                                info!(target: LOG_TARGET, "High-priority transaction detected (priority: {}), producing block immediately", priority);
                                collection_window = None;
                                return Some((SlotTrigger::CreateBlock, (last_status, collection_window, interval, load_tracker)));
                            }
                        }
                        
                        let smart_window = SmartCollectionWindow::new(
                            &config.collection,
                            ready_count,
                            highest_priority,
                            current_tps,
                            false, // Not priority triggered by default
                        );
                        collection_window = Some(smart_window);
                        
                        // Check if we should produce immediately for large batches
                        if ready_count >= config.collection.max_batch_size {
                            info!(target: LOG_TARGET, "Large batch detected ({} transactions), producing block immediately", ready_count);
                            collection_window = None;
                            return Some((SlotTrigger::CreateBlock, (last_status, collection_window, interval, load_tracker)));
                        }
                    } else if ready_count == 0 && collection_window.is_some() {
                        info!(target: LOG_TARGET, "Pool became empty, canceling smart collection window");
                        collection_window = None;
                    }
                }
                
                // Check if smart collection window has expired
                if let Some(ref mut window) = collection_window {
                    if window.is_expired() {
                        if ready_count > 0 {
                            info!(target: LOG_TARGET, 
                                "Smart collection window expired after {:?}, producing block with {} transactions (priority: {:?}, load: {:.2})", 
                                window.elapsed(), ready_count, window.highest_priority, window.network_load
                            );
                            collection_window = None;
                            return Some((SlotTrigger::CreateBlock, (last_status, collection_window, interval, load_tracker)));
                        } else {
                            collection_window = None;
                        }
                    }
                }
                
                // Continue monitoring
                Some((SlotTrigger::NoAction, (last_status, collection_window, interval, load_tracker)))
            }
        }
    ).filter_map(|trigger| async move {
        match trigger {
            SlotTrigger::CreateBlock => {
                info!(target: LOG_TARGET, "Emitting CreateBlock trigger from smart collection");
                Some(SlotTrigger::CreateBlock)
            }
            SlotTrigger::NoAction => None,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_collection_window_duration_calculation() {
        use sp_runtime::{generic::Header, traits::BlakeTwo256, testing::UncheckedExtrinsic};
        type TestBlock = sp_runtime::generic::Block<Header<u32, BlakeTwo256>, UncheckedExtrinsic>;
        
        let config = EventDrivenConfig::default();
        let controller = EventDrivenController::<TestBlock>::new(config.clone());
        
        // Test different transaction counts
        assert_eq!(
            controller.calculate_collection_window_duration(1),
            config.collection.max_collection_time
        );
        
        assert_eq!(
            controller.calculate_collection_window_duration(25),
            Duration::from_millis(1000)
        );
        
        assert_eq!(
            controller.calculate_collection_window_duration(100),
            config.collection.min_collection_time
        );
    }
    
    #[test]
    fn test_immediate_production_triggers() {
        use sp_runtime::{generic::Header, traits::BlakeTwo256, testing::UncheckedExtrinsic};
        type TestBlock = sp_runtime::generic::Block<Header<u32, BlakeTwo256>, UncheckedExtrinsic>;
        
        let config = EventDrivenConfig::default();
        let mut controller = EventDrivenController::<TestBlock>::new(config.clone());
        
        // Test large batch triggers immediate production
        assert!(controller.should_produce_immediately(config.collection.max_batch_size));
        
        // Test normal batch doesn't trigger immediate production
        assert!(!controller.should_produce_immediately(10));
    }
    
    #[test]
    fn test_block_production_triggers() {
        use sp_runtime::{generic::Header, traits::BlakeTwo256, testing::UncheckedExtrinsic};
        type TestBlock = sp_runtime::generic::Block<Header<u32, BlakeTwo256>, UncheckedExtrinsic>;
        
        let config = EventDrivenConfig::default();
        let mut controller = EventDrivenController::<TestBlock>::new(config);
        
        // Test transaction added starts collection window
        let trigger = controller.handle_pool_event(PoolEvent::PoolReady(5, None));
        assert_eq!(trigger, BlockProductionTrigger::StartCollectionWindow);
        
        // Test large batch triggers immediate production
        let trigger = controller.handle_pool_event(PoolEvent::PoolReady(1000, None));
        assert_eq!(trigger, BlockProductionTrigger::ProduceImmediately);
        
        // Test empty pool cancels collection
        let trigger = controller.handle_pool_event(PoolEvent::PoolEmpty);
        assert_eq!(trigger, BlockProductionTrigger::None);
        assert!(controller.collection_window.is_none());
    }

    #[test]
    fn test_event_driven_integration() {
        use sp_runtime::{generic::Header, traits::BlakeTwo256, testing::UncheckedExtrinsic};
        type TestBlock = sp_runtime::generic::Block<Header<u32, BlakeTwo256>, UncheckedExtrinsic>;
        
        let config = EventDrivenConfig::default();
        
        // Test that event-driven config can be created and used
        assert_eq!(config.collection.min_collection_time, Duration::from_millis(500));
        assert_eq!(config.collection.max_batch_size, 1000);
        assert!(config.enable_priority_fast_track);
        
        // Test that controller can be created with config
        let controller = EventDrivenController::<TestBlock>::new(config);
        assert_eq!(controller.pending_transactions, 0);
    }

    #[test]
    fn test_smart_collection_window() {
        let config = CollectionConfig::default();
        
        // Test normal collection window
        let window = SmartCollectionWindow::new(&config, 10, None, 1.0, false);
        assert_eq!(window.initial_tx_count, 10);
        assert!(window.network_load > 0.0);
        assert!(!window.priority_triggered);
        
        // Test priority collection window
        let priority_window = SmartCollectionWindow::new(&config, 5, Some(1000), 2.0, true);
        assert_eq!(priority_window.initial_tx_count, 5);
        assert_eq!(priority_window.highest_priority, Some(1000));
        assert!(priority_window.priority_triggered);
    }

    #[test]
    fn test_network_load_tracker() {
        let mut tracker = NetworkLoadTracker::new(100);
        
        // Test initial state
        assert_eq!(tracker.get_tps(), 0.0);
        
        // Test recording transactions
        let tps = tracker.record_transaction();
        assert!(tps >= 0.0);
        
        // Test multiple transactions
        for _ in 0..5 {
            tracker.record_transaction();
        }
        
        let final_tps = tracker.get_tps();
        assert!(final_tps >= 0.0);
    }

    #[test]
    fn test_smart_event_driven_controller() {
        use sp_runtime::{generic::Header, traits::BlakeTwo256, testing::UncheckedExtrinsic};
        type TestBlock = sp_runtime::generic::Block<Header<u32, BlakeTwo256>, UncheckedExtrinsic>;
        
        let config = EventDrivenConfig::default();
        let mut controller = SmartEventDrivenController::<TestBlock>::new(config);
        
        // Test normal transaction handling
        let trigger = controller.handle_pool_event(PoolEvent::PoolReady(10, None));
        assert_eq!(trigger, BlockProductionTrigger::StartCollectionWindow);
        
        // Test high priority transaction
        let trigger = controller.handle_pool_event(PoolEvent::HighPriorityTransactionAdded(
            TransactionHash::default(), 
            TransactionPriority::MAX
        ));
        assert_eq!(trigger, BlockProductionTrigger::ProduceImmediately);
        
        // Test network load tracking
        let load = controller.get_network_load();
        assert!(load >= 0.0);
    }

    #[test]
    fn test_collection_config_adaptive_timing() {
        let config = CollectionConfig {
            min_collection_time: Duration::from_millis(100),
            max_collection_time: Duration::from_millis(1000),
            max_batch_size: 500,
            priority_threshold: 1000,
            network_load_factor: 1.0,
            enable_adaptive_timing: true,
        };
        
        // Test high load scenario
        let window = SmartCollectionWindow::new(&config, 20, None, 10.0, false);
        assert!(window.duration >= config.min_collection_time);
        assert!(window.duration <= config.max_collection_time);
        
        // Test high priority scenario
        let priority_window = SmartCollectionWindow::new(&config, 5, Some(1500), 1.0, true);
        assert!(priority_window.duration >= config.min_collection_time);
        assert!(priority_window.highest_priority == Some(1500));
    }
}