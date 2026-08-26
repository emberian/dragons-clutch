//! Complete General Hot36 candidate-register ABI.
//!
//! The AccountProfile first projects independently authenticated readonly
//! observations into one exact register bank.  General then overwrites only
//! the semantic coordinates it owns, preserving every other byte until the
//! complete runtime-width plan accepts.  The resulting bank contains every
//! identity, revision, Position-table coordinate, and Custody replay fact
//! needed to project canonical `AffineBatchV2`, `ProtocolPositionV2`, and
//! `CustodyRequestV1` packets.  Trading remains the sole writer and CPI
//! authority.

use dclutch_execution_strategy_contract::v2::{ExecutionCandidateV2, register_bank_bytes_v2};

use crate::{
    runtime_settlement::{RuntimeSettlementActionV2, RuntimeSettlementEffectPlanV2},
    runtime_verify::RuntimeCompleteSetMoveV2,
};

/// Exact common scalar-register count in the General Hot36 ABI.
pub const GENERAL_HOT_COMMON_SCALARS_V3: u32 = 44;
/// Outcome index plus exact quantity for every Product outcome.
pub const GENERAL_HOT_ITEM_SCALAR_STRIDE_V3: u32 = 2;
/// Exact common identity-register count in the General Hot36 ABI.
pub const GENERAL_HOT_COMMON_IDENTITIES_V3: u32 = 27;
/// General has no per-outcome identity tail.
pub const GENERAL_HOT_ITEM_IDENTITY_STRIDE_V3: u32 = 0;

/// Scalar coordinates consumed by exact child-packet projection.
pub mod scalar {
    /// Settlement action tag.
    pub const ACTION: u32 = 0;
    /// Complete-set movement tag.
    pub const COMPLETE_SET_MOVE: u32 = 1;
    /// Whether the affine Claims route is active.
    pub const CLAIMS_AFFINE_ACTIVE: u32 = 2;
    /// Whether the Custody route is active.
    pub const CUSTODY_ACTIVE: u32 = 3;
    /// Whether the General state becomes terminal.
    pub const TERMINAL: u32 = 4;
    /// One-based order coordinate.
    pub const ORDER_COORDINATE: u32 = 5;
    /// Consumed General settlement revision.
    pub const SETTLEMENT_REVISION: u32 = 6;
    /// Signed-order replay nonce.
    pub const ORDER_NONCE: u32 = 7;
    /// Exact collateral quantity and Custody transfer amount.
    pub const QUOTE_QUANTITY: u32 = 8;
    /// Uniform complete-set quantity.
    pub const COMPLETE_SET_QUANTITY: u32 = 9;
    /// Product-authenticated outcome count echoed by AccountProfile.
    pub const OUTCOME_COUNT: u32 = 10;
    /// Nonzero General terminal coordinate.
    pub const TERMINAL_COORDINATE: u32 = 11;
    /// Immutable Market generation.
    pub const GENERATION: u32 = 12;
    /// Immutable page coordinate.
    pub const PAGE_INDEX: u32 = 13;
    /// Immutable execution coordinate.
    pub const EXECUTION_INDEX: u32 = 14;
    /// Ordered Custody transfer coordinate.
    pub const TRANSFER_INDEX: u32 = 15;
    /// Custody replay pre-revision.
    pub const CUSTODY_EXPECTED_REVISION: u32 = 16;
    /// Custody replay required post-revision.
    pub const CUSTODY_RESULTING_REVISION: u32 = 17;
    /// Exact Custody rent movement; zero for settlement transfers.
    pub const CUSTODY_RENT_LAMPORTS: u32 = 18;
    /// Claims aggregate pre-revision.
    pub const CLAIMS_MARKET_REVISION: u32 = 19;
    /// Current row-owner Position pre-revision.
    pub const OWNER_POSITION_REVISION: u32 = 20;
    /// Current settlement Position pre-revision.
    pub const SETTLEMENT_POSITION_REVISION: u32 = 21;
    /// Exact AffineBatch position-table count.
    pub const CLAIMS_POSITION_COUNT: u32 = 22;
    /// Exact AffineBatch row count, equal to Product N when active.
    pub const CLAIMS_ROW_COUNT: u32 = 23;
    /// Whether a vacant settlement Position must be admitted.
    pub const CLAIMS_ADMIT_ACTIVE: u32 = 24;
    /// Whether a zero settlement Position must be closed after affine mutation.
    pub const CLAIMS_CLOSE_ACTIVE: u32 = 25;
    /// Canonical Custody operation tag (`Transfer`).
    pub const CUSTODY_OPERATION: u32 = 26;
    /// Canonical Custody source-compartment tag.
    pub const CUSTODY_SOURCE_COMPARTMENT: u32 = 27;
    /// Canonical Custody destination-compartment tag.
    pub const CUSTODY_DESTINATION_COMPARTMENT: u32 = 28;
    /// Affine row source-presence bit.
    pub const CLAIMS_SOURCE_PRESENT: u32 = 29;
    /// Affine row destination-presence bit.
    pub const CLAIMS_DESTINATION_PRESENT: u32 = 30;
    /// Canonical sorted Position-table source index.
    pub const CLAIMS_SOURCE_POSITION_INDEX: u32 = 31;
    /// Canonical sorted Position-table destination index.
    pub const CLAIMS_DESTINATION_POSITION_INDEX: u32 = 32;
    /// Affine aggregate signed-magnitude direction.
    pub const CLAIMS_AGGREGATE_DIRECTION: u32 = 33;
    /// Affine source signed-magnitude direction.
    pub const CLAIMS_SOURCE_DIRECTION: u32 = 34;
    /// Affine destination signed-magnitude direction.
    pub const CLAIMS_DESTINATION_DIRECTION: u32 = 35;
    /// Observed settlement Position lamports for lifecycle admission/close.
    pub const OBSERVED_POSITION_LAMPORTS: u32 = 36;
    /// Observed admission-record lamports.
    pub const OBSERVED_ADMISSION_LAMPORTS: u32 = 37;
    /// Current Position rent principal.
    pub const POSITION_RENT_PRINCIPAL: u32 = 38;
    /// Current admission-record rent principal.
    pub const ADMISSION_RENT_PRINCIPAL: u32 = 39;
    /// Whether the settlement Position exists before this action.
    pub const SETTLEMENT_POSITION_PRESENT: u32 = 40;
    /// Canonical sorted Position-table coordinate zero pre-revision.
    pub const POSITION_ZERO_REVISION: u32 = 41;
    /// Canonical sorted Position-table coordinate one pre-revision.
    pub const POSITION_ONE_REVISION: u32 = 42;
    /// Exact total number of active position coordinates (zero, one, or two).
    pub const POSITION_TABLE_COUNT: u32 = 43;
}

/// Identity coordinates consumed by exact child-packet projection.
pub mod identity {
    /// Digest of the authenticated Hot parent request.
    pub const PARENT_REQUEST_DIGEST: u32 = 0;
    /// Best-valid-submitted Candidate identity.
    pub const CANDIDATE: u32 = 1;
    /// Current row owner.
    pub const OWNER: u32 = 2;
    /// Current signed-order identity.
    pub const ORDER: u32 = 3;
    /// Immutable terminal surplus beneficiary.
    pub const BENEFICIARY: u32 = 4;
    /// Selected release-set identity.
    pub const RELEASE_SET: u32 = 5;
    /// Canonical logical Market identity.
    pub const MARKET: u32 = 6;
    /// Exact Product record digest.
    pub const PRODUCT_RECORD_DIGEST: u32 = 7;
    /// Semantic LiabilityBasis identity.
    pub const SEMANTIC_BASIS_ID: u32 = 8;
    /// Linked-basis finalized-record digest.
    pub const LINKED_BASIS_RECORD_DIGEST: u32 = 9;
    /// Immutable Realm identity.
    pub const REALM: u32 = 10;
    /// Registry-selected Trading program.
    pub const TRADING_PROGRAM: u32 = 11;
    /// Exact Custody source token account.
    pub const CUSTODY_SOURCE: u32 = 12;
    /// Exact Custody destination token account.
    pub const CUSTODY_DESTINATION: u32 = 13;
    /// Custody source-vault context, zero only for External.
    pub const SOURCE_VAULT_CONTEXT: u32 = 14;
    /// Custody destination-vault context, zero only for External.
    pub const DESTINATION_VAULT_CONTEXT: u32 = 15;
    /// Realm-selected collateral Mint.
    pub const MINT: u32 = 16;
    /// Realm-selected Token or Token-2022 program.
    pub const TOKEN_PROGRAM: u32 = 17;
    /// Custody rent payer; zero for Transfer.
    pub const PAYER: u32 = 18;
    /// Custody rent beneficiary; zero for Transfer.
    pub const RENT_REFUND: u32 = 19;
    /// General settlement Position owner.
    pub const SETTLEMENT_POSITION_OWNER: u32 = 20;
    /// Permanent RentCredit identity for Claims lifecycle.
    pub const RENT_CREDIT: u32 = 21;
    /// Program owning the RentCredit.
    pub const RENT_PROGRAM: u32 = 22;
    /// External Custody source owner, zero for a vault.
    pub const CUSTODY_SOURCE_OWNER: u32 = 23;
    /// External Custody destination owner, zero for a vault.
    pub const CUSTODY_DESTINATION_OWNER: u32 = 24;
    /// Canonical sorted Position-table owner zero.
    pub const POSITION_ZERO_OWNER: u32 = 25;
    /// Canonical sorted Position-table owner one; zero for one-position plans.
    pub const POSITION_ONE_OWNER: u32 = 26;
}

/// Independently authenticated environment needed by exact Claims/Custody packets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralHotEnvironmentV3 {
    /// Digest of the exact authenticated Hot request.
    pub parent_request_digest: [u8; 32],
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Canonical logical Market.
    pub market: [u8; 32],
    /// Exact Product-record digest.
    pub product_record_digest: [u8; 32],
    /// Exact semantic LiabilityBasis identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact linked-basis finalized-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Realm collateral authority.
    pub realm: [u8; 32],
    /// Selected Trading program.
    pub trading_program: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Exact candidate-page coordinate for row actions, zero otherwise.
    pub page_index: u32,
    /// Exact execution coordinate inside the page, zero otherwise.
    pub execution_index: u32,
    /// Claims Market optimistic revision.
    pub claims_market_revision: u64,
    /// Current row-owner Position revision.
    pub owner_position_revision: u64,
    /// Current settlement Position revision.
    pub settlement_position_revision: u64,
    /// Whether the settlement Position exists before this action.
    pub settlement_position_present: bool,
    /// Whether the post-affine settlement Position is proven empty and closeable.
    pub close_settlement_position: bool,
    /// Exact settlement Position owner.
    pub settlement_position_owner: [u8; 32],
    /// Claims Position RentCredit.
    pub rent_credit: [u8; 32],
    /// Program owning the RentCredit.
    pub rent_program: [u8; 32],
    /// Observed Position lamports.
    pub observed_position_lamports: u64,
    /// Observed admission-record lamports.
    pub observed_admission_lamports: u64,
    /// Position rent principal.
    pub position_rent_principal: u64,
    /// Admission-record rent principal.
    pub admission_rent_principal: u64,
    /// Exact Custody source token account.
    pub custody_source: [u8; 32],
    /// Exact Custody destination token account.
    pub custody_destination: [u8; 32],
    /// Exact external source owner or zero.
    pub custody_source_owner: [u8; 32],
    /// Exact external destination owner or zero.
    pub custody_destination_owner: [u8; 32],
    /// Source-vault context or zero for External.
    pub source_vault_context: [u8; 32],
    /// Destination-vault context or zero for External.
    pub destination_vault_context: [u8; 32],
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Custody payer; Transfer requires zero.
    pub payer: [u8; 32],
    /// Custody rent refund; Transfer requires zero.
    pub rent_refund: [u8; 32],
    /// Custody replay pre-revision.
    pub custody_expected_revision: u64,
    /// Ordered transfer index.
    pub transfer_index: u16,
}

/// Stable refusal from General Hot candidate projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralHotCandidateErrorV3 {
    /// Settlement effect bytes refused.
    InvalidPlan,
    /// Product width differed from the exact effect width.
    TailCountMismatch,
    /// Caller-owned banks were not one exact complete candidate width.
    InvalidCapacity,
    /// An authenticated child coordinate was zero, aliased, or noncanonical.
    InvalidCoordinate,
    /// Position or Custody optimistic revision could not advance.
    RevisionOverflow,
    /// Checked register or byte arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for General Hot candidate projection.
pub type Result<T> = core::result::Result<T, GeneralHotCandidateErrorV3>;

/// Return the exact scalar count for Product width `outcome_count`.
pub fn general_hot_scalar_count_v3(outcome_count: u32) -> Result<u32> {
    GENERAL_HOT_COMMON_SCALARS_V3
        .checked_add(
            outcome_count
                .checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)
}

/// Return the exact scalar-then-identity bank width.
pub fn general_hot_candidate_bank_len_v3(outcome_count: u32) -> Result<usize> {
    if outcome_count == 0 {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    let bytes = register_bank_bytes_v2(
        general_hot_scalar_count_v3(outcome_count)?,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
    )
    .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    usize::try_from(bytes).map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)
}

/// Project one complete General plan without discarding authenticated inputs.
///
/// `authenticated_input`, `scratch`, and `output` must have the one exact
/// Product-derived capacity. The entire input is copied to scratch first;
/// output changes only after every semantic and child-ABI coordinate accepts.
pub fn project_general_hot_candidate_v3<'a>(
    effect_plan: &[u8],
    outcome_count: u32,
    environment: GeneralHotEnvironmentV3,
    authenticated_input: &[u8],
    scratch: &mut [u8],
    output: &'a mut [u8],
) -> Result<ExecutionCandidateV2<'a>> {
    let plan = RuntimeSettlementEffectPlanV2::decode(effect_plan)
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?;
    if plan.header().outcome_count != outcome_count {
        return Err(GeneralHotCandidateErrorV3::TailCountMismatch);
    }
    let required = general_hot_candidate_bank_len_v3(outcome_count)?;
    if authenticated_input.len() != required
        || scratch.len() != required
        || output.len() != required
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCapacity);
    }
    let input_scalar_count = general_hot_scalar_count_v3(outcome_count)?;
    if read_scalar(authenticated_input, scalar::OUTCOME_COUNT)? != u64::from(outcome_count)
        || read_identity(
            authenticated_input,
            input_scalar_count,
            identity::PARENT_REQUEST_DIGEST,
        )? != environment.parent_request_digest
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    validate_environment(
        plan.header().action,
        plan.header().custody_active,
        environment,
    )?;
    scratch.copy_from_slice(authenticated_input);
    let header = plan.header();
    let position = position_geometry(header.action, header.claims_active, environment, header)?;
    let custody = custody_geometry(
        header.action,
        header.custody_active,
        header.complete_set_move,
    );
    let custody_resulting_revision = environment
        .custody_expected_revision
        .checked_add(1)
        .ok_or(GeneralHotCandidateErrorV3::RevisionOverflow)?;
    for (coordinate, value) in [
        (scalar::ACTION, action_tag(header.action)),
        (
            scalar::COMPLETE_SET_MOVE,
            move_tag(header.complete_set_move),
        ),
        (
            scalar::CLAIMS_AFFINE_ACTIVE,
            u64::from(header.claims_active),
        ),
        (scalar::CUSTODY_ACTIVE, u64::from(header.custody_active)),
        (scalar::TERMINAL, u64::from(header.terminal)),
        (scalar::ORDER_COORDINATE, u64::from(header.order_coordinate)),
        (scalar::SETTLEMENT_REVISION, header.revision),
        (scalar::ORDER_NONCE, header.nonce),
        (scalar::QUOTE_QUANTITY, header.quote_quantity),
        (scalar::COMPLETE_SET_QUANTITY, header.complete_set_quantity),
        (scalar::OUTCOME_COUNT, u64::from(outcome_count)),
        (scalar::TERMINAL_COORDINATE, header.terminal_coordinate),
        (scalar::GENERATION, environment.generation),
        (scalar::PAGE_INDEX, u64::from(environment.page_index)),
        (
            scalar::EXECUTION_INDEX,
            u64::from(environment.execution_index),
        ),
        (
            scalar::TRANSFER_INDEX,
            u64::from(environment.transfer_index),
        ),
        (
            scalar::CUSTODY_EXPECTED_REVISION,
            environment.custody_expected_revision,
        ),
        (
            scalar::CUSTODY_RESULTING_REVISION,
            custody_resulting_revision,
        ),
        (scalar::CUSTODY_RENT_LAMPORTS, 0),
        (
            scalar::CLAIMS_MARKET_REVISION,
            environment.claims_market_revision,
        ),
        (
            scalar::OWNER_POSITION_REVISION,
            environment.owner_position_revision,
        ),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            environment.settlement_position_revision,
        ),
        (scalar::CLAIMS_POSITION_COUNT, u64::from(position.count)),
        (
            scalar::CLAIMS_ROW_COUNT,
            if header.claims_active {
                u64::from(outcome_count)
            } else {
                0
            },
        ),
        (scalar::CLAIMS_ADMIT_ACTIVE, u64::from(position.admit)),
        (scalar::CLAIMS_CLOSE_ACTIVE, u64::from(position.close)),
        (scalar::CUSTODY_OPERATION, 2),
        (scalar::CUSTODY_SOURCE_COMPARTMENT, custody.source),
        (scalar::CUSTODY_DESTINATION_COMPARTMENT, custody.destination),
        (
            scalar::CLAIMS_SOURCE_PRESENT,
            u64::from(position.source_present),
        ),
        (
            scalar::CLAIMS_DESTINATION_PRESENT,
            u64::from(position.destination_present),
        ),
        (
            scalar::CLAIMS_SOURCE_POSITION_INDEX,
            u64::from(position.source_index),
        ),
        (
            scalar::CLAIMS_DESTINATION_POSITION_INDEX,
            u64::from(position.destination_index),
        ),
        (
            scalar::CLAIMS_AGGREGATE_DIRECTION,
            position.aggregate_direction,
        ),
        (scalar::CLAIMS_SOURCE_DIRECTION, position.source_direction),
        (
            scalar::CLAIMS_DESTINATION_DIRECTION,
            position.destination_direction,
        ),
        (
            scalar::OBSERVED_POSITION_LAMPORTS,
            environment.observed_position_lamports,
        ),
        (
            scalar::OBSERVED_ADMISSION_LAMPORTS,
            environment.observed_admission_lamports,
        ),
        (
            scalar::POSITION_RENT_PRINCIPAL,
            environment.position_rent_principal,
        ),
        (
            scalar::ADMISSION_RENT_PRINCIPAL,
            environment.admission_rent_principal,
        ),
        (
            scalar::SETTLEMENT_POSITION_PRESENT,
            u64::from(environment.settlement_position_present),
        ),
        (scalar::POSITION_ZERO_REVISION, position.zero_revision),
        (scalar::POSITION_ONE_REVISION, position.one_revision),
        (scalar::POSITION_TABLE_COUNT, u64::from(position.count)),
    ] {
        write_scalar(scratch, coordinate, value)?;
    }
    for item in 0..outcome_count {
        let base = GENERAL_HOT_COMMON_SCALARS_V3
            .checked_add(
                item.checked_mul(GENERAL_HOT_ITEM_SCALAR_STRIDE_V3)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            )
            .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
        write_scalar(scratch, base, u64::from(item))?;
        write_scalar(
            scratch,
            base.checked_add(1)
                .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
            plan.quantity(item)
                .map_err(|_| GeneralHotCandidateErrorV3::InvalidPlan)?,
        )?;
    }
    let scalar_count = general_hot_scalar_count_v3(outcome_count)?;
    for (coordinate, value) in [
        (
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        ),
        (identity::CANDIDATE, header.candidate_id),
        (identity::OWNER, header.owner_id),
        (identity::ORDER, header.order_id),
        (identity::BENEFICIARY, header.beneficiary),
        (identity::RELEASE_SET, environment.release_set),
        (identity::MARKET, environment.market),
        (
            identity::PRODUCT_RECORD_DIGEST,
            environment.product_record_digest,
        ),
        (identity::SEMANTIC_BASIS_ID, environment.semantic_basis_id),
        (
            identity::LINKED_BASIS_RECORD_DIGEST,
            environment.linked_basis_record_digest,
        ),
        (identity::REALM, environment.realm),
        (identity::TRADING_PROGRAM, environment.trading_program),
        (identity::CUSTODY_SOURCE, environment.custody_source),
        (
            identity::CUSTODY_DESTINATION,
            environment.custody_destination,
        ),
        (
            identity::SOURCE_VAULT_CONTEXT,
            environment.source_vault_context,
        ),
        (
            identity::DESTINATION_VAULT_CONTEXT,
            environment.destination_vault_context,
        ),
        (identity::MINT, environment.mint),
        (identity::TOKEN_PROGRAM, environment.token_program),
        (identity::PAYER, environment.payer),
        (identity::RENT_REFUND, environment.rent_refund),
        (
            identity::SETTLEMENT_POSITION_OWNER,
            environment.settlement_position_owner,
        ),
        (identity::RENT_CREDIT, environment.rent_credit),
        (identity::RENT_PROGRAM, environment.rent_program),
        (
            identity::CUSTODY_SOURCE_OWNER,
            environment.custody_source_owner,
        ),
        (
            identity::CUSTODY_DESTINATION_OWNER,
            environment.custody_destination_owner,
        ),
        (identity::POSITION_ZERO_OWNER, position.zero_owner),
        (identity::POSITION_ONE_OWNER, position.one_owner),
    ] {
        write_identity(scratch, scalar_count, coordinate, value)?;
    }
    output.copy_from_slice(scratch);
    Ok(ExecutionCandidateV2::Accepted(output))
}

#[derive(Clone, Copy)]
struct PositionGeometryV3 {
    count: u32,
    source_present: bool,
    destination_present: bool,
    source_index: u32,
    destination_index: u32,
    aggregate_direction: u64,
    source_direction: u64,
    destination_direction: u64,
    zero_owner: [u8; 32],
    one_owner: [u8; 32],
    zero_revision: u64,
    one_revision: u64,
    admit: bool,
    close: bool,
}

fn position_geometry(
    action: RuntimeSettlementActionV2,
    claims_active: bool,
    environment: GeneralHotEnvironmentV3,
    header: crate::runtime_settlement::RuntimeSettlementEffectHeaderV2,
) -> Result<PositionGeometryV3> {
    if !claims_active {
        return Ok(PositionGeometryV3 {
            count: 0,
            source_present: false,
            destination_present: false,
            source_index: 0,
            destination_index: 0,
            aggregate_direction: 0,
            source_direction: 0,
            destination_direction: 0,
            zero_owner: [0; 32],
            one_owner: [0; 32],
            zero_revision: 0,
            one_revision: 0,
            admit: false,
            close: false,
        });
    }
    match action {
        RuntimeSettlementActionV2::Collect | RuntimeSettlementActionV2::Distribute => {
            if header.owner_id == environment.settlement_position_owner {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
            let collect = action == RuntimeSettlementActionV2::Collect;
            let (source_owner, source_revision, destination_owner, destination_revision) =
                if collect {
                    (
                        header.owner_id,
                        environment.owner_position_revision,
                        environment.settlement_position_owner,
                        environment.settlement_position_revision,
                    )
                } else {
                    (
                        environment.settlement_position_owner,
                        environment.settlement_position_revision,
                        header.owner_id,
                        environment.owner_position_revision,
                    )
                };
            let (zero_owner, zero_revision, one_owner, one_revision, source_index) =
                if source_owner < destination_owner {
                    (
                        source_owner,
                        source_revision,
                        destination_owner,
                        destination_revision,
                        0,
                    )
                } else {
                    (
                        destination_owner,
                        destination_revision,
                        source_owner,
                        source_revision,
                        1,
                    )
                };
            Ok(PositionGeometryV3 {
                count: 2,
                source_present: true,
                destination_present: true,
                source_index,
                destination_index: 1 - source_index,
                aggregate_direction: 0,
                source_direction: 2,
                destination_direction: 1,
                zero_owner,
                one_owner,
                zero_revision,
                one_revision,
                admit: collect && !environment.settlement_position_present,
                close: !collect && environment.close_settlement_position,
            })
        }
        RuntimeSettlementActionV2::Materialize => {
            let (source_present, destination_present, aggregate, source, destination) =
                match header.complete_set_move {
                    RuntimeCompleteSetMoveV2::Mint => (false, true, 1, 0, 1),
                    RuntimeCompleteSetMoveV2::Merge => (true, false, 2, 2, 0),
                    RuntimeCompleteSetMoveV2::None => {
                        return Err(GeneralHotCandidateErrorV3::InvalidPlan);
                    }
                };
            let position_revision = if environment.settlement_position_present {
                environment.settlement_position_revision
            } else {
                if environment.settlement_position_revision != 0 {
                    return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
                }
                0
            };
            Ok(PositionGeometryV3 {
                count: 1,
                source_present,
                destination_present,
                source_index: 0,
                destination_index: 0,
                aggregate_direction: aggregate,
                source_direction: source,
                destination_direction: destination,
                zero_owner: environment.settlement_position_owner,
                one_owner: [0; 32],
                zero_revision: position_revision,
                one_revision: 0,
                admit: !environment.settlement_position_present,
                close: false,
            })
        }
        RuntimeSettlementActionV2::Close => Err(GeneralHotCandidateErrorV3::InvalidPlan),
    }
}

#[derive(Clone, Copy)]
struct CustodyGeometryV3 {
    source: u64,
    destination: u64,
}

fn custody_geometry(
    action: RuntimeSettlementActionV2,
    active: bool,
    movement: RuntimeCompleteSetMoveV2,
) -> CustodyGeometryV3 {
    if !active {
        return CustodyGeometryV3 {
            source: 0,
            destination: 0,
        };
    }
    let (source, destination) = match action {
        RuntimeSettlementActionV2::Collect => (1, 2),
        RuntimeSettlementActionV2::Distribute | RuntimeSettlementActionV2::Close => (2, 1),
        RuntimeSettlementActionV2::Materialize => match movement {
            RuntimeCompleteSetMoveV2::Mint => (2, 3),
            RuntimeCompleteSetMoveV2::Merge => (3, 2),
            RuntimeCompleteSetMoveV2::None => (0, 0),
        },
    };
    CustodyGeometryV3 {
        source,
        destination,
    }
}

fn validate_environment(
    action: RuntimeSettlementActionV2,
    custody_active: bool,
    environment: GeneralHotEnvironmentV3,
) -> Result<()> {
    for identity in [
        environment.parent_request_digest,
        environment.release_set,
        environment.market,
        environment.product_record_digest,
        environment.semantic_basis_id,
        environment.linked_basis_record_digest,
        environment.realm,
        environment.trading_program,
        environment.settlement_position_owner,
        environment.rent_credit,
        environment.rent_program,
    ] {
        if identity == [0; 32] {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    if environment.claims_market_revision == u64::MAX
        || environment.owner_position_revision == u64::MAX
        || environment.settlement_position_revision == u64::MAX
        || environment.position_rent_principal == 0
        || environment.admission_rent_principal == 0
        || environment.observed_position_lamports < environment.position_rent_principal
        || environment.observed_admission_lamports < environment.admission_rent_principal
        || environment.payer != [0; 32]
        || environment.rent_refund != [0; 32]
    {
        return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
    }
    if custody_active {
        for identity in [
            environment.custody_source,
            environment.custody_destination,
            environment.mint,
            environment.token_program,
        ] {
            if identity == [0; 32] {
                return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
            }
        }
        if environment.custody_source == environment.custody_destination
            || environment.custody_expected_revision == u64::MAX
        {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
        let source_external = action == RuntimeSettlementActionV2::Collect;
        let destination_external = matches!(
            action,
            RuntimeSettlementActionV2::Distribute | RuntimeSettlementActionV2::Close
        );
        let source_shape = if source_external {
            environment.source_vault_context == [0; 32]
                && environment.custody_source_owner != [0; 32]
        } else {
            environment.source_vault_context != [0; 32]
                && environment.custody_source_owner == [0; 32]
        };
        let destination_shape = if destination_external {
            environment.destination_vault_context == [0; 32]
                && environment.custody_destination_owner != [0; 32]
        } else {
            environment.destination_vault_context != [0; 32]
                && environment.custody_destination_owner == [0; 32]
        };
        if !source_shape || !destination_shape {
            return Err(GeneralHotCandidateErrorV3::InvalidCoordinate);
        }
    }
    Ok(())
}

fn action_tag(action: RuntimeSettlementActionV2) -> u64 {
    match action {
        RuntimeSettlementActionV2::Collect => 1,
        RuntimeSettlementActionV2::Materialize => 2,
        RuntimeSettlementActionV2::Distribute => 3,
        RuntimeSettlementActionV2::Close => 4,
    }
}

fn move_tag(value: RuntimeCompleteSetMoveV2) -> u64 {
    match value {
        RuntimeCompleteSetMoveV2::None => 0,
        RuntimeCompleteSetMoveV2::Mint => 1,
        RuntimeCompleteSetMoveV2::Merge => 2,
    }
}

fn write_scalar(output: &mut [u8], coordinate: u32, value: u64) -> Result<()> {
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    put(output, offset, &value.to_le_bytes())
}

fn read_scalar(input: &[u8], coordinate: u32) -> Result<u64> {
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let bytes: [u8; 8] = input
        .get(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_identity(
    output: &mut [u8],
    scalar_count: u32,
    coordinate: u32,
    value: [u8; 32],
) -> Result<()> {
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let identity_offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(32)
        .and_then(|offset| scalar_bytes.checked_add(offset))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    put(output, identity_offset, &value)
}

fn read_identity(input: &[u8], scalar_count: u32, coordinate: u32) -> Result<[u8; 32]> {
    let scalar_bytes = usize::try_from(scalar_count)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(8)
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    let offset = usize::try_from(coordinate)
        .map_err(|_| GeneralHotCandidateErrorV3::ArithmeticOverflow)?
        .checked_mul(32)
        .and_then(|value| scalar_bytes.checked_add(value))
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    input
        .get(
            offset
                ..offset
                    .checked_add(32)
                    .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?,
        )
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .try_into()
        .map_err(|_| GeneralHotCandidateErrorV3::InvalidCapacity)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(GeneralHotCandidateErrorV3::ArithmeticOverflow)?;
    output
        .get_mut(offset..end)
        .ok_or(GeneralHotCandidateErrorV3::InvalidCapacity)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use super::*;
    use crate::runtime_settlement::RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2;

    fn put_test(output: &mut [u8], offset: usize, value: &[u8]) {
        output[offset..offset + value.len()].copy_from_slice(value);
    }

    fn materialize_plan(outcome_count: u32) -> Vec<u8> {
        let mut output = vec![
            0_u8;
            RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
                + usize::try_from(outcome_count).expect("test count") * 8
        ];
        put_test(&mut output, 0, b"DCGFXP02");
        put_test(&mut output, 8, &2_u16.to_le_bytes());
        output[10] = RuntimeSettlementActionV2::Materialize as u8;
        output[11] = 1;
        put_test(&mut output, 12, &outcome_count.to_le_bytes());
        output[16] = 0b11;
        put_test(&mut output, 24, &7_u64.to_le_bytes());
        put_test(&mut output, 40, &[0x31; 32]);
        put_test(&mut output, 168, &3_u64.to_le_bytes());
        put_test(&mut output, 176, &3_u64.to_le_bytes());
        for item in 0..outcome_count {
            let offset = RUNTIME_SETTLEMENT_EFFECT_HEADER_BYTES_V2
                + usize::try_from(item).expect("item") * 8;
            put_test(&mut output, offset, &3_u64.to_le_bytes());
        }
        RuntimeSettlementEffectPlanV2::decode(&output).expect("canonical plan");
        output
    }

    fn environment() -> GeneralHotEnvironmentV3 {
        GeneralHotEnvironmentV3 {
            parent_request_digest: [1; 32],
            release_set: [2; 32],
            market: [3; 32],
            product_record_digest: [4; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            realm: [7; 32],
            trading_program: [8; 32],
            generation: 9,
            page_index: 0,
            execution_index: 0,
            claims_market_revision: 10,
            owner_position_revision: 11,
            settlement_position_revision: 0,
            settlement_position_present: false,
            close_settlement_position: false,
            settlement_position_owner: [12; 32],
            rent_credit: [13; 32],
            rent_program: [14; 32],
            observed_position_lamports: 20,
            observed_admission_lamports: 21,
            position_rent_principal: 18,
            admission_rent_principal: 19,
            custody_source: [15; 32],
            custody_destination: [16; 32],
            custody_source_owner: [0; 32],
            custody_destination_owner: [0; 32],
            source_vault_context: [17; 32],
            destination_vault_context: [18; 32],
            mint: [19; 32],
            token_program: [20; 32],
            payer: [0; 32],
            rent_refund: [0; 32],
            custody_expected_revision: 22,
            transfer_index: 1,
        }
    }

    fn authenticated_input(outcome_count: u32, environment: GeneralHotEnvironmentV3) -> Vec<u8> {
        let len = general_hot_candidate_bank_len_v3(outcome_count).expect("bank length");
        let mut input = vec![0x7a; len];
        write_scalar(&mut input, scalar::OUTCOME_COUNT, u64::from(outcome_count))
            .expect("tail witness");
        write_identity(
            &mut input,
            general_hot_scalar_count_v3(outcome_count).expect("scalar count"),
            identity::PARENT_REQUEST_DIGEST,
            environment.parent_request_digest,
        )
        .expect("parent witness");
        input
    }

    #[test]
    fn runtime_width_one_and_two_fifty_eight_emit_complete_child_facts() {
        for outcome_count in [1_u32, 258] {
            let environment = environment();
            let input = authenticated_input(outcome_count, environment);
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            let accepted = project_general_hot_candidate_v3(
                &materialize_plan(outcome_count),
                outcome_count,
                environment,
                &input,
                &mut scratch,
                &mut output,
            )
            .expect("complete candidate");
            assert!(matches!(accepted, ExecutionCandidateV2::Accepted(_)));
            assert_eq!(read_scalar(&output, scalar::CLAIMS_ADMIT_ACTIVE), Ok(1));
            assert_eq!(read_scalar(&output, scalar::CLAIMS_POSITION_COUNT), Ok(1));
            assert_eq!(
                read_scalar(&output, scalar::CLAIMS_ROW_COUNT),
                Ok(u64::from(outcome_count))
            );
            let last = GENERAL_HOT_COMMON_SCALARS_V3
                + (outcome_count - 1) * GENERAL_HOT_ITEM_SCALAR_STRIDE_V3;
            assert_eq!(read_scalar(&output, last), Ok(u64::from(outcome_count - 1)));
            assert_eq!(read_scalar(&output, last + 1), Ok(3));
            assert_eq!(
                read_identity(
                    &output,
                    general_hot_scalar_count_v3(outcome_count).expect("scalars"),
                    identity::POSITION_ZERO_OWNER,
                ),
                Ok(environment.settlement_position_owner)
            );
        }
    }

    #[test]
    fn substituted_product_or_parent_witness_preserves_output() {
        let outcome_count = 258;
        let environment = environment();
        let plan = materialize_plan(outcome_count);
        let canonical = authenticated_input(outcome_count, environment);
        let mut hostile_tail = canonical.clone();
        write_scalar(&mut hostile_tail, scalar::OUTCOME_COUNT, 257).expect("hostile tail");
        let mut hostile_parent = canonical;
        write_identity(
            &mut hostile_parent,
            general_hot_scalar_count_v3(outcome_count).expect("scalars"),
            identity::PARENT_REQUEST_DIGEST,
            [0x99; 32],
        )
        .expect("hostile parent");
        for input in [hostile_tail, hostile_parent] {
            let mut scratch = vec![0_u8; input.len()];
            let mut output = vec![0x55_u8; input.len()];
            let before = output.clone();
            assert!(
                project_general_hot_candidate_v3(
                    &plan,
                    outcome_count,
                    environment,
                    &input,
                    &mut scratch,
                    &mut output,
                )
                .is_err()
            );
            assert_eq!(output, before);
        }
    }
}
