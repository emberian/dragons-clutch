//! Chain-derived Core effects for the complete funded Resolution lifecycle.
//!
//! The builders treat the Registry activation cache, current Loader V3
//! deployments, finalized material/manifest/Product graph, Source state,
//! physical funding custody, and terminal certificate as authorities. They
//! emit only unsigned permissionless instructions.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Shared finalized Product-graph authentication used by successor operators.
pub mod product_graph_observation_v3;

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingCustodyObservationV1,
    FundingStateV1, FundingStatus,
};
use dclutch_market_core_codec::{
    Action, CapabilityFundingHeaderV1, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState,
    Identity, Phase, Readiness, Request, Role,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    RESOLUTION_CONTROLLER_RELEASE_ID_V4, ResolutionCertificateKindV2, ResolutionCertificateV2,
    ResolutionCoreActionV1, ResolutionCoreReceiptKindV1, ResolutionRoleRequestV1,
    SOURCE_CLOSURE_RECEIPT_BYTES_V2, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
    SOURCE_FUNDING_SET_DIGEST_DOMAIN_V1, SourceClosureReceiptV2,
};
use dclutch_source_contract::{
    RECOVERY_POLICY_BYTES_V2, RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SOURCE_RESOLUTION_STATE_BYTES_V2,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceMaterialV2, SourceResolutionPhaseV1,
    SourceResolutionStateV2,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use product_graph_observation_v3::{
    FinalizedProductGraphAccountsV3, authenticate_product_graph_observation_v3,
};

/// An immutable finality label supplied with an observation report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Finality {
    /// Observed at processed commitment.
    Processed,
    /// Observed at confirmed commitment.
    Confirmed,
    /// Observed at finalized commitment.
    Finalized,
}

/// Slot, wall-clock time, and finality attached to an account observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Observation {
    /// Observed slot.
    pub slot: u64,
    /// Observed Unix time.
    pub unix_timestamp: i64,
    /// Commitment/finality label.
    pub finality: Finality,
}

/// Host-observed account metadata and exact bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccount {
    /// Observation provenance.
    pub observation: Observation,
    /// Account address.
    pub key: Pubkey,
    /// Program owner.
    pub owner: Pubkey,
    /// Observed lamports.
    pub lamports: u64,
    /// Observed executable bit.
    pub executable: bool,
    /// Exact account bytes.
    pub data: Vec<u8>,
}

/// Exact account count consumed by Core and Resolution for terminal admission.
pub const RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3: usize = 24;
/// Exact account count consumed by Core and Resolution for funding creation.
pub const RESOLUTION_CREATE_FUND_ACCOUNT_COUNT_V3: usize = 20;
/// Exact account count consumed by Core and Resolution for funding readiness.
pub const RESOLUTION_VERIFY_FUND_ACCOUNT_COUNT_V3: usize = 21;

/// Same-finalized state selecting dust-tolerant Source/funding creation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionCreateFundSnapshotV3 {
    /// Founding Core Market.
    pub market: ObservedAccount,
    /// Market-selected Registry activation cache.
    pub activation_cache: ObservedAccount,
    /// Current Registry program.
    pub registry_program: ObservedAccount,
    /// Current Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current Resolution program.
    pub resolution_program: ObservedAccount,
    /// Current Resolution ProgramData.
    pub resolution_programdata: ObservedAccount,
    /// Finalized SourceMaterial record.
    pub source_material: ObservedAccount,
    /// Vacant SourceMaterial staging cursor.
    pub source_material_staging: ObservedAccount,
    /// Finalized capability manifest.
    pub capability_manifest: ObservedAccount,
    /// Vacant capability-manifest staging cursor.
    pub capability_manifest_staging: ObservedAccount,
    /// Vacant canonical Source PDA; harmless System-owned dust is admitted.
    pub source_destination: ObservedAccount,
    /// Vacant canonical recovery-funding PDA.
    pub recovery_destination: ObservedAccount,
    /// Vacant canonical exhaustion-funding PDA.
    pub exhaustion_destination: ObservedAccount,
    /// Vacant canonical explicit-failure-funding PDA.
    pub failure_destination: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Finalized RecoveryPolicy record.
    pub recovery_policy: ObservedAccount,
    /// Vacant RecoveryPolicy staging cursor.
    pub recovery_policy_staging: ObservedAccount,
}

/// Exact unsigned Core CreateFund report plus required pre-execution top-ups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionCreateFundReportV3 {
    /// Permissionless unsigned Core instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical Core caller-authority PDA.
    pub caller_authority: Pubkey,
    /// Immutable Market rent beneficiary carried in the role request.
    pub beneficiary: Pubkey,
    /// Chain-derived recovery/exhaustion/failure manifest indices.
    pub funding_entry_indices: [u16; 3],
    /// Minimum Source-PDA top-up after harmless existing dust.
    pub source_top_up_lamports: u64,
    /// Exact ordered recovery/exhaustion/failure top-ups.
    pub funding_top_up_lamports: [u64; 3],
    /// Digest of the exact funded role request.
    pub role_request_digest: [u8; 32],
}

/// Same-finalized state selecting activation of the exact three pending Funds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionVerifyFundReadySnapshotV3 {
    /// Founding Core Market.
    pub market: ObservedAccount,
    /// Market-selected Registry activation cache.
    pub activation_cache: ObservedAccount,
    /// Current Registry program.
    pub registry_program: ObservedAccount,
    /// Current Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current Resolution program.
    pub resolution_program: ObservedAccount,
    /// Current Resolution ProgramData.
    pub resolution_programdata: ObservedAccount,
    /// Finalized SourceMaterial record.
    pub source_material: ObservedAccount,
    /// Vacant SourceMaterial staging cursor.
    pub source_material_staging: ObservedAccount,
    /// Finalized capability manifest.
    pub capability_manifest: ObservedAccount,
    /// Vacant capability-manifest staging cursor.
    pub capability_manifest_staging: ObservedAccount,
    /// Primary Source state created by CreateFund.
    pub source_state: ObservedAccount,
    /// Pending recovery funding ledger and custody.
    pub recovery_funding: ObservedAccount,
    /// Pending exhaustion funding ledger and custody.
    pub exhaustion_funding: ObservedAccount,
    /// Pending explicit-failure funding ledger and custody.
    pub failure_funding: ObservedAccount,
    /// Immutable Market rent beneficiary receiving activation debits.
    pub beneficiary: ObservedAccount,
    /// Canonical Clock sysvar selecting activation slot.
    pub clock_sysvar: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Finalized RecoveryPolicy record.
    pub recovery_policy: ObservedAccount,
    /// Vacant RecoveryPolicy staging cursor.
    pub recovery_policy_staging: ObservedAccount,
}

/// Exact unsigned Core VerifyFundReady report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionVerifyFundReadyReportV3 {
    /// Permissionless unsigned Core instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting every input.
    pub observation: Observation,
    /// Canonical Core caller-authority PDA.
    pub caller_authority: Pubkey,
    /// Immutable Market rent beneficiary receiving activation debits.
    pub beneficiary: Pubkey,
    /// Chain-derived recovery/exhaustion/failure manifest indices.
    pub funding_entry_indices: [u16; 3],
    /// Authenticated positive activation slot.
    pub activation_slot: u64,
    /// Exact Rent+Creation lamports atomically returned to the beneficiary.
    pub expected_beneficiary_credit_lamports: u64,
    /// Digest of the exact funded role request.
    pub role_request_digest: [u8; 32],
}

/// Same-finalized chain state selecting one terminal Resolution admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionAdmitTerminalSnapshotV3 {
    /// Mutable Core Market in Open/Consumed state.
    pub market: ObservedAccount,
    /// Registry-owned activation cache selected immutably by the Market.
    pub activation_cache: ObservedAccount,
    /// Current executable Registry program.
    pub registry_program: ObservedAccount,
    /// Current executable Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current executable Resolution program.
    pub resolution_program: ObservedAccount,
    /// Current Resolution ProgramData.
    pub resolution_programdata: ObservedAccount,
    /// Finalized SourceMaterialV2 record selected by Market.
    pub source_material: ObservedAccount,
    /// Vacant SourceMaterial staging cursor.
    pub source_material_staging: ObservedAccount,
    /// Finalized capability manifest selected by Market.
    pub capability_manifest: ObservedAccount,
    /// Vacant capability-manifest staging cursor.
    pub capability_manifest_staging: ObservedAccount,
    /// Terminal canonical Source state written by real provider execution.
    pub source_state: ObservedAccount,
    /// Active recovery funding compartment.
    pub recovery_funding: ObservedAccount,
    /// Active exhaustion funding compartment.
    pub exhaustion_funding: ObservedAccount,
    /// Active explicit-failure funding compartment.
    pub failure_funding: ObservedAccount,
    /// Terminal Resolution certificate produced with Source state.
    pub certificate: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Finalized Product record.
    pub product_raw: ObservedAccount,
    /// Vacant Product staging cursor.
    pub product_staging: ObservedAccount,
    /// Finalized ResultDomain record.
    pub result_domain_raw: ObservedAccount,
    /// Vacant ResultDomain staging cursor.
    pub result_domain_staging: ObservedAccount,
    /// Finalized Portfolio record.
    pub portfolio_raw: ObservedAccount,
    /// Vacant Portfolio staging cursor.
    pub portfolio_staging: ObservedAccount,
}

/// Exact unsigned Core terminal-admission report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionAdmitTerminalReportV3 {
    /// Permissionless unsigned Core instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every semantic account.
    pub observation: Observation,
    /// Canonical Core caller-authority PDA.
    pub caller_authority: Pubkey,
    /// Exact terminal sequence projected from Source and certificate.
    pub terminal_sequence: u64,
    /// Product-authenticated terminal selector.
    pub selector: u32,
    /// Product-authenticated outcome count.
    pub outcome_count: u32,
    /// SHA-256 of the exact role-owned request bytes.
    pub role_request_digest: [u8; 32],
}

/// Same-finalized state selecting one atomic Resolution-fund close.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionCloseFundSnapshotV3 {
    /// Mutable Core Market in Retiring/Consumed state.
    pub market: ObservedAccount,
    /// Registry-owned release activation selected by Market.
    pub activation_cache: ObservedAccount,
    /// Current executable Registry program.
    pub registry_program: ObservedAccount,
    /// Current executable Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current executable Resolution program.
    pub resolution_program: ObservedAccount,
    /// Current Resolution ProgramData.
    pub resolution_programdata: ObservedAccount,
    /// Finalized SourceMaterialV2 record selected by Market.
    pub source_material: ObservedAccount,
    /// Vacant SourceMaterial staging cursor.
    pub source_material_staging: ObservedAccount,
    /// Finalized capability manifest selected by Market.
    pub capability_manifest: ObservedAccount,
    /// Vacant capability-manifest staging cursor.
    pub capability_manifest_staging: ObservedAccount,
    /// Admitted terminal Source state to discharge.
    pub source_state: ObservedAccount,
    /// Active recovery funding compartment.
    pub recovery_funding: ObservedAccount,
    /// Active exhaustion funding compartment.
    pub exhaustion_funding: ObservedAccount,
    /// Active explicit-failure funding compartment.
    pub failure_funding: ObservedAccount,
    /// Core-admitted terminal Resolution certificate.
    pub certificate: ObservedAccount,
    /// Prepaid vacant canonical Source closure receipt PDA.
    pub closure_destination: ObservedAccount,
    /// Immutable Market/Source rent beneficiary receiving every discharge.
    pub beneficiary: ObservedAccount,
    /// Canonical Clock sysvar selecting close time.
    pub clock_sysvar: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Finalized RecoveryPolicyV2 record selected by SourceMaterial.
    pub recovery_policy: ObservedAccount,
    /// Vacant RecoveryPolicy staging cursor.
    pub recovery_policy_staging: ObservedAccount,
}

/// Exact unsigned Core CloseFund report and the receipt facts it will commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionCloseFundReportV3 {
    /// Permissionless unsigned Core instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting every semantic account.
    pub observation: Observation,
    /// Canonical Core caller-authority PDA.
    pub caller_authority: Pubkey,
    /// Canonical closure receipt PDA.
    pub closure_receipt: Pubkey,
    /// Terminal sequence preserved in the receipt.
    pub terminal_sequence: u64,
    /// Close replay sequence, exactly terminal sequence plus one.
    pub closure_sequence: u64,
    /// Exact lamports discharged from Source and all three funds.
    pub expected_refund_lamports: u64,
    /// SHA-256 of the exact role-owned request bytes.
    pub role_request_digest: [u8; 32],
    /// Exact typed facts the post-close retirement waist must consume.
    pub expected_retirement_facts: ResolutionRetirementReceiptFactsV3,
}

/// Resolution-owned facts consumed by the joined retirement waist.
///
/// Claims and Custody receipts must reproduce `market`, `generation`,
/// `terminal_certificate`, `beneficiary`, `selector`, and `terminal_sequence`.
/// The three prestate digests prevent a closure receipt from another Source or
/// funding set from completing this Market's retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionRetirementReceiptFactsV3 {
    /// Canonical Market account.
    pub market: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Canonical Resolution closure receipt account.
    pub resolution_closure_receipt: [u8; 32],
    /// Discharged canonical Source state account.
    pub source_state: [u8; 32],
    /// Finalized SourceMaterial identity.
    pub source_material: [u8; 32],
    /// Finalized capability-manifest identity.
    pub capability_manifest: [u8; 32],
    /// Core-admitted terminal certificate.
    pub terminal_certificate: [u8; 32],
    /// Immutable beneficiary of every returned lamport.
    pub beneficiary: [u8; 32],
    /// Product-native terminal selector.
    pub selector: u32,
    /// Original terminal replay sequence.
    pub terminal_sequence: u64,
    /// Digest of the terminal Source prestate.
    pub source_state_digest: [u8; 32],
    /// Digest of the admitted terminal certificate.
    pub terminal_certificate_digest: [u8; 32],
    /// Digest of the ordered recovery/exhaustion/failure funding prestates.
    pub funding_set_digest: [u8; 32],
    /// Exact Source and funding lamports discharged.
    pub refund_lamports: u64,
    /// Authenticated Clock timestamp at atomic close.
    pub closed_at: u64,
}

/// Stable refusal from terminal-chain authentication or instruction assembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionCoreOperatorErrorV3 {
    /// Inputs were not one finalized snapshot.
    Snapshot,
    /// Market state or its immutable coordinates differed.
    Market,
    /// Registry cache or a current Loader deployment differed.
    Release,
    /// A finalized record coordinate or Product graph differed.
    Record,
    /// Source state, certificate, selector, or terminal sequence differed.
    Terminal,
    /// Funding state, manifest binding, or physical custody differed.
    Funding,
    /// A derived address, account alias, or privilege profile differed.
    Frame,
    /// Fixed-width request construction refused.
    Encoding,
}

/// Construct the canonical Core `CreateFund` effect from finalized chain state.
///
/// Funding destinations may contain System-owned dust below their exact target;
/// the returned top-ups are intended to be composed before this instruction in
/// the same transaction. Surplus funding-destination lamports are refused
/// because the current funding ledger has no pre-creation donation category.
pub fn build_resolution_create_fund_v3(
    snapshot: &ResolutionCreateFundSnapshotV3,
) -> Result<ResolutionCreateFundReportV3, ResolutionCoreOperatorErrorV3> {
    let observation = same_finalized_create_observation(snapshot)?;
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Market)?;
    authenticate_founding_market(
        &snapshot.market,
        &snapshot.registry_program,
        &snapshot.core_program,
        market,
    )?;
    authenticate_release_coordinates(
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        market,
    )?;
    let rent =
        decode_rent(&snapshot.rent_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    authenticate_system(&snapshot.system_program)?;
    let (material, recovery_policy, entries) = authenticate_founding_records(
        snapshot.registry_program.key,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        &snapshot.recovery_policy,
        &snapshot.recovery_policy_staging,
        market,
        &rent,
    )?;
    let manifest = CapabilityManifestV1::decode(&snapshot.capability_manifest.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let manifest_id = CapabilityContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;

    let (expected_source, _) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            snapshot.market.key.as_ref(),
            &market.identity.generation.to_le_bytes(),
        ],
        &snapshot.resolution_program.key,
    );
    authenticate_vacant_destination(&snapshot.source_destination, expected_source)?;
    let source_target = rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES_V2);
    let source_top_up_lamports = source_target.saturating_sub(snapshot.source_destination.lamports);

    let destinations = [
        &snapshot.recovery_destination,
        &snapshot.exhaustion_destination,
        &snapshot.failure_destination,
    ];
    let mut funding_top_up_lamports = [0_u64; 3];
    for (slot, (destination, entry_index)) in destinations.into_iter().zip(entries).enumerate() {
        let (funding, top_up) =
            prospective_pending_funding(destination, manifest_id, manifest, entry_index, &rent)?;
        let derivation = CapabilityFundingDerivationV1::new(
            snapshot.market.key.to_bytes(),
            market.identity.generation,
            manifest_id,
            manifest,
            funding,
        )
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        if Pubkey::find_program_address(
            &derivation.seed_components(),
            &snapshot.resolution_program.key,
        )
        .0 != destination.key
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        *funding_top_up_lamports
            .get_mut(slot)
            .ok_or(ResolutionCoreOperatorErrorV3::Funding)? = top_up;
    }
    // Keep these decoded authorities live in the builder rather than accepting
    // caller-selected entry coordinates.
    if material.recovery_policy().is_none() || recovery_policy.attempt_count() != 1 {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }
    let role_request = funding_role_request(
        ResolutionCoreActionV1::CreateFund,
        snapshot.source_destination.key,
        market,
        [
            snapshot.recovery_destination.key,
            snapshot.exhaustion_destination.key,
            snapshot.failure_destination.key,
        ],
        entries,
    );
    let (role_bytes, role_request_digest) = encode_funding_role_request(role_request)?;
    let caller_authority = core_caller_authority(
        market,
        snapshot.core_program.key,
        snapshot.source_destination.key,
        role_request_digest,
    )?;
    let instruction = assemble_funding_instruction(
        ResolutionCoreActionV1::CreateFund,
        market,
        &snapshot.market.data,
        snapshot.core_program.key,
        snapshot.source_destination.key,
        caller_authority,
        role_request_digest,
        role_bytes,
        create_accounts(snapshot, caller_authority),
    )?;
    validate_funding_frame(
        &instruction,
        ResolutionCoreActionV1::CreateFund,
        caller_authority,
        snapshot.source_destination.key,
        market.rent_beneficiary.to_bytes(),
    )?;
    Ok(ResolutionCreateFundReportV3 {
        instruction,
        observation,
        caller_authority,
        beneficiary: Pubkey::new_from_array(market.rent_beneficiary.to_bytes()),
        funding_entry_indices: entries,
        source_top_up_lamports,
        funding_top_up_lamports,
        role_request_digest,
    })
}

/// Construct the canonical Core `VerifyFundReady` effect from three pending
/// funding ledgers and their exact physical custody.
pub fn build_resolution_verify_fund_ready_v3(
    snapshot: &ResolutionVerifyFundReadySnapshotV3,
) -> Result<ResolutionVerifyFundReadyReportV3, ResolutionCoreOperatorErrorV3> {
    let observation = same_finalized_verify_observation(snapshot)?;
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Market)?;
    authenticate_founding_market(
        &snapshot.market,
        &snapshot.registry_program,
        &snapshot.core_program,
        market,
    )?;
    authenticate_release_coordinates(
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        market,
    )?;
    let rent =
        decode_rent(&snapshot.rent_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let clock =
        decode_clock(&snapshot.clock_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if clock.slot == 0
        || snapshot.beneficiary.key.to_bytes() != market.rent_beneficiary.to_bytes()
        || snapshot.beneficiary.executable
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let (_, _, entries) = authenticate_founding_records(
        snapshot.registry_program.key,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        &snapshot.recovery_policy,
        &snapshot.recovery_policy_staging,
        market,
        &rent,
    )?;
    authenticate_primary_source(snapshot, market, &rent)?;
    let manifest = CapabilityManifestV1::decode(&snapshot.capability_manifest.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let manifest_id = CapabilityContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let accounts = [
        &snapshot.recovery_funding,
        &snapshot.exhaustion_funding,
        &snapshot.failure_funding,
    ];
    let mut expected_beneficiary_credit_lamports = 0_u64;
    for (account, entry_index) in accounts.into_iter().zip(entries) {
        let mut funding = authenticate_pending_funding(
            snapshot.market.key,
            snapshot.resolution_program.key,
            account,
            market.identity.generation,
            manifest_id,
            manifest,
            entry_index,
            &rent,
        )?;
        let custody = FundingCustodyObservationV1::native_only(
            account.lamports,
            rent.minimum_balance(FUNDING_STATE_BYTES),
        )
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        let debit = funding
            .activate(manifest_id, manifest, custody, clock.slot)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        expected_beneficiary_credit_lamports = expected_beneficiary_credit_lamports
            .checked_add(debit.rent_lamports())
            .and_then(|value| value.checked_add(debit.creation_lamports()))
            .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
        let post_lamports = account
            .lamports
            .checked_sub(
                debit
                    .rent_lamports()
                    .checked_add(debit.creation_lamports())
                    .ok_or(ResolutionCoreOperatorErrorV3::Funding)?,
            )
            .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
        funding
            .validate_against(
                manifest_id,
                manifest,
                FundingCustodyObservationV1::native_only(
                    post_lamports,
                    rent.minimum_balance(FUNDING_STATE_BYTES),
                )
                .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?,
            )
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    }
    let role_request = funding_role_request(
        ResolutionCoreActionV1::VerifyFundReady,
        snapshot.source_state.key,
        market,
        [
            snapshot.recovery_funding.key,
            snapshot.exhaustion_funding.key,
            snapshot.failure_funding.key,
        ],
        entries,
    );
    let (role_bytes, role_request_digest) = encode_funding_role_request(role_request)?;
    let caller_authority = core_caller_authority(
        market,
        snapshot.core_program.key,
        snapshot.source_state.key,
        role_request_digest,
    )?;
    let instruction = assemble_funding_instruction(
        ResolutionCoreActionV1::VerifyFundReady,
        market,
        &snapshot.market.data,
        snapshot.core_program.key,
        snapshot.source_state.key,
        caller_authority,
        role_request_digest,
        role_bytes,
        verify_accounts(snapshot, caller_authority),
    )?;
    validate_funding_frame(
        &instruction,
        ResolutionCoreActionV1::VerifyFundReady,
        caller_authority,
        snapshot.source_state.key,
        snapshot.beneficiary.key.to_bytes(),
    )?;
    Ok(ResolutionVerifyFundReadyReportV3 {
        instruction,
        observation,
        caller_authority,
        beneficiary: snapshot.beneficiary.key,
        funding_entry_indices: entries,
        activation_slot: clock.slot,
        expected_beneficiary_credit_lamports,
        role_request_digest,
    })
}

/// Construct the one canonical Core `AdmitTerminal` instruction from chain state.
pub fn build_resolution_admit_terminal_v3(
    snapshot: &ResolutionAdmitTerminalSnapshotV3,
) -> Result<ResolutionAdmitTerminalReportV3, ResolutionCoreOperatorErrorV3> {
    let observation = same_finalized_observation(snapshot)?;
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Market)?;
    if snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.executable
        || snapshot.market.key.to_bytes() != market.identity.market_id.to_bytes()
        || market.phase != Phase::Open
        || market.readiness != Readiness::Consumed
        || snapshot.registry_program.key.to_bytes() != market.identity.registry_program.to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Market);
    }
    authenticate_release_graph(snapshot, market)?;
    let rent =
        decode_rent(&snapshot.rent_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        market.identity.resolution_policy.to_bytes(),
        &rent,
    )?;
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market.identity.capability_manifest.to_bytes(),
        &rent,
    )?;
    let product = authenticate_product_graph_observation_v3(FinalizedProductGraphAccountsV3 {
        registry_program: snapshot.registry_program.key,
        product_raw: &snapshot.product_raw,
        product_staging: &snapshot.product_staging,
        domain_raw: &snapshot.result_domain_raw,
        domain_staging: &snapshot.result_domain_staging,
        portfolio_raw: &snapshot.portfolio_raw,
        portfolio_staging: &snapshot.portfolio_staging,
    })
    .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if product.product_record != market.identity.product_record.to_bytes() {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }

    let source = SourceResolutionStateV2::decode(&snapshot.source_state.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    authenticate_source(snapshot, market, source)?;
    let decision = source
        .decision(product.outcome_count)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    let (receipt_kind, certificate_kind, kind_tag) = match source.phase() {
        SourceResolutionPhaseV1::Resolved => (
            ResolutionCoreReceiptKindV1::TerminalSuccess,
            ResolutionCertificateKindV2::ResolutionSuccess,
            1_u8,
        ),
        SourceResolutionPhaseV1::FailureCommitted => (
            ResolutionCoreReceiptKindV1::TerminalFailure,
            ResolutionCertificateKindV2::ResolutionFailure,
            4_u8,
        ),
        _ => return Err(ResolutionCoreOperatorErrorV3::Terminal),
    };
    let certificate = ResolutionCertificateV2::decode(&snapshot.certificate.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    if snapshot.certificate.owner != snapshot.resolution_program.key
        || snapshot.certificate.executable
        || snapshot.certificate.data.len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || !rent.is_exempt(
            snapshot.certificate.lamports,
            snapshot.certificate.data.len(),
        )
        || certificate.kind != certificate_kind
        || certificate.market != snapshot.market.key.to_bytes()
        || certificate.source_material != market.identity.resolution_policy.to_bytes()
        || certificate.product_record_digest != product.product_record
        || certificate.receipt_account != snapshot.certificate.key.to_bytes()
        || certificate.generation != market.identity.generation
        || certificate.selector != decision.selector()
        || certificate
            .validate_terminal_product(product.product_record, product.outcome_count)
            .is_err()
    {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }
    let sequence = decision.terminal_sequence();
    let expected_certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            snapshot.source_state.key.as_ref(),
            &[kind_tag],
            &sequence.to_le_bytes(),
        ],
        &snapshot.resolution_program.key,
    )
    .0;
    if snapshot.certificate.key != expected_certificate {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }
    let entries = authenticate_funding(snapshot, market, &rent)?;
    let role_request = ResolutionRoleRequestV1 {
        action: ResolutionCoreActionV1::AdmitTerminal,
        receipt_kind,
        source_state: snapshot.source_state.key.to_bytes(),
        source_material: market.identity.resolution_policy.to_bytes(),
        capability_manifest: market.identity.capability_manifest.to_bytes(),
        recovery_funding: snapshot.recovery_funding.key.to_bytes(),
        exhaustion_funding: snapshot.exhaustion_funding.key.to_bytes(),
        failure_funding: snapshot.failure_funding.key.to_bytes(),
        receipt: snapshot.certificate.key.to_bytes(),
        beneficiary: [0; 32],
        recovery_entry_index: entries[0],
        exhaustion_entry_index: entries[1],
        failure_entry_index: entries[2],
        receipt_sequence: sequence,
    };
    let role_body = role_request
        .to_bytes()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let header = CapabilityFundingHeaderV1::new(3)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?
        .encode();
    let mut role_bytes = Vec::with_capacity(header.len() + role_body.len());
    role_bytes.extend_from_slice(&header);
    role_bytes.extend_from_slice(&role_body);
    let role_request_digest = hash(&role_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        market.identity.selected_release_set.to_bytes(),
        snapshot.market.key.to_bytes(),
        ExecutionRoleV1::Core,
        snapshot.source_state.key.to_bytes(),
        role_request_digest,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let caller_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &snapshot.core_program.key).0;
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::AdmitTerminal,
        Role::Resolution,
        identity(snapshot.core_program.key.to_bytes())?,
        identity(caller_authority.to_bytes())?,
        market.identity.selected_release_set,
        market.identity.market_id,
        identity(snapshot.source_state.key.to_bytes())?,
        identity(hash(&snapshot.market.data).to_bytes())?,
        identity(role_request_digest)?,
        market.identity.generation,
        sequence,
        1,
        u32::try_from(role_bytes.len()).map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let request = Request::administrative(
        Action::AdmitTerminal,
        market.identity.generation,
        market.identity.market_id,
    );
    let mut data = Vec::new();
    data.extend_from_slice(
        &request
            .encode()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    );
    data.extend_from_slice(
        &envelope
            .encode()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    );
    data.extend_from_slice(&role_bytes);
    let accounts = admit_accounts(snapshot, caller_authority);
    if !exact_admit_frame(&accounts, snapshot, caller_authority) {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(ResolutionAdmitTerminalReportV3 {
        instruction: Instruction {
            program_id: snapshot.core_program.key,
            accounts,
            data,
        },
        observation,
        caller_authority,
        terminal_sequence: sequence,
        selector: decision.selector(),
        outcome_count: product.outcome_count,
        role_request_digest,
    })
}

/// Construct the canonical Core `CloseFund` instruction from admitted state.
pub fn build_resolution_close_fund_v3(
    snapshot: &ResolutionCloseFundSnapshotV3,
) -> Result<ResolutionCloseFundReportV3, ResolutionCoreOperatorErrorV3> {
    let observation = same_finalized_close_observation(snapshot)?;
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Market)?;
    if snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.executable
        || snapshot.market.key.to_bytes() != market.identity.market_id.to_bytes()
        || market.phase != Phase::Retiring
        || market.readiness != Readiness::Consumed
        || snapshot.registry_program.key.to_bytes() != market.identity.registry_program.to_bytes()
        || snapshot.beneficiary.key.to_bytes() != market.rent_beneficiary.to_bytes()
        || snapshot.beneficiary.executable
    {
        return Err(ResolutionCoreOperatorErrorV3::Market);
    }
    authenticate_close_release_graph(snapshot, market)?;
    let rent =
        decode_rent(&snapshot.rent_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let clock =
        decode_clock(&snapshot.clock_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if clock.unix_timestamp <= 0
        || snapshot.system_program.key != system_program::ID
        || snapshot.system_program.owner != native_loader::ID
        || !snapshot.system_program.executable
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        market.identity.resolution_policy.to_bytes(),
        &rent,
    )?;
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market.identity.capability_manifest.to_bytes(),
        &rent,
    )?;
    let material = SourceMaterialV2::decode(&snapshot.source_material.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let recovery_policy_id = material
        .recovery_policy()
        .ok_or(ResolutionCoreOperatorErrorV3::Record)?;
    authenticate_finalized_record(
        snapshot.registry_program.key,
        &snapshot.recovery_policy,
        &snapshot.recovery_policy_staging,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        recovery_policy_id.to_bytes(),
        &rent,
    )?;
    if snapshot.recovery_policy.data.len() != RECOVERY_POLICY_BYTES_V2 {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }
    let recovery_policy = RecoveryPolicyV2::decode(&snapshot.recovery_policy.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if recovery_policy.attempt_count() != 1 {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }

    let source = SourceResolutionStateV2::decode(&snapshot.source_state.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    authenticate_close_source(snapshot, market, source)?;
    let terminal = source
        .terminal_projection()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    if terminal.selector() != market.terminal_winner
        || source.rent_beneficiary() != market.rent_beneficiary.to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }
    let mut retired = source;
    retired
        .retire(market.identity.generation, clock.unix_timestamp, 1, 1)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    let terminal_sequence = terminal.terminal_sequence();
    let closure_sequence = terminal_sequence
        .checked_add(1)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let certificate = ResolutionCertificateV2::decode(&snapshot.certificate.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    if snapshot.certificate.owner != snapshot.resolution_program.key
        || snapshot.certificate.executable
        || snapshot.certificate.data.len() != RESOLUTION_CERTIFICATE_BYTES_V2
        || !rent.is_exempt(
            snapshot.certificate.lamports,
            snapshot.certificate.data.len(),
        )
        || snapshot.certificate.key.to_bytes()
            != market
                .terminal_receipt
                .ok_or(ResolutionCoreOperatorErrorV3::Terminal)?
                .to_bytes()
        || certificate.market != snapshot.market.key.to_bytes()
        || certificate.source_material != market.identity.resolution_policy.to_bytes()
        || certificate.product_record_digest != market.identity.product_record.to_bytes()
        || certificate.receipt_account != snapshot.certificate.key.to_bytes()
        || certificate.generation != market.identity.generation
        || certificate.selector != terminal.selector()
        || certificate
            .validate_admitted_terminal(
                market.identity.product_record.to_bytes(),
                market.terminal_winner,
            )
            .is_err()
    {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }
    let kind_tag = match certificate.kind {
        ResolutionCertificateKindV2::ResolutionSuccess => 1_u8,
        ResolutionCertificateKindV2::ResolutionFailure => 4_u8,
        ResolutionCertificateKindV2::RecoveryAdvanced | ResolutionCertificateKindV2::Exhausted => {
            return Err(ResolutionCoreOperatorErrorV3::Terminal);
        }
    };
    let expected_certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            snapshot.source_state.key.as_ref(),
            &[kind_tag],
            &terminal_sequence.to_le_bytes(),
        ],
        &snapshot.resolution_program.key,
    )
    .0;
    if snapshot.certificate.key != expected_certificate
        || snapshot.source_state.lamports
            < rent.minimum_balance(dclutch_source_contract::SOURCE_RESOLUTION_STATE_BYTES_V2)
    {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }

    let (entries, expected_refund_lamports) =
        authenticate_close_funding(snapshot, market, material, recovery_policy, &rent)?;
    let closure_receipt = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
            snapshot.source_state.key.as_ref(),
            &closure_sequence.to_le_bytes(),
        ],
        &snapshot.resolution_program.key,
    )
    .0;
    if snapshot.closure_destination.key != closure_receipt
        || snapshot.closure_destination.owner != system_program::ID
        || snapshot.closure_destination.executable
        || !snapshot.closure_destination.data.is_empty()
        || snapshot.closure_destination.lamports
            < rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V2)
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let source_state_digest = hash(&snapshot.source_state.data).to_bytes();
    let terminal_certificate_digest = hash(&snapshot.certificate.data).to_bytes();
    let funding_set_digest = hashv(&[
        SOURCE_FUNDING_SET_DIGEST_DOMAIN_V1,
        &snapshot.recovery_funding.data,
        &snapshot.exhaustion_funding.data,
        &snapshot.failure_funding.data,
    ])
    .to_bytes();
    let expected_retirement_facts = ResolutionRetirementReceiptFactsV3 {
        market: snapshot.market.key.to_bytes(),
        generation: market.identity.generation,
        resolution_closure_receipt: closure_receipt.to_bytes(),
        source_state: snapshot.source_state.key.to_bytes(),
        source_material: market.identity.resolution_policy.to_bytes(),
        capability_manifest: market.identity.capability_manifest.to_bytes(),
        terminal_certificate: snapshot.certificate.key.to_bytes(),
        beneficiary: snapshot.beneficiary.key.to_bytes(),
        selector: terminal.selector(),
        terminal_sequence,
        source_state_digest,
        terminal_certificate_digest,
        funding_set_digest,
        refund_lamports: expected_refund_lamports,
        closed_at: u64::try_from(clock.unix_timestamp)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?,
    };
    let role_request = ResolutionRoleRequestV1 {
        action: ResolutionCoreActionV1::CloseFund,
        receipt_kind: ResolutionCoreReceiptKindV1::Closure,
        source_state: snapshot.source_state.key.to_bytes(),
        source_material: market.identity.resolution_policy.to_bytes(),
        capability_manifest: market.identity.capability_manifest.to_bytes(),
        recovery_funding: snapshot.recovery_funding.key.to_bytes(),
        exhaustion_funding: snapshot.exhaustion_funding.key.to_bytes(),
        failure_funding: snapshot.failure_funding.key.to_bytes(),
        receipt: closure_receipt.to_bytes(),
        beneficiary: snapshot.beneficiary.key.to_bytes(),
        recovery_entry_index: entries[0],
        exhaustion_entry_index: entries[1],
        failure_entry_index: entries[2],
        receipt_sequence: closure_sequence,
    };
    let role_body = role_request
        .to_bytes()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let header = CapabilityFundingHeaderV1::new(3)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?
        .encode();
    let mut role_bytes = Vec::with_capacity(header.len() + role_body.len());
    role_bytes.extend_from_slice(&header);
    role_bytes.extend_from_slice(&role_body);
    let role_request_digest = hash(&role_bytes).to_bytes();
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        market.identity.selected_release_set.to_bytes(),
        snapshot.market.key.to_bytes(),
        ExecutionRoleV1::Core,
        snapshot.source_state.key.to_bytes(),
        role_request_digest,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let caller_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &snapshot.core_program.key).0;
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::CloseFund,
        Role::Resolution,
        identity(snapshot.core_program.key.to_bytes())?,
        identity(caller_authority.to_bytes())?,
        market.identity.selected_release_set,
        market.identity.market_id,
        identity(snapshot.source_state.key.to_bytes())?,
        identity(hash(&snapshot.market.data).to_bytes())?,
        identity(role_request_digest)?,
        market.identity.generation,
        terminal_sequence,
        1,
        u32::try_from(role_bytes.len()).map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let request = Request::administrative(
        Action::Retire,
        market.identity.generation,
        market.identity.market_id,
    );
    let mut data = Vec::new();
    data.extend_from_slice(
        &request
            .encode()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    );
    data.extend_from_slice(
        &envelope
            .encode()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    );
    data.extend_from_slice(&role_bytes);
    let accounts = close_accounts(snapshot, caller_authority);
    if !exact_close_frame(&accounts, snapshot, caller_authority) {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let report = ResolutionCloseFundReportV3 {
        instruction: Instruction {
            program_id: snapshot.core_program.key,
            accounts,
            data,
        },
        observation,
        caller_authority,
        closure_receipt,
        terminal_sequence,
        closure_sequence,
        expected_refund_lamports,
        role_request_digest,
        expected_retirement_facts,
    };
    validate_resolution_close_fund_report_v3(&report)?;
    Ok(report)
}

/// Authenticate the persisted Resolution closure into retirement-waist facts.
pub fn authenticate_resolution_retirement_receipt_v3(
    receipt: &ObservedAccount,
    rent_sysvar: &ObservedAccount,
    resolution_program: Pubkey,
    expected: ResolutionRetirementReceiptFactsV3,
) -> Result<ResolutionRetirementReceiptFactsV3, ResolutionCoreOperatorErrorV3> {
    if receipt.observation != rent_sysvar.observation
        || receipt.observation.finality != Finality::Finalized
        || receipt.owner != resolution_program
        || receipt.executable
        || receipt.data.len() != SOURCE_CLOSURE_RECEIPT_BYTES_V2
    {
        return Err(ResolutionCoreOperatorErrorV3::Snapshot);
    }
    let rent = decode_rent(rent_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if !rent.is_exempt(receipt.lamports, receipt.data.len()) {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let decoded = SourceClosureReceiptV2::decode(&receipt.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    let closure_sequence = decoded
        .terminal_sequence
        .checked_add(1)
        .ok_or(ResolutionCoreOperatorErrorV3::Terminal)?;
    let expected_key = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
            &decoded.source_state,
            &closure_sequence.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;
    let observed = ResolutionRetirementReceiptFactsV3 {
        market: decoded.market,
        generation: decoded.generation,
        resolution_closure_receipt: decoded.receipt_account,
        source_state: decoded.source_state,
        source_material: decoded.source_material,
        capability_manifest: decoded.capability_manifest,
        terminal_certificate: decoded.terminal_certificate,
        beneficiary: decoded.beneficiary,
        selector: decoded.selector,
        terminal_sequence: decoded.terminal_sequence,
        source_state_digest: decoded.source_state_digest,
        terminal_certificate_digest: decoded.terminal_certificate_digest,
        funding_set_digest: decoded.funding_set_digest,
        refund_lamports: decoded.refund_lamports,
        closed_at: decoded.closed_at,
    };
    if receipt.key != expected_key
        || receipt.key.to_bytes() != decoded.receipt_account
        || observed != expected
    {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }
    Ok(observed)
}

fn authenticate_founding_market(
    market_account: &ObservedAccount,
    registry_program: &ObservedAccount,
    core_program: &ObservedAccount,
    market: CoreState,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    if market_account.owner != core_program.key
        || market_account.executable
        || market_account.key.to_bytes() != market.identity.market_id.to_bytes()
        || market.phase != Phase::Founding
        || market.readiness != Readiness::Prepaid
        || registry_program.key.to_bytes() != market.identity.registry_program.to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Market);
    }
    Ok(())
}

fn authenticate_system(system: &ObservedAccount) -> Result<(), ResolutionCoreOperatorErrorV3> {
    if system.key != system_program::ID || system.owner != native_loader::ID || !system.executable {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_founding_records(
    registry: Pubkey,
    source_material: &ObservedAccount,
    source_material_staging: &ObservedAccount,
    capability_manifest: &ObservedAccount,
    capability_manifest_staging: &ObservedAccount,
    recovery_policy: &ObservedAccount,
    recovery_policy_staging: &ObservedAccount,
    market: CoreState,
    rent: &solana_program::rent::Rent,
) -> Result<(SourceMaterialV2, RecoveryPolicyV2, [u16; 3]), ResolutionCoreOperatorErrorV3> {
    authenticate_finalized_record(
        registry,
        source_material,
        source_material_staging,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        market.identity.resolution_policy.to_bytes(),
        rent,
    )?;
    authenticate_finalized_record(
        registry,
        capability_manifest,
        capability_manifest_staging,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market.identity.capability_manifest.to_bytes(),
        rent,
    )?;
    let material = SourceMaterialV2::decode(&source_material.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if material.product_record_digest().to_bytes() != market.identity.product_record.to_bytes() {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }
    let recovery_policy_id = material
        .recovery_policy()
        .ok_or(ResolutionCoreOperatorErrorV3::Record)?;
    authenticate_finalized_record(
        registry,
        recovery_policy,
        recovery_policy_staging,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        recovery_policy_id.to_bytes(),
        rent,
    )?;
    if recovery_policy.data.len() != RECOVERY_POLICY_BYTES_V2 {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }
    let policy = RecoveryPolicyV2::decode(&recovery_policy.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    if policy.attempt_count() != 1 {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }
    let manifest = CapabilityManifestV1::decode(&capability_manifest.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let entries = select_resolution_funding_entries(material, policy, manifest)?;
    Ok((material, policy, entries))
}

fn select_resolution_funding_entries(
    material: SourceMaterialV2,
    policy: RecoveryPolicyV2,
    manifest: CapabilityManifestV1<'_>,
) -> Result<[u16; 3], ResolutionCoreOperatorErrorV3> {
    let recovery_policy = material
        .recovery_policy()
        .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
    let expected = [
        policy
            .attempt(0)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?
            .funding_allocation_id()
            .to_bytes(),
        recovery_policy.to_bytes(),
        hash(&material.to_bytes()).to_bytes(),
    ];
    let mut selected = [None; 3];
    let mut entry_index = 0_u16;
    while entry_index < manifest.entry_count() {
        let entry = manifest
            .entry(entry_index)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        for (slot, expected_config) in expected.iter().enumerate() {
            if entry.config_id().to_bytes() == *expected_config
                && entry.release_id().to_bytes() == RESOLUTION_CONTROLLER_RELEASE_ID_V4
            {
                let selection = selected
                    .get_mut(slot)
                    .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
                if selection.replace(entry_index).is_some() {
                    return Err(ResolutionCoreOperatorErrorV3::Funding);
                }
            }
        }
        entry_index = entry_index
            .checked_add(1)
            .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
    }
    let result = [
        selected[0].ok_or(ResolutionCoreOperatorErrorV3::Funding)?,
        selected[1].ok_or(ResolutionCoreOperatorErrorV3::Funding)?,
        selected[2].ok_or(ResolutionCoreOperatorErrorV3::Funding)?,
    ];
    if !distinct_funding_entries(result) {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    Ok(result)
}

fn authenticate_vacant_destination(
    destination: &ObservedAccount,
    expected_key: Pubkey,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    if destination.key != expected_key
        || destination.owner != system_program::ID
        || destination.executable
        || !destination.data.is_empty()
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    Ok(())
}

fn prospective_pending_funding(
    destination: &ObservedAccount,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
    rent: &solana_program::rent::Rent,
) -> Result<(FundingStateV1, u64), ResolutionCoreOperatorErrorV3> {
    if destination.owner != system_program::ID
        || destination.executable
        || !destination.data.is_empty()
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let quote = manifest
        .entry(entry_index)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?
        .funding_quote();
    if quote.realm_collateral().is_some() {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let target = rent
        .minimum_balance(FUNDING_STATE_BYTES)
        .checked_add(quote.amounts().native_lamports_total())
        .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
    let top_up = exact_funding_top_up(destination.lamports, target)?;
    let custody =
        FundingCustodyObservationV1::native_only(target, rent.minimum_balance(FUNDING_STATE_BYTES))
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let funding = FundingStateV1::new(manifest_id, manifest, entry_index, custody)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    Ok((funding, top_up))
}

fn exact_funding_top_up(
    observed_lamports: u64,
    exact_target_lamports: u64,
) -> Result<u64, ResolutionCoreOperatorErrorV3> {
    exact_target_lamports
        .checked_sub(observed_lamports)
        .ok_or(ResolutionCoreOperatorErrorV3::Funding)
}

const fn distinct_funding_entries(entries: [u16; 3]) -> bool {
    let [recovery, exhaustion, failure] = entries;
    recovery != exhaustion && recovery != failure && exhaustion != failure
}

#[allow(clippy::too_many_arguments)]
fn authenticate_pending_funding(
    market: Pubkey,
    resolution_program: Pubkey,
    account: &ObservedAccount,
    generation: u64,
    manifest_id: CapabilityContentId,
    manifest: CapabilityManifestV1<'_>,
    entry_index: u16,
    rent: &solana_program::rent::Rent,
) -> Result<FundingStateV1, ResolutionCoreOperatorErrorV3> {
    if account.owner != resolution_program
        || account.executable
        || account.data.len() != FUNDING_STATE_BYTES
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let funding = FundingStateV1::decode(&account.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    if funding.entry_index() != entry_index || funding.status() != FundingStatus::Pending {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    funding
        .validate_against(
            manifest_id,
            manifest,
            FundingCustodyObservationV1::native_only(
                account.lamports,
                rent.minimum_balance(FUNDING_STATE_BYTES),
            )
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?,
        )
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    if Pubkey::find_program_address(&derivation.seed_components(), &resolution_program).0
        != account.key
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    Ok(funding)
}

fn authenticate_primary_source(
    snapshot: &ResolutionVerifyFundReadySnapshotV3,
    market: CoreState,
    rent: &solana_program::rent::Rent,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    if snapshot.source_state.owner != snapshot.resolution_program.key
        || snapshot.source_state.executable
        || snapshot.source_state.data.len() != SOURCE_RESOLUTION_STATE_BYTES_V2
        || !rent.is_exempt(
            snapshot.source_state.lamports,
            snapshot.source_state.data.len(),
        )
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let source = SourceResolutionStateV2::decode(&snapshot.source_state.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let seeds = source.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        &snapshot.resolution_program.key,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    if source.phase() != SourceResolutionPhaseV1::Primary
        || snapshot.source_state.key != expected
        || source.market() != snapshot.market.key.to_bytes()
        || source.generation() != market.identity.generation
        || source.material_id().to_bytes() != market.identity.resolution_policy.to_bytes()
        || source.rent_beneficiary() != market.rent_beneficiary.to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    Ok(())
}

fn funding_role_request(
    action: ResolutionCoreActionV1,
    source_state: Pubkey,
    market: CoreState,
    funding: [Pubkey; 3],
    entries: [u16; 3],
) -> ResolutionRoleRequestV1 {
    ResolutionRoleRequestV1 {
        action,
        receipt_kind: ResolutionCoreReceiptKindV1::None,
        source_state: source_state.to_bytes(),
        source_material: market.identity.resolution_policy.to_bytes(),
        capability_manifest: market.identity.capability_manifest.to_bytes(),
        recovery_funding: funding[0].to_bytes(),
        exhaustion_funding: funding[1].to_bytes(),
        failure_funding: funding[2].to_bytes(),
        receipt: [0; 32],
        beneficiary: market.rent_beneficiary.to_bytes(),
        recovery_entry_index: entries[0],
        exhaustion_entry_index: entries[1],
        failure_entry_index: entries[2],
        receipt_sequence: 0,
    }
}

fn encode_funding_role_request(
    request: ResolutionRoleRequestV1,
) -> Result<(Vec<u8>, [u8; 32]), ResolutionCoreOperatorErrorV3> {
    let body = request
        .to_bytes()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let header = CapabilityFundingHeaderV1::new(3)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?
        .encode();
    let mut role_bytes = Vec::with_capacity(header.len() + body.len());
    role_bytes.extend_from_slice(&header);
    role_bytes.extend_from_slice(&body);
    let digest = hash(&role_bytes).to_bytes();
    Ok((role_bytes, digest))
}

fn core_caller_authority(
    market: CoreState,
    core_program: Pubkey,
    source_state: Pubkey,
    request_digest: [u8; 32],
) -> Result<Pubkey, ResolutionCoreOperatorErrorV3> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        market.identity.selected_release_set.to_bytes(),
        market.identity.market_id.to_bytes(),
        ExecutionRoleV1::Core,
        source_state.to_bytes(),
        request_digest,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &core_program).0)
}

#[allow(clippy::too_many_arguments)]
fn assemble_funding_instruction(
    action: ResolutionCoreActionV1,
    market: CoreState,
    market_data: &[u8],
    core_program: Pubkey,
    source_state: Pubkey,
    caller_authority: Pubkey,
    request_digest: [u8; 32],
    role_bytes: Vec<u8>,
    accounts: Vec<AccountMeta>,
) -> Result<Instruction, ResolutionCoreOperatorErrorV3> {
    let effect = match action {
        ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund,
        ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady,
        _ => return Err(ResolutionCoreOperatorErrorV3::Encoding),
    };
    let envelope = CoreEffectEnvelopeV1::new(
        effect,
        Role::Resolution,
        identity(core_program.to_bytes())?,
        identity(caller_authority.to_bytes())?,
        market.identity.selected_release_set,
        market.identity.market_id,
        identity(source_state.to_bytes())?,
        identity(hash(market_data).to_bytes())?,
        identity(request_digest)?,
        market.identity.generation,
        0,
        0,
        u32::try_from(role_bytes.len()).map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let request = Request::administrative(
        Action::VerifyReadiness,
        market.identity.generation,
        market.identity.market_id,
    );
    let mut data = Vec::new();
    data.extend_from_slice(
        &request
            .encode()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    );
    data.extend_from_slice(
        &envelope
            .encode()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?,
    );
    data.extend_from_slice(&role_bytes);
    Ok(Instruction {
        program_id: core_program,
        accounts,
        data,
    })
}

fn create_accounts(
    snapshot: &ResolutionCreateFundSnapshotV3,
    authority: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(snapshot.market.key, false),
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.source_material.key, false),
        AccountMeta::new_readonly(snapshot.source_material_staging.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest_staging.key, false),
        AccountMeta::new(snapshot.source_destination.key, false),
        AccountMeta::new(snapshot.recovery_destination.key, false),
        AccountMeta::new(snapshot.exhaustion_destination.key, false),
        AccountMeta::new(snapshot.failure_destination.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.system_program.key, false),
        AccountMeta::new_readonly(snapshot.recovery_policy.key, false),
        AccountMeta::new_readonly(snapshot.recovery_policy_staging.key, false),
    ]
}

fn verify_accounts(
    snapshot: &ResolutionVerifyFundReadySnapshotV3,
    authority: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(snapshot.market.key, false),
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.source_material.key, false),
        AccountMeta::new_readonly(snapshot.source_material_staging.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest_staging.key, false),
        AccountMeta::new_readonly(snapshot.source_state.key, false),
        AccountMeta::new(snapshot.recovery_funding.key, false),
        AccountMeta::new(snapshot.exhaustion_funding.key, false),
        AccountMeta::new(snapshot.failure_funding.key, false),
        AccountMeta::new(snapshot.beneficiary.key, false),
        AccountMeta::new_readonly(snapshot.clock_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.recovery_policy.key, false),
        AccountMeta::new_readonly(snapshot.recovery_policy_staging.key, false),
    ]
}

fn validate_funding_frame(
    instruction: &Instruction,
    action: ResolutionCoreActionV1,
    authority: Pubkey,
    source_state: Pubkey,
    beneficiary: [u8; 32],
) -> Result<ResolutionRoleRequestV1, ResolutionCoreOperatorErrorV3> {
    let account_count = match action {
        ResolutionCoreActionV1::CreateFund => RESOLUTION_CREATE_FUND_ACCOUNT_COUNT_V3,
        ResolutionCoreActionV1::VerifyFundReady => RESOLUTION_VERIFY_FUND_ACCOUNT_COUNT_V3,
        _ => return Err(ResolutionCoreOperatorErrorV3::Frame),
    };
    let accounts = &instruction.accounts;
    let writable: &[usize] = match action {
        ResolutionCoreActionV1::CreateFund => &[1, 12, 13, 14, 15],
        ResolutionCoreActionV1::VerifyFundReady => &[1, 13, 14, 15, 16],
        _ => return Err(ResolutionCoreOperatorErrorV3::Frame),
    };
    if accounts.len() != account_count
        || accounts.iter().any(|account| account.is_signer)
        || accounts.first().map(|account| account.pubkey) != Some(authority)
        || instruction.program_id
            != accounts
                .get(4)
                .map(|account| account.pubkey)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
        || accounts
            .iter()
            .enumerate()
            .any(|(index, account)| account.is_writable != writable.contains(&index))
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    for (left, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left + 1)
            .any(|other| other.pubkey == account.pubkey)
        {
            return Err(ResolutionCoreOperatorErrorV3::Frame);
        }
    }
    let request_end = dclutch_market_core_codec::REQUEST_BYTES;
    let envelope_end = request_end
        .checked_add(dclutch_market_core_codec::CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let request = Request::decode(
        instruction
            .data
            .get(..request_end)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let envelope = CoreEffectEnvelopeV1::decode(
        instruction
            .data
            .get(request_end..envelope_end)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let role_bytes = instruction
        .data
        .get(envelope_end..)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let header = CapabilityFundingHeaderV1::decode(
        role_bytes
            .get(..dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let role = ResolutionRoleRequestV1::decode(
        role_bytes
            .get(dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let effect = match action {
        ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund,
        ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady,
        _ => return Err(ResolutionCoreOperatorErrorV3::Frame),
    };
    let digest = hash(role_bytes).to_bytes();
    if header.funding_count() != 3
        || request.action != Action::VerifyReadiness
        || request.market.to_bytes()
            != accounts
                .get(1)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || envelope.action() != effect
        || envelope.target_role() != Role::Resolution
        || envelope.caller_program().to_bytes() != instruction.program_id.to_bytes()
        || envelope.caller_authority().to_bytes() != authority.to_bytes()
        || envelope.market().to_bytes() != request.market.to_bytes()
        || envelope.generation() != request.generation
        || envelope.expected_resource_a_revision() != 0
        || envelope.expected_resource_b_revision() != 0
        || envelope.context().to_bytes() != source_state.to_bytes()
        || envelope.role_request_digest().to_bytes() != digest
        || role.action != action
        || role.receipt_kind != ResolutionCoreReceiptKindV1::None
        || role.source_state != source_state.to_bytes()
        || role.recovery_funding
            != accounts
                .get(13)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || role.exhaustion_funding
            != accounts
                .get(14)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || role.failure_funding
            != accounts
                .get(15)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || role.receipt != [0; 32]
        || role.beneficiary != beneficiary
        || !distinct_funding_entries([
            role.recovery_entry_index,
            role.exhaustion_entry_index,
            role.failure_entry_index,
        ])
        || role.receipt_sequence != 0
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let expected_authority = Pubkey::find_program_address(
        &envelope
            .caller_authority_seeds()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?
            .as_slices(),
        &instruction.program_id,
    )
    .0;
    if expected_authority != authority {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(role)
}

fn authenticate_release_graph(
    snapshot: &ResolutionAdmitTerminalSnapshotV3,
    market: CoreState,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    authenticate_release_coordinates(
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        market,
    )
}

fn authenticate_close_release_graph(
    snapshot: &ResolutionCloseFundSnapshotV3,
    market: CoreState,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    authenticate_release_coordinates(
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        market,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_release_coordinates(
    activation_cache: &ObservedAccount,
    registry_program: &ObservedAccount,
    core_program: &ObservedAccount,
    core_programdata: &ObservedAccount,
    resolution_program: &ObservedAccount,
    resolution_programdata: &ObservedAccount,
    market: CoreState,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    if activation_cache.owner != registry_program.key
        || activation_cache.executable
        || activation_cache.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || registry_program.owner != bpf_loader_upgradeable::ID
        || !registry_program.executable
    {
        return Err(ResolutionCoreOperatorErrorV3::Release);
    }
    ProgramV3View::parse(&registry_program.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    let view = ActivatedExecutionReleaseSetViewV1::decode(&activation_cache.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    let release_set = view
        .execution_release_set_id()
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
        &registry_program.key,
    )
    .0;
    if release_set.to_bytes() != market.identity.selected_release_set.to_bytes()
        || activation_cache.key != expected_cache
    {
        return Err(ResolutionCoreOperatorErrorV3::Release);
    }
    authenticate_role_deployment(view, ExecutionRoleV1::Core, core_program, core_programdata)?;
    authenticate_role_deployment(
        view,
        ExecutionRoleV1::Resolution,
        resolution_program,
        resolution_programdata,
    )
}

fn authenticate_role_deployment(
    view: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    let activated = view
        .role(role)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    let release = activated.release();
    let observation = deployment_observation(program, programdata, release)?;
    activated
        .authenticate_current_deployment(observation)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, ResolutionCoreOperatorErrorV3> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(ResolutionCoreOperatorErrorV3::Release);
    }
    let program_view =
        ProgramV3View::parse(&program.data).map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    let derived =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != derived {
        return Err(ResolutionCoreOperatorErrorV3::Release);
    }
    let data = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Release)?;
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        data.deployment_slot(),
        hash(data.elf()).to_bytes(),
        data.upgrade_authority(),
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Release)
}

fn authenticate_source(
    snapshot: &ResolutionAdmitTerminalSnapshotV3,
    market: CoreState,
    source: SourceResolutionStateV2,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    let seeds = source.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        &snapshot.resolution_program.key,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    if snapshot.source_state.owner != snapshot.resolution_program.key
        || snapshot.source_state.executable
        || snapshot.source_state.key != expected
        || source.market() != snapshot.market.key.to_bytes()
        || source.generation() != market.identity.generation
        || source.material_id().to_bytes() != market.identity.resolution_policy.to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }
    Ok(())
}

fn authenticate_close_source(
    snapshot: &ResolutionCloseFundSnapshotV3,
    market: CoreState,
    source: SourceResolutionStateV2,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    let seeds = source.pda_seeds();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            &seeds.market(),
            &seeds.generation_le(),
            &bump,
        ],
        &snapshot.resolution_program.key,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Terminal)?;
    if snapshot.source_state.owner != snapshot.resolution_program.key
        || snapshot.source_state.executable
        || snapshot.source_state.key != expected
        || !matches!(
            source.phase(),
            SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
        )
        || source.market() != snapshot.market.key.to_bytes()
        || source.generation() != market.identity.generation
        || source.material_id().to_bytes() != market.identity.resolution_policy.to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Terminal);
    }
    Ok(())
}

fn authenticate_funding(
    snapshot: &ResolutionAdmitTerminalSnapshotV3,
    market: CoreState,
    rent: &solana_program::rent::Rent,
) -> Result<[u16; 3], ResolutionCoreOperatorErrorV3> {
    let manifest_id = CapabilityContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let manifest = CapabilityManifestV1::decode(&snapshot.capability_manifest.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let accounts = [
        &snapshot.recovery_funding,
        &snapshot.exhaustion_funding,
        &snapshot.failure_funding,
    ];
    let mut indices = [0_u16; 3];
    for (slot, account) in accounts.into_iter().enumerate() {
        if account.owner != snapshot.resolution_program.key
            || account.executable
            || account.data.len() != FUNDING_STATE_BYTES
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        let funding = FundingStateV1::decode(&account.data)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        if funding.status() != FundingStatus::Active {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        funding
            .validate_against(
                manifest_id,
                manifest,
                FundingCustodyObservationV1::native_only(
                    account.lamports,
                    rent.minimum_balance(FUNDING_STATE_BYTES),
                )
                .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?,
            )
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        let derivation = CapabilityFundingDerivationV1::new(
            snapshot.market.key.to_bytes(),
            market.identity.generation,
            manifest_id,
            manifest,
            funding,
        )
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        if Pubkey::find_program_address(
            &derivation.seed_components(),
            &snapshot.resolution_program.key,
        )
        .0 != account.key
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        *indices
            .get_mut(slot)
            .ok_or(ResolutionCoreOperatorErrorV3::Funding)? = funding.entry_index();
    }
    if !distinct_funding_entries(indices) {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    Ok(indices)
}

fn authenticate_close_funding(
    snapshot: &ResolutionCloseFundSnapshotV3,
    market: CoreState,
    material: SourceMaterialV2,
    recovery_policy: RecoveryPolicyV2,
    rent: &solana_program::rent::Rent,
) -> Result<([u16; 3], u64), ResolutionCoreOperatorErrorV3> {
    let manifest_id = CapabilityContentId::new(market.identity.capability_manifest.to_bytes())
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let manifest = CapabilityManifestV1::decode(&snapshot.capability_manifest.data)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
    let accounts = [
        &snapshot.recovery_funding,
        &snapshot.exhaustion_funding,
        &snapshot.failure_funding,
    ];
    let mut indices = [0_u16; 3];
    let mut refund = snapshot.source_state.lamports;
    for (slot, account) in accounts.into_iter().enumerate() {
        if account.owner != snapshot.resolution_program.key
            || account.executable
            || account.data.len() != FUNDING_STATE_BYTES
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        let funding = FundingStateV1::decode(&account.data)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        if funding.status() != FundingStatus::Active {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        let custody = FundingCustodyObservationV1::native_only(
            account.lamports,
            rent.minimum_balance(FUNDING_STATE_BYTES),
        )
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        funding
            .validate_against(manifest_id, manifest, custody)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        let close = funding
            .close(
                manifest_id,
                manifest,
                custody,
                snapshot.beneficiary.key.to_bytes(),
            )
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        if close.native_rent_credit() != snapshot.beneficiary.key.to_bytes()
            || close.realm_token_beneficiary().is_some()
            || close.remaining_realm_collateral() != 0
            || close.realm_collateral_donation() != 0
            || close.vault_rent_lamports() != 0
            || close.vault_lamport_donation() != 0
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        let derivation = CapabilityFundingDerivationV1::new(
            snapshot.market.key.to_bytes(),
            market.identity.generation,
            manifest_id,
            manifest,
            funding,
        )
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        if Pubkey::find_program_address(
            &derivation.seed_components(),
            &snapshot.resolution_program.key,
        )
        .0 != account.key
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
        *indices
            .get_mut(slot)
            .ok_or(ResolutionCoreOperatorErrorV3::Funding)? = funding.entry_index();
        refund = refund
            .checked_add(account.lamports)
            .ok_or(ResolutionCoreOperatorErrorV3::Funding)?;
    }
    if !distinct_funding_entries(indices) {
        return Err(ResolutionCoreOperatorErrorV3::Funding);
    }
    let recovery_allocation = recovery_policy
        .attempt(0)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?
        .funding_allocation_id()
        .to_bytes();
    let recovery_policy_id = material
        .recovery_policy()
        .ok_or(ResolutionCoreOperatorErrorV3::Funding)?
        .to_bytes();
    for (index, expected_config) in [
        (indices[0], recovery_allocation),
        (indices[1], recovery_policy_id),
        (indices[2], market.identity.resolution_policy.to_bytes()),
    ] {
        let entry = manifest
            .entry(index)
            .map_err(|_| ResolutionCoreOperatorErrorV3::Funding)?;
        if entry.config_id().to_bytes() != expected_config
            || entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V4
        {
            return Err(ResolutionCoreOperatorErrorV3::Funding);
        }
    }
    Ok((indices, refund))
}

fn authenticate_finalized_record(
    registry: Pubkey,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    schema: [u8; 32],
    expected_digest: [u8; 32],
    rent: &solana_program::rent::Rent,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    let digest = hash(&raw.data).to_bytes();
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    if digest != expected_digest
        || raw.key != expected_raw
        || raw.owner != registry
        || raw.executable
        || raw.data.is_empty()
        || !rent.is_exempt(raw.lamports, raw.data.len())
        || staging.key != expected_staging
        || staging.owner != system_program::ID
        || staging.lamports != 0
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(ResolutionCoreOperatorErrorV3::Record);
    }
    Ok(())
}

fn same_finalized_create_observation(
    snapshot: &ResolutionCreateFundSnapshotV3,
) -> Result<Observation, ResolutionCoreOperatorErrorV3> {
    let accounts = [
        &snapshot.market,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        &snapshot.source_destination,
        &snapshot.recovery_destination,
        &snapshot.exhaustion_destination,
        &snapshot.failure_destination,
        &snapshot.rent_sysvar,
        &snapshot.system_program,
        &snapshot.recovery_policy,
        &snapshot.recovery_policy_staging,
    ];
    same_finalized_accounts(&accounts)
}

fn same_finalized_verify_observation(
    snapshot: &ResolutionVerifyFundReadySnapshotV3,
) -> Result<Observation, ResolutionCoreOperatorErrorV3> {
    let accounts = [
        &snapshot.market,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        &snapshot.source_state,
        &snapshot.recovery_funding,
        &snapshot.exhaustion_funding,
        &snapshot.failure_funding,
        &snapshot.beneficiary,
        &snapshot.clock_sysvar,
        &snapshot.rent_sysvar,
        &snapshot.recovery_policy,
        &snapshot.recovery_policy_staging,
    ];
    same_finalized_accounts(&accounts)
}

fn same_finalized_accounts(
    accounts: &[&ObservedAccount],
) -> Result<Observation, ResolutionCoreOperatorErrorV3> {
    let first = accounts
        .first()
        .ok_or(ResolutionCoreOperatorErrorV3::Snapshot)?
        .observation;
    if first.finality != Finality::Finalized
        || accounts.iter().any(|account| account.observation != first)
    {
        return Err(ResolutionCoreOperatorErrorV3::Snapshot);
    }
    Ok(first)
}

fn same_finalized_observation(
    snapshot: &ResolutionAdmitTerminalSnapshotV3,
) -> Result<Observation, ResolutionCoreOperatorErrorV3> {
    let accounts = [
        &snapshot.market,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        &snapshot.source_state,
        &snapshot.recovery_funding,
        &snapshot.exhaustion_funding,
        &snapshot.failure_funding,
        &snapshot.certificate,
        &snapshot.rent_sysvar,
        &snapshot.product_raw,
        &snapshot.product_staging,
        &snapshot.result_domain_raw,
        &snapshot.result_domain_staging,
        &snapshot.portfolio_raw,
        &snapshot.portfolio_staging,
    ];
    let first = accounts
        .first()
        .ok_or(ResolutionCoreOperatorErrorV3::Snapshot)?
        .observation;
    if first.finality != Finality::Finalized
        || accounts.iter().any(|account| account.observation != first)
    {
        return Err(ResolutionCoreOperatorErrorV3::Snapshot);
    }
    Ok(first)
}

fn same_finalized_close_observation(
    snapshot: &ResolutionCloseFundSnapshotV3,
) -> Result<Observation, ResolutionCoreOperatorErrorV3> {
    let accounts = [
        &snapshot.market,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.source_material,
        &snapshot.source_material_staging,
        &snapshot.capability_manifest,
        &snapshot.capability_manifest_staging,
        &snapshot.source_state,
        &snapshot.recovery_funding,
        &snapshot.exhaustion_funding,
        &snapshot.failure_funding,
        &snapshot.certificate,
        &snapshot.closure_destination,
        &snapshot.beneficiary,
        &snapshot.clock_sysvar,
        &snapshot.rent_sysvar,
        &snapshot.system_program,
        &snapshot.recovery_policy,
        &snapshot.recovery_policy_staging,
    ];
    let first = accounts
        .first()
        .ok_or(ResolutionCoreOperatorErrorV3::Snapshot)?
        .observation;
    if first.finality != Finality::Finalized
        || accounts.iter().any(|account| account.observation != first)
    {
        return Err(ResolutionCoreOperatorErrorV3::Snapshot);
    }
    Ok(first)
}

fn admit_accounts(
    snapshot: &ResolutionAdmitTerminalSnapshotV3,
    authority: Pubkey,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(snapshot.market.key, false),
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.source_material.key, false),
        AccountMeta::new_readonly(snapshot.source_material_staging.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest_staging.key, false),
        AccountMeta::new_readonly(snapshot.source_state.key, false),
        AccountMeta::new_readonly(snapshot.recovery_funding.key, false),
        AccountMeta::new_readonly(snapshot.exhaustion_funding.key, false),
        AccountMeta::new_readonly(snapshot.failure_funding.key, false),
        AccountMeta::new_readonly(snapshot.certificate.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.product_raw.key, false),
        AccountMeta::new_readonly(snapshot.product_staging.key, false),
        AccountMeta::new_readonly(snapshot.result_domain_raw.key, false),
        AccountMeta::new_readonly(snapshot.result_domain_staging.key, false),
        AccountMeta::new_readonly(snapshot.portfolio_raw.key, false),
        AccountMeta::new_readonly(snapshot.portfolio_staging.key, false),
    ]
}

fn close_accounts(snapshot: &ResolutionCloseFundSnapshotV3, authority: Pubkey) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new(snapshot.market.key, false),
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.source_material.key, false),
        AccountMeta::new_readonly(snapshot.source_material_staging.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest.key, false),
        AccountMeta::new_readonly(snapshot.capability_manifest_staging.key, false),
        AccountMeta::new(snapshot.source_state.key, false),
        AccountMeta::new(snapshot.recovery_funding.key, false),
        AccountMeta::new(snapshot.exhaustion_funding.key, false),
        AccountMeta::new(snapshot.failure_funding.key, false),
        AccountMeta::new_readonly(snapshot.certificate.key, false),
        AccountMeta::new(snapshot.closure_destination.key, false),
        AccountMeta::new(snapshot.beneficiary.key, false),
        AccountMeta::new_readonly(snapshot.clock_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new_readonly(snapshot.system_program.key, false),
        AccountMeta::new_readonly(snapshot.recovery_policy.key, false),
        AccountMeta::new_readonly(snapshot.recovery_policy_staging.key, false),
    ]
}

fn exact_admit_frame(
    accounts: &[AccountMeta],
    snapshot: &ResolutionAdmitTerminalSnapshotV3,
    authority: Pubkey,
) -> bool {
    if accounts.len() != RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3
        || accounts.iter().any(|account| account.is_signer)
        || accounts.first().map(|account| account.pubkey) != Some(authority)
        || accounts.get(1).map(|account| account.pubkey) != Some(snapshot.market.key)
        || !accounts.get(1).is_some_and(|account| account.is_writable)
        || accounts
            .iter()
            .enumerate()
            .any(|(index, account)| index != 1 && account.is_writable)
    {
        return false;
    }
    for (left, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left + 1)
            .any(|other| other.pubkey == account.pubkey)
        {
            return false;
        }
    }
    true
}

fn exact_close_frame(
    accounts: &[AccountMeta],
    snapshot: &ResolutionCloseFundSnapshotV3,
    authority: Pubkey,
) -> bool {
    if accounts.len() != RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3
        || accounts.iter().any(|account| account.is_signer)
        || accounts.first().map(|account| account.pubkey) != Some(authority)
        || accounts.get(1).map(|account| account.pubkey) != Some(snapshot.market.key)
        || accounts.get(17).map(|account| account.pubkey) != Some(snapshot.closure_destination.key)
    {
        return false;
    }
    let writable = [1_usize, 12, 13, 14, 15, 17, 18];
    if accounts
        .iter()
        .enumerate()
        .any(|(index, account)| account.is_writable != writable.contains(&index))
    {
        return false;
    }
    for (left, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left + 1)
            .any(|other| other.pubkey == account.pubkey)
        {
            return false;
        }
    }
    true
}

fn decode_rent(account: &ObservedAccount) -> Result<Rent, ()> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(());
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Rent::from_account_info(&info).map_err(|_| ())
}

fn decode_clock(account: &ObservedAccount) -> Result<Clock, ()> {
    if account.key != sysvar::clock::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Clock::size_of()
    {
        return Err(());
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Clock::from_account_info(&info).map_err(|_| ())
}

fn identity(bytes: [u8; 32]) -> Result<Identity, ResolutionCoreOperatorErrorV3> {
    Identity::new(bytes).map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)
}

/// Revalidate an assembled CreateFund report before composing its top-ups.
pub fn validate_resolution_create_fund_report_v3(
    report: &ResolutionCreateFundReportV3,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    let source = report
        .instruction
        .accounts
        .get(12)
        .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
        .pubkey;
    let role = validate_funding_frame(
        &report.instruction,
        ResolutionCoreActionV1::CreateFund,
        report.caller_authority,
        source,
        report.beneficiary.to_bytes(),
    )?;
    if role.recovery_entry_index != report.funding_entry_indices[0]
        || role.exhaustion_entry_index != report.funding_entry_indices[1]
        || role.failure_entry_index != report.funding_entry_indices[2]
        || hash(
            report
                .instruction
                .data
                .get(
                    dclutch_market_core_codec::REQUEST_BYTES
                        + dclutch_market_core_codec::CORE_EFFECT_ENVELOPE_BYTES_V1..,
                )
                .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
        )
        .to_bytes()
            != report.role_request_digest
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(())
}

/// Revalidate an assembled VerifyFundReady report before compilation.
pub fn validate_resolution_verify_fund_ready_report_v3(
    report: &ResolutionVerifyFundReadyReportV3,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    let source = report
        .instruction
        .accounts
        .get(12)
        .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
        .pubkey;
    let role = validate_funding_frame(
        &report.instruction,
        ResolutionCoreActionV1::VerifyFundReady,
        report.caller_authority,
        source,
        report.beneficiary.to_bytes(),
    )?;
    if report
        .instruction
        .accounts
        .get(16)
        .map(|account| account.pubkey)
        != Some(report.beneficiary)
        || report.activation_slot == 0
        || role.recovery_entry_index != report.funding_entry_indices[0]
        || role.exhaustion_entry_index != report.funding_entry_indices[1]
        || role.failure_entry_index != report.funding_entry_indices[2]
        || hash(
            report
                .instruction
                .data
                .get(
                    dclutch_market_core_codec::REQUEST_BYTES
                        + dclutch_market_core_codec::CORE_EFFECT_ENVELOPE_BYTES_V1..,
                )
                .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
        )
        .to_bytes()
            != report.role_request_digest
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(())
}

/// Revalidate an assembled report before transaction compilation.
pub fn validate_resolution_admit_terminal_report_v3(
    report: &ResolutionAdmitTerminalReportV3,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    if report.instruction.accounts.len() != RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3
        || report.instruction.program_id
            != report
                .instruction
                .accounts
                .get(4)
                .map(|account| account.pubkey)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
        || report
            .instruction
            .accounts
            .iter()
            .any(|account| account.is_signer)
        || report
            .instruction
            .accounts
            .first()
            .map(|account| account.pubkey)
            != Some(report.caller_authority)
        || !report
            .instruction
            .accounts
            .get(1)
            .is_some_and(|account| account.is_writable)
        || report
            .instruction
            .accounts
            .iter()
            .enumerate()
            .any(|(index, account)| index != 1 && account.is_writable)
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let request_end = dclutch_market_core_codec::REQUEST_BYTES;
    let envelope_end = request_end
        .checked_add(dclutch_market_core_codec::CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let request = Request::decode(
        report
            .instruction
            .data
            .get(..request_end)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let envelope = CoreEffectEnvelopeV1::decode(
        report
            .instruction
            .data
            .get(request_end..envelope_end)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let role_bytes = report
        .instruction
        .data
        .get(envelope_end..)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let body = role_bytes
        .get(dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let resolution = ResolutionRoleRequestV1::decode(body)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let observed_digest = hash(role_bytes).to_bytes();
    if request.action != Action::AdmitTerminal
        || envelope.action() != CoreEffectActionV1::AdmitTerminal
        || envelope.target_role() != Role::Resolution
        || envelope.role_request_digest().to_bytes() != observed_digest
        || observed_digest != report.role_request_digest
        || resolution.action != ResolutionCoreActionV1::AdmitTerminal
        || resolution.receipt_sequence != report.terminal_sequence
        || resolution.source_state
            != report
                .instruction
                .accounts
                .get(12)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || resolution.receipt
            != report
                .instruction
                .accounts
                .get(16)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let expected_authority = Pubkey::find_program_address(
        &envelope
            .caller_authority_seeds()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?
            .as_slices(),
        &report.instruction.program_id,
    )
    .0;
    if expected_authority != report.caller_authority {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(())
}

/// Revalidate an assembled CloseFund report before transaction compilation.
pub fn validate_resolution_close_fund_report_v3(
    report: &ResolutionCloseFundReportV3,
) -> Result<(), ResolutionCoreOperatorErrorV3> {
    let accounts = &report.instruction.accounts;
    if accounts.len() != RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3
        || report.instruction.program_id
            != accounts
                .get(4)
                .map(|account| account.pubkey)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
        || accounts.iter().any(|account| account.is_signer)
        || accounts.first().map(|account| account.pubkey) != Some(report.caller_authority)
        || accounts.get(17).map(|account| account.pubkey) != Some(report.closure_receipt)
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let writable = [1_usize, 12, 13, 14, 15, 17, 18];
    if accounts
        .iter()
        .enumerate()
        .any(|(index, account)| account.is_writable != writable.contains(&index))
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    for (left, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left + 1)
            .any(|other| other.pubkey == account.pubkey)
        {
            return Err(ResolutionCoreOperatorErrorV3::Frame);
        }
    }
    let request_end = dclutch_market_core_codec::REQUEST_BYTES;
    let envelope_end = request_end
        .checked_add(dclutch_market_core_codec::CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let request = Request::decode(
        report
            .instruction
            .data
            .get(..request_end)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let envelope = CoreEffectEnvelopeV1::decode(
        report
            .instruction
            .data
            .get(request_end..envelope_end)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let role_bytes = report
        .instruction
        .data
        .get(envelope_end..)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let header_bytes = role_bytes
        .get(..dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
        .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?;
    let header = CapabilityFundingHeaderV1::decode(header_bytes)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let role = ResolutionRoleRequestV1::decode(
        role_bytes
            .get(dclutch_market_core_codec::CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1..)
            .ok_or(ResolutionCoreOperatorErrorV3::Encoding)?,
    )
    .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?;
    let digest = hash(role_bytes).to_bytes();
    let facts = report.expected_retirement_facts;
    if header.funding_count() != 3
        || request.action != Action::Retire
        || request.market.to_bytes() != facts.market
        || request.generation != facts.generation
        || envelope.action() != CoreEffectActionV1::CloseFund
        || envelope.target_role() != Role::Resolution
        || envelope.caller_program().to_bytes() != report.instruction.program_id.to_bytes()
        || envelope.caller_authority().to_bytes() != report.caller_authority.to_bytes()
        || envelope.context().to_bytes() != facts.source_state
        || envelope.expected_resource_a_revision() != report.terminal_sequence
        || envelope.expected_resource_b_revision() != 1
        || envelope.role_request_digest().to_bytes() != digest
        || digest != report.role_request_digest
        || role.action != ResolutionCoreActionV1::CloseFund
        || role.receipt_kind != ResolutionCoreReceiptKindV1::Closure
        || role.receipt_sequence != report.closure_sequence
        || report.terminal_sequence.checked_add(1) != Some(report.closure_sequence)
        || role.source_state != facts.source_state
        || role.source_material != facts.source_material
        || role.capability_manifest != facts.capability_manifest
        || role.receipt != facts.resolution_closure_receipt
        || role.beneficiary != facts.beneficiary
        || role.recovery_funding
            != accounts
                .get(13)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || role.exhaustion_funding
            != accounts
                .get(14)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || role.failure_funding
            != accounts
                .get(15)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || facts.resolution_closure_receipt != report.closure_receipt.to_bytes()
        || facts.terminal_sequence != report.terminal_sequence
        || facts.refund_lamports != report.expected_refund_lamports
        || facts.terminal_certificate
            != accounts
                .get(16)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
        || facts.beneficiary
            != accounts
                .get(18)
                .ok_or(ResolutionCoreOperatorErrorV3::Frame)?
                .pubkey
                .to_bytes()
    {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    let expected_authority = Pubkey::find_program_address(
        &envelope
            .caller_authority_seeds()
            .map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)?
            .as_slices(),
        &report.instruction.program_id,
    )
    .0;
    if expected_authority != report.caller_authority {
        return Err(ResolutionCoreOperatorErrorV3::Frame);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::{rent::Rent, sysvar::SysvarSerialize};
    use solana_sdk_ids::sysvar;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn set_account(accounts: &mut [AccountMeta], index: usize, value: AccountMeta) {
        *accounts.get_mut(index).expect("fixed account index") = value;
    }

    fn funding_instruction(action: ResolutionCoreActionV1) -> (Instruction, Pubkey, [u8; 32]) {
        let core = key(4);
        let market = key(1);
        let source = key(12);
        let beneficiary = key(40);
        let role_request = ResolutionRoleRequestV1 {
            action,
            receipt_kind: ResolutionCoreReceiptKindV1::None,
            source_state: source.to_bytes(),
            source_material: [31; 32],
            capability_manifest: [32; 32],
            recovery_funding: key(13).to_bytes(),
            exhaustion_funding: key(14).to_bytes(),
            failure_funding: key(15).to_bytes(),
            receipt: [0; 32],
            beneficiary: beneficiary.to_bytes(),
            recovery_entry_index: 1,
            exhaustion_entry_index: 2,
            failure_entry_index: 3,
            receipt_sequence: 0,
        };
        let (role_bytes, digest) =
            encode_funding_role_request(role_request).expect("funding role request");
        let seeds = CallerAuthoritySeedsV1::from_bytes(
            [41; 32],
            market.to_bytes(),
            ExecutionRoleV1::Core,
            source.to_bytes(),
            digest,
        )
        .expect("seeds");
        let authority = Pubkey::find_program_address(&seeds.as_slices(), &core).0;
        let effect = match action {
            ResolutionCoreActionV1::CreateFund => CoreEffectActionV1::CreateFund,
            ResolutionCoreActionV1::VerifyFundReady => CoreEffectActionV1::VerifyFundReady,
            _ => CoreEffectActionV1::CreateFund,
        };
        let envelope = CoreEffectEnvelopeV1::new(
            effect,
            Role::Resolution,
            Identity::new(core.to_bytes()).expect("core"),
            Identity::new(authority.to_bytes()).expect("authority"),
            Identity::new([41; 32]).expect("release"),
            Identity::new(market.to_bytes()).expect("market"),
            Identity::new(source.to_bytes()).expect("source"),
            Identity::new([42; 32]).expect("state"),
            Identity::new(digest).expect("digest"),
            1,
            0,
            0,
            u32::try_from(role_bytes.len()).expect("length"),
        )
        .expect("envelope");
        let request = Request::administrative(
            Action::VerifyReadiness,
            1,
            Identity::new(market.to_bytes()).expect("market"),
        );
        let mut data = Vec::new();
        data.extend_from_slice(&request.encode().expect("request"));
        data.extend_from_slice(&envelope.encode().expect("envelope"));
        data.extend_from_slice(&role_bytes);
        let account_count = match action {
            ResolutionCoreActionV1::CreateFund => RESOLUTION_CREATE_FUND_ACCOUNT_COUNT_V3,
            ResolutionCoreActionV1::VerifyFundReady => RESOLUTION_VERIFY_FUND_ACCOUNT_COUNT_V3,
            _ => RESOLUTION_CREATE_FUND_ACCOUNT_COUNT_V3,
        };
        let mut accounts: Vec<AccountMeta> = (0..account_count)
            .map(|index| {
                AccountMeta::new_readonly(
                    key(u8::try_from(index).expect("index").saturating_add(50)),
                    false,
                )
            })
            .collect();
        set_account(
            &mut accounts,
            0,
            AccountMeta::new_readonly(authority, false),
        );
        set_account(&mut accounts, 1, AccountMeta::new(market, false));
        set_account(&mut accounts, 4, AccountMeta::new_readonly(core, false));
        set_account(
            &mut accounts,
            12,
            if action == ResolutionCoreActionV1::CreateFund {
                AccountMeta::new(source, false)
            } else {
                AccountMeta::new_readonly(source, false)
            },
        );
        for index in 13_u8..=15 {
            set_account(
                &mut accounts,
                usize::from(index),
                AccountMeta::new(key(index), false),
            );
        }
        if action == ResolutionCoreActionV1::VerifyFundReady {
            set_account(&mut accounts, 16, AccountMeta::new(beneficiary, false));
        }
        (
            Instruction {
                program_id: core,
                accounts,
                data,
            },
            authority,
            digest,
        )
    }

    fn create_report() -> ResolutionCreateFundReportV3 {
        let (instruction, caller_authority, role_request_digest) =
            funding_instruction(ResolutionCoreActionV1::CreateFund);
        ResolutionCreateFundReportV3 {
            instruction,
            observation: Observation {
                slot: 1,
                unix_timestamp: 1,
                finality: Finality::Finalized,
            },
            caller_authority,
            beneficiary: key(40),
            funding_entry_indices: [1, 2, 3],
            source_top_up_lamports: 7,
            funding_top_up_lamports: [11, 12, 13],
            role_request_digest,
        }
    }

    fn verify_report() -> ResolutionVerifyFundReadyReportV3 {
        let (instruction, caller_authority, role_request_digest) =
            funding_instruction(ResolutionCoreActionV1::VerifyFundReady);
        ResolutionVerifyFundReadyReportV3 {
            instruction,
            observation: Observation {
                slot: 2,
                unix_timestamp: 2,
                finality: Finality::Finalized,
            },
            caller_authority,
            beneficiary: key(40),
            funding_entry_indices: [1, 2, 3],
            activation_slot: 9,
            expected_beneficiary_credit_lamports: 17,
            role_request_digest,
        }
    }

    fn report() -> ResolutionAdmitTerminalReportV3 {
        let core = key(4);
        let market = key(1);
        let source = key(12);
        let certificate = key(16);
        let role_request = ResolutionRoleRequestV1 {
            action: ResolutionCoreActionV1::AdmitTerminal,
            receipt_kind: ResolutionCoreReceiptKindV1::TerminalSuccess,
            source_state: source.to_bytes(),
            source_material: [31; 32],
            capability_manifest: [32; 32],
            recovery_funding: key(13).to_bytes(),
            exhaustion_funding: key(14).to_bytes(),
            failure_funding: key(15).to_bytes(),
            receipt: certificate.to_bytes(),
            beneficiary: [0; 32],
            recovery_entry_index: 1,
            exhaustion_entry_index: 2,
            failure_entry_index: 3,
            receipt_sequence: 9,
        };
        let body = role_request.to_bytes().expect("role request");
        assert_eq!(
            body.len(),
            dclutch_resolution_codec::RESOLUTION_CORE_ROLE_REQUEST_BYTES
        );
        let header = CapabilityFundingHeaderV1::new(3).expect("header").encode();
        let mut role_bytes = Vec::new();
        role_bytes.extend_from_slice(&header);
        role_bytes.extend_from_slice(&body);
        let digest = hash(&role_bytes).to_bytes();
        let seeds = CallerAuthoritySeedsV1::from_bytes(
            [41; 32],
            market.to_bytes(),
            ExecutionRoleV1::Core,
            source.to_bytes(),
            digest,
        )
        .expect("seeds");
        let authority = Pubkey::find_program_address(&seeds.as_slices(), &core).0;
        let envelope = CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::AdmitTerminal,
            Role::Resolution,
            Identity::new(core.to_bytes()).expect("core"),
            Identity::new(authority.to_bytes()).expect("authority"),
            Identity::new([41; 32]).expect("release"),
            Identity::new(market.to_bytes()).expect("market"),
            Identity::new(source.to_bytes()).expect("source"),
            Identity::new([42; 32]).expect("state"),
            Identity::new(digest).expect("digest"),
            1,
            9,
            1,
            u32::try_from(role_bytes.len()).expect("length"),
        )
        .expect("envelope");
        let request = Request::administrative(
            Action::AdmitTerminal,
            1,
            Identity::new(market.to_bytes()).expect("market"),
        );
        let mut data = Vec::new();
        data.extend_from_slice(&request.encode().expect("request"));
        data.extend_from_slice(&envelope.encode().expect("envelope"));
        data.extend_from_slice(&role_bytes);
        let mut accounts: Vec<AccountMeta> = (0_u8..24)
            .map(|index| AccountMeta::new_readonly(key(index.saturating_add(50)), false))
            .collect();
        set_account(
            &mut accounts,
            0,
            AccountMeta::new_readonly(authority, false),
        );
        set_account(&mut accounts, 1, AccountMeta::new(market, false));
        set_account(&mut accounts, 4, AccountMeta::new_readonly(core, false));
        set_account(&mut accounts, 12, AccountMeta::new_readonly(source, false));
        set_account(
            &mut accounts,
            16,
            AccountMeta::new_readonly(certificate, false),
        );
        ResolutionAdmitTerminalReportV3 {
            instruction: Instruction {
                program_id: core,
                accounts,
                data,
            },
            observation: Observation {
                slot: 1,
                unix_timestamp: 1,
                finality: Finality::Finalized,
            },
            caller_authority: authority,
            terminal_sequence: 9,
            selector: 0,
            outcome_count: 2,
            role_request_digest: digest,
        }
    }

    fn close_report() -> ResolutionCloseFundReportV3 {
        let core = key(4);
        let market = key(1);
        let source = key(12);
        let certificate = key(16);
        let closure = key(17);
        let beneficiary = key(18);
        let role_request = ResolutionRoleRequestV1 {
            action: ResolutionCoreActionV1::CloseFund,
            receipt_kind: ResolutionCoreReceiptKindV1::Closure,
            source_state: source.to_bytes(),
            source_material: [31; 32],
            capability_manifest: [32; 32],
            recovery_funding: key(13).to_bytes(),
            exhaustion_funding: key(14).to_bytes(),
            failure_funding: key(15).to_bytes(),
            receipt: closure.to_bytes(),
            beneficiary: beneficiary.to_bytes(),
            recovery_entry_index: 1,
            exhaustion_entry_index: 2,
            failure_entry_index: 3,
            receipt_sequence: 10,
        };
        let body = role_request.to_bytes().expect("role request");
        let header = CapabilityFundingHeaderV1::new(3).expect("header").encode();
        let mut role_bytes = Vec::new();
        role_bytes.extend_from_slice(&header);
        role_bytes.extend_from_slice(&body);
        let digest = hash(&role_bytes).to_bytes();
        let seeds = CallerAuthoritySeedsV1::from_bytes(
            [41; 32],
            market.to_bytes(),
            ExecutionRoleV1::Core,
            source.to_bytes(),
            digest,
        )
        .expect("seeds");
        let authority = Pubkey::find_program_address(&seeds.as_slices(), &core).0;
        let envelope = CoreEffectEnvelopeV1::new(
            CoreEffectActionV1::CloseFund,
            Role::Resolution,
            Identity::new(core.to_bytes()).expect("core"),
            Identity::new(authority.to_bytes()).expect("authority"),
            Identity::new([41; 32]).expect("release"),
            Identity::new(market.to_bytes()).expect("market"),
            Identity::new(source.to_bytes()).expect("source"),
            Identity::new([42; 32]).expect("state"),
            Identity::new(digest).expect("digest"),
            1,
            9,
            1,
            u32::try_from(role_bytes.len()).expect("length"),
        )
        .expect("envelope");
        let request = Request::administrative(
            Action::Retire,
            1,
            Identity::new(market.to_bytes()).expect("market"),
        );
        let mut data = Vec::new();
        data.extend_from_slice(&request.encode().expect("request"));
        data.extend_from_slice(&envelope.encode().expect("envelope"));
        data.extend_from_slice(&role_bytes);
        let mut accounts: Vec<AccountMeta> = (0_u8..24)
            .map(|index| AccountMeta::new_readonly(key(index.saturating_add(50)), false))
            .collect();
        set_account(
            &mut accounts,
            0,
            AccountMeta::new_readonly(authority, false),
        );
        set_account(&mut accounts, 1, AccountMeta::new(market, false));
        set_account(&mut accounts, 4, AccountMeta::new_readonly(core, false));
        set_account(&mut accounts, 12, AccountMeta::new(source, false));
        set_account(&mut accounts, 13, AccountMeta::new(key(13), false));
        set_account(&mut accounts, 14, AccountMeta::new(key(14), false));
        set_account(&mut accounts, 15, AccountMeta::new(key(15), false));
        set_account(
            &mut accounts,
            16,
            AccountMeta::new_readonly(certificate, false),
        );
        set_account(&mut accounts, 17, AccountMeta::new(closure, false));
        set_account(&mut accounts, 18, AccountMeta::new(beneficiary, false));
        let facts = ResolutionRetirementReceiptFactsV3 {
            market: market.to_bytes(),
            generation: 1,
            resolution_closure_receipt: closure.to_bytes(),
            source_state: source.to_bytes(),
            source_material: [31; 32],
            capability_manifest: [32; 32],
            terminal_certificate: certificate.to_bytes(),
            beneficiary: beneficiary.to_bytes(),
            selector: 1,
            terminal_sequence: 9,
            source_state_digest: [71; 32],
            terminal_certificate_digest: [72; 32],
            funding_set_digest: [73; 32],
            refund_lamports: 100,
            closed_at: 11,
        };
        ResolutionCloseFundReportV3 {
            instruction: Instruction {
                program_id: core,
                accounts,
                data,
            },
            observation: Observation {
                slot: 1,
                unix_timestamp: 1,
                finality: Finality::Finalized,
            },
            caller_authority: authority,
            closure_receipt: closure,
            terminal_sequence: 9,
            closure_sequence: 10,
            expected_refund_lamports: 100,
            role_request_digest: digest,
            expected_retirement_facts: facts,
        }
    }

    #[test]
    fn exact_report_revalidates_and_certificate_substitution_refuses() {
        let exact = report();
        assert_eq!(validate_resolution_admit_terminal_report_v3(&exact), Ok(()));
        let mut substituted = exact;
        substituted
            .instruction
            .accounts
            .get_mut(16)
            .expect("certificate account")
            .pubkey = key(99);
        assert_eq!(
            validate_resolution_admit_terminal_report_v3(&substituted),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
    }

    #[test]
    fn create_and_readiness_reports_refuse_substitution_and_privilege() {
        let create = create_report();
        assert_eq!(validate_resolution_create_fund_report_v3(&create), Ok(()));
        assert_eq!(create.source_top_up_lamports, 7);
        assert_eq!(create.funding_top_up_lamports, [11, 12, 13]);

        let mut substituted = create.clone();
        substituted
            .instruction
            .accounts
            .get_mut(13)
            .expect("recovery funding")
            .pubkey = key(99);
        assert_eq!(
            validate_resolution_create_fund_report_v3(&substituted),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut privilege = create;
        privilege
            .instruction
            .accounts
            .get_mut(12)
            .expect("Source output")
            .is_writable = false;
        assert_eq!(
            validate_resolution_create_fund_report_v3(&privilege),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let ready = verify_report();
        assert_eq!(
            validate_resolution_verify_fund_ready_report_v3(&ready),
            Ok(())
        );
        let mut wrong_beneficiary = ready.clone();
        wrong_beneficiary
            .instruction
            .accounts
            .get_mut(16)
            .expect("beneficiary")
            .pubkey = key(98);
        assert_eq!(
            validate_resolution_verify_fund_ready_report_v3(&wrong_beneficiary),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
        let mut stale = ready;
        stale.activation_slot = 0;
        assert_eq!(
            validate_resolution_verify_fund_ready_report_v3(&stale),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
    }

    #[test]
    fn funding_top_up_is_dust_tolerant_but_refuses_surplus() {
        assert_eq!(exact_funding_top_up(0, 100), Ok(100));
        assert_eq!(exact_funding_top_up(99, 100), Ok(1));
        assert_eq!(exact_funding_top_up(100, 100), Ok(0));
        assert_eq!(
            exact_funding_top_up(101, 100),
            Err(ResolutionCoreOperatorErrorV3::Funding)
        );
        assert!(distinct_funding_entries([3, 1, 2]));
        assert!(!distinct_funding_entries([3, 1, 3]));
    }

    #[test]
    fn system_authentication_accepts_native_marker_data_and_refuses_substitution() {
        let exact = ObservedAccount {
            observation: Observation {
                slot: 1,
                unix_timestamp: 1,
                finality: Finality::Finalized,
            },
            key: system_program::ID,
            owner: native_loader::ID,
            lamports: 1,
            executable: true,
            data: b"solana system program".to_vec(),
        };
        assert_eq!(authenticate_system(&exact), Ok(()));

        let mut wrong_key = exact.clone();
        wrong_key.key = key(99);
        assert_eq!(
            authenticate_system(&wrong_key),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
        let mut wrong_owner = exact.clone();
        wrong_owner.owner = key(98);
        assert_eq!(
            authenticate_system(&wrong_owner),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
        let mut non_executable = exact;
        non_executable.executable = false;
        assert_eq!(
            authenticate_system(&non_executable),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut privilege = create_report();
        privilege
            .instruction
            .accounts
            .get_mut(17)
            .expect("System Program account")
            .is_writable = true;
        assert_eq!(
            validate_resolution_create_fund_report_v3(&privilege),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
    }

    #[test]
    fn request_mutation_and_privilege_escalation_refuse() {
        let mut request_mutation = report();
        *request_mutation
            .instruction
            .data
            .last_mut()
            .expect("nonempty instruction") ^= 1;
        assert!(validate_resolution_admit_terminal_report_v3(&request_mutation).is_err());

        let mut privilege = report();
        privilege
            .instruction
            .accounts
            .get_mut(16)
            .expect("certificate account")
            .is_writable = true;
        assert_eq!(
            validate_resolution_admit_terminal_report_v3(&privilege),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
    }

    #[test]
    fn close_report_refuses_receipt_beneficiary_and_privilege_substitution() {
        let exact = close_report();
        assert_eq!(validate_resolution_close_fund_report_v3(&exact), Ok(()));

        let mut receipt = exact.clone();
        receipt
            .instruction
            .accounts
            .get_mut(17)
            .expect("closure account")
            .pubkey = key(99);
        assert_eq!(
            validate_resolution_close_fund_report_v3(&receipt),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut beneficiary = exact.clone();
        beneficiary
            .instruction
            .accounts
            .get_mut(18)
            .expect("beneficiary account")
            .pubkey = key(98);
        assert_eq!(
            validate_resolution_close_fund_report_v3(&beneficiary),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );

        let mut privilege = exact;
        privilege
            .instruction
            .accounts
            .get_mut(16)
            .expect("certificate account")
            .is_writable = true;
        assert_eq!(
            validate_resolution_close_fund_report_v3(&privilege),
            Err(ResolutionCoreOperatorErrorV3::Frame)
        );
    }

    #[test]
    fn persisted_resolution_receipt_rejoins_exact_retirement_facts() {
        let resolution_program = key(90);
        let source = key(12);
        let terminal_sequence = 9_u64;
        let closure_sequence = terminal_sequence + 1;
        let receipt_key = Pubkey::find_program_address(
            &[
                SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V2,
                source.as_ref(),
                &closure_sequence.to_le_bytes(),
            ],
            &resolution_program,
        )
        .0;
        let receipt_value = SourceClosureReceiptV2 {
            market: key(1).to_bytes(),
            source_state: source.to_bytes(),
            source_material: [31; 32],
            capability_manifest: [32; 32],
            terminal_certificate: key(16).to_bytes(),
            receipt_account: receipt_key.to_bytes(),
            beneficiary: key(18).to_bytes(),
            source_state_digest: [71; 32],
            terminal_certificate_digest: [72; 32],
            funding_set_digest: [73; 32],
            generation: 1,
            terminal_sequence,
            selector: 1,
            refund_lamports: 100,
            closed_at: 11,
        };
        let observation = Observation {
            slot: 5,
            unix_timestamp: 11,
            finality: Finality::Finalized,
        };
        let rent = Rent::default();
        let bytes = receipt_value.to_bytes().expect("receipt").to_vec();
        let receipt = ObservedAccount {
            observation,
            key: receipt_key,
            owner: resolution_program,
            lamports: rent.minimum_balance(bytes.len()),
            executable: false,
            data: bytes,
        };
        let mut rent_data = vec![0; Rent::size_of()];
        {
            let mut lamports = 1;
            let owner = sysvar::ID;
            let mut info = solana_program::account_info::AccountInfo::new(
                &sysvar::rent::ID,
                false,
                false,
                &mut lamports,
                &mut rent_data,
                &owner,
                false,
            );
            rent.to_account_info(&mut info).expect("serialize rent");
        }
        let rent_account = ObservedAccount {
            observation,
            key: sysvar::rent::ID,
            owner: sysvar::ID,
            lamports: 1,
            executable: false,
            data: rent_data,
        };
        let expected = ResolutionRetirementReceiptFactsV3 {
            market: receipt_value.market,
            generation: receipt_value.generation,
            resolution_closure_receipt: receipt_value.receipt_account,
            source_state: receipt_value.source_state,
            source_material: receipt_value.source_material,
            capability_manifest: receipt_value.capability_manifest,
            terminal_certificate: receipt_value.terminal_certificate,
            beneficiary: receipt_value.beneficiary,
            selector: receipt_value.selector,
            terminal_sequence,
            source_state_digest: receipt_value.source_state_digest,
            terminal_certificate_digest: receipt_value.terminal_certificate_digest,
            funding_set_digest: receipt_value.funding_set_digest,
            refund_lamports: receipt_value.refund_lamports,
            closed_at: receipt_value.closed_at,
        };
        assert_eq!(
            authenticate_resolution_retirement_receipt_v3(
                &receipt,
                &rent_account,
                resolution_program,
                expected,
            ),
            Ok(expected)
        );
        let mut substituted = expected;
        substituted.funding_set_digest[0] ^= 1;
        assert_eq!(
            authenticate_resolution_retirement_receipt_v3(
                &receipt,
                &rent_account,
                resolution_program,
                substituted,
            ),
            Err(ResolutionCoreOperatorErrorV3::Terminal)
        );
    }
}
