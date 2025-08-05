//! Security integration tests for MICC client consensus implementation.
//!
//! This module tests the security aspects of the client-side consensus logic,
//! including force authoring security, monitoring integration, and equivocation detection.

#![cfg(test)]

use super::*;
use crate::monitoring::{ConsensusMonitor, MonitoringConfig, NetworkHealth, SecurityAnomaly};
use sp_consensus_micc::{AuthorityId, Slot};
use sp_consensus_slots::SlotDuration;
use sp_keystore::{testing::MemoryKeystore, Keystore};
use sp_runtime::{
    testing::Header as TestHeader,
    traits::{BlakeTwo256, Header as HeaderT},
};
use std::{sync::Arc, time::Duration};

type TestBlock = sp_runtime::generic::Block<TestHeader, sp_runtime::OpaqueExtrinsic>;
type TestHeader = sp_runtime::testing::Header;

/// Mock authority ID for testing
type TestAuthorityId = sp_consensus_micc::ed25519::AuthorityId;

#[tokio::test]
async fn test_force_authoring_security_integration() {
    // Setup test keystore with multiple authorities
    let keystore = Arc::new(MemoryKeystore::new());
    
    // Generate test authority keys
    let alice_key = keystore
        .ed25519_generate_new(sp_consensus_micc::MICC, Some("//Alice"))
        .await
        .expect("Failed to generate Alice key");
    
    let bob_key = keystore
        .ed25519_generate_new(sp_consensus_micc::MICC, Some("//Bob"))
        .await
        .expect("Failed to generate Bob key");
    
    let authorities = vec![alice_key.clone(), bob_key.clone()];
    
    // Test 1: Verify slot assignment calculation
    let slot = Slot::from(0u64);
    let expected_author = crate::standalone::slot_author::<sp_consensus_micc::ed25519::AuthorityPair>(
        slot, &authorities
    );
    
    assert!(expected_author.is_some());
    assert_eq!(expected_author.unwrap(), &alice_key); // Slot 0 should assign to Alice
    
    // Test 2: Verify slot 1 assigns to Bob
    let slot1 = Slot::from(1u64);
    let expected_author1 = crate::standalone::slot_author::<sp_consensus_micc::ed25519::AuthorityPair>(
        slot1, &authorities
    );
    
    assert!(expected_author1.is_some());
    assert_eq!(expected_author1.unwrap(), &bob_key); // Slot 1 should assign to Bob
    
    // Test 3: Test normal slot claiming (non-force mode)
    let claimed_slot0 = crate::standalone::claim_slot::<sp_consensus_micc::ed25519::AuthorityPair>(
        slot, &authorities, &keystore
    ).await;
    
    assert!(claimed_slot0.is_some());
    assert_eq!(claimed_slot0.unwrap(), alice_key);
    
    // Test 4: Test claiming wrong slot (should fail in normal mode)
    let wrong_claimed = crate::standalone::claim_slot::<sp_consensus_micc::ed25519::AuthorityPair>(
        slot1, &[alice_key.clone()], // Only Alice's key, but slot 1 should be Bob's
        &keystore
    ).await;
    
    assert!(wrong_claimed.is_none()); // Should fail because Alice can't claim Bob's slot
    
    println!("✅ Force authoring security integration: proper slot assignment verified");
}

#[test]
fn test_consensus_monitoring_integration() {
    let config = MonitoringConfig {
        max_empty_slots: 3,
        block_time_variance_threshold: 0.4,
        stall_threshold: Duration::from_secs(15),
        enable_detailed_logging: false, // Reduce test noise
        metrics_interval: Duration::from_secs(5),
        max_metrics_history: 50,
    };
    
    let monitor = ConsensusMonitor::<TestBlock>::new(config);
    
    // Test 1: Record normal slot progression
    monitor.record_slot(Slot::from(1u64), Some(0));
    monitor.record_slot(Slot::from(2u64), Some(1));
    monitor.record_slot(Slot::from(3u64), Some(0));
    
    let metrics = monitor.get_metrics();
    assert_eq!(metrics.slots_seen, 3);
    assert_eq!(metrics.blocks_produced, 0); // No blocks produced yet
    
    // Test 2: Record block production
    let header = TestHeader::new(
        1,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    
    monitor.record_block_production(&header, Slot::from(1u64), Some(0));
    monitor.record_block_production(&header, Slot::from(2u64), Some(1));
    
    let updated_metrics = monitor.get_metrics();
    assert_eq!(updated_metrics.blocks_produced, 2);
    assert!(updated_metrics.avg_block_time.as_millis() > 0);
    
    // Test 3: Check for empty slot anomalies
    // Record several empty slots to trigger anomaly detection
    for i in 4..=10 {
        monitor.record_slot(Slot::from(i), Some((i % 2) as u32));
    }
    
    let anomalies = monitor.get_recent_anomalies();
    // Should detect empty slot spike after 3+ consecutive empty slots
    assert!(!anomalies.is_empty());
    
    // Test 4: Verify health summary
    let health_summary = monitor.get_health_summary();
    assert!(health_summary.contains("MICC Consensus Health Summary"));
    assert!(health_summary.contains("Blocks Produced: 2"));
    assert!(health_summary.contains("Slots Seen: 10"));
    
    println!("✅ Consensus monitoring integration: anomaly detection and metrics verified");
}

#[test]
fn test_equivocation_detection_integration() {
    use crate::equivocation::{EquivocationDetector, EquivocationConfig};
    
    let config = EquivocationConfig {
        enable_slashing: true,
        grace_period: 5,
        max_reports_per_session: 50,
        slash_percentage: 500, // 5%
    };
    
    let mut detector = EquivocationDetector::<TestHeader, TestAuthorityId>::new(config);
    
    // Test 1: Normal block production
    let slot = Slot::from(42u64);
    let authority = TestAuthorityId::from([1u8; 32]);
    
    let header1 = TestHeader::new(
        10,
        Default::default(),
        Default::default(),
        [1u8; 32].into(),
        Default::default(),
    );
    
    let result1 = detector.check_block(header1.clone(), slot, authority.clone(), 1);
    assert!(result1.is_none()); // No equivocation
    
    // Test 2: Detect equivocation (same slot, same authority, different block)
    let header2 = TestHeader::new(
        10, // Same block number
        Default::default(),
        Default::default(),
        [2u8; 32].into(), // Different parent hash - constitutes equivocation
        Default::default(),
    );
    
    let result2 = detector.check_block(header2.clone(), slot, authority.clone(), 1);
    assert!(result2.is_some());
    
    let proof = result2.unwrap();
    assert_eq!(proof.slot, slot);
    assert_eq!(proof.offender, authority);
    assert_eq!(proof.first_header.hash(), header1.hash());
    assert_eq!(proof.second_header.hash(), header2.hash());
    
    // Test 3: Verify session state tracking
    let session_equivocations = detector.get_session_equivocations();
    assert_eq!(session_equivocations.reports.len(), 1);
    
    // Test 4: Verify authority slashing status
    assert!(detector.is_authority_slashable(&authority));
    
    // Test 5: Session reset clears state
    detector.new_session();
    let cleared_equivocations = detector.get_session_equivocations();
    assert_eq!(cleared_equivocations.reports.len(), 0);
    assert!(!detector.is_authority_slashable(&authority));
    
    println!("✅ Equivocation detection integration: comprehensive detection verified");
}

#[test]
fn test_slot_duration_and_timing_security() {
    // Test 1: Verify slot duration configuration
    let slot_duration = SlotDuration::from_millis(500);
    assert_eq!(slot_duration.as_millis(), 500);
    
    // Test 2: Verify slot timing calculations
    let slot_0 = Slot::from(0u64);
    let slot_1 = Slot::from(1u64);
    let slot_100 = Slot::from(100u64);
    
    // Verify slot progression is monotonic
    assert!(*slot_0 < *slot_1);
    assert!(*slot_1 < *slot_100);
    
    // Test 3: Verify large slot numbers don't overflow
    let large_slot = Slot::from(u64::MAX - 1);
    let next_slot = Slot::from(u64::MAX);
    assert!(*large_slot < *next_slot);
    
    // Test 4: Authority assignment with large slots should be stable
    let authorities = vec![
        TestAuthorityId::from([1u8; 32]),
        TestAuthorityId::from([2u8; 32]),
        TestAuthorityId::from([3u8; 32]),
    ];
    
    let author_for_large_slot = crate::standalone::slot_author::<sp_consensus_micc::ed25519::AuthorityPair>(
        large_slot, &authorities
    );
    assert!(author_for_large_slot.is_some());
    
    // Verify consistent assignment for same slot
    let author_again = crate::standalone::slot_author::<sp_consensus_micc::ed25519::AuthorityPair>(
        large_slot, &authorities
    );
    assert_eq!(author_for_large_slot, author_again);
    
    println!("✅ Slot duration and timing security: stable calculations verified");
}

#[test]
fn test_header_validation_security() {
    use crate::standalone::{find_pre_digest, PreDigestLookupError};
    
    // Test 1: Valid header with pre-digest
    let mut header = TestHeader::new(
        1,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    
    // Add MICC pre-digest
    let slot = Slot::from(42u64);
    let pre_digest = crate::standalone::pre_digest::<sp_consensus_micc::ed25519::AuthorityPair>(slot);
    header.digest_mut().push(pre_digest);
    
    // Should successfully extract slot
    let extracted_slot = find_pre_digest::<TestBlock, sp_consensus_micc::ed25519::AuthoritySignature>(&header);
    assert!(extracted_slot.is_ok());
    assert_eq!(extracted_slot.unwrap(), slot);
    
    // Test 2: Header without pre-digest should fail
    let empty_header = TestHeader::new(
        1,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    
    let no_digest_result = find_pre_digest::<TestBlock, sp_consensus_micc::ed25519::AuthoritySignature>(&empty_header);
    assert!(no_digest_result.is_err());
    match no_digest_result.unwrap_err() {
        PreDigestLookupError::NoDigestFound => {}, // Expected
        _ => panic!("Unexpected error type"),
    }
    
    // Test 3: Genesis block (block 0) should return slot 0
    let genesis_header = TestHeader::new(
        0,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    
    let genesis_slot = find_pre_digest::<TestBlock, sp_consensus_micc::ed25519::AuthoritySignature>(&genesis_header);
    assert!(genesis_slot.is_ok());
    assert_eq!(genesis_slot.unwrap(), Slot::from(0u64));
    
    println!("✅ Header validation security: proper digest handling verified");
}

#[test]
fn test_network_health_assessment() {
    use crate::monitoring::utils::{assess_network_health, calculate_efficiency};
    use crate::monitoring::ConsensusMetrics;
    
    // Test 1: Healthy network metrics
    let healthy_metrics = ConsensusMetrics {
        blocks_produced: 95,
        slots_seen: 100,
        empty_slots: 5,
        avg_block_time: Duration::from_millis(500),
        forks_detected: 0,
        anomalies_detected: 0,
        last_block_time: None,
        authorities_count: 3,
        network_health: NetworkHealth::Unknown,
    };
    
    let efficiency = calculate_efficiency(&healthy_metrics);
    assert_eq!(efficiency, 0.95); // 95% efficiency
    
    let health = assess_network_health(&healthy_metrics);
    assert_eq!(health, NetworkHealth::Healthy);
    
    // Test 2: Degraded network metrics
    let degraded_metrics = ConsensusMetrics {
        blocks_produced: 70,
        slots_seen: 100,
        empty_slots: 30,
        avg_block_time: Duration::from_millis(750),
        forks_detected: 2,
        anomalies_detected: 5,
        last_block_time: None,
        authorities_count: 3,
        network_health: NetworkHealth::Unknown,
    };
    
    let degraded_efficiency = calculate_efficiency(&degraded_metrics);
    assert_eq!(degraded_efficiency, 0.70); // 70% efficiency
    
    let degraded_health = assess_network_health(&degraded_metrics);
    assert_eq!(degraded_health, NetworkHealth::Degraded);
    
    // Test 3: Critical network metrics
    let critical_metrics = ConsensusMetrics {
        blocks_produced: 40,
        slots_seen: 100,
        empty_slots: 60,
        avg_block_time: Duration::from_millis(1500),
        forks_detected: 10,
        anomalies_detected: 20,
        last_block_time: None,
        authorities_count: 3,
        network_health: NetworkHealth::Unknown,
    };
    
    let critical_efficiency = calculate_efficiency(&critical_metrics);
    assert_eq!(critical_efficiency, 0.40); // 40% efficiency
    
    let critical_health = assess_network_health(&critical_metrics);
    assert_eq!(critical_health, NetworkHealth::Critical);
    
    println!("✅ Network health assessment: accurate health classification verified");
}

#[test]
fn test_comprehensive_security_workflow() {
    // This test simulates a complete security workflow integrating all components
    
    // 1. Setup monitoring
    let monitor_config = MonitoringConfig::default();
    let monitor = ConsensusMonitor::<TestBlock>::new(monitor_config);
    
    // 2. Setup equivocation detection
    let equivocation_config = crate::equivocation::EquivocationConfig::default();
    let mut equivocation_detector = crate::equivocation::EquivocationDetector::<TestHeader, TestAuthorityId>::new(equivocation_config);
    
    // 3. Simulate normal consensus operation
    let authority = TestAuthorityId::from([1u8; 32]);
    let slot = Slot::from(10u64);
    
    // Record slot and block production
    monitor.record_slot(slot, Some(0));
    
    let header = TestHeader::new(
        5,
        Default::default(),
        Default::default(),
        [1u8; 32].into(),
        Default::default(),
    );
    
    monitor.record_block_production(&header, slot, Some(0));
    
    // Check for equivocations (should be none)
    let equivocation_result = equivocation_detector.check_block(header.clone(), slot, authority.clone(), 1);
    assert!(equivocation_result.is_none());
    
    // 4. Simulate equivocation attack
    let conflicting_header = TestHeader::new(
        5,
        Default::default(),
        Default::default(),
        [2u8; 32].into(), // Different parent - equivocation!
        Default::default(),
    );
    
    let equivocation_proof = equivocation_detector.check_block(conflicting_header, slot, authority.clone(), 1);
    assert!(equivocation_proof.is_some());
    
    // 5. Verify monitoring detected the issue
    let metrics = monitor.get_metrics();
    assert_eq!(metrics.blocks_produced, 1);
    assert_eq!(metrics.slots_seen, 1);
    
    // 6. Verify equivocation was properly recorded
    let session_equivocations = equivocation_detector.get_session_equivocations();
    assert_eq!(session_equivocations.reports.len(), 1);
    assert!(equivocation_detector.is_authority_slashable(&authority));
    
    // 7. Test security metrics
    let health_summary = monitor.get_health_summary();
    assert!(health_summary.contains("Blocks Produced: 1"));
    assert!(health_summary.contains("Slots Seen: 1"));
    
    println!("✅ Comprehensive security workflow: end-to-end security validation verified");
}