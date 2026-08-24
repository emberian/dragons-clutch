// SPDX-License-Identifier: AGPL-3.0-or-later
//! Checked, capability-disabled wire contract for current Market Failure.
//!
//! Recovery78/v1 actions 10 through 13 retain their allocated coordinates,
//! but they no longer inherit the withdrawn caller-ID payloads or occurrence-
//! scoped `ExternalV2` account contract. This always-compiled module owns the
//! exact current ordered roles and the only hostile payload decoder. While the
//! central capability is false, [`process`] refuses before inspecting either.

use crate::accounts::{require, Outcome};
use crate::capabilities;
use crate::error::ClutchError;
use clutch_solana_layout::registry::{self, RecoveryAction};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Only paid advance accepts an operation parameter.
pub const FAILURE_MARKET_ADVANCE_PAYLOAD_BYTES_V2: usize = 8;

/// Semantic account roles for the complete current action family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureMarketAccountRoleV2 {
    MarketLifecycleRoot,
    SeriesMarketLink,
    FailureAdmissionRoot,
    FailureRuntimeRoot,
    FailureIntervalCell,
    FailureIntervalHistory,
    FailureMarketReplay,
    SeriesRegistry,
    RegistryProgram,
    RegistryProgramData,
    RegistryReleaseArtifact,
    CapabilityProfileArtifact,
    CompilerBundleArtifact,
    FundingQuoteArtifact,
    SeriesPlanArtifact,
    ProductTemplateArtifact,
    NativeClaimBasisArtifact,
    RecoveryPolicyArtifact,
    PriceMeasurePolicyArtifact,
    MarketGenesisArtifact,
    AttachmentPlanArtifact,
    MarketInstanceArtifact,
    SourceReleaseArtifact,
    SourceSpecArtifact,
    SourceOccurrence,
    SourceWindowArtifact,
    SourceStatisticKeyArtifact,
    SourceSummaryArtifact,
    SourceWindowSeal,
    SourceStatisticResult,
    SourceResultLineage,
    SourceHandoffReceipt,
    SourceWorkReceipt,
    FailureLivenessPolicy,
    FailureRecoveryCompartment,
    Keeper,
    RecoveryRefundOwner,
    NeutralSink,
    Realm,
    CollateralProfile,
    CollateralPolicyRelease,
    CollateralTokenProgram,
    GeneralMarketBinding,
    GeneralMarketRuntime,
    ResolutionV5,
    HoardV2,
    ClaimLedgerV3,
    SourceTerminalPolicy,
    SourceTerminalReceipt,
    SourceLivenessPolicy,
    SourceLivenessCompartment,
    SourcePayerRefund,
    SourceNeutralSink,
    SourceAccountPayer,
    RentSysvar,
    SystemProgram,
}

/// Exact privilege contract for one ordered role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureMarketAccountMetaV2 {
    pub role: FailureMarketAccountRoleV2,
    pub writable: bool,
    pub signer: bool,
    pub executable: bool,
}

const fn meta(
    role: FailureMarketAccountRoleV2,
    writable: bool,
    signer: bool,
    executable: bool,
) -> FailureMarketAccountMetaV2 {
    FailureMarketAccountMetaV2 {
        role,
        writable,
        signer,
        executable,
    }
}

use FailureMarketAccountRoleV2 as Role;

/// Begin reopens the current Product/Source graph and pins exactly one link.
pub const BEGIN_FAILURE_MARKET_SESSION_METAS_V2: &[FailureMarketAccountMetaV2] = &[
    meta(Role::MarketLifecycleRoot, false, false, false),
    meta(Role::SeriesMarketLink, true, false, false),
    meta(Role::FailureAdmissionRoot, false, false, false),
    meta(Role::FailureRuntimeRoot, true, false, false),
    meta(Role::FailureIntervalCell, true, false, false),
    meta(Role::FailureIntervalHistory, false, false, false),
    meta(Role::SeriesRegistry, false, false, false),
    meta(Role::RegistryProgram, false, false, true),
    meta(Role::RegistryProgramData, false, false, false),
    meta(Role::RegistryReleaseArtifact, false, false, false),
    meta(Role::CapabilityProfileArtifact, false, false, false),
    meta(Role::CompilerBundleArtifact, false, false, false),
    meta(Role::FundingQuoteArtifact, false, false, false),
    meta(Role::SeriesPlanArtifact, false, false, false),
    meta(Role::ProductTemplateArtifact, false, false, false),
    meta(Role::NativeClaimBasisArtifact, false, false, false),
    meta(Role::RecoveryPolicyArtifact, false, false, false),
    meta(Role::PriceMeasurePolicyArtifact, false, false, false),
    meta(Role::MarketGenesisArtifact, false, false, false),
    meta(Role::AttachmentPlanArtifact, false, false, false),
    meta(Role::MarketInstanceArtifact, false, false, false),
    meta(Role::SourceReleaseArtifact, false, false, false),
    meta(Role::SourceSpecArtifact, false, false, false),
    meta(Role::SourceOccurrence, false, false, false),
    meta(Role::SourceWindowArtifact, false, false, false),
    meta(Role::SourceStatisticKeyArtifact, false, false, false),
    meta(Role::SourceSummaryArtifact, false, false, false),
    meta(Role::SourceWindowSeal, false, false, false),
    meta(Role::SourceStatisticResult, false, false, false),
    meta(Role::SourceResultLineage, false, false, false),
    meta(Role::SourceHandoffReceipt, false, false, false),
    meta(Role::SourceWorkReceipt, false, false, false),
    meta(Role::FailureLivenessPolicy, false, false, false),
];

/// Paid advance mutates only the reusable cell, shared runtime, and Recovery.
pub const ADVANCE_FAILURE_MARKET_SESSION_METAS_V2: &[FailureMarketAccountMetaV2] = &[
    meta(Role::MarketLifecycleRoot, false, false, false),
    meta(Role::SeriesMarketLink, false, false, false),
    meta(Role::FailureAdmissionRoot, false, false, false),
    meta(Role::FailureRuntimeRoot, true, false, false),
    meta(Role::FailureIntervalCell, true, false, false),
    meta(Role::FailureIntervalHistory, false, false, false),
    meta(Role::SeriesRegistry, false, false, false),
    meta(Role::RegistryProgram, false, false, true),
    meta(Role::RegistryProgramData, false, false, false),
    meta(Role::RegistryReleaseArtifact, false, false, false),
    meta(Role::CapabilityProfileArtifact, false, false, false),
    meta(Role::CompilerBundleArtifact, false, false, false),
    meta(Role::SourceReleaseArtifact, false, false, false),
    meta(Role::SourceSpecArtifact, false, false, false),
    meta(Role::SourceOccurrence, false, false, false),
    meta(Role::SourceWindowArtifact, false, false, false),
    meta(Role::SourceStatisticKeyArtifact, false, false, false),
    meta(Role::SourceSummaryArtifact, false, false, false),
    meta(Role::SourceWindowSeal, false, false, false),
    meta(Role::SourceStatisticResult, false, false, false),
    meta(Role::SourceResultLineage, false, false, false),
    meta(Role::SourceHandoffReceipt, false, false, false),
    meta(Role::SourceWorkReceipt, false, false, false),
    meta(Role::FailureLivenessPolicy, false, false, false),
    meta(Role::FailureRecoveryCompartment, true, false, false),
    meta(Role::Keeper, true, false, false),
    meta(Role::RecoveryRefundOwner, true, false, false),
];

/// Successful resolve owns Resolution V5, Source terminalization, archive,
/// Recovery close, permanent replay, family seal, and Product consumption.
pub const RESOLVE_FAILURE_MARKET_SESSION_METAS_V2: &[FailureMarketAccountMetaV2] = &[
    meta(Role::MarketLifecycleRoot, true, false, false),
    meta(Role::SeriesMarketLink, true, false, false),
    meta(Role::FailureAdmissionRoot, false, false, false),
    meta(Role::FailureRuntimeRoot, true, false, false),
    meta(Role::FailureIntervalCell, true, false, false),
    meta(Role::FailureIntervalHistory, true, false, false),
    meta(Role::FailureMarketReplay, true, false, false),
    meta(Role::SeriesRegistry, false, false, false),
    meta(Role::RegistryProgram, false, false, true),
    meta(Role::RegistryProgramData, false, false, false),
    meta(Role::RegistryReleaseArtifact, false, false, false),
    meta(Role::CapabilityProfileArtifact, false, false, false),
    meta(Role::CompilerBundleArtifact, false, false, false),
    meta(Role::MarketInstanceArtifact, false, false, false),
    meta(Role::SourceReleaseArtifact, false, false, false),
    meta(Role::SourceSpecArtifact, false, false, false),
    meta(Role::SourceOccurrence, false, false, false),
    meta(Role::SourceWindowArtifact, false, false, false),
    meta(Role::SourceStatisticKeyArtifact, false, false, false),
    meta(Role::SourceSummaryArtifact, false, false, false),
    meta(Role::SourceWindowSeal, false, false, false),
    meta(Role::SourceStatisticResult, false, false, false),
    meta(Role::SourceResultLineage, false, false, false),
    meta(Role::SourceHandoffReceipt, false, false, false),
    meta(Role::SourceWorkReceipt, false, false, false),
    meta(Role::Realm, false, false, false),
    meta(Role::CollateralProfile, false, false, false),
    meta(Role::CollateralPolicyRelease, false, false, false),
    meta(Role::CollateralTokenProgram, false, false, true),
    meta(Role::GeneralMarketBinding, false, false, false),
    meta(Role::GeneralMarketRuntime, false, false, false),
    meta(Role::ResolutionV5, true, false, false),
    meta(Role::HoardV2, true, false, false),
    meta(Role::ClaimLedgerV3, true, false, false),
    meta(Role::SourceTerminalPolicy, true, false, false),
    meta(Role::SourceTerminalReceipt, true, false, false),
    meta(Role::SourceLivenessPolicy, false, false, false),
    meta(Role::SourceLivenessCompartment, true, false, false),
    meta(Role::SourcePayerRefund, true, false, false),
    meta(Role::SourceNeutralSink, true, false, false),
    meta(Role::SourceAccountPayer, true, true, false),
    meta(Role::FailureLivenessPolicy, false, false, false),
    meta(Role::FailureRecoveryCompartment, true, false, false),
    meta(Role::RecoveryRefundOwner, true, false, false),
    meta(Role::NeutralSink, true, false, false),
    meta(Role::RentSysvar, false, false, false),
    meta(Role::SystemProgram, false, false, true),
];

/// Exhausted sessions append and reset for reuse; they do not close Recovery.
pub const ARCHIVE_FAILURE_MARKET_SESSION_METAS_V2: &[FailureMarketAccountMetaV2] = &[
    meta(Role::MarketLifecycleRoot, false, false, false),
    meta(Role::SeriesMarketLink, true, false, false),
    meta(Role::FailureAdmissionRoot, false, false, false),
    meta(Role::FailureRuntimeRoot, true, false, false),
    meta(Role::FailureIntervalCell, true, false, false),
    meta(Role::FailureIntervalHistory, true, false, false),
];

/// Current caller-neutral payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureMarketActionPayloadV2 {
    Begin,
    Advance { requested_coordinates: u16 },
    Resolve,
    Archive,
}

/// Return the exact current account contract; legacy actions have none.
pub const fn account_metas_v2(
    action: RecoveryAction,
) -> Option<&'static [FailureMarketAccountMetaV2]> {
    match action {
        RecoveryAction::BeginIntervalConsensus => Some(BEGIN_FAILURE_MARKET_SESSION_METAS_V2),
        RecoveryAction::AdvanceIntervalConsensus => Some(ADVANCE_FAILURE_MARKET_SESSION_METAS_V2),
        RecoveryAction::ResolveIntervalConsensus => Some(RESOLVE_FAILURE_MARKET_SESSION_METAS_V2),
        RecoveryAction::CloseIntervalConsensusWork => {
            Some(ARCHIVE_FAILURE_MARKET_SESSION_METAS_V2)
        }
        RecoveryAction::InitializeFailureRoot
        | RecoveryAction::TriggerSourceFailure
        | RecoveryAction::TriggerRelationRefusal
        | RecoveryAction::AdvanceRecoverySchedule
        | RecoveryAction::AcceptRecoveryWork
        | RecoveryAction::ResolveCallerFunded
        | RecoveryAction::ResolvePaidRecovery
        | RecoveryAction::CloseRecoveryFunding
        | RecoveryAction::CloseFailureRoot => None,
    }
}

/// Decode only operation parameters. No authority ID is accepted from wire.
pub fn decode_payload_v2(
    action: RecoveryAction,
    payload: &[u8],
) -> Outcome<FailureMarketActionPayloadV2> {
    match action {
        RecoveryAction::BeginIntervalConsensus => {
            require(payload.is_empty(), ClutchError::NonCanonical)?;
            Ok(FailureMarketActionPayloadV2::Begin)
        }
        RecoveryAction::AdvanceIntervalConsensus => {
            require(
                payload.len() == FAILURE_MARKET_ADVANCE_PAYLOAD_BYTES_V2,
                ClutchError::NonCanonical,
            )?;
            require(
                payload[2..FAILURE_MARKET_ADVANCE_PAYLOAD_BYTES_V2]
                    .iter()
                    .all(|byte| *byte == 0),
                ClutchError::NonCanonical,
            )?;
            let requested_coordinates = u16::from_le_bytes([payload[0], payload[1]]);
            require(requested_coordinates != 0, ClutchError::NonCanonical)?;
            Ok(FailureMarketActionPayloadV2::Advance {
                requested_coordinates,
            })
        }
        RecoveryAction::ResolveIntervalConsensus => {
            require(payload.is_empty(), ClutchError::NonCanonical)?;
            Ok(FailureMarketActionPayloadV2::Resolve)
        }
        RecoveryAction::CloseIntervalConsensusWork => {
            require(payload.is_empty(), ClutchError::NonCanonical)?;
            Ok(FailureMarketActionPayloadV2::Archive)
        }
        RecoveryAction::InitializeFailureRoot
        | RecoveryAction::TriggerSourceFailure
        | RecoveryAction::TriggerRelationRefusal
        | RecoveryAction::AdvanceRecoverySchedule
        | RecoveryAction::AcceptRecoveryWork
        | RecoveryAction::ResolveCallerFunded
        | RecoveryAction::ResolvePaidRecovery
        | RecoveryAction::CloseRecoveryFunding
        | RecoveryAction::CloseFailureRoot => Err(ClutchError::UnsupportedInstruction.into()),
    }
}

/// Validate exact count and privileges after capability admission.
pub fn validate_account_contract_v2(
    action: RecoveryAction,
    accounts: &[AccountInfo<'_>],
) -> Outcome<()> {
    let Some(contract) = account_metas_v2(action) else {
        return Err(ClutchError::UnsupportedInstruction.into());
    };
    require(accounts.len() == contract.len(), ClutchError::WrongAccountCount)?;
    let mut index = 0usize;
    while index < contract.len() {
        let expected = contract[index];
        let observed = &accounts[index];
        require(
            observed.is_writable == expected.writable
                && observed.is_signer == expected.signer
                && observed.executable == expected.executable
                && *observed.key != Pubkey::default(),
            ClutchError::NonCanonical,
        )?;
        index += 1;
    }
    Ok(())
}

/// Exhaustive wire byte without an executable enum cast.
pub const fn recovery_action_byte_v2(action: RecoveryAction) -> u8 {
    match action {
        RecoveryAction::InitializeFailureRoot => 1,
        RecoveryAction::TriggerSourceFailure => 2,
        RecoveryAction::TriggerRelationRefusal => 3,
        RecoveryAction::AdvanceRecoverySchedule => 4,
        RecoveryAction::AcceptRecoveryWork => 5,
        RecoveryAction::ResolveCallerFunded => 6,
        RecoveryAction::ResolvePaidRecovery => 7,
        RecoveryAction::CloseRecoveryFunding => 8,
        RecoveryAction::CloseFailureRoot => 9,
        RecoveryAction::BeginIntervalConsensus => 10,
        RecoveryAction::AdvanceIntervalConsensus => 11,
        RecoveryAction::ResolveIntervalConsensus => 12,
        RecoveryAction::CloseIntervalConsensusWork => 13,
    }
}

/// Always-compiled dispatcher boundary. False capability refuses before the
/// payload decoder or any account field is touched.
#[inline(never)]
pub fn process(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: RecoveryAction,
    payload: &[u8],
) -> Outcome<()> {
    if !capabilities::extension_intent_action_enabled(
        registry::RECOVERY_FAMILY_TAG,
        registry::RECOVERY_FAMILY_VERSION,
        recovery_action_byte_v2(action),
    ) {
        return process_reserved_disabled(action);
    }
    require(sequence == 0, ClutchError::Replay)?;
    let _payload = decode_payload_v2(action, payload)?;
    validate_account_contract_v2(action, accounts)?;
    Err(ClutchError::UnsupportedInstruction.into())
}

/// Exhaustive refusal prevents newly allocated actions inheriting a route.
#[inline(never)]
pub fn process_reserved_disabled(action: RecoveryAction) -> Outcome<()> {
    match action {
        RecoveryAction::InitializeFailureRoot
        | RecoveryAction::TriggerSourceFailure
        | RecoveryAction::TriggerRelationRefusal
        | RecoveryAction::AdvanceRecoverySchedule
        | RecoveryAction::AcceptRecoveryWork
        | RecoveryAction::ResolveCallerFunded
        | RecoveryAction::ResolvePaidRecovery
        | RecoveryAction::CloseRecoveryFunding
        | RecoveryAction::CloseFailureRoot
        | RecoveryAction::BeginIntervalConsensus
        | RecoveryAction::AdvanceIntervalConsensus
        | RecoveryAction::ResolveIntervalConsensus
        | RecoveryAction::CloseIntervalConsensusWork => {
            Err(ClutchError::UnsupportedInstruction.into())
        }
    }
}

#[cfg(test)]
mod adversarial_contract_tests {
    use super::*;

    #[test]
    fn current_payloads_accept_no_caller_authority_ids() {
        assert_eq!(
            decode_payload_v2(RecoveryAction::BeginIntervalConsensus, &[]),
            Ok(FailureMarketActionPayloadV2::Begin)
        );
        assert_eq!(
            decode_payload_v2(RecoveryAction::ResolveIntervalConsensus, &[]),
            Ok(FailureMarketActionPayloadV2::Resolve)
        );
        assert_eq!(
            decode_payload_v2(RecoveryAction::CloseIntervalConsensusWork, &[]),
            Ok(FailureMarketActionPayloadV2::Archive)
        );
        assert!(decode_payload_v2(RecoveryAction::BeginIntervalConsensus, &[1]).is_err());
        assert!(decode_payload_v2(RecoveryAction::ResolveIntervalConsensus, &[1; 32]).is_err());
    }

    #[test]
    fn advance_rejects_zero_padding_and_authority_tail() {
        assert!(decode_payload_v2(
            RecoveryAction::AdvanceIntervalConsensus,
            &[1, 0, 0, 0, 0, 0, 0, 0]
        )
        .is_ok());
        assert!(decode_payload_v2(
            RecoveryAction::AdvanceIntervalConsensus,
            &[0, 0, 0, 0, 0, 0, 0, 0]
        )
        .is_err());
        assert!(decode_payload_v2(
            RecoveryAction::AdvanceIntervalConsensus,
            &[1, 0, 0, 0, 0, 0, 0, 1]
        )
        .is_err());
        assert!(decode_payload_v2(RecoveryAction::AdvanceIntervalConsensus, &[1; 40]).is_err());
    }

    #[test]
    fn withdrawn_legacy_actions_have_no_current_contract() {
        for action in [
            RecoveryAction::InitializeFailureRoot,
            RecoveryAction::TriggerSourceFailure,
            RecoveryAction::TriggerRelationRefusal,
            RecoveryAction::AdvanceRecoverySchedule,
            RecoveryAction::AcceptRecoveryWork,
            RecoveryAction::ResolveCallerFunded,
            RecoveryAction::ResolvePaidRecovery,
            RecoveryAction::CloseRecoveryFunding,
            RecoveryAction::CloseFailureRoot,
        ] {
            assert!(account_metas_v2(action).is_none());
            assert!(decode_payload_v2(action, &[]).is_err());
        }
    }

    #[test]
    fn capability_refusal_precedes_payload_and_account_access() {
        let source = include_str!("failure_market_dispatch_v2.rs");
        let process = source
            .split("pub fn process(")
            .nth(1)
            .and_then(|value| value.split("pub fn process_reserved_disabled").next())
            .expect("single dispatcher");
        let capability = process
            .find("extension_intent_action_enabled")
            .expect("capability check");
        let payload = process.find("decode_payload_v2").expect("payload decode");
        let accounts = process
            .find("validate_account_contract_v2")
            .expect("account validation");
        assert!(capability < payload && payload < accounts);
    }

    #[test]
    fn every_current_contract_has_unique_ordered_roles() {
        for action in [
            RecoveryAction::BeginIntervalConsensus,
            RecoveryAction::AdvanceIntervalConsensus,
            RecoveryAction::ResolveIntervalConsensus,
            RecoveryAction::CloseIntervalConsensusWork,
        ] {
            let contract = account_metas_v2(action).expect("current contract");
            assert!(!contract.is_empty());
            let mut index = 0usize;
            while index < contract.len() {
                let mut prior = 0usize;
                while prior < index {
                    assert_ne!(contract[index].role, contract[prior].role);
                    prior += 1;
                }
                index += 1;
            }
        }
    }

    #[test]
    fn disabled_current_actions_ignore_hostile_payloads_and_accounts() {
        for action in [
            RecoveryAction::BeginIntervalConsensus,
            RecoveryAction::AdvanceIntervalConsensus,
            RecoveryAction::ResolveIntervalConsensus,
            RecoveryAction::CloseIntervalConsensusWork,
        ] {
            assert!(!capabilities::extension_intent_action_enabled(
                registry::RECOVERY_FAMILY_TAG,
                registry::RECOVERY_FAMILY_VERSION,
                recovery_action_byte_v2(action),
            ));
            assert_eq!(
                process(&Pubkey::new_from_array([7; 32]), &[], u64::MAX, action, &[0xff; 41]),
                Err(ClutchError::UnsupportedInstruction.into()),
            );
        }
    }

    #[test]
    fn resolve_contract_contains_every_atomic_terminal_owner() {
        for role in [
            Role::MarketLifecycleRoot,
            Role::SeriesMarketLink,
            Role::FailureAdmissionRoot,
            Role::FailureRuntimeRoot,
            Role::FailureIntervalCell,
            Role::FailureIntervalHistory,
            Role::FailureMarketReplay,
            Role::ResolutionV5,
            Role::HoardV2,
            Role::ClaimLedgerV3,
            Role::SourceTerminalPolicy,
            Role::SourceTerminalReceipt,
            Role::SourceLivenessCompartment,
            Role::FailureRecoveryCompartment,
            Role::RecoveryRefundOwner,
            Role::NeutralSink,
        ] {
            assert!(RESOLVE_FAILURE_MARKET_SESSION_METAS_V2
                .iter()
                .any(|meta| meta.role == role));
        }
    }

    #[test]
    fn current_contracts_bind_only_fresh_market_roles() {
        let source = include_str!("failure_market_dispatch_v2.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source");
        assert!(!production.contains("FailureRecoveryPayloadV1"));
        assert!(!production.contains("ExternalRecoveryStateV1"));
        assert!(!production.contains("failure_replay_tombstone"));
        assert!(production.contains("FailureMarketReplay"));
        assert!(production.contains("FailureIntervalHistory"));
    }
}
