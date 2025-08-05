# 🏛️ MICC Consensus Pallet

> **MICC (Metamui Instant Confirmation Consensus)** - Runtime pallet for event-driven consensus

## 📋 Overview

The MICC pallet is the runtime-side component of the MICC consensus system. It integrates with Substrate's session and authority management systems to provide slot-based block production with event-driven enhancements.

**Key Features:**
- ✅ **Authority Management** - Manages validator sets and rotations
- ✅ **Slot-based Authority Selection** - Round-robin authority assignment
- ✅ **Session Integration** - Seamless validator set changes
- ✅ **Event-driven Block Production** - Revolutionary consensus timing

## 🔧 Architecture

### **Authority Management**
```rust
// Round-robin authority selection
let authority_index = slot % authorities.len();
let expected_authority = authorities[authority_index];
```

### **Slot Duration Configuration**
```rust
// Configurable slot duration (default: 2x timestamp minimum period)
pub type MinimumPeriodTimesTwo<T> = ConstU64<{ 2 * SLOT_DURATION }>;
```

## 📦 Core Components

### **Storage Items**

| Storage | Type | Description |
|---------|------|-------------|
| `Authorities<T>` | `Vec<T::AuthorityId>` | Current validator authority set |
| `CurrentSlot<T>` | `Slot` | Current consensus slot number |

### **Configuration Trait**
```rust
pub trait Config: frame_system::Config + pallet_timestamp::Config {
    type AuthorityId: Member + Parameter + RuntimeAppPublic + Ord + MaybeSerializeDeserialize;
    type MaxAuthorities: Get<u32>;
    type MinimumPeriodTimesTwo: Get<u64>;
}
```

## 🎯 Key Functions

### **Authority Management**
- **`change_authorities()`** - Updates the validator set
- **`initialize_authorities()`** - Sets genesis authorities
- **`current_authorities()`** - Returns current validator set

### **Slot Operations**
- **`slot_duration()`** - Returns configured slot duration
- **`current_slot_from_digests()`** - Extracts slot from block digests
- **`minimum_period()`** - Returns minimum period for slot calculations

### **Session Integration**
- **`on_genesis_session()`** - Initializes authorities at genesis
- **`on_new_session()`** - Handles validator set changes

## 🔗 Integration with Other Modules

### **With MICC Client (`micc-client`)**
```rust
// Provides authority information for block production
let authorities = micc::Pallet::<Runtime>::authorities();
let slot_duration = micc::Pallet::<Runtime>::slot_duration();
```

### **With MICC Primitives (`micc-primitives`)**
```rust
// Uses shared consensus types
use sp_consensus_micc::{AuthorityId, Slot, ConsensusLog};
```

### **With Slots Module (`slots`)**
```rust
// Provides slot timing utilities
use sc_consensus_slots::{SlotInfo, BackoffAuthoringBlocksStrategy};
```

## ⚙️ Configuration Example

```rust
// In runtime/src/configs/mod.rs
impl pallet_micc::Config for Runtime {
    type AuthorityId = MiccId;
    type MaxAuthorities = ConstU32<32>;
    type MinimumPeriodTimesTwo = ConstU64<{ 2 * MILLISECS_PER_BLOCK }>;
}

// In runtime/src/lib.rs
#[runtime::pallet_index(2)]
pub type Micc = pallet_micc::Pallet<Runtime>;
```

## 🔄 Authority Rotation Logic

```rust
// Deterministic authority selection
fn expected_authority(slot: Slot, authorities: &[AuthorityId]) -> Option<&AuthorityId> {
    if authorities.is_empty() {
        return None;
    }
    
    let authority_index = (*slot % authorities.len() as u64) as usize;
    authorities.get(authority_index)
}
```

## 📊 Events

| Event | Description |
|-------|-------------|
| `AuthoritiesChanged` | Emitted when validator set changes |
| `NewAuthorities` | Emitted when new authorities are set |

## 🔍 Inherent Data

The pallet provides slot inherent data:
```rust
impl<T: Config> ProvideInherent for Pallet<T> {
    type Call = Call<T>;
    type Error = InherentError;
    const INHERENT_IDENTIFIER: InherentIdentifier = MICC_INHERENT_IDENTIFIER;
    
    fn create_inherent(data: &InherentData) -> Option<Self::Call> {
        // Creates slot inherent from provided data
    }
}
```

## 🛡️ Security Considerations

### **Authority Validation**
- ✅ Only session handlers can change authorities
- ✅ Authority set size limits enforced (`MaxAuthorities`)
- ✅ Slot-based authority assignment prevents gaming

### **Slot Integrity**
- ✅ Slot numbers are monotonically increasing
- ✅ Slot validation in block digests
- ✅ Prevents slot manipulation attacks

## 🧪 Testing

```bash
# Run pallet tests
cargo test -p pallet-micc

# Run with consensus integration tests
cargo test -p solochain-template-runtime -- consensus
```

## 📚 Related Documentation

- **[MICC Client](../micc-client/README.md)** - Block production and event-driven consensus
- **[MICC Primitives](../micc-primitives/README.md)** - Core consensus types
- **[Slots](../slots/README.md)** - Slot timing utilities
- **[Substrate Session](https://docs.rs/pallet-session/latest/pallet_session/)** - Session management
- **[Substrate Consensus](https://docs.substrate.io/fundamentals/consensus/)** - Consensus fundamentals

## 📜 License

Apache-2.0

## 🏷️ Release

Based on Polkadot SDK stable2409 with MICC consensus enhancements.

---

> 💡 **Pro Tip**: For event-driven block production, the pallet works in conjunction with `micc-client` which monitors transaction pool events for instant block production triggers.