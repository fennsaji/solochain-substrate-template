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
    traits::{Get, ReservableCurrency, Currency},
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
    }

    impl<T: Config> Pallet<T> {
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
            
            // Check per-account pending transaction limit
            if usage.pending_transactions >= T::MaxTransactionsPerAccount::get() {
                Self::deposit_event(Event::AccountPoolLimitExceeded {
                    account: who.clone(),
                    pending_count: usage.pending_transactions,
                    byte_usage: usage.total_bytes_used,
                });
                return Err(Error::<T>::TooManyPendingTransactions);
            }
            
            // Check per-account byte limit
            if usage.total_bytes_used.saturating_add(transaction_bytes) >= T::MaxBytesPerAccount::get() {
                Self::deposit_event(Event::AccountPoolLimitExceeded {
                    account: who.clone(),
                    pending_count: usage.pending_transactions,
                    byte_usage: usage.total_bytes_used,
                });
                return Err(Error::<T>::AccountPoolLimitExceeded);
            }
            
            // Check per-minute transaction rate (important for 500ms blocks)
            if usage.transactions_per_minute >= T::MaxTransactionsPerMinute::get() {
                Self::deposit_event(Event::MinuteRateLimitExceeded {
                    account: who.clone(),
                    current_rate: usage.transactions_per_minute,
                    limit: T::MaxTransactionsPerMinute::get(),
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

            // Reset block counter if we're in a new block
            if rate_limit.last_reset_block != current_block {
                rate_limit.current_block_count = 0;
                rate_limit.last_reset_block = current_block;
            }

            // Check per-block limit
            if rate_limit.current_block_count >= rate_limit.max_per_block {
                Self::deposit_event(Event::TransactionBlocked {
                    account: account.clone(),
                    current_count: rate_limit.current_block_count,
                    limit: rate_limit.max_per_block,
                });
                return Err(Error::<T>::RateLimitExceededPerBlock.into());
            }

            // Clean old transactions from minute window (60 seconds = 60,000 milliseconds)
            let current_timestamp = pallet_timestamp::Pallet::<T>::get();
            let minute_cutoff = current_timestamp.saturating_sub(60_000u64);
            rate_limit.recent_transactions.retain(|&timestamp| timestamp > minute_cutoff);

            // Check per-minute limit
            if rate_limit.recent_transactions.len() as u32 >= rate_limit.max_per_minute {
                Self::deposit_event(Event::TransactionBlocked {
                    account: account.clone(),
                    current_count: rate_limit.recent_transactions.len() as u32,
                    limit: rate_limit.max_per_minute,
                });
                return Err(Error::<T>::RateLimitExceededPerMinute.into());
            }

            Ok(())
        }

        /// Record a transaction for rate limiting purposes
        pub fn record_transaction(account: &T::AccountId) -> DispatchResult {
            let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
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

            // Reset block counter if we're in a new block
            if rate_limit.last_reset_block != current_block {
                rate_limit.current_block_count = 0;
                rate_limit.last_reset_block = current_block;
            }

            // Increment counters
            rate_limit.current_block_count = rate_limit.current_block_count.saturating_add(1);
            
            // Record transaction timestamp for minute window tracking
            let current_timestamp = pallet_timestamp::Pallet::<T>::get();
            let _ = rate_limit.recent_transactions.try_push(current_timestamp); // Ignore if at capacity

            // Clean old transactions and limit vector size (60 seconds = 60,000 milliseconds)
            let minute_cutoff = current_timestamp.saturating_sub(60_000u64);
            rate_limit.recent_transactions.retain(|&timestamp| timestamp > minute_cutoff);

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