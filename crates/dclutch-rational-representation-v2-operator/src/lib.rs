#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Chain-derived unsigned instruction construction for exact Rational
//! Representation V2 actions.
//!
//! This crate is deliberately an untrusted projection: it hostile-decodes the
//! canonical descriptor, graph, activation cache, Core Market, Claims, Token,
//! Realm, and Custody records, derives every account identity from their public
//! seed contracts, and emits the exact child instruction. The onchain Claims
//! adapter reauthenticates every observation.

use dclutch_claims_svm::{
    CallerRole, NO_POSITION_REVISION,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    product_basis_terminal_v3::{
        ProductBasisTerminalInputV3, TERMINAL_CANDIDATE_DOMAIN_V3, TERMINAL_COORDINATE_BYTES_V2,
        TERMINAL_COORDINATE_MAGIC_V2, TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
        encode_product_basis_terminal_signed_delta_v3,
    },
    protocol_position_v2::{ProtocolPositionClaimsCapabilitySeedsV2, ProtocolPositionSeedsV2},
    signed_delta_v3::{DeltaDirectionV3, SignedDeltaV3, plan_bytes},
};
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1 as CustodyCallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyRequestV1,
    CustodyVaultSeedsV1, OperationV1,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase as CorePhase};
use dclutch_product_payoff_v2_codec::runtime_v3::{BasisKindV3, ProductBasisV3};
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV3,
    representation_v3::{
        RepresentationRuntimeContextV3, RepresentationRuntimeFrameV3,
        authenticate_product_representation_v3,
    },
};
use dclutch_rational_representation_v2_contract::{
    ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, RATIONAL_REPLAY_BYTES_V2,
    RATIONAL_REPLAY_SEED_V2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    RATIONAL_SHARD_MINT_SEED_V2, RATIONAL_STRUCTURED_CUSTODY_SEED_V2, REQUEST_HEADER_BYTES_V2,
    RationalReplayV2, RepresentationActionV2, RepresentationRequestHeaderV2,
    RepresentationRequestV2,
};
use dclutch_rational_representation_v2_kernel::{
    DescriptorAdmissionV2, REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
    RepresentationDescriptorV2,
    product_v3::{RepresentationAdmissionV3, TerminalScenarioV3},
};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_representation_composition_v3_kernel::{
    CompositionExposureBundleV3, CompositionExposureExecutionExpectedV3, RecordAdmissionV3,
};
use dclutch_token_svm::{AccountState, COption, Mint, TokenAccount, TokenProgram};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{system_program, sysvar};
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;

/// Stable operator refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A required actor, context, program, or content identity was zero.
    ZeroIdentity,
    /// A runtime outcome or asset width differed from its canonical shape.
    InvalidWidth,
    /// A finalized raw/staging record was substituted or not finalized.
    InvalidFinalizedRecord,
    /// The activation cache, release set, or role binding differed.
    InvalidActivation,
    /// The Core Market owner, PDA, state, phase, or descriptor join differed.
    InvalidCoreMarket,
    /// The Claims aggregate or Position identity/revision differed.
    InvalidClaims,
    /// A Mint, Token account, owner, balance, or canonical ATA differed.
    InvalidToken,
    /// A replay address/state/revision differed.
    InvalidReplay,
    /// The immutable descriptor or graph refused exact authentication.
    InvalidRepresentation,
    /// An action carried a noncanonical revision, quantity, or account shape.
    InvalidAction,
    /// A terminal Realm, winner, Custody replay, Vault, or recipient differed.
    InvalidTerminal,
    /// Request construction or its round-trip decode refused.
    InvalidRequest,
    /// A checked allocation or revision arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for unsigned operator construction.
pub type Result<T> = core::result::Result<T, Error>;

/// Chain-observed SVM account projection.
///
/// This value is never an authority of its own. Constructors authenticate the
/// relevant owner, executable flag, canonical address, and exact data parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedAccountV2<'a> {
    /// Observed account address.
    pub key: Pubkey,
    /// Observed account owner.
    pub owner: Pubkey,
    /// Observed lamports.
    pub lamports: u64,
    /// Observed executable flag.
    pub executable: bool,
    /// Complete observed account data.
    pub data: &'a [u8],
}

/// Finalized raw record plus its exact vacant staging cursor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedRecordObservationV2<'a> {
    /// Exact canonical schema identity selecting the Registry PDA domain.
    pub schema_id: [u8; 32],
    /// Registry-owned content-addressed raw record.
    pub raw: ObservedAccountV2<'a>,
    /// System-owned vacant staging cursor.
    pub staging: ObservedAccountV2<'a>,
}

/// Exact immutable records required by the Claims economic/Product join.
///
/// These remain Registry-owned facts. The operator authenticates their
/// content-addressed coordinates and the Claims adapter repeats full semantic
/// Product/LiabilityBasis composition before any effect commits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductEvidenceObservationV2<'a> {
    /// Product-linked LiabilityBasisV2 record selected by Claims.
    pub linked_basis: FinalizedRecordObservationV2<'a>,
    /// Product Runtime V2 graph-root record.
    pub product: FinalizedRecordObservationV2<'a>,
    /// Product-selected runtime result-domain record.
    pub result_domain: FinalizedRecordObservationV2<'a>,
    /// Product-selected exact rational portfolio record.
    pub portfolio: FinalizedRecordObservationV2<'a>,
}

/// One physical runtime coordinate observed from Claims and Token state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetObservationV2<'a> {
    /// Product outcome represented by this row.
    pub outcome: u32,
    /// Claims Position holding materialized native backing.
    pub claims_custody_position: ObservedAccountV2<'a>,
    /// Token-owned canonical shard Mint.
    pub shard_mint: ObservedAccountV2<'a>,
    /// Actor's canonical shard ATA.
    pub actor_shard_account: ObservedAccountV2<'a>,
    /// Canonical Claims-derived Structured custody token account.
    pub structured_custody_account: ObservedAccountV2<'a>,
}

/// Chain-observed per-holder representation replay.
///
/// The data must be decoded by the public Rational replay ABI. It is kept as
/// an account observation rather than a caller-provided revision scalar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayObservationV2<'a> {
    /// Canonically derived replay account and its exact data.
    pub account: ObservedAccountV2<'a>,
}

/// Common chain observations used by every Rational action.
#[derive(Clone, Copy, Debug)]
pub struct RationalObservationV2<'a> {
    /// Registry-authenticated upstream caller role.
    pub caller_role: CallerRoleV2,
    /// Current Registry program.
    pub registry_program: Pubkey,
    /// Complete Registry-owned activation cache.
    pub activation_cache: ObservedAccountV2<'a>,
    /// Finalized immutable Rational descriptor.
    pub descriptor: FinalizedRecordObservationV2<'a>,
    /// Finalized graph selected by the descriptor.
    pub graph: FinalizedRecordObservationV2<'a>,
    /// Exact immutable Product/LiabilityBasis evidence consumed by Claims.
    pub product_evidence: ProductEvidenceObservationV2<'a>,
    /// Current canonical Core Market.
    pub core_market: ObservedAccountV2<'a>,
    /// Current Claims aggregate.
    pub claims_aggregate: ObservedAccountV2<'a>,
    /// Current per-descriptor/actor representation replay.
    pub replay: ReplayObservationV2<'a>,
    /// Token-owned Structured receipt Mint.
    pub receipt_mint: ObservedAccountV2<'a>,
    /// Actor receipt ATA for Structured actions; absent for selected actions.
    pub actor_receipt_account: Option<ObservedAccountV2<'a>>,
    /// Actor Claims Position for Denominate/Reconstitute; otherwise absent.
    pub actor_claims_position: Option<ObservedAccountV2<'a>>,
    /// Exact action-shaped asset observations: N ordered rows for Structured,
    /// one selected row for Denominate/Reconstitute/RedeemTerminal.
    pub assets: &'a [AssetObservationV2<'a>],
    /// Transaction-signing actor and Claims Position owner.
    pub actor: Pubkey,
    /// Complete nonzero upstream request/replay context.
    pub parent_context: [u8; 32],
    /// Chain-observed Rent sysvar used to authenticate finalized records.
    pub rent: &'a Rent,
}

/// Selected-coordinate action input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedActionInputV2 {
    /// Selected Product outcome.
    pub outcome: u32,
    /// Exact native Claims quantity.
    pub quantity: u64,
}

/// Structured full-width action input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredActionInputV2 {
    /// Exact Structured receipt quantity.
    pub quantity: u64,
}

/// Product-terminal payout observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalObservationV2<'a> {
    /// Exact native claim coordinate being redeemed. It need not be the
    /// resolved winner: losing claims redeem at an exact zero payout.
    pub outcome: u32,
    /// Exact terminal shard/native quantity.
    pub quantity: u64,
    /// Finalized immutable Realm selected by Core.
    pub realm: FinalizedRecordObservationV2<'a>,
    /// Finalized Core-owned terminal coordinate for an ordinary graded result;
    /// the Core program placeholder pair for categorical or failure results.
    pub terminal_coordinate: FinalizedRecordObservationV2<'a>,
    /// Current Custody replay account.
    pub custody_replay: ObservedAccountV2<'a>,
    /// Realm-selected collateral Mint.
    pub collateral_mint: ObservedAccountV2<'a>,
    /// Canonical Hoard-principal Vault.
    pub hoard: ObservedAccountV2<'a>,
    /// Actor-owned collateral recipient Token account.
    pub collateral_recipient: ObservedAccountV2<'a>,
}

/// Canonical identities derived for one request asset row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedAssetIdentitiesV2 {
    /// Product outcome represented by the row.
    pub outcome: u32,
    /// Canonical Claims custody owner PDA.
    pub claims_custody_owner: Pubkey,
    /// Canonical Claims custody Position PDA.
    pub claims_custody_position: Pubkey,
    /// Canonical shard Mint PDA.
    pub shard_mint: Pubkey,
    /// Actor shard ATA.
    pub actor_shard_account: Pubkey,
    /// Canonical Claims-derived Structured custody token account.
    pub structured_custody_account: Pubkey,
}

/// Canonical Product-terminal SignedDelta and Custody projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedTerminalIdentitiesV2 {
    /// Finalized Realm content identity.
    pub realm: Pubkey,
    /// Claims caller-authority PDA used for the nested Custody CPI.
    pub custody_caller_authority: Pubkey,
    /// Custody replay PDA.
    pub custody_replay: Pubkey,
    /// Hoard-principal Vault PDA.
    pub hoard: Pubkey,
    /// Custody transfer-authority PDA.
    pub custody_authority: Pubkey,
    /// Exact ProductBasisV3 payout for the selected debit quantity.
    pub payout: u64,
    /// Canonical Claims SignedDeltaV3 packet derived from the final request.
    pub signed_delta_packet: Vec<u8>,
    /// SHA-256 of the canonical SignedDeltaV3 packet.
    pub signed_delta_packet_digest: [u8; 32],
    /// Candidate binding packet, payout, Product admission, and terminal result.
    pub candidate_digest: [u8; 32],
    /// Product/Claims-owned custody context persisted by LBV2 Market.
    pub custody_context: [u8; 32],
    /// Exact typed Custody request. It is absent for zero-payout redemption.
    pub custody_request: Option<CustodyRequestV1>,
}

/// Complete exact unsigned Claims child instruction and derived identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructedInstructionV2 {
    /// Exact unsigned Claims instruction. Its caller-authority meta is a PDA
    /// signer supplied by the selected upstream caller program via CPI.
    pub instruction: Instruction,
    /// SHA-256 of the complete exact request data.
    pub request_digest: [u8; 32],
    /// Canonical Claims representation-authority PDA.
    pub representation_authority: Pubkey,
    /// Canonical per-descriptor/actor replay PDA.
    pub representation_replay: Pubkey,
    /// Canonical Claims aggregate PDA.
    pub claims_aggregate: Pubkey,
    /// Exact action-shaped derived asset identities.
    pub assets: Vec<DerivedAssetIdentitiesV2>,
    /// Product-terminal evidence, absent for all other actions.
    pub terminal: Option<DerivedTerminalIdentitiesV2>,
}

#[derive(Clone, Copy)]
struct RoleProgramsV2 {
    caller: Pubkey,
    caller_programdata: Pubkey,
    claims: Pubkey,
    claims_programdata: Pubkey,
    core: Pubkey,
    core_programdata: Pubkey,
    custody: Pubkey,
    custody_programdata: Pubkey,
}

#[derive(Clone, Copy)]
struct CommonV2<'a> {
    observation: RationalObservationV2<'a>,
    descriptor: RepresentationDescriptorV2<'a>,
    roles: RoleProgramsV2,
    core: CoreState,
    representation_authority: Pubkey,
    representation_replay: Pubkey,
    claims_aggregate: Pubkey,
    claims_basis_id: [u8; 32],
    claims_custody_context: [u8; 32],
    representation_revision: u64,
    claims_market_revision: u64,
    receipt_supply: u64,
    product_record_digest: [u8; 32],
    result_outcome_count: u32,
    representation_admission: RepresentationAdmissionV3,
}

#[derive(Clone, Copy)]
struct ResolvedAssetV2 {
    identities: DerivedAssetIdentitiesV2,
    coefficient: u64,
    shard_supply: u64,
    actor_shards: u64,
    structured_shards: u64,
    custody_position_revision: u64,
}

/// Construct exact Denominate request data and ordered account metas.
pub fn construct_denominate(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
) -> Result<ConstructedInstructionV2> {
    construct_selected(RepresentationActionV2::Denominate, observation, input)
}

/// Construct exact Reconstitute request data and ordered account metas.
pub fn construct_reconstitute(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
) -> Result<ConstructedInstructionV2> {
    construct_selected(RepresentationActionV2::Reconstitute, observation, input)
}

/// Construct exact IssueStructured request data and ordered account metas.
pub fn construct_issue_structured(
    observation: RationalObservationV2<'_>,
    input: StructuredActionInputV2,
) -> Result<ConstructedInstructionV2> {
    construct_structured(RepresentationActionV2::IssueStructured, observation, input)
}

/// Construct exact UnwrapStructured request data and ordered account metas.
pub fn construct_unwrap_structured(
    observation: RationalObservationV2<'_>,
    input: StructuredActionInputV2,
) -> Result<ConstructedInstructionV2> {
    construct_structured(RepresentationActionV2::UnwrapStructured, observation, input)
}

/// Construct exact ProductV3 RedeemTerminal request data and ordered account
/// metas. Any in-domain native claim may be redeemed after resolution; a
/// losing claim produces the same canonical SignedDelta debit and no Custody
/// transfer because its exact payout is zero.
pub fn construct_redeem_terminal(
    observation: RationalObservationV2<'_>,
    terminal: TerminalObservationV2<'_>,
) -> Result<ConstructedInstructionV2> {
    if terminal.quantity == 0 {
        return Err(Error::InvalidAction);
    }
    let common = authenticate_common(observation, RepresentationActionV2::RedeemTerminal)?;
    if common.core.phase != CorePhase::Terminal
        || terminal.outcome >= common.representation_admission.basis_width()
    {
        return Err(Error::InvalidTerminal);
    }
    let assets = authenticate_assets(common, Some(terminal.outcome))?;
    let selected = assets.first().copied().ok_or(Error::InvalidWidth)?;
    let terminal_context = authenticate_terminal_context(common, terminal)?;
    let payout = evaluate_terminal_payout(
        common,
        terminal_context.scenario,
        terminal.outcome,
        terminal.quantity,
    )?;
    let custody_replay_revision = if payout == 0 {
        ABSENT_REVISION
    } else {
        authenticate_positive_custody_replay(common, terminal, terminal_context)?
    };
    let header = request_header(
        common,
        HeaderActionV2 {
            action: RepresentationActionV2::RedeemTerminal,
            quantity: terminal.quantity,
            selected_outcome: terminal.outcome,
            custody_position_revision: selected.custody_position_revision,
            actor_position_revision: NO_POSITION_REVISION,
            custody_replay_revision,
            realm: terminal_context.realm_id.to_bytes(),
            recipient: terminal.collateral_recipient.key.to_bytes(),
        },
    );
    let request_data = encode_request(header, &assets)?;
    let terminal_derived = derive_terminal_after_request(
        common,
        terminal,
        terminal_context,
        &request_data,
        payout,
        selected,
    )?;
    finish_instruction(
        common,
        header,
        request_data,
        assets,
        Some(TerminalFrameV2 {
            observation: terminal,
            derived: terminal_derived,
            token_program: Pubkey::new_from_array(*terminal_context.realm.token_program()),
        }),
    )
}

fn construct_selected(
    action: RepresentationActionV2,
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
) -> Result<ConstructedInstructionV2> {
    if input.quantity == 0
        || !matches!(
            action,
            RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
        )
    {
        return Err(Error::InvalidAction);
    }
    let common = authenticate_common(observation, action)?;
    require_open(common)?;
    let assets = authenticate_assets(common, Some(input.outcome))?;
    let selected = assets.first().copied().ok_or(Error::InvalidWidth)?;
    let actor_revision = authenticate_actor_position(common)?;
    let header = request_header(
        common,
        HeaderActionV2 {
            action,
            quantity: input.quantity,
            selected_outcome: input.outcome,
            custody_position_revision: selected.custody_position_revision,
            actor_position_revision: actor_revision,
            custody_replay_revision: ABSENT_REVISION,
            realm: [0; 32],
            recipient: [0; 32],
        },
    );
    let request_data = encode_request(header, &assets)?;
    finish_instruction(common, header, request_data, assets, None)
}

fn construct_structured(
    action: RepresentationActionV2,
    observation: RationalObservationV2<'_>,
    input: StructuredActionInputV2,
) -> Result<ConstructedInstructionV2> {
    if input.quantity == 0
        || !matches!(
            action,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        )
    {
        return Err(Error::InvalidAction);
    }
    let common = authenticate_common(observation, action)?;
    require_open(common)?;
    let assets = authenticate_assets(common, None)?;
    authenticate_actor_receipt(common)?;
    let header = request_header(
        common,
        HeaderActionV2 {
            action,
            quantity: input.quantity,
            selected_outcome: u32::MAX,
            custody_position_revision: ABSENT_REVISION,
            actor_position_revision: ABSENT_REVISION,
            custody_replay_revision: ABSENT_REVISION,
            realm: [0; 32],
            recipient: [0; 32],
        },
    );
    let request_data = encode_request(header, &assets)?;
    finish_instruction(common, header, request_data, assets, None)
}

fn authenticate_common<'a>(
    observation: RationalObservationV2<'a>,
    action: RepresentationActionV2,
) -> Result<CommonV2<'a>> {
    require_nonzero(observation.actor.to_bytes())?;
    require_nonzero(observation.parent_context)?;
    require_nonzero(observation.registry_program.to_bytes())?;

    let descriptor_id = hash(observation.descriptor.raw.data).to_bytes();
    authenticate_record(
        observation.descriptor,
        observation.registry_program,
        REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
        descriptor_id,
        observation.rent,
    )?;
    let representation_authority = Pubkey::find_program_address(
        &[RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, &descriptor_id],
        &activated_claims_program(observation)?,
    )
    .0;
    let descriptor = RepresentationDescriptorV2::decode(
        observation.descriptor.raw.data,
        DescriptorAdmissionV2 {
            selected_descriptor_id: descriptor_id,
            finalized_descriptor_id: descriptor_id,
            recomputed_descriptor_digest: descriptor_id,
            finalized_descriptor_digest: descriptor_id,
            record_authenticated: true,
            derived_representation_authority: representation_authority.to_bytes(),
            authority_derivation_authenticated: true,
        },
    )
    .map_err(|_| Error::InvalidRepresentation)?;
    authenticate_record(
        observation.graph,
        observation.registry_program,
        dclutch_representation_composition_v3_kernel::COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        descriptor.graph_digest(),
        observation.rent,
    )?;
    let exposure = CompositionExposureBundleV3::decode(
        observation.graph.raw.data,
        RecordAdmissionV3 {
            selected_id: descriptor.graph_id(),
            finalized_id: descriptor.graph_id(),
            recomputed_digest: descriptor.graph_digest(),
            finalized_digest: descriptor.graph_digest(),
            record_authenticated: true,
        },
    )
    .map_err(|_| Error::InvalidRepresentation)?;
    descriptor
        .authenticate_exposure(exposure)
        .map_err(|_| Error::InvalidRepresentation)?;
    for record in [
        observation.product_evidence.linked_basis,
        observation.product_evidence.product,
        observation.product_evidence.result_domain,
        observation.product_evidence.portfolio,
    ] {
        authenticate_observed_record(record, observation.registry_program, observation.rent)?;
    }

    let (roles, release_set) = authenticate_activation(observation, descriptor)?;
    let core = authenticate_core(observation, descriptor, roles.core, release_set)?;
    let claims_aggregate = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            descriptor.market_id().as_slice(),
        ],
        &roles.claims,
    )
    .0;
    let claims_market = LiabilityBasisMarketViewV2::decode(observation.claims_aggregate.data)
        .map_err(|_| Error::InvalidClaims)?;
    if observation.claims_aggregate.key != claims_aggregate
        || observation.claims_aggregate.owner != roles.claims
        || observation.claims_aggregate.executable
        || claims_market.logical_market != descriptor.market_id()
        || claims_market.release_set != descriptor.release_set_id()
        || claims_market.registry_program != observation.registry_program.to_bytes()
        || claims_market.claim_count != descriptor.outcome_count()
        || claims_market.product_instance_id != core.identity.product_id.to_bytes()
        || claims_market.realm_id != core.identity.realm_id.to_bytes()
        || claims_market.generation != core.identity.generation
        || hash(observation.product_evidence.product.raw.data).to_bytes()
            != core.identity.product_record.to_bytes()
    {
        return Err(Error::InvalidClaims);
    }
    let authenticated_product = authenticate_product_representation_observation_v3(
        observation,
        roles.claims,
        core.identity.product_record.to_bytes(),
        descriptor_id,
        claims_market,
    )?;
    if authenticated_product.admission.descriptor_id() != descriptor_id
        || authenticated_product.admission.graph_id() != descriptor.graph_id()
        || authenticated_product.admission.graph_digest() != descriptor.graph_digest()
        || authenticated_product.admission.market_id() != descriptor.market_id()
        || authenticated_product.admission.release_set_id() != descriptor.release_set_id()
        || authenticated_product.admission.semantic_basis_id() != claims_market.basis_id
        || authenticated_product.admission.product_id() != claims_market.product_instance_id
        || authenticated_product.admission.basis_width() != descriptor.outcome_count()
        || authenticated_product.admission.receipt_mint() != descriptor.receipt_mint()
        || authenticated_product.admission.token_program() != descriptor.token_program()
        || authenticated_product.admission.representation_authority()
            != representation_authority.to_bytes()
        || authenticated_product.product_record_digest != core.identity.product_record.to_bytes()
    {
        return Err(Error::InvalidRepresentation);
    }
    if !phases_admit(action, core.phase) {
        return Err(Error::InvalidCoreMarket);
    }

    let representation_replay = Pubkey::find_program_address(
        &[
            RATIONAL_REPLAY_SEED_V2,
            descriptor.descriptor_id().as_slice(),
            observation.actor.as_ref(),
        ],
        &roles.claims,
    )
    .0;
    if observation.replay.account.key != representation_replay
        || observation.replay.account.executable
    {
        return Err(Error::InvalidReplay);
    }
    let representation_revision = authenticate_replay(
        observation.replay,
        roles.claims,
        descriptor.descriptor_id(),
        observation.actor.to_bytes(),
        observation.rent,
    )?;
    let receipt = authenticate_mint(
        observation.receipt_mint,
        Pubkey::new_from_array(descriptor.receipt_mint()),
        Pubkey::new_from_array(descriptor.token_program()),
        representation_authority,
    )?;
    Ok(CommonV2 {
        observation,
        descriptor,
        roles,
        core,
        representation_authority,
        representation_replay,
        claims_aggregate,
        claims_basis_id: claims_market.basis_id,
        claims_custody_context: claims_market.custody_context,
        representation_revision,
        claims_market_revision: claims_market.revision,
        receipt_supply: receipt.supply,
        product_record_digest: authenticated_product.product_record_digest,
        result_outcome_count: authenticated_product.result_outcome_count,
        representation_admission: authenticated_product.admission,
    })
}

#[derive(Clone, Copy)]
struct AuthenticatedProductObservationV3 {
    product_record_digest: [u8; 32],
    result_outcome_count: u32,
    admission: RepresentationAdmissionV3,
}

struct BackingAccountV3 {
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: Vec<u8>,
    executable: bool,
}

impl BackingAccountV3 {
    fn new(observed: ObservedAccountV2<'_>) -> Self {
        Self {
            key: observed.key,
            owner: observed.owner,
            lamports: observed.lamports,
            data: observed.data.to_vec(),
            executable: observed.executable,
        }
    }

    fn info(&mut self) -> AccountInfo<'_> {
        AccountInfo::new(
            &self.key,
            false,
            false,
            &mut self.lamports,
            &mut self.data,
            &self.owner,
            self.executable,
        )
    }
}

fn authenticate_product_representation_observation_v3(
    observation: RationalObservationV2<'_>,
    claims_program: Pubkey,
    product_digest: [u8; 32],
    descriptor_digest: [u8; 32],
    market: LiabilityBasisMarketViewV2,
) -> Result<AuthenticatedProductObservationV3> {
    let evidence = observation.product_evidence;
    let mut product_raw = BackingAccountV3::new(evidence.product.raw);
    let mut product_staging = BackingAccountV3::new(evidence.product.staging);
    let mut domain_raw = BackingAccountV3::new(evidence.result_domain.raw);
    let mut domain_staging = BackingAccountV3::new(evidence.result_domain.staging);
    let mut portfolio_raw = BackingAccountV3::new(evidence.portfolio.raw);
    let mut portfolio_staging = BackingAccountV3::new(evidence.portfolio.staging);
    let mut basis_raw = BackingAccountV3::new(evidence.linked_basis.raw);
    let mut basis_staging = BackingAccountV3::new(evidence.linked_basis.staging);
    let mut descriptor_raw = BackingAccountV3::new(observation.descriptor.raw);
    let mut descriptor_staging = BackingAccountV3::new(observation.descriptor.staging);
    let mut graph_raw = BackingAccountV3::new(observation.graph.raw);
    let mut graph_staging = BackingAccountV3::new(observation.graph.staging);
    let product_raw = product_raw.info();
    let product_staging = product_staging.info();
    let domain_raw = domain_raw.info();
    let domain_staging = domain_staging.info();
    let portfolio_raw = portfolio_raw.info();
    let portfolio_staging = portfolio_staging.info();
    let basis_raw = basis_raw.info();
    let basis_staging = basis_staging.info();
    let descriptor_raw = descriptor_raw.info();
    let descriptor_staging = descriptor_staging.info();
    let graph_raw = graph_raw.info();
    let graph_staging = graph_staging.info();
    let authenticated = authenticate_product_representation_v3(
        &observation.registry_program,
        observation.rent,
        ProductContentId::new(product_digest).map_err(|_| Error::InvalidRepresentation)?,
        ProductContentId::new(descriptor_digest).map_err(|_| Error::InvalidRepresentation)?,
        RepresentationRuntimeContextV3 {
            claims_program,
            market: observation.core_market.key,
            release_set: Pubkey::new_from_array(market.release_set),
            claims_basis_id: ProductContentId::new(market.basis_id)
                .map_err(|_| Error::InvalidRepresentation)?,
            claims_width: market.claim_count,
            receipt_mint: observation.receipt_mint.key,
            token_program: observation.receipt_mint.owner,
        },
        RepresentationRuntimeFrameV3 {
            product: ProductRuntimeFrameV3 {
                product: FinalizedRecordFrameV2 {
                    raw: &product_raw,
                    staging: &product_staging,
                },
                result_domain: FinalizedRecordFrameV2 {
                    raw: &domain_raw,
                    staging: &domain_staging,
                },
                portfolio: FinalizedRecordFrameV2 {
                    raw: &portfolio_raw,
                    staging: &portfolio_staging,
                },
                linked_basis: FinalizedRecordFrameV2 {
                    raw: &basis_raw,
                    staging: &basis_staging,
                },
            },
            descriptor: FinalizedRecordFrameV2 {
                raw: &descriptor_raw,
                staging: &descriptor_staging,
            },
            graph: FinalizedRecordFrameV2 {
                raw: &graph_raw,
                staging: &graph_staging,
            },
        },
    )
    .map_err(|_| Error::InvalidRepresentation)?;
    Ok(AuthenticatedProductObservationV3 {
        product_record_digest: authenticated.product_record_digest.to_bytes(),
        result_outcome_count: authenticated.result_outcome_count,
        admission: authenticated.admission,
    })
}

fn activated_claims_program(observation: RationalObservationV2<'_>) -> Result<Pubkey> {
    let cache = ActivatedExecutionReleaseSetViewV1::decode(observation.activation_cache.data)
        .map_err(|_| Error::InvalidActivation)?;
    Ok(Pubkey::new_from_array(
        cache
            .role(ExecutionRoleV1::Claims)
            .map_err(|_| Error::InvalidActivation)?
            .release()
            .program()
            .to_bytes(),
    ))
}

fn authenticate_activation(
    observation: RationalObservationV2<'_>,
    descriptor: RepresentationDescriptorV2<'_>,
) -> Result<(RoleProgramsV2, [u8; 32])> {
    if observation.activation_cache.owner != observation.registry_program
        || observation.activation_cache.executable
    {
        return Err(Error::InvalidActivation);
    }
    let cache = ActivatedExecutionReleaseSetViewV1::decode(observation.activation_cache.data)
        .map_err(|_| Error::InvalidActivation)?;
    let release = cache
        .execution_release_set_id()
        .map_err(|_| Error::InvalidActivation)?;
    if release.to_bytes() != descriptor.release_set_id() {
        return Err(Error::InvalidActivation);
    }
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release.as_bytes()],
        &observation.registry_program,
    )
    .0;
    if observation.activation_cache.key != expected {
        return Err(Error::InvalidActivation);
    }
    let role = match observation.caller_role {
        CallerRoleV2::Core => ExecutionRoleV1::Core,
        CallerRoleV2::Trading => ExecutionRoleV1::Trading,
    };
    let caller = cache
        .role(role)
        .map_err(|_| Error::InvalidActivation)?
        .release();
    let claims = cache
        .role(ExecutionRoleV1::Claims)
        .map_err(|_| Error::InvalidActivation)?
        .release();
    let core = cache
        .role(ExecutionRoleV1::Core)
        .map_err(|_| Error::InvalidActivation)?
        .release();
    let custody = cache
        .role(ExecutionRoleV1::Custody)
        .map_err(|_| Error::InvalidActivation)?
        .release();
    Ok((
        RoleProgramsV2 {
            caller: Pubkey::new_from_array(caller.program().to_bytes()),
            caller_programdata: Pubkey::new_from_array(caller.programdata()),
            claims: Pubkey::new_from_array(claims.program().to_bytes()),
            claims_programdata: Pubkey::new_from_array(claims.programdata()),
            core: Pubkey::new_from_array(core.program().to_bytes()),
            core_programdata: Pubkey::new_from_array(core.programdata()),
            custody: Pubkey::new_from_array(custody.program().to_bytes()),
            custody_programdata: Pubkey::new_from_array(custody.programdata()),
        },
        release.to_bytes(),
    ))
}

fn authenticate_core(
    observation: RationalObservationV2<'_>,
    descriptor: RepresentationDescriptorV2<'_>,
    core_program: Pubkey,
    release_set: [u8; 32],
) -> Result<CoreState> {
    if observation.core_market.key.to_bytes() != descriptor.market_id()
        || observation.core_market.owner != core_program
        || observation.core_market.executable
    {
        return Err(Error::InvalidCoreMarket);
    }
    let core =
        CoreState::decode(observation.core_market.data).map_err(|_| Error::InvalidCoreMarket)?;
    let derived = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        &core_program,
    )
    .0;
    if derived != observation.core_market.key
        || core.identity.market_id.to_bytes() != descriptor.market_id()
        || core.identity.selected_release_set.to_bytes() != release_set
        || core.identity.registry_program.to_bytes() != observation.registry_program.to_bytes()
    {
        return Err(Error::InvalidCoreMarket);
    }
    Ok(core)
}

fn authenticate_record(
    observation: FinalizedRecordObservationV2<'_>,
    registry_program: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
    rent: &Rent,
) -> Result<()> {
    if observation.schema_id != schema {
        return Err(Error::InvalidFinalizedRecord);
    }
    authenticate_observed_record(observation, registry_program, rent)?;
    if hash(observation.raw.data).to_bytes() != digest {
        return Err(Error::InvalidFinalizedRecord);
    }
    Ok(())
}

fn authenticate_observed_record(
    observation: FinalizedRecordObservationV2<'_>,
    registry_program: Pubkey,
    rent: &Rent,
) -> Result<()> {
    require_nonzero(observation.schema_id)?;
    let digest = hash(observation.raw.data).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &observation.schema_id, &digest],
        &registry_program,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &observation.schema_id, &digest],
        &registry_program,
    )
    .0;
    if observation.raw.key != raw
        || observation.raw.owner != registry_program
        || observation.raw.executable
        || observation.raw.data.is_empty()
        || !rent.is_exempt(observation.raw.lamports, observation.raw.data.len())
        || observation.staging.key != staging
        || observation.staging.owner != system_program::ID
        || observation.staging.executable
        || !observation.staging.data.is_empty()
    {
        return Err(Error::InvalidFinalizedRecord);
    }
    Ok(())
}

fn authenticate_assets(
    common: CommonV2<'_>,
    selected: Option<u32>,
) -> Result<Vec<ResolvedAssetV2>> {
    let expected = selected.map_or_else(
        || usize::try_from(common.descriptor.outcome_count()).map_err(|_| Error::InvalidWidth),
        |_| Ok(1),
    )?;
    if common.observation.assets.len() != expected {
        return Err(Error::InvalidWidth);
    }
    let token_program = Pubkey::new_from_array(common.descriptor.token_program());
    let mut resolved = Vec::with_capacity(expected);
    for (row, observation) in common.observation.assets.iter().copied().enumerate() {
        let outcome = selected.unwrap_or(u32::try_from(row).map_err(|_| Error::InvalidWidth)?);
        if observation.outcome != outcome || outcome >= common.descriptor.outcome_count() {
            return Err(Error::InvalidWidth);
        }
        let outcome_bytes = outcome.to_le_bytes();
        let shard_mint = Pubkey::find_program_address(
            &[
                RATIONAL_SHARD_MINT_SEED_V2,
                common.descriptor.descriptor_id().as_slice(),
                &outcome_bytes,
            ],
            &common.roles.claims,
        )
        .0;
        let custody_owner_seeds = ProtocolPositionClaimsCapabilitySeedsV2::new(
            common.descriptor.descriptor_id(),
            outcome,
        )
        .map_err(|_| Error::InvalidClaims)?;
        let custody_owner =
            Pubkey::find_program_address(&custody_owner_seeds.as_slices(), &common.roles.claims).0;
        let custody_position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(
                common.claims_aggregate.to_bytes(),
                custody_owner.to_bytes(),
            )
            .map_err(|_| Error::InvalidClaims)?
            .as_slices(),
            &common.roles.claims,
        )
        .0;
        let actor_shard = get_associated_token_address_with_program_id(
            &common.observation.actor,
            &shard_mint,
            &token_program,
        );
        let structured = Pubkey::find_program_address(
            &[
                RATIONAL_STRUCTURED_CUSTODY_SEED_V2,
                common.descriptor.descriptor_id().as_slice(),
                &outcome_bytes,
            ],
            &common.roles.claims,
        )
        .0;
        let position =
            LiabilityBasisPositionViewV2::decode(observation.claims_custody_position.data)
                .map_err(|_| Error::InvalidClaims)?;
        if observation.claims_custody_position.key != custody_position
            || observation.claims_custody_position.owner != common.roles.claims
            || observation.claims_custody_position.executable
            || position.market_account != common.claims_aggregate.to_bytes()
            || position.owner != custody_owner.to_bytes()
            || position.basis_id != common.claims_basis_id
            || position.claim_count != common.descriptor.outcome_count()
        {
            return Err(Error::InvalidClaims);
        }
        let custody_position_revision = position.revision;
        let mint = authenticate_mint(
            observation.shard_mint,
            shard_mint,
            token_program,
            common.representation_authority,
        )?;
        let actor = authenticate_token_account(
            observation.actor_shard_account,
            actor_shard,
            token_program,
            shard_mint,
            common.observation.actor,
        )?;
        let structured_account = authenticate_token_account(
            observation.structured_custody_account,
            structured,
            token_program,
            shard_mint,
            common.representation_authority,
        )?;
        resolved.push(ResolvedAssetV2 {
            identities: DerivedAssetIdentitiesV2 {
                outcome,
                claims_custody_owner: custody_owner,
                claims_custody_position: custody_position,
                shard_mint,
                actor_shard_account: actor_shard,
                structured_custody_account: structured,
            },
            coefficient: common
                .descriptor
                .coefficient(outcome)
                .map_err(|_| Error::InvalidRepresentation)?,
            shard_supply: mint.supply,
            actor_shards: actor.amount,
            structured_shards: structured_account.amount,
            custody_position_revision,
        });
    }
    Ok(resolved)
}

fn authenticate_actor_position(common: CommonV2<'_>) -> Result<u64> {
    let observed = common
        .observation
        .actor_claims_position
        .ok_or(Error::InvalidAction)?;
    let expected = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(
            common.claims_aggregate.to_bytes(),
            common.observation.actor.to_bytes(),
        )
        .map_err(|_| Error::InvalidClaims)?
        .as_slices(),
        &common.roles.claims,
    )
    .0;
    let position =
        LiabilityBasisPositionViewV2::decode(observed.data).map_err(|_| Error::InvalidClaims)?;
    if observed.key != expected
        || observed.owner != common.roles.claims
        || observed.executable
        || position.market_account != common.claims_aggregate.to_bytes()
        || position.owner != common.observation.actor.to_bytes()
        || position.basis_id != common.claims_basis_id
        || position.claim_count != common.descriptor.outcome_count()
    {
        return Err(Error::InvalidClaims);
    }
    Ok(position.revision)
}

fn authenticate_actor_receipt(common: CommonV2<'_>) -> Result<()> {
    if common.observation.actor_claims_position.is_some() {
        return Err(Error::InvalidAction);
    }
    let observed = common
        .observation
        .actor_receipt_account
        .ok_or(Error::InvalidAction)?;
    let token_program = Pubkey::new_from_array(common.descriptor.token_program());
    let mint = Pubkey::new_from_array(common.descriptor.receipt_mint());
    let expected = get_associated_token_address_with_program_id(
        &common.observation.actor,
        &mint,
        &token_program,
    );
    authenticate_token_account(
        observed,
        expected,
        token_program,
        mint,
        common.observation.actor,
    )
    .map(|_| ())
}

fn authenticate_mint(
    observed: ObservedAccountV2<'_>,
    expected_key: Pubkey,
    token_program: Pubkey,
    authority: Pubkey,
) -> Result<Mint> {
    if observed.key != expected_key || observed.owner != token_program || observed.executable {
        return Err(Error::InvalidToken);
    }
    let mint = Mint::parse(observed.data).map_err(|_| Error::InvalidToken)?;
    if !mint.is_initialized
        || mint.mint_authority != COption::Some(authority.to_bytes())
        || mint.decimals != 0
        || mint.freeze_authority != COption::None
    {
        return Err(Error::InvalidToken);
    }
    Ok(mint)
}

fn authenticate_token_account(
    observed: ObservedAccountV2<'_>,
    expected_key: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
) -> Result<TokenAccount> {
    if observed.key != expected_key || observed.owner != token_program || observed.executable {
        return Err(Error::InvalidToken);
    }
    let token = TokenAccount::parse(observed.data).map_err(|_| Error::InvalidToken)?;
    if token.mint != mint.to_bytes()
        || token.owner != owner.to_bytes()
        || token.state != AccountState::Initialized
        || token.native_reserve != COption::None
        || token.delegate != COption::None
        || token.delegated_amount != 0
        || token.close_authority != COption::None
    {
        return Err(Error::InvalidToken);
    }
    Ok(token)
}

fn require_open(common: CommonV2<'_>) -> Result<()> {
    if common.core.phase != CorePhase::Open
        || common.observation.actor_receipt_account.is_some()
            == common.observation.actor_claims_position.is_some()
    {
        return Err(Error::InvalidAction);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct HeaderActionV2 {
    action: RepresentationActionV2,
    quantity: u64,
    selected_outcome: u32,
    custody_position_revision: u64,
    actor_position_revision: u64,
    custody_replay_revision: u64,
    realm: [u8; 32],
    recipient: [u8; 32],
}

fn request_header(common: CommonV2<'_>, input: HeaderActionV2) -> RepresentationRequestHeaderV2 {
    let structured = matches!(
        input.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    RepresentationRequestHeaderV2 {
        action: input.action,
        caller_role: common.observation.caller_role,
        release_set: common.descriptor.release_set_id(),
        market: common.descriptor.market_id(),
        graph_id: common.descriptor.graph_id(),
        descriptor_id: common.descriptor.descriptor_id(),
        parent_context: common.observation.parent_context,
        actor: common.observation.actor.to_bytes(),
        receipt_mint: common.descriptor.receipt_mint(),
        receipt_account: if structured {
            common
                .observation
                .actor_receipt_account
                .map_or([0; 32], |value| value.key.to_bytes())
        } else {
            [0; 32]
        },
        representation_authority: common.representation_authority.to_bytes(),
        token_program: common.descriptor.token_program(),
        realm: input.realm,
        collateral_recipient: input.recipient,
        expected_representation_revision: common.representation_revision,
        expected_claims_market_revision: if input.action.uses_claims() {
            common.claims_market_revision
        } else {
            ABSENT_REVISION
        },
        expected_actor_position_revision: input.actor_position_revision,
        expected_custody_position_revision: input.custody_position_revision,
        expected_custody_replay_revision: input.custody_replay_revision,
        generation: common.core.identity.generation,
        quantity: input.quantity,
        denominator: common.descriptor.denominator(),
        expected_receipt_supply: common.receipt_supply,
        outcome_count: common.descriptor.outcome_count(),
        selected_outcome: input.selected_outcome,
        asset_count: if input.action.selected_outcome() {
            1
        } else {
            common.descriptor.outcome_count()
        },
    }
}

fn encode_request(
    header: RepresentationRequestHeaderV2,
    assets: &[ResolvedAssetV2],
) -> Result<Vec<u8>> {
    let row_bytes = assets
        .len()
        .checked_mul(ASSET_BYTES_V2)
        .ok_or(Error::ArithmeticOverflow)?;
    let mut rows = vec![0; row_bytes];
    for (index, asset) in assets.iter().copied().enumerate() {
        let start = index
            .checked_mul(ASSET_BYTES_V2)
            .ok_or(Error::ArithmeticOverflow)?;
        let end = start
            .checked_add(ASSET_BYTES_V2)
            .ok_or(Error::ArithmeticOverflow)?;
        AssetV2 {
            shard_mint: asset.identities.shard_mint.to_bytes(),
            actor_shard_account: asset.identities.actor_shard_account.to_bytes(),
            structured_custody_account: asset.identities.structured_custody_account.to_bytes(),
            claims_custody_owner: asset.identities.claims_custody_owner.to_bytes(),
            coefficient: asset.coefficient,
            expected_shard_supply: asset.shard_supply,
            expected_actor_shards: asset.actor_shards,
            expected_structured_shards: asset.structured_shards,
        }
        .encode_into(rows.get_mut(start..end).ok_or(Error::InvalidWidth)?)
        .map_err(|_| Error::InvalidRequest)?;
    }
    let request = RepresentationRequestV2::new(header, &rows).map_err(|_| Error::InvalidRequest)?;
    let mut output = vec![
        0;
        REQUEST_HEADER_BYTES_V2
            .checked_add(row_bytes)
            .ok_or(Error::ArithmeticOverflow)?
    ];
    request
        .encode_into(&mut output)
        .map_err(|_| Error::InvalidRequest)?;
    let decoded = RepresentationRequestV2::decode(&output).map_err(|_| Error::InvalidRequest)?;
    if decoded.header() != header || decoded.asset_bytes() != rows {
        return Err(Error::InvalidRequest);
    }
    Ok(output)
}

#[derive(Clone, Copy)]
struct TerminalContextV2 {
    realm_id: Pubkey,
    realm: RealmV1,
    scenario: TerminalScenarioV3,
    hoard_before: u64,
}

#[derive(Clone)]
struct TerminalFrameV2<'a> {
    observation: TerminalObservationV2<'a>,
    derived: DerivedTerminalIdentitiesV2,
    token_program: Pubkey,
}

fn authenticate_terminal_context(
    common: CommonV2<'_>,
    terminal: TerminalObservationV2<'_>,
) -> Result<TerminalContextV2> {
    if common.observation.actor_claims_position.is_some()
        || common.observation.actor_receipt_account.is_some()
    {
        return Err(Error::InvalidAction);
    }
    let realm_digest = hash(terminal.realm.raw.data).to_bytes();
    authenticate_record(
        terminal.realm,
        common.observation.registry_program,
        REALM_SCHEMA_RELEASE_ID_V1,
        realm_digest,
        common.observation.rent,
    )?;
    if common.core.identity.realm_id.to_bytes() != realm_digest {
        return Err(Error::InvalidTerminal);
    }
    let realm = RealmV1::decode(terminal.realm.raw.data).map_err(|_| Error::InvalidTerminal)?;
    TokenProgram::parse(*realm.token_program()).map_err(|_| Error::InvalidTerminal)?;
    let scenario = authenticate_terminal_scenario(common, terminal)?;
    let token_program = Pubkey::new_from_array(*realm.token_program());
    let collateral_mint = Pubkey::new_from_array(*realm.collateral_mint());
    if terminal.collateral_mint.key != collateral_mint
        || terminal.collateral_mint.owner != token_program
        || terminal.collateral_mint.executable
        || !Mint::parse(terminal.collateral_mint.data)
            .map_err(|_| Error::InvalidTerminal)?
            .is_initialized
    {
        return Err(Error::InvalidTerminal);
    }
    let recipient = authenticate_token_account(
        terminal.collateral_recipient,
        terminal.collateral_recipient.key,
        token_program,
        collateral_mint,
        common.observation.actor,
    )?;
    let hoard = TokenAccount::parse(terminal.hoard.data).map_err(|_| Error::InvalidTerminal)?;
    if terminal.hoard.owner != token_program
        || terminal.hoard.executable
        || hoard.mint != collateral_mint.to_bytes()
        || hoard.state != AccountState::Initialized
        || hoard.native_reserve != COption::None
        || hoard.delegate != COption::None
        || hoard.delegated_amount != 0
        || hoard.close_authority != COption::None
        || recipient.mint != collateral_mint.to_bytes()
    {
        return Err(Error::InvalidTerminal);
    }
    Ok(TerminalContextV2 {
        realm_id: Pubkey::new_from_array(realm_digest),
        realm,
        scenario,
        hoard_before: hoard.amount,
    })
}

fn authenticate_terminal_scenario(
    common: CommonV2<'_>,
    terminal: TerminalObservationV2<'_>,
) -> Result<TerminalScenarioV3> {
    if common.core.terminal_winner >= common.result_outcome_count {
        return Err(Error::InvalidTerminal);
    }
    match common.representation_admission.basis_kind() {
        BasisKindV3::CategoricalQ1 => {
            require_terminal_coordinate_placeholders(common, terminal)?;
            Ok(TerminalScenarioV3::Categorical(common.core.terminal_winner))
        }
        BasisKindV3::GradedExactComplement => {
            let failure = common
                .result_outcome_count
                .checked_sub(1)
                .ok_or(Error::InvalidTerminal)?;
            if common.core.terminal_winner == failure {
                require_terminal_coordinate_placeholders(common, terminal)?;
                return Ok(TerminalScenarioV3::Failure);
            }
            let digest = common
                .core
                .terminal_receipt
                .ok_or(Error::InvalidTerminal)?
                .to_bytes();
            authenticate_record(
                terminal.terminal_coordinate,
                common.roles.core,
                TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2,
                digest,
                common.observation.rent,
            )?;
            let bytes = terminal.terminal_coordinate.raw.data;
            if bytes.len() != TERMINAL_COORDINATE_BYTES_V2
                || bytes.get(..8) != Some(TERMINAL_COORDINATE_MAGIC_V2.as_slice())
                || bytes.get(8..10) != Some(2_u16.to_le_bytes().as_slice())
                || bytes
                    .get(10..16)
                    .is_none_or(|value| value.iter().any(|byte| *byte != 0))
                || bytes
                    .get(28..32)
                    .is_none_or(|value| value.iter().any(|byte| *byte != 0))
            {
                return Err(Error::InvalidTerminal);
            }
            let numerator = i64::from_le_bytes(
                bytes
                    .get(16..24)
                    .and_then(|value| value.try_into().ok())
                    .ok_or(Error::InvalidTerminal)?,
            );
            let denominator = u32::from_le_bytes(
                bytes
                    .get(24..28)
                    .and_then(|value| value.try_into().ok())
                    .ok_or(Error::InvalidTerminal)?,
            );
            if denominator == 0 {
                return Err(Error::InvalidTerminal);
            }
            Ok(TerminalScenarioV3::Rational {
                numerator: i128::from(numerator),
                denominator: u64::from(denominator),
            })
        }
    }
}

fn require_terminal_coordinate_placeholders(
    common: CommonV2<'_>,
    terminal: TerminalObservationV2<'_>,
) -> Result<()> {
    if terminal.terminal_coordinate.raw.key != common.roles.core
        || terminal.terminal_coordinate.staging.key != common.roles.core
    {
        return Err(Error::InvalidTerminal);
    }
    Ok(())
}

fn evaluate_terminal_payout(
    common: CommonV2<'_>,
    scenario: TerminalScenarioV3,
    outcome: u32,
    quantity: u64,
) -> Result<u64> {
    let basis = ProductBasisV3::decode(common.observation.product_evidence.linked_basis.raw.data)
        .map_err(|_| Error::InvalidRepresentation)?;
    let product_width = usize::try_from(basis.basis_width()).map_err(|_| Error::InvalidWidth)?;
    let claims_width = usize::try_from(common.representation_admission.basis_width())
        .map_err(|_| Error::InvalidWidth)?;
    let mut product_payouts = vec![0_u64; product_width];
    match scenario {
        TerminalScenarioV3::Categorical(selector) => {
            basis.evaluate_categorical(selector, &mut product_payouts)
        }
        TerminalScenarioV3::Rational {
            numerator,
            denominator,
        } => basis.evaluate_rational(numerator, denominator, &mut product_payouts),
        TerminalScenarioV3::Failure => basis.evaluate_failure(&mut product_payouts),
    }
    .map_err(|_| Error::InvalidTerminal)?;
    let exposure = authenticate_operator_exposure(common, basis.basis_width())?;
    let mut translation_scratch = vec![0_u64; claims_width];
    let mut claims_payouts = vec![0_u64; claims_width];
    exposure
        .translate_product_payouts(
            &product_payouts,
            &mut translation_scratch,
            &mut claims_payouts,
        )
        .map_err(|_| Error::InvalidRepresentation)?;
    let selected = usize::try_from(outcome).map_err(|_| Error::InvalidWidth)?;
    claims_payouts
        .get(selected)
        .copied()
        .ok_or(Error::InvalidWidth)?
        .checked_mul(quantity)
        .ok_or(Error::ArithmeticOverflow)
}

fn authenticate_operator_exposure<'a>(
    common: CommonV2<'a>,
    product_width: u32,
) -> Result<CompositionExposureBundleV3<'a>> {
    let admission = common.representation_admission;
    CompositionExposureBundleV3::decode(
        common.observation.graph.raw.data,
        RecordAdmissionV3 {
            selected_id: admission.graph_id(),
            finalized_id: admission.graph_id(),
            recomputed_digest: hash(common.observation.graph.raw.data).to_bytes(),
            finalized_digest: admission.graph_digest(),
            record_authenticated: true,
        },
    )
    .and_then(|bundle| {
        bundle.verify_execution_for(CompositionExposureExecutionExpectedV3 {
            market: admission.market_id(),
            result_domain: admission.result_domain_id(),
            release_set: admission.release_set_id(),
            product_basis: admission.linked_basis_record_digest(),
            representation_basis: admission.semantic_basis_id(),
            product_width,
            representation_width: admission.basis_width(),
        })
    })
    .map_err(|_| Error::InvalidRepresentation)
}

fn authenticate_positive_custody_replay(
    common: CommonV2<'_>,
    terminal: TerminalObservationV2<'_>,
    context: TerminalContextV2,
) -> Result<u64> {
    let replay = CustodyReplayV1::decode(terminal.custody_replay.data)
        .map_err(|_| Error::InvalidTerminal)?;
    if terminal.custody_replay.owner != common.roles.custody
        || terminal.custody_replay.executable
        || terminal.custody_replay.data.len() != CUSTODY_REPLAY_BYTES_V1
        || replay.open_vault_count == 0
        || replay.next_revision == u64::MAX
        || replay.generation != common.core.identity.generation
        || replay.caller_role != CustodyCallerRoleV1::Claims
        || replay.release_set != common.descriptor.release_set_id()
        || replay.market != common.descriptor.market_id()
        || replay.realm != context.realm_id.to_bytes()
        || replay.context != common.claims_custody_context
        || replay.caller_program != common.roles.claims.to_bytes()
    {
        return Err(Error::InvalidTerminal);
    }
    Ok(replay.next_revision)
}

fn derive_terminal_after_request(
    common: CommonV2<'_>,
    terminal: TerminalObservationV2<'_>,
    context: TerminalContextV2,
    request_data: &[u8],
    payout: u64,
    selected: ResolvedAssetV2,
) -> Result<DerivedTerminalIdentitiesV2> {
    let request_digest = hash(request_data).to_bytes();
    let width = common.representation_admission.basis_width();
    let product_width =
        usize::try_from(common.result_outcome_count).map_err(|_| Error::InvalidWidth)?;
    let claims_width = usize::try_from(width).map_err(|_| Error::InvalidWidth)?;
    let neutral =
        SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).map_err(|_| Error::InvalidTerminal)?;
    let mut product_payout_scratch = vec![0_u64; product_width];
    let mut translation_scratch = vec![0_u64; claims_width];
    let mut claims_payout_scratch = vec![0_u64; claims_width];
    let mut aggregate_scratch = vec![neutral; claims_width];
    let mut packet = vec![0_u8; plan_bytes(width, 1, 1).map_err(|_| Error::InvalidTerminal)?];
    let planner_payout = encode_product_basis_terminal_signed_delta_v3(
        ProductBasisTerminalInputV3 {
            product_basis_bytes: common.observation.product_evidence.linked_basis.raw.data,
            representation: common.representation_admission,
            composition_exposure_bytes: common.observation.graph.raw.data,
            composition_exposure_admission: RecordAdmissionV3 {
                selected_id: common.representation_admission.graph_id(),
                finalized_id: common.representation_admission.graph_id(),
                recomputed_digest: hash(common.observation.graph.raw.data).to_bytes(),
                finalized_digest: common.representation_admission.graph_digest(),
                record_authenticated: true,
            },
            product_record_digest: common.product_record_digest,
            market_account: common.claims_aggregate.to_bytes(),
            market_bytes: common.observation.claims_aggregate.data,
            position_bytes: common
                .observation
                .assets
                .first()
                .ok_or(Error::InvalidWidth)?
                .claims_custody_position
                .data,
            owner: selected.identities.claims_custody_owner.to_bytes(),
            request_id: request_digest,
            caller_role: match common.observation.caller_role {
                CallerRoleV2::Core => CallerRole::Core,
                CallerRoleV2::Trading => CallerRole::Trading,
            },
            terminal: context.scenario,
            claim_index: terminal.outcome,
            quantity: terminal.quantity,
            expected_generation: common.core.identity.generation,
            expected_market_revision: common.claims_market_revision,
            expected_position_revision: selected.custody_position_revision,
            hoard_before: context.hoard_before,
        },
        &mut product_payout_scratch,
        &mut translation_scratch,
        &mut claims_payout_scratch,
        &mut aggregate_scratch,
        &mut packet,
    )
    .map_err(|_| Error::InvalidTerminal)?;
    if planner_payout != payout {
        return Err(Error::InvalidTerminal);
    }
    let packet_digest = hash(&packet).to_bytes();
    let candidate_digest = hashv(&[
        TERMINAL_CANDIDATE_DOMAIN_V3,
        &packet_digest,
        &payout.to_le_bytes(),
        &common.representation_admission.to_bytes(),
        &common.core.terminal_winner.to_le_bytes(),
    ])
    .to_bytes();
    let expected_revision = if payout == 0 {
        0
    } else {
        RepresentationRequestV2::decode(request_data)
            .map_err(|_| Error::InvalidRequest)?
            .header()
            .expected_custody_replay_revision
    };
    let mut custody_request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CustodyCallerRoleV1::Claims,
        source_compartment: CompartmentV1::HoardPrincipal,
        destination_compartment: CompartmentV1::External,
        release_set: common.descriptor.release_set_id(),
        market: common.descriptor.market_id(),
        realm: context.realm_id.to_bytes(),
        context: common.claims_custody_context,
        caller_program: common.roles.claims.to_bytes(),
        semantic: ContextV1 {
            candidate: candidate_digest,
            source_owner: [0; 32],
            destination_owner: common.observation.actor.to_bytes(),
            order: [0; 32],
            parent_request_digest: request_digest,
            order_nonce: common.representation_revision,
            generation: common.core.identity.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source: common.descriptor.market_id(),
        destination: terminal.collateral_recipient.key.to_bytes(),
        // The Hoard Vault's namespace is the aggregate's persisted
        // `custody_context`, the same coordinate `context` above already uses.
        // Naming the Market here made this constructor build a request the
        // program refuses for every Market not founded at its own address --
        // which is every Market the atomic founding creates.
        source_vault_context: common.claims_custody_context,
        destination_vault_context: [0; 32],
        mint: *context.realm.collateral_mint(),
        token_program: *context.realm.token_program(),
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?,
        amount: payout.max(1),
        rent_lamports: 0,
    };
    let hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::from_request(custody_request, true).as_slices(),
        &common.roles.custody,
    )
    .0;
    custody_request.source = hoard.to_bytes();
    custody_request
        .validate()
        .map_err(|_| Error::InvalidTerminal)?;
    let custody_bytes = custody_request
        .to_bytes()
        .map_err(|_| Error::InvalidTerminal)?;
    let custody_digest = hash(&custody_bytes).to_bytes();
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            custody_request.release_set,
            custody_request.market,
            ExecutionRoleV1::Claims,
            custody_request.context,
            custody_digest,
        )
        .map_err(|_| Error::InvalidTerminal)?
        .as_slices(),
        &common.roles.claims,
    )
    .0;
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(custody_request).as_slices(),
        &common.roles.custody,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(custody_request).as_slices(),
        &common.roles.custody,
    )
    .0;
    if terminal.hoard.key != hoard {
        return Err(Error::InvalidTerminal);
    }
    let token_program = Pubkey::new_from_array(custody_request.token_program);
    let mint = Pubkey::new_from_array(custody_request.mint);
    authenticate_token_account(
        terminal.hoard,
        hoard,
        token_program,
        mint,
        custody_authority,
    )?;
    let (custody_caller_authority, custody_replay, custody_request) = if payout == 0 {
        if terminal.custody_replay.key != common.roles.custody {
            return Err(Error::InvalidTerminal);
        }
        (common.roles.claims, common.roles.custody, None)
    } else {
        if terminal.custody_replay.key != replay {
            return Err(Error::InvalidTerminal);
        }
        custody_request.amount = payout;
        custody_request
            .validate()
            .map_err(|_| Error::InvalidTerminal)?;
        (caller, replay, Some(custody_request))
    };
    Ok(DerivedTerminalIdentitiesV2 {
        realm: context.realm_id,
        custody_caller_authority,
        custody_replay,
        hoard,
        custody_authority,
        payout,
        signed_delta_packet: packet,
        signed_delta_packet_digest: packet_digest,
        candidate_digest,
        custody_context: common.claims_custody_context,
        custody_request,
    })
}

fn finish_instruction(
    common: CommonV2<'_>,
    header: RepresentationRequestHeaderV2,
    data: Vec<u8>,
    assets: Vec<ResolvedAssetV2>,
    terminal: Option<TerminalFrameV2<'_>>,
) -> Result<ConstructedInstructionV2> {
    let request_digest = hash(&data).to_bytes();
    let role = match header.caller_role {
        CallerRoleV2::Core => ExecutionRoleV1::Core,
        CallerRoleV2::Trading => ExecutionRoleV1::Trading,
    };
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            header.release_set,
            header.market,
            role,
            header.parent_context,
            request_digest,
        )
        .map_err(|_| Error::InvalidRequest)?
        .as_slices(),
        &common.roles.caller,
    )
    .0;
    let structured = matches!(
        header.action,
        RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
    );
    let physical_account_count = RepresentationRequestV2::decode(&data)
        .and_then(RepresentationRequestV2::physical_account_count)
        .map_err(|_| Error::InvalidRequest)?;
    let mut metas = Vec::with_capacity(physical_account_count);
    metas.extend([
        AccountMeta::new_readonly(caller, true),
        AccountMeta::new_readonly(common.roles.caller, false),
        AccountMeta::new_readonly(common.roles.caller_programdata, false),
        AccountMeta::new_readonly(common.observation.actor, true),
        AccountMeta::new_readonly(common.representation_authority, false),
        AccountMeta::new_readonly(common.observation.descriptor.raw.key, false),
        AccountMeta::new_readonly(common.observation.descriptor.staging.key, false),
        AccountMeta::new_readonly(common.observation.graph.raw.key, false),
        AccountMeta::new_readonly(common.observation.graph.staging.key, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new(common.representation_replay, false),
        if header.action.uses_claims() {
            AccountMeta::new(common.claims_aggregate, false)
        } else {
            AccountMeta::new_readonly(common.claims_aggregate, false)
        },
        AccountMeta::new_readonly(common.observation.activation_cache.key, false),
        AccountMeta::new_readonly(common.roles.claims, false),
        AccountMeta::new_readonly(common.roles.claims_programdata, false),
        AccountMeta::new_readonly(common.observation.registry_program, false),
        AccountMeta::new_readonly(common.observation.core_market.key, false),
        AccountMeta::new_readonly(common.roles.core, false),
        AccountMeta::new_readonly(common.roles.core_programdata, false),
        if structured {
            AccountMeta::new(common.observation.receipt_mint.key, false)
        } else {
            AccountMeta::new_readonly(common.observation.receipt_mint.key, false)
        },
        if structured {
            AccountMeta::new(
                common
                    .observation
                    .actor_receipt_account
                    .ok_or(Error::InvalidAction)?
                    .key,
                false,
            )
        } else {
            AccountMeta::new_readonly(common.roles.claims, false)
        },
        AccountMeta::new_readonly(Pubkey::new_from_array(header.token_program), false),
        if matches!(
            header.action,
            RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
        ) {
            AccountMeta::new(
                common
                    .observation
                    .actor_claims_position
                    .ok_or(Error::InvalidAction)?
                    .key,
                false,
            )
        } else {
            AccountMeta::new_readonly(common.roles.claims, false)
        },
        AccountMeta::new_readonly(
            common.observation.product_evidence.linked_basis.raw.key,
            false,
        ),
        AccountMeta::new_readonly(
            common.observation.product_evidence.linked_basis.staging.key,
            false,
        ),
        AccountMeta::new_readonly(common.observation.product_evidence.product.raw.key, false),
        AccountMeta::new_readonly(
            common.observation.product_evidence.product.staging.key,
            false,
        ),
        AccountMeta::new_readonly(
            common.observation.product_evidence.result_domain.raw.key,
            false,
        ),
        AccountMeta::new_readonly(
            common
                .observation
                .product_evidence
                .result_domain
                .staging
                .key,
            false,
        ),
        AccountMeta::new_readonly(common.observation.product_evidence.portfolio.raw.key, false),
        AccountMeta::new_readonly(
            common.observation.product_evidence.portfolio.staging.key,
            false,
        ),
    ]);
    for asset in &assets {
        let selected = header.action.selected_outcome();
        metas.extend([
            if selected {
                AccountMeta::new(asset.identities.claims_custody_position, false)
            } else {
                AccountMeta::new_readonly(asset.identities.claims_custody_position, false)
            },
            if selected {
                AccountMeta::new(asset.identities.shard_mint, false)
            } else {
                AccountMeta::new_readonly(asset.identities.shard_mint, false)
            },
            AccountMeta::new(asset.identities.actor_shard_account, false),
            if structured {
                AccountMeta::new(asset.identities.structured_custody_account, false)
            } else {
                AccountMeta::new_readonly(asset.identities.structured_custody_account, false)
            },
        ]);
    }
    let terminal_identities = if let Some(frame) = terminal {
        metas.extend([
            AccountMeta::new_readonly(frame.derived.custody_caller_authority, false),
            AccountMeta::new_readonly(common.roles.custody, false),
            AccountMeta::new_readonly(common.roles.custody_programdata, false),
            AccountMeta::new_readonly(frame.observation.terminal_coordinate.raw.key, false),
            AccountMeta::new_readonly(frame.observation.terminal_coordinate.staging.key, false),
            AccountMeta::new_readonly(frame.observation.realm.raw.key, false),
            AccountMeta::new_readonly(frame.observation.realm.staging.key, false),
            AccountMeta::new(frame.derived.custody_replay, false),
            AccountMeta::new_readonly(frame.observation.collateral_mint.key, false),
            AccountMeta::new(frame.derived.hoard, false),
            AccountMeta::new(frame.observation.collateral_recipient.key, false),
            AccountMeta::new_readonly(frame.derived.custody_authority, false),
            AccountMeta::new_readonly(frame.token_program, false),
        ]);
        Some(frame.derived)
    } else {
        None
    };
    let identities = assets
        .iter()
        .map(|value| value.identities)
        .collect::<Vec<_>>();
    if metas.len() != physical_account_count {
        return Err(Error::InvalidRequest);
    }
    Ok(ConstructedInstructionV2 {
        instruction: Instruction {
            program_id: common.roles.claims,
            accounts: metas,
            data,
        },
        request_digest,
        representation_authority: common.representation_authority,
        representation_replay: common.representation_replay,
        claims_aggregate: common.claims_aggregate,
        assets: identities,
        terminal: terminal_identities,
    })
}

fn phases_admit(action: RepresentationActionV2, core: CorePhase) -> bool {
    match action {
        RepresentationActionV2::Denominate
        | RepresentationActionV2::Reconstitute
        | RepresentationActionV2::IssueStructured
        | RepresentationActionV2::UnwrapStructured => core == CorePhase::Open,
        RepresentationActionV2::RedeemTerminal => core == CorePhase::Terminal,
    }
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if value.iter().all(|byte| *byte == 0) {
        Err(Error::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn authenticate_replay(
    observation: ReplayObservationV2<'_>,
    claims_program: Pubkey,
    descriptor: [u8; 32],
    actor: [u8; 32],
    rent: &Rent,
) -> Result<u64> {
    if observation.account.owner == claims_program {
        let replay = RationalReplayV2::decode(observation.account.data)
            .and_then(|value| value.authenticate(descriptor, actor))
            .map_err(|_| Error::InvalidReplay)?;
        if replay.revision() == u64::MAX {
            return Err(Error::InvalidReplay);
        }
        return Ok(replay.revision());
    }
    if observation.account.owner == system_program::ID
        && !observation.account.executable
        && observation.account.data.is_empty()
        && observation.account.lamports >= rent.minimum_balance(RATIONAL_REPLAY_BYTES_V2)
    {
        return Ok(0);
    }
    Err(Error::InvalidReplay)
}
