#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact Fractional economic lowering into the family-neutral Claims waist.
//!
//! The Fractional request remains the only family request. This crate emits
//! the existing [`dclutch_claims_svm::signed_delta_v3::SignedDeltaPlanV3`]
//! packet, uses the canonical Claims receipt commitment, and introduces no
//! child request, receipt, Position layout, or payout authority. Callers own
//! all runtime-width scratch and candidate state buffers.

mod exposure_v2;

use dclutch_claims_svm::{
    CallerRole,
    liability_basis_state_v2::{
        LiabilityBasisMarketLayoutV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionLayoutV2,
        LiabilityBasisPositionViewV2,
    },
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SIGNED_DELTA_BYTES_V3,
        SIGNED_DELTA_PLAN_HEADER_BYTES_V3, SIGNED_DELTA_POSITION_BYTES_V3,
        SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3, SIGNED_DELTA_ROW_BYTES_V3,
        SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3, SignedDeltaPlanInputV3, SignedDeltaPlanV3,
        SignedDeltaPositionV3, SignedDeltaReceiptCommitmentV3, SignedDeltaReceiptV3, SignedDeltaV3,
        ValidatedSignedDeltaConstructionV3, plan_bytes,
    },
};
use dclutch_fractional_claim_contract::{
    FractionalActionV1, FractionalFamilyRequestV1, NO_TERMINAL_OUTCOME_V1,
};
use sha2::{Digest, Sha256};

pub use exposure_v2::{
    FractionalExposureSignedDeltaInputV2, FractionalExposureSignedDeltaShapeV2,
    PreparedFractionalExposureSignedDeltaV2, fractional_exposure_signed_delta_shape_v2,
    prepare_fractional_exposure_signed_delta_v2,
    validate_fractional_exposure_signed_delta_postcondition_v2,
};

const CLAIM_ATOM_BYTES: usize = 8;

/// Stable refusal from Fractional-to-Claims physical lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// This Fractional action has no native Claims mutation.
    NoClaimsMutation,
    /// A required identity was zero, aliased, or disagreed with the request.
    IdentityMismatch,
    /// Canonical Claims aggregate or Position state refused hostile decoding.
    ClaimsState,
    /// Runtime width, Position count, or caller scratch width differed.
    WidthMismatch,
    /// Optimistic Fractional or Claims revisions were stale or could not advance.
    RevisionMismatch,
    /// Fractional action economics differed from the exact Claims pre-state.
    EconomicMismatch,
    /// A balance, supply, count, or offset overflowed exact arithmetic.
    Arithmetic,
    /// Canonical SignedDeltaV3 construction or decoding refused.
    SignedDelta,
    /// Claims receipt or exact returned post-resource state differed.
    ReceiptMismatch,
}

/// Result alias for Fractional Claims lowering.
pub type Result<T> = core::result::Result<T, Error>;

/// Exact chain-derived inputs for one Fractional native-Claims mutation.
#[derive(Clone, Copy, Debug)]
pub struct FractionalSignedDeltaInputV1<'a> {
    /// Existing canonical Fractional family request.
    pub request: FractionalFamilyRequestV1,
    /// Product semantic identity authenticated from the same finalized graph.
    pub semantic_product_id: [u8; 32],
    /// Canonical Claims aggregate account identity.
    pub market_account: [u8; 32],
    /// Exact canonical Claims aggregate pre-state bytes.
    pub market_bytes: &'a [u8],
    /// Exact finalized linked-basis raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Current Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// Canonical Fractional root identity, which solely owns reserve claims.
    pub reserve_owner: [u8; 32],
    /// Exact reserve Position pre-state bytes.
    pub reserve_position_bytes: &'a [u8],
    /// Exact actor Position pre-state for wrap/whole unwrap; absent otherwise.
    pub actor_position_bytes: Option<&'a [u8]>,
    /// Whole native claims transferred or terminally debited.
    pub native_claims: u64,
    /// Exact categorical collateral payout; nonzero only for winning redeem.
    pub collateral_atoms: u64,
    /// Kernel-derived selected reserve balance after wrap/unwrap/redeem.
    pub expected_post_reserve_native_claims: Option<u64>,
    /// Outcome-ordered zero-payout native burns for retirement.
    pub retirement_native_burns: &'a [u64],
    /// Required Fractional root revision after the enclosing atomic action.
    pub post_fractional_revision: u64,
}

/// Exact runtime buffer geometry for one native Claims mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSignedDeltaShapeV1 {
    claim_count: u32,
    position_count: u32,
    position_delta_count: u32,
    packet_bytes: usize,
}

impl FractionalSignedDeltaShapeV1 {
    /// Runtime Product/Claims width.
    pub const fn claim_count(self) -> u32 {
        self.claim_count
    }

    /// Unique canonical Position-table width.
    pub const fn position_count(self) -> u32 {
        self.position_count
    }

    /// Exact number of nonzero Position rows.
    pub const fn position_delta_count(self) -> u32 {
        self.position_delta_count
    }

    /// Exact canonical SignedDeltaV3 packet width.
    pub const fn packet_bytes(self) -> usize {
        self.packet_bytes
    }
}

/// Successful exact lowering and expected Fractional economic postcondition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSignedDeltaLoweringV1 {
    action: FractionalActionV1,
    request_digest: [u8; 32],
    packet_digest: [u8; 32],
    table_digest: [u8; 32],
    post_resource_digest: [u8; 32],
    claims_program: [u8; 32],
    pre_fractional_revision: u64,
    post_fractional_revision: u64,
    pre_market_revision: u64,
    post_market_revision: u64,
    native_claims: u64,
    collateral_atoms: u64,
    shape: FractionalSignedDeltaShapeV1,
}

/// Canonical Fractional economic plan prepared for a physical Claims CPI.
///
/// This allocation-bounded form commits the exact existing Fractional request
/// and canonical SignedDeltaV3 packet without materializing duplicate complete
/// post-state candidates in the adapter heap. Claims remains the sole writer;
/// its receipt is joined to the actual returned resources afterward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedFractionalSignedDeltaV1 {
    release_set: [u8; 32],
    market: [u8; 32],
    product_record_digest: [u8; 32],
    semantic_basis_id: [u8; 32],
    linked_basis_record_digest: [u8; 32],
    positions: CanonicalPositions,
    action: FractionalActionV1,
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    pre_fractional_revision: u64,
    post_fractional_revision: u64,
    pre_market_revision: u64,
    post_market_revision: u64,
    native_claims: u64,
    collateral_atoms: u64,
    shape: FractionalSignedDeltaShapeV1,
}

impl PreparedFractionalSignedDeltaV1 {
    /// Existing Fractional action whose native effect was prepared.
    pub const fn action(self) -> FractionalActionV1 {
        self.action
    }

    /// SHA-256 of the existing exact Fractional request bytes.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }

    /// Registry-selected Claims program required to produce the receipt.
    pub const fn claims_program(self) -> [u8; 32] {
        self.claims_program
    }

    /// Fractional optimistic pre-revision bound by the request.
    pub const fn pre_fractional_revision(self) -> u64 {
        self.pre_fractional_revision
    }

    /// Fractional revision required after the enclosing atomic action.
    pub const fn post_fractional_revision(self) -> u64 {
        self.post_fractional_revision
    }

    /// Claims aggregate optimistic pre-revision.
    pub const fn pre_market_revision(self) -> u64 {
        self.pre_market_revision
    }

    /// Claims aggregate revision required after child success.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }

    /// Whole native claims moved or terminally debited.
    pub const fn native_claims(self) -> u64 {
        self.native_claims
    }

    /// Exact categorical collateral owed by the enclosing action.
    pub const fn collateral_atoms(self) -> u64 {
        self.collateral_atoms
    }

    /// Exact runtime packet and table geometry.
    pub const fn shape(self) -> FractionalSignedDeltaShapeV1 {
        self.shape
    }

    /// Current release set authenticated from Claims state.
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }

    /// Logical Core Market authenticated from Claims state.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Borrow the exact canonical Position, aggregate, and row table preimages.
    ///
    /// This is the small physical-adapter hashing boundary. The prepared value
    /// owns the authenticated runtime geometry, while the adapter hashes the
    /// actual packet bytes it will pass to Claims using its native SHA-256
    /// facility. The packet is still hostile-decoded independently by Claims.
    pub fn table_bytes(self, packet: &[u8]) -> Result<(&[u8], &[u8], &[u8])> {
        if packet.len() != self.shape.packet_bytes {
            return Err(Error::WidthMismatch);
        }
        let position_bytes = usize::try_from(self.shape.position_count)
            .ok()
            .and_then(|count| count.checked_mul(SIGNED_DELTA_POSITION_BYTES_V3))
            .ok_or(Error::Arithmetic)?;
        let aggregate_bytes = usize::try_from(self.shape.claim_count)
            .ok()
            .and_then(|count| count.checked_mul(SIGNED_DELTA_BYTES_V3))
            .ok_or(Error::Arithmetic)?;
        let row_bytes = usize::try_from(self.shape.position_delta_count)
            .ok()
            .and_then(|count| count.checked_mul(SIGNED_DELTA_ROW_BYTES_V3))
            .ok_or(Error::Arithmetic)?;
        let positions_start = SIGNED_DELTA_PLAN_HEADER_BYTES_V3;
        let aggregates_start = positions_start
            .checked_add(position_bytes)
            .ok_or(Error::Arithmetic)?;
        let rows_start = aggregates_start
            .checked_add(aggregate_bytes)
            .ok_or(Error::Arithmetic)?;
        let rows_end = rows_start.checked_add(row_bytes).ok_or(Error::Arithmetic)?;
        if rows_end != packet.len() {
            return Err(Error::WidthMismatch);
        }
        Ok((
            packet
                .get(positions_start..aggregates_start)
                .ok_or(Error::WidthMismatch)?,
            packet
                .get(aggregates_start..rows_start)
                .ok_or(Error::WidthMismatch)?,
            packet
                .get(rows_start..rows_end)
                .ok_or(Error::WidthMismatch)?,
        ))
    }
}

impl FractionalSignedDeltaLoweringV1 {
    /// Existing Fractional action whose economic effect was lowered.
    pub const fn action(self) -> FractionalActionV1 {
        self.action
    }

    /// SHA-256 of the exact existing Fractional request bytes.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }

    /// SHA-256 of the complete canonical SignedDelta packet.
    pub const fn packet_digest(self) -> [u8; 32] {
        self.packet_digest
    }

    /// Digest of the exact ordered SignedDelta tables.
    pub const fn table_digest(self) -> [u8; 32] {
        self.table_digest
    }

    /// Digest of the exact post Market and canonical ordered post Positions.
    pub const fn post_resource_digest(self) -> [u8; 32] {
        self.post_resource_digest
    }

    /// Registry-selected Claims program expected to produce the receipt.
    pub const fn claims_program(self) -> [u8; 32] {
        self.claims_program
    }

    /// Fractional root optimistic pre-revision from the family request.
    pub const fn pre_fractional_revision(self) -> u64 {
        self.pre_fractional_revision
    }

    /// Fractional root revision required after atomic child success.
    pub const fn post_fractional_revision(self) -> u64 {
        self.post_fractional_revision
    }

    /// Claims aggregate optimistic pre-revision.
    pub const fn pre_market_revision(self) -> u64 {
        self.pre_market_revision
    }

    /// Claims aggregate revision required from the canonical receipt.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }

    /// Whole native claims moved or terminally debited.
    pub const fn native_claims(self) -> u64 {
        self.native_claims
    }

    /// Exact categorical collateral owed by the enclosing economic action.
    pub const fn collateral_atoms(self) -> u64 {
        self.collateral_atoms
    }

    /// Exact runtime buffer geometry.
    pub const fn shape(self) -> FractionalSignedDeltaShapeV1 {
        self.shape
    }
}

/// Determine exact scratch and packet widths from authenticated state and action.
pub fn fractional_signed_delta_shape_v1(
    input: FractionalSignedDeltaInputV1<'_>,
) -> Result<FractionalSignedDeltaShapeV1> {
    let market = validate_common(input)?;
    let position_delta_count = match input.request.action() {
        FractionalActionV1::Wrap | FractionalActionV1::WholeUnwrap => 2,
        FractionalActionV1::WinningRedeem => 1,
        FractionalActionV1::ZeroSupplyRetire => {
            let count = input
                .retirement_native_burns
                .iter()
                .filter(|quantity| **quantity != 0)
                .count();
            u32::try_from(count).map_err(|_| Error::Arithmetic)?
        }
        FractionalActionV1::Transfer
        | FractionalActionV1::LosingZeroBurn
        | FractionalActionV1::Terminalize => return Err(Error::NoClaimsMutation),
    };
    if position_delta_count == 0 {
        return Err(Error::NoClaimsMutation);
    }
    let position_count = match input.request.action() {
        FractionalActionV1::Wrap | FractionalActionV1::WholeUnwrap => 2,
        FractionalActionV1::WinningRedeem | FractionalActionV1::ZeroSupplyRetire => 1,
        _ => return Err(Error::NoClaimsMutation),
    };
    Ok(FractionalSignedDeltaShapeV1 {
        claim_count: market.claim_count,
        position_count,
        position_delta_count,
        packet_bytes: plan_bytes(market.claim_count, position_count, position_delta_count)
            .map_err(|_| Error::SignedDelta)?,
    })
}

/// Prepare canonical SignedDeltaV3 bytes without duplicating complete
/// post-state candidate buffers in a physical adapter.
pub fn prepare_fractional_signed_delta_v1(
    input: FractionalSignedDeltaInputV1<'_>,
    aggregate_scratch: &mut [SignedDeltaV3],
    position_delta_scratch: &mut [PositionDeltaV3],
    packet_output: &mut [u8],
) -> Result<PreparedFractionalSignedDeltaV1> {
    let market = validate_common(input)?;
    let shape = fractional_signed_delta_shape_v1(input)?;
    if aggregate_scratch.len()
        != usize::try_from(shape.claim_count).map_err(|_| Error::Arithmetic)?
        || position_delta_scratch.len()
            != usize::try_from(shape.position_delta_count).map_err(|_| Error::Arithmetic)?
        || packet_output.len() != shape.packet_bytes
    {
        return Err(Error::WidthMismatch);
    }
    let reserve = decode_position(
        input.reserve_position_bytes,
        input.market_account,
        input.reserve_owner,
        market.basis_id,
        market.claim_count,
    )?;
    let actor = match input.actor_position_bytes {
        Some(bytes) => Some(decode_position(
            bytes,
            input.market_account,
            input.request.input().owner,
            market.basis_id,
            market.claim_count,
        )?),
        None => None,
    };
    let neutral =
        SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).map_err(|_| Error::SignedDelta)?;
    aggregate_scratch.fill(neutral);
    let (positions, reserve_index, actor_index) = canonical_positions(input, reserve, actor)?;
    validate_and_fill_effects(
        input,
        market,
        reserve,
        actor,
        reserve_index,
        actor_index,
        aggregate_scratch,
        position_delta_scratch,
    )?;
    let request_digest = digest(&input.request.to_bytes());
    let header = SignedDeltaPlanInputV3 {
        caller_role: CallerRole::Trading,
        release_set: market.release_set,
        market: market.logical_market,
        request_id: request_digest,
        product_record_digest: input.request.input().product_record,
        semantic_basis_id: market.basis_id,
        linked_basis_record_digest: input.linked_basis_record_digest,
        expected_market_revision: market.revision,
        claim_count: market.claim_count,
    };
    let one = core::slice::from_ref(&positions.first);
    let two = positions.second.map(|second| [positions.first, second]);
    let position_table = two.as_ref().map_or(one, <[_; 2]>::as_slice);
    let construction = ValidatedSignedDeltaConstructionV3::new(
        header,
        position_table,
        aggregate_scratch,
        position_delta_scratch,
    )
    .map_err(|_| Error::SignedDelta)?;
    construction
        .encode_plan_into(packet_output)
        .map_err(|_| Error::SignedDelta)?;
    Ok(PreparedFractionalSignedDeltaV1 {
        release_set: header.release_set,
        market: header.market,
        product_record_digest: header.product_record_digest,
        semantic_basis_id: header.semantic_basis_id,
        linked_basis_record_digest: header.linked_basis_record_digest,
        positions,
        action: input.request.action(),
        request_digest,
        claims_program: input.claims_program,
        pre_fractional_revision: input.request.input().expected_revision,
        post_fractional_revision: input.post_fractional_revision,
        pre_market_revision: market.revision,
        post_market_revision: market.revision.checked_add(1).ok_or(Error::Arithmetic)?,
        native_claims: input.native_claims,
        collateral_atoms: input.collateral_atoms,
        shape,
    })
}

/// Join a prepared Fractional plan to the sole Claims receipt and exact
/// returned post-resource bytes.
pub fn validate_prepared_fractional_signed_delta_postcondition_v1(
    expected: PreparedFractionalSignedDeltaV1,
    packet_digest: [u8; 32],
    table_digest: [u8; 32],
    post_resource_digest: [u8; 32],
    receipt_bytes: &[u8],
    post_market: &[u8],
    post_positions: &[&[u8]],
) -> Result<()> {
    if post_positions.len()
        != usize::try_from(expected.shape.position_count).map_err(|_| Error::Arithmetic)?
    {
        return Err(Error::ReceiptMismatch);
    }
    let market =
        LiabilityBasisMarketViewV2::decode(post_market).map_err(|_| Error::ReceiptMismatch)?;
    if market.revision != expected.post_market_revision
        || market.logical_market != expected.market
        || market.release_set != expected.release_set
        || market.basis_id != expected.semantic_basis_id
        || market.claim_count != expected.shape.claim_count
    {
        return Err(Error::ReceiptMismatch);
    }
    let mut index = 0_u32;
    while index < expected.shape.position_count {
        let table = expected.positions.position(index)?;
        let bytes = *post_positions
            .get(usize::try_from(index).map_err(|_| Error::Arithmetic)?)
            .ok_or(Error::ReceiptMismatch)?;
        let position =
            LiabilityBasisPositionViewV2::decode(bytes).map_err(|_| Error::ReceiptMismatch)?;
        if position.owner != table.owner()
            || position.revision
                != table
                    .expected_revision()
                    .checked_add(1)
                    .ok_or(Error::Arithmetic)?
            || position.basis_id != market.basis_id
            || position.claim_count != expected.shape.claim_count
        {
            return Err(Error::ReceiptMismatch);
        }
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let commitment = SignedDeltaReceiptCommitmentV3::new(
        packet_digest,
        table_digest,
        expected.claims_program,
        post_resource_digest,
    )
    .map_err(|_| Error::ReceiptMismatch)?;
    let receipt =
        SignedDeltaReceiptV3::decode(receipt_bytes).map_err(|_| Error::ReceiptMismatch)?;
    if receipt.caller_role() != CallerRole::Trading
        || receipt.release_set() != expected.release_set
        || receipt.market() != expected.market
        || receipt.request_id() != expected.request_digest
        || receipt.product_record_digest() != expected.product_record_digest
        || receipt.semantic_basis_id() != expected.semantic_basis_id
        || receipt.linked_basis_record_digest() != expected.linked_basis_record_digest
        || receipt.pre_market_revision() != expected.pre_market_revision
        || receipt.post_market_revision() != expected.post_market_revision
        || receipt.claim_count() != expected.shape.claim_count
        || receipt.position_count() != expected.shape.position_count
        || receipt.position_delta_count() != expected.shape.position_delta_count
    {
        return Err(Error::ReceiptMismatch);
    }
    receipt
        .validate_physical_commitment(commitment)
        .map_err(|_| Error::ReceiptMismatch)
}

/// Lower one exact Fractional native effect into canonical SignedDeltaV3.
///
/// `aggregate_scratch`, `position_delta_scratch`, `packet_scratch`, and the
/// post-state candidate buffers are temporary and may change on refusal. The
/// canonical packet output is written only after hostile state, identity,
/// width, revision, economic, and commitment checks succeed.
#[allow(clippy::too_many_arguments)]
pub fn lower_fractional_signed_delta_v1(
    input: FractionalSignedDeltaInputV1<'_>,
    aggregate_scratch: &mut [SignedDeltaV3],
    position_delta_scratch: &mut [PositionDeltaV3],
    packet_scratch: &mut [u8],
    packet_output: &mut [u8],
    post_market: &mut [u8],
    post_positions: &mut [&mut [u8]],
) -> Result<FractionalSignedDeltaLoweringV1> {
    let market = validate_common(input)?;
    let shape = fractional_signed_delta_shape_v1(input)?;
    validate_buffers(
        input,
        shape,
        aggregate_scratch,
        position_delta_scratch,
        packet_scratch,
        packet_output,
        post_market,
        post_positions,
    )?;
    let reserve = decode_position(
        input.reserve_position_bytes,
        input.market_account,
        input.reserve_owner,
        market.basis_id,
        market.claim_count,
    )?;
    let actor = match input.actor_position_bytes {
        Some(bytes) => Some(decode_position(
            bytes,
            input.market_account,
            input.request.input().owner,
            market.basis_id,
            market.claim_count,
        )?),
        None => None,
    };
    let neutral =
        SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).map_err(|_| Error::SignedDelta)?;
    aggregate_scratch.fill(neutral);
    let (positions, reserve_index, actor_index) = canonical_positions(input, reserve, actor)?;
    validate_and_fill_effects(
        input,
        market,
        reserve,
        actor,
        reserve_index,
        actor_index,
        aggregate_scratch,
        position_delta_scratch,
    )?;
    let request_bytes = input.request.to_bytes();
    let request_digest = digest(&request_bytes);
    let header = SignedDeltaPlanInputV3 {
        caller_role: CallerRole::Trading,
        release_set: market.release_set,
        market: market.logical_market,
        request_id: request_digest,
        product_record_digest: input.request.input().product_record,
        semantic_basis_id: market.basis_id,
        linked_basis_record_digest: input.linked_basis_record_digest,
        expected_market_revision: market.revision,
        claim_count: market.claim_count,
    };
    match positions.second {
        None => SignedDeltaPlanV3::encode_into(
            header,
            core::slice::from_ref(&positions.first),
            aggregate_scratch,
            position_delta_scratch,
            packet_scratch,
        ),
        Some(second) => SignedDeltaPlanV3::encode_into(
            header,
            &[positions.first, second],
            aggregate_scratch,
            position_delta_scratch,
            packet_scratch,
        ),
    }
    .map_err(|_| Error::SignedDelta)?;
    let plan = SignedDeltaPlanV3::decode(packet_scratch).map_err(|_| Error::SignedDelta)?;

    write_candidates(
        input,
        market,
        reserve,
        actor,
        reserve_index,
        actor_index,
        aggregate_scratch,
        position_delta_scratch,
        post_market,
        post_positions,
    )?;
    let packet_digest = digest(packet_scratch);
    let (position_table, aggregate_table, delta_table) = plan.table_bytes();
    let table_digest = digestv(&[
        SIGNED_DELTA_TABLE_DIGEST_DOMAIN_V3,
        position_table,
        aggregate_table,
        delta_table,
    ]);
    let post_resource_digest = post_resource_digest(post_market, post_positions);
    SignedDeltaReceiptCommitmentV3::new(
        packet_digest,
        table_digest,
        input.claims_program,
        post_resource_digest,
    )
    .map_err(|_| Error::SignedDelta)?;
    packet_output.copy_from_slice(packet_scratch);
    Ok(FractionalSignedDeltaLoweringV1 {
        action: input.request.action(),
        request_digest,
        packet_digest,
        table_digest,
        post_resource_digest,
        claims_program: input.claims_program,
        pre_fractional_revision: input.request.input().expected_revision,
        post_fractional_revision: input.post_fractional_revision,
        pre_market_revision: market.revision,
        post_market_revision: market.revision.checked_add(1).ok_or(Error::Arithmetic)?,
        native_claims: input.native_claims,
        collateral_atoms: input.collateral_atoms,
        shape,
    })
}

/// Validate the sole Claims receipt and exact returned post-resource state.
///
/// Success is the Fractional economic postcondition itself; there is no
/// Fractional receipt type. The caller must still commit its root and Token or
/// Custody candidates only after this check succeeds.
pub fn validate_fractional_signed_delta_postcondition_v1(
    expected: FractionalSignedDeltaLoweringV1,
    packet: &[u8],
    receipt_bytes: &[u8],
    post_market: &[u8],
    post_positions: &[&[u8]],
) -> Result<()> {
    let plan = SignedDeltaPlanV3::decode(packet).map_err(|_| Error::ReceiptMismatch)?;
    if digest(packet) != expected.packet_digest
        || plan.request_id() != expected.request_digest
        || plan.expected_market_revision() != expected.pre_market_revision
        || plan.position_count() != expected.shape.position_count
        || post_positions.len()
            != usize::try_from(expected.shape.position_count).map_err(|_| Error::Arithmetic)?
    {
        return Err(Error::ReceiptMismatch);
    }
    let market =
        LiabilityBasisMarketViewV2::decode(post_market).map_err(|_| Error::ReceiptMismatch)?;
    if market.revision != expected.post_market_revision
        || market.logical_market != plan.market()
        || market.release_set != plan.release_set()
        || market.basis_id != plan.semantic_basis_id()
        || market.claim_count != plan.claim_count()
    {
        return Err(Error::ReceiptMismatch);
    }
    let mut index = 0_u32;
    while index < plan.position_count() {
        let table = plan.position(index).map_err(|_| Error::ReceiptMismatch)?;
        let bytes = *post_positions
            .get(usize::try_from(index).map_err(|_| Error::Arithmetic)?)
            .ok_or(Error::ReceiptMismatch)?;
        let position =
            LiabilityBasisPositionViewV2::decode(bytes).map_err(|_| Error::ReceiptMismatch)?;
        if position.owner != table.owner()
            || position.revision
                != table
                    .expected_revision()
                    .checked_add(1)
                    .ok_or(Error::Arithmetic)?
            || position.basis_id != plan.semantic_basis_id()
            || position.claim_count != plan.claim_count()
        {
            return Err(Error::ReceiptMismatch);
        }
        index = index.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    let resource_digest = post_resource_digest(post_market, post_positions);
    if resource_digest != expected.post_resource_digest {
        return Err(Error::ReceiptMismatch);
    }
    let receipt =
        SignedDeltaReceiptV3::decode(receipt_bytes).map_err(|_| Error::ReceiptMismatch)?;
    let commitment = SignedDeltaReceiptCommitmentV3::new(
        expected.packet_digest,
        expected.table_digest,
        expected.claims_program,
        resource_digest,
    )
    .map_err(|_| Error::ReceiptMismatch)?;
    receipt
        .validate_commitment(plan, commitment)
        .map_err(|_| Error::ReceiptMismatch)
}

fn validate_common(input: FractionalSignedDeltaInputV1<'_>) -> Result<LiabilityBasisMarketViewV2> {
    let request = input.request.input();
    if [
        input.semantic_product_id,
        input.market_account,
        input.linked_basis_record_digest,
        input.claims_program,
        input.reserve_owner,
    ]
    .iter()
    .any(is_zero)
        || request.release_set == [0; 32]
        || request.market == [0; 32]
        || request.product_record == [0; 32]
        || request.expected_revision == u64::MAX
        || request.expected_revision.checked_add(1) != Some(input.post_fractional_revision)
    {
        return Err(Error::IdentityMismatch);
    }
    let market =
        LiabilityBasisMarketViewV2::decode(input.market_bytes).map_err(|_| Error::ClaimsState)?;
    if market.logical_market != request.market
        || market.release_set != request.release_set
        || market.product_instance_id != input.semantic_product_id
        || market.revision == u64::MAX
    {
        return Err(Error::IdentityMismatch);
    }
    let selected = request.outcome;
    match input.request.action() {
        FractionalActionV1::Wrap | FractionalActionV1::WholeUnwrap => {
            if request.owner == [0; 32]
                || request.owner == input.reserve_owner
                || selected >= market.claim_count
                || input.actor_position_bytes.is_none()
                || input.native_claims == 0
                || input.collateral_atoms != 0
                || input.expected_post_reserve_native_claims.is_none()
                || !input.retirement_native_burns.is_empty()
            {
                return Err(Error::EconomicMismatch);
            }
        }
        FractionalActionV1::WinningRedeem => {
            if request.owner == [0; 32]
                || selected >= market.claim_count
                || input.actor_position_bytes.is_some()
                || input.native_claims == 0
                || input.collateral_atoms != input.native_claims
                || input.expected_post_reserve_native_claims.is_none()
                || !input.retirement_native_burns.is_empty()
            {
                return Err(Error::EconomicMismatch);
            }
        }
        FractionalActionV1::ZeroSupplyRetire => {
            if request.owner != [0; 32]
                || request.outcome != NO_TERMINAL_OUTCOME_V1
                || input.actor_position_bytes.is_some()
                || input.native_claims != 0
                || input.collateral_atoms != 0
                || input.expected_post_reserve_native_claims.is_some()
                || input.retirement_native_burns.len()
                    != usize::try_from(market.claim_count).map_err(|_| Error::Arithmetic)?
            {
                return Err(Error::EconomicMismatch);
            }
        }
        FractionalActionV1::Transfer
        | FractionalActionV1::LosingZeroBurn
        | FractionalActionV1::Terminalize => return Err(Error::NoClaimsMutation),
    }
    Ok(market)
}

#[allow(clippy::too_many_arguments)]
fn validate_buffers(
    input: FractionalSignedDeltaInputV1<'_>,
    shape: FractionalSignedDeltaShapeV1,
    aggregates: &[SignedDeltaV3],
    rows: &[PositionDeltaV3],
    packet_scratch: &[u8],
    packet_output: &[u8],
    post_market: &[u8],
    post_positions: &[&mut [u8]],
) -> Result<()> {
    let position_count = usize::try_from(shape.position_count).map_err(|_| Error::Arithmetic)?;
    let row_count = usize::try_from(shape.position_delta_count).map_err(|_| Error::Arithmetic)?;
    if aggregates.len() != usize::try_from(shape.claim_count).map_err(|_| Error::Arithmetic)?
        || rows.len() != row_count
        || packet_scratch.len() != shape.packet_bytes
        || packet_output.len() != shape.packet_bytes
        || post_market.len() != input.market_bytes.len()
        || post_positions.len() != position_count
        || post_positions
            .iter()
            .any(|bytes| bytes.len() != input.reserve_position_bytes.len())
    {
        return Err(Error::WidthMismatch);
    }
    Ok(())
}

fn decode_position(
    bytes: &[u8],
    market_account: [u8; 32],
    owner: [u8; 32],
    basis: [u8; 32],
    claim_count: u32,
) -> Result<LiabilityBasisPositionViewV2> {
    let position = LiabilityBasisPositionViewV2::decode(bytes).map_err(|_| Error::ClaimsState)?;
    if position.market_account != market_account
        || position.owner != owner
        || position.basis_id != basis
        || position.claim_count != claim_count
        || position.revision == u64::MAX
    {
        return Err(Error::IdentityMismatch);
    }
    Ok(position)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CanonicalPositions {
    first: SignedDeltaPositionV3,
    second: Option<SignedDeltaPositionV3>,
}

impl CanonicalPositions {
    fn position(self, index: u32) -> Result<SignedDeltaPositionV3> {
        match index {
            0 => Ok(self.first),
            1 => self.second.ok_or(Error::ReceiptMismatch),
            _ => Err(Error::ReceiptMismatch),
        }
    }
}

fn canonical_positions(
    input: FractionalSignedDeltaInputV1<'_>,
    reserve: LiabilityBasisPositionViewV2,
    actor: Option<LiabilityBasisPositionViewV2>,
) -> Result<(CanonicalPositions, u32, Option<u32>)> {
    let reserve_entry = SignedDeltaPositionV3::new(reserve.owner, reserve.revision)
        .map_err(|_| Error::SignedDelta)?;
    let Some(actor) = actor else {
        return Ok((
            CanonicalPositions {
                first: reserve_entry,
                second: None,
            },
            0,
            None,
        ));
    };
    let actor_entry =
        SignedDeltaPositionV3::new(actor.owner, actor.revision).map_err(|_| Error::SignedDelta)?;
    if actor.owner < input.reserve_owner {
        Ok((
            CanonicalPositions {
                first: actor_entry,
                second: Some(reserve_entry),
            },
            1,
            Some(0),
        ))
    } else {
        Ok((
            CanonicalPositions {
                first: reserve_entry,
                second: Some(actor_entry),
            },
            0,
            Some(1),
        ))
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_and_fill_effects(
    input: FractionalSignedDeltaInputV1<'_>,
    market: LiabilityBasisMarketViewV2,
    reserve: LiabilityBasisPositionViewV2,
    actor: Option<LiabilityBasisPositionViewV2>,
    reserve_index: u32,
    actor_index: Option<u32>,
    aggregates: &mut [SignedDeltaV3],
    rows: &mut [PositionDeltaV3],
) -> Result<()> {
    let debit = |quantity| {
        SignedDeltaV3::new(DeltaDirectionV3::Debit, quantity).map_err(|_| Error::SignedDelta)
    };
    let credit = |quantity| {
        SignedDeltaV3::new(DeltaDirectionV3::Credit, quantity).map_err(|_| Error::SignedDelta)
    };
    match input.request.action() {
        FractionalActionV1::Wrap | FractionalActionV1::WholeUnwrap => {
            let outcome = input.request.input().outcome;
            let actor = actor.ok_or(Error::EconomicMismatch)?;
            let actor_index = actor_index.ok_or(Error::EconomicMismatch)?;
            let reserve_before = reserve
                .balance(input.reserve_position_bytes, outcome)
                .map_err(|_| Error::ClaimsState)?;
            let actor_bytes = input.actor_position_bytes.ok_or(Error::EconomicMismatch)?;
            let actor_before = actor
                .balance(actor_bytes, outcome)
                .map_err(|_| Error::ClaimsState)?;
            let reserve_after;
            let actor_after;
            let reserve_delta;
            let actor_delta;
            if input.request.action() == FractionalActionV1::Wrap {
                reserve_after = reserve_before
                    .checked_add(input.native_claims)
                    .ok_or(Error::Arithmetic)?;
                actor_after = actor_before
                    .checked_sub(input.native_claims)
                    .ok_or(Error::EconomicMismatch)?;
                reserve_delta = credit(input.native_claims)?;
                actor_delta = debit(input.native_claims)?;
            } else {
                reserve_after = reserve_before
                    .checked_sub(input.native_claims)
                    .ok_or(Error::EconomicMismatch)?;
                actor_after = actor_before
                    .checked_add(input.native_claims)
                    .ok_or(Error::Arithmetic)?;
                reserve_delta = debit(input.native_claims)?;
                actor_delta = credit(input.native_claims)?;
            }
            let expected = input
                .expected_post_reserve_native_claims
                .ok_or(Error::EconomicMismatch)?;
            if reserve_after != expected {
                return Err(Error::EconomicMismatch);
            }
            let supply = market
                .supply(input.market_bytes, outcome)
                .map_err(|_| Error::ClaimsState)?;
            if reserve_before > supply || actor_before > supply || actor_after > supply {
                return Err(Error::EconomicMismatch);
            }
            set_row(
                rows,
                0,
                actor_index,
                outcome,
                actor_delta,
                2,
                market.claim_count,
            )?;
            set_row(
                rows,
                1,
                reserve_index,
                outcome,
                reserve_delta,
                2,
                market.claim_count,
            )?;
            if actor_index > reserve_index {
                rows.swap(0, 1);
            }
        }
        FractionalActionV1::WinningRedeem => {
            let outcome = input.request.input().outcome;
            let reserve_before = reserve
                .balance(input.reserve_position_bytes, outcome)
                .map_err(|_| Error::ClaimsState)?;
            let reserve_after = reserve_before
                .checked_sub(input.native_claims)
                .ok_or(Error::EconomicMismatch)?;
            if Some(reserve_after) != input.expected_post_reserve_native_claims {
                return Err(Error::EconomicMismatch);
            }
            let supply = market
                .supply(input.market_bytes, outcome)
                .map_err(|_| Error::ClaimsState)?;
            if reserve_before > supply || supply < input.native_claims {
                return Err(Error::EconomicMismatch);
            }
            let delta = debit(input.native_claims)?;
            *aggregates
                .get_mut(usize::try_from(outcome).map_err(|_| Error::Arithmetic)?)
                .ok_or(Error::WidthMismatch)? = delta;
            set_row(
                rows,
                0,
                reserve_index,
                outcome,
                delta,
                1,
                market.claim_count,
            )?;
        }
        FractionalActionV1::ZeroSupplyRetire => {
            let mut row = 0_usize;
            for (index, quantity) in input.retirement_native_burns.iter().copied().enumerate() {
                let outcome = u32::try_from(index).map_err(|_| Error::Arithmetic)?;
                let reserve_before = reserve
                    .balance(input.reserve_position_bytes, outcome)
                    .map_err(|_| Error::ClaimsState)?;
                if reserve_before != quantity {
                    return Err(Error::EconomicMismatch);
                }
                if quantity == 0 {
                    continue;
                }
                let supply = market
                    .supply(input.market_bytes, outcome)
                    .map_err(|_| Error::ClaimsState)?;
                if supply < quantity {
                    return Err(Error::EconomicMismatch);
                }
                let delta = debit(quantity)?;
                *aggregates.get_mut(index).ok_or(Error::WidthMismatch)? = delta;
                set_row(
                    rows,
                    row,
                    reserve_index,
                    outcome,
                    delta,
                    1,
                    market.claim_count,
                )?;
                row = row.checked_add(1).ok_or(Error::Arithmetic)?;
            }
            if row != rows.len() {
                return Err(Error::WidthMismatch);
            }
        }
        _ => return Err(Error::NoClaimsMutation),
    }
    Ok(())
}

fn set_row(
    rows: &mut [PositionDeltaV3],
    index: usize,
    position_index: u32,
    outcome: u32,
    delta: SignedDeltaV3,
    position_count: u32,
    claim_count: u32,
) -> Result<()> {
    *rows.get_mut(index).ok_or(Error::WidthMismatch)? = PositionDeltaV3::new(
        PositionDeltaInputV3 {
            position_index,
            outcome,
            delta,
        },
        position_count,
        claim_count,
    )
    .map_err(|_| Error::SignedDelta)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_candidates(
    input: FractionalSignedDeltaInputV1<'_>,
    market: LiabilityBasisMarketViewV2,
    reserve: LiabilityBasisPositionViewV2,
    actor: Option<LiabilityBasisPositionViewV2>,
    reserve_index: u32,
    actor_index: Option<u32>,
    aggregates: &[SignedDeltaV3],
    rows: &[PositionDeltaV3],
    post_market: &mut [u8],
    post_positions: &mut [&mut [u8]],
) -> Result<()> {
    post_market.copy_from_slice(input.market_bytes);
    write_u64(
        post_market,
        LiabilityBasisMarketLayoutV2::REVISION,
        market.revision.checked_add(1).ok_or(Error::Arithmetic)?,
    )?;
    let mut outcome = 0_u32;
    while outcome < market.claim_count {
        // `validate_common` already hostile-decoded this exact aggregate. Do
        // not re-decode it for every runtime tail coordinate.
        let before = read_claim(
            input.market_bytes,
            LiabilityBasisMarketLayoutV2::SUPPLIES,
            outcome,
        )?;
        let after = apply(
            before,
            *aggregates
                .get(usize::try_from(outcome).map_err(|_| Error::Arithmetic)?)
                .ok_or(Error::WidthMismatch)?,
        )?;
        write_claim(
            post_market,
            LiabilityBasisMarketLayoutV2::SUPPLIES,
            outcome,
            after,
        )?;
        outcome = outcome.checked_add(1).ok_or(Error::Arithmetic)?;
    }
    copy_position_candidate(post_positions, reserve_index, input.reserve_position_bytes)?;
    if let (Some(actor), Some(actor_index), Some(actor_bytes)) =
        (actor, actor_index, input.actor_position_bytes)
    {
        let _ = actor;
        copy_position_candidate(post_positions, actor_index, actor_bytes)?;
    }
    for (index, position) in [Some((reserve_index, reserve)), actor_index.zip(actor)]
        .into_iter()
        .flatten()
    {
        let output = post_positions
            .get_mut(usize::try_from(index).map_err(|_| Error::Arithmetic)?)
            .ok_or(Error::WidthMismatch)?;
        write_u64(
            output,
            LiabilityBasisPositionLayoutV2::REVISION,
            position.revision.checked_add(1).ok_or(Error::Arithmetic)?,
        )?;
    }
    for row in rows.iter().copied() {
        let output = post_positions
            .get_mut(usize::try_from(row.position_index()).map_err(|_| Error::Arithmetic)?)
            .ok_or(Error::WidthMismatch)?;
        let before = read_claim(
            output,
            LiabilityBasisPositionLayoutV2::BALANCES,
            row.outcome(),
        )?;
        write_claim(
            output,
            LiabilityBasisPositionLayoutV2::BALANCES,
            row.outcome(),
            apply(before, row.delta())?,
        )?;
    }
    Ok(())
}

fn copy_position_candidate(outputs: &mut [&mut [u8]], index: u32, source: &[u8]) -> Result<()> {
    let output = outputs
        .get_mut(usize::try_from(index).map_err(|_| Error::Arithmetic)?)
        .ok_or(Error::WidthMismatch)?;
    if output.len() != source.len() {
        return Err(Error::WidthMismatch);
    }
    output.copy_from_slice(source);
    Ok(())
}

fn apply(value: u64, delta: SignedDeltaV3) -> Result<u64> {
    match delta.direction() {
        DeltaDirectionV3::Neutral => Ok(value),
        DeltaDirectionV3::Credit => value
            .checked_add(delta.magnitude())
            .ok_or(Error::Arithmetic),
        DeltaDirectionV3::Debit => value
            .checked_sub(delta.magnitude())
            .ok_or(Error::EconomicMismatch),
    }
}

fn read_claim(bytes: &[u8], base: usize, outcome: u32) -> Result<u64> {
    let offset = claim_offset(base, outcome)?;
    read_u64(bytes, offset)
}

fn write_claim(bytes: &mut [u8], base: usize, outcome: u32, value: u64) -> Result<()> {
    write_u64(bytes, claim_offset(base, outcome)?, value)
}

fn claim_offset(base: usize, outcome: u32) -> Result<usize> {
    usize::try_from(outcome)
        .ok()
        .and_then(|index| index.checked_mul(CLAIM_ATOM_BYTES))
        .and_then(|relative| base.checked_add(relative))
        .ok_or(Error::Arithmetic)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(CLAIM_ATOM_BYTES)
        .ok_or(Error::Arithmetic)?;
    let value: [u8; CLAIM_ATOM_BYTES] = bytes
        .get(offset..end)
        .ok_or(Error::WidthMismatch)?
        .try_into()
        .map_err(|_| Error::WidthMismatch)?;
    Ok(u64::from_le_bytes(value))
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<()> {
    let end = offset
        .checked_add(CLAIM_ATOM_BYTES)
        .ok_or(Error::Arithmetic)?;
    bytes
        .get_mut(offset..end)
        .ok_or(Error::WidthMismatch)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn post_resource_digest(market: &[u8], positions: &[impl AsRef<[u8]>]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SIGNED_DELTA_POST_RESOURCE_DIGEST_DOMAIN_V3);
    hasher.update(market);
    for position in positions {
        hasher.update(position.as_ref());
    }
    hasher.finalize().into()
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn digestv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests;
