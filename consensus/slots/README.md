# ⏰ Slots - Timing Utilities for Consensus

> **Precision timing framework** for slot-based and event-driven consensus mechanisms

## 📋 Overview

The Slots module provides **timing utilities and abstractions** for slot-based consensus algorithms in Substrate. It serves as the foundation for both traditional slot-based consensus (like Aura/BABE) and innovative event-driven consensus systems like MICC.

**Key Capabilities:**
- ⏱️ **Precise Slot Timing** - Accurate slot duration calculations
- 🎯 **Smart Worker Abstractions** - Framework for consensus block production
- 📊 **Slot Information Management** - Complete slot context for decision making
- 🔄 **Event-Driven Extensions** - Advanced timing for reactive consensus
- 🛡️ **Backoff Strategies** - Intelligent responses to network conditions

## 🏗️ Core Architecture

### **Traditional vs Event-Driven Slot Management**

| Aspect | Traditional Slots | Event-Driven Slots |
|--------|------------------|-------------------|
| **Timing** | Fixed intervals | Adaptive windows |
| **Triggers** | Clock-based | Transaction events |
| **Efficiency** | Regular polling | Smart triggering |
| **Resource Usage** | Constant | On-demand |
| **Responsiveness** | Slot duration latency | Near-instant |

## 🧩 Core Components

### **1. Slot Information (`SlotInfo`)**
```rust
/// Complete information about a consensus slot
pub struct SlotInfo<Block> {
    /// The slot number
    pub slot: Slot,
    /// Duration of the slot in milliseconds
    pub duration: SlotDuration,
    /// Current chain head when slot started
    pub chain_head: Block::Header,
    /// Block number of the chain head
    pub block_number: NumberFor<Block>,
    /// Timestamp when the slot starts
    pub timestamp: u64,
    /// Total time remaining in this slot
    pub ends_at: Instant,
}

impl<Block> SlotInfo<Block> {
    /// Time remaining in this slot
    pub fn remaining_duration(&self) -> Duration {
        self.ends_at.saturating_duration_since(Instant::now())
    }
    
    /// Check if we're in the proposing portion of the slot
    pub fn is_proposing_time(&self) -> bool {
        let elapsed = self.duration.saturating_sub(
            self.remaining_duration().as_millis() as u64
        );
        elapsed < (self.duration as f32 * PROPOSING_TIME_PERCENT) as u64
    }
}
```

### **2. Slot Worker Traits**

#### **Simple Slot Worker**
```rust
/// Basic slot worker for traditional consensus
#[async_trait]
pub trait SimpleSlotWorker<B: BlockT> {
    /// Information about the current chain
    type Claim: Clone + Send + 'static;
    type Proof: Clone + Send + 'static;  
    type EpochData: Clone + Send + 'static;
    type Error: std::error::Error + Send + 'static;

    /// Logging target for this worker
    fn logging_target(&self) -> &'static str;

    /// Propose a new block for this slot
    async fn propose(
        &mut self,
        slot_info: SlotInfo<B>,
        end_proposing_at: Instant,
    ) -> Result<BlockImportParams<B>, Self::Error>;

    /// Check if this worker can author in the given slot
    async fn claim_slot(
        &mut self,
        header: &B::Header,
        slot: Slot,
        epoch_data: &Self::EpochData,
    ) -> Option<Self::Claim>;

    /// Sign the provided block
    async fn sign_block(
        &mut self,
        claim: &Self::Claim,
        proof: &Self::Proof,
        block: B,
    ) -> Result<B, Self::Error>;

    /// Calculate remaining time for proposing
    fn proposing_remaining_duration(
        &self,
        slot_info: &SlotInfo<B>,
        claim: &Self::Claim,
    ) -> Duration;
}
```

#### **Event-Driven Slot Worker**
```rust
/// Advanced slot worker with event-driven capabilities
#[async_trait]
pub trait EventDrivenSlotWorker<B: BlockT>: SimpleSlotWorker<B> {
    /// Transaction pool for monitoring events
    type TransactionPool: TransactionPool;
    
    /// Handle transaction pool events
    async fn handle_transaction_event(
        &mut self,
        slot_info: SlotInfo<B>,
        event: TransactionEvent,
    ) -> Result<ProduceBlockDecision, Self::Error>;
    
    /// Determine optimal collection window
    fn calculate_collection_window(
        &self,
        slot_info: &SlotInfo<B>,
        transaction_count: usize,
        network_load: NetworkLoad,
    ) -> Duration;
    
    /// Check if immediate block production is warranted
    fn should_produce_immediately(
        &self,
        slot_info: &SlotInfo<B>,
        transaction_priority: TransactionPriority,
        pool_state: &PoolState,
    ) -> bool;
}
```

### **3. Slot Streams and Triggers**

#### **Slot Stream**
```rust
/// Stream that yields slot information at appropriate times
pub struct Slots<Block, SC, IDP, SO> {
    chain_head_stream: Pin<Box<dyn Stream<Item = Block::Header> + Send>>,
    slot_duration: SlotDuration,
    inner_delay: Option<Delay>,
    logging_target: &'static str,
    // Event-driven components
    transaction_pool: Option<Arc<dyn TransactionPool>>,
    event_config: Option<EventDrivenConfig>,
}

impl<Block, SC, IDP, SO> Stream for Slots<Block, SC, IDP, SO>
where
    Block: BlockT,
    SC: SelectChain<Block>,
    IDP: CreateInherentDataProviders<Block, ()>,
    SO: SyncOracle,
{
    type Item = SlotInfo<Block>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        // Intelligent slot timing with event-driven enhancements
        // Can be triggered by:
        // 1. Natural slot progression (traditional)
        // 2. Transaction pool events (event-driven)
        // 3. Network conditions (adaptive)
    }
}
```

#### **Slot Triggers**
```rust
/// Different ways to trigger slot processing
#[derive(Debug, Clone)]
pub enum SlotTrigger {
    /// Traditional time-based trigger
    Timer(Instant),
    /// Transaction pool event trigger  
    TransactionEvent(TransactionHash),
    /// Network condition trigger
    NetworkCondition(NetworkEvent),
    /// Manual trigger for testing
    Manual,
}

/// Configuration for event-driven slot processing
#[derive(Debug, Clone)]
pub struct EventDrivenConfig {
    /// Enable transaction pool monitoring
    pub enable_tx_pool_monitoring: bool,
    /// Maximum collection window duration
    pub max_collection_window: Duration,
    /// Minimum transactions to trigger immediate production
    pub immediate_threshold: u32,
    /// Enable priority transaction fast-tracking
    pub enable_priority_fast_track: bool,
}
```

## 🎯 Key Functions

### **Slot Worker Management**

#### **Traditional Slot Worker**
```rust
/// Start traditional slot-based consensus worker
pub fn start_slot_worker<B, C, W, SO, SC, CAW, CIDP>(
    slot_duration: SlotDuration,
    client: Arc<C>,
    select_chain: SC,
    worker: W,
    sync_oracle: SO,
    create_inherent_data_providers: CIDP,
    can_author_with: CAW,
) -> impl Future<Output = ()>
where
    B: BlockT,
    C: UsageProvider<B> + HeaderBackend<B> + Send + Sync + 'static,
    W: SimpleSlotWorker<B> + Send + 'static,
    SO: SyncOracle + Send + Sync + 'static,
    SC: SelectChain<B> + 'static,
    CAW: CanAuthorWith<B> + Send + 'static,
    CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
{
    // Traditional time-based slot processing
    async move {
        let mut slots = Slots::new(
            slot_duration,
            client,
            select_chain,
            create_inherent_data_providers,
            sync_oracle,
        );

        while let Some(slot_info) = slots.next().await {
            process_slot_traditional(slot_info, &mut worker).await;
        }
    }
}
```

#### **Event-Driven Slot Worker V2**
```rust
/// Start advanced event-driven slot worker
pub fn start_slot_worker_v2<B, C, W, SO, SC, CAW, CIDP, TP>(
    slot_duration: SlotDuration,
    client: Arc<C>,
    select_chain: SC,
    worker: W,
    sync_oracle: SO,
    create_inherent_data_providers: CIDP,
    can_author_with: CAW,
    transaction_pool: Arc<TP>,
    event_config: EventDrivenConfig,
) -> impl Future<Output = ()>
where
    B: BlockT,
    C: UsageProvider<B> + HeaderBackend<B> + Send + Sync + 'static,
    W: EventDrivenSlotWorker<B, TransactionPool = TP> + Send + 'static,
    SO: SyncOracle + Send + Sync + 'static,
    SC: SelectChain<B> + 'static,
    CAW: CanAuthorWith<B> + Send + 'static,
    CIDP: CreateInherentDataProviders<B, ()> + Send + 'static,
    TP: TransactionPool + 'static,
{
    async move {
        // Create event-driven slots stream
        let mut slots = Slots::new_event_driven(
            slot_duration,
            client,
            select_chain,
            create_inherent_data_providers,
            sync_oracle,
            transaction_pool.clone(),
            event_config,
        );

        // Transaction pool event stream
        let mut tx_stream = transaction_pool.import_notification_stream();

        // Process both slot timing and transaction events
        loop {
            tokio::select! {
                Some(slot_info) = slots.next() => {
                    process_slot_event_driven(slot_info, &mut worker).await;
                }
                Some(tx_hash) = tx_stream.next() => {
                    handle_transaction_event(tx_hash, &mut worker, &transaction_pool).await;
                }
            }
        }
    }
}
```

### **Timing Calculations**

#### **Slot Duration Utilities**
```rust
/// Calculate time until next slot
pub fn time_until_next_slot(
    slot_duration: SlotDuration,
    timestamp: u64,
) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    let current_slot = now / slot_duration;
    let next_slot_start = (current_slot + 1) * slot_duration;
    
    Duration::from_millis(next_slot_start.saturating_sub(now))
}

/// Calculate proposing duration for a slot
pub fn proposing_remaining_duration(
    slot_info: &SlotInfo<impl BlockT>,
    proposing_percent: f32,
) -> Duration {
    let slot_remaining = slot_info.remaining_duration();
    let proposing_duration = slot_remaining.mul_f32(proposing_percent);
    
    std::cmp::max(
        proposing_duration,
        Duration::from_millis(MIN_PROPOSING_TIME_MS)
    )
}
```

#### **Event-Driven Timing**
```rust
/// Calculate optimal collection window based on network conditions
pub fn calculate_collection_window(
    base_duration: Duration,
    transaction_count: usize,
    network_load: NetworkLoad,
    priority_level: TransactionPriority,
) -> Duration {
    let load_multiplier = match network_load {
        NetworkLoad::Low => 1.5,
        NetworkLoad::Medium => 1.0,
        NetworkLoad::High => 0.5,
    };
    
    let priority_multiplier = match priority_level {
        TransactionPriority::High => 0.2,
        TransactionPriority::Medium => 1.0,
        TransactionPriority::Low => 2.0,
    };
    
    let transaction_factor = if transaction_count > 20 {
        0.5
    } else if transaction_count > 10 {
        0.8
    } else {
        1.0
    };
    
    let final_duration = base_duration
        .mul_f32(load_multiplier)
        .mul_f32(priority_multiplier)
        .mul_f32(transaction_factor);
    
    // Ensure minimum and maximum bounds
    std::cmp::min(
        std::cmp::max(final_duration, Duration::from_millis(50)),
        Duration::from_millis(2000)
    )
}
```

## 🛡️ Backoff Strategies

### **Authoring Backoff**
```rust
/// Strategy for backing off block authoring under adverse conditions
pub trait BackoffAuthoringBlocksStrategy<N> {
    /// Should we backoff from authoring blocks?
    fn should_backoff(
        &self,
        chain_head_number: N,
        chain_head_slot: Slot,
        finalized_number: N,
        slow_now: Slot,
        logging_target: &str,
    ) -> bool;
}

/// Default backoff strategy implementation
pub struct BackoffAuthoringOnFinalizedHeadLagging<N> {
    /// Number of slots to wait for finalization
    max_interval: N,
    /// Time to wait before checking again
    wait_duration: Duration,
}

impl<N> BackoffAuthoringOnFinalizedHeadLagging<N>
where
    N: BaseArithmetic + Copy,
{
    pub fn new(max_interval: N) -> Self {
        Self {
            max_interval,
            wait_duration: Duration::from_secs(2),
        }
    }
}

impl<N> BackoffAuthoringBlocksStrategy<N> for BackoffAuthoringOnFinalizedHeadLagging<N>
where
    N: BaseArithmetic + Copy,
{
    fn should_backoff(
        &self,
        chain_head_number: N,
        _chain_head_slot: Slot,
        finalized_number: N,
        _slow_now: Slot,
        logging_target: &str,
    ) -> bool {
        let diff = chain_head_number.saturating_sub(finalized_number);
        
        if diff >= self.max_interval {
            log::info!(
                target: logging_target,
                "Backing off authoring blocks due to finality lag: {} unfinalized blocks",
                diff
            );
            true
        } else {
            false
        }
    }
}
```

### **Network-Aware Backoff**
```rust
/// Advanced backoff strategy considering network conditions
pub struct NetworkAwareBackoffStrategy {
    /// Maximum allowed unfinalized blocks
    max_unfinalized: u32,
    /// Maximum transaction pool size before backoff
    max_pool_size: usize,
    /// Network sync threshold
    sync_threshold: f32,
}

impl NetworkAwareBackoffStrategy {
    pub fn should_backoff_advanced<B: BlockT>(
        &self,
        chain_info: &ChainInfo<B>,
        pool_state: &PoolState,
        sync_state: &SyncState,
    ) -> BackoffDecision {
        // Check finality lag
        let finality_lag = chain_info.best_number.saturating_sub(chain_info.finalized_number);
        if finality_lag > self.max_unfinalized.into() {
            return BackoffDecision::FinalityLag { lag: finality_lag };
        }
        
        // Check transaction pool overload
        if pool_state.ready_count > self.max_pool_size {
            return BackoffDecision::PoolOverload { 
                count: pool_state.ready_count 
            };
        }
        
        // Check network sync state
        if sync_state.sync_percent < self.sync_threshold {
            return BackoffDecision::NetworkSync { 
                percent: sync_state.sync_percent 
            };
        }
        
        BackoffDecision::Continue
    }
}

#[derive(Debug)]
pub enum BackoffDecision {
    Continue,
    FinalityLag { lag: u32 },
    PoolOverload { count: usize },
    NetworkSync { percent: f32 },
}
```

## 📊 Event-Driven Enhancements

### **Transaction Pool Integration**
```rust
/// Monitor transaction pool for production triggers
pub async fn monitor_transaction_events<TP, W>(
    transaction_pool: Arc<TP>,
    worker: Arc<Mutex<W>>,
    config: EventDrivenConfig,
) where
    TP: TransactionPool,
    W: EventDrivenSlotWorker<TP::Block>,
{
    let mut import_stream = transaction_pool.import_notification_stream();
    let mut ready_stream = transaction_pool.ready();
    
    while let Some(tx_hash) = import_stream.next().await {
        let pool_state = PoolState {
            ready_count: ready_stream.count(),
            pending_count: transaction_pool.status().ready,
        };
        
        let should_trigger = analyze_trigger_condition(
            &tx_hash,
            &pool_state,
            &config,
        ).await;
        
        if should_trigger {
            trigger_block_production(&worker, tx_hash).await;
        }
    }
}

/// Analyze whether transaction should trigger block production
async fn analyze_trigger_condition(
    tx_hash: &TransactionHash,
    pool_state: &PoolState,
    config: &EventDrivenConfig,
) -> bool {
    // Immediate production for high count
    if pool_state.ready_count >= config.immediate_threshold as usize {
        return true;
    }
    
    // Check transaction priority
    if config.enable_priority_fast_track {
        if let Some(priority) = get_transaction_priority(tx_hash).await {
            if priority == TransactionPriority::High {
                return true;
            }
        }
    }
    
    false
}
```

### **Adaptive Timing**
```rust
/// Adaptive slot timing based on network conditions
pub struct AdaptiveSlotTiming {
    base_duration: Duration,
    network_monitor: NetworkMonitor,
    transaction_analyzer: TransactionAnalyzer,
}

impl AdaptiveSlotTiming {
    pub fn calculate_next_slot_timing(
        &self,
        current_conditions: &NetworkConditions,
        transaction_activity: &TransactionActivity,
    ) -> SlotTimingDecision {
        let load_factor = self.network_monitor.get_load_factor();
        let tx_urgency = self.transaction_analyzer.get_urgency_score();
        
        match (load_factor, tx_urgency) {
            // High urgency, low load: immediate production
            (NetworkLoad::Low, urgency) if urgency > 0.8 => {
                SlotTimingDecision::Immediate
            }
            
            // Normal conditions: adaptive window
            (load, urgency) => {
                let window = self.calculate_adaptive_window(load, urgency);
                SlotTimingDecision::DelayedProduction { window }
            }
        }
    }
    
    fn calculate_adaptive_window(
        &self,
        load: NetworkLoad,
        urgency: f32,
    ) -> Duration {
        let base_ms = self.base_duration.as_millis() as f32;
        let load_multiplier = match load {
            NetworkLoad::Low => 1.5,
            NetworkLoad::Medium => 1.0,
            NetworkLoad::High => 0.6,
        };
        
        let urgency_multiplier = 2.0 - urgency; // Higher urgency = lower multiplier
        let final_duration = base_ms * load_multiplier * urgency_multiplier;
        
        Duration::from_millis(final_duration as u64)
    }
}
```

## 🔗 Integration Examples

### **With MICC Consensus**
```rust
// MICC client using slots for event-driven consensus
use sc_consensus_slots::{SlotInfo, start_slot_worker_v2};

let slot_worker_future = start_slot_worker_v2(
    slot_duration,
    client.clone(),
    select_chain,
    micc_worker,
    sync_oracle,
    create_inherent_data_providers,
    can_author_with,
    transaction_pool,
    EventDrivenConfig {
        enable_tx_pool_monitoring: true,
        max_collection_window: Duration::from_millis(500),
        immediate_threshold: 10,
        enable_priority_fast_track: true,
    },
);
```

### **With Traditional Consensus**
```rust
// Traditional Aura-style consensus using basic slots
use sc_consensus_slots::{SlotInfo, start_slot_worker};

let slot_worker_future = start_slot_worker(
    slot_duration,
    client.clone(),
    select_chain,
    aura_worker,
    sync_oracle,
    create_inherent_data_providers,
    can_author_with,
);
```

## 📊 Performance Optimizations

### **Smart Caching**
```rust
/// Cache for slot calculations and timing data
pub struct SlotCache {
    slot_duration: SlotDuration,
    cached_calculations: LruCache<Slot, SlotTimingData>,
    genesis_time: u64,
}

impl SlotCache {
    pub fn get_slot_timing(&mut self, slot: Slot) -> &SlotTimingData {
        self.cached_calculations.get_or_insert(slot, || {
            self.calculate_slot_timing(slot)
        })
    }
}
```

### **Efficient Event Processing**
```rust
/// Optimized event processing pipeline
pub struct EventProcessor {
    event_queue: VecDeque<SlotEvent>,
    batch_processor: BatchProcessor,
    priority_queue: BinaryHeap<PriorityEvent>,
}

impl EventProcessor {
    pub async fn process_events_batch(&mut self) -> Vec<ProcessedEvent> {
        // Batch process events for efficiency
        let mut results = Vec::new();
        
        while let Some(event) = self.event_queue.pop_front() {
            if let Some(processed) = self.process_single_event(event).await {
                results.push(processed);
            }
            
            // Yield control periodically
            if results.len() % 10 == 0 {
                tokio::task::yield_now().await;
            }
        }
        
        results
    }
}
```

## 🧪 Testing Utilities

```rust
/// Testing utilities for slot-based consensus
pub mod testing {
    use super::*;
    
    /// Mock slot worker for testing
    pub struct MockSlotWorker {
        pub claim_slot_result: Option<MockClaim>,
        pub propose_result: Result<MockBlock, MockError>,
    }
    
    /// Create deterministic slot stream for testing
    pub fn create_test_slot_stream(
        slot_duration: SlotDuration,
        slot_count: usize,
    ) -> impl Stream<Item = SlotInfo<MockBlock>> {
        stream::iter((0..slot_count).map(move |i| {
            SlotInfo {
                slot: Slot::from(i as u64),
                duration: slot_duration,
                timestamp: i as u64 * slot_duration,
                // ... other test data
            }
        }))
    }
    
    /// Test event-driven slot behavior
    pub async fn test_event_driven_slots() {
        let mut slots = create_test_slot_stream(1000, 10);
        let mut events_processed = 0;
        
        while let Some(slot_info) = slots.next().await {
            // Simulate event-driven processing
            events_processed += 1;
            assert!(slot_info.duration == 1000);
        }
        
        assert_eq!(events_processed, 10);
    }
}
```

## 📚 Related Documentation

- **[MICC Client](../micc-client/README.md)** - Event-driven consensus implementation
- **[MICC Pallet](../micc/README.md)** - Runtime consensus integration
- **[MICC Primitives](../micc-primitives/README.md)** - Core consensus types
- **[Substrate Consensus](https://docs.substrate.io/fundamentals/consensus/)** - Consensus fundamentals

## 📜 License

GPL-3.0-or-later WITH Classpath-exception-2.0

## 🏷️ Release

Based on Polkadot SDK stable2409 with event-driven slot enhancements.

---

> ⚡ **Innovation**: The Slots module bridges traditional time-based consensus with revolutionary event-driven approaches, enabling both reliable timing and instant reactivity in blockchain consensus systems.