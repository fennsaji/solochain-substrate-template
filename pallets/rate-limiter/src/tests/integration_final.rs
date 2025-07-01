//! Final integration tests for rate limiter pallet
//!
//! These tests verify the rate limiter works correctly with a minimal runtime,
//! focusing on emergency controls and adaptive scaling.

use super::*;
use crate::mock_simple::*;
use frame_support::{assert_ok, assert_noop};
use crate::{Pallet, Error};

/// Test emergency rate reduction integration with runtime
#[test]
fn test_emergency_rate_reduction_integration() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let base_limit = 10u32;
        
        // Set up rate limit
        assert_ok!(Pallet::<Test>::set_rate_limit(
            RuntimeOrigin::root(),
            account,
            base_limit,
            base_limit * 6,
        ));
        
        // Verify normal operation - should allow base_limit transactions
        for i in 0..base_limit {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        // Should be at limit now
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
        
        // Enable emergency rate reduction (50%)
        assert_ok!(Pallet::<Test>::set_emergency_rate_reduction(
            RuntimeOrigin::root(),
            50u8
        ));
        
        // Move to next block
        System::set_block_number(2);
        
        // Now should only allow 50% of original limit (5 transactions)
        for i in 0..5 {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        // Should be at reduced limit now
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
        
        // Test emergency shutdown (100% reduction)
        assert_ok!(Pallet::<Test>::set_emergency_rate_reduction(
            RuntimeOrigin::root(),
            100u8
        ));
        
        System::set_block_number(3);
        
        // Should reject all transactions
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
        
        // Clear emergency reduction
        assert_ok!(Pallet::<Test>::clear_emergency_rate_reduction(
            RuntimeOrigin::root()
        ));
        
        System::set_block_number(4);
        
        // Should be back to normal limits
        assert_ok!(Pallet::<Test>::check_rate_limit(&account));
    });
}

/// Test adaptive load scaling integration
#[test]
fn test_adaptive_load_scaling_integration() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let base_limit = 10u32;
        
        // Set up rate limit
        assert_ok!(Pallet::<Test>::set_rate_limit(
            RuntimeOrigin::root(),
            account,
            base_limit,
            base_limit * 6,
        ));
        
        // Enable adaptive scaling (150%)
        assert_ok!(Pallet::<Test>::set_adaptive_load_scaling(
            RuntimeOrigin::root(),
            150u8
        ));
        
        // Should now allow 150% of base limit (15 transactions)
        for i in 0..15 {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        // Should be at scaled limit now
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
        
        // Test maximum scaling (200%)
        assert_ok!(Pallet::<Test>::set_adaptive_load_scaling(
            RuntimeOrigin::root(),
            200u8
        ));
        
        System::set_block_number(2);
        
        // Should allow 200% of base limit (20 transactions)
        for i in 0..20 {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
    });
}

/// Test that emergency reduction takes priority over adaptive scaling
#[test]
fn test_emergency_priority_over_adaptive() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let base_limit = 20u32;
        
        assert_ok!(Pallet::<Test>::set_rate_limit(
            RuntimeOrigin::root(),
            account,
            base_limit,
            base_limit * 6,
        ));
        
        // Enable both emergency reduction (50%) and adaptive scaling (150%)
        assert_ok!(Pallet::<Test>::set_emergency_rate_reduction(
            RuntimeOrigin::root(),
            50u8
        ));
        assert_ok!(Pallet::<Test>::set_adaptive_load_scaling(
            RuntimeOrigin::root(),
            150u8
        ));
        
        // Should only allow emergency-reduced limit (10 transactions)
        // NOT the adaptive scaled amount (30 transactions)
        for i in 0..10 {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
        
        // Clear emergency reduction - adaptive scaling should take effect
        assert_ok!(Pallet::<Test>::clear_emergency_rate_reduction(
            RuntimeOrigin::root()
        ));
        
        System::set_block_number(2);
        
        // Should now allow adaptive scaled limit (30 transactions = 20 * 150%)
        for i in 0..30 {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
    });
}

/// Test rate limiting across multiple blocks
#[test]
fn test_cross_block_rate_limiting() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let limit_per_block = 5u32;
        let limit_per_minute = 25u32;
        
        assert_ok!(Pallet::<Test>::set_rate_limit(
            RuntimeOrigin::root(),
            account,
            limit_per_block,
            limit_per_minute,
        ));
        
        // Fill up the first block
        for i in 0..limit_per_block {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        // Should be at block limit
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerBlock
        );
        
        // Move to next block - block counter should reset
        System::set_block_number(2);
        
        // Should be able to do more transactions (new block)
        for i in 0..limit_per_block {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        // Continue for several blocks to test minute limit
        for block in 3..7 {
            System::set_block_number(block);
            for i in 0..limit_per_block {
                assert_ok!(Pallet::<Test>::check_rate_limit(&account));
                assert_ok!(Pallet::<Test>::record_transaction(&account));
            }
        }
        
        // Should now be at minute limit (25 transactions over 5 blocks)
        System::set_block_number(7);
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerMinute
        );
    });
}

/// Test storage cleanup occurs before validation
#[test]
fn test_cleanup_before_validation() {
    new_test_ext().execute_with(|| {
        let account = 1u64;
        let limit_per_minute = 5u32;
        
        assert_ok!(Pallet::<Test>::set_rate_limit(
            RuntimeOrigin::root(),
            account,
            100u32, // High block limit so we only test minute limit
            limit_per_minute,
        ));
        
        // Set initial timestamp
        let start_time = 1000u64;
        Timestamp::set_timestamp(start_time);
        
        // Fill up to the minute limit
        for i in 0..limit_per_minute {
            assert_ok!(Pallet::<Test>::check_rate_limit(&account));
            assert_ok!(Pallet::<Test>::record_transaction(&account));
        }
        
        // Should be at minute limit
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&account),
            Error::<Test>::RateLimitExceededPerMinute
        );
        
        // Advance time by more than a minute (cleanup should occur)
        Timestamp::set_timestamp(start_time + 70_000); // 70 seconds
        
        // Should be able to transact again (old transactions cleaned up)
        assert_ok!(Pallet::<Test>::check_rate_limit(&account));
        assert_ok!(Pallet::<Test>::record_transaction(&account));
    });
}

/// Test multiple accounts with different limits
#[test]
fn test_multi_account_different_limits() {
    new_test_ext().execute_with(|| {
        let regular_account = 1u64;
        let priority_account = 2u64;
        
        // Set different limits
        assert_ok!(Pallet::<Test>::set_rate_limit(
            RuntimeOrigin::root(),
            regular_account,
            5u32,
            30u32,
        ));
        
        assert_ok!(Pallet::<Test>::set_rate_limit(
            RuntimeOrigin::root(),
            priority_account,
            20u32,
            120u32,
        ));
        
        // Regular account - 5 transactions
        for i in 0..5 {
            assert_ok!(Pallet::<Test>::check_rate_limit(&regular_account));
            assert_ok!(Pallet::<Test>::record_transaction(&regular_account));
        }
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&regular_account),
            Error::<Test>::RateLimitExceededPerBlock
        );
        
        // Priority account - 20 transactions
        for i in 0..20 {
            assert_ok!(Pallet::<Test>::check_rate_limit(&priority_account));
            assert_ok!(Pallet::<Test>::record_transaction(&priority_account));
        }
        assert_noop!(
            Pallet::<Test>::check_rate_limit(&priority_account),
            Error::<Test>::RateLimitExceededPerBlock
        );
    });
}