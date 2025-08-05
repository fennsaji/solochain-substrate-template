//! Transaction extension for rate limiting integration.
//!
//! This module provides the CheckRateLimit transaction extension that integrates
//! rate limiting into the transaction validation pipeline.

use crate::{Config, Pallet};
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{
    dispatch::{DispatchInfo, PostDispatchInfo},
    traits::OriginTrait,
    weights::Weight,
};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{
        DispatchInfoOf, Dispatchable, PostDispatchInfoOf, 
        TransactionExtension, ValidateResult,
    },
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource,
        TransactionValidityError, ValidTransaction,
    },
};
use sp_std::{marker::PhantomData, vec, vec::Vec};

/// Transaction extension for rate limiting.
/// 
/// This extension checks rate limits before allowing transactions to be included
/// in the transaction pool, providing spam protection for fee-free transactions.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct CheckRateLimit<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> CheckRateLimit<T> {
    /// Create new `CheckRateLimit` extension.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: Config + Send + Sync> Default for CheckRateLimit<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Config + Send + Sync> core::fmt::Debug for CheckRateLimit<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "CheckRateLimit")
    }
}

impl<T: Config + Send + Sync> TransactionExtension<T::RuntimeCall> for CheckRateLimit<T>
where
    T::RuntimeCall: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str = "CheckRateLimit";
    type Implicit = ();
    type Val = ();
    type Pre = Option<T::AccountId>;
    
    fn implicit(&self) -> Result<Self::Implicit, TransactionValidityError> {
        Ok(())
    }

    fn weight(&self, _call: &T::RuntimeCall) -> Weight {
        // Light weight for rate limit check
        Weight::from_parts(10_000, 0)
    }

    fn validate(
        &self,
        origin: <T::RuntimeCall as Dispatchable>::RuntimeOrigin,
        call: &T::RuntimeCall,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        _len: usize,
        _self_implicit: Self::Implicit,
        _inherited_implication: &impl Encode,
        _source: TransactionSource,
    ) -> ValidateResult<Self::Val, T::RuntimeCall> {
        // Extract the account ID from the origin
        let who = match origin.clone().into_signer() {
            Some(account) => account,
            None => {
                // Non-signed transactions (inherents, etc.) are allowed
                return Ok((Default::default(), Default::default(), origin));
            }
        };

        // Check if this call should be exempt from rate limiting
        if !should_rate_limit::<T>(call) {
            log::debug!(
                target: "rate-limiter",
                "🔓 Call exempt from rate limiting for account: {:?}",
                who
            );
            return Ok((Default::default(), Default::default(), origin));
        }

        // Get transaction size for pool limits
        let transaction_bytes = call.encode().len() as u32;
        
        // Check enhanced pool limits first
        if let Err(error) = Pallet::<T>::can_submit_transaction(&who, transaction_bytes) {
            log::warn!(
                target: "rate-limiter",
                "❌ Pool limit exceeded for account: {:?}, bytes: {}, error: {:?}",
                who, transaction_bytes, error
            );
            
            // Convert to appropriate InvalidTransaction error - all resource limit errors
            let invalid_error = InvalidTransaction::ExhaustsResources;
            
            return Err(TransactionValidityError::Invalid(invalid_error));
        }

        // Check traditional rate limits for the account
        match Pallet::<T>::check_rate_limit(&who) {
            Ok(()) => {
                // Rate limit check passed
                log::debug!(
                    target: "rate-limiter",
                    "✅ Rate limit check passed for account: {:?}",
                    who
                );
                
                // Return valid transaction with medium priority
                let valid_transaction = ValidTransaction {
                    priority: TransactionPriority::default(),
                    requires: vec![],
                    provides: vec![],
                    longevity: 64,
                    propagate: true,
                };
                
                // Reconstruct origin from account
                let new_origin = <T::RuntimeCall as Dispatchable>::RuntimeOrigin::from(
                    frame_system::RawOrigin::Signed(who)
                );
                
                Ok((valid_transaction, (), new_origin))
            },
            Err(error) => {
                // Rate limit exceeded - reject transaction
                log::warn!(
                    target: "rate-limiter",
                    "❌ Rate limit exceeded for account: {:?}, error: {:?}",
                    who, error
                );
                
                // Convert DispatchError to InvalidTransaction
                let invalid_error = match error {
                    sp_runtime::DispatchError::Module(module_error) => {
                        log::warn!(
                            target: "rate-limiter",
                            "Module error details - index: {}, error: {:?}",
                            module_error.index, module_error.error
                        );
                        InvalidTransaction::ExhaustsResources
                    },
                    _ => InvalidTransaction::Call,
                };
                
                Err(TransactionValidityError::Invalid(invalid_error))
            }
        }
    }

    fn prepare(
        self,
        _val: Self::Val,
        origin: &<T::RuntimeCall as Dispatchable>::RuntimeOrigin,
        _call: &T::RuntimeCall,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        // Store the account ID for post-dispatch recording
        let account_id = match origin.clone().into_signer() {
            Some(account) => Some(account),
            None => None, // Inherents and unsigned transactions
        };
        
        log::debug!(
            target: "rate-limiter",
            "🔧 Preparing rate limit check for account: {:?}",
            account_id
        );
        
        Ok(account_id)
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        _info: &DispatchInfoOf<T::RuntimeCall>,
        _post_info: &PostDispatchInfoOf<T::RuntimeCall>,
        len: usize,
        result: &sp_runtime::DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        // Note: Transaction size estimation (better to get from context if available)
        let transaction_bytes = len as u32;
        
        if let Some(account_id) = pre {
            match result {
                Ok(_) => {
                    // Record successful transaction for rate limiting
                    match Pallet::<T>::record_transaction(&account_id) {
                        Ok(()) => {
                            log::debug!(
                                target: "rate-limiter",
                                "✅ Successfully recorded transaction for account: {:?}",
                                account_id
                            );
                        },
                        Err(error) => {
                            log::warn!(
                                target: "rate-limiter",
                                "❌ Failed to record transaction for account: {:?}, error: {:?}",
                                account_id, error
                            );
                            // Don't fail the transaction if recording fails, just log it
                        }
                    }
                    
                    // Transaction was successful, pool tracking already updated in can_submit_transaction
                    Ok(Weight::from_parts(10_000, 0))
                },
                Err(_) => {
                    // Transaction failed - clean up pool usage since it won't be in pool anymore  
                    Pallet::<T>::on_transaction_removed(&account_id, transaction_bytes);
                    
                    log::debug!(
                        target: "rate-limiter", 
                        "Transaction failed, cleaned up pool usage for account: {:?}",
                        account_id
                    );
                    
                    Ok(Weight::from_parts(5_000, 0))
                }
            }
        } else {
            // Unsigned transaction - no cleanup needed
            Ok(Weight::from_parts(0, 0))
        }
    }
}

/// Helper function to check if a call should be rate limited.
/// 
/// Some calls (like sudo calls or governance calls) are exempt from rate limiting
/// to prevent deadlock situations where the system cannot be unpaused.
fn should_rate_limit<T: Config>(call: &T::RuntimeCall) -> bool {
    use codec::Encode;
    
    // Get the encoded call to inspect its structure
    let encoded = call.encode();
    
    // Check if this is a sudo call (pallet index 5 in most substrate runtimes)
    // The first byte is typically the pallet index
    if !encoded.is_empty() {
        let pallet_index = encoded[0];
        
        // Pallet index 5 is typically sudo in substrate runtimes
        // Allow sudo calls to bypass rate limiting to prevent deadlock
        if pallet_index == 5 {
            log::debug!(
                target: "rate-limiter",
                "🔓 Exempting sudo call from rate limiting (pallet index: {})",
                pallet_index
            );
            return false;
        }
    }
    
    // Rate limit all other calls
    true
}

// Tests are covered in the main pallet lib.rs file which has proper test setup