//! K-width Fractional V2 wrap/unwrap lowering into canonical SignedDeltaV3.
//!
//! This module consumes the exposure-bound V2 request directly. It never
//! synthesizes a categorical V1 request and never projects Product width `N`
//! onto Claims width `K`.

use dclutch_claims_svm::{
    CallerRole,
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3,
        SignedDeltaPlanV3, SignedDeltaPositionV3, SignedDeltaReceiptCommitmentV3,
        SignedDeltaReceiptV3, SignedDeltaV3, plan_bytes,
    },
};
use dclutch_fractional_claim_contract::{FractionalExposureActionV2, FractionalExposureRequestV2};
use dclutch_fractional_claim_kernel::{FractionalExposureTermsV2, divide_exposure_shards_v2};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

/// Chain-derived inputs for one V2 wrap or whole unwrap.
#[derive(Clone, Copy, Debug)]
pub struct FractionalExposureSignedDeltaInputV2<'a> {
    /// Exact exposure-bound V2 request.
    pub request: FractionalExposureRequestV2,
    /// Authenticated immutable V2 terms.
    pub terms: FractionalExposureTermsV2<'a>,
    /// Semantic Product identity authenticated from Product Runtime.
    pub semantic_product_id: [u8; 32],
    /// Canonical Claims aggregate account identity.
    pub market_account: [u8; 32],
    /// Exact Claims aggregate prestate.
    pub market_bytes: &'a [u8],
    /// Registry-selected current Claims program.
    pub claims_program: [u8; 32],
    /// Fractional root owning the reserve Position.
    pub reserve_owner: [u8; 32],
    /// Exact reserve Position prestate.
    pub reserve_position_bytes: &'a [u8],
    /// Exact actor Position prestate.
    pub actor_position_bytes: &'a [u8],
}

/// Exact V2 SignedDelta shape for one two-Position transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureSignedDeltaShapeV2 {
    claim_count: u32,
    packet_bytes: usize,
}

impl FractionalExposureSignedDeltaShapeV2 {
    /// Runtime Claims width `K`.
    pub const fn claim_count(self) -> u32 {
        self.claim_count
    }

    /// Exact canonical SignedDelta packet bytes.
    pub const fn packet_bytes(self) -> usize {
        self.packet_bytes
    }
}

/// Checked V2 Claims transfer prepared for one physical Claims CPI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedFractionalExposureSignedDeltaV2 {
    action: FractionalExposureActionV2,
    release_set: [u8; 32],
    market: [u8; 32],
    product_record: [u8; 32],
    semantic_basis: [u8; 32],
    linked_basis_record: [u8; 32],
    request_digest: [u8; 32],
    claims_program: [u8; 32],
    positions: [SignedDeltaPositionV3; 2],
    pre_fractional_revision: u64,
    post_fractional_revision: u64,
    pre_market_revision: u64,
    post_market_revision: u64,
    native_claims: u64,
    coordinate: u32,
    shape: FractionalExposureSignedDeltaShapeV2,
}

impl PreparedFractionalExposureSignedDeltaV2 {
    /// Exact V2 action.
    pub const fn action(self) -> FractionalExposureActionV2 {
        self.action
    }
    /// SHA-256 of the exact 416-byte V2 request.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }
    /// Current selected Claims program.
    pub const fn claims_program(self) -> [u8; 32] {
        self.claims_program
    }
    /// Root revision before the transaction.
    pub const fn pre_fractional_revision(self) -> u64 {
        self.pre_fractional_revision
    }
    /// Root revision committed last after child success.
    pub const fn post_fractional_revision(self) -> u64 {
        self.post_fractional_revision
    }
    /// Claims aggregate revision before execution.
    pub const fn pre_market_revision(self) -> u64 {
        self.pre_market_revision
    }
    /// Claims aggregate revision after execution.
    pub const fn post_market_revision(self) -> u64 {
        self.post_market_revision
    }
    /// Exact native Claims units moved.
    pub const fn native_claims(self) -> u64 {
        self.native_claims
    }
    /// Claims representation coordinate in `[0,K)`.
    pub const fn coordinate(self) -> u32 {
        self.coordinate
    }
    /// Exact packet geometry.
    pub const fn shape(self) -> FractionalExposureSignedDeltaShapeV2 {
        self.shape
    }
    /// Borrow exact ordered SignedDelta table preimages from the packet.
    pub fn table_bytes(self, packet: &[u8]) -> Result<(&[u8], &[u8], &[u8])> {
        let plan = SignedDeltaPlanV3::decode(packet).map_err(|_| Error::SignedDelta)?;
        if packet.len() != self.shape.packet_bytes
            || plan.request_id() != self.request_digest
            || plan.claim_count() != self.shape.claim_count
            || plan.position_count() != 2
            || plan.position_delta_count() != 2
        {
            return Err(Error::WidthMismatch);
        }
        Ok(plan.table_bytes())
    }
}

/// Return the exact V2 packet shape after hostile state and identity checks.
pub fn fractional_exposure_signed_delta_shape_v2(
    input: FractionalExposureSignedDeltaInputV2<'_>,
) -> Result<FractionalExposureSignedDeltaShapeV2> {
    let market = validate_common(input)?;
    Ok(FractionalExposureSignedDeltaShapeV2 {
        claim_count: market.claim_count,
        packet_bytes: plan_bytes(market.claim_count, 2, 2).map_err(|_| Error::SignedDelta)?,
    })
}

/// Prepare the sole V2 wrap/unwrap Claims packet into caller-owned storage.
pub fn prepare_fractional_exposure_signed_delta_v2(
    input: FractionalExposureSignedDeltaInputV2<'_>,
    aggregate_scratch: &mut [SignedDeltaV3],
    row_scratch: &mut [PositionDeltaV3],
    packet_output: &mut [u8],
) -> Result<PreparedFractionalExposureSignedDeltaV2> {
    let market = validate_common(input)?;
    let shape = fractional_exposure_signed_delta_shape_v2(input)?;
    if aggregate_scratch.len()
        != usize::try_from(shape.claim_count).map_err(|_| Error::Arithmetic)?
        || row_scratch.len() != 2
        || packet_output.len() != shape.packet_bytes
    {
        return Err(Error::WidthMismatch);
    }
    let request = input.request.input();
    let reserve = decode_position(
        input.reserve_position_bytes,
        input.market_account,
        input.reserve_owner,
        market.basis_id,
        market.claim_count,
    )?;
    let actor = decode_position(
        input.actor_position_bytes,
        input.market_account,
        request.owner,
        market.basis_id,
        market.claim_count,
    )?;
    let reserve_entry = SignedDeltaPositionV3::new(reserve.owner, reserve.revision)
        .map_err(|_| Error::SignedDelta)?;
    let actor_entry =
        SignedDeltaPositionV3::new(actor.owner, actor.revision).map_err(|_| Error::SignedDelta)?;
    let (positions, reserve_index, actor_index) = if actor.owner < reserve.owner {
        ([actor_entry, reserve_entry], 1, 0)
    } else {
        ([reserve_entry, actor_entry], 0, 1)
    };
    let neutral =
        SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).map_err(|_| Error::SignedDelta)?;
    aggregate_scratch.fill(neutral);
    let coordinate = request.representation_coordinate;
    let native_claims = match input.request.action() {
        FractionalExposureActionV2::Wrap => request.quantity,
        FractionalExposureActionV2::WholeUnwrap => {
            divide_exposure_shards_v2(input.terms, coordinate, request.quantity)
                .map_err(|_| Error::EconomicMismatch)?
                .whole_claims
        }
        _ => return Err(Error::NoClaimsMutation),
    };
    let reserve_before = reserve
        .balance(input.reserve_position_bytes, coordinate)
        .map_err(|_| Error::ClaimsState)?;
    let actor_before = actor
        .balance(input.actor_position_bytes, coordinate)
        .map_err(|_| Error::ClaimsState)?;
    let supply = market
        .supply(input.market_bytes, coordinate)
        .map_err(|_| Error::ClaimsState)?;
    if reserve_before > supply || actor_before > supply {
        return Err(Error::EconomicMismatch);
    }
    let debit = SignedDeltaV3::new(DeltaDirectionV3::Debit, native_claims)
        .map_err(|_| Error::SignedDelta)?;
    let credit = SignedDeltaV3::new(DeltaDirectionV3::Credit, native_claims)
        .map_err(|_| Error::SignedDelta)?;
    let (reserve_delta, actor_delta, reserve_after, actor_after) = match input.request.action() {
        FractionalExposureActionV2::Wrap => {
            let reserve_after = reserve_before
                .checked_add(native_claims)
                .ok_or(Error::Arithmetic)?;
            let actor_after = actor_before
                .checked_sub(native_claims)
                .ok_or(Error::EconomicMismatch)?;
            (credit, debit, reserve_after, actor_after)
        }
        FractionalExposureActionV2::WholeUnwrap => {
            let reserve_after = reserve_before
                .checked_sub(native_claims)
                .ok_or(Error::EconomicMismatch)?;
            let actor_after = actor_before
                .checked_add(native_claims)
                .ok_or(Error::Arithmetic)?;
            (debit, credit, reserve_after, actor_after)
        }
        _ => return Err(Error::NoClaimsMutation),
    };
    if reserve_after > supply || actor_after > supply {
        return Err(Error::EconomicMismatch);
    }
    let mut rows = [
        PositionDeltaV3::new(
            PositionDeltaInputV3 {
                position_index: actor_index,
                outcome: coordinate,
                delta: actor_delta,
            },
            2,
            market.claim_count,
        )
        .map_err(|_| Error::SignedDelta)?,
        PositionDeltaV3::new(
            PositionDeltaInputV3 {
                position_index: reserve_index,
                outcome: coordinate,
                delta: reserve_delta,
            },
            2,
            market.claim_count,
        )
        .map_err(|_| Error::SignedDelta)?,
    ];
    if actor_index > reserve_index {
        rows.swap(0, 1);
    }
    row_scratch.copy_from_slice(&rows);
    let request_digest: [u8; 32] = Sha256::digest(
        input
            .request
            .to_bytes()
            .map_err(|_| Error::IdentityMismatch)?,
    )
    .into();
    SignedDeltaPlanV3::encode_into(
        SignedDeltaPlanInputV3 {
            caller_role: CallerRole::Trading,
            release_set: market.release_set,
            market: market.logical_market,
            request_id: request_digest,
            product_record_digest: request.product_record,
            semantic_basis_id: market.basis_id,
            linked_basis_record_digest: input.terms.product_basis(),
            expected_market_revision: market.revision,
            claim_count: market.claim_count,
        },
        &positions,
        aggregate_scratch,
        row_scratch,
        packet_output,
    )
    .map_err(|_| Error::SignedDelta)?;
    SignedDeltaPlanV3::decode(packet_output).map_err(|_| Error::SignedDelta)?;
    Ok(PreparedFractionalExposureSignedDeltaV2 {
        action: input.request.action(),
        release_set: market.release_set,
        market: market.logical_market,
        product_record: request.product_record,
        semantic_basis: market.basis_id,
        linked_basis_record: input.terms.product_basis(),
        request_digest,
        claims_program: input.claims_program,
        positions,
        pre_fractional_revision: request.expected_revision,
        post_fractional_revision: request
            .expected_revision
            .checked_add(1)
            .ok_or(Error::RevisionMismatch)?,
        pre_market_revision: market.revision,
        post_market_revision: market
            .revision
            .checked_add(1)
            .ok_or(Error::RevisionMismatch)?,
        native_claims,
        coordinate,
        shape,
    })
}

/// Validate the sole Claims receipt and exact V2 postresources.
#[allow(clippy::too_many_arguments)]
pub fn validate_fractional_exposure_signed_delta_postcondition_v2(
    expected: PreparedFractionalExposureSignedDeltaV2,
    packet: &[u8],
    packet_digest: [u8; 32],
    table_digest: [u8; 32],
    post_resource_digest: [u8; 32],
    receipt_bytes: &[u8],
    post_market: &[u8],
    post_positions: &[&[u8]],
) -> Result<()> {
    let plan = SignedDeltaPlanV3::decode(packet).map_err(|_| Error::ReceiptMismatch)?;
    if post_positions.len() != 2
        || plan.request_id() != expected.request_digest
        || plan.claim_count() != expected.shape.claim_count
        || plan.position_count() != 2
        || plan.position_delta_count() != 2
    {
        return Err(Error::ReceiptMismatch);
    }
    let market =
        LiabilityBasisMarketViewV2::decode(post_market).map_err(|_| Error::ReceiptMismatch)?;
    if market.revision != expected.post_market_revision
        || market.logical_market != expected.market
        || market.release_set != expected.release_set
        || market.basis_id != expected.semantic_basis
        || market.claim_count != expected.shape.claim_count
    {
        return Err(Error::ReceiptMismatch);
    }
    let mut index = 0_u32;
    while index < 2 {
        let table = *expected
            .positions
            .get(usize::try_from(index).map_err(|_| Error::Arithmetic)?)
            .ok_or(Error::ReceiptMismatch)?;
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
            || position.basis_id != expected.semantic_basis
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
        || receipt.product_record_digest() != expected.product_record
        || receipt.semantic_basis_id() != expected.semantic_basis
        || receipt.linked_basis_record_digest() != expected.linked_basis_record
        || receipt.pre_market_revision() != expected.pre_market_revision
        || receipt.post_market_revision() != expected.post_market_revision
        || receipt.claim_count() != expected.shape.claim_count
        || receipt.position_count() != 2
        || receipt.position_delta_count() != 2
    {
        return Err(Error::ReceiptMismatch);
    }
    receipt
        .validate_physical_commitment(commitment)
        .map_err(|_| Error::ReceiptMismatch)
}

fn validate_common(
    input: FractionalExposureSignedDeltaInputV2<'_>,
) -> Result<LiabilityBasisMarketViewV2> {
    let request = input
        .request
        .bind_terms(input.terms)
        .map_err(|_| Error::IdentityMismatch)?;
    let fields = request.input();
    if !matches!(
        request.action(),
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap
    ) || [
        input.semantic_product_id,
        input.market_account,
        input.claims_program,
        input.reserve_owner,
    ]
    .iter()
    .any(is_zero)
        || fields.owner == input.reserve_owner
        || fields.expected_revision == u64::MAX
    {
        return Err(Error::IdentityMismatch);
    }
    let market =
        LiabilityBasisMarketViewV2::decode(input.market_bytes).map_err(|_| Error::ClaimsState)?;
    if market.logical_market != input.terms.market()
        || market.release_set != input.terms.release_set()
        || market.product_instance_id != input.semantic_product_id
        || market.basis_id != input.terms.representation_basis()
        || market.claim_count != input.terms.representation_width()
        || market.revision == u64::MAX
    {
        return Err(Error::IdentityMismatch);
    }
    Ok(market)
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

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}
