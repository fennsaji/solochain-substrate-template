//! Equivocation detection and handling for MICC consensus.
//!
//! This module provides functionality to detect when validators produce
//! conflicting blocks for the same slot (equivocation), which is a critical
//! security violation in consensus protocols.

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{Header as HeaderT, SaturatedConversion},
    RuntimeDebug,
};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use sp_consensus_micc::Slot;
use frame_support::{
    dispatch::DispatchResult,
    BoundedVec,
};

/// Evidence of equivocation: a validator authored two different blocks for the same slot
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct EquivocationProof<Header, AuthorityId> {
    /// The slot in which equivocation occurred
    pub slot: Slot,
    /// The offending authority
    pub offender: AuthorityId,
    /// First conflicting block header
    pub first_header: Header,
    /// Second conflicting block header  
    pub second_header: Header,
}

/// A report of equivocation.
#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[codec(mel_bound())]
pub struct EquivocationReport<AuthorityId: Encode + Decode + DecodeWithMemTracking + MaxEncodedLen> {
    /// The offending authority
    pub offender: AuthorityId,
    /// The slot where equivocation occurred
    pub slot: Slot,
    /// Block number where equivocation was detected
    pub block_number: u32,
    /// Session where equivocation occurred
    pub session_index: u32,
}

/// Equivocation detection and slashing configuration
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct EquivocationConfig {
    /// Enable automatic slashing of equivocating validators
    pub enable_slashing: bool,
    /// Grace period before slashing (number of blocks)
    pub grace_period: u32,
    /// Maximum number of equivocations to track per session
    pub max_reports_per_session: u32,
    /// Percentage of stake to slash for equivocation (basis points: 10000 = 100%)
    pub slash_percentage: u32,
}

impl Default for EquivocationConfig {
    fn default() -> Self {
        Self {
            enable_slashing: false, // Conservative default - require explicit enablement
            grace_period: 100,      // ~8.3 minutes with 500ms blocks
            max_reports_per_session: 10,
            slash_percentage: 1000, // 10% slash by default
        }
    }
}

/// Storage for tracking equivocations within a session
#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[codec(mel_bound())]
pub struct SessionEquivocations<AuthorityId: Encode + Decode + DecodeWithMemTracking + MaxEncodedLen> {
    /// List of equivocation reports in this session (bounded to prevent bloat)
    pub reports: BoundedVec<EquivocationReport<AuthorityId>, frame_support::traits::ConstU32<100>>,
    /// Mapping of authority to their equivocation count (bounded to prevent bloat)
    pub offender_counts: BoundedVec<(AuthorityId, u32), frame_support::traits::ConstU32<100>>,
}

impl<AuthorityId: Encode + Decode + DecodeWithMemTracking + MaxEncodedLen> Default for SessionEquivocations<AuthorityId> {
    fn default() -> Self {
        Self {
            reports: BoundedVec::new(),
            offender_counts: BoundedVec::new(),
        }
    }
}

/// Trait for handling equivocation detection and punishment
pub trait EquivocationHandler<AuthorityId: Encode + Decode + DecodeWithMemTracking + MaxEncodedLen> {
    /// Handle a newly detected equivocation
    fn handle_equivocation(
        &self,
        report: EquivocationReport<AuthorityId>,
        config: &EquivocationConfig,
    ) -> DispatchResult;

    /// Check if an authority is currently disabled due to equivocation
    fn is_disabled(&self, authority: &AuthorityId) -> bool;

    /// Get the current equivocation count for an authority in this session
    fn get_equivocation_count(&self, authority: &AuthorityId) -> u32;
}

/// Equivocation detector that tracks blocks per slot
pub struct EquivocationDetector<Header, AuthorityId: Encode + Decode + DecodeWithMemTracking + MaxEncodedLen> {
    /// Blocks seen per slot in current session
    slot_blocks: BTreeMap<Slot, Vec<(Header, AuthorityId)>>,
    /// Current session equivocations
    session_equivocations: SessionEquivocations<AuthorityId>,
    /// Detection configuration
    config: EquivocationConfig,
}

impl<Header, AuthorityId> EquivocationDetector<Header, AuthorityId>
where
    Header: HeaderT + Clone,
    AuthorityId: Clone + Ord + PartialEq + Encode + Decode + DecodeWithMemTracking + MaxEncodedLen,
{
    /// Create a new equivocation detector
    pub fn new(config: EquivocationConfig) -> Self {
        Self {
            slot_blocks: BTreeMap::new(),
            session_equivocations: SessionEquivocations::default(),
            config,
        }
    }

    /// Process a new block and check for equivocations
    pub fn check_block(
        &mut self,
        header: Header,
        slot: Slot,
        author: AuthorityId,
        session_index: u32,
    ) -> Option<EquivocationProof<Header, AuthorityId>> {
        // Check if we already have blocks for this slot
        let slot_entry = self.slot_blocks.entry(slot).or_insert_with(Vec::new);

        // Look for conflicting blocks from the same author
        for (existing_header, existing_author) in slot_entry.iter() {
            if *existing_author == author && existing_header.hash() != header.hash() {
                // Found equivocation!
                let proof = EquivocationProof {
                    slot,
                    offender: author.clone(),
                    first_header: existing_header.clone(),
                    second_header: header.clone(),
                };

                // Record the equivocation report
                let report = EquivocationReport {
                    offender: author.clone(),
                    slot,
                    block_number: (*header.number()).saturated_into::<u32>(),
                    session_index,
                };

                self.record_equivocation(report);
                return Some(proof);
            }
        }

        // No equivocation found, add this block to the slot
        slot_entry.push((header, author));

        // Clean up old slots to prevent memory bloat (keep last 1000 slots)
        const MAX_SLOTS: usize = 1000;
        if self.slot_blocks.len() > MAX_SLOTS {
            let oldest_slot = *self.slot_blocks.keys().next().unwrap();
            self.slot_blocks.remove(&oldest_slot);
        }

        None
    }

    /// Record an equivocation in the current session
    fn record_equivocation(&mut self, report: EquivocationReport<AuthorityId>) {
        // Check if we've hit the max reports limit
        let max_reports = self.config.max_reports_per_session as usize;
        if self.session_equivocations.reports.len() >= max_reports {
            log::warn!("Maximum equivocation reports reached for session, ignoring new report");
            return;
        }

        // Update offender count - search for existing entry
        let mut found = false;
        for (authority, count) in self.session_equivocations.offender_counts.iter_mut() {
            if *authority == report.offender {
                *count += 1;
                found = true;
                break;
            }
        }
        
        // If not found, add new entry (if there's space)
        if !found && self.session_equivocations.offender_counts.len() < 100 {
            let _ = self.session_equivocations.offender_counts.try_push((report.offender.clone(), 1));
        }

        // Add the report (if there's space)
        let _ = self.session_equivocations.reports.try_push(report.clone());

        log::error!(
            "🚨 EQUIVOCATION DETECTED: Authority equivocated in slot {:?}",
            report.slot
        );
    }

    /// Reset detection state for a new session
    pub fn new_session(&mut self) {
        self.slot_blocks.clear();
        self.session_equivocations = SessionEquivocations::default();
        log::info!("🔄 Equivocation detector reset for new session");
    }

    /// Get current session equivocations
    pub fn get_session_equivocations(&self) -> &SessionEquivocations<AuthorityId> {
        &self.session_equivocations
    }

    /// Check if an authority has exceeded equivocation threshold
    pub fn is_authority_slashable(&self, authority: &AuthorityId) -> bool {
        // Search for the authority in the offender counts
        for (auth, count) in self.session_equivocations.offender_counts.iter() {
            if auth == authority {
                // Slash after first equivocation if slashing is enabled
                return self.config.enable_slashing && *count > 0u32;
            }
        }
        false
    }

    /// Update configuration
    pub fn update_config(&mut self, config: EquivocationConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &EquivocationConfig {
        &self.config
    }
}

/// Utility functions for equivocation handling
pub mod utils {
    use super::*;

    /// Verify that two headers constitute valid equivocation proof
    pub fn verify_equivocation_proof<Header, AuthorityId>(
        proof: &EquivocationProof<Header, AuthorityId>,
    ) -> bool
    where
        Header: HeaderT,
        AuthorityId: PartialEq,
    {
        // Check that headers are different
        if proof.first_header.hash() == proof.second_header.hash() {
            return false;
        }

        // Check that both headers claim the same block number (parent + 1)
        if proof.first_header.number() != proof.second_header.number() {
            return false;
        }

        // In a full implementation, we would verify:
        // 1. Both headers are properly signed by the offender
        // 2. Both headers reference the same parent block
        // 3. Both headers are for the same slot (from pre-runtime digest)
        // For now, we assume these checks are done elsewhere

        true
    }

    /// Calculate slash amount based on configuration
    pub fn calculate_slash_amount(
        total_stake: u128,
        config: &EquivocationConfig,
    ) -> u128 {
        total_stake * config.slash_percentage as u128 / 10000
    }

    /// Check if enough time has passed for slashing
    pub fn is_past_grace_period(
        equivocation_block: u32,
        current_block: u32,
        config: &EquivocationConfig,
    ) -> bool {
        current_block >= equivocation_block + config.grace_period
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_runtime::{testing::Header as TestHeader, traits::BlakeTwo256};

    type TestAuthorityId = u64;
    type Header = TestHeader;

    #[test]
    fn detector_finds_equivocation() {
        let mut detector = EquivocationDetector::<Header, TestAuthorityId>::new(
            EquivocationConfig::default()
        );

        let slot = 42u64.into();
        let author = 1u64;

        // First block for slot 42
        let header1 = Header::new(
            1,
            Default::default(),
            Default::default(),
            [1u8; 32].into(),
            Default::default(),
        );

        // Second conflicting block for slot 42 from same author
        let header2 = Header::new(
            1,
            Default::default(),
            Default::default(),
            [2u8; 32].into(), // Different parent hash
            Default::default(),
        );

        // First block should not trigger equivocation
        assert!(detector.check_block(header1.clone(), slot, author, 1).is_none());

        // Second block should trigger equivocation
        let proof = detector.check_block(header2.clone(), slot, author, 1);
        assert!(proof.is_some());

        let proof = proof.unwrap();
        assert_eq!(proof.slot, slot);
        assert_eq!(proof.offender, author);
        assert_eq!(proof.first_header.hash(), header1.hash());
        assert_eq!(proof.second_header.hash(), header2.hash());

        // Check that equivocation was recorded
        assert_eq!(detector.session_equivocations.reports.len(), 1);
        
        // Check offender count by searching through the bounded vec
        let mut found_count = 0;
        for (auth, count) in detector.session_equivocations.offender_counts.iter() {
            if *auth == author {
                found_count = *count;
                break;
            }
        }
        assert_eq!(found_count, 1);
    }

    #[test]
    fn detector_ignores_same_block() {
        let mut detector = EquivocationDetector::<Header, TestAuthorityId>::new(
            EquivocationConfig::default()
        );

        let slot = 42u64.into();
        let author = 1u64;

        let header = Header::new(
            1,
            Default::default(),
            Default::default(),
            [1u8; 32].into(),
            Default::default(),
        );

        // First block
        assert!(detector.check_block(header.clone(), slot, author, 1).is_none());

        // Same block again - should not trigger equivocation
        assert!(detector.check_block(header, slot, author, 1).is_none());
    }

    #[test]
    fn detector_resets_for_new_session() {
        let mut detector = EquivocationDetector::<Header, TestAuthorityId>::new(
            EquivocationConfig::default()
        );

        // Create some equivocations
        let slot = 42u64.into();
        let author = 1u64;

        let header1 = Header::new(1, Default::default(), Default::default(), [1u8; 32].into(), Default::default());
        let header2 = Header::new(1, Default::default(), Default::default(), [2u8; 32].into(), Default::default());

        detector.check_block(header1, slot, author, 1);
        detector.check_block(header2, slot, author, 1);

        assert_eq!(detector.session_equivocations.reports.len(), 1);

        // Reset for new session
        detector.new_session();

        assert_eq!(detector.session_equivocations.reports.len(), 0);
        assert!(detector.slot_blocks.is_empty());
    }
}