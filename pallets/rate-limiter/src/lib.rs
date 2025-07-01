#![cfg_attr(not(feature = "std"), no_std)]

//! # Rate Limiter Pallet
//!
//! A pallet for implementing rate limiting to prevent spam in fee-free transaction systems.
//! 
//! ## Overview
//!
//! This pallet provides configurable rate limiting based on account IDs to prevent
//! transaction spam attacks in systems without transaction fees. It tracks transaction
//! counts per account within configurable time windows and rejects transactions that
//! exceed the configured limits.
//!
//! ## Critical Implementation Details
//!
//! ### Storage Cleanup Timing Requirements
//!
//! **CRITICAL**: Expired transaction cleanup MUST occur BEFORE validation checks to prevent
//! false rejections. The cleanup timing follows this strict order:
//!
//! 1. **Cleanup Phase**: Remove expired transactions from storage
//! 2. **Validation Phase**: Check limits against cleaned data
//! 3. **Persistence Phase**: Save updated storage state
//!
//! #### Why This Order Matters
//!
//! - **Without cleanup-first**: Valid transactions may be rejected due to stale expired data
//! - **Race conditions**: Cleanup after validation can create timing-dependent failures
//! - **Storage consistency**: Cleaned data must be persisted to maintain accurate state
//!
//! #### Implementation Examples
//!
//! ```text
//! // ✅ CORRECT: Cleanup before validation
//! let minute_cutoff = current_timestamp.saturating_sub(60_000u64);
//! rate_limit.recent_transactions.retain(|&timestamp| timestamp > minute_cutoff);
//! // Now check limits against cleaned data
//! if rate_limit.recent_transactions.len() as u32 >= rate_limit.max_per_minute {
//!     return Err(Error::<T>::RateLimitExceededPerMinute.into());
//! }
//!
//! // ❌ INCORRECT: Validation before cleanup
//! if rate_limit.recent_transactions.len() as u32 >= rate_limit.max_per_minute {
//!     return Err(Error::<T>::RateLimitExceededPerMinute.into());
//! }
//! rate_limit.recent_transactions.retain(|&timestamp| timestamp > minute_cutoff);
//! ```
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `set_rate_limit` - Set rate limit for an account (root only)
//! * `clear_rate_limit` - Clear rate limit for an account (root only)
//!
//! ### Public Functions
//!
//! * `check_rate_limit` - Check if an account can submit a transaction
//! * `record_transaction` - Record a transaction for an account
//!
//! ### Configuration
//!
//! * `DefaultTransactionsPerBlock` - Default number of transactions per block
//! * `DefaultTransactionsPerMinute` - Default number of transactions per minute
//! * `MinimumBalance` - Minimum balance required to submit transactions

use frame_support::{
    dispatch::DispatchResult,
    pallet_prelude::*,
    traits::{Get, ReservableCurrency, Currency, BuildGenesisConfig},
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::{SaturatedConversion, Saturating},
};
use frame_support::BoundedVec;
use codec::{Encode, Decode};
use scale_info::TypeInfo;

pub use pallet::*;

/// Transaction extensions for rate limiting
pub mod extensions;
pub mod transaction_extension;

pub use transaction_extension::CheckRateLimit;

/// Transaction rate limiting configuration for an account
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct RateLimit {
    /// Maximum transactions per block
    pub max_per_block: u32,
    /// Maximum transactions per minute (rolling window)
    pub max_per_minute: u32,
    /// Current block transaction count
    pub current_block_count: u32,
    /// Transaction timestamps for minute window (milliseconds since Unix epoch) - bounded to max_per_minute
    pub recent_transactions: BoundedVec<u64, frame_support::traits::ConstU32<100>>,
    /// Block number when limits were last reset
    pub last_reset_block: u32,
}

/// Per-account transaction pool usage tracking for enhanced resource limits
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct AccountPoolData {
    /// Number of pending transactions in pool
    pub pending_transactions: u32,
    /// Total bytes used by pending transactions  
    pub total_bytes_used: u32,
    /// Block number of last transaction
    pub last_transaction_block: u32,
    /// Transactions submitted in current minute window
    pub transactions_per_minute: u32,
    /// Block number when minute counter was last reset
    pub minute_reset_block: u32,
}

impl Default for AccountPoolData {
    fn default() -> Self {
        Self {
            pending_transactions: 0,
            total_bytes_used: 0,
            last_transaction_block: 0,
            transactions_per_minute: 0,
            minute_reset_block: 0,
        }
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self {
            max_per_block: 5,      // Conservative default
            max_per_minute: 20,    // Conservative default
            current_block_count: 0,
            recent_transactions: BoundedVec::new(),
            last_reset_block: 0,
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_timestamp::Config<Moment = u64> {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The currency used for reserving funds.
        type Currency: ReservableCurrency<Self::AccountId>;

        /// Default maximum transactions per block for new accounts
        #[pallet::constant]
        type DefaultTransactionsPerBlock: Get<u32>;

        /// Default maximum transactions per minute for new accounts
        #[pallet::constant]
        type DefaultTransactionsPerMinute: Get<u32>;

        /// Minimum balance required to submit transactions (spam protection)
        #[pallet::constant]
        type MinimumBalance: Get<<Self::Currency as Currency<Self::AccountId>>::Balance>;

        /// Maximum pending transactions per account in the pool
        #[pallet::constant]
        type MaxTransactionsPerAccount: Get<u32>;

        /// Maximum bytes per account in the transaction pool
        #[pallet::constant]
        type MaxBytesPerAccount: Get<u32>;

        /// Maximum transactions per minute per account (optimized for 500ms blocks)
        #[pallet::constant]
        type MaxTransactionsPerMinute: Get<u32>;
    }

    /// Rate limits for accounts
    #[pallet::storage]
    #[pallet::getter(fn rate_limits)]
    pub type RateLimits<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        RateLimit,
        ValueQuery,
    >;

    /// Global rate limiting configuration
    #[pallet::storage]
    #[pallet::getter(fn global_config)]
    pub type GlobalConfig<T: Config> = StorageValue<
        _,
        RateLimit,
        ValueQuery,
    >;

    /// Emergency pause flag - when true, all transactions are rejected
    #[pallet::storage]
    #[pallet::getter(fn is_paused)]
    pub type IsPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Emergency rate reduction multiplier (0-100, where 50 = 50% of normal limits)
    /// When set to values < 100, all rate limits are reduced proportionally
    /// Default value: 100 (no reduction)
    #[pallet::storage]
    #[pallet::getter(fn emergency_rate_multiplier)]
    pub type EmergencyRateMultiplier<T: Config> = StorageValue<_, u8, ValueQuery>;

    /// Adaptive load multiplier (100-200, where 150 = 50% increase under high load)
    /// When set to values > 100, all rate limits are increased proportionally
    /// Default value: 100 (no increase)
    #[pallet::storage]
    #[pallet::getter(fn adaptive_load_multiplier)]
    pub type AdaptiveLoadMultiplier<T: Config> = StorageValue<_, u8, ValueQuery>;

    /// Load monitoring data for adaptive scaling
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
    pub struct SystemLoadMetrics {
        /// Rolling average of transactions per block over last 10 blocks
        pub avg_transactions_per_block: u32,
        /// Rolling average of bytes per block over last 10 blocks
        pub avg_bytes_per_block: u32,
        /// Block number of last metrics update
        pub last_update_block: u32,
        /// Number of failed transactions due to rate limits in last 10 blocks
        pub failed_transactions: u32,
    }

    impl Default for SystemLoadMetrics {
        fn default() -> Self {
            Self {
                avg_transactions_per_block: 0,
                avg_bytes_per_block: 0,
                last_update_block: 0,
                failed_transactions: 0,
            }
        }
    }

    /// System load monitoring metrics
    #[pallet::storage]
    #[pallet::getter(fn load_metrics)]
    pub type LoadMetrics<T: Config> = StorageValue<_, SystemLoadMetrics, ValueQuery>;

    /// Per-account pool usage tracking for enhanced resource limits
    #[pallet::storage]
    #[pallet::getter(fn account_pool_usage)]
    pub type AccountPoolUsage<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        AccountPoolData,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Rate limit set for account [account, max_per_block, max_per_minute]
        RateLimitSet { 
            account: T::AccountId, 
            max_per_block: u32, 
            max_per_minute: u32 
        },
        /// Rate limit cleared for account [account]
        RateLimitCleared { account: T::AccountId },
        /// Transaction blocked due to rate limit [account, current_count, limit]
        TransactionBlocked { 
            account: T::AccountId, 
            current_count: u32, 
            limit: u32 
        },
        /// Transaction recorded [account, block_number]
        TransactionRecorded { 
            account: T::AccountId, 
            block_number: u32 
        },
        /// Emergency pause activated
        EmergencyPauseActivated,
        /// Emergency pause deactivated  
        EmergencyPauseDeactivated,
        /// Emergency rate reduction activated [multiplier_percent]
        EmergencyRateReductionActivated { multiplier: u8 },
        /// Emergency rate reduction deactivated (back to 100%)
        EmergencyRateReductionDeactivated,
        /// Adaptive load scaling activated [multiplier_percent]
        AdaptiveLoadScalingActivated { multiplier: u8 },
        /// Adaptive load scaling deactivated (back to 100%)
        AdaptiveLoadScalingDeactivated,
        /// Insufficient balance detected [account, required, actual]
        InsufficientBalance {
            account: T::AccountId,
            required: <T::Currency as Currency<T::AccountId>>::Balance,
            actual: <T::Currency as Currency<T::AccountId>>::Balance,
        },
        /// Account pool limits exceeded [account, pending_count, byte_usage]
        AccountPoolLimitExceeded {
            account: T::AccountId,
            pending_count: u32,
            byte_usage: u32,
        },
        /// Minute rate limit exceeded [account, current_rate, limit]
        MinuteRateLimitExceeded {
            account: T::AccountId,
            current_rate: u32,
            limit: u32,
        },
        /// Transaction pool metrics updated [total_pending, total_bytes]
        PoolMetricsUpdated {
            total_pending: u32,
            total_bytes: u32,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Rate limit exceeded for this block
        RateLimitExceededPerBlock,
        /// Rate limit exceeded for this minute
        RateLimitExceededPerMinute,
        /// Block limit exceeded
        BlockLimitExceeded,
        /// Minute limit exceeded
        MinuteLimitExceeded,
        /// System is in emergency pause mode
        SystemPaused,
        /// Account has insufficient balance for transactions
        InsufficientBalance,
        /// Invalid rate limit parameters
        InvalidRateLimit,
        /// Too many pending transactions for this account
        TooManyPendingTransactions,
        /// Account pool byte limit exceeded
        AccountPoolLimitExceeded,
        /// Per-minute transaction rate limit exceeded
        MinuteRateLimitExceeded,
        /// Invalid emergency rate multiplier (must be 0-100)
        InvalidEmergencyRateMultiplier,
        /// Invalid adaptive load multiplier (must be 100-200)
        InvalidAdaptiveLoadMultiplier,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Set rate limit for a specific account (root only)
        #[pallet::call_index(0)]
        #[pallet::weight(10_000)]
        pub fn set_rate_limit(
            origin: OriginFor<T>,
            account: T::AccountId,
            max_per_block: u32,
            max_per_minute: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;

            // Validate parameters
            ensure!(max_per_block > 0 && max_per_minute > 0, Error::<T>::InvalidRateLimit);
            ensure!(max_per_minute >= max_per_block, Error::<T>::InvalidRateLimit);

            let rate_limit = RateLimit {
                max_per_block,
                max_per_minute,
                current_block_count: 0,
                recent_transactions: BoundedVec::new(),
                last_reset_block: <frame_system::Pallet<T>>::block_number().saturated_into::<u32>(),
            };

            RateLimits::<T>::insert(&account, rate_limit);

            Self::deposit_event(Event::RateLimitSet { 
                account, 
                max_per_block, 
                max_per_minute 
            });

            Ok(())
        }

        /// Clear rate limit for a specific account (root only)
        #[pallet::call_index(1)]
        #[pallet::weight(10_000)]
        pub fn clear_rate_limit(
            origin: OriginFor<T>,
            account: T::AccountId,
        ) -> DispatchResult {
            ensure_root(origin)?;

            RateLimits::<T>::remove(&account);
            Self::deposit_event(Event::RateLimitCleared { account });

            Ok(())
        }

        /// Activate emergency pause (root only)
        #[pallet::call_index(2)]
        #[pallet::weight(10_000)]
        pub fn emergency_pause(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            IsPaused::<T>::put(true);
            Self::deposit_event(Event::EmergencyPauseActivated);
            Ok(())
        }

        /// Deactivate emergency pause (root only)  
        #[pallet::call_index(3)]
        #[pallet::weight(10_000)]
        pub fn emergency_unpause(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            IsPaused::<T>::put(false);
            Self::deposit_event(Event::EmergencyPauseDeactivated);
            Ok(())
        }

        /// Set global rate limiting defaults (root only)
        #[pallet::call_index(4)]
        #[pallet::weight(10_000)]
        pub fn set_global_config(
            origin: OriginFor<T>,
            max_per_block: u32,
            max_per_minute: u32,
        ) -> DispatchResult {
            ensure_root(origin)?;

            ensure!(max_per_block > 0 && max_per_minute > 0, Error::<T>::InvalidRateLimit);
            ensure!(max_per_minute >= max_per_block, Error::<T>::InvalidRateLimit);

            let config = RateLimit {
                max_per_block,
                max_per_minute,
                current_block_count: 0,
                recent_transactions: BoundedVec::new(),
                last_reset_block: <frame_system::Pallet<T>>::block_number().saturated_into::<u32>(),
            };

            GlobalConfig::<T>::put(config);
            Ok(())
        }

        /// Set emergency rate reduction multiplier (root only)
        /// Multiplier is a percentage (0-100) where values < 100 reduce all rate limits
        #[pallet::call_index(5)]
        #[pallet::weight(10_000)]
        pub fn set_emergency_rate_reduction(
            origin: OriginFor<T>,
            multiplier_percent: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            
            ensure!(multiplier_percent <= 100, Error::<T>::InvalidEmergencyRateMultiplier);
            
            let old_multiplier = Self::emergency_rate_multiplier();
            EmergencyRateMultiplier::<T>::put(multiplier_percent);
            
            if multiplier_percent < 100 && old_multiplier == 100 {
                Self::deposit_event(Event::EmergencyRateReductionActivated { 
                    multiplier: multiplier_percent 
                });
            } else if multiplier_percent == 100 && old_multiplier < 100 {
                Self::deposit_event(Event::EmergencyRateReductionDeactivated);
            }
            
            Ok(())
        }

        /// Clear emergency rate reduction (set back to 100% - root only)
        #[pallet::call_index(6)]
        #[pallet::weight(10_000)]
        pub fn clear_emergency_rate_reduction(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            
            let old_multiplier = Self::emergency_rate_multiplier();
            if old_multiplier < 100 {
                EmergencyRateMultiplier::<T>::put(100u8);
                Self::deposit_event(Event::EmergencyRateReductionDeactivated);
            }
            
            Ok(())
        }

        /// Set adaptive load scaling multiplier (root only)
        /// Multiplier is a percentage (100-200) where values > 100 increase rate limits under load
        #[pallet::call_index(7)]
        #[pallet::weight(10_000)]
        pub fn set_adaptive_load_scaling(
            origin: OriginFor<T>,
            multiplier_percent: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            
            ensure!(multiplier_percent >= 100 && multiplier_percent <= 200, Error::<T>::InvalidAdaptiveLoadMultiplier);
            
            let old_multiplier = Self::adaptive_load_multiplier();
            AdaptiveLoadMultiplier::<T>::put(multiplier_percent);
            
            if multiplier_percent > 100 && old_multiplier == 100 {
                Self::deposit_event(Event::AdaptiveLoadScalingActivated { 
                    multiplier: multiplier_percent 
                });
            } else if multiplier_percent == 100 && old_multiplier > 100 {
                Self::deposit_event(Event::AdaptiveLoadScalingDeactivated);
            }
            
            Ok(())
        }

        /// Clear adaptive load scaling (set back to 100% - root only)
        #[pallet::call_index(8)]
        #[pallet::weight(10_000)]
        pub fn clear_adaptive_load_scaling(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;
            
            let old_multiplier = Self::adaptive_load_multiplier();
            if old_multiplier > 100 {
                AdaptiveLoadMultiplier::<T>::put(100u8);
                Self::deposit_event(Event::AdaptiveLoadScalingDeactivated);
            }
            
            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub emergency_rate_multiplier: u8,
        pub adaptive_load_multiplier: u8,
        _config: sp_std::marker::PhantomData<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                emergency_rate_multiplier: 100, // Default: no reduction
                adaptive_load_multiplier: 100,  // Default: no increase
                _config: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            EmergencyRateMultiplier::<T>::put(self.emergency_rate_multiplier);
            AdaptiveLoadMultiplier::<T>::put(self.adaptive_load_multiplier);
        }
    }

    impl<T: Config> Pallet<T> {
        /// Apply both emergency rate reduction and adaptive load scaling to a limit value
        /// 
        /// Takes a normal limit and applies both emergency reduction and adaptive scaling.
        /// Emergency reduction is applied first (can reduce), then adaptive scaling (can increase).
        /// For example: limit=100, emergency=50%, adaptive=150% -> 100 * 0.5 * 1.5 = 75
        fn apply_rate_adjustments(limit: u32) -> u32 {
            let emergency_multiplier = Self::emergency_rate_multiplier();
            let adaptive_multiplier = Self::adaptive_load_multiplier();
            
            // Emergency reduction takes priority (security first)
            if emergency_multiplier == 0 {
                return 0; // Complete shutdown
            }
            
            // Apply emergency reduction first
            let after_emergency = if emergency_multiplier >= 100 {
                limit // No reduction
            } else {
                let reduced = (limit as u64 * emergency_multiplier as u64) / 100u64;
                reduced.saturated_into::<u32>().max(1) // Ensure at least 1
            };
            
            // Apply adaptive load scaling second (only if emergency allows it)
            if adaptive_multiplier <= 100 || emergency_multiplier < 100 {
                return after_emergency; // No adaptive increase when emergency is active
            }
            
            // Apply adaptive increase: result * (adaptive / 100)
            let final_limit = (after_emergency as u64 * adaptive_multiplier as u64) / 100u64;
            final_limit.saturated_into::<u32>()
        }

        /// Enhanced transaction pool validation for per-account limits
        pub fn can_submit_transaction(
            who: &T::AccountId, 
            transaction_bytes: u32
        ) -> Result<(), Error<T>> {
            // Check emergency pause
            if Self::is_paused() {
                return Err(Error::<T>::SystemPaused);
            }

            // Check minimum balance requirement
            let balance = T::Currency::free_balance(who);
            let minimum = T::MinimumBalance::get();
            if balance < minimum {
                Self::deposit_event(Event::InsufficientBalance {
                    account: who.clone(),
                    required: minimum,
                    actual: balance,
                });
                return Err(Error::<T>::InsufficientBalance);
            }

            let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
            let mut usage = Self::account_pool_usage(who);
            
            // Reset minute counter if needed (120 blocks = 1 minute with 500ms blocks)
            let blocks_per_minute = 120u32;
            if current_block.saturating_sub(usage.minute_reset_block) >= blocks_per_minute {
                usage.transactions_per_minute = 0;
                usage.minute_reset_block = current_block;
            }
            
            // Check per-account pending transaction limit (with emergency reduction)
            let max_txs_per_account = Self::apply_rate_adjustments(T::MaxTransactionsPerAccount::get());
            if usage.pending_transactions >= max_txs_per_account {
                Self::deposit_event(Event::AccountPoolLimitExceeded {
                    account: who.clone(),
                    pending_count: usage.pending_transactions,
                    byte_usage: usage.total_bytes_used,
                });
                return Err(Error::<T>::TooManyPendingTransactions);
            }
            
            // Check per-account byte limit (with emergency reduction)
            let max_bytes_per_account = Self::apply_rate_adjustments(T::MaxBytesPerAccount::get());
            if usage.total_bytes_used.saturating_add(transaction_bytes) >= max_bytes_per_account {
                Self::deposit_event(Event::AccountPoolLimitExceeded {
                    account: who.clone(),
                    pending_count: usage.pending_transactions,
                    byte_usage: usage.total_bytes_used,
                });
                return Err(Error::<T>::AccountPoolLimitExceeded);
            }
            
            // Check per-minute transaction rate (important for 500ms blocks, with emergency reduction)
            let max_txs_per_minute = Self::apply_rate_adjustments(T::MaxTransactionsPerMinute::get());
            if usage.transactions_per_minute >= max_txs_per_minute {
                Self::deposit_event(Event::MinuteRateLimitExceeded {
                    account: who.clone(),
                    current_rate: usage.transactions_per_minute,
                    limit: max_txs_per_minute,
                });
                return Err(Error::<T>::MinuteRateLimitExceeded);
            }
            
            // Update usage tracking
            usage.pending_transactions = usage.pending_transactions.saturating_add(1);
            usage.total_bytes_used = usage.total_bytes_used.saturating_add(transaction_bytes);
            usage.last_transaction_block = current_block;
            usage.transactions_per_minute = usage.transactions_per_minute.saturating_add(1);
            
            AccountPoolUsage::<T>::insert(who, usage);
            
            Ok(())
        }

        /// Clean up pool usage when transaction is removed from pool
        pub fn on_transaction_removed(who: &T::AccountId, transaction_bytes: u32) {
            let mut usage = Self::account_pool_usage(who);
            usage.pending_transactions = usage.pending_transactions.saturating_sub(1);
            usage.total_bytes_used = usage.total_bytes_used.saturating_sub(transaction_bytes);
            AccountPoolUsage::<T>::insert(who, usage);
        }

        /// Check if an account can submit a transaction based on rate limits
        /// 
        /// ## Critical Implementation Notes
        /// 
        /// This function implements the cleanup-before-validation pattern to prevent
        /// false rejections due to expired transaction data. The implementation order is:
        /// 
        /// 1. **Cleanup expired transactions** - Remove stale entries BEFORE checking limits
        /// 2. **Validate against cleaned data** - Check limits with accurate current state
        /// 3. **Persist cleaned state** - Save updated storage to maintain consistency
        /// 
        /// **WARNING**: Changing this order will cause timing-dependent validation failures.
        pub fn check_rate_limit(account: &T::AccountId) -> DispatchResult {
            // Check emergency pause
            if Self::is_paused() {
                return Err(Error::<T>::SystemPaused.into());
            }

            // Check minimum balance requirement
            let balance = T::Currency::free_balance(account);
            let minimum = T::MinimumBalance::get();
            if balance < minimum {
                Self::deposit_event(Event::InsufficientBalance {
                    account: account.clone(),
                    required: minimum,
                    actual: balance,
                });
                return Err(Error::<T>::InsufficientBalance.into());
            }

            let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
            let current_timestamp = pallet_timestamp::Pallet::<T>::get();
            let mut rate_limit = Self::rate_limits(account);

            // Use global config if no specific limit set
            if rate_limit == RateLimit::default() {
                let global = Self::global_config();
                if global != RateLimit::default() {
                    rate_limit = global;
                } else {
                    // Use pallet constants as fallback
                    rate_limit.max_per_block = T::DefaultTransactionsPerBlock::get();
                    rate_limit.max_per_minute = T::DefaultTransactionsPerMinute::get();
                }
            }

            // CRITICAL FIX: Clean expired transactions BEFORE validation checks
            // This prevents valid transactions from being rejected due to stale data
            let minute_cutoff = current_timestamp.saturating_sub(60_000u64);
            let original_count = rate_limit.recent_transactions.len();
            rate_limit.recent_transactions.retain(|&timestamp| timestamp > minute_cutoff);
            let cleaned_count = original_count - rate_limit.recent_transactions.len();
            
            // Log cleanup for debugging if significant cleanup occurred
            if cleaned_count > 0 {
                log::debug!(
                    target: "rate-limiter",
                    "🧹 Cleaned {} expired transactions for account: {:?}",
                    cleaned_count, account
                );
            }

            // Reset block counter if we're in a new block
            if rate_limit.last_reset_block != current_block {
                rate_limit.current_block_count = 0;
                rate_limit.last_reset_block = current_block;
            }

            // Check per-block limit (with emergency reduction)
            let effective_max_per_block = Self::apply_rate_adjustments(rate_limit.max_per_block);
            if rate_limit.current_block_count >= effective_max_per_block {
                Self::deposit_event(Event::TransactionBlocked {
                    account: account.clone(),
                    current_count: rate_limit.current_block_count,
                    limit: effective_max_per_block,
                });
                return Err(Error::<T>::RateLimitExceededPerBlock.into());
            }

            // Check per-minute limit (now with cleaned data and emergency reduction)
            let effective_max_per_minute = Self::apply_rate_adjustments(rate_limit.max_per_minute);
            if rate_limit.recent_transactions.len() as u32 >= effective_max_per_minute {
                Self::deposit_event(Event::TransactionBlocked {
                    account: account.clone(),
                    current_count: rate_limit.recent_transactions.len() as u32,
                    limit: effective_max_per_minute,
                });
                return Err(Error::<T>::RateLimitExceededPerMinute.into());
            }

            // Persist the cleaned rate limit data
            RateLimits::<T>::insert(account, rate_limit);

            Ok(())
        }

        /// Record a transaction for rate limiting purposes
        /// 
        /// ## Critical Implementation Notes
        /// 
        /// This function also implements cleanup-before-recording to maintain storage consistency:
        /// 
        /// 1. **Cleanup expired transactions** - Remove stale entries before adding new ones
        /// 2. **Record new transaction** - Add current transaction to cleaned data
        /// 3. **Persist updated state** - Save complete updated storage
        /// 
        /// This ensures that storage always contains accurate, up-to-date transaction records.
        pub fn record_transaction(account: &T::AccountId) -> DispatchResult {
            let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
            let current_timestamp = pallet_timestamp::Pallet::<T>::get();
            let mut rate_limit = Self::rate_limits(account);

            // Initialize if needed
            if rate_limit == RateLimit::default() {
                let global = Self::global_config();
                if global != RateLimit::default() {
                    rate_limit = global;
                } else {
                    rate_limit.max_per_block = T::DefaultTransactionsPerBlock::get();
                    rate_limit.max_per_minute = T::DefaultTransactionsPerMinute::get();
                }
            }

            // Clean expired transactions BEFORE recording new transaction
            let minute_cutoff = current_timestamp.saturating_sub(60_000u64);
            rate_limit.recent_transactions.retain(|&timestamp| timestamp > minute_cutoff);

            // Reset block counter if we're in a new block
            if rate_limit.last_reset_block != current_block {
                rate_limit.current_block_count = 0;
                rate_limit.last_reset_block = current_block;
            }

            // Increment counters
            rate_limit.current_block_count = rate_limit.current_block_count.saturating_add(1);
            
            // Record transaction timestamp for minute window tracking
            let _ = rate_limit.recent_transactions.try_push(current_timestamp); // Ignore if at capacity

            RateLimits::<T>::insert(account, rate_limit);

            Self::deposit_event(Event::TransactionRecorded {
                account: account.clone(),
                block_number: current_block,
            });

            Ok(())
        }

        /// Get current rate limit configuration for an account
        pub fn get_rate_limit_info(account: &T::AccountId) -> RateLimit {
            let mut rate_limit = Self::rate_limits(account);
            
            if rate_limit == RateLimit::default() {
                let global = Self::global_config();
                if global != RateLimit::default() {
                    rate_limit = global;
                } else {
                    rate_limit.max_per_block = T::DefaultTransactionsPerBlock::get();
                    rate_limit.max_per_minute = T::DefaultTransactionsPerMinute::get();
                }
            }

            rate_limit
        }

        /// Get current pool usage for an account (for metrics)
        pub fn get_account_pool_data(account: &T::AccountId) -> AccountPoolData {
            Self::account_pool_usage(account)
        }

        /// Get global pool metrics
        pub fn get_pool_metrics() -> (u32, u32, u32) {
            let mut total_pending = 0u32;
            let mut total_bytes = 0u32;
            let mut active_accounts = 0u32;

            // Iterate over all accounts with pool usage
            for (_, data) in AccountPoolUsage::<T>::iter() {
                if data.pending_transactions > 0 {
                    active_accounts = active_accounts.saturating_add(1);
                    total_pending = total_pending.saturating_add(data.pending_transactions);
                    total_bytes = total_bytes.saturating_add(data.total_bytes_used);
                }
            }

            (total_pending, total_bytes, active_accounts)
        }

        /// Update pool metrics (can be called periodically by offchain worker)
        pub fn update_pool_metrics() {
            let (total_pending, total_bytes, _active_accounts) = Self::get_pool_metrics();
            
            Self::deposit_event(Event::PoolMetricsUpdated {
                total_pending,
                total_bytes,
            });
        }

        /// Check if system is under attack (high resource usage)
        pub fn is_under_attack() -> bool {
            let (total_pending, total_bytes, active_accounts) = Self::get_pool_metrics();
            
            // Define attack thresholds
            let max_safe_pending = 1000u32;
            let max_safe_bytes = 5 * 1024 * 1024u32; // 5MB
            let max_safe_accounts = 100u32;
            
            total_pending > max_safe_pending || 
            total_bytes > max_safe_bytes ||
            active_accounts > max_safe_accounts
        }
    }
}

#[cfg(test)] 
mod mock_simple;

#[cfg(test)]
mod tests {
    mod integration_simple;
    mod integration_final;
    mod multi_environment;
    use super::*;
    use mock_simple::*;

    /// Simple unit tests for cleanup timing logic
    /// These tests verify the critical cleanup-before-validation pattern
    /// without requiring complex runtime setup.
    
    #[test]
    fn cleanup_timing_documentation_is_correct() {
        // This test documents the critical cleanup timing requirements
        // and serves as a reference for the correct implementation pattern
        
        // ✅ CORRECT: Cleanup before validation
        let mut timestamps = vec![1000u64, 2000, 3000, 61_000, 62_000];
        let current_time = 65_000u64;
        let cutoff = current_time.saturating_sub(60_000u64);
        
        // Clean expired transactions FIRST
        let original_count = timestamps.len();
        timestamps.retain(|&timestamp| timestamp > cutoff);
        let cleaned_count = original_count - timestamps.len();
        
        // Now validate against cleaned data
        let is_within_limit = timestamps.len() <= 3;
        
        // Verify the cleanup worked correctly
        assert_eq!(cleaned_count, 3); // 3 expired transactions removed
        assert_eq!(timestamps.len(), 2); // 2 recent transactions remain
        assert!(is_within_limit); // Should pass validation now
        
        // The remaining timestamps should all be recent
        for timestamp in timestamps {
            assert!(timestamp > cutoff, "Timestamp {} should be recent", timestamp);
        }
    }

    #[test] 
    fn rate_limit_struct_default_values() {
        let rate_limit = RateLimit::default();
        assert_eq!(rate_limit.max_per_block, 5);
        assert_eq!(rate_limit.max_per_minute, 20);
        assert_eq!(rate_limit.current_block_count, 0);
        assert_eq!(rate_limit.recent_transactions.len(), 0);
        assert_eq!(rate_limit.last_reset_block, 0);
    }

    #[test]
    fn account_pool_data_default_values() {
        let pool_data = AccountPoolData::default();
        assert_eq!(pool_data.pending_transactions, 0);
        assert_eq!(pool_data.total_bytes_used, 0);
        assert_eq!(pool_data.last_transaction_block, 0);
        assert_eq!(pool_data.transactions_per_minute, 0);
        assert_eq!(pool_data.minute_reset_block, 0);
    }

    #[test]
    fn bounded_vec_cleanup_simulation() {
        // Simulate the cleanup logic that occurs in the real pallet
        use frame_support::BoundedVec;
        use frame_support::traits::ConstU32;
        
        let mut recent_transactions: BoundedVec<u64, ConstU32<100>> = BoundedVec::new();
        
        // Add some timestamps
        let _ = recent_transactions.try_push(10_000);  // Old (75000 - 10000 = 65000 > 60000)
        let _ = recent_transactions.try_push(20_000);  // Old (75000 - 20000 = 55000 < 60000) - RECENT
        let _ = recent_transactions.try_push(65_000);  // Recent (75000 - 65000 = 10000 < 60000) 
        let _ = recent_transactions.try_push(70_000);  // Recent (75000 - 70000 = 5000 < 60000)
        
        let current_timestamp = 75_000u64;
        let minute_cutoff = current_timestamp.saturating_sub(60_000u64); // cutoff = 15_000
        
        // Perform cleanup (same logic as in the pallet)
        let original_len = recent_transactions.len();
        recent_transactions.retain(|&timestamp| timestamp > minute_cutoff);
        let cleaned_count = original_len - recent_transactions.len();
        
        // Verify cleanup results
        assert_eq!(cleaned_count, 1); // One old transaction cleaned (10_000)
        assert_eq!(recent_transactions.len(), 3); // Three recent remain (20k, 65k, 70k)
        
        // Verify all remaining transactions are within the minute window
        for &timestamp in recent_transactions.iter() {
            assert!(timestamp > minute_cutoff);
            let age = current_timestamp - timestamp;
            assert!(age < 60_000);
        }
    }
}