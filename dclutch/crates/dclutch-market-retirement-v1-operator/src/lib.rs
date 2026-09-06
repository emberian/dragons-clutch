//! Chain-derived construction for the canonical aggregate Market retirement.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod terminal_stage_order_v1;

use dclutch_claims::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2, read_claim_v2,
    },
    market_closure_v1::{
        CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
        CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1,
        CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1, ClaimsMarketClosureReceiptInputV1,
        ClaimsMarketClosureReceiptV1, ClaimsMarketClosureRequestInputV1,
        ClaimsMarketClosureRequestV1,
    },
    protocol_position_v2::failure_escrow_v1,
    retirement_checkpoint_handoff_v1::{
        CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_POST_DIGEST_DOMAIN_V1,
        ClaimsRetirementCheckpointHandoffReceiptV1, ClaimsRetirementCheckpointHandoffRequestV1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_POSTSTATE_DOMAIN_V1, CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyReceiptV1, CustodyReplaySeedsV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, OperationV1, ReceiptEvidenceV1,
};
use dclutch_market::realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_market::rent::lifecycle_v2::{
    LifecycleAccountIdV2, LifecycleRentCoreCloseAuthoritySeedsV2, LifecycleRentCreditV2,
};
use dclutch_market::{
    AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1, AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1,
    AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1, AGGREGATE_RETIREMENT_FINISH_MAGIC_V1,
    AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1, Action, AggregateRetirementSuffixBindingV1,
    AggregateRetirementSuffixRequestV1, CoreState, MarketCoreStateSeedsV2, Phase, REQUEST_BYTES,
    RETIREMENT_BUNDLE_BYTES_V1, RETIREMENT_CUSTODY_RECEIPT_COUNT_V1,
    RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1, RETIREMENT_ROLE_COUNT_V1, Request,
    RetirementBundleInputV1, RetirementBundleV1, STATE_BYTES,
};
use dclutch_product::economic_slice::refunding_failure_index;
use dclutch_product::payoff::runtime_v3::{ProductBasisV3, semantic_basis_id_v3};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{
    CallerAuthoritySeedsV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2, ProtocolInfrastructureProfileV2,
};
use dclutch_registry::svm::{
    ProgramDataV3View, ProgramV3View,
    continuation_v1::{
        REGISTRY_CONTINUATION_REQUEST_BYTES_V1, RegistryContinuationAdmissionSeedsV1,
        RegistryContinuationRequestV1,
    },
};
use dclutch_registry::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1,
    ArtifactReleaseV1, DeploymentObservationV1, require_slot_pinned_release_v1,
};
use dclutch_resolution_core_v3_operator::authenticate_resolution_retirement_receipt_v3;
pub use dclutch_resolution_core_v3_operator::{
    Finality, Observation, ObservedAccount, ResolutionRetirementReceiptFactsV3,
};
use dclutch_source::resolution::SourceClosureReceiptV3;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_program_pack::Pack;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use spl_token_interface::state::{Account as SplTokenAccount, AccountState};

/// Exact top-level Registry prefix before the nested Core retirement frame.
pub const REGISTRY_RETIREMENT_CONTINUATION_PREFIX_ACCOUNTS_V1: usize = 10;
/// Exact Core retirement frame before the invocation-scoped Registry admission.
pub const CORE_RETIREMENT_ACCOUNT_COUNT_V1: usize = 35;
/// Decision 0025's escrow tail: the Position, its admission and the basis record.
pub const CORE_RETIREMENT_ESCROW_TAIL_ACCOUNTS_V1: usize = 3;
/// Exact Core retirement frame for a REFUNDING Market's checkpointed route.
///
/// Present on ALL FOUR packets, because one retirement presents one frame and
/// the three suffix packets carry accounts they never read. This is the whole
/// of shape A's frame growth, and the two counts here are the ONLY two widths
/// a checkpointed retirement's packet may have -- a consumer that has to tell
/// the shapes apart asks these rather than spelling 35 and 38 again.
pub const CORE_REFUNDING_RETIREMENT_ACCOUNT_COUNT_V1: usize =
    CORE_RETIREMENT_ACCOUNT_COUNT_V1 + CORE_RETIREMENT_ESCROW_TAIL_ACCOUNTS_V1;
/// Exact nested Core retirement data width.
pub const MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1: usize = REQUEST_BYTES
    + RETIREMENT_BUNDLE_BYTES_V1
    + CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1
    + 2 * CUSTODY_REQUEST_BYTES_V1;
/// Exact top-level Registry account count for one aggregate retirement.
pub const MARKET_RETIREMENT_ACCOUNT_COUNT_V1: usize =
    REGISTRY_RETIREMENT_CONTINUATION_PREFIX_ACCOUNTS_V1 + CORE_RETIREMENT_ACCOUNT_COUNT_V1 + 1;
/// Exact Core prepare width for checkpointed retirement.
pub const CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1: usize =
    REQUEST_BYTES + RETIREMENT_BUNDLE_BYTES_V1 + CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1;
/// Exact direct Custody suffix width.
pub const CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1: usize =
    AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1 + CUSTODY_REQUEST_BYTES_V1;
/// Exact direct terminal suffix width.
pub const CHECKPOINT_RETIREMENT_FINISH_BYTES_V1: usize =
    AGGREGATE_RETIREMENT_SUFFIX_REQUEST_BYTES_V1 + REQUEST_BYTES + RETIREMENT_BUNDLE_BYTES_V1;

const RETIREMENT_CANDIDATE_DOMAIN_V1: &[u8] = b"dclutch/market-retirement-candidate/v1";
const RETIREMENT_ORDER_DOMAIN_V1: &[u8] = b"dclutch/market-retirement-order/v1";

/// Same-finalized accounts required to derive one complete aggregate retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketRetirementSnapshotV1 {
    /// Retiring Core Market.
    pub market: ObservedAccount,
    /// Permanent lifecycle RentCredit.
    pub rent_credit: ObservedAccount,
    /// Current activated release cache.
    pub activation_cache: ObservedAccount,
    /// Current Registry program.
    pub registry_program: ObservedAccount,
    /// Current Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current Claims program.
    pub claims_program: ObservedAccount,
    /// Current Claims ProgramData.
    pub claims_programdata: ObservedAccount,
    /// Current Resolution program.
    pub resolution_program: ObservedAccount,
    /// Current Resolution ProgramData.
    pub resolution_programdata: ObservedAccount,
    /// Current Custody program.
    pub custody_program: ObservedAccount,
    /// Current Custody ProgramData.
    pub custody_programdata: ObservedAccount,
    /// Infrastructure-selected Rent program.
    pub rent_program: ObservedAccount,
    /// Resolution-owned closure receipt.
    pub source_receipt: ObservedAccount,
    /// Claims-owned runtime-width aggregate.
    pub claims_aggregate: ObservedAccount,
    /// Custody replay cursor.
    pub custody_replay: ObservedAccount,
    /// Empty HoardPrincipal vault.
    pub hoard_vault: ObservedAccount,
    /// Custody token authority PDA.
    pub custody_authority: ObservedAccount,
    /// Realm-selected collateral Mint.
    pub collateral_mint: ObservedAccount,
    /// Realm-selected collateral token program.
    pub collateral_token_program: ObservedAccount,
    /// Finalized Realm raw record.
    pub realm_raw: ObservedAccount,
    /// Vacant Realm staging cursor.
    pub realm_staging: ObservedAccount,
    /// Immutable Core infrastructure profile.
    pub infrastructure_profile: ObservedAccount,
    /// Finalized Registry ArtifactRelease record.
    pub registry_artifact_raw: ObservedAccount,
    /// Vacant Registry ArtifactRelease staging cursor.
    pub registry_artifact_staging: ObservedAccount,
    /// Current Registry ProgramData.
    pub registry_programdata: ObservedAccount,
    /// Finalized Rent ArtifactRelease record.
    pub rent_artifact_raw: ObservedAccount,
    /// Vacant Rent ArtifactRelease staging cursor.
    pub rent_artifact_staging: ObservedAccount,
    /// Current Rent ProgramData.
    pub rent_programdata: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Immutable lifecycle refund wallet.
    pub refund_wallet: ObservedAccount,
    /// A refunding Market's failure-escrow Position, when it has one.
    ///
    /// Decision 0025 seats a refunding complete set's failure coordinate in a
    /// Position derived from the Market and held by nobody, and no certificate
    /// pays it -- so a Market carrying one cannot retire until the closure
    /// burns it. These three are what the burn needs in frame, and the caller
    /// finds them by deriving the escrow off the aggregate
    /// (`dclutch_claims::protocol_position_v2::failure_escrow_v1`) and reading
    /// the Market's linked basis record.
    ///
    /// `None` on all three is a categorical Market, or a refunding one whose
    /// column was never seated, and the retirement built from it is the exact
    /// thirty-five-account one that shipped. All three or none: half a tail is
    /// a shape neither program accepts.
    pub failure_escrow_position: Option<ObservedAccount>,
    /// That Position's protocol-Position admission record.
    pub failure_escrow_admission: Option<ObservedAccount>,
    /// The Market's linked `ProductBasisV3` record.
    pub linked_basis_record: Option<ObservedAccount>,
}

/// What a snapshot's escrow tail resolves to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureEscrowStateV1 {
    /// No escrow tail: the retirement is the categorical one.
    Vacant,
    /// A live Claims-owned escrow holding this quantity at the failure
    /// coordinate, with the record and the two accounts to discharge it.
    Seated {
        /// Failure-coordinate units the closure will burn.
        residue: u64,
        /// Rent the escrow pair surrenders to the aggregate at closure.
        rent: u64,
    },
}

/// Exact unsigned retirement transaction constructed solely from finalized state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketRetirementReportV1 {
    /// Top-level Registry continuation instruction.
    pub instruction: Instruction,
    /// Nested Core instruction before the Registry admission is appended.
    pub direct_instruction: Instruction,
    /// Shared finalized observation.
    pub observation: Observation,
    /// Invocation-scoped Registry continuation admission.
    pub registry_admission: Pubkey,
    /// Core-derived Claims close authority.
    pub claims_authority: Pubkey,
    /// Core-derived Custody CloseVault authority.
    pub close_vault_authority: Pubkey,
    /// Core-derived Custody CloseReplay authority.
    pub close_replay_authority: Pubkey,
    /// Core-derived RentV2 close authority.
    pub rent_close_authority: Pubkey,
    /// Exact Resolution-owned Source and subset-ledger closure facts.
    pub resolution_facts: ResolutionRetirementReceiptFactsV3,
    /// Exact post-Resolution lamports credited through Claims, Custody, Core, and RentV2.
    pub expected_refund_delta: u64,
    /// Runtime Claims width; never assumed equal to a compile-time `N`.
    pub claim_count: u32,
    /// Whether this Market's failure column sits in its derived escrow.
    ///
    /// TRUE means this instruction cannot be submitted. The one-shot route's
    /// Core frame is fixed at thirty-five accounts, so it carries no escrow
    /// tail and the Claims closure inside it reaches the supply loop with the
    /// column still standing -- `ClaimsMarketClosureSbfErrorV1::Liability`,
    /// `0x5503`. Decision 0025's shape A discharges the column in the
    /// checkpointed route, and `build_checkpoint_market_retirement_v1` is where
    /// a refunding Market retires.
    pub failure_escrow_seated: bool,
    /// Exact Registry continuation header.
    pub continuation: RegistryContinuationRequestV1,
}

/// Four packet-bounded instructions for one crash-resumable retirement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointMarketRetirementReportV1 {
    /// Direct Core Claims handoff and first checkpoint commit.
    pub prepare: Instruction,
    /// Direct Core HoardPrincipal close and second checkpoint commit.
    pub close_vault: Instruction,
    /// Direct Core Custody replay close and third checkpoint commit.
    pub close_replay: Instruction,
    /// Direct Core checkpoint/Core/Rent terminal closure.
    pub finish: Instruction,
    /// Same finalized observation used for all four packets.
    pub observation: Observation,
    /// Exact terminal refund wallet delta.
    pub expected_refund_delta: u64,
    /// Failure-coordinate units the `prepare` packet's closure burns.
    ///
    /// Zero on a categorical Market and on a refunding one whose column was
    /// never seated, and those two retirements carry the exact thirty-five
    /// accounts that shipped. Nonzero means the four packets carry the escrow
    /// tail and the first one discharges a column no certificate pays
    /// (decision 0025's shape A).
    pub burned_failure_units: u64,
    /// Rent the escrow pair surrenders to the checkpoint at closure.
    pub failure_escrow_rent_lamports: u64,
}

/// Stable refusal from chain observation, semantic join, or instruction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarketRetirementOperatorErrorV1 {
    /// Observations were not one finalized snapshot.
    Observation,
    /// A Registry cache or Loader deployment differed.
    Release,
    /// Core Market or RentV2 lifecycle facts differed.
    Market,
    /// Resolution closure facts differed.
    Resolution,
    /// Claims aggregate identity, width, revision, or liability differed.
    Claims,
    /// The escrow tail's own accusations, split out of `Claims` on 2026-09-06.
    ///
    /// `Claims` covered eleven conjuncts across two functions, and cohort-17's
    /// market 2 met it at the last stage of the first retirement any market has
    /// reached with a seated failure column -- with nothing to say which. These
    /// are five different accusations about four different accounts, so they
    /// are five codes.
    ///
    /// A supply standing at a coordinate the closure will not discharge: no
    /// escrow at all, or a non-failure column that is not zero.
    UnescrowedSupply,
    /// The refunding failure index, or the derived escrow's disagreement with
    /// it.
    FailureIndex,
    /// The linked LiabilityBasis record did not decode, or its semantic
    /// identity is not the aggregate's.
    BasisRecord,
    /// The basis record decoded and is the aggregate's, but its width or its
    /// refund-on-failure contract is not this retirement's.
    BasisContract,
    /// The escrow Position or its admission is not the derived pair, or the
    /// Position does not answer to the aggregate.
    EscrowFrame,
    /// The failure column is not exactly the escrow's residue: an unpayable
    /// column partly in hands that can be paid is an outstanding liability.
    EscrowResidue,
    /// Custody replay, vault, Realm, token, or conservation facts differed.
    Custody,
    /// A deterministic address, account alias, or privilege profile differed.
    Frame,
    /// Fixed-width request or digest construction refused.
    Encoding,
    /// Checked lamport or revision arithmetic overflowed.
    Arithmetic,
    /// `dclutch_market` refused; the cause is its own.
    MarketCore(dclutch_market::Error),
    /// `dclutch_claims` refused; the cause is its own.
    ClaimsMarketClosure(dclutch_claims::market_closure_v1::ClaimsMarketClosureErrorV1),
    /// `dclutch_custody` refused; the cause is its own.
    CustodyContract(dclutch_custody::Error),
    /// `dclutch_market::rent` refused; the cause is its own.
    LifecycleRent(dclutch_market::rent::lifecycle_v2::LifecycleRentErrorV2),
    /// `dclutch_market` refused; the cause is its own.
    Retirement(dclutch_market::RetirementErrorV1),
    /// `dclutch_market` refused; the cause is its own.
    AggregateRetirementCheckpoint(dclutch_market::AggregateRetirementCheckpointErrorV1),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
    /// `dclutch_registry::svm` refused; the cause is its own.
    RegistrySvm(dclutch_registry::svm::Error),
    /// `dclutch_source::resolution` refused; the cause is its own.
    ResolutionCodec(dclutch_source::resolution::Error),
    /// `dclutch_resolution_core_v3_operator` refused; the cause is its own.
    ResolutionCoreOperator(dclutch_resolution_core_v3_operator::ResolutionCoreOperatorErrorV3),
    /// `dclutch_claims` refused; the cause is its own.
    LiabilityBasisState(dclutch_claims::liability_basis_state_v2::LiabilityBasisStateErrorV2),
    /// `dclutch_market::realm` refused; the cause is its own.
    Realm(dclutch_market::realm::Error),
    /// `dclutch_core_contract` refused; the cause is its own.
    Core(dclutch_core_contract::Error),
    /// `dclutch_registry::svm` refused; the cause is its own.
    Batch(dclutch_registry::svm::batch_v2::BatchErrorV2),
}

#[derive(Clone, Copy)]
struct AuthenticatedRetirementV1 {
    observation: Observation,
    market: CoreState,
    source: ResolutionRetirementReceiptFactsV3,
    claims: LiabilityBasisMarketViewV2,
    replay: CustodyReplayV1,
    escrow: FailureEscrowStateV1,
}

/// Construct the sole canonical aggregate-retirement transaction from one
/// finalized chain snapshot. The caller contributes no release, revision,
/// width, custody, refund, or child-receipt truth.
pub fn build_market_retirement_v1(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<MarketRetirementReportV1, MarketRetirementOperatorErrorV1> {
    let authenticated = authenticate_snapshot(snapshot)?;
    let market = authenticated.market;
    let release_set = market.identity.selected_release_set.to_bytes();
    let market_key = snapshot.market.key;
    let core_request = Request::administrative(
        Action::Retire,
        market.identity.generation,
        market.identity.market_id,
    );
    let core_bytes = core_request
        .encode()
        .map_err(MarketRetirementOperatorErrorV1::MarketCore)?;
    let parent_digest = hash(&core_bytes).to_bytes();

    let claims_request = ClaimsMarketClosureRequestV1::new(ClaimsMarketClosureRequestInputV1 {
        release_set,
        market: market_key.to_bytes(),
        aggregate: snapshot.claims_aggregate.key.to_bytes(),
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        parent_request_digest: parent_digest,
        core_program: snapshot.core_program.key.to_bytes(),
        generation: market.identity.generation,
        expected_revision: authenticated.claims.revision,
        resulting_revision: authenticated
            .claims
            .revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        claim_count: authenticated.claims.claim_count,
    })
    .map_err(MarketRetirementOperatorErrorV1::ClaimsMarketClosure)?;
    let claims_bytes = claims_request.to_bytes();

    let candidate = hashv(&[
        RETIREMENT_CANDIDATE_DOMAIN_V1,
        market_key.as_ref(),
        &market.identity.generation.to_le_bytes(),
        &parent_digest,
    ])
    .to_bytes();
    let order = hashv(&[
        RETIREMENT_ORDER_DOMAIN_V1,
        market_key.as_ref(),
        &authenticated.replay.next_revision.to_le_bytes(),
        &parent_digest,
    ])
    .to_bytes();
    let close_vault = custody_request(
        snapshot,
        authenticated,
        OperationV1::CloseVault,
        parent_digest,
        candidate,
        order,
        authenticated.replay.next_revision,
        0,
    )?;
    authenticate_custody_authority(snapshot, close_vault)?;
    let close_vault_bytes = close_vault
        .to_bytes()
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let close_replay = custody_request(
        snapshot,
        authenticated,
        OperationV1::CloseReplay,
        parent_digest,
        candidate,
        order,
        authenticated
            .replay
            .next_revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        1,
    )?;
    let close_replay_bytes = close_replay
        .to_bytes()
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;

    let claims_request_digest = hash(&claims_bytes).to_bytes();
    let claims_receipt = projected_claims_receipt(snapshot, authenticated, claims_request_digest)?;
    let claims_receipt_digest = hash(&claims_receipt.to_bytes()).to_bytes();
    let (close_vault_receipt_digest, close_replay_receipt_digest) = projected_custody_receipts(
        snapshot,
        authenticated,
        close_vault,
        &close_vault_bytes,
        close_replay,
        &close_replay_bytes,
    )?;

    let source_digest = hash(&snapshot.source_receipt.data).to_bytes();
    let custody_refund = snapshot
        .hoard_vault
        .lamports
        .checked_add(snapshot.custody_replay.lamports)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let expected_refund_delta = snapshot
        .rent_credit
        .lamports
        .checked_add(snapshot.claims_aggregate.lamports)
        .and_then(|value| value.checked_add(custody_refund))
        .and_then(|value| value.checked_add(snapshot.market.lamports))
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let post_resource_digest = hashv(&[
        &RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &[RETIREMENT_ROLE_COUNT_V1],
        &[RETIREMENT_CUSTODY_RECEIPT_COUNT_V1],
        snapshot.rent_credit.key.as_ref(),
        &source_digest,
        &claims_receipt_digest,
        &close_vault_receipt_digest,
        &close_replay_receipt_digest,
        &snapshot.market.lamports.to_le_bytes(),
        &snapshot.claims_aggregate.lamports.to_le_bytes(),
        &custody_refund.to_le_bytes(),
        &expected_refund_delta.to_le_bytes(),
    ])
    .to_bytes();
    let rent_close_seeds = LifecycleRentCoreCloseAuthoritySeedsV2::new(
        LifecycleAccountIdV2::new(snapshot.rent_credit.key.to_bytes())
            .map_err(MarketRetirementOperatorErrorV1::LifecycleRent)?,
        post_resource_digest,
    )
    .map_err(MarketRetirementOperatorErrorV1::LifecycleRent)?;
    let rent_credit_seed = rent_close_seeds.credit().to_bytes();
    let rent_close_digest = rent_close_seeds.post_resource_digest();
    let rent_close_authority = Pubkey::find_program_address(
        &[
            rent_close_seeds.domain(),
            &rent_credit_seed,
            &rent_close_digest,
        ],
        &snapshot.core_program.key,
    )
    .0;

    let bundle = RetirementBundleV1::new(RetirementBundleInputV1 {
        market: market_key.to_bytes(),
        release_set,
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        source_receipt_account: snapshot.source_receipt.key.to_bytes(),
        claims_aggregate: snapshot.claims_aggregate.key.to_bytes(),
        custody_replay: snapshot.custody_replay.key.to_bytes(),
        hoard_vault: snapshot.hoard_vault.key.to_bytes(),
        source_receipt_digest: source_digest,
        claims_request_digest,
        custody_close_vault_request_digest: hash(&close_vault_bytes).to_bytes(),
        custody_close_replay_request_digest: hash(&close_replay_bytes).to_bytes(),
        core_prestate_digest: hash(&snapshot.market.data).to_bytes(),
        generation: market.identity.generation,
        source_closure_revision: authenticated
            .source
            .terminal_sequence
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        claims_pre_revision: authenticated.claims.revision,
        claims_post_revision: authenticated
            .claims
            .revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        custody_pre_revision: authenticated.replay.next_revision,
        custody_middle_revision: authenticated
            .replay
            .next_revision
            .checked_add(1)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        custody_post_revision: authenticated
            .replay
            .next_revision
            .checked_add(2)
            .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?,
        expected_core_lamports: snapshot.market.lamports,
    })
    .map_err(MarketRetirementOperatorErrorV1::Retirement)?;

    let claims_authority = caller_authority(
        release_set,
        market_key,
        parent_digest,
        &claims_bytes,
        snapshot.core_program.key,
    )?;
    let close_vault_authority = caller_authority(
        release_set,
        market_key,
        authenticated.replay.context,
        &close_vault_bytes,
        snapshot.core_program.key,
    )?;
    let close_replay_authority = caller_authority(
        release_set,
        market_key,
        authenticated.replay.context,
        &close_replay_bytes,
        snapshot.core_program.key,
    )?;

    let mut data = Vec::with_capacity(MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1);
    data.extend_from_slice(&core_bytes);
    data.extend_from_slice(&bundle.to_bytes());
    data.extend_from_slice(&claims_bytes);
    data.extend_from_slice(&close_vault_bytes);
    data.extend_from_slice(&close_replay_bytes);
    if data.len() != MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1 {
        return Err(MarketRetirementOperatorErrorV1::Encoding);
    }
    let direct_instruction = Instruction {
        program_id: snapshot.core_program.key,
        accounts: core_accounts(
            snapshot,
            claims_authority,
            close_vault_authority,
            close_replay_authority,
            rent_close_authority,
            // The one-shot route's frame is fixed at thirty-five in Core's own
            // `parse`, so it carries no escrow tail and cannot discharge a
            // seated failure column. `failure_escrow_seated` on the report is
            // how a caller is told that before submitting.
            FailureEscrowStateV1::Vacant,
        ),
        data,
    };
    if direct_instruction.accounts.len() != CORE_RETIREMENT_ACCOUNT_COUNT_V1 {
        return Err(MarketRetirementOperatorErrorV1::Frame);
    }
    let (instruction, registry_admission, continuation) =
        wrap_registry_continuation(snapshot, &direct_instruction)?;
    Ok(MarketRetirementReportV1 {
        instruction,
        direct_instruction,
        observation: authenticated.observation,
        registry_admission,
        claims_authority,
        close_vault_authority,
        close_replay_authority,
        rent_close_authority,
        resolution_facts: authenticated.source,
        expected_refund_delta,
        claim_count: authenticated.claims.claim_count,
        failure_escrow_seated: authenticated.escrow.seated(),
        continuation,
    })
}

/// Construct the four packet-bounded retirement instructions from the same
/// finalized snapshot used by the legacy aggregate builder.
pub fn build_checkpoint_market_retirement_v1(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<CheckpointMarketRetirementReportV1, MarketRetirementOperatorErrorV1> {
    let legacy = build_market_retirement_v1(snapshot)?;
    let authenticated = authenticate_snapshot(snapshot)?;
    let core_bytes: [u8; REQUEST_BYTES] = legacy
        .direct_instruction
        .data
        .get(..REQUEST_BYTES)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(MarketRetirementOperatorErrorV1::Encoding)?;
    let bundle_start = REQUEST_BYTES;
    let claims_start = bundle_start + RETIREMENT_BUNDLE_BYTES_V1;
    let vault_start = claims_start + CLAIMS_MARKET_CLOSURE_REQUEST_BYTES_V1;
    let replay_start = vault_start + CUSTODY_REQUEST_BYTES_V1;
    let old_bundle = RetirementBundleV1::decode(
        legacy
            .direct_instruction
            .data
            .get(bundle_start..claims_start)
            .ok_or(MarketRetirementOperatorErrorV1::Encoding)?,
    )
    .map_err(MarketRetirementOperatorErrorV1::Retirement)?;
    let old_claims = ClaimsMarketClosureRequestV1::decode(
        legacy
            .direct_instruction
            .data
            .get(claims_start..vault_start)
            .ok_or(MarketRetirementOperatorErrorV1::Encoding)?,
    )
    .map_err(MarketRetirementOperatorErrorV1::ClaimsMarketClosure)?;
    let handoff = ClaimsRetirementCheckpointHandoffRequestV1::new(old_claims.input())
        .map_err(MarketRetirementOperatorErrorV1::ClaimsMarketClosure)?;
    let handoff_bytes = handoff.to_bytes();
    let close_vault_bytes = legacy
        .direct_instruction
        .data
        .get(vault_start..replay_start)
        .ok_or(MarketRetirementOperatorErrorV1::Encoding)?;
    let close_replay_bytes = legacy
        .direct_instruction
        .data
        .get(replay_start..)
        .ok_or(MarketRetirementOperatorErrorV1::Encoding)?;
    let old = old_bundle.input();
    let bundle = RetirementBundleV1::new(RetirementBundleInputV1 {
        market: old.market,
        release_set: old.release_set,
        rent_credit: old.rent_credit,
        source_receipt_account: old.source_receipt_account,
        claims_aggregate: old.claims_aggregate,
        custody_replay: old.custody_replay,
        hoard_vault: old.hoard_vault,
        source_receipt_digest: old.source_receipt_digest,
        claims_request_digest: hash(&handoff_bytes).to_bytes(),
        custody_close_vault_request_digest: old.custody_close_vault_request_digest,
        custody_close_replay_request_digest: old.custody_close_replay_request_digest,
        core_prestate_digest: old.core_prestate_digest,
        generation: old.generation,
        source_closure_revision: old.source_closure_revision,
        claims_pre_revision: old.claims_pre_revision,
        claims_post_revision: old.claims_post_revision,
        custody_pre_revision: old.custody_pre_revision,
        custody_middle_revision: old.custody_middle_revision,
        custody_post_revision: old.custody_post_revision,
        expected_core_lamports: old.expected_core_lamports,
    })
    .map_err(MarketRetirementOperatorErrorV1::Retirement)?;
    let bundle_bytes = bundle.to_bytes();
    let bundle_digest = hash(&bundle_bytes).to_bytes();
    let source_digest = hash(&snapshot.source_receipt.data).to_bytes();
    let handoff_receipt =
        projected_claims_handoff_receipt(snapshot, authenticated, hash(&handoff_bytes).to_bytes())?;
    let handoff_receipt_digest = hash(&handoff_receipt.to_bytes()).to_bytes();
    let close_vault = CustodyRequestV1::decode(close_vault_bytes)
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let close_replay = CustodyRequestV1::decode(close_replay_bytes)
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let (vault_receipt_digest, replay_receipt_digest) = projected_custody_receipts(
        snapshot,
        authenticated,
        close_vault,
        close_vault_bytes,
        close_replay,
        close_replay_bytes,
    )?;
    let custody_refund = snapshot
        .hoard_vault
        .lamports
        .checked_add(snapshot.custody_replay.lamports)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    // Core hashes `checkpoint.claims_refund_lamports()` in this slot and it is
    // the aggregate's rent PLUS whatever the closure's escrow pair surrendered
    // to it, so the projection has to grow with it or the derived
    // `rent_close_authority` stops being the PDA Core will authenticate.
    let claims_refund = snapshot
        .claims_aggregate
        .lamports
        .checked_add(authenticated.escrow.rent())
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let expected_refund_delta = snapshot
        .rent_credit
        .lamports
        .checked_add(claims_refund)
        .and_then(|value| value.checked_add(custody_refund))
        .and_then(|value| value.checked_add(snapshot.market.lamports))
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let post_resource_digest = hashv(&[
        &RETIREMENT_POST_RESOURCE_DIGEST_DOMAIN_V1,
        &[RETIREMENT_ROLE_COUNT_V1],
        &[RETIREMENT_CUSTODY_RECEIPT_COUNT_V1],
        snapshot.rent_credit.key.as_ref(),
        &source_digest,
        &handoff_receipt_digest,
        &vault_receipt_digest,
        &replay_receipt_digest,
        &snapshot.market.lamports.to_le_bytes(),
        &claims_refund.to_le_bytes(),
        &custody_refund.to_le_bytes(),
        &expected_refund_delta.to_le_bytes(),
    ])
    .to_bytes();
    let rent_seeds = LifecycleRentCoreCloseAuthoritySeedsV2::new(
        LifecycleAccountIdV2::new(snapshot.rent_credit.key.to_bytes())
            .map_err(MarketRetirementOperatorErrorV1::LifecycleRent)?,
        post_resource_digest,
    )
    .map_err(MarketRetirementOperatorErrorV1::LifecycleRent)?;
    let credit = rent_seeds.credit().to_bytes();
    let post = rent_seeds.post_resource_digest();
    let rent_close_authority = Pubkey::find_program_address(
        &[rent_seeds.domain(), credit.as_slice(), post.as_slice()],
        &snapshot.core_program.key,
    )
    .0;
    let claims_authority = caller_authority(
        old.release_set,
        snapshot.market.key,
        hash(&core_bytes).to_bytes(),
        &handoff_bytes,
        snapshot.core_program.key,
    )?;
    let close_vault_authority = caller_authority(
        old.release_set,
        snapshot.market.key,
        authenticated.replay.context,
        close_vault_bytes,
        snapshot.core_program.key,
    )?;
    let close_replay_authority = caller_authority(
        old.release_set,
        snapshot.market.key,
        authenticated.replay.context,
        close_replay_bytes,
        snapshot.core_program.key,
    )?;
    let accounts = core_accounts(
        snapshot,
        claims_authority,
        close_vault_authority,
        close_replay_authority,
        rent_close_authority,
        authenticated.escrow,
    );
    let mut prepare_data = Vec::with_capacity(CHECKPOINT_RETIREMENT_PREPARE_CORE_BYTES_V1);
    prepare_data.extend_from_slice(&core_bytes);
    prepare_data.extend_from_slice(&bundle_bytes);
    prepare_data.extend_from_slice(&handoff_bytes);
    let prepare = Instruction {
        program_id: snapshot.core_program.key,
        accounts: accounts.clone(),
        data: prepare_data,
    };
    // Which retirement of which market all three suffixes name, written once.
    // These four were four positional digests repeated per phase; the three
    // requests agreeing on them is the whole point of the phase chain.
    let binding = AggregateRetirementSuffixBindingV1 {
        market: snapshot.market.key.to_bytes(),
        checkpoint: snapshot.claims_aggregate.key.to_bytes(),
        bundle_digest,
        source_receipt_digest: source_digest,
    };
    let vault_suffix = AggregateRetirementSuffixRequestV1::new(
        AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1,
        binding,
        hash(close_vault_bytes).to_bytes(),
        1,
        old.custody_pre_revision,
    )
    .map_err(MarketRetirementOperatorErrorV1::AggregateRetirementCheckpoint)?;
    let replay_suffix = AggregateRetirementSuffixRequestV1::new(
        AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1,
        binding,
        hash(close_replay_bytes).to_bytes(),
        2,
        old.custody_middle_revision,
    )
    .map_err(MarketRetirementOperatorErrorV1::AggregateRetirementCheckpoint)?;
    let finish_suffix = AggregateRetirementSuffixRequestV1::new(
        AGGREGATE_RETIREMENT_FINISH_MAGIC_V1,
        binding,
        [0; 32],
        3,
        old.custody_post_revision,
    )
    .map_err(MarketRetirementOperatorErrorV1::AggregateRetirementCheckpoint)?;
    let direct = |data: Vec<u8>| Instruction {
        program_id: snapshot.core_program.key,
        accounts: accounts.clone(),
        data,
    };
    let mut vault_data = Vec::with_capacity(CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1);
    vault_data.extend_from_slice(&vault_suffix.to_bytes());
    vault_data.extend_from_slice(close_vault_bytes);
    let mut replay_data = Vec::with_capacity(CHECKPOINT_RETIREMENT_CUSTODY_SUFFIX_BYTES_V1);
    replay_data.extend_from_slice(&replay_suffix.to_bytes());
    replay_data.extend_from_slice(close_replay_bytes);
    let mut finish_data = Vec::with_capacity(CHECKPOINT_RETIREMENT_FINISH_BYTES_V1);
    finish_data.extend_from_slice(&finish_suffix.to_bytes());
    finish_data.extend_from_slice(&core_bytes);
    finish_data.extend_from_slice(&bundle_bytes);
    Ok(CheckpointMarketRetirementReportV1 {
        prepare,
        close_vault: direct(vault_data),
        close_replay: direct(replay_data),
        finish: direct(finish_data),
        observation: authenticated.observation,
        expected_refund_delta,
        burned_failure_units: authenticated.escrow.residue(),
        failure_escrow_rent_lamports: authenticated.escrow.rent(),
    })
}

fn authenticate_snapshot(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<AuthenticatedRetirementV1, MarketRetirementOperatorErrorV1> {
    let accounts = snapshot_accounts(snapshot);
    let observation = accounts
        .first()
        .map(|account| account.observation)
        .ok_or(MarketRetirementOperatorErrorV1::Observation)?;
    if accounts
        .iter()
        .any(|account| account.observation.finality != Finality::Finalized)
        || accounts
            .iter()
            .any(|account| account.observation != observation)
    {
        return Err(MarketRetirementOperatorErrorV1::Observation);
    }
    for (left_index, left) in accounts.iter().enumerate() {
        for right in accounts.iter().skip(left_index.saturating_add(1)) {
            if left.key == right.key {
                return Err(MarketRetirementOperatorErrorV1::Frame);
            }
        }
    }

    let market = CoreState::decode(&snapshot.market.data)
        .map_err(MarketRetirementOperatorErrorV1::MarketCore)?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &snapshot.core_program.key,
    )
    .0;
    if snapshot.market.owner != snapshot.core_program.key
        || snapshot.market.executable
        || snapshot.market.data.len() != STATE_BYTES
        || snapshot.market.lamports == 0
        || snapshot.market.key != expected_market
        || market.identity.market_id.to_bytes() != snapshot.market.key.to_bytes()
        || market.identity.registry_program.to_bytes() != snapshot.registry_program.key.to_bytes()
        || market.phase != Phase::Retiring
        || market.outstanding_capabilities != 0
        || market.rent_beneficiary.to_bytes() != snapshot.rent_credit.key.to_bytes()
    {
        return Err(MarketRetirementOperatorErrorV1::Market);
    }

    authenticate_release_set(snapshot, market)?;
    authenticate_infrastructure(snapshot)?;
    authenticate_rent(snapshot, market)?;
    let source = authenticate_resolution(snapshot, market)?;
    let claims = authenticate_claims(snapshot, market)?;
    let escrow = authenticate_failure_escrow(snapshot, claims)?;
    let (replay, _) = authenticate_custody(snapshot, market, source)?;
    Ok(AuthenticatedRetirementV1 {
        observation,
        market,
        source,
        claims,
        replay,
        escrow,
    })
}

fn snapshot_accounts(snapshot: &MarketRetirementSnapshotV1) -> Vec<&ObservedAccount> {
    let mut accounts = vec![
        &snapshot.market,
        &snapshot.rent_credit,
        &snapshot.activation_cache,
        &snapshot.registry_program,
        &snapshot.core_program,
        &snapshot.core_programdata,
        &snapshot.claims_program,
        &snapshot.claims_programdata,
        &snapshot.resolution_program,
        &snapshot.resolution_programdata,
        &snapshot.custody_program,
        &snapshot.custody_programdata,
        &snapshot.rent_program,
        &snapshot.source_receipt,
        &snapshot.claims_aggregate,
        &snapshot.custody_replay,
        &snapshot.hoard_vault,
        &snapshot.custody_authority,
        &snapshot.collateral_mint,
        &snapshot.collateral_token_program,
        &snapshot.realm_raw,
        &snapshot.realm_staging,
        &snapshot.infrastructure_profile,
        &snapshot.registry_artifact_raw,
        &snapshot.registry_artifact_staging,
        &snapshot.registry_programdata,
        &snapshot.rent_artifact_raw,
        &snapshot.rent_artifact_staging,
        &snapshot.rent_programdata,
        &snapshot.rent_sysvar,
        &snapshot.refund_wallet,
    ];
    accounts.extend(
        [
            snapshot.failure_escrow_position.as_ref(),
            snapshot.failure_escrow_admission.as_ref(),
            snapshot.linked_basis_record.as_ref(),
        ]
        .into_iter()
        .flatten(),
    );
    accounts
}

fn authenticate_release_set(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    if snapshot.registry_program.owner != bpf_loader_upgradeable::ID
        || !snapshot.registry_program.executable
        || ProgramV3View::parse(&snapshot.registry_program.data).is_err()
        || snapshot.activation_cache.owner != snapshot.registry_program.key
        || snapshot.activation_cache.executable
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&snapshot.activation_cache.data)
        .map_err(MarketRetirementOperatorErrorV1::Registry)?;
    let release_set = activated
        .execution_release_set_id()
        .map_err(MarketRetirementOperatorErrorV1::Registry)?;
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_bytes()],
        &snapshot.registry_program.key,
    )
    .0;
    if release_set.to_bytes() != market.identity.selected_release_set.to_bytes()
        || expected_cache != snapshot.activation_cache.key
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    for (role, program, programdata) in [
        (
            ExecutionRoleV1::Core,
            &snapshot.core_program,
            &snapshot.core_programdata,
        ),
        (
            ExecutionRoleV1::Claims,
            &snapshot.claims_program,
            &snapshot.claims_programdata,
        ),
        (
            ExecutionRoleV1::Resolution,
            &snapshot.resolution_program,
            &snapshot.resolution_programdata,
        ),
        (
            ExecutionRoleV1::Custody,
            &snapshot.custody_program,
            &snapshot.custody_programdata,
        ),
    ] {
        authenticate_current_role(activated, role, program, programdata)?;
    }
    Ok(())
}

fn authenticate_current_role(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
    role: ExecutionRoleV1,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let selected = activated
        .role(role)
        .map_err(MarketRetirementOperatorErrorV1::Registry)?;
    let release = selected.release();
    if release.program().to_bytes() != program.key.to_bytes()
        || release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let deployment = deployment_observation(program, programdata)?;
    selected
        .authenticate_current_deployment(deployment)
        .map_err(MarketRetirementOperatorErrorV1::Registry)
}

fn authenticate_infrastructure(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let expected_profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &snapshot.core_program.key,
    )
    .0;
    if snapshot.infrastructure_profile.key != expected_profile
        || snapshot.infrastructure_profile.owner != snapshot.core_program.key
        || snapshot.infrastructure_profile.executable
        || snapshot.infrastructure_profile.lamports == 0
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let profile = ProtocolInfrastructureProfileV2::decode(&snapshot.infrastructure_profile.data)
        .map_err(MarketRetirementOperatorErrorV1::ReleaseSet)?;
    authenticate_infrastructure_artifact(
        snapshot,
        profile.registry(),
        &snapshot.registry_artifact_raw,
        &snapshot.registry_artifact_staging,
        &snapshot.registry_program,
        &snapshot.registry_programdata,
    )?;
    authenticate_infrastructure_artifact(
        snapshot,
        profile.rent(),
        &snapshot.rent_artifact_raw,
        &snapshot.rent_artifact_staging,
        &snapshot.rent_program,
        &snapshot.rent_programdata,
    )?;
    if snapshot.rent_sysvar.key != sysvar::rent::ID
        || snapshot.rent_sysvar.owner != sysvar::ID
        || snapshot.rent_sysvar.executable
        || snapshot.rent_sysvar.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    Ok(())
}

fn authenticate_infrastructure_artifact(
    snapshot: &MarketRetirementSnapshotV1,
    selected: ExecutionRoleBindingV1,
    raw: &ObservedAccount,
    staging: &ObservedAccount,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let digest = hash(&raw.data).to_bytes();
    let expected_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    if selected.program().to_bytes() != program.key.to_bytes()
        || selected.artifact_release().to_bytes() != digest
        || raw.key != expected_raw
        || raw.owner != snapshot.registry_program.key
        || raw.executable
        || raw.lamports == 0
        || staging.key != expected_staging
        || staging.owner != system_program::ID
        || staging.executable
        || !staging.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let release =
        ArtifactReleaseV1::decode(&raw.data).map_err(MarketRetirementOperatorErrorV1::Registry)?;
    if release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    require_slot_pinned_release_v1(release).map_err(MarketRetirementOperatorErrorV1::Registry)?;
    release
        .authenticate_deployment(deployment_observation(program, programdata)?)
        .map_err(MarketRetirementOperatorErrorV1::Registry)
}

fn deployment_observation(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
) -> Result<DeploymentObservationV1, MarketRetirementOperatorErrorV1> {
    let program_view = ProgramV3View::parse(&program.data)
        .map_err(MarketRetirementOperatorErrorV1::RegistrySvm)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != programdata.key.to_bytes()
        || programdata.key != expected_programdata
    {
        return Err(MarketRetirementOperatorErrorV1::Release);
    }
    let programdata_view = ProgramDataV3View::parse(&programdata.data)
        .map_err(MarketRetirementOperatorErrorV1::RegistrySvm)?;
    DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(MarketRetirementOperatorErrorV1::Registry)
}

fn authenticate_rent(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<LifecycleRentCreditV2, MarketRetirementOperatorErrorV1> {
    let credit = LifecycleRentCreditV2::decode(&snapshot.rent_credit.data)
        .map_err(MarketRetirementOperatorErrorV1::LifecycleRent)?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let generation = seeds.generation();
    let market_seed = seeds.market().to_bytes();
    let expected = Pubkey::create_program_address(
        &[seeds.domain(), &market_seed, &generation, &bump],
        &snapshot.rent_program.key,
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Market)?;
    if snapshot.rent_credit.owner != snapshot.rent_program.key
        || snapshot.rent_credit.executable
        || snapshot.rent_credit.lamports == 0
        || snapshot.rent_credit.key != expected
        || credit.market().to_bytes() != snapshot.market.key.to_bytes()
        || credit.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || credit.generation() != market.identity.generation
        || credit.refund_wallet().to_bytes() != snapshot.refund_wallet.key.to_bytes()
        || snapshot.refund_wallet.owner != system_program::ID
        || snapshot.refund_wallet.executable
        || !snapshot.refund_wallet.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Market);
    }
    Ok(credit)
}

fn authenticate_resolution(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<ResolutionRetirementReceiptFactsV3, MarketRetirementOperatorErrorV1> {
    let source = SourceClosureReceiptV3::decode(&snapshot.source_receipt.data)
        .map_err(MarketRetirementOperatorErrorV1::ResolutionCodec)?;
    let classified_total = source
        .source_refund_lamports
        .checked_add(source.ledger_remaining_native_principal)
        .and_then(|value| value.checked_add(source.ledger_rent_lamports))
        .and_then(|value| value.checked_add(source.ledger_lamport_surplus))
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    if classified_total != source.refund_lamports
        || source.receipt_account != snapshot.source_receipt.key.to_bytes()
        || source.market != snapshot.market.key.to_bytes()
        || source.generation != market.identity.generation
        || source.source_material != market.identity.resolution_policy.to_bytes()
        || source.capability_manifest != market.identity.capability_manifest.to_bytes()
        || source.beneficiary != snapshot.rent_credit.key.to_bytes()
        || source.selector != market.terminal_winner
        || market.terminal_receipt.map(|value| value.to_bytes())
            != Some(source.terminal_certificate)
    {
        return Err(MarketRetirementOperatorErrorV1::Resolution);
    }
    let expected = ResolutionRetirementReceiptFactsV3 {
        market: source.market,
        generation: source.generation,
        resolution_closure_receipt: source.receipt_account,
        source_state: source.source_state,
        source_material: source.source_material,
        capability_manifest: source.capability_manifest,
        terminal_certificate: source.terminal_certificate,
        beneficiary: source.beneficiary,
        selector: source.selector,
        terminal_sequence: source.terminal_sequence,
        source_state_digest: source.source_state_digest,
        terminal_certificate_digest: source.terminal_certificate_digest,
        funding_set_digest: source.funding_set_digest,
        source_refund_lamports: source.source_refund_lamports,
        ledger_remaining_native_principal: source.ledger_remaining_native_principal,
        ledger_rent_lamports: source.ledger_rent_lamports,
        ledger_lamport_surplus: source.ledger_lamport_surplus,
        refund_lamports: source.refund_lamports,
        closed_at: source.closed_at,
    };
    authenticate_resolution_retirement_receipt_v3(
        &snapshot.source_receipt,
        &snapshot.rent_sysvar,
        snapshot.resolution_program.key,
        expected,
    )
    .map_err(MarketRetirementOperatorErrorV1::ResolutionCoreOperator)
}

fn authenticate_claims(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
) -> Result<LiabilityBasisMarketViewV2, MarketRetirementOperatorErrorV1> {
    let claims = LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
        .map_err(MarketRetirementOperatorErrorV1::LiabilityBasisState)?;
    let expected = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, snapshot.market.key.as_ref()],
        &snapshot.claims_program.key,
    )
    .0;
    if snapshot.claims_aggregate.owner != snapshot.claims_program.key
        || snapshot.claims_aggregate.executable
        || snapshot.claims_aggregate.lamports == 0
        || snapshot.claims_aggregate.key != expected
        || claims.claim_count < 2
        || claims.logical_market != snapshot.market.key.to_bytes()
        || claims.release_set != market.identity.selected_release_set.to_bytes()
        || claims.registry_program != snapshot.registry_program.key.to_bytes()
        // THE PRODUCT INSTANCE, NOT THE PRODUCT RECORD. The Claims program is
        // the semantic owner of this field and binds it to the Core state's
        // `product_id` -- `rational_product_v3.rs:201` and
        // `affine_batch_v2.rs:700` in the deployed link both compare
        // `core.identity.product_id.to_bytes() != market.product_instance_id`
        // -- and so do every other host reader of it
        // (`wallet_terminal_input.rs:488`, `rational_representation.rs:642`).
        // This reader alone read the RECORD, the Registry content identity of
        // the product's record, which is a different 32 bytes on every Market
        // that has ever been founded. Only a Market that actually reaches the
        // closure meets it, and until cohort-17's market 2 none ever had; the
        // fixture agreed because one hand wrote both it and this line.
        || claims.product_instance_id != market.identity.product_id.to_bytes()
        || claims.realm_id != market.identity.realm_id.to_bytes()
        || claims.generation != market.identity.generation
    {
        return Err(MarketRetirementOperatorErrorV1::Claims);
    }
    Ok(claims)
}

/// Resolve the snapshot's escrow tail and prove every aggregate supply the
/// closure will not discharge is already zero.
///
/// # Why the zero-supply sweep moved here
///
/// It used to be an unconditional "every coordinate is zero", and on a
/// refunding Market that is the WALL rather than a check: the failure column
/// is unpayable under every certificate and its holder has no key, so a
/// retirement built against it never gets built and the operator's reader is
/// sent looking for a payout to produce. Decision 0025's shape A discharges it
/// in the closure instead, so the rule is now "every ordinary coordinate is
/// zero, and the failure coordinate is zero UNLESS the escrow in frame holds
/// exactly it".
///
/// # What this does and does not authenticate
///
/// It authenticates the tail against the AGGREGATE: the pair is the canonical
/// protocol-Position pair under the Position's own recorded owner, the Position
/// joins this Market's aggregate at this width, it holds the failure column and
/// nothing else, and the linked basis record reproduces the aggregate's own
/// `basis_id` and says the Market refunds on failure.
///
/// It also RE-DERIVES the escrow's owner the way the chain does, off the
/// aggregate's own coordinates -- `failure_escrow_v1` takes the Claims program
/// from the aggregate's owner, the logical Market and the runtime width from
/// its header -- and requires the pair in frame to be that derivation's two
/// addresses under that owner. Until 2026-09-06 it took the Position's OWN
/// recorded owner as the seed and so authenticated the canonical-pair shape
/// without authenticating WHICH owner, which made "these are this Market's
/// escrow accounts" a property of the caller's derivation rather than of this
/// builder. The chain refuses the difference by name
/// (`ClaimsSbfError::FailureEscrow`, `0x5010`), so nothing unsafe was
/// constructible; what was missing is that the builder could not say it, and a
/// plan refused on chain costs a submission to learn what one read decides.
fn authenticate_failure_escrow(
    snapshot: &MarketRetirementSnapshotV1,
    claims: LiabilityBasisMarketViewV2,
) -> Result<FailureEscrowStateV1, MarketRetirementOperatorErrorV1> {
    let (position_account, admission_account, basis_account) = match (
        snapshot.failure_escrow_position.as_ref(),
        snapshot.failure_escrow_admission.as_ref(),
        snapshot.linked_basis_record.as_ref(),
    ) {
        (Some(position), Some(admission), Some(basis)) => (position, admission, basis),
        (None, None, None) => {
            for claim in 0..claims.claim_count {
                if claims
                    .supply(&snapshot.claims_aggregate.data, claim)
                    .map_err(MarketRetirementOperatorErrorV1::LiabilityBasisState)?
                    != 0
                {
                    return Err(MarketRetirementOperatorErrorV1::UnescrowedSupply);
                }
            }
            return Ok(FailureEscrowStateV1::Vacant);
        }
        // Half a tail is not a shape either program accepts, and refusing it
        // here keeps "the frame is thirty-eight accounts" a property of the
        // snapshot rather than of which of three reads happened to return.
        _ => return Err(MarketRetirementOperatorErrorV1::Frame),
    };
    let failure_selector = u32::try_from(
        refunding_failure_index(claims.claim_count)
            .map_err(|_| MarketRetirementOperatorErrorV1::FailureIndex)?,
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::FailureIndex)?;
    let basis = ProductBasisV3::decode(&basis_account.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::BasisRecord)?;
    let semantic = semantic_basis_id_v3(&basis_account.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::BasisRecord)?;
    if semantic != claims.basis_id {
        return Err(MarketRetirementOperatorErrorV1::BasisRecord);
    }
    if basis.basis_width() != claims.claim_count || !basis.refunds_on_failure() {
        return Err(MarketRetirementOperatorErrorV1::BasisContract);
    }
    let position = LiabilityBasisPositionViewV2::decode(&position_account.data)
        .map_err(MarketRetirementOperatorErrorV1::LiabilityBasisState)?;
    // The owner is DERIVED, not read off the Position. Every input is the
    // aggregate's own -- program, logical Market, runtime width -- so this
    // cannot be pointed at another Market's escrow, and it is the same
    // derivation `FailureEscrowIdentityV1::derive` makes inside Claims.
    let derived = failure_escrow_v1(
        snapshot.claims_program.key,
        claims.logical_market,
        snapshot.claims_aggregate.key,
        claims.claim_count,
    )
    .map_err(|_| MarketRetirementOperatorErrorV1::Frame)?;
    if derived.failure_selector != failure_selector {
        return Err(MarketRetirementOperatorErrorV1::FailureIndex);
    }
    if position_account.owner != snapshot.claims_program.key
        || admission_account.owner != snapshot.claims_program.key
        || position_account.key != derived.position
        || admission_account.key != derived.admission
        || position.owner != derived.owner.to_bytes()
        || admission_account.data.is_empty()
        || position.market_account != snapshot.claims_aggregate.key.to_bytes()
        || position.basis_id != claims.basis_id
        || position.claim_count != claims.claim_count
    {
        return Err(MarketRetirementOperatorErrorV1::EscrowFrame);
    }
    let mut residue = 0;
    for claim in 0..claims.claim_count {
        let supply = claims
            .supply(&snapshot.claims_aggregate.data, claim)
            .map_err(MarketRetirementOperatorErrorV1::LiabilityBasisState)?;
        let held = read_claim_v2(
            &position_account.data,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            claims.claim_count,
            claim,
        )
        .map_err(MarketRetirementOperatorErrorV1::LiabilityBasisState)?;
        if claim == failure_selector {
            // The residue rule as an EQUALITY against the escrow's own balance.
            // A Market whose failure column is only partly in the escrow has the
            // rest of it in hands that can be paid, and that is an outstanding
            // liability rather than a residue.
            if supply == 0 || supply != held {
                return Err(MarketRetirementOperatorErrorV1::EscrowResidue);
            }
            residue = supply;
        } else if supply != 0 || held != 0 {
            return Err(MarketRetirementOperatorErrorV1::UnescrowedSupply);
        }
    }
    let rent = position_account
        .lamports
        .checked_add(admission_account.lamports)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    Ok(FailureEscrowStateV1::Seated { residue, rent })
}

impl FailureEscrowStateV1 {
    /// What the escrow's own accounts surrender to the aggregate at closure.
    const fn rent(self) -> u64 {
        match self {
            Self::Vacant => 0,
            Self::Seated { rent, .. } => rent,
        }
    }

    /// Failure-coordinate units this retirement's closure burns.
    const fn residue(self) -> u64 {
        match self {
            Self::Vacant => 0,
            Self::Seated { residue, .. } => residue,
        }
    }

    /// Whether the retirement's frames carry the escrow tail.
    const fn seated(self) -> bool {
        matches!(self, Self::Seated { .. })
    }
}

fn authenticate_custody(
    snapshot: &MarketRetirementSnapshotV1,
    market: CoreState,
    source: ResolutionRetirementReceiptFactsV3,
) -> Result<(CustodyReplayV1, RealmV1), MarketRetirementOperatorErrorV1> {
    let replay = CustodyReplayV1::decode(&snapshot.custody_replay.data)
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let realm_digest = market.identity.realm_id.to_bytes();
    let expected_realm_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    let expected_realm_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm_digest,
        ],
        &snapshot.registry_program.key,
    )
    .0;
    let realm = RealmV1::decode(&snapshot.realm_raw.data)
        .map_err(MarketRetirementOperatorErrorV1::Realm)?;
    if snapshot.realm_raw.owner != snapshot.registry_program.key
        || snapshot.realm_raw.executable
        || snapshot.realm_raw.key != expected_realm_raw
        || hash(&snapshot.realm_raw.data).to_bytes() != realm_digest
        || snapshot.realm_staging.key != expected_realm_staging
        || snapshot.realm_staging.owner != system_program::ID
        || snapshot.realm_staging.executable
        || !snapshot.realm_staging.data.is_empty()
        || replay.caller_role != CallerRoleV1::Core
        || replay.release_set != market.identity.selected_release_set.to_bytes()
        || replay.market != snapshot.market.key.to_bytes()
        || replay.realm != realm_digest
        || replay.caller_program != snapshot.core_program.key.to_bytes()
        || replay.rent_refund != snapshot.rent_credit.key.to_bytes()
        || replay.open_vault_count != 1
        || replay.next_revision == 0
        || replay.generation != market.identity.generation
        || replay.context != claims_context(snapshot)?
        || snapshot.custody_replay.owner != snapshot.custody_program.key
        || snapshot.custody_replay.executable
        || snapshot.custody_replay.lamports == 0
        || realm.collateral_mint() != &snapshot.collateral_mint.key.to_bytes()
        || realm.token_program() != &snapshot.collateral_token_program.key.to_bytes()
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    let replay_seeds = CustodyReplaySeedsV1::from_request(custody_request(
        snapshot,
        AuthenticatedRetirementV1 {
            observation: snapshot.market.observation,
            market,
            source,
            claims: LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
                .map_err(MarketRetirementOperatorErrorV1::LiabilityBasisState)?,
            replay,
            // `custody_request` reads no Claims coordinate, so this local
            // reconstruction of the authenticated snapshot never consults it.
            escrow: FailureEscrowStateV1::Vacant,
        },
        OperationV1::CloseVault,
        [1; 32],
        [2; 32],
        [3; 32],
        replay.next_revision,
        0,
    )?);
    if Pubkey::find_program_address(&replay_seeds.as_slices(), &snapshot.custody_program.key).0
        != snapshot.custody_replay.key
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    authenticate_vault(snapshot, replay, realm)?;
    Ok((replay, realm))
}

fn claims_context(
    snapshot: &MarketRetirementSnapshotV1,
) -> Result<[u8; 32], MarketRetirementOperatorErrorV1> {
    LiabilityBasisMarketViewV2::decode(&snapshot.claims_aggregate.data)
        .map(|view| view.custody_context)
        .map_err(MarketRetirementOperatorErrorV1::LiabilityBasisState)
}

fn authenticate_vault(
    snapshot: &MarketRetirementSnapshotV1,
    replay: CustodyReplayV1,
    realm: RealmV1,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let token = SplTokenAccount::unpack(&snapshot.hoard_vault.data)
        .map_err(|_| MarketRetirementOperatorErrorV1::Custody)?;
    let expected_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            replay.market,
            replay.release_set,
            replay.context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &snapshot.custody_program.key,
    )
    .0;
    if snapshot.hoard_vault.key != expected_vault
        || snapshot.hoard_vault.owner != snapshot.collateral_token_program.key
        || snapshot.hoard_vault.executable
        || snapshot.hoard_vault.lamports == 0
        || token.mint.to_bytes() != *realm.collateral_mint()
        || token.owner != snapshot.custody_authority.key
        || token.amount != 0
        || token.state != AccountState::Initialized
        || token.delegate.is_some()
        || token.delegated_amount != 0
        || token.is_native.is_some()
        || token.close_authority.is_some()
        || !snapshot.collateral_token_program.executable
        || snapshot.collateral_mint.owner != snapshot.collateral_token_program.key
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn custody_request(
    snapshot: &MarketRetirementSnapshotV1,
    authenticated: AuthenticatedRetirementV1,
    operation: OperationV1,
    parent_request_digest: [u8; 32],
    candidate: [u8; 32],
    order: [u8; 32],
    expected_revision: u64,
    transfer_index: u16,
) -> Result<CustodyRequestV1, MarketRetirementOperatorErrorV1> {
    let close_vault = operation == OperationV1::CloseVault;
    if !matches!(
        operation,
        OperationV1::CloseVault | OperationV1::CloseReplay
    ) {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    let resulting_revision = expected_revision
        .checked_add(1)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let request = CustodyRequestV1 {
        operation,
        caller_role: CallerRoleV1::Core,
        source_compartment: if close_vault {
            CompartmentV1::HoardPrincipal
        } else {
            CompartmentV1::None
        },
        destination_compartment: CompartmentV1::None,
        release_set: authenticated
            .market
            .identity
            .selected_release_set
            .to_bytes(),
        market: snapshot.market.key.to_bytes(),
        realm: authenticated.market.identity.realm_id.to_bytes(),
        context: authenticated.replay.context,
        caller_program: snapshot.core_program.key.to_bytes(),
        semantic: ContextV1 {
            candidate,
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order,
            parent_request_digest,
            order_nonce: authenticated.market.identity.generation,
            generation: authenticated.market.identity.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index,
        },
        source: if close_vault {
            snapshot.hoard_vault.key.to_bytes()
        } else {
            [0; 32]
        },
        destination: [0; 32],
        source_vault_context: if close_vault {
            authenticated.replay.context
        } else {
            [0; 32]
        },
        destination_vault_context: [0; 32],
        mint: if close_vault {
            snapshot.collateral_mint.key.to_bytes()
        } else {
            [0; 32]
        },
        token_program: if close_vault {
            snapshot.collateral_token_program.key.to_bytes()
        } else {
            [0; 32]
        },
        payer: [0; 32],
        rent_refund: snapshot.rent_credit.key.to_bytes(),
        expected_revision,
        resulting_revision,
        amount: 0,
        rent_lamports: if close_vault {
            snapshot.hoard_vault.lamports
        } else {
            snapshot.custody_replay.lamports
        },
    };
    request
        .to_bytes()
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    Ok(request)
}

fn authenticate_custody_authority(
    snapshot: &MarketRetirementSnapshotV1,
    close_vault: CustodyRequestV1,
) -> Result<(), MarketRetirementOperatorErrorV1> {
    let expected = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::from_request(close_vault).as_slices(),
        &snapshot.custody_program.key,
    )
    .0;
    if snapshot.custody_authority.key != expected
        || snapshot.custody_authority.owner != system_program::ID
        || snapshot.custody_authority.executable
        || !snapshot.custody_authority.data.is_empty()
    {
        return Err(MarketRetirementOperatorErrorV1::Custody);
    }
    Ok(())
}

fn projected_claims_receipt(
    snapshot: &MarketRetirementSnapshotV1,
    authenticated: AuthenticatedRetirementV1,
    request_digest: [u8; 32],
) -> Result<ClaimsMarketClosureReceiptV1, MarketRetirementOperatorErrorV1> {
    let post_revision = authenticated
        .claims
        .revision
        .checked_add(1)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let credit_after = snapshot
        .rent_credit
        .lamports
        .checked_add(snapshot.claims_aggregate.lamports)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let pre_resource_digest = hashv(&[
        &CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1,
        snapshot.claims_aggregate.key.as_ref(),
        &snapshot.claims_aggregate.data,
    ])
    .to_bytes();
    let post_resource_digest = hashv(&[
        &CLAIMS_MARKET_CLOSURE_POST_RESOURCE_DIGEST_DOMAIN_V1,
        snapshot.claims_aggregate.key.as_ref(),
        snapshot.rent_credit.key.as_ref(),
        &post_revision.to_le_bytes(),
        &snapshot.claims_aggregate.lamports.to_le_bytes(),
        &credit_after.to_le_bytes(),
    ])
    .to_bytes();
    ClaimsMarketClosureReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
        producer: snapshot.claims_program.key.to_bytes(),
        release_set: authenticated
            .market
            .identity
            .selected_release_set
            .to_bytes(),
        market: snapshot.market.key.to_bytes(),
        aggregate: snapshot.claims_aggregate.key.to_bytes(),
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        request_digest,
        pre_resource_digest,
        post_resource_digest,
        generation: authenticated.market.identity.generation,
        pre_revision: authenticated.claims.revision,
        post_revision,
        liability_units: 0,
        refund_lamports: snapshot.claims_aggregate.lamports,
        claim_count: authenticated.claims.claim_count,
    })
    .map_err(MarketRetirementOperatorErrorV1::ClaimsMarketClosure)
}

fn projected_claims_handoff_receipt(
    snapshot: &MarketRetirementSnapshotV1,
    authenticated: AuthenticatedRetirementV1,
    request_digest: [u8; 32],
) -> Result<ClaimsRetirementCheckpointHandoffReceiptV1, MarketRetirementOperatorErrorV1> {
    let post_revision = authenticated
        .claims
        .revision
        .checked_add(1)
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let pre_resource_digest = hashv(&[
        &CLAIMS_MARKET_CLOSURE_PRE_RESOURCE_DIGEST_DOMAIN_V1,
        snapshot.claims_aggregate.key.as_ref(),
        &snapshot.claims_aggregate.data,
    ])
    .to_bytes();
    // What the checkpoint account holds after the handoff. The closure hands
    // the escrow pair's rent to the aggregate rather than to a fourth account,
    // so on a refunding Market this is strictly more than the aggregate came in
    // with -- and it is the ONE number Core asserts, the receipt carries and
    // `Finish` eventually pays the refund wallet.
    let refund_lamports = snapshot
        .claims_aggregate
        .lamports
        .checked_add(authenticated.escrow.rent())
        .ok_or(MarketRetirementOperatorErrorV1::Arithmetic)?;
    let post_resource_digest = hashv(&[
        CLAIMS_RETIREMENT_CHECKPOINT_HANDOFF_POST_DIGEST_DOMAIN_V1,
        snapshot.claims_aggregate.key.as_ref(),
        snapshot.core_program.key.as_ref(),
        AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1
            .to_le_bytes()
            .as_slice(),
        refund_lamports.to_le_bytes().as_slice(),
        snapshot.rent_credit.lamports.to_le_bytes().as_slice(),
    ])
    .to_bytes();
    ClaimsRetirementCheckpointHandoffReceiptV1::new(ClaimsMarketClosureReceiptInputV1 {
        producer: snapshot.claims_program.key.to_bytes(),
        release_set: authenticated
            .market
            .identity
            .selected_release_set
            .to_bytes(),
        market: snapshot.market.key.to_bytes(),
        aggregate: snapshot.claims_aggregate.key.to_bytes(),
        rent_credit: snapshot.rent_credit.key.to_bytes(),
        request_digest,
        pre_resource_digest,
        post_resource_digest,
        generation: authenticated.market.identity.generation,
        pre_revision: authenticated.claims.revision,
        post_revision,
        liability_units: 0,
        refund_lamports,
        claim_count: authenticated.claims.claim_count,
    })
    .map_err(MarketRetirementOperatorErrorV1::ClaimsMarketClosure)
}

fn projected_custody_receipts(
    snapshot: &MarketRetirementSnapshotV1,
    authenticated: AuthenticatedRetirementV1,
    close_vault: CustodyRequestV1,
    close_vault_bytes: &[u8],
    close_replay: CustodyRequestV1,
    close_replay_bytes: &[u8],
) -> Result<([u8; 32], [u8; 32]), MarketRetirementOperatorErrorV1> {
    let close_vault_digest = hash(close_vault_bytes).to_bytes();
    let close_vault_poststate = custody_poststate(
        close_vault_digest,
        snapshot.hoard_vault.key,
        snapshot.rent_credit.key,
        snapshot.hoard_vault.lamports,
    );
    let replay_after_vault = authenticated
        .replay
        .advance(close_vault, close_vault_digest, close_vault_poststate)
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let replay_after_vault_bytes = replay_after_vault
        .to_bytes()
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let close_vault_receipt = CustodyReceiptV1::new(
        close_vault,
        close_vault_digest,
        ReceiptEvidenceV1 {
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            poststate_commitment: close_vault_poststate,
            replay_state_digest: hash(&replay_after_vault_bytes).to_bytes(),
        },
    )
    .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let close_vault_receipt_digest = hash(
        &close_vault_receipt
            .to_bytes()
            .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?,
    )
    .to_bytes();

    let close_replay_digest = hash(close_replay_bytes).to_bytes();
    let close_replay_poststate = custody_poststate(
        close_replay_digest,
        snapshot.custody_replay.key,
        snapshot.rent_credit.key,
        snapshot.custody_replay.lamports,
    );
    replay_after_vault
        .advance(close_replay, close_replay_digest, close_replay_poststate)
        .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let close_replay_receipt = CustodyReceiptV1::new(
        close_replay,
        close_replay_digest,
        ReceiptEvidenceV1 {
            source_before: 0,
            source_after: 0,
            destination_before: 0,
            destination_after: 0,
            poststate_commitment: close_replay_poststate,
            replay_state_digest: hash(&[]).to_bytes(),
        },
    )
    .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?;
    let close_replay_receipt_digest = hash(
        &close_replay_receipt
            .to_bytes()
            .map_err(MarketRetirementOperatorErrorV1::CustodyContract)?,
    )
    .to_bytes();
    Ok((close_vault_receipt_digest, close_replay_receipt_digest))
}

fn custody_poststate(
    request_digest: [u8; 32],
    source: Pubkey,
    destination: Pubkey,
    rent_lamports: u64,
) -> [u8; 32] {
    hashv(&[
        CUSTODY_POSTSTATE_DOMAIN_V1,
        &request_digest,
        source.as_ref(),
        destination.as_ref(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &rent_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

fn caller_authority(
    release_set: [u8; 32],
    market: Pubkey,
    context: [u8; 32],
    request_bytes: &[u8],
    core_program: Pubkey,
) -> Result<Pubkey, MarketRetirementOperatorErrorV1> {
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        context,
        hash(request_bytes).to_bytes(),
    )
    .map_err(MarketRetirementOperatorErrorV1::ReleaseSet)?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &core_program).0)
}

/// The Core retirement frame, and the escrow tail a refunding Market adds.
///
/// The tail is TRAILING and appended for the whole retirement rather than per
/// packet: `aggregate_retirement_journal.rs` requires the four checkpoint
/// operations to present an identical account frame, so the packets that never
/// read the escrow carry it anyway. A categorical Market's retirement is the
/// exact thirty-five metas that shipped, in the exact order.
fn core_accounts(
    snapshot: &MarketRetirementSnapshotV1,
    claims_authority: Pubkey,
    close_vault_authority: Pubkey,
    close_replay_authority: Pubkey,
    rent_close_authority: Pubkey,
    escrow: FailureEscrowStateV1,
) -> Vec<AccountMeta> {
    let mut accounts = vec![
        AccountMeta::new(snapshot.market.key, false),
        AccountMeta::new(snapshot.rent_credit.key, false),
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.registry_program.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.claims_program.key, false),
        AccountMeta::new_readonly(snapshot.claims_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.custody_program.key, false),
        AccountMeta::new_readonly(snapshot.custody_programdata.key, false),
        AccountMeta::new_readonly(snapshot.rent_program.key, false),
        AccountMeta::new_readonly(snapshot.source_receipt.key, false),
        AccountMeta::new(snapshot.claims_aggregate.key, false),
        AccountMeta::new(snapshot.custody_replay.key, false),
        AccountMeta::new(snapshot.hoard_vault.key, false),
        AccountMeta::new_readonly(snapshot.custody_authority.key, false),
        AccountMeta::new_readonly(snapshot.collateral_mint.key, false),
        AccountMeta::new_readonly(snapshot.collateral_token_program.key, false),
        AccountMeta::new_readonly(snapshot.realm_raw.key, false),
        AccountMeta::new_readonly(snapshot.realm_staging.key, false),
        AccountMeta::new_readonly(claims_authority, false),
        AccountMeta::new_readonly(close_vault_authority, false),
        AccountMeta::new_readonly(close_replay_authority, false),
        AccountMeta::new_readonly(snapshot.infrastructure_profile.key, false),
        AccountMeta::new_readonly(snapshot.registry_artifact_raw.key, false),
        AccountMeta::new_readonly(snapshot.registry_artifact_staging.key, false),
        AccountMeta::new_readonly(snapshot.registry_programdata.key, false),
        AccountMeta::new_readonly(snapshot.rent_artifact_raw.key, false),
        AccountMeta::new_readonly(snapshot.rent_artifact_staging.key, false),
        AccountMeta::new_readonly(snapshot.rent_programdata.key, false),
        AccountMeta::new_readonly(snapshot.rent_sysvar.key, false),
        AccountMeta::new(snapshot.refund_wallet.key, false),
        AccountMeta::new_readonly(rent_close_authority, false),
    ];
    if let (true, Some(position), Some(admission), Some(basis)) = (
        escrow.seated(),
        snapshot.failure_escrow_position.as_ref(),
        snapshot.failure_escrow_admission.as_ref(),
        snapshot.linked_basis_record.as_ref(),
    ) {
        let tail = [
            AccountMeta::new(position.key, false),
            AccountMeta::new(admission.key, false),
            AccountMeta::new_readonly(basis.key, false),
        ];
        debug_assert_eq!(tail.len(), CORE_RETIREMENT_ESCROW_TAIL_ACCOUNTS_V1);
        accounts.extend_from_slice(&tail);
    }
    accounts
}

fn wrap_registry_continuation(
    snapshot: &MarketRetirementSnapshotV1,
    direct: &Instruction,
) -> Result<(Instruction, Pubkey, RegistryContinuationRequestV1), MarketRetirementOperatorErrorV1> {
    let release_set = ContentId::new(
        CoreState::decode(&snapshot.market.data)
            .map_err(|_| MarketRetirementOperatorErrorV1::Market)?
            .identity
            .selected_release_set
            .to_bytes(),
    )
    .map_err(MarketRetirementOperatorErrorV1::Core)?;
    let activation_digest = ContentId::new(hash(&snapshot.activation_cache.data).to_bytes())
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let instruction_digest = ContentId::new(hash(&direct.data).to_bytes())
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let instruction_len =
        u32::try_from(direct.data.len()).map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let roles = [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ];
    let continuation = RegistryContinuationRequestV1::new(
        release_set,
        activation_digest,
        instruction_digest,
        instruction_len,
        ExecutionRoleV1::Core,
        &roles,
    )
    .map_err(MarketRetirementOperatorErrorV1::Batch)?;
    let batch = continuation
        .role_batch_request()
        .map_err(MarketRetirementOperatorErrorV1::Batch)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| MarketRetirementOperatorErrorV1::Encoding)?;
    let seeds = RegistryContinuationAdmissionSeedsV1::new(
        continuation,
        snapshot.activation_cache.key.to_bytes(),
        batch_digest,
    )
    .map_err(MarketRetirementOperatorErrorV1::Batch)?;
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let role_batch = seeds.batch_request_digest();
    let role_mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let continuation_digest = seeds.continuation_digest();
    let admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            role_batch.as_slice(),
            role_mask.as_slice(),
            role.as_slice(),
            continuation_digest.as_slice(),
        ],
        &snapshot.registry_program.key,
    )
    .0;
    if direct.accounts.iter().any(|meta| meta.pubkey == admission) {
        return Err(MarketRetirementOperatorErrorV1::Frame);
    }

    let mut child_accounts = direct.accounts.clone();
    child_accounts.push(AccountMeta::new_readonly(admission, false));
    let mut accounts = Vec::with_capacity(MARKET_RETIREMENT_ACCOUNT_COUNT_V1);
    accounts.extend([
        AccountMeta::new_readonly(snapshot.activation_cache.key, false),
        AccountMeta::new_readonly(snapshot.core_program.key, false),
        AccountMeta::new_readonly(snapshot.core_programdata.key, false),
        AccountMeta::new_readonly(snapshot.claims_program.key, false),
        AccountMeta::new_readonly(snapshot.claims_programdata.key, false),
        AccountMeta::new_readonly(snapshot.resolution_program.key, false),
        AccountMeta::new_readonly(snapshot.resolution_programdata.key, false),
        AccountMeta::new_readonly(snapshot.custody_program.key, false),
        AccountMeta::new_readonly(snapshot.custody_programdata.key, false),
        AccountMeta::new_readonly(admission, false),
    ]);
    accounts.extend(child_accounts);
    if accounts.len() != MARKET_RETIREMENT_ACCOUNT_COUNT_V1 {
        return Err(MarketRetirementOperatorErrorV1::Frame);
    }
    let mut data = Vec::with_capacity(
        REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + MARKET_RETIREMENT_CORE_INSTRUCTION_BYTES_V1,
    );
    data.extend_from_slice(&continuation.to_bytes());
    data.extend_from_slice(&direct.data);
    Ok((
        Instruction {
            program_id: snapshot.registry_program.key,
            accounts,
            data,
        },
        admission,
        continuation,
    ))
}
