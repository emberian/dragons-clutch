//! Signed, runtime-width requests for Dealer V3 junior equity.
//!
//! The request header binds every chain-derived pool coordinate. Any Claims
//! mutation is one complete canonical SignedDeltaV3 packet carried as the
//! trailing signed witness. Contribution Claims are derived from authenticated
//! LP debit rows; no duplicate contribution vector is encoded or persisted.

#[cfg(not(target_os = "solana"))]
extern crate alloc;

#[cfg(not(target_os = "solana"))]
use alloc::{vec, vec::Vec};

use dclutch_capability_program_contract::set_v1::{CapabilityProgramSetV1, SelectorWidthV1};
use dclutch_claims_svm::signed_delta_v3::{DeltaDirectionV3, SignedDeltaPlanV3};
use dclutch_core_contract::ContentId;
use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
use solana_program::{hash::hash, hash::hashv, pubkey::Pubkey};

use super::{
    v3_equity::{
        PoolEquityActionV3, PoolEquityContributionV3, PoolEquityInputV3, PoolEquityRedemptionV3,
        plan_pool_equity_v3,
    },
    v3_equity_claims::{
        EquityClaimsContextV3, EquityClaimsTransitionV3, equity_claims_geometry_v3,
        validate_equity_claims_packet_v3,
    },
    v3_multi_lp::{
        DEALER_LP_POSITION_PDA_DOMAIN_V3, DealerLpAccountObservationV3, DealerLpPositionV3,
        MultiLpCollateralFrameV3, MultiLpContextV3, MultiLpIntentV3, MultiLpPlanV3,
        prepare_multi_lp_v3,
    },
    v3_obligation::{DEALER_OBLIGATION_PDA_DOMAIN_V3, DealerObligationProjectionV3},
};

#[cfg(not(target_os = "solana"))]
use super::v3_equity_claims::encode_equity_claims_packet_v3;

/// Canonical junior-equity request magic.
pub const DEALER_EQUITY_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLMEQ03";
/// Successor request version carrying an exact signed Claims suffix.
pub const DEALER_EQUITY_REQUEST_VERSION_V3: u16 = 2;
/// Family-neutral CapabilityProgramSet selector offset.
pub const DEALER_EQUITY_SELECTOR_OFFSET_V3: u32 = 10;
/// Fixed signed header before the optional complete SignedDeltaV3 packet.
pub const DEALER_EQUITY_HEADER_BYTES_V3: usize = 480;
/// Header scalar containing the exact borrowed suffix width.
pub const DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3: usize = 472;

/// Contribution with no Claims child route.
pub const DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3: u16 = 1;
/// Contribution whose sparse packet changes one Position.
pub const DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3: u16 = 2;
/// Contribution whose sparse packet changes Dealer and LP Positions.
pub const DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3: u16 = 3;
/// Redemption with no Claims child route.
pub const DEALER_EQUITY_REDEEM_P0_SELECTOR_V3: u16 = 4;
/// Redemption whose sparse packet changes one Position.
pub const DEALER_EQUITY_REDEEM_P1_SELECTOR_V3: u16 = 5;
/// Redemption whose sparse packet changes Dealer and LP Positions.
pub const DEALER_EQUITY_REDEEM_P2_SELECTOR_V3: u16 = 6;

const CLAIMS_PROJECTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch/dealer/claims-projection/v3";
const CLAIMS_PROJECTION_DIGEST_STEP_V3: &[u8] = b"dclutch/dealer/claims-projection/step/v3";
const COLLATERAL_PROJECTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch/dealer/collateral-projection/v3";

/// Stable refusal from construction, decoding, or execution rejoin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityOperatorErrorV3 {
    /// Count-derived bytes, reserved bytes, or selector shape refused.
    InvalidRequest,
    /// Authenticated chain identities, PDAs, states, revisions, or digests differed.
    InvalidProjection,
    /// The selected contribution/redemption was economically inadmissible.
    InvalidIntent,
    /// CapabilityProgramSet did not select the exact physical shape.
    ProgramSelection,
    /// Caller-owned runtime scratch or output capacity was too small.
    WidthMismatch,
    /// The canonical scenario/custody/share physical planner refused.
    Physical,
    /// SignedDeltaV3 suffix differed from the recomputed Claims transition.
    Claims,
}

/// Economic action independently derived from the physical selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityRequestActionV3 {
    /// Contribute a proportional scenario basket and mint shares.
    Contribute,
    /// Burn shares and receive the pro-rata scenario basket.
    Redeem,
}

/// Transient host choice; the Claims vector is not encoded separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityRequestIntentV3<'a> {
    /// Exact scenario basket offered for exact minted shares.
    Contribute {
        /// Present collateral atoms supplied by the LP.
        collateral: u64,
        /// Native Claims supplied per outcome; encoded only as signed rows.
        claims: &'a [u64],
        /// Exact junior shares requested.
        minted_shares: u64,
    },
    /// Burn shares at the named floor-rounding boundary.
    Redeem {
        /// Exact junior shares burned.
        burned_shares: u64,
    },
}

/// Authenticated pool state used for construction and execution rejoin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EquityPoolChainProjectionV3<'a> {
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Immutable execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Canonical Trading-owned obligation PDA.
    pub obligation_address: [u8; 32],
    /// Current authenticated obligation projection.
    pub obligation: DealerObligationProjectionV3<'a>,
    /// Canonical LP Position PDA.
    pub lp_position_address: [u8; 32],
    /// Exact decoded LP Position.
    pub lp_position: DealerLpPositionV3,
    /// Exact LP Position account bytes.
    pub lp_position_bytes: &'a [u8],
    /// Current canonical Dealer Claims Position projection.
    pub dealer_claims: ClaimsInventoryObservation<'a>,
    /// Current canonical LP Claims Position projection.
    pub lp_claims: ClaimsInventoryObservation<'a>,
    /// Exact finalized Product raw-record digest.
    pub product_record_digest: [u8; 32],
    /// Exact finalized linked LiabilityBasis raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Current Claims aggregate revision.
    pub claims_market_revision: u64,
    /// Exact physical collateral accounts and pre-balances.
    pub collateral: MultiLpCollateralFrameV3,
    /// Immutable scenario residual floor.
    pub locked_capital_floor: u64,
    /// Current Core Market generation.
    pub generation: u64,
    /// Current slot/time coordinate.
    pub now: u64,
    /// Last admitted slot/time coordinate copied into the request.
    pub expires_at: u64,
    /// Whether the Market has entered terminal settlement.
    pub terminal: bool,
}

/// Borrowed hostile-decoded junior-equity request and signed Claims suffix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEquityRequestV3<'a> {
    bytes: &'a [u8],
    selector: u16,
    action: EquityRequestActionV3,
    expected_position_count: u32,
    /// Runtime Product outcome width.
    pub width: u32,
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Canonical Trading-owned LP Position PDA.
    pub lp_position: [u8; 32],
    /// LP authority and external-capital owner.
    pub lp_owner: [u8; 32],
    /// Canonical Trading-owned obligation PDA.
    pub obligation: [u8; 32],
    /// Digest of the exact obligation prestate.
    pub obligation_digest: [u8; 32],
    /// Digest of the exact LP Position prestate.
    pub lp_digest: [u8; 32],
    /// Canonical Dealer Claims Position owner.
    pub dealer_position_owner: [u8; 32],
    /// Digest of the exact Dealer Claims projection.
    pub dealer_claims_digest: [u8; 32],
    /// Digest of the exact LP Claims projection.
    pub lp_claims_digest: [u8; 32],
    /// Digest of the exact collateral endpoints and pre-balances.
    pub collateral_digest: [u8; 32],
    /// Current obligation revision.
    pub obligation_revision: u64,
    /// Current LP Position revision.
    pub lp_revision: u64,
    /// Current Dealer Claims Position revision.
    pub dealer_claims_revision: u64,
    /// Current LP Claims Position revision.
    pub lp_claims_revision: u64,
    /// Current Core Market generation.
    pub generation: u64,
    /// Last admitted slot/time coordinate.
    pub expires_at: u64,
    /// Immutable scenario residual floor.
    pub locked_capital_floor: u64,
    /// Present collateral contribution; zero on redemption.
    pub collateral: u64,
    /// Shares minted or burned.
    pub shares: u64,
    /// Exact borrowed SignedDeltaV3 suffix width.
    pub claims_packet_bytes: u32,
}

impl<'a> DealerEquityRequestV3<'a> {
    /// Hostile-decode one exact header plus optional complete Claims packet.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, EquityOperatorErrorV3> {
        if bytes.len() < DEALER_EQUITY_HEADER_BYTES_V3
            || bytes.get(..8) != Some(DEALER_EQUITY_REQUEST_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != DEALER_EQUITY_REQUEST_VERSION_V3
            || bytes.get(476..480) != Some([0_u8; 4].as_slice())
        {
            return Err(EquityOperatorErrorV3::InvalidRequest);
        }
        let selector = read_u16(bytes, 10)?;
        let (action, expected_position_count) = selector_shape(selector)?;
        let width = read_u32(bytes, 12)?;
        let claims_packet_bytes = read_u32(bytes, DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3)?;
        let expected = equity_request_bytes_v3(claims_packet_bytes)?;
        if width == 0 || bytes.len() != expected {
            return Err(EquityOperatorErrorV3::InvalidRequest);
        }
        let value = Self {
            bytes,
            selector,
            action,
            expected_position_count,
            width,
            release_set: read_identity(bytes, 16)?,
            market: read_identity(bytes, 48)?,
            child_root: read_identity(bytes, 80)?,
            lp_position: read_identity(bytes, 112)?,
            lp_owner: read_identity(bytes, 144)?,
            obligation: read_identity(bytes, 176)?,
            obligation_digest: read_identity(bytes, 208)?,
            lp_digest: read_identity(bytes, 240)?,
            dealer_position_owner: read_identity(bytes, 272)?,
            dealer_claims_digest: read_identity(bytes, 304)?,
            lp_claims_digest: read_identity(bytes, 336)?,
            collateral_digest: read_identity(bytes, 368)?,
            obligation_revision: read_u64(bytes, 400)?,
            lp_revision: read_u64(bytes, 408)?,
            dealer_claims_revision: read_u64(bytes, 416)?,
            lp_claims_revision: read_u64(bytes, 424)?,
            generation: read_u64(bytes, 432)?,
            expires_at: read_u64(bytes, 440)?,
            locked_capital_floor: read_u64(bytes, 448)?,
            collateral: read_u64(bytes, 456)?,
            shares: read_u64(bytes, 464)?,
            claims_packet_bytes,
        };
        if value.obligation_revision == 0
            || value.lp_revision == 0
            || value.dealer_claims_revision == 0
            || value.lp_claims_revision == 0
            || value.generation == 0
            || value.shares == 0
            || (action == EquityRequestActionV3::Redeem && value.collateral != 0)
        {
            return Err(EquityOperatorErrorV3::InvalidRequest);
        }
        match value.claims_plan()? {
            None if expected_position_count != 0 => Err(EquityOperatorErrorV3::InvalidRequest),
            Some(plan)
                if plan.position_count() != expected_position_count
                    || plan.claim_count() != width
                    || plan.release_set() != value.release_set
                    || plan.market() != value.market
                    || plan.request_id() != hash(value.header_bytes()).to_bytes() =>
            {
                Err(EquityOperatorErrorV3::InvalidRequest)
            }
            _ => Ok(value),
        }
    }

    /// Economic action derived from the selected physical shape.
    pub const fn action(self) -> EquityRequestActionV3 {
        self.action
    }

    /// Exact CapabilityProgramSet selector.
    pub const fn selector(self) -> u16 {
        self.selector
    }

    /// Borrow the complete exact signed request.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Borrow only the fixed header used to derive the child request identity.
    pub fn header_bytes(self) -> &'a [u8] {
        self.bytes
            .get(..DEALER_EQUITY_HEADER_BYTES_V3)
            .unwrap_or(&[])
    }

    /// Borrow the exact SignedDeltaV3 suffix; empty means no Claims route.
    pub fn claims_packet(self) -> &'a [u8] {
        self.bytes
            .get(DEALER_EQUITY_HEADER_BYTES_V3..)
            .unwrap_or(&[])
    }

    /// Hostile-decode the optional complete child packet.
    pub fn claims_plan(self) -> Result<Option<SignedDeltaPlanV3<'a>>, EquityOperatorErrorV3> {
        if self.claims_packet_bytes == 0 {
            return Ok(None);
        }
        SignedDeltaPlanV3::decode(self.claims_packet())
            .map(Some)
            .map_err(|_| EquityOperatorErrorV3::Claims)
    }
}

/// Metadata for one caller-buffer-backed unsigned request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedEquityRequestV3 {
    /// Exact initialized prefix in the caller-owned output.
    pub request_bytes: usize,
    /// Exact CapabilityProgramV3 selected for action and Claims frame P.
    pub selected_program: ContentId,
}

/// Exact request width for one optional Claims suffix.
pub fn equity_request_bytes_v3(claims_packet_bytes: u32) -> Result<usize, EquityOperatorErrorV3> {
    usize::try_from(claims_packet_bytes)
        .ok()
        .and_then(|suffix| DEALER_EQUITY_HEADER_BYTES_V3.checked_add(suffix))
        .ok_or(EquityOperatorErrorV3::InvalidRequest)
}

/// Build a signed request without encoding a duplicate Claims vector.
///
/// `output` may be a maximum-sized caller buffer; only `request_bytes` are
/// initialized. Scratch buffers are ephemeral projections and may change on a
/// refusal, while `output` remains untouched until the exact request passes
/// hostile decode and chain reauthentication.
#[cfg(not(target_os = "solana"))]
#[allow(clippy::too_many_arguments)]
pub fn build_equity_request_v3(
    chain: EquityPoolChainProjectionV3<'_>,
    intent: EquityRequestIntentV3<'_>,
    set: CapabilityProgramSetV1<'_>,
    output: &mut [u8],
    obligation_scratch: &mut [u64],
    residual_before: &mut [u64],
    residual_after: &mut [u64],
    claims_transferred: &mut [u64],
    post_dealer_claims: &mut [u64],
    post_lp_claims: &mut [u64],
) -> Result<UnsignedEquityRequestV3, EquityOperatorErrorV3> {
    validate_projection(chain)?;
    let width = chain.dealer_claims.inventory.len();
    for observed in [
        obligation_scratch.len(),
        residual_before.len(),
        residual_after.len(),
        claims_transferred.len(),
        post_dealer_claims.len(),
        post_lp_claims.len(),
    ] {
        if observed != width {
            return Err(EquityOperatorErrorV3::WidthMismatch);
        }
    }
    for (destination, source) in obligation_scratch
        .iter_mut()
        .zip(chain.obligation.obligations())
    {
        *destination = source;
    }
    let (action, collateral, shares, pool_action) = match intent {
        EquityRequestIntentV3::Contribute {
            collateral,
            claims,
            minted_shares,
        } => {
            if claims.len() != width
                || collateral > chain.collateral.lp_external_balance
                || claims
                    .iter()
                    .zip(chain.lp_claims.inventory.iter())
                    .any(|(supplied, available)| supplied > available)
            {
                return Err(EquityOperatorErrorV3::InvalidIntent);
            }
            (
                EquityRequestActionV3::Contribute,
                collateral,
                minted_shares,
                PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                    collateral,
                    claims,
                    minted_shares,
                }),
            )
        }
        EquityRequestIntentV3::Redeem { burned_shares } => {
            if burned_shares > chain.lp_position.equity_shares {
                return Err(EquityOperatorErrorV3::InvalidIntent);
            }
            (
                EquityRequestActionV3::Redeem,
                0,
                burned_shares,
                PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares }),
            )
        }
    };
    if shares == 0 {
        return Err(EquityOperatorErrorV3::InvalidIntent);
    }
    let equity = plan_pool_equity_v3(
        PoolEquityInputV3 {
            collateral: chain.collateral.principal_balance,
            claims: chain.dealer_claims.inventory,
            obligations: obligation_scratch,
            total_shares: chain.obligation.total_equity_shares(),
            locked_capital_floor: chain.locked_capital_floor,
            action: pool_action,
        },
        residual_before,
        residual_after,
        claims_transferred,
        post_dealer_claims,
    )
    .map_err(|_| EquityOperatorErrorV3::InvalidIntent)?;
    materialize_lp_poststate(
        action,
        chain.lp_claims.inventory,
        claims_transferred,
        intent,
        post_lp_claims,
    )?;
    let transition = EquityClaimsTransitionV3 {
        dealer_before: chain.dealer_claims.inventory,
        dealer_after: post_dealer_claims,
        lp_before: chain.lp_claims.inventory,
        lp_after: post_lp_claims,
        minimum_complete_sets_to_split: equity.minimum_complete_sets_to_split,
        maximum_complete_sets_to_merge: equity.maximum_complete_sets_to_merge,
    };
    let provisional = claims_context([1; 32], chain);
    let geometry = equity_claims_geometry_v3(provisional, transition)
        .map_err(|_| EquityOperatorErrorV3::Claims)?;
    let claims_packet_bytes =
        u32::try_from(geometry.packet_bytes).map_err(|_| EquityOperatorErrorV3::WidthMismatch)?;
    let selector = physical_selector(action, geometry.position_count)?;
    if set.selector_offset() != DEALER_EQUITY_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U16
    {
        return Err(EquityOperatorErrorV3::ProgramSelection);
    }
    let mut selector_bytes = [0_u8; 12];
    write_bytes(&mut selector_bytes, 10, &selector.to_le_bytes())?;
    let selected_program = set
        .select(&selector_bytes)
        .map_err(|_| EquityOperatorErrorV3::ProgramSelection)?;

    let mut header = [0_u8; DEALER_EQUITY_HEADER_BYTES_V3];
    encode_header(
        chain,
        selector,
        u32::try_from(width).map_err(|_| EquityOperatorErrorV3::WidthMismatch)?,
        collateral,
        shares,
        claims_packet_bytes,
        &mut header,
    )?;
    let request_id = hash(&header).to_bytes();
    let mut packet = vec![0_u8; geometry.packet_bytes];
    encode_equity_claims_packet_v3(claims_context(request_id, chain), transition, &mut packet)
        .map_err(|_| EquityOperatorErrorV3::Claims)?;
    let request_bytes = header
        .len()
        .checked_add(packet.len())
        .ok_or(EquityOperatorErrorV3::WidthMismatch)?;
    if output.len() < request_bytes {
        return Err(EquityOperatorErrorV3::WidthMismatch);
    }
    let mut request = Vec::with_capacity(request_bytes);
    request.extend_from_slice(&header);
    request.extend_from_slice(&packet);
    let decoded = DealerEquityRequestV3::decode(&request)?;
    authenticate_equity_request_v3(decoded, chain)?;
    output
        .get_mut(..request_bytes)
        .ok_or(EquityOperatorErrorV3::WidthMismatch)?
        .copy_from_slice(&request);
    Ok(UnsignedEquityRequestV3 {
        request_bytes,
        selected_program,
    })
}

/// Rejoin one signed request to the exact current authenticated chain state.
pub fn authenticate_equity_request_v3(
    request: DealerEquityRequestV3<'_>,
    chain: EquityPoolChainProjectionV3<'_>,
) -> Result<(), EquityOperatorErrorV3> {
    validate_projection(chain)?;
    if chain.terminal
        || chain.now > request.expires_at
        || request.expires_at != chain.expires_at
        || usize::try_from(request.width).ok() != Some(chain.dealer_claims.inventory.len())
        || request.release_set != chain.release_set
        || request.market != chain.market
        || request.child_root != chain.child_root
        || request.lp_position != chain.lp_position_address
        || request.lp_owner != chain.lp_position.lp_owner
        || request.obligation != chain.obligation_address
        || request.obligation_digest != chain.obligation.state_digest()
        || request.lp_digest != hash(chain.lp_position_bytes).to_bytes()
        || request.dealer_position_owner != chain.dealer_claims.position_owner
        || request.dealer_claims_digest != claims_projection_digest_v3(chain.dealer_claims)
        || request.lp_claims_digest != claims_projection_digest_v3(chain.lp_claims)
        || request.collateral_digest != collateral_projection_digest_v3(chain.collateral)
        || request.obligation_revision != chain.obligation.revision()
        || request.lp_revision != chain.lp_position.revision
        || request.dealer_claims_revision != chain.dealer_claims.revision
        || request.lp_claims_revision != chain.lp_claims.revision
        || request.generation != chain.generation
        || request.locked_capital_floor != chain.locked_capital_floor
    {
        return Err(EquityOperatorErrorV3::InvalidProjection);
    }
    if let Some(plan) = request.claims_plan()? {
        let context = claims_context(hash(request.header_bytes()).to_bytes(), chain);
        if plan.product_record_digest() != context.product_record_digest
            || plan.semantic_basis_id() != context.semantic_basis_id
            || plan.linked_basis_record_digest() != context.linked_basis_record_digest
            || plan.expected_market_revision() != context.expected_market_revision
        {
            return Err(EquityOperatorErrorV3::InvalidProjection);
        }
    }
    Ok(())
}

/// Derive the physical intent solely from authenticated request data.
pub fn materialize_equity_intent_v3<'a>(
    request: DealerEquityRequestV3<'_>,
    chain: EquityPoolChainProjectionV3<'_>,
    claims_scratch: &'a mut [u64],
) -> Result<MultiLpIntentV3<'a>, EquityOperatorErrorV3> {
    if usize::try_from(request.width).ok() != Some(claims_scratch.len()) {
        return Err(EquityOperatorErrorV3::WidthMismatch);
    }
    claims_scratch.fill(0);
    if request.action == EquityRequestActionV3::Contribute {
        if let Some(plan) = request.claims_plan()? {
            for row_index in 0..plan.position_delta_count() {
                let row = plan
                    .position_delta(row_index)
                    .map_err(|_| EquityOperatorErrorV3::Claims)?;
                let position = plan
                    .position(row.position_index())
                    .map_err(|_| EquityOperatorErrorV3::Claims)?;
                if position.owner() == chain.lp_claims.position_owner {
                    if row.delta().direction() != DeltaDirectionV3::Debit {
                        return Err(EquityOperatorErrorV3::InvalidIntent);
                    }
                    let coordinate = usize::try_from(row.outcome())
                        .map_err(|_| EquityOperatorErrorV3::WidthMismatch)?;
                    *claims_scratch
                        .get_mut(coordinate)
                        .ok_or(EquityOperatorErrorV3::WidthMismatch)? = row.delta().magnitude();
                } else if position.owner() != chain.dealer_claims.position_owner {
                    return Err(EquityOperatorErrorV3::InvalidIntent);
                }
            }
        }
        Ok(MultiLpIntentV3::Contribute {
            collateral: request.collateral,
            claims: claims_scratch,
            minted_shares: request.shares,
            expected_lp_revision: request.lp_revision,
            expected_lp_digest: request.lp_digest,
        })
    } else {
        Ok(MultiLpIntentV3::Redeem {
            burned_shares: request.shares,
            expected_lp_revision: request.lp_revision,
            expected_lp_digest: request.lp_digest,
        })
    }
}

/// Authenticate, plan physical effects, and byte-compare the Claims suffix.
#[allow(clippy::too_many_arguments)]
pub fn prepare_equity_request_v3(
    request: DealerEquityRequestV3<'_>,
    chain: EquityPoolChainProjectionV3<'_>,
    context: MultiLpContextV3,
    request_claims_scratch: &mut [u64],
    obligation_scratch: &mut [u64],
    residual_before: &mut [u64],
    residual_after: &mut [u64],
    claims_transferred: &mut [u64],
    post_dealer_claims: &mut [u64],
    post_lp_claims: &mut [u64],
    post_obligation: &mut [u8],
    post_lp: &mut [u8],
) -> Result<MultiLpPlanV3, EquityOperatorErrorV3> {
    authenticate_equity_request_v3(request, chain)?;
    if context.trading_program != chain.trading_program
        || context.release_set != chain.release_set
        || context.market != chain.market
        || context.child_root != chain.child_root
        || context.obligation_account != chain.obligation_address
        || context.generation != chain.generation
        || context.locked_capital_floor != chain.locked_capital_floor
        || context.parent_request_digest != hash(request.bytes()).to_bytes()
    {
        return Err(EquityOperatorErrorV3::InvalidProjection);
    }
    let intent = materialize_equity_intent_v3(request, chain, request_claims_scratch)?;
    let plan = prepare_multi_lp_v3(
        context,
        chain.collateral,
        DealerLpAccountObservationV3 {
            address: chain.lp_position_address,
            owner: chain.trading_program,
            data: chain.lp_position_bytes,
        },
        chain.obligation,
        chain.dealer_claims,
        chain.lp_claims,
        intent,
        obligation_scratch,
        residual_before,
        residual_after,
        claims_transferred,
        post_dealer_claims,
        post_lp_claims,
        post_obligation,
        post_lp,
    )
    .map_err(|_| EquityOperatorErrorV3::Physical)?;
    validate_equity_claims_packet_v3(
        claims_context(hash(request.header_bytes()).to_bytes(), chain),
        EquityClaimsTransitionV3 {
            dealer_before: chain.dealer_claims.inventory,
            dealer_after: post_dealer_claims,
            lp_before: chain.lp_claims.inventory,
            lp_after: post_lp_claims,
            minimum_complete_sets_to_split: plan.minimum_complete_sets_to_split,
            maximum_complete_sets_to_merge: plan.maximum_complete_sets_to_merge,
        },
        request.claims_packet(),
    )
    .map_err(|_| EquityOperatorErrorV3::Claims)?;
    Ok(plan)
}

/// Exact authenticated witness range and Position-frame count for Hot.
pub fn equity_claims_witness_v3(
    request: DealerEquityRequestV3<'_>,
) -> Result<Option<(usize, usize, u32)>, EquityOperatorErrorV3> {
    match request.claims_plan()? {
        None => Ok(None),
        Some(plan) => Ok(Some((
            DEALER_EQUITY_HEADER_BYTES_V3,
            request.claims_packet().len(),
            plan.position_count(),
        ))),
    }
}

/// Digest the complete authenticated Claims projection without allocation.
pub fn claims_projection_digest_v3(observation: ClaimsInventoryObservation<'_>) -> [u8; 32] {
    let revision = observation.revision.to_le_bytes();
    let width = u64::try_from(observation.inventory.len())
        .unwrap_or(u64::MAX)
        .to_le_bytes();
    let mut digest = hashv(&[
        CLAIMS_PROJECTION_DIGEST_DOMAIN_V3,
        &observation.market_id,
        &observation.product_id,
        &observation.liability_basis_id,
        &observation.position_owner,
        &revision,
        &width,
    ])
    .to_bytes();
    for value in observation.inventory {
        digest = hashv(&[
            CLAIMS_PROJECTION_DIGEST_STEP_V3,
            &digest,
            &value.to_le_bytes(),
        ])
        .to_bytes();
    }
    digest
}

/// Digest every collateral endpoint and pre-balance used by physical planning.
pub fn collateral_projection_digest_v3(frame: MultiLpCollateralFrameV3) -> [u8; 32] {
    hashv(&[
        COLLATERAL_PROJECTION_DIGEST_DOMAIN_V3,
        &frame.lp_external_account,
        &frame.lp_owner,
        &frame.lp_external_balance.to_le_bytes(),
        &frame.lp_external_delegate,
        &frame.lp_external_delegated_amount.to_le_bytes(),
        &frame.principal_vault,
        &frame.principal_balance.to_le_bytes(),
        &frame.hoard_vault,
        &frame.hoard_balance.to_le_bytes(),
    ])
    .to_bytes()
}

fn encode_header(
    chain: EquityPoolChainProjectionV3<'_>,
    selector: u16,
    width: u32,
    collateral: u64,
    shares: u64,
    claims_packet_bytes: u32,
    output: &mut [u8; DEALER_EQUITY_HEADER_BYTES_V3],
) -> Result<(), EquityOperatorErrorV3> {
    output.fill(0);
    write_bytes(output, 0, &DEALER_EQUITY_REQUEST_MAGIC_V3)?;
    write_bytes(output, 8, &DEALER_EQUITY_REQUEST_VERSION_V3.to_le_bytes())?;
    write_bytes(output, 10, &selector.to_le_bytes())?;
    write_bytes(output, 12, &width.to_le_bytes())?;
    for (offset, identity) in [
        (16, chain.release_set),
        (48, chain.market),
        (80, chain.child_root),
        (112, chain.lp_position_address),
        (144, chain.lp_position.lp_owner),
        (176, chain.obligation_address),
        (208, chain.obligation.state_digest()),
        (240, hash(chain.lp_position_bytes).to_bytes()),
        (272, chain.dealer_claims.position_owner),
        (304, claims_projection_digest_v3(chain.dealer_claims)),
        (336, claims_projection_digest_v3(chain.lp_claims)),
        (368, collateral_projection_digest_v3(chain.collateral)),
    ] {
        write_bytes(output, offset, &identity)?;
    }
    for (offset, value) in [
        (400, chain.obligation.revision()),
        (408, chain.lp_position.revision),
        (416, chain.dealer_claims.revision),
        (424, chain.lp_claims.revision),
        (432, chain.generation),
        (440, chain.expires_at),
        (448, chain.locked_capital_floor),
        (456, collateral),
        (464, shares),
    ] {
        write_bytes(output, offset, &value.to_le_bytes())?;
    }
    write_bytes(
        output,
        DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3,
        &claims_packet_bytes.to_le_bytes(),
    )
}

fn materialize_lp_poststate(
    action: EquityRequestActionV3,
    current: &[u64],
    transferred: &[u64],
    intent: EquityRequestIntentV3<'_>,
    output: &mut [u64],
) -> Result<(), EquityOperatorErrorV3> {
    if current.len() != transferred.len() || output.len() != current.len() {
        return Err(EquityOperatorErrorV3::WidthMismatch);
    }
    let supplied = match intent {
        EquityRequestIntentV3::Contribute { claims, .. } => Some(claims),
        EquityRequestIntentV3::Redeem { .. } => None,
    };
    for (index, destination) in output.iter_mut().enumerate() {
        let current = current
            .get(index)
            .copied()
            .ok_or(EquityOperatorErrorV3::WidthMismatch)?;
        *destination = match (action, supplied) {
            (EquityRequestActionV3::Contribute, Some(claims)) => current
                .checked_sub(
                    claims
                        .get(index)
                        .copied()
                        .ok_or(EquityOperatorErrorV3::WidthMismatch)?,
                )
                .ok_or(EquityOperatorErrorV3::InvalidIntent)?,
            (EquityRequestActionV3::Redeem, None) => current
                .checked_add(
                    transferred
                        .get(index)
                        .copied()
                        .ok_or(EquityOperatorErrorV3::WidthMismatch)?,
                )
                .ok_or(EquityOperatorErrorV3::InvalidIntent)?,
            _ => return Err(EquityOperatorErrorV3::InvalidIntent),
        };
    }
    Ok(())
}

fn claims_context(
    request_id: [u8; 32],
    chain: EquityPoolChainProjectionV3<'_>,
) -> EquityClaimsContextV3 {
    EquityClaimsContextV3 {
        release_set: chain.release_set,
        market: chain.market,
        request_id,
        product_record_digest: chain.product_record_digest,
        semantic_basis_id: chain.dealer_claims.liability_basis_id,
        linked_basis_record_digest: chain.linked_basis_record_digest,
        expected_market_revision: chain.claims_market_revision,
        dealer_owner: chain.dealer_claims.position_owner,
        dealer_revision: chain.dealer_claims.revision,
        lp_owner: chain.lp_claims.position_owner,
        lp_revision: chain.lp_claims.revision,
    }
}

fn selector_shape(selector: u16) -> Result<(EquityRequestActionV3, u32), EquityOperatorErrorV3> {
    match selector {
        DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3 => Ok((EquityRequestActionV3::Contribute, 0)),
        DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3 => Ok((EquityRequestActionV3::Contribute, 1)),
        DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3 => Ok((EquityRequestActionV3::Contribute, 2)),
        DEALER_EQUITY_REDEEM_P0_SELECTOR_V3 => Ok((EquityRequestActionV3::Redeem, 0)),
        DEALER_EQUITY_REDEEM_P1_SELECTOR_V3 => Ok((EquityRequestActionV3::Redeem, 1)),
        DEALER_EQUITY_REDEEM_P2_SELECTOR_V3 => Ok((EquityRequestActionV3::Redeem, 2)),
        _ => Err(EquityOperatorErrorV3::InvalidRequest),
    }
}

fn physical_selector(
    action: EquityRequestActionV3,
    positions: u32,
) -> Result<u16, EquityOperatorErrorV3> {
    match (action, positions) {
        (EquityRequestActionV3::Contribute, 0) => Ok(DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3),
        (EquityRequestActionV3::Contribute, 1) => Ok(DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3),
        (EquityRequestActionV3::Contribute, 2) => Ok(DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3),
        (EquityRequestActionV3::Redeem, 0) => Ok(DEALER_EQUITY_REDEEM_P0_SELECTOR_V3),
        (EquityRequestActionV3::Redeem, 1) => Ok(DEALER_EQUITY_REDEEM_P1_SELECTOR_V3),
        (EquityRequestActionV3::Redeem, 2) => Ok(DEALER_EQUITY_REDEEM_P2_SELECTOR_V3),
        _ => Err(EquityOperatorErrorV3::InvalidIntent),
    }
}

fn validate_projection(
    chain: EquityPoolChainProjectionV3<'_>,
) -> Result<(), EquityOperatorErrorV3> {
    for identity in [
        chain.trading_program,
        chain.release_set,
        chain.market,
        chain.child_root,
        chain.obligation_address,
        chain.lp_position_address,
        chain.dealer_claims.position_owner,
        chain.lp_claims.position_owner,
        chain.product_record_digest,
        chain.linked_basis_record_digest,
        chain.collateral.lp_external_account,
        chain.collateral.lp_owner,
        chain.collateral.principal_vault,
        chain.collateral.hoard_vault,
    ] {
        if identity == [0; 32] {
            return Err(EquityOperatorErrorV3::InvalidProjection);
        }
    }
    let width = chain.dealer_claims.inventory.len();
    let trading = Pubkey::new_from_array(chain.trading_program);
    let expected_obligation = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &chain.child_root],
        &trading,
    )
    .0
    .to_bytes();
    let expected_lp = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            &chain.child_root,
            &chain.lp_position.lp_owner,
        ],
        &trading,
    )
    .0
    .to_bytes();
    let descriptor = chain.obligation.descriptor(chain.locked_capital_floor);
    if chain.terminal
        || chain.now > chain.expires_at
        || chain.generation == 0
        || width == 0
        || usize::try_from(chain.obligation.width()).ok() != Some(width)
        || chain.lp_claims.inventory.len() != width
        || chain.obligation_address != expected_obligation
        || chain.lp_position_address != expected_lp
        || chain.obligation.child_root() != chain.child_root
        || chain.obligation.position_owner() != chain.dealer_claims.position_owner
        || chain.obligation.total_equity_shares() < chain.lp_position.equity_shares
        || chain.lp_position.release_set != chain.release_set
        || chain.lp_position.market != chain.market
        || chain.lp_position.child_root != chain.child_root
        || chain.lp_position.obligation_account != chain.obligation_address
        || chain.lp_position.generation != chain.generation
        || chain.collateral.lp_owner != chain.lp_position.lp_owner
        || chain.dealer_claims.market_id != chain.market
        || chain.lp_claims.market_id != chain.market
        || chain.dealer_claims.product_id != chain.lp_claims.product_id
        || chain.dealer_claims.liability_basis_id != chain.lp_claims.liability_basis_id
        || chain.dealer_claims.position_owner == chain.lp_claims.position_owner
        || chain.lp_claims.position_owner != chain.lp_position.lp_owner
        || descriptor.market_id != chain.market
        || descriptor.product_id != chain.dealer_claims.product_id
        || descriptor.liability_basis_id != chain.dealer_claims.liability_basis_id
        || DealerLpPositionV3::decode(chain.lp_position_bytes) != Ok(chain.lp_position)
    {
        return Err(EquityOperatorErrorV3::InvalidProjection);
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, EquityOperatorErrorV3> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(EquityOperatorErrorV3::InvalidRequest)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, EquityOperatorErrorV3> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(EquityOperatorErrorV3::InvalidRequest)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, EquityOperatorErrorV3> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(EquityOperatorErrorV3::InvalidRequest)
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<[u8; 32], EquityOperatorErrorV3> {
    let value = bytes
        .get(offset..offset + 32)
        .and_then(|value| value.try_into().ok())
        .ok_or(EquityOperatorErrorV3::InvalidRequest)?;
    if value == [0; 32] {
        Err(EquityOperatorErrorV3::InvalidRequest)
    } else {
        Ok(value)
    }
}

fn write_bytes(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<(), EquityOperatorErrorV3> {
    let end = offset
        .checked_add(value.len())
        .ok_or(EquityOperatorErrorV3::InvalidRequest)?;
    bytes
        .get_mut(offset..end)
        .ok_or(EquityOperatorErrorV3::InvalidRequest)?
        .copy_from_slice(value);
    Ok(())
}
