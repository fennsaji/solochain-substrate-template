//! Minimal mock runtime for rate limiter pallet testing

use frame_support::{
    derive_impl, parameter_types,
    traits::{ConstU32, ConstU64, ConstU128},
    weights::Weight,
};
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a minimal mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Timestamp: pallet_timestamp,
        RateLimiter: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
}

impl pallet_timestamp::Config for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

parameter_types! {
    pub const DefaultTransactionsPerBlock: u32 = 10;
    pub const DefaultTransactionsPerMinute: u32 = 60;
    pub const MinimumBalance: u128 = 100;
}

/// Minimal currency implementation for testing
pub struct TestCurrency;

impl frame_support::traits::Currency<u64> for TestCurrency {
    type Balance = u128;
    type PositiveImbalance = ();
    type NegativeImbalance = ();

    fn total_balance(who: &u64) -> Self::Balance {
        1000 // Everyone has 1000 balance for testing
    }

    fn can_slash(who: &u64, value: Self::Balance) -> bool {
        false
    }

    fn total_issuance() -> Self::Balance {
        10000
    }

    fn minimum_balance() -> Self::Balance {
        1
    }

    fn burn(amount: Self::Balance) -> Self::PositiveImbalance {
        ()
    }

    fn issue(amount: Self::Balance) -> Self::NegativeImbalance {
        ()
    }

    fn pair(amount: Self::Balance) -> (Self::PositiveImbalance, Self::NegativeImbalance) {
        ((), ())
    }

    fn free_balance(who: &u64) -> Self::Balance {
        1000
    }

    fn ensure_can_withdraw(
        who: &u64,
        amount: Self::Balance,
        reasons: frame_support::traits::WithdrawReasons,
        new_balance: Self::Balance,
    ) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn transfer(
        source: &u64,
        dest: &u64,
        value: Self::Balance,
        existence_requirement: frame_support::traits::ExistenceRequirement,
    ) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn slash(who: &u64, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        ((), 0)
    }

    fn deposit_into_existing(
        who: &u64,
        value: Self::Balance,
    ) -> Result<Self::PositiveImbalance, sp_runtime::DispatchError> {
        Ok(())
    }

    fn deposit_creating(who: &u64, value: Self::Balance) -> Self::PositiveImbalance {
        ()
    }

    fn withdraw(
        who: &u64,
        value: Self::Balance,
        reasons: frame_support::traits::WithdrawReasons,
        liveness: frame_support::traits::ExistenceRequirement,
    ) -> Result<Self::NegativeImbalance, sp_runtime::DispatchError> {
        Ok(())
    }

    fn make_free_balance_be(
        who: &u64,
        balance: Self::Balance,
    ) -> frame_support::traits::SignedImbalance<Self::Balance, Self::PositiveImbalance> {
        frame_support::traits::SignedImbalance::Positive(())
    }
}

impl frame_support::traits::ReservableCurrency<u64> for TestCurrency {
    fn can_reserve(_who: &u64, _value: Self::Balance) -> bool {
        true
    }

    fn slash_reserved(
        _who: &u64,
        _value: Self::Balance,
    ) -> (Self::NegativeImbalance, Self::Balance) {
        ((), 0)
    }

    fn reserved_balance(_who: &u64) -> Self::Balance {
        0
    }

    fn reserve(_who: &u64, _value: Self::Balance) -> sp_runtime::DispatchResult {
        Ok(())
    }

    fn unreserve(_who: &u64, _value: Self::Balance) -> Self::Balance {
        0
    }

    fn repatriate_reserved(
        _slashed: &u64,
        _beneficiary: &u64,
        _value: Self::Balance,
        _status: frame_support::traits::BalanceStatus,
    ) -> Result<Self::Balance, sp_runtime::DispatchError> {
        Ok(0)
    }
}

impl crate::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Currency = TestCurrency;
    type DefaultTransactionsPerBlock = DefaultTransactionsPerBlock;
    type DefaultTransactionsPerMinute = DefaultTransactionsPerMinute;
    type MinimumBalance = MinimumBalance;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| {
        System::set_block_number(1);
        Timestamp::set_timestamp(12345);
    });
    ext
}