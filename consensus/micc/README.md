# Micc Consensus Module

- [`micc::Config`](https://docs.rs/pallet-micc/latest/pallet_micc/pallet/trait.Config.html)
- [`Pallet`](https://docs.rs/pallet-micc/latest/pallet_micc/pallet/struct.Pallet.html)

## Overview

The micc module extends micc consensus by managing offline reporting.

## Interface

### Public Functions

- `slot_duration` - Determine the Micc slot-duration based on the Timestamp module configuration.

## Related Modules

- [Timestamp](https://docs.rs/pallet-timestamp/latest/pallet_timestamp/): The Timestamp module is used in Micc to track
consensus rounds (via `slots`).

## References

If you're interested in hacking on this module, it is useful to understand the interaction with
`substrate/primitives/inherents/src/lib.rs` and, specifically, the required implementation of
[`ProvideInherent`](https://docs.rs/sp-inherents/latest/sp_inherents/trait.ProvideInherent.html) and
[`ProvideInherentData`](https://docs.rs/sp-inherents/latest/sp_inherents/trait.ProvideInherentData.html) to create and
check inherents.

License: Apache-2.0


## Release

Polkadot SDK stable2409
