//! Chain-derived Core admission of one terminal Resolution result.
//!
//! This is the missing operator edge between an already-consumed real-provider
//! update and the canonical Core terminal state.  The builder treats the
//! Registry activation cache, current Loader V3 deployments, Product graph,
//! terminal Source state, funding accounts, and certificate as authorities. It
//! emits only an unsigned permissionless instruction.

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
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SourceMaterialV2, SourceResolutionPhaseV1,
    SourceResolutionStateV2,
};
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program};

use crate::{
    Finality, Observation, ObservedAccount, foundation,
    product_graph_observation_v3::{
        FinalizedProductGraphAccountsV3, authenticate_product_graph_observation_v3,
    },
};

/// Exact account count consumed by Core and Resolution for terminal admission.
pub const RESOLUTION_ADMIT_TERMINAL_ACCOUNT_COUNT_V3: usize = 24;

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
    let rent = foundation::decode_rent(&snapshot.rent_sysvar)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
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
    let rent = foundation::decode_rent(&snapshot.rent_sysvar)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
    let clock = crate::verticals::decode_clock(&snapshot.clock_sysvar)
        .map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
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
    let rent =
        foundation::decode_rent(rent_sysvar).map_err(|_| ResolutionCoreOperatorErrorV3::Record)?;
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
    if !(indices[0] < indices[1] && indices[1] < indices[2]) {
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
    if !(indices[0] < indices[1] && indices[1] < indices[2]) {
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

fn identity(bytes: [u8; 32]) -> Result<Identity, ResolutionCoreOperatorErrorV3> {
    Identity::new(bytes).map_err(|_| ResolutionCoreOperatorErrorV3::Encoding)
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
