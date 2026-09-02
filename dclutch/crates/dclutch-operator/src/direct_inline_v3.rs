//! Chain-derived Direct V3 inline execution construction.
//!
//! This host-only adapter joins the canonical action-selected Direct artifact
//! bundle, expands the authenticated AccountProfile account space, and emits
//! the mandatory compute limit plus adjacent native-Ed25519 and Trading batch. It never performs
//! RPC, signs maker material, signs a transaction, or submits one.

use crate::{
    Finality, Observation, ObservedAccount,
    observation::{FinalizedRecordProof, authenticate_finalized_record, decode_rent},
    product_graph_observation_v3::{
        AuthenticatedProductGraphObservationV3, FinalizedProductGraphAccountsV3,
        authenticate_product_graph_observation_v3,
    },
};
use dclutch_account_profile_contract::v2::PhysicalAccountDataGeometryV2;
use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3, HOT_ACTIVATION_CACHE_ACCOUNT_V3,
        HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_STAGING_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_MANIFEST_RAW_ACCOUNT_V3, HOT_MANIFEST_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
        HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_STRATEGY_STAGING_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_TRANSITION_STAGING_ACCOUNT_V3, HotBumpHintsV1, HotExecutionEnvelopeV3,
    },
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
    v4::{
        ArtifactReferenceV4, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
    },
};
use dclutch_custody_contract::{CallerRoleV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1};
use dclutch_direct_codec::{
    artifacts_v4::{
        DirectArtifactBundleV4, DirectArtifactBytesV4, DirectArtifactSelectionV4,
        authenticate_direct_artifacts_v4,
    },
    execution_v3::{
        DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3, DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3,
        DIRECT_REGISTRATION_REQUEST_BYTES_V3, DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        DirectExecutionActionV3, DirectExecutionRequestV3, DirectRegistrationRequestV3,
        DirectSignedTerminalRequestV3, encode_header_v3, native_signature_count_v3,
    },
    native_evidence_v3::{
        DIRECT_NATIVE_EVIDENCE_BYTES_V3, DirectNativeEvidenceContainerV3,
        direct_native_evidence_bytes_v3,
        encode_direct_headerless_registry_native_evidence_many_v4_atomic,
        encode_direct_native_evidence_many_v3_atomic, encode_direct_native_evidence_v3_atomic,
    },
    registered_requests_v4::{
        encode_direct_empty_action_request_v3_atomic,
        encode_direct_registered_cancel_request_v3_atomic,
        encode_direct_registration_request_v3_atomic,
    },
    successor::{DirectCoordinatesV1, MakerReplaySeedsV1},
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, Phase, STATE_BYTES};
use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, semantic_basis_preimage_v3},
};
use dclutch_product_runtime_v2::ResultDomainV2;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
    ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::ExecutionRoleV1;
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, ed25519_program, sysvar};

use crate::versioned::{VersionedMessagePlanV0, compile_v0_message};

pub use dclutch_direct_codec::execution_v3::DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3;

/// Mine the bumps the hot route would otherwise search for, off chain.
///
/// Every one of these is a `find_program_address` the PROGRAM used to run, at
/// 1,500 CU per rejected candidate, on a depth drawn from the participant keys.
/// Run here it costs the caller nothing anybody measures, and the program
/// reproduces each address with `create_program_address` and refuses unless it
/// reproduces the account it was handed. See `HotBumpHintsV1`.
///
/// The two child caller-authority slots are NOT filled here. Their seeds end in
/// a digest over each child's projected request, which this builder does not
/// compute; `derive_direct_inline_child_authorities_v3` does, and reports them
/// on `DirectInlineChildAuthoritiesV3`. A slot left zero searches, so a caller
/// that has not projected its children is correct and merely slower.
fn direct_hot_bump_hints_v1(
    state: &DirectInlineHotStateV3,
    trading_program: &Pubkey,
) -> Result<HotBumpHintsV1, Error> {
    let market_account = &fixed_account(state, HOT_MARKET_ACCOUNT_V3)?.account;
    let core_program = fixed_account(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?
        .account
        .key;
    let market = CoreState::decode(&market_account.data).map_err(|_| Error::ArtifactMismatch)?;
    let market_bump = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &core_program,
    )
    .1;
    let root_account = &fixed_account(state, HOT_ROOT_ACCOUNT_V3)?.account;
    let root = CapabilityRootHeaderV1::decode(
        root_account
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(Error::ArtifactMismatch)?,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    let root_bump = Pubkey::find_program_address(&root.seeds().as_slices(), trading_program).1;
    Ok(HotBumpHintsV1 {
        market: market_bump,
        root: root_bump,
        ..HotBumpHintsV1::ABSENT
    })
}

/// Add the two maker-replay bumps an InlineOrdinary lifecycle creates.
///
/// Seller then buyer, because that is the order the lifecycle materializes
/// them in and the slot an account owns is its position in that order. A
/// mis-ordered pair is not a hazard, only a slower trade: each slot is
/// reproduced against the account the frame supplies and a swap refuses.
fn direct_inline_hot_bump_hints_v1(
    state: &DirectInlineHotStateV3,
    trading_program: &Pubkey,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
) -> Result<HotBumpHintsV1, Error> {
    let market_account = &fixed_account(state, HOT_MARKET_ACCOUNT_V3)?.account;
    let coordinates = DirectCoordinatesV1::new(market_account.key.to_bytes(), state.generation)
        .map_err(|_| Error::ArtifactMismatch)?;
    let mut lifecycle = [0_u8; 2];
    let mut buyer_maker_root = Pubkey::default();
    for (slot, maker) in lifecycle.iter_mut().zip([seller.maker, buyer.maker]) {
        let seeds = MakerReplaySeedsV1::new(coordinates, maker.to_bytes())
            .map_err(|_| Error::ArtifactMismatch)?;
        let (address, bump) = Pubkey::find_program_address(&seeds.as_slices(), trading_program);
        *slot = bump;
        buyer_maker_root = address;
    }
    // The two addresses Custody derives for ITSELF and can carry from nowhere:
    // its replay, whose context is the buyer's maker-replay root, and its
    // transfer authority. Both run through the Market, so both move with the
    // participant keys; neither can be stored, because CUSTODY_REPLAY_BYTES_V1
    // is exactly packed and Lean-emitted and the replay's own bump would have
    // to be written before the account exists. Mined here and relayed after the
    // Custody child request.
    // Custody is not in the hot fixed frame; the Market's activation cache is,
    // and it names the release set's Custody deployment.
    let activation = &fixed_account(state, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.account;
    let custody_program = Pubkey::new_from_array(
        ActivatedExecutionReleaseSetViewV1::decode(&activation.data)
            .map_err(|_| Error::ArtifactMismatch)?
            .role(ExecutionRoleV1::Custody)
            .map_err(|_| Error::ArtifactMismatch)?
            .release()
            .program()
            .to_bytes(),
    );
    let child_relay = [
        Pubkey::find_program_address(
            &CustodyReplaySeedsV1::new(
                market_account.key.to_bytes(),
                state.release_set,
                CallerRoleV1::Trading,
                buyer_maker_root.to_bytes(),
            )
            .as_slices(),
            &custody_program,
        )
        .1,
        Pubkey::find_program_address(
            &CustodyAuthoritySeedsV1::new(market_account.key.to_bytes(), state.release_set)
                .as_slices(),
            &custody_program,
        )
        .1,
    ];
    Ok(HotBumpHintsV1 {
        lifecycle,
        child_relay,
        ..direct_hot_bump_hints_v1(state, trading_program)?
    })
}

/// Direct Hot cannot execute within Solana's default transaction CU allocation.
pub const DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1: u32 = 1_400_000;

/// Where the Hot instruction sits in a SIGNED top-level Direct transaction.
///
/// Compute limit, heap frame, native Ed25519 evidence, Trading. The native
/// evidence names the index it expects to find the signed Hot instruction at,
/// so this constant is what the builder encodes and what the validator
/// re-encodes to authenticate a caller's evidence -- one name rather than the
/// bare `3` that would have to agree in three places.
pub const DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1: u16 = 3;

/// Where the Hot instruction sits in an UNSIGNED top-level Direct transaction.
///
/// Compute limit, heap frame, Trading: no evidence instruction, so everything
/// after the grant shifts down one. The grant itself is NOT optional here --
/// the heap refusal is a property of the route, not of whether makers signed,
/// because both arms take the same Registry reauthentication path.
pub const DIRECT_HOT_UNSIGNED_TRADING_INSTRUCTION_INDEX_V1: usize = 2;

/// Neither can the capability seal, which authenticates one finalized registry
/// proof and hashes one record body per role before writing the closure. It
/// took 789,336 CU on public devnet over the four-entry Direct release set, so
/// the 200,000 default was never going to hold it, and the cost grows with the
/// set. It declares the same ceiling as Hot rather than that reading plus a
/// guessed margin: both are one-shot composed routes where a limit set too low
/// is a transaction that can never be replayed.
pub const DIRECT_SEAL_COMPUTE_UNIT_LIMIT_V1: u32 = 1_400_000;

/// One exact detached maker signature and its canonical signed intent.
///
/// DECLARED IN `dclutch-direct-ticket`, not here, and re-exported so this
/// module's path is unchanged for every caller. The struct the ticket author
/// signs into and the struct this builder consumes must be the same struct or
/// they will drift, and the author is the one that cannot afford to depend on
/// this crate: it is called by the released read-only CLI, which links no
/// program. The dependency therefore points down, and this crate takes the ticket
/// crate with `default-features = false` -- no signer comes in with it, so the
/// sentence at the top of this file stays true.
pub use dclutch_direct_ticket::SignedDirectIntentV3;

/// One same-finalized account plus the privileges requested by the transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedAccountMetaV3 {
    /// Exact finalized account observation.
    pub account: ObservedAccount,
    /// Whether the transaction requests signer privilege.
    pub is_signer: bool,
    /// Whether the transaction requests writable privilege.
    pub is_writable: bool,
}

impl ObservedAccountMetaV3 {
    fn meta(&self) -> AccountMeta {
        AccountMeta {
            pubkey: self.account.key,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}

/// Checked-release evidence that the selected Trading artifact implements the
/// common V3 hot outer.
///
/// This value is not a hard-coded client constant. A chain/release checker must
/// construct it only after the selected immutable ArtifactRelease and current
/// Loader observations match a user-supplied checked manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedHotOuterReleaseV3 {
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact immutable Trading ArtifactRelease identity.
    pub artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized authority and exact physical account projection for one hot
/// Direct instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotStateV3 {
    /// Exact 38-account family-neutral prefix in canonical ABI order.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Exact disposition-selected ExecutionStrategy account suffix.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// Canonically packed AccountProfile physical representatives, including
    /// the capability root/config/Product/portfolio/linked-basis prefix. The
    /// common Hot adapter expands every logical route alias from this vector;
    /// those five injected representatives are not appended a second time.
    pub runtime_accounts: Vec<ObservedAccountMetaV3>,
    /// Immutable execution release-set content identity selected by Market.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Trusted Clock slot used for an exact economic preview.
    pub clock_slot: u64,
    /// Lowest finalized slot accepted for this construction attempt.
    pub minimum_finalized_slot: u64,
    /// Checked current hot outer, absent while the common entrypoint is not an
    /// accepted immutable release.
    pub hot_outer: Option<CheckedHotOuterReleaseV3>,
}

/// Action-neutral Hot state selected from one finalized Direct capability.
///
/// The existing inline name remains source-compatible, but the state itself
/// has never carried inline-only account authority: its exact AccountProfile
/// and CapabilityProgramSet select the action-specific runtime geometry.
pub type DirectHotStateV4 = DirectInlineHotStateV3;

/// Exact economic preview derived from immutable Direct config and the request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineEconomicPreviewV3 {
    /// Claims transferred from seller to buyer.
    pub claim_transfer: u64,
    /// Exact gross collateral at the immutable price scale.
    pub gross_collateral: u64,
    /// Gross less the seller-side floor fee.
    pub seller_net_collateral_credit: u64,
    /// Gross plus the buyer-side floor fee.
    pub buyer_collateral_debit: u64,
    /// Sum of seller-withheld and buyer-added floor fees.
    pub total_fee_transfer: u64,
}

/// Complete unsigned adjacent-evidence execution material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotReportV3 {
    /// Compute limit, heap frame, then native Ed25519 immediately followed by
    /// Trading.
    ///
    /// The heap grant sits at index 1, ahead of the evidence, and both ends of
    /// that position are forced. It cannot be appended: the runtime clears
    /// return data at the start of every top-level instruction, so a trailing
    /// ComputeBudget instruction erases the commit-last ACK the Hot execution
    /// just produced. It cannot be omitted either: this route makes two
    /// Registry reauthentication CPIs a continuation never makes, and Trading
    /// refuses it by name with `TradingSbfError::HeapFrame` when the grant did
    /// not arrive.
    pub instructions: [Instruction; 4],
    /// Complete exact HotExecutionEnvelopeV3 plus Direct request bytes.
    pub hot_instruction_data: Vec<u8>,
    /// Same finalized observation selecting every physical account.
    pub observation: Observation,
    /// Schema of the action-selected CapabilityProgramV4 descriptor.
    pub selected_program_schema: [u8; 32],
    /// Action-selected CapabilityProgramV4 content digest.
    pub selected_program: [u8; 32],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
    /// Wallet keys which the Trading instruction requires to sign.
    pub required_instruction_signers: Vec<Pubkey>,
    /// Exact economic preview; onchain execution remains authoritative.
    pub preview: DirectInlineEconomicPreviewV3,
}

/// Complete unsigned generic-Hot material for one selected Direct action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectHotReportV4 {
    /// Compute limit, then adjacent native evidence and Trading for signed actions.
    pub instructions: Vec<Instruction>,
    /// Complete exact HotExecutionEnvelopeV3 plus action request bytes.
    pub hot_instruction_data: Vec<u8>,
    /// Same finalized observation selecting every physical account.
    pub observation: Observation,
    /// Exact selected Direct action.
    pub action: DirectExecutionActionV3,
    /// CapabilityProgramV4 schema selected by the finalized SetV2.
    pub selected_program_schema: [u8; 32],
    /// Exact selected CapabilityProgramV4 digest.
    pub selected_program: [u8; 32],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Digest of the checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
    /// Wallet keys which the Trading instruction itself requires to sign.
    pub required_instruction_signers: Vec<Pubkey>,
}

/// Exact unsigned Direct v0 transaction and its signer/provenance report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotTransactionPlanV3 {
    /// Packet-safe v0 message compiled through the sole canonical LUT.
    pub message: VersionedMessagePlanV0,
    /// Exact eventual wallet signer order, beginning with the fee payer.
    pub required_signers: Vec<Pubkey>,
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Schema of the action-selected CapabilityProgramV4 descriptor.
    pub selected_program_schema: [u8; 32],
    /// Action-selected CapabilityProgramV4 content digest.
    pub selected_program: [u8; 32],
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Stable refusal from canonical Direct transaction routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineTransactionErrorV3 {
    /// Payer, report, or LUT did not share one finalized observation.
    Snapshot,
    /// LUT bytes were not the one exact canonical address sequence.
    LookupTable,
    /// Instruction signer reporting differed from the compiled message.
    Signer,
    /// Compute limit, native evidence, or Trading adjacency was noncanonical.
    InstructionSequence,
    /// Lookup-table activation, message compilation, or packet sizing refused.
    Routing(crate::versioned::Error),
}

/// Stable refusal from stale authority, malformed signatures, artifact joins,
/// account-profile expansion, or transaction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The current selected immutable Trading release is not checked as a V3 hot outer.
    HotOuterUnavailable,
    /// A required identity, maker, or signature was zero.
    ZeroIdentity,
    /// Account observations were not finalized at one exact snapshot.
    ObservationMismatch,
    /// The family-neutral fixed frame or selected program identity differed.
    FixedFrameMismatch,
    /// Action-selected finalized artifacts did not form one Direct bundle.
    ArtifactMismatch,
    /// Runtime AccountProfile width or privileges differed.
    RuntimeProfileMismatch,
    /// The finalized Product/domain/portfolio graph refused.
    ProductGraphMismatch,
    /// Interpreted execution carried a nonempty accelerator transport suffix.
    StrategyGeometry,
    /// Intent, slot, price, fee, or quantity facts were incompatible.
    EconomicMismatch,
    /// Checked arithmetic or instruction encoding failed.
    Arithmetic,
}

/// Encode the sole canonical Direct V3 InlineOrdinary family request.
pub fn compile_direct_inline_request_v3(
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
) -> Result<[u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3], Error> {
    if seller.maker == Pubkey::default()
        || buyer.maker == Pubkey::default()
        || seller.maker == buyer.maker
        || seller.signature.iter().all(|byte| *byte == 0)
        || buyer.signature.iter().all(|byte| *byte == 0)
        || fill == 0
        || execution_price == 0
    {
        return Err(Error::ZeroIdentity);
    }
    let mut output = [0_u8; DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3];
    let body = encode_header_v3(DirectExecutionActionV3::InlineOrdinary, &mut output)
        .map_err(|_| Error::Arithmetic)?;
    let seller_message = seller
        .intent
        .signed_preimage()
        .map_err(|_| Error::EconomicMismatch)?;
    let buyer_message = buyer
        .intent
        .signed_preimage()
        .map_err(|_| Error::EconomicMismatch)?;
    put(body, 0, seller.maker.as_ref())?;
    put(body, 32, &seller_message)?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        buyer.maker.as_ref(),
    )?;
    put(
        body,
        DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 32,
        &buyer_message,
    )?;
    put(
        body,
        2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3,
        &fill.to_le_bytes(),
    )?;
    put(
        body,
        2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 8,
        &execution_price.to_le_bytes(),
    )?;
    DirectExecutionRequestV3::decode(&output, u32::MAX).map_err(|_| Error::EconomicMismatch)?;
    Ok(output)
}

/// Build one complete chain-derived Direct inline batch without signing or submitting.
///
/// Artifact selection and bytes are derived exclusively from the finalized
/// accounts in `state`; callers cannot supply a parallel descriptor/config
/// selection or detached artifact bodies.
#[allow(clippy::too_many_arguments)]
pub fn build_direct_inline_hot_v4(
    state: &DirectInlineHotStateV3,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
    // The two child caller-authority bumps, Claims then Custody, from
    // `derive_direct_inline_child_authorities_v3`. Their seeds end in a digest
    // over each child's PROJECTED request, which this builder does not compute,
    // so they are the caller's to supply. `[0, 0]` is correct and merely
    // slower: the on-chain walk searches for them exactly as it used to.
    child_caller: [u8; 2],
) -> Result<DirectInlineHotReportV3, Error> {
    let checked = state.hot_outer.ok_or(Error::HotOuterUnavailable)?;
    if checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || state.release_set == [0; 32]
    {
        return Err(Error::ZeroIdentity);
    }
    let observation = validate_frame(state, checked)?;
    let product = authenticate_product_graph(state)?;
    let request = compile_direct_inline_request_v3(seller, buyer, fill, execution_price)?;
    let bundle = authenticate_chain_artifacts_v4(state, &request, product.outcome_count)?;
    if bundle.action != DirectExecutionActionV3::InlineOrdinary
        || !bundle.request_profile.requires_native_signature()
    {
        return Err(Error::ArtifactMismatch);
    }
    if !state.strategy_accounts.is_empty() {
        return Err(Error::StrategyGeometry);
    }
    validate_runtime_profile(state, bundle, product.outcome_count)?;
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?
        .account
        .key;
    let root = &state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?
        .account;
    let preview = preview_economics(
        market,
        state,
        bundle.config,
        seller,
        buyer,
        fill,
        execution_price,
        product.outcome_count,
    )?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).map_err(|_| Error::Arithmetic)?,
        state.release_set,
        market.to_bytes(),
        state.generation,
        hash(&root.data).to_bytes(),
    )
    .map_err(|_| Error::FixedFrameMismatch)?
    .with_bump_hints(HotBumpHintsV1 {
        child_caller,
        ..direct_inline_hot_bump_hints_v1(state, &checked.trading_program, seller, buyer)?
    });
    let mut hot_instruction_data = Vec::with_capacity(HOT_FAMILY_REQUEST_OFFSET_V3 + request.len());
    hot_instruction_data.extend_from_slice(&envelope.to_bytes());
    hot_instruction_data.extend_from_slice(&request);

    let mut accounts = Vec::new();
    accounts.extend(state.fixed_accounts.iter().map(ObservedAccountMetaV3::meta));
    accounts.extend(
        state
            .strategy_accounts
            .iter()
            .map(ObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .runtime_accounts
            .iter()
            .skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
            .map(ObservedAccountMetaV3::meta),
    );
    let required_instruction_signers = signer_keys(&accounts)?;
    let trading = Instruction {
        program_id: checked.trading_program,
        accounts,
        data: hot_instruction_data.clone(),
    };
    let native = native_ed25519_instruction(
        DirectNativeEvidenceContainerV3::TradingHot,
        DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
        &hot_instruction_data,
        [seller.signature, buyer.signature],
    )?;
    Ok(DirectInlineHotReportV3 {
        instructions: [
            ComputeBudgetInstruction::set_compute_unit_limit(DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1),
            ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
            native,
            trading,
        ],
        hot_instruction_data,
        observation,
        selected_program_schema: CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        selected_program: hash(
            &fixed_account(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?
                .account
                .data,
        )
        .to_bytes(),
        outcome_count: product.outcome_count,
        product_record: product.product_record,
        trading_artifact_release: checked.artifact_release,
        checked_manifest_digest: checked.checked_manifest_digest,
        required_instruction_signers,
        preview,
    })
}

/// Build one action-selected Direct request through the family-neutral Hot
/// authority without signing or submitting it.
///
/// The request is first decoded at the Product-authenticated width, then the
/// finalized SetV2/CapabilityV4 and all six artifacts are reauthenticated from
/// `state`. Signed actions use the sole packet-safe current-instruction
/// evidence encoder; unsigned matcher/permissionless actions cannot smuggle a
/// native evidence instruction.
pub fn build_direct_hot_request_v4(
    state: &DirectHotStateV4,
    request: &[u8],
    signatures: &[[u8; 64]],
) -> Result<DirectHotReportV4, Error> {
    let checked = state.hot_outer.ok_or(Error::HotOuterUnavailable)?;
    if checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || state.release_set == [0; 32]
    {
        return Err(Error::ZeroIdentity);
    }
    let observation = validate_frame(state, checked)?;
    let product = authenticate_product_graph(state)?;
    let decoded = DirectExecutionRequestV3::decode(request, product.outcome_count)
        .map_err(|_| Error::EconomicMismatch)?;
    let bundle = authenticate_chain_artifacts_v4(state, request, product.outcome_count)?;
    if bundle.action != decoded.action()
        || bundle.request_profile.requires_native_signature() != !signatures.is_empty()
    {
        return Err(Error::ArtifactMismatch);
    }
    if !state.strategy_accounts.is_empty() {
        return Err(Error::StrategyGeometry);
    }
    validate_runtime_profile(state, bundle, product.outcome_count)?;

    let market = fixed_account(state, HOT_MARKET_ACCOUNT_V3)?.account.key;
    let root = &fixed_account(state, HOT_ROOT_ACCOUNT_V3)?.account;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request.len()).map_err(|_| Error::Arithmetic)?,
        state.release_set,
        market.to_bytes(),
        state.generation,
        hash(&root.data).to_bytes(),
    )
    .map_err(|_| Error::FixedFrameMismatch)?
    // Family-neutral: the Market and the capability root are read by every hot
    // action. Family-specific slots stay zero here and search, which is what a
    // builder that does not know the action's lifecycle should do.
    .with_bump_hints(direct_hot_bump_hints_v1(state, &checked.trading_program)?);
    let mut hot_instruction_data = Vec::with_capacity(HOT_FAMILY_REQUEST_OFFSET_V3 + request.len());
    hot_instruction_data.extend_from_slice(&envelope.to_bytes());
    hot_instruction_data.extend_from_slice(request);

    let mut accounts = Vec::new();
    accounts.extend(state.fixed_accounts.iter().map(ObservedAccountMetaV3::meta));
    accounts.extend(
        state
            .strategy_accounts
            .iter()
            .map(ObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .runtime_accounts
            .iter()
            .skip(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
            .map(ObservedAccountMetaV3::meta),
    );
    let required_instruction_signers = signer_keys(&accounts)?;
    let trading = Instruction {
        program_id: checked.trading_program,
        accounts,
        data: hot_instruction_data.clone(),
    };
    let mut instructions = Vec::with_capacity(usize::from(!signatures.is_empty()) + 3);
    instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
        DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1,
    ));
    // Unconditional, and ahead of any evidence. The heap refusal is a property
    // of the top-level ROUTE, not of whether the makers signed: both arms take
    // the same Registry reauthentication path, so an unsigned submission needs
    // the grant exactly as much as a signed one.
    instructions.push(ComputeBudgetInstruction::request_heap_frame(
        DIRECT_HOT_HEAP_FRAME_BYTES_V1,
    ));
    if !signatures.is_empty() {
        instructions.push(native_ed25519_instruction_many(
            DirectNativeEvidenceContainerV3::TradingHot,
            DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
            &hot_instruction_data,
            decoded.action(),
            product.outcome_count,
            signatures,
        )?);
    }
    instructions.push(trading);

    Ok(DirectHotReportV4 {
        instructions,
        hot_instruction_data,
        observation,
        action: decoded.action(),
        selected_program_schema: CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        selected_program: hash(
            &fixed_account(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?
                .account
                .data,
        )
        .to_bytes(),
        outcome_count: product.outcome_count,
        product_record: product.product_record,
        trading_artifact_release: checked.artifact_release,
        checked_manifest_digest: checked.checked_manifest_digest,
        required_instruction_signers,
    })
}

/// Build a signed RegisterSell or RegisterBuy request through generic Hot.
///
/// The caller supplies detached signature bytes only. The action-selected
/// request encoder owns the exact 316-byte wire, and [`build_direct_hot_request_v4`]
/// independently reauthenticates SetV2, CapabilityV4, Profile14, LifecycleV5,
/// Transition, Strategy, and Effect records from the finalized chain snapshot.
pub fn build_direct_registration_hot_v4(
    state: &DirectHotStateV4,
    action: DirectExecutionActionV3,
    request: DirectRegistrationRequestV3,
    signature: [u8; 64],
) -> Result<DirectHotReportV4, Error> {
    if signature.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentity);
    }
    let mut encoded = [0_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3];
    encode_direct_registration_request_v3_atomic(action, request, &mut encoded)
        .map_err(|_| Error::EconomicMismatch)?;
    build_direct_hot_request_v4(state, &encoded, core::slice::from_ref(&signature))
}

/// Build one maker-authorized registered cancellation through generic Hot.
///
/// The Product graph in `state` is the sole outcome-count authority used to
/// hostile-decode the signed intent. The caller supplies only the canonical
/// signed terminal body and detached Ed25519 signature; this function emits no
/// legacy registered-controller instruction and does not sign or submit.
pub fn build_direct_registered_cancel_hot_v4(
    state: &DirectHotStateV4,
    request: DirectSignedTerminalRequestV3,
    signature: [u8; 64],
) -> Result<DirectHotReportV4, Error> {
    if signature.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentity);
    }
    let outcome_count = authenticate_product_graph(state)?.outcome_count;
    let mut encoded = [0_u8; DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3];
    encode_direct_registered_cancel_request_v3_atomic(request, outcome_count, &mut encoded)
        .map_err(|_| Error::EconomicMismatch)?;
    build_direct_hot_request_v4(state, &encoded, core::slice::from_ref(&signature))
}

/// Build one permissionless registered expiry through generic Hot.
///
/// Expiry has an empty family body and therefore emits no Ed25519 evidence.
/// The selected lifecycle program remains the semantic owner of the live slot
/// boundary; an early transaction is built faithfully and refuses onchain
/// without mutating the registered record or its collateral/rent accounts.
pub fn build_direct_registered_expire_hot_v4(
    state: &DirectHotStateV4,
) -> Result<DirectHotReportV4, Error> {
    let outcome_count = authenticate_product_graph(state)?.outcome_count;
    let mut encoded = [0_u8; DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3];
    encode_direct_empty_action_request_v3_atomic(
        DirectExecutionActionV3::ExpireRegistered,
        outcome_count,
        &mut encoded,
    )
    .map_err(|_| Error::EconomicMismatch)?;
    build_direct_hot_request_v4(state, &encoded, &[])
}

/// Complete chain-authenticated selection needed before a Direct route may
/// provision permanent routing infrastructure or construct Hot execution.
///
/// This is the one host semantic owner for the conjunction the Trading outer
/// rechecks: canonical Core Market, activated release set, current Core and
/// Trading Loader deployments, exact Trading artifact release and semantic
/// release, manifest/ProgramSet/descriptor selection, every finalized
/// selected artifact, and the finalized Product graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedDirectHotChainV4 {
    /// Same finalized observation shared by the complete frame.
    pub observation: Observation,
    /// Market-selected immutable execution release set.
    pub release_set: [u8; 32],
    /// Canonical Core Market address.
    pub market: Pubkey,
    /// Market-selected Registry program.
    pub registry_program: Pubkey,
    /// Exact activated Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Semantic release derived from that authenticated Trading release.
    pub trading_semantic_release: [u8; 32],
    /// Exact action-selected CapabilityProgramV4 digest.
    pub selected_program: [u8; 32],
    /// Product-authenticated outcome count.
    pub outcome_count: u32,
    /// Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Product-stable identity joined through Product, Market, Claims, and basis.
    pub product_id: [u8; 32],
    /// Product-selected semantic liability-basis identity.
    pub semantic_basis: [u8; 32],
    /// Exact finalized linked-basis body digest.
    pub linked_basis_record: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedDirectLinkedBasisV4 {
    semantic_basis: [u8; 32],
    record_digest: [u8; 32],
}

/// Authenticate the complete chain selection for one exact Direct request.
pub fn authenticate_direct_hot_chain_v4(
    state: &DirectHotStateV4,
    request: &[u8],
) -> Result<AuthenticatedDirectHotChainV4, Error> {
    let checked = state.hot_outer.ok_or(Error::HotOuterUnavailable)?;
    if checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || state.release_set == [0; 32]
    {
        return Err(Error::ZeroIdentity);
    }
    let observation = validate_frame(state, checked)?;
    let rent_account = &fixed_account(state, HOT_RENT_SYSVAR_ACCOUNT_V3)?.account;
    let rent = decode_rent(rent_account).map_err(|_| Error::ArtifactMismatch)?;
    let (market, trading_semantic_release) =
        authenticate_direct_market_release_v4(state, checked, &rent)?;
    let product = authenticate_product_graph(state)?;
    let market_state =
        CoreState::decode(&fixed_account(state, HOT_MARKET_ACCOUNT_V3)?.account.data)
            .map_err(|_| Error::ProductGraphMismatch)?;
    if market_state.identity.product_record.to_bytes() != product.product_record {
        return Err(Error::ProductGraphMismatch);
    }
    let linked_basis = authenticate_direct_linked_basis_v4(state, &rent, product)?;
    let decoded = DirectExecutionRequestV3::decode(request, product.outcome_count)
        .map_err(|_| Error::EconomicMismatch)?;
    let bundle = authenticate_chain_artifacts_v4(state, request, product.outcome_count)?;
    if bundle.action != decoded.action() {
        return Err(Error::ArtifactMismatch);
    }
    Ok(AuthenticatedDirectHotChainV4 {
        observation,
        release_set: state.release_set,
        market,
        registry_program: fixed_account(state, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?
            .account
            .key,
        trading_artifact_release: checked.artifact_release,
        trading_semantic_release,
        selected_program: hash(
            &fixed_account(state, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?
                .account
                .data,
        )
        .to_bytes(),
        outcome_count: product.outcome_count,
        product_record: product.product_record,
        product_id: product.product_id,
        semantic_basis: linked_basis.semantic_basis,
        linked_basis_record: linked_basis.record_digest,
    })
}

fn authenticate_direct_linked_basis_v4(
    state: &DirectHotStateV4,
    rent: &solana_program::rent::Rent,
    product: AuthenticatedProductGraphObservationV3,
) -> Result<AuthenticatedDirectLinkedBasisV4, Error> {
    let registry = fixed_account(state, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?
        .account
        .key;
    let linked = finalized_record(
        state,
        registry,
        rent,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3 + 1,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        hash(
            &fixed_account(state, HOT_LINKED_BASIS_RAW_ACCOUNT_V3)?
                .account
                .data,
        )
        .to_bytes(),
    )?;
    let basis = ProductBasisV3::decode(linked).map_err(|_| Error::ProductGraphMismatch)?;
    let domain = ResultDomainV2::decode(
        &fixed_account(state, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3)?
            .account
            .data,
    )
    .map_err(|_| Error::ProductGraphMismatch)?;
    let semantic = semantic_basis_preimage_v3(linked).map_err(|_| Error::ProductGraphMismatch)?;
    let semantic_id = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes();
    if semantic_id != product.liability_basis_id
        || basis.product_id() != product.product_id
        || basis.result_domain_id() != product.result_domain_id
        || basis.coordinate_domain_id() != domain.coordinate_domain_id().to_bytes()
        || basis.result_unit_id() != domain.result_unit_id().to_bytes()
        || basis.payout_scale() == 0
    {
        return Err(Error::ProductGraphMismatch);
    }
    Ok(AuthenticatedDirectLinkedBasisV4 {
        semantic_basis: semantic_id,
        record_digest: hash(linked).to_bytes(),
    })
}

fn authenticate_direct_market_release_v4(
    state: &DirectHotStateV4,
    checked: CheckedHotOuterReleaseV3,
    rent: &solana_program::rent::Rent,
) -> Result<(Pubkey, [u8; 32]), Error> {
    let market_account = &fixed_account(state, HOT_MARKET_ACCOUNT_V3)?.account;
    let core_program = &fixed_account(state, HOT_CORE_PROGRAM_ACCOUNT_V3)?.account;
    let core_programdata = &fixed_account(state, HOT_CORE_PROGRAMDATA_ACCOUNT_V3)?.account;
    let trading_program = &fixed_account(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?.account;
    let trading_programdata = &fixed_account(state, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3)?.account;
    let registry_program = &fixed_account(state, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?.account;
    let activation = &fixed_account(state, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.account;
    if market_account.owner != core_program.key
        || market_account.executable
        || market_account.data.len() != STATE_BYTES
        || !rent.is_exempt(market_account.lamports, market_account.data.len())
        || registry_program.owner != bpf_loader_upgradeable::ID
        || !registry_program.executable
        || ProgramV3View::parse(&registry_program.data).is_err()
    {
        return Err(Error::FixedFrameMismatch);
    }
    let market = CoreState::decode(&market_account.data).map_err(|_| Error::ArtifactMismatch)?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &core_program.key,
    )
    .0;
    if market
        .encode()
        .map_err(|_| Error::ArtifactMismatch)?
        .as_slice()
        != market_account.data
        || market.phase != Phase::Open
        || market_account.key != expected_market
        || market.identity.market_id.to_bytes() != market_account.key.to_bytes()
        || market.identity.registry_program.to_bytes() != registry_program.key.to_bytes()
        || market.identity.selected_release_set.to_bytes() != state.release_set
        || market.identity.generation != state.generation
    {
        return Err(Error::ArtifactMismatch);
    }
    if activation.owner != registry_program.key
        || activation.executable
        || activation.data.len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        || !rent.is_exempt(activation.lamports, activation.data.len())
    {
        return Err(Error::ArtifactMismatch);
    }
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &state.release_set],
        &registry_program.key,
    )
    .0;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&activation.data)
        .map_err(|_| Error::ArtifactMismatch)?;
    if activation.key != expected_cache
        || activated
            .execution_release_set_id()
            .map_err(|_| Error::ArtifactMismatch)?
            .to_bytes()
            != state.release_set
    {
        return Err(Error::ArtifactMismatch);
    }
    let core = authenticate_direct_role_deployment_v4(
        activated,
        ExecutionRoleV1::Core,
        core_program,
        core_programdata,
    )?;
    let trading = authenticate_direct_role_deployment_v4(
        activated,
        ExecutionRoleV1::Trading,
        trading_program,
        trading_programdata,
    )?;
    if trading_program.key != checked.trading_program
        || trading.artifact_release_id().to_bytes() != checked.artifact_release
        || core.release().program().to_bytes() != core_program.key.to_bytes()
    {
        return Err(Error::ArtifactMismatch);
    }
    Ok((
        market_account.key,
        trading.release().semantic_release_id().to_bytes(),
    ))
}

pub(crate) fn authenticate_direct_role_deployment_v4(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<dclutch_registry_contract::ActivatedRoleV1, Error> {
    let selected = activated.role(role).map_err(|_| Error::ArtifactMismatch)?;
    let observation = direct_deployment_observation_v4(program, programdata, selected.release())?;
    selected
        .authenticate_current_deployment(observation)
        .map_err(|_| Error::ArtifactMismatch)?;
    Ok(selected)
}

pub(crate) fn direct_deployment_observation_v4(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    release: ArtifactReleaseV1,
) -> Result<DeploymentObservationV1, Error> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || program.key.to_bytes() != release.program().to_bytes()
        || programdata.key.to_bytes() != release.programdata()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(Error::ArtifactMismatch);
    }
    let program_view = ProgramV3View::parse(&program.data).map_err(|_| Error::ArtifactMismatch)?;
    let expected =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes() || programdata.key != expected {
        return Err(Error::ArtifactMismatch);
    }
    let data = ProgramDataV3View::parse(&programdata.data).map_err(|_| Error::ArtifactMismatch)?;
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
    .map_err(|_| Error::ArtifactMismatch)
}

fn authenticate_chain_artifacts_v4<'a>(
    state: &'a DirectInlineHotStateV3,
    request: &'a [u8],
    outcome_count: u32,
) -> Result<DirectArtifactBundleV4<'a>, Error> {
    let registry = fixed_account(state, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?
        .account
        .key;
    let rent = decode_rent(&fixed_account(state, HOT_RENT_SYSVAR_ACCOUNT_V3)?.account)
        .map_err(|_| Error::ArtifactMismatch)?;
    let root = &fixed_account(state, HOT_ROOT_ACCOUNT_V3)?.account;
    let header = CapabilityRootHeaderV1::decode(
        root.data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(Error::ArtifactMismatch)?,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    let trading = fixed_account(state, HOT_TRADING_PROGRAM_ACCOUNT_V3)?
        .account
        .key;
    let market = fixed_account(state, HOT_MARKET_ACCOUNT_V3)?.account.key;
    let seeds = header.seeds();
    if root.owner != trading
        || root.executable
        || header.release_set().to_bytes() != state.release_set
        || header.market() != market.to_bytes()
        || header.generation() != state.generation
        || header.selection().executor_role() != ExecutionRoleV1::Trading
        || Pubkey::find_program_address(&seeds.as_slices(), &trading).0 != root.key
    {
        return Err(Error::ArtifactMismatch);
    }
    let selection = header.selection();
    let manifest_data = finalized_record(
        state,
        registry,
        &rent,
        HOT_MANIFEST_RAW_ACCOUNT_V3,
        HOT_MANIFEST_STAGING_ACCOUNT_V3,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        selection.manifest().to_bytes(),
    )?;
    let manifest =
        CapabilityManifestV1::decode(manifest_data).map_err(|_| Error::ArtifactMismatch)?;
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(|_| Error::ArtifactMismatch)?;

    let program_set_data = finalized_record(
        state,
        registry,
        &rent,
        HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        selection.capability_release().to_bytes(),
    )?;
    let program_set = CapabilityProgramSetV2::decode_selected(
        selection.capability_release().to_bytes(),
        hash(program_set_data).to_bytes(),
        program_set_data,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    let descriptor_reference = program_set
        .select_descriptor(request)
        .map_err(|_| Error::ArtifactMismatch)?;
    if descriptor_reference.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_ID_V4 {
        return Err(Error::ArtifactMismatch);
    }
    let descriptor_data = finalized_record(
        state,
        registry,
        &rent,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        descriptor_reference.program().to_bytes(),
    )?;
    let descriptor =
        CapabilityProgramV4::decode(descriptor_data).map_err(|_| Error::ArtifactMismatch)?;
    descriptor
        .validate_selection(selection, entry)
        .map_err(|_| Error::ArtifactMismatch)?;
    let expected_root_bytes = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(usize::try_from(descriptor.root_state_bytes()).map_err(|_| Error::Arithmetic)?)
        .ok_or(Error::Arithmetic)?;
    if root.data.len() != expected_root_bytes {
        return Err(Error::ArtifactMismatch);
    }

    let config = finalized_record(
        state,
        registry,
        &rent,
        HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CONFIG_STAGING_ACCOUNT_V3,
        descriptor.config_schema().to_bytes(),
        selection.config().to_bytes(),
    )?;
    let account_profile = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
        descriptor.account_profile(),
    )?;
    let request_profile = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
        descriptor.request_profile(),
    )?;
    let transition = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_TRANSITION_STAGING_ACCOUNT_V3,
        descriptor.transition(),
    )?;
    let effect = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_EFFECT_STAGING_ACCOUNT_V3,
        descriptor.effect(),
    )?;
    let lifecycle_policy = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
        descriptor.lifecycle(),
    )?;
    let strategy = finalized_artifact(
        state,
        registry,
        &rent,
        HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_STRATEGY_STAGING_ACCOUNT_V3,
        descriptor.strategy(),
    )?;
    authenticate_direct_artifacts_v4(
        DirectArtifactSelectionV4 {
            program_set: selection.capability_release().to_bytes(),
            config: selection.config().to_bytes(),
        },
        DirectArtifactBytesV4 {
            program_set: program_set_data,
            descriptor: descriptor_data,
            config,
            account_profile,
            lifecycle_policy,
            request_profile,
            strategy,
            transition,
            effect,
        },
        request,
        outcome_count,
    )
    .map_err(|_| Error::ArtifactMismatch)
}

fn finalized_artifact<'a>(
    state: &'a DirectInlineHotStateV3,
    registry: Pubkey,
    rent: &solana_program::rent::Rent,
    raw_coordinate: usize,
    staging_coordinate: usize,
    reference: ArtifactReferenceV4,
) -> Result<&'a [u8], Error> {
    finalized_record(
        state,
        registry,
        rent,
        raw_coordinate,
        staging_coordinate,
        reference.schema().to_bytes(),
        reference.program().to_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
fn finalized_record<'a>(
    state: &'a DirectInlineHotStateV3,
    registry: Pubkey,
    rent: &solana_program::rent::Rent,
    raw_coordinate: usize,
    staging_coordinate: usize,
    schema: [u8; 32],
    expected_content: [u8; 32],
) -> Result<&'a [u8], Error> {
    let raw = &fixed_account(state, raw_coordinate)?.account;
    let staging = &fixed_account(state, staging_coordinate)?.account;
    authenticate_finalized_record(
        registry,
        rent,
        raw,
        &FinalizedRecordProof {
            schema_release_id: schema,
            staging_cursor: staging.clone(),
        },
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    if hash(&raw.data).to_bytes() != expected_content {
        return Err(Error::ArtifactMismatch);
    }
    Ok(&raw.data)
}

fn fixed_account(
    state: &DirectInlineHotStateV3,
    coordinate: usize,
) -> Result<&ObservedAccountMetaV3, Error> {
    state
        .fixed_accounts
        .get(coordinate)
        .ok_or(Error::FixedFrameMismatch)
}

/// Compile the exact budgeted adjacent batch through one canonical finalized LUT.
pub fn compile_direct_inline_hot_v0(
    report: &DirectInlineHotReportV3,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<DirectInlineHotTransactionPlanV3, DirectInlineTransactionErrorV3> {
    if payer == Pubkey::default()
        || report.observation.finality != Finality::Finalized
        || report.observation.slot == 0
        || report.selected_program_schema != CAPABILITY_PROGRAM_SCHEMA_ID_V4
        || report.trading_artifact_release == [0; 32]
        || report.checked_manifest_digest == [0; 32]
        || lookup_table.observation != report.observation
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
    {
        return Err(DirectInlineTransactionErrorV3::Snapshot);
    }
    validate_direct_hot_instruction_sequence_v4(
        DirectExecutionActionV3::InlineOrdinary,
        report.outcome_count,
        &report.hot_instruction_data,
        &report.instructions,
    )?;
    let expected = canonical_direct_inline_lookup_addresses_v3(report, payer)?;
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| DirectInlineTransactionErrorV3::LookupTable)?;
    if table.addresses.as_ref() != expected.as_slice() {
        return Err(DirectInlineTransactionErrorV3::LookupTable);
    }
    let message = compile_v0_message(
        payer,
        &report.instructions,
        recent_blockhash,
        report.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(DirectInlineTransactionErrorV3::Routing)?;
    let mut required_signers = vec![payer];
    for signer in &report.required_instruction_signers {
        if !required_signers.contains(signer) {
            required_signers.push(*signer);
        }
    }
    if usize::from(message.required_signatures) != required_signers.len() {
        return Err(DirectInlineTransactionErrorV3::Signer);
    }
    Ok(DirectInlineHotTransactionPlanV3 {
        message,
        required_signers,
        outcome_count: report.outcome_count,
        selected_program_schema: report.selected_program_schema,
        selected_program: report.selected_program,
        trading_artifact_release: report.trading_artifact_release,
        checked_manifest_digest: report.checked_manifest_digest,
    })
}

/// Return the sole sorted, duplicate-free LUT address sequence for Direct.
pub fn canonical_direct_inline_lookup_addresses_v3(
    report: &DirectInlineHotReportV3,
    payer: Pubkey,
) -> Result<Vec<Pubkey>, DirectInlineTransactionErrorV3> {
    canonical_lookup_addresses(
        &report.instructions,
        &report.required_instruction_signers,
        payer,
    )
}

/// The sole author of a canonical Direct lookup-table address set.
///
/// SIGNERS ARE EXCLUDED BY KEY, NOT BY OCCURRENCE. A v0 message must carry every
/// signer as a static key, and the same account may appear as a signing meta in
/// one instruction and a non-signing meta in another -- so a filter that tests
/// `meta.is_signer` per occurrence admits a signer into the table through its
/// non-signing appearance and compiles a message whose signer is not static.
/// Testing the signer KEY SET cannot do that.
///
/// `pub` since 2026-09-02: the program-test waist had its own copy carrying
/// exactly the per-occurrence filter above, which is two authors for one message
/// and one of them wrong in a case the fixtures had not yet reached.
pub fn canonical_lookup_addresses(
    instructions: &[Instruction],
    instruction_signers: &[Pubkey],
    payer: Pubkey,
) -> Result<Vec<Pubkey>, DirectInlineTransactionErrorV3> {
    if payer == Pubkey::default() {
        return Err(DirectInlineTransactionErrorV3::Snapshot);
    }
    let mut signers = vec![payer];
    for signer in instruction_signers {
        if *signer == Pubkey::default() {
            return Err(DirectInlineTransactionErrorV3::Signer);
        }
        if !signers.contains(signer) {
            signers.push(*signer);
        }
    }
    canonical_lookup_addresses_excluding_signers(instructions, &signers)
}

/// The same filter, for a caller that states its complete signer set directly.
///
/// A SUBMISSION always has a fee payer, and `canonical_lookup_addresses` refuses
/// without one. A table BUILDER does not always: a market's lookup table is
/// published before anyone has chosen who will pay for the transactions that use
/// it, and a harness that stages one has no payer to name either. Those callers
/// state the signer set they know and get the same filter; they do not get to
/// skip it, and they do not get a second copy of it.
pub fn canonical_lookup_addresses_excluding_signers(
    instructions: &[Instruction],
    signers: &[Pubkey],
) -> Result<Vec<Pubkey>, DirectInlineTransactionErrorV3> {
    let program_ids = instructions
        .iter()
        .map(|instruction| instruction.program_id)
        .collect::<Vec<_>>();
    let mut addresses = instructions
        .iter()
        .flat_map(|instruction| &instruction.accounts)
        .filter(|account| {
            !signers.contains(&account.pubkey) && !program_ids.contains(&account.pubkey)
        })
        .map(|account| account.pubkey)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > 256 {
        return Err(DirectInlineTransactionErrorV3::LookupTable);
    }
    Ok(addresses)
}

pub(crate) fn validate_direct_hot_instruction_sequence_v4(
    action: DirectExecutionActionV3,
    tail_count: u32,
    hot_instruction_data: &[u8],
    instructions: &[Instruction],
) -> Result<(), DirectInlineTransactionErrorV3> {
    let compute =
        ComputeBudgetInstruction::set_compute_unit_limit(DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1);
    if instructions.first() != Some(&compute) {
        return Err(DirectInlineTransactionErrorV3::InstructionSequence);
    }
    // The heap grant is checked here rather than trusted, and on BOTH arms. A
    // top-level submission that omits it is not a transaction that merely runs
    // differently: Trading refuses it with `TradingSbfError::HeapFrame`, so
    // emitting one is emitting a guaranteed refusal. Catching it at the
    // operator costs one comparison and names the omission at the only place
    // that can still fix it.
    let heap = ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1);
    if instructions.get(1) != Some(&heap) {
        return Err(DirectInlineTransactionErrorV3::InstructionSequence);
    }
    let signature_count = native_signature_count_v3(action, tail_count);
    let count = usize::try_from(signature_count)
        .map_err(|_| DirectInlineTransactionErrorV3::InstructionSequence)?;
    if count == 0 {
        let trading = instructions
            .get(DIRECT_HOT_UNSIGNED_TRADING_INSTRUCTION_INDEX_V1)
            .ok_or(DirectInlineTransactionErrorV3::InstructionSequence)?;
        if instructions.len() != DIRECT_HOT_UNSIGNED_TRADING_INSTRUCTION_INDEX_V1 + 1
            || trading.data != hot_instruction_data
        {
            return Err(DirectInlineTransactionErrorV3::InstructionSequence);
        }
        return Ok(());
    }
    if instructions.len() != usize::from(DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1) + 1 {
        return Err(DirectInlineTransactionErrorV3::InstructionSequence);
    }
    let native = instructions
        .get(2)
        .ok_or(DirectInlineTransactionErrorV3::InstructionSequence)?;
    let trading = instructions
        .get(usize::from(DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1))
        .ok_or(DirectInlineTransactionErrorV3::InstructionSequence)?;
    if native.program_id != ed25519_program::ID
        || !native.accounts.is_empty()
        || trading.data != hot_instruction_data
    {
        return Err(DirectInlineTransactionErrorV3::InstructionSequence);
    }
    let bytes = direct_native_evidence_bytes_v3(action, tail_count)
        .map_err(|_| DirectInlineTransactionErrorV3::InstructionSequence)?;
    let header = 2_usize
        .checked_add(
            count
                .checked_mul(14)
                .ok_or(DirectInlineTransactionErrorV3::InstructionSequence)?,
        )
        .ok_or(DirectInlineTransactionErrorV3::InstructionSequence)?;
    if native.data.len() != bytes {
        return Err(DirectInlineTransactionErrorV3::InstructionSequence);
    }
    let mut signatures = Vec::with_capacity(count);
    for signature in native
        .data
        .get(header..)
        .ok_or(DirectInlineTransactionErrorV3::InstructionSequence)?
        .chunks_exact(64)
    {
        signatures.push(
            <[u8; 64]>::try_from(signature)
                .map_err(|_| DirectInlineTransactionErrorV3::InstructionSequence)?,
        );
    }
    if signatures.len() != count {
        return Err(DirectInlineTransactionErrorV3::InstructionSequence);
    }
    let mut scratch = vec![0_u8; bytes];
    let mut expected = vec![0_u8; bytes];
    encode_direct_native_evidence_many_v3_atomic(
        DirectNativeEvidenceContainerV3::TradingHot,
        DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
        hot_instruction_data,
        tail_count,
        &signatures,
        &mut scratch,
        &mut expected,
    )
    .map_err(|_| DirectInlineTransactionErrorV3::InstructionSequence)?;
    if native.data != expected {
        return Err(DirectInlineTransactionErrorV3::InstructionSequence);
    }
    Ok(())
}

fn validate_frame(
    state: &DirectInlineHotStateV3,
    checked: CheckedHotOuterReleaseV3,
) -> Result<Observation, Error> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.minimum_finalized_slot == 0
        || state.runtime_accounts.len() < HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3
    {
        return Err(Error::FixedFrameMismatch);
    }
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let trading = state
        .fixed_accounts
        .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let rent = state
        .fixed_accounts
        .get(HOT_RENT_SYSVAR_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let instructions = state
        .fixed_accounts
        .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    let registry = state
        .fixed_accounts
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(Error::FixedFrameMismatch)?;
    if trading.account.key != checked.trading_program
        || !trading.account.executable
        || !registry.account.executable
        || rent.account.key != sysvar::rent::ID
        || instructions.account.key != sysvar::instructions::ID
    {
        return Err(Error::FixedFrameMismatch);
    }
    let observation = market.account.observation;
    if observation.finality != Finality::Finalized
        || observation.slot < state.minimum_finalized_slot
    {
        return Err(Error::ObservationMismatch);
    }
    for (index, value) in state.fixed_accounts.iter().enumerate() {
        if value.account.observation != observation
            || value.is_signer
            || value.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(Error::FixedFrameMismatch);
        }
    }
    for value in state
        .strategy_accounts
        .iter()
        .chain(&state.runtime_accounts)
    {
        if value.account.observation.finality != Finality::Finalized
            || value.account.observation != observation
        {
            return Err(Error::ObservationMismatch);
        }
    }
    for (runtime, physical) in [
        (0, HOT_ROOT_ACCOUNT_V3),
        (1, HOT_CONFIG_RAW_ACCOUNT_V3),
        (2, HOT_PRODUCT_RAW_ACCOUNT_V3),
        (3, HOT_PORTFOLIO_RAW_ACCOUNT_V3),
        (4, HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
    ] {
        if state.runtime_accounts.get(runtime) != state.fixed_accounts.get(physical) {
            return Err(Error::RuntimeProfileMismatch);
        }
    }
    Ok(observation)
}

fn authenticate_product_graph(
    state: &DirectInlineHotStateV3,
) -> Result<AuthenticatedProductGraphObservationV3, Error> {
    let account = |index: usize| {
        state
            .fixed_accounts
            .get(index)
            .map(|value| &value.account)
            .ok_or(Error::ProductGraphMismatch)
    };
    authenticate_product_graph_observation_v3(FinalizedProductGraphAccountsV3 {
        registry_program: account(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?.key,
        product_raw: account(HOT_PRODUCT_RAW_ACCOUNT_V3)?,
        product_staging: account(HOT_PRODUCT_RAW_ACCOUNT_V3 + 1)?,
        domain_raw: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3)?,
        domain_staging: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3 + 1)?,
        portfolio_raw: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3)?,
        portfolio_staging: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3 + 1)?,
    })
    .map_err(|_| Error::ProductGraphMismatch)
}

fn validate_runtime_profile(
    state: &DirectInlineHotStateV3,
    bundle: dclutch_direct_codec::artifacts_v4::DirectArtifactBundleV4<'_>,
    outcome_count: u32,
) -> Result<(), Error> {
    let profile = bundle.account_profile;
    let expected = profile
        .physical_account_count_with_dynamic_spans(outcome_count, &[])
        .map_err(|_| Error::RuntimeProfileMismatch)?;
    if expected < HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3 || state.runtime_accounts.len() != expected
    {
        return Err(Error::RuntimeProfileMismatch);
    }
    for (physical_ordinal, account) in state.runtime_accounts.iter().enumerate() {
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(outcome_count, &[], physical_ordinal)
            .map_err(|_| Error::RuntimeProfileMismatch)?;
        let privileges = geometry.privileges();
        let data_matches = match geometry.data() {
            PhysicalAccountDataGeometryV2::Exact { bytes } => account.account.data.len() == bytes,
            PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
                account.account.data.is_empty() || account.account.data.len() == live_bytes
            }
            PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                !account.account.data.is_empty() && account.account.data.len() >= minimum_bytes
            }
            PhysicalAccountDataGeometryV2::Opaque => true,
        };
        if account.is_signer != privileges.signer()
            || account.is_writable != privileges.writable()
            || account.account.executable != privileges.executable()
            || !data_matches
        {
            return Err(Error::RuntimeProfileMismatch);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn preview_economics(
    market: Pubkey,
    state: &DirectInlineHotStateV3,
    config: dclutch_direct_codec::successor::DirectExecutionConfigV1,
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
    outcome_count: u32,
) -> Result<DirectInlineEconomicPreviewV3, Error> {
    for (participant, side) in [(seller, 0_u8), (buyer, 1_u8)] {
        let intent = participant.intent;
        if intent.side != side
            || intent.lifecycle > 1
            || intent.market != market.to_bytes()
            || intent.generation != state.generation
            || intent.outcome >= outcome_count
            || intent.maximum_fill < fill
            || intent.fee_basis_points != config.fee_basis_points()
            || state.clock_slot < intent.valid_from
            || state.clock_slot > intent.valid_through
        {
            return Err(Error::EconomicMismatch);
        }
        if intent.lifecycle == 0 && intent.maximum_fill != fill {
            return Err(Error::EconomicMismatch);
        }
    }
    if seller.intent.outcome != buyer.intent.outcome
        || execution_price < seller.intent.limit_price
        || execution_price > buyer.intent.limit_price
        || execution_price > config.price_scale()
    {
        return Err(Error::EconomicMismatch);
    }
    let scaled = u128::from(fill)
        .checked_mul(u128::from(execution_price))
        .ok_or(Error::Arithmetic)?;
    let scale = u128::from(config.price_scale());
    if scaled % scale != 0 {
        return Err(Error::EconomicMismatch);
    }
    let gross = u64::try_from(scaled / scale).map_err(|_| Error::Arithmetic)?;
    let fee = u64::try_from(
        u128::from(gross)
            .checked_mul(u128::from(config.fee_basis_points()))
            .ok_or(Error::Arithmetic)?
            / 10_000,
    )
    .map_err(|_| Error::Arithmetic)?;
    Ok(DirectInlineEconomicPreviewV3 {
        claim_transfer: fill,
        gross_collateral: gross,
        seller_net_collateral_credit: gross.checked_sub(fee).ok_or(Error::Arithmetic)?,
        buyer_collateral_debit: gross.checked_add(fee).ok_or(Error::Arithmetic)?,
        total_fee_transfer: fee.checked_mul(2).ok_or(Error::Arithmetic)?,
    })
}

/// Append packet-safe native evidence immediately before an outer Registry
/// instruction, deriving its index from the complete top-level prefix. The
/// Registry data must be byte-identical Hot bytes beginning at zero; message
/// offsets cannot be supplied by callers.
pub fn append_direct_headerless_registry_native_evidence_v6(
    top_level: &mut Vec<Instruction>,
    registry: Instruction,
    signatures: [[u8; 64]; 2],
) -> Result<(), Error> {
    append_direct_headerless_registry_native_evidence_many_v6(
        top_level,
        registry,
        DirectExecutionActionV3::InlineOrdinary,
        u32::MAX,
        &signatures,
    )
}

/// Append packet-safe action-selected evidence immediately before Registry.
///
/// The current Registry instruction index is derived from `top_level`; the
/// codec independently rechecks the headerless Hot request and signature
/// count. Unsigned actions must omit this pair and append their Registry
/// instruction directly.
pub fn append_direct_headerless_registry_native_evidence_many_v6(
    top_level: &mut Vec<Instruction>,
    registry: Instruction,
    action: DirectExecutionActionV3,
    tail_count: u32,
    signatures: &[[u8; 64]],
) -> Result<(), Error> {
    let registry_index = u16::try_from(top_level.len().checked_add(1).ok_or(Error::Arithmetic)?)
        .map_err(|_| Error::Arithmetic)?;
    let native = headerless_registry_native_ed25519_instruction_many(
        registry_index,
        &registry.data,
        action,
        tail_count,
        signatures,
    )?;
    top_level.push(native);
    top_level.push(registry);
    Ok(())
}

fn headerless_registry_native_ed25519_instruction_many(
    current_instruction_index: u16,
    current_instruction_data: &[u8],
    action: DirectExecutionActionV3,
    tail_count: u32,
    signatures: &[[u8; 64]],
) -> Result<Instruction, Error> {
    let bytes =
        direct_native_evidence_bytes_v3(action, tail_count).map_err(|_| Error::ArtifactMismatch)?;
    let mut scratch = vec![0_u8; bytes];
    let mut data = vec![0_u8; bytes];
    encode_direct_headerless_registry_native_evidence_many_v4_atomic(
        current_instruction_index,
        current_instruction_data,
        tail_count,
        signatures,
        &mut scratch,
        &mut data,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    Ok(Instruction {
        program_id: ed25519_program::ID,
        accounts: Vec::new(),
        data,
    })
}

fn native_ed25519_instruction(
    container: DirectNativeEvidenceContainerV3,
    current_instruction_index: u16,
    current_instruction_data: &[u8],
    signatures: [[u8; 64]; 2],
) -> Result<Instruction, Error> {
    let mut data = vec![0_u8; DIRECT_NATIVE_EVIDENCE_BYTES_V3];
    encode_direct_native_evidence_v3_atomic(
        container,
        current_instruction_index,
        current_instruction_data,
        signatures,
        &mut data,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    Ok(Instruction {
        program_id: ed25519_program::ID,
        accounts: Vec::new(),
        data,
    })
}

fn native_ed25519_instruction_many(
    container: DirectNativeEvidenceContainerV3,
    current_instruction_index: u16,
    current_instruction_data: &[u8],
    action: DirectExecutionActionV3,
    tail_count: u32,
    signatures: &[[u8; 64]],
) -> Result<Instruction, Error> {
    let bytes =
        direct_native_evidence_bytes_v3(action, tail_count).map_err(|_| Error::ArtifactMismatch)?;
    let mut scratch = vec![0_u8; bytes];
    let mut data = vec![0_u8; bytes];
    encode_direct_native_evidence_many_v3_atomic(
        container,
        current_instruction_index,
        current_instruction_data,
        tail_count,
        signatures,
        &mut scratch,
        &mut data,
    )
    .map_err(|_| Error::ArtifactMismatch)?;
    Ok(Instruction {
        program_id: ed25519_program::ID,
        accounts: Vec::new(),
        data,
    })
}

fn signer_keys(accounts: &[AccountMeta]) -> Result<Vec<Pubkey>, Error> {
    let mut signers = Vec::new();
    for account in accounts.iter().filter(|account| account.is_signer) {
        if account.pubkey == Pubkey::default() {
            return Err(Error::ZeroIdentity);
        }
        if !signers.contains(&account.pubkey) {
            signers.push(account.pubkey);
        }
    }
    Ok(signers)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), Error> {
    let end = offset.checked_add(value.len()).ok_or(Error::Arithmetic)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::Arithmetic)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_capability_program_contract::{
        SelectedRecordBumpsV1,
        set_v2::{
            CapabilityDescriptorReferenceV2, CapabilityProgramSetEntryV2, SelectorWidthV2,
            encode_program_set_v2, encoded_program_set_bytes_v2,
        },
    };
    use dclutch_custody_contract::CustodyReplayLayoutV1;
    use dclutch_direct_codec::{
        execution_v3::{
            DIRECT_REGISTRATION_REQUEST_BYTES_V3, DirectRegistrationRequestV3,
            DirectSignedParticipantV3,
        },
        // Only the tests build an intent here: the module's own code takes one
        // already signed, inside a `SignedDirectIntentV3` that now arrives from
        // `dclutch-direct-ticket`. Importing it at file scope would be unused in
        // the lib build, which is exactly how it got dropped in the first place.
        intent_v2::CompactIntentV2,
        ordinary_account_artifacts_v3::DirectInlineOrdinaryAccountProfileInputV3,
        ordinary_bundle_v4::{
            DirectInlineOrdinaryHotBundleInputV4, build_direct_inline_ordinary_hot_bundle_v4,
        },
        ordinary_effect_artifacts_v3::{
            DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
        },
        ordinary_geometry_v3::DirectOrdinaryGeometryV3,
        registered_requests_v4::encode_direct_registration_request_v3_atomic,
        successor::{
            DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_MAKER_REPLAY_BYTES_V1,
            DIRECT_ROOT_SCHEMA_ID_V1, DirectExecutionConfigV1, DirectRootStateV1,
        },
    };
    use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
    use dclutch_realm_contract::REALM_BYTES;
    use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_release_set_contract::CapabilityExecutionSelectionV1;
    use dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
    use solana_address_lookup_table_interface::state::LookupTableMeta;
    use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};
    use solana_sdk_ids::system_program;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn observation() -> Observation {
        Observation {
            slot: 500,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn intent(side: u8, maker_byte: u8) -> SignedDirectIntentV3 {
        SignedDirectIntentV3 {
            maker: Pubkey::new_from_array([maker_byte; 32]),
            signature: [maker_byte; 64],
            intent: CompactIntentV2 {
                side,
                lifecycle: 1,
                outcome: 70_000,
                market: [7; 32],
                generation: 9,
                nonce: 3,
                valid_from: 100,
                valid_through: 200,
                maximum_fill: 1_000,
                limit_price: if side == 0 { 400_000 } else { 600_000 },
                fee_basis_points: 25,
                collateral_account: [maker_byte + 10; 32],
            },
        }
    }

    fn transaction_report(extra_accounts: usize) -> DirectInlineHotReportV3 {
        let actor = key(1);
        let mut accounts = vec![AccountMeta::new_readonly(actor, true)];
        accounts.extend((2_u8..92).map(|value| AccountMeta::new(key(value), false)));
        *accounts.last_mut().expect("Trading program coordinate") =
            AccountMeta::new_readonly(key(200), false);
        accounts.extend((0..extra_accounts).map(|_| AccountMeta::new(Pubkey::new_unique(), false)));
        let seller = intent(0, 1);
        let buyer = intent(1, 2);
        let request = compile_direct_inline_request_v3(seller, buyer, 1_000, 500_000)
            .expect("Direct request");
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(request.len()).expect("request width"),
            [1; 32],
            [7; 32],
            9,
            [2; 32],
        )
        .expect("Hot envelope");
        let mut hot_instruction_data = envelope.to_bytes().to_vec();
        hot_instruction_data.extend_from_slice(&request);
        let native = native_ed25519_instruction(
            DirectNativeEvidenceContainerV3::TradingHot,
            DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
            &hot_instruction_data,
            [seller.signature, buyer.signature],
        )
        .expect("native evidence");
        DirectInlineHotReportV3 {
            instructions: [
                ComputeBudgetInstruction::set_compute_unit_limit(DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1),
                ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
                native,
                Instruction {
                    program_id: key(200),
                    accounts,
                    data: hot_instruction_data.clone(),
                },
            ],
            hot_instruction_data,
            observation: observation(),
            selected_program_schema: CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            selected_program: [8; 32],
            outcome_count: 258,
            product_record: [9; 32],
            trading_artifact_release: [10; 32],
            checked_manifest_digest: [11; 32],
            required_instruction_signers: vec![actor],
            preview: DirectInlineEconomicPreviewV3 {
                claim_transfer: 10,
                gross_collateral: 5,
                seller_net_collateral_credit: 4,
                buyer_collateral_debit: 6,
                total_fee_transfer: 2,
            },
        }
    }

    fn lookup(report: &DirectInlineHotReportV3, payer: Pubkey) -> ObservedAccount {
        let addresses = canonical_direct_inline_lookup_addresses_v3(report, payer)
            .expect("canonical addresses");
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(key(201)),
                last_extended_slot: observation().slot - 1,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: observation(),
            key: key(202),
            owner: lookup_table_program::id(),
            lamports: 1_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("lookup bytes"),
        }
    }

    fn hot38_state() -> (DirectInlineHotStateV3, CheckedHotOuterReleaseV3) {
        let checked = CheckedHotOuterReleaseV3 {
            trading_program: key(200),
            artifact_release: [20; 32],
            checked_manifest_digest: [21; 32],
        };
        let mut fixed_accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| ObservedAccountMetaV3 {
                account: ObservedAccount {
                    observation: observation(),
                    key: key(u8::try_from(index + 100).expect("test key")),
                    owner: key(220),
                    lamports: 1,
                    executable: false,
                    data: vec![0],
                },
                is_signer: false,
                is_writable: index == HOT_ROOT_ACCOUNT_V3,
            })
            .collect::<Vec<_>>();
        let trading = fixed_accounts
            .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .expect("Trading coordinate");
        trading.account.key = checked.trading_program;
        trading.account.executable = true;
        fixed_accounts
            .get_mut(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
            .expect("Registry coordinate")
            .account
            .executable = true;
        fixed_accounts
            .get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3)
            .expect("Rent coordinate")
            .account
            .key = sysvar::rent::ID;
        fixed_accounts
            .get_mut(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .expect("Instructions coordinate")
            .account
            .key = sysvar::instructions::ID;
        let runtime_accounts = [
            HOT_ROOT_ACCOUNT_V3,
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        ]
        .map(|index| {
            fixed_accounts
                .get(index)
                .expect("injected coordinate")
                .clone()
        })
        .into_iter()
        .collect();
        (
            DirectInlineHotStateV3 {
                fixed_accounts,
                strategy_accounts: Vec::new(),
                runtime_accounts,
                release_set: [22; 32],
                generation: 1,
                clock_slot: observation().slot,
                minimum_finalized_slot: observation().slot,
                hot_outer: Some(checked),
            },
            checked,
        )
    }

    fn ordinary_logical_lengths() -> Vec<u32> {
        let mut output = vec![0_u32; usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)];
        *output.get_mut(0).expect("root") = u32::try_from(
            CAPABILITY_ROOT_HEADER_BYTES_V1
                + dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1,
        )
        .expect("root width");
        *output.get_mut(1).expect("config") =
            u32::try_from(dclutch_direct_codec::successor::DIRECT_EXECUTION_CONFIG_BYTES_V1)
                .expect("config width");
        *output.get_mut(2).expect("Product") =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product width");
        // One geometry, derived rather than restated: `DirectOrdinaryGeometryV3`
        // owns what each runtime-width record measures at a given market
        // width, and the profile cross-checks all five against ONE of them.
        let geometry = DirectOrdinaryGeometryV3::CANONICAL;
        *output.get_mut(3).expect("portfolio") =
            geometry.portfolio_record_bytes().expect("portfolio width");
        *output.get_mut(4).expect("basis") = 24;
        for coordinate in [5_usize, 8] {
            *output.get_mut(coordinate).expect("maker") =
                u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker width");
        }
        *output.get_mut(7).expect("lifecycle RentCredit") =
            u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit width");
        *output.get_mut(10).expect("Rent program") =
            u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES)
                .expect("Rent program width");
        *output.get_mut(13).expect("Claims aggregate") = geometry
            .claims_aggregate_record_bytes()
            .expect("Claims aggregate width");
        *output.get_mut(14).expect("basis alias") = *output.get(4).expect("basis");
        *output.get_mut(16).expect("Product alias") =
            u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product width");
        *output.get_mut(18).expect("domain") = geometry
            .result_domain_record_bytes()
            .expect("result domain width");
        *output.get_mut(20).expect("portfolio alias") = *output.get(3).expect("portfolio");
        *output.get_mut(22).expect("Registry") = 17;
        *output.get_mut(23).expect("Core") =
            u32::try_from(dclutch_market_core_codec::STATE_BYTES).expect("Core width");
        *output.get_mut(24).expect("activation") =
            u32::try_from(dclutch_registry_contract::ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1)
                .expect("activation width");
        *output.get_mut(25).expect("Registry program") =
            u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES)
                .expect("Registry program width");
        *output.get_mut(26).expect("Trading program") =
            u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES)
                .expect("Trading program width");
        *output.get_mut(27).expect("Claims ProgramData") = 1_024;
        *output.get_mut(28).expect("Claims program") =
            u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES)
                .expect("Claims program width");
        *output.get_mut(29).expect("source staging") = 1_024;
        *output.get_mut(30).expect("Core program") =
            u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES)
                .expect("Core program width");
        *output.get_mut(31).expect("destination staging") = 1_024;
        let position = geometry
            .claims_position_record_bytes()
            .expect("Claims Position width");
        *output.get_mut(32).expect("source Position") = position;
        *output.get_mut(33).expect("destination Position") = position;
        *output.get_mut(35).expect("Core alias") = *output.get(23).expect("Core");
        *output.get_mut(36).expect("activation alias") = *output.get(24).expect("activation");
        *output.get_mut(37).expect("Registry alias") = *output.get(25).expect("Registry");
        *output.get_mut(38).expect("Claims alias") = *output.get(26).expect("Claims");
        *output.get_mut(39).expect("ProgramData alias") = *output.get(27).expect("ProgramData");
        *output.get_mut(40).expect("Realm") = u32::try_from(REALM_BYTES).expect("Realm width");
        *output.get_mut(42).expect("Custody replay") =
            u32::try_from(CustodyReplayLayoutV1::BYTES).expect("replay width");
        *output.get_mut(43).expect("mint") = 82;
        *output.get_mut(44).expect("buyer token") = 165;
        *output.get_mut(45).expect("seller token") = 165;
        *output.get_mut(47).expect("token program") = 36;
        *output.get_mut(73).expect("fee token") = 165;
        for (account, representative) in [
            (49, 23),
            (50, 24),
            (51, 25),
            (52, 26),
            (53, 27),
            (54, 40),
            (55, 41),
            (56, 42),
            (57, 43),
            (58, 44),
            (59, 45),
            (60, 46),
            (61, 47),
            (63, 23),
            (64, 24),
            (65, 25),
            (66, 26),
            (67, 27),
            (68, 40),
            (69, 41),
            (70, 42),
            (71, 43),
            (72, 44),
            (74, 46),
            (75, 47),
            (77, 23),
            (78, 24),
            (79, 25),
            (80, 26),
            (81, 27),
            (82, 40),
            (83, 41),
            (84, 42),
            (85, 43),
            (86, 44),
            (87, 73),
            (88, 46),
            (89, 47),
        ] {
            let value = *output.get(representative).expect("representative");
            *output.get_mut(account).expect("route alias") = value;
        }
        // Descriptive only: the Custody program's rule is opaque, so no loader
        // record width is pinned at this coordinate.
        *output
            .get_mut(usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3))
            .expect("Custody program") =
            u32::try_from(dclutch_registry_svm::LOADER_V3_PROGRAM_BYTES)
                .expect("Custody program width");
        output
    }

    fn put_rent(state: &mut DirectInlineHotStateV3, rent: &Rent) {
        let rent_account = &mut state
            .fixed_accounts
            .get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3)
            .expect("Rent account")
            .account;
        rent_account.owner = sysvar::ID;
        rent_account.data = vec![0_u8; Rent::size_of()];
        let mut lamports = rent_account.lamports;
        let mut info = AccountInfo::new(
            &rent_account.key,
            false,
            false,
            &mut lamports,
            &mut rent_account.data,
            &rent_account.owner,
            false,
        );
        rent.to_account_info(&mut info).expect("serialize Rent");
    }

    fn put_finalized_record(
        state: &mut DirectInlineHotStateV3,
        rent: &Rent,
        raw_coordinate: usize,
        staging_coordinate: usize,
        schema: [u8; 32],
        data: Vec<u8>,
    ) {
        let registry = state
            .fixed_accounts
            .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
            .expect("Registry")
            .account
            .key;
        let digest = hash(&data).to_bytes();
        let raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &registry,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &registry,
        )
        .0;
        let raw_account = &mut state
            .fixed_accounts
            .get_mut(raw_coordinate)
            .expect("raw record")
            .account;
        raw_account.key = raw;
        raw_account.owner = registry;
        raw_account.lamports = rent.minimum_balance(data.len());
        raw_account.executable = false;
        raw_account.data = data;
        let staging_account = &mut state
            .fixed_accounts
            .get_mut(staging_coordinate)
            .expect("staging cursor")
            .account;
        staging_account.key = staging;
        staging_account.owner = system_program::ID;
        staging_account.lamports = 0;
        staging_account.executable = false;
        staging_account.data.clear();
    }

    fn core_id(bytes: [u8; 32]) -> dclutch_core_contract::ContentId {
        dclutch_core_contract::ContentId::new(bytes).expect("nonzero core content ID")
    }

    fn capability_id(bytes: [u8; 32]) -> dclutch_capability_contract::ContentId {
        dclutch_capability_contract::ContentId::new(bytes).expect("nonzero capability content ID")
    }

    fn chain_artifact_fixture() -> (DirectInlineHotStateV3, [u8; 456]) {
        let (mut state, checked) = hot38_state();
        let rent = Rent::default();
        put_rent(&mut state, &rent);
        let lengths = ordinary_logical_lengths();
        let capacity_profile = [0x44; 32];
        let bundle =
            build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
                account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &lengths,
                },
                capacity_profile,
            })
            .expect("ordinary artifact bundle");
        let descriptor = CapabilityProgramV4::decode(&bundle.descriptor).expect("descriptor");
        let config = DirectExecutionConfigV1::new(1_000_000, 25, [0x45; 32])
            .expect("config")
            .encode();
        let config_digest = hash(&config).to_bytes();
        let descriptor_digest = hash(&bundle.descriptor).to_bytes();
        let set_entry = CapabilityProgramSetEntryV2::new(
            DirectExecutionActionV3::InlineOrdinary as u32,
            CapabilityDescriptorReferenceV2::new(
                core_id(CAPABILITY_PROGRAM_SCHEMA_ID_V4),
                core_id(descriptor_digest),
            ),
        );
        let mut program_set =
            vec![0_u8; encoded_program_set_bytes_v2(1).expect("ProgramSet width")];
        encode_program_set_v2(12, SelectorWidthV2::U32, &[set_entry], &mut program_set)
            .expect("ProgramSet");
        let program_set_digest = hash(&program_set).to_bytes();
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(1).expect("Rent funding"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("Funding amounts");
        let entry = CapabilityEntryV1::new(
            capability_id(dclutch_direct_codec::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3),
            capability_id(program_set_digest),
            capability_id(config_digest),
            capability_id(capacity_profile),
            capability_id(DIRECT_ROOT_SCHEMA_ID_V1),
            capability_id(hash(&bundle.lifecycle_policy).to_bytes()),
            ActivationPolicy::PrepaidLazy,
            1_000,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("Funding quote"),
        )
        .expect("manifest entry");
        let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest");
        let manifest_digest = hash(&manifest).to_bytes();
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            capability_id(manifest_digest),
            capability_id(dclutch_direct_codec::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3),
            capability_id(program_set_digest),
            capability_id(config_digest),
        )
        .expect("selection");
        let market = state
            .fixed_accounts
            .get(HOT_MARKET_ACCOUNT_V3)
            .expect("Market")
            .account
            .key;
        let release_set = [0x46; 32];
        state.release_set = release_set;
        state.generation = 9;
        let registry = state
            .fixed_accounts
            .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
            .expect("Registry")
            .account
            .key;
        let record_bumps = |schema: [u8; 32], digest: [u8; 32]| {
            (
                Pubkey::find_program_address(
                    &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                    &registry,
                )
                .1,
                Pubkey::find_program_address(
                    &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                    &registry,
                )
                .1,
            )
        };
        let manifest_bumps =
            record_bumps(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifest_digest);
        let program_set_bumps = record_bumps(
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            program_set_digest,
        );
        let config_bumps = record_bumps(DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, config_digest);
        let header = CapabilityRootHeaderV1::new(
            core_id(release_set),
            market.to_bytes(),
            state.generation,
            selection
                .with_capability_release_record_bumps(program_set_bumps.0, program_set_bumps.1),
            SelectedRecordBumpsV1::new(
                manifest_bumps.0,
                manifest_bumps.1,
                config_bumps.0,
                config_bumps.1,
            ),
        )
        .expect("root header");
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(&DirectRootStateV1::new().encode());
        let root_key =
            Pubkey::find_program_address(&header.seeds().as_slices(), &checked.trading_program).0;
        let root = &mut state
            .fixed_accounts
            .get_mut(HOT_ROOT_ACCOUNT_V3)
            .expect("root")
            .account;
        root.key = root_key;
        root.owner = checked.trading_program;
        root.lamports = rent.minimum_balance(root_data.len());
        root.data = root_data;
        state
            .runtime_accounts
            .get_mut(0)
            .expect("runtime root")
            .account = root.clone();

        for (raw, staging, schema, data) in [
            (
                HOT_MANIFEST_RAW_ACCOUNT_V3,
                HOT_MANIFEST_STAGING_ACCOUNT_V3,
                CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
                manifest,
            ),
            (
                HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
                HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
                CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
                program_set,
            ),
            (
                HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
                HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
                CAPABILITY_PROGRAM_SCHEMA_ID_V4,
                bundle.descriptor.to_vec(),
            ),
            (
                HOT_CONFIG_RAW_ACCOUNT_V3,
                HOT_CONFIG_STAGING_ACCOUNT_V3,
                DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
                config.to_vec(),
            ),
            (
                HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
                HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
                descriptor.account_profile().schema().to_bytes(),
                bundle.account_profile.to_vec(),
            ),
            (
                HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
                HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
                descriptor.request_profile().schema().to_bytes(),
                bundle.request_profile.to_vec(),
            ),
            (
                HOT_TRANSITION_RAW_ACCOUNT_V3,
                HOT_TRANSITION_STAGING_ACCOUNT_V3,
                descriptor.transition().schema().to_bytes(),
                bundle.transition.to_vec(),
            ),
            (
                HOT_EFFECT_RAW_ACCOUNT_V3,
                HOT_EFFECT_STAGING_ACCOUNT_V3,
                descriptor.effect().schema().to_bytes(),
                bundle.effect.to_vec(),
            ),
            (
                HOT_LIFECYCLE_RAW_ACCOUNT_V3,
                HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
                descriptor.lifecycle().schema().to_bytes(),
                bundle.lifecycle_policy.to_vec(),
            ),
            (
                HOT_STRATEGY_RAW_ACCOUNT_V3,
                HOT_STRATEGY_STAGING_ACCOUNT_V3,
                descriptor.strategy().schema().to_bytes(),
                bundle.strategy.to_vec(),
            ),
        ] {
            put_finalized_record(&mut state, &rent, raw, staging, schema, data);
        }
        let request = compile_direct_inline_request_v3(intent(0, 1), intent(1, 2), 1_000, 500_000)
            .expect("Direct request");
        (state, request)
    }

    fn packed_runtime_accounts(
        profile: dclutch_account_profile_contract::v2::AccountProfileV2<'_>,
        outcome_count: u32,
    ) -> Vec<ObservedAccountMetaV3> {
        let count = profile
            .physical_account_count_with_dynamic_spans(outcome_count, &[])
            .expect("physical account count");
        (0..count)
            .map(|ordinal| {
                let geometry = profile
                    .physical_account_geometry_with_dynamic_spans(outcome_count, &[], ordinal)
                    .expect("physical geometry");
                let privileges = geometry.privileges();
                let data_bytes = match geometry.data() {
                    PhysicalAccountDataGeometryV2::Exact { bytes }
                    | PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: bytes } => bytes,
                    PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable {
                        minimum_bytes,
                    } => minimum_bytes
                        .checked_add(7)
                        .expect("variable fixture width"),
                    PhysicalAccountDataGeometryV2::Opaque => 17,
                };
                ObservedAccountMetaV3 {
                    account: ObservedAccount {
                        observation: observation(),
                        key: Pubkey::new_unique(),
                        owner: key(220),
                        lamports: 1,
                        executable: privileges.executable(),
                        data: vec![0; data_bytes],
                    },
                    is_signer: privileges.signer(),
                    is_writable: privileges.writable(),
                }
            })
            .collect()
    }

    #[test]
    fn inline_request_has_exact_signed_offsets_and_u32_outcome() {
        let seller = intent(0, 1);
        let buyer = intent(1, 2);
        let request = compile_direct_inline_request_v3(seller, buyer, 1_000, 500_000)
            .expect("inline request");
        assert_eq!(request.len(), 456);
        let seller_message = seller.intent.signed_preimage().expect("seller message");
        let buyer_message = buyer.intent.signed_preimage().expect("buyer message");
        assert_eq!(request.get(64..236), Some(seller_message.as_slice()));
        assert_eq!(request.get(268..440), Some(buyer_message.as_slice()));
        assert!(matches!(
            DirectExecutionRequestV3::decode(&request, 70_001),
            Ok(DirectExecutionRequestV3::InlineOrdinary(_))
        ));
        assert_eq!(
            request.get(440..448),
            Some(1_000_u64.to_le_bytes().as_slice())
        );
        assert_eq!(
            request.get(448..456),
            Some(500_000_u64.to_le_bytes().as_slice())
        );
    }

    #[test]
    fn chain_frame_is_the_only_artifact_authority() {
        let (state, request) = chain_artifact_fixture();
        let bundle = authenticate_chain_artifacts_v4(&state, &request, 70_001)
            .expect("chain-selected Direct artifacts");
        assert_eq!(bundle.action, DirectExecutionActionV3::InlineOrdinary);

        let mut wrong_root_owner = state.clone();
        wrong_root_owner
            .fixed_accounts
            .get_mut(HOT_ROOT_ACCOUNT_V3)
            .expect("root")
            .account
            .owner = key(199);
        assert_eq!(
            authenticate_chain_artifacts_v4(&wrong_root_owner, &request, 70_001),
            Err(Error::ArtifactMismatch)
        );

        let mut live_staging = state.clone();
        live_staging
            .fixed_accounts
            .get_mut(HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)
            .expect("descriptor staging")
            .account
            .data = vec![1];
        assert_eq!(
            authenticate_chain_artifacts_v4(&live_staging, &request, 70_001),
            Err(Error::ArtifactMismatch)
        );

        let mut substituted_descriptor = state;
        *substituted_descriptor
            .fixed_accounts
            .get_mut(HOT_DESCRIPTOR_RAW_ACCOUNT_V3)
            .expect("descriptor")
            .account
            .data
            .get_mut(16)
            .expect("descriptor kind") ^= 1;
        assert_eq!(
            authenticate_chain_artifacts_v4(&substituted_descriptor, &request, 70_001),
            Err(Error::ArtifactMismatch)
        );
    }

    #[test]
    fn adjacent_ed25519_references_exact_current_direct_or_registry_instruction() {
        let seller = intent(0, 1);
        let buyer = intent(1, 2);
        let request =
            compile_direct_inline_request_v3(seller, buyer, 1_000, 500_000).expect("request");
        let envelope =
            HotExecutionEnvelopeV3::new(456, [1; 32], [7; 32], 9, [2; 32]).expect("envelope");
        let mut hot = envelope.to_bytes().to_vec();
        hot.extend_from_slice(&request);
        let direct = native_ed25519_instruction(
            DirectNativeEvidenceContainerV3::TradingHot,
            DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
            &hot,
            [seller.signature, buyer.signature],
        )
        .expect("direct native evidence");
        assert_eq!(direct.program_id, ed25519_program::ID);
        assert_eq!(direct.data.first().copied(), Some(2));
        assert_eq!(direct.data.len(), 158);
        for (descriptor, expected_public_key, expected_message) in
            [(2_usize, 160_u16, 192_u16), (16, 364, 396)]
        {
            assert_eq!(
                read_test_u16(&direct.data, descriptor + 4),
                expected_public_key
            );
            assert_eq!(
                read_test_u16(&direct.data, descriptor + 8),
                expected_message
            );
            assert_eq!(read_test_u16(&direct.data, descriptor + 10), 172);
            assert_eq!(read_test_u16(&direct.data, descriptor + 2), u16::MAX);
            assert_eq!(read_test_u16(&direct.data, descriptor + 6), 3);
            assert_eq!(read_test_u16(&direct.data, descriptor + 12), 3);
        }

        let registry = Instruction {
            program_id: key(199),
            accounts: Vec::new(),
            data: hot.clone(),
        };
        let mut sequence = vec![Instruction {
            program_id: key(198),
            accounts: Vec::new(),
            data: Vec::new(),
        }];
        append_direct_headerless_registry_native_evidence_v6(
            &mut sequence,
            registry,
            [seller.signature, buyer.signature],
        )
        .expect("Registry evidence");
        assert_eq!(sequence.len(), 3);
        let native = sequence.get(1).expect("native evidence");
        for (descriptor, expected_public_key, expected_message) in
            [(2_usize, 160_u16, 192_u16), (16, 364, 396)]
        {
            assert_eq!(
                read_test_u16(&native.data, descriptor + 4),
                expected_public_key
            );
            assert_eq!(read_test_u16(&native.data, descriptor + 6), 2);
            assert_eq!(
                read_test_u16(&native.data, descriptor + 8),
                expected_message
            );
            assert_eq!(read_test_u16(&native.data, descriptor + 12), 2);
        }

        let mut registered_intent = seller.intent;
        registered_intent.lifecycle = 2;
        let registration = DirectRegistrationRequestV3 {
            participant: DirectSignedParticipantV3 {
                maker: seller.maker.to_bytes(),
                intent: registered_intent,
            },
            maker_rent_credit: [31; 32],
            record_rent_credit: [32; 32],
            maker_rent_principal: 10_000,
            record_rent_principal: 20_000,
        };
        let mut registered_request = [0_u8; DIRECT_REGISTRATION_REQUEST_BYTES_V3];
        encode_direct_registration_request_v3_atomic(
            DirectExecutionActionV3::RegisterSell,
            registration,
            &mut registered_request,
        )
        .expect("registered request");
        let registered_envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(registered_request.len()).expect("request width"),
            [1; 32],
            [7; 32],
            9,
            [2; 32],
        )
        .expect("registered envelope");
        let mut registered_hot = registered_envelope.to_bytes().to_vec();
        registered_hot.extend_from_slice(&registered_request);
        let registered = Instruction {
            program_id: key(199),
            accounts: Vec::new(),
            data: registered_hot,
        };
        let mut registered_sequence = Vec::new();
        append_direct_headerless_registry_native_evidence_many_v6(
            &mut registered_sequence,
            registered,
            DirectExecutionActionV3::RegisterSell,
            70_001,
            core::slice::from_ref(&seller.signature),
        )
        .expect("registered Registry evidence");
        let registered_native = registered_sequence.first().expect("native evidence");
        assert_eq!(registered_native.data.first().copied(), Some(1));
        assert_eq!(registered_native.data.len(), 80);
        assert_eq!(read_test_u16(&registered_native.data, 2 + 4), 160);
        assert_eq!(read_test_u16(&registered_native.data, 2 + 6), 1);
        assert_eq!(read_test_u16(&registered_native.data, 2 + 8), 192);
        assert_eq!(read_test_u16(&registered_native.data, 2 + 10), 172);
        assert_eq!(read_test_u16(&registered_native.data, 2 + 12), 1);

        let unchanged = registered_sequence.clone();
        assert_eq!(
            append_direct_headerless_registry_native_evidence_many_v6(
                &mut registered_sequence,
                unchanged.last().expect("Registry").clone(),
                DirectExecutionActionV3::RegisterSell,
                70_001,
                &[],
            ),
            Err(Error::ArtifactMismatch)
        );
        assert_eq!(registered_sequence, unchanged);
    }

    fn read_test_u16(bytes: &[u8], offset: usize) -> u16 {
        let end = offset.checked_add(2).expect("test offset");
        let encoded = bytes.get(offset..end).expect("test u16 bytes");
        u16::from_le_bytes(<[u8; 2]>::try_from(encoded).expect("test u16 width"))
    }

    #[test]
    fn zero_signature_and_maker_alias_refuse_before_artifact_use() {
        let seller = intent(0, 1);
        let mut buyer = intent(1, 2);
        buyer.signature = [0; 64];
        assert_eq!(
            compile_direct_inline_request_v3(seller, buyer, 1, 1),
            Err(Error::ZeroIdentity)
        );
        let mut buyer = intent(1, 2);
        buyer.maker = seller.maker;
        assert_eq!(
            compile_direct_inline_request_v3(seller, buyer, 1, 1),
            Err(Error::ZeroIdentity)
        );

        let (state, _) = hot38_state();
        let mut registered_intent = seller.intent;
        registered_intent.lifecycle = 2;
        let registration = DirectRegistrationRequestV3 {
            participant: DirectSignedParticipantV3 {
                maker: seller.maker.to_bytes(),
                intent: registered_intent,
            },
            maker_rent_credit: [31; 32],
            record_rent_credit: [32; 32],
            maker_rent_principal: 10_000,
            record_rent_principal: 20_000,
        };
        assert_eq!(
            build_direct_registration_hot_v4(
                &state,
                DirectExecutionActionV3::RegisterSell,
                registration,
                [0; 64],
            ),
            Err(Error::ZeroIdentity)
        );
        assert_eq!(
            build_direct_registration_hot_v4(
                &state,
                DirectExecutionActionV3::InlineOrdinary,
                registration,
                seller.signature,
            ),
            Err(Error::EconomicMismatch)
        );
    }

    #[test]
    fn registered_terminal_exteriors_bind_native_slices_and_unsigned_expiry() {
        let seller = intent(0, 1);
        let mut registered_intent = seller.intent;
        registered_intent.lifecycle = 2;
        let terminal = DirectSignedTerminalRequestV3 {
            maker: seller.maker.to_bytes(),
            intent: registered_intent,
        };

        let mut cancel = [0_u8; DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3];
        encode_direct_registered_cancel_request_v3_atomic(terminal, 70_001, &mut cancel)
            .expect("registered cancel request");
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(cancel.len()).expect("cancel width"),
            [1; 32],
            [7; 32],
            9,
            [2; 32],
        )
        .expect("cancel envelope");
        let mut cancel_hot = envelope.to_bytes().to_vec();
        cancel_hot.extend_from_slice(&cancel);
        let native = native_ed25519_instruction_many(
            DirectNativeEvidenceContainerV3::TradingHot,
            DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
            &cancel_hot,
            DirectExecutionActionV3::CancelRegistered,
            70_001,
            core::slice::from_ref(&seller.signature),
        )
        .expect("cancel native evidence");
        assert_eq!(native.program_id, ed25519_program::ID);
        assert!(native.accounts.is_empty());
        assert_eq!(native.data.first().copied(), Some(1));
        assert_eq!(native.data.len(), 80);
        assert_eq!(read_test_u16(&native.data, 2 + 4), 160);
        assert_eq!(read_test_u16(&native.data, 2 + 6), 3);
        assert_eq!(read_test_u16(&native.data, 2 + 8), 192);
        assert_eq!(read_test_u16(&native.data, 2 + 10), 172);
        assert_eq!(read_test_u16(&native.data, 2 + 12), 3);

        // The signature descriptors bind the action-selected request. Turning
        // the exact Cancel bytes into Expire under them refuses atomically.
        let mut substituted = cancel_hot.clone();
        *substituted
            .get_mut(HOT_FAMILY_REQUEST_OFFSET_V3 + 12)
            .expect("selector byte") = DirectExecutionActionV3::ExpireRegistered as u8;
        assert_eq!(
            native_ed25519_instruction_many(
                DirectNativeEvidenceContainerV3::TradingHot,
                DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
                &substituted,
                DirectExecutionActionV3::CancelRegistered,
                70_001,
                core::slice::from_ref(&seller.signature),
            ),
            Err(Error::ArtifactMismatch)
        );

        let mut expire = [0_u8; DIRECT_EMPTY_ACTION_REQUEST_BYTES_V3];
        encode_direct_empty_action_request_v3_atomic(
            DirectExecutionActionV3::ExpireRegistered,
            70_001,
            &mut expire,
        )
        .expect("registered expiry request");
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(expire.len()).expect("expiry width"),
            [1; 32],
            [7; 32],
            9,
            [2; 32],
        )
        .expect("expiry envelope");
        let mut expire_hot = envelope.to_bytes().to_vec();
        expire_hot.extend_from_slice(&expire);
        let trading = Instruction {
            program_id: key(199),
            accounts: Vec::new(),
            data: expire_hot.clone(),
        };
        let unsigned = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1),
            ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
            trading.clone(),
        ];
        assert_eq!(
            validate_direct_hot_instruction_sequence_v4(
                DirectExecutionActionV3::ExpireRegistered,
                70_001,
                &expire_hot,
                &unsigned,
            ),
            Ok(())
        );
        let mut smuggled = unsigned;
        smuggled.insert(2, native);
        assert_eq!(
            validate_direct_hot_instruction_sequence_v4(
                DirectExecutionActionV3::ExpireRegistered,
                70_001,
                &expire_hot,
                &smuggled,
            ),
            Err(DirectInlineTransactionErrorV3::InstructionSequence)
        );

        let (state, _) = hot38_state();
        assert_eq!(
            build_direct_registered_cancel_hot_v4(&state, terminal, [0; 64]),
            Err(Error::ZeroIdentity)
        );
        let mut malformed = terminal;
        malformed.intent.lifecycle = 1;
        let mut malformed_bytes = [0_u8; DIRECT_REGISTERED_CANCEL_REQUEST_BYTES_V3];
        assert!(
            encode_direct_registered_cancel_request_v3_atomic(
                malformed,
                70_001,
                &mut malformed_bytes,
            )
            .is_err()
        );
        // `hot38_state` deliberately does not carry a finalized Product graph.
        // Both nonzero terminal exteriors must stop at that semantic owner,
        // before either can borrow its otherwise valid InlineOrdinary family.
        assert_eq!(
            build_direct_registered_cancel_hot_v4(&state, terminal, seller.signature),
            Err(Error::ProductGraphMismatch)
        );
        assert_eq!(
            build_direct_registered_expire_hot_v4(&state),
            Err(Error::ProductGraphMismatch)
        );
    }

    #[test]
    fn hot38_requires_all_five_injected_runtime_coordinates() {
        let (state, checked) = hot38_state();
        assert_eq!(validate_frame(&state, checked), Ok(observation()));

        let mut substituted = state.clone();
        let root = substituted
            .runtime_accounts
            .first()
            .expect("runtime root")
            .clone();
        *substituted
            .runtime_accounts
            .get_mut(1)
            .expect("runtime config") = root;
        assert_eq!(
            validate_frame(&substituted, checked),
            Err(Error::RuntimeProfileMismatch)
        );

        let mut stale_prefix = state;
        stale_prefix.fixed_accounts.truncate(30);
        assert_eq!(
            validate_frame(&stale_prefix, checked),
            Err(Error::FixedFrameMismatch)
        );
    }

    #[test]
    fn runtime_profile_accepts_only_packed_kernel_geometry() {
        let (mut state, request) = chain_artifact_fixture();
        let profile_bytes = state
            .fixed_accounts
            .get(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)
            .expect("AccountProfile")
            .account
            .data
            .clone();
        let profile =
            dclutch_account_profile_contract::v2::AccountProfileV2::decode(&profile_bytes)
                .expect("AccountProfile");
        state.runtime_accounts = packed_runtime_accounts(profile, 3);
        assert!(
            state.runtime_accounts.len() < profile.logical_account_count(3).expect("logical count")
        );
        let bundle = authenticate_chain_artifacts_v4(&state, &request, 3).expect("artifact bundle");
        assert_eq!(validate_runtime_profile(&state, bundle, 3), Ok(()));

        let mut duplicate_alias = state.clone();
        duplicate_alias.runtime_accounts.push(
            state
                .runtime_accounts
                .first()
                .expect("first account")
                .clone(),
        );
        let bundle = authenticate_chain_artifacts_v4(&duplicate_alias, &request, 3)
            .expect("artifact bundle");
        assert_eq!(
            validate_runtime_profile(&duplicate_alias, bundle, 3),
            Err(Error::RuntimeProfileMismatch)
        );

        let basis_ordinal = profile
            .physical_account_ordinal(
                3,
                dclutch_capability_program_contract::hot_v3::HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3,
            )
            .expect("basis ordinal");
        let mut short_variable = state.clone();
        short_variable
            .runtime_accounts
            .get_mut(basis_ordinal)
            .expect("basis account")
            .account
            .data
            .clear();
        let bundle =
            authenticate_chain_artifacts_v4(&short_variable, &request, 3).expect("artifact bundle");
        assert_eq!(
            validate_runtime_profile(&short_variable, bundle, 3),
            Err(Error::RuntimeProfileMismatch)
        );

        let claims_ordinal = profile
            .physical_account_ordinal(3, 13)
            .expect("Claims Market ordinal");
        let mut short_affine = state;
        short_affine
            .runtime_accounts
            .get_mut(claims_ordinal)
            .expect("Claims Market")
            .account
            .data
            .pop();
        let bundle =
            authenticate_chain_artifacts_v4(&short_affine, &request, 3).expect("artifact bundle");
        assert_eq!(
            validate_runtime_profile(&short_affine, bundle, 3),
            Err(Error::RuntimeProfileMismatch)
        );
    }

    #[test]
    fn canonical_lut_compiles_exact_budgeted_packet_and_reports_sole_payer() {
        let report = transaction_report(0);
        let payer = key(1);
        let lookup = lookup(&report, payer);
        let plan =
            compile_direct_inline_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &lookup)
                .expect("packet-safe Direct action");
        assert_eq!(plan.required_signers, vec![payer]);
        assert_eq!(plan.message.required_signatures, 1);
        assert_eq!(plan.message.loaded_addresses, 89);
        assert_eq!(plan.message.wire_bytes, 1_212);
        assert_eq!(
            crate::versioned::PACKET_DATA_BYTES - plan.message.wire_bytes,
            20
        );
        let message = match &plan.message.message {
            solana_message::VersionedMessage::V0(message) => message,
            _ => panic!("Direct compiler emitted a legacy message"),
        };
        assert_eq!(message.instructions.len(), 4);
        assert_eq!(message.instructions[0].data, vec![2, 0xc0, 0x5c, 0x15, 0]);
        // RequestHeapFrame(65_536): discriminant 1, then the u32 little-endian.
        assert_eq!(message.instructions[1].data, vec![1, 0, 0, 1, 0]);
        assert_eq!(message.instructions[2].data.len(), 158);
        assert_eq!(message.instructions[2].data[2 + 6], 3);
        assert_eq!(message.instructions[2].data[2 + 12], 3);
        assert_eq!(plan.outcome_count, 258);
        assert_eq!(
            plan.selected_program_schema,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4
        );
        assert_eq!(plan.selected_program, [8; 32]);
    }

    #[test]
    fn missing_reordered_or_substituted_budget_and_evidence_refuse() {
        let report = transaction_report(0);
        assert_eq!(
            validate_direct_hot_instruction_sequence_v4(
                DirectExecutionActionV3::InlineOrdinary,
                report.outcome_count,
                &report.hot_instruction_data,
                &report.instructions[1..],
            ),
            Err(DirectInlineTransactionErrorV3::InstructionSequence)
        );

        for case in 0..5 {
            let mut hostile = report.clone();
            match case {
                0 => {
                    hostile.instructions[0] =
                        ComputeBudgetInstruction::set_compute_unit_limit(1_399_999)
                }
                1 => hostile.instructions.swap(0, 1),
                2 => hostile.instructions[2].data[2 + 4] ^= 1,
                3 => hostile.instructions[2].data[2 + 6] ^= 1,
                _ => hostile.instructions[2].data[2 + 12] ^= 1,
            }
            assert_eq!(
                validate_direct_hot_instruction_sequence_v4(
                    DirectExecutionActionV3::InlineOrdinary,
                    hostile.outcome_count,
                    &hostile.hot_instruction_data,
                    &hostile.instructions,
                ),
                Err(DirectInlineTransactionErrorV3::InstructionSequence),
                "hostile sequence case {case}"
            );
        }
    }

    #[test]
    fn stale_extra_lookup_and_oversized_packet_refuse() {
        let payer = key(1);
        let report = transaction_report(0);
        let mut stale = lookup(&report, payer);
        stale.observation.slot += 1;
        assert_eq!(
            compile_direct_inline_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &stale,),
            Err(DirectInlineTransactionErrorV3::Snapshot)
        );

        let mut extra = lookup(&report, payer);
        let decoded = AddressLookupTable::deserialize(&extra.data).expect("table");
        let mut addresses = decoded.addresses.into_owned();
        addresses.push(key(249));
        addresses.sort_unstable_by_key(Pubkey::to_bytes);
        extra.data = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(addresses),
        }
        .serialize_for_tests()
        .expect("extra table");
        assert_eq!(
            compile_direct_inline_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &extra,),
            Err(DirectInlineTransactionErrorV3::LookupTable)
        );

        let oversized = transaction_report(20);
        let oversized_lookup = lookup(&oversized, payer);
        assert_eq!(
            compile_direct_inline_hot_v0(
                &oversized,
                payer,
                Hash::new_from_array([16; 32]),
                &oversized_lookup,
            ),
            Err(DirectInlineTransactionErrorV3::Routing(
                crate::versioned::Error::PacketTooLarge
            ))
        );

        let mut wrong_schema = report;
        wrong_schema.selected_program_schema[0] ^= 1;
        let lookup = lookup(&wrong_schema, payer);
        assert_eq!(
            compile_direct_inline_hot_v0(
                &wrong_schema,
                payer,
                Hash::new_from_array([16; 32]),
                &lookup,
            ),
            Err(DirectInlineTransactionErrorV3::Snapshot)
        );
    }
}
