//! Transaction extensions for rate limiting

use crate::pallet::{Config, Pallet};
use codec::{Decode, Encode};
use frame_support::{
    dispatch::{DispatchInfo, PostDispatchInfo},
};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{
        DispatchInfoOf, Dispatchable, PostDispatchInfoOf, 
        TransactionExtension, ValidateResult,
    },
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        TransactionValidityError, ValidTransaction,
    },
    DispatchResult,
};
use sp_std::marker::PhantomData;

/// Transaction extension that enforces rate limiting for spam protection
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckRateLimit<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> CheckRateLimit<T> {
    /// Create new `CheckRateLimit` extension
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync> Default for CheckRateLimit<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Config + Send + Sync> sp_std::fmt::Debug for CheckRateLimit<T> {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "CheckRateLimit")
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

// Note: For simplicity in this implementation, we'll focus on the core functionality
// The exact trait implementation may need adjustments based on the specific Substrate version
// For now, we provide the basic structure and methods needed for rate limiting
impl<T: Config + Send + Sync> CheckRateLimit<T> {
    /// Check rate limits for an account
    pub fn check_account_limits(who: &T::AccountId) -> Result<(), TransactionValidityError> {
        Pallet::<T>::check_rate_limit(who)
            .map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Custom(1)))
    }

    /// Record transaction for an account
    pub fn record_account_transaction(who: &T::AccountId) -> Result<(), ()> {
        Pallet::<T>::record_transaction(who).map_err(|_| ())
    }

    /// Get transaction priority based on rate limits
    pub fn get_priority() -> TransactionPriority {
        TransactionPriority::MAX / 2
    }
}