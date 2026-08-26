//! Runtime-width chain-derived requests for Dealer V3 junior equity.
//!
//! The signed request contains only the LP's economic choice. Market, Product,
//! LiabilityBasis, Dealer/LP Claims inventories, obligation state, LP state,
//! physical collateral balances, revisions, generation, and expiry are copied
//! from authenticated chain projections and rejoined before execution. The
//! request therefore cannot smuggle a second NAV, obligation, or inventory
//! authority into the protocol.

use dclutch_capability_program_contract::set_v1::{CapabilityProgramSetV1, SelectorWidthV1};
use dclutch_core_contract::ContentId;
use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
use solana_program::{hash::hashv, pubkey::Pubkey};

use super::{
    v3_equity::{
        preflight_pool_equity_v3, PoolEquityActionV3, PoolEquityContributionV3, PoolEquityInputV3,
        PoolEquityRedemptionV3,
    },
    v3_multi_lp::{
        prepare_multi_lp_v3, DealerLpAccountObservationV3, DealerLpPositionV3,
        MultiLpCollateralFrameV3, MultiLpContextV3, MultiLpIntentV3, MultiLpPlanV3,
        DEALER_LP_POSITION_PDA_DOMAIN_V3,
    },
    v3_obligation::{DealerObligationProjectionV3, DEALER_OBLIGATION_PDA_DOMAIN_V3},
};

/// Canonical junior-equity request magic.
pub const DEALER_EQUITY_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLMEQ03";
/// Canonical request version.
pub const DEALER_EQUITY_REQUEST_VERSION_V3: u16 = 1;
/// Family-neutral CapabilityProgramSet selector offset.
pub const DEALER_EQUITY_SELECTOR_OFFSET_V3: u32 = 10;
/// Fixed prefix before one contribution quantity per outcome.
pub const DEALER_EQUITY_HEADER_BYTES_V3: usize = 480;
/// Exact contributed-Claims item width.
pub const DEALER_EQUITY_ITEM_BYTES_V3: usize = 8;
/// Contribute an exactly proportional scenario basket.
pub const DEALER_EQUITY_CONTRIBUTE_ACTION_V3: u16 = 1;
/// Burn shares for the canonical pro-rata scenario basket.
pub const DEALER_EQUITY_REDEEM_ACTION_V3: u16 = 2;

const CLAIMS_PROJECTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch/dealer/claims-projection/v3";
const CLAIMS_PROJECTION_DIGEST_STEP_V3: &[u8] = b"dclutch/dealer/claims-projection/step/v3";
const COLLATERAL_PROJECTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch/dealer/collateral-projection/v3";

/// Stable refusal from junior-equity request construction or authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityOperatorErrorV3 {
    /// Count-derived bytes, reserved bytes, or action encoding refused.
    InvalidRequest,
    /// Current chain identities, PDAs, states, revisions, or digests differed.
    InvalidProjection,
    /// The selected contribution/redemption was economically inadmissible.
    InvalidIntent,
    /// Family-neutral CapabilityProgramSet did not select the exact action.
    ProgramSelection,
    /// Caller-owned runtime scratch had the wrong width.
    WidthMismatch,
    /// The canonical scenario/custody/share physical planner refused.
    Physical,
}

/// Exact action selected by one junior-equity request.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityRequestActionV3 {
    /// Contribute an exact scenario basket and mint shares.
    Contribute = DEALER_EQUITY_CONTRIBUTE_ACTION_V3,
    /// Burn shares and receive the canonical scenario basket.
    Redeem = DEALER_EQUITY_REDEEM_ACTION_V3,
}

/// The only caller-selected economic coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EquityRequestIntentV3<'a> {
    /// Exact collateral and Claims basket offered for exact minted shares.
    Contribute {
        /// Present collateral atoms supplied by the LP.
        collateral: u64,
        /// Native Claims supplied per outcome.
        claims: &'a [u64],
        /// Exact junior shares requested.
        minted_shares: u64,
    },
    /// Exact shares burned at the named floor-rounding boundary.
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

/// Borrowed hostile-decoded junior-equity request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerEquityRequestV3<'a> {
    bytes: &'a [u8],
    /// Selected action.
    pub action: EquityRequestActionV3,
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
}

impl<'a> DealerEquityRequestV3<'a> {
    /// Hostile-decode one exact count-derived request.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, EquityOperatorErrorV3> {
        if bytes.len() < DEALER_EQUITY_HEADER_BYTES_V3
            || bytes.get(..8) != Some(DEALER_EQUITY_REQUEST_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != DEALER_EQUITY_REQUEST_VERSION_V3
            || bytes
                .get(472..480)
                .is_none_or(|reserved| reserved.iter().any(|value| *value != 0))
        {
            return Err(EquityOperatorErrorV3::InvalidRequest);
        }
        let action = match read_u16(bytes, 10)? {
            DEALER_EQUITY_CONTRIBUTE_ACTION_V3 => EquityRequestActionV3::Contribute,
            DEALER_EQUITY_REDEEM_ACTION_V3 => EquityRequestActionV3::Redeem,
            _ => return Err(EquityOperatorErrorV3::InvalidRequest),
        };
        let width = read_u32(bytes, 12)?;
        if width == 0 || bytes.len() != equity_request_bytes_v3(width)? {
            return Err(EquityOperatorErrorV3::InvalidRequest);
        }
        let value = Self {
            bytes,
            action,
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
        };
        if value.obligation_revision == 0
            || value.lp_revision == 0
            || value.dealer_claims_revision == 0
            || value.lp_claims_revision == 0
            || value.generation == 0
            || value.shares == 0
        {
            return Err(EquityOperatorErrorV3::InvalidRequest);
        }
        let mut any_claims = false;
        for index in 0..width {
            any_claims |= value.claim(index)? != 0;
        }
        match action {
            EquityRequestActionV3::Contribute if value.collateral == 0 && !any_claims => {
                Err(EquityOperatorErrorV3::InvalidRequest)
            }
            EquityRequestActionV3::Redeem if value.collateral != 0 || any_claims => {
                Err(EquityOperatorErrorV3::InvalidRequest)
            }
            _ => Ok(value),
        }
    }

    /// Borrow the exact bytes hashed by the common parent request boundary.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Decode one contributed Claims quantity.
    pub fn claim(self, index: u32) -> Result<u64, EquityOperatorErrorV3> {
        if index >= self.width {
            return Err(EquityOperatorErrorV3::InvalidRequest);
        }
        let offset = usize::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(DEALER_EQUITY_ITEM_BYTES_V3))
            .and_then(|index| DEALER_EQUITY_HEADER_BYTES_V3.checked_add(index))
            .ok_or(EquityOperatorErrorV3::InvalidRequest)?;
        read_u64(self.bytes, offset)
    }

    /// Decode the runtime contribution vector into caller-owned scratch.
    pub fn decode_claims(self, output: &mut [u64]) -> Result<(), EquityOperatorErrorV3> {
        if usize::try_from(self.width).ok() != Some(output.len()) {
            return Err(EquityOperatorErrorV3::WidthMismatch);
        }
        for (index, destination) in output.iter_mut().enumerate() {
            *destination = self
                .claim(u32::try_from(index).map_err(|_| EquityOperatorErrorV3::WidthMismatch)?)?;
        }
        Ok(())
    }
}

/// Metadata for one caller-buffer-backed unsigned request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedEquityRequestV3 {
    /// Exact initialized request bytes in the caller-owned output.
    pub request_bytes: usize,
    /// Exact CapabilityProgramV3 selected from the authenticated set.
    pub selected_program: ContentId,
}

/// Exact count-derived request width.
pub fn equity_request_bytes_v3(width: u32) -> Result<usize, EquityOperatorErrorV3> {
    usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(DEALER_EQUITY_ITEM_BYTES_V3))
        .and_then(|body| DEALER_EQUITY_HEADER_BYTES_V3.checked_add(body))
        .ok_or(EquityOperatorErrorV3::InvalidRequest)
}

/// Build one chain-derived unsigned contribution/redemption request.
///
/// `output` remains byte-for-byte unchanged on every refusal. The obligation
/// scratch may contain the authenticated Trading vector after economic
/// preflight; it is ephemeral materialization, never persisted authority.
pub fn build_equity_request_v3(
    chain: EquityPoolChainProjectionV3<'_>,
    intent: EquityRequestIntentV3<'_>,
    set: CapabilityProgramSetV1<'_>,
    output: &mut [u8],
    obligation_scratch: &mut [u64],
) -> Result<UnsignedEquityRequestV3, EquityOperatorErrorV3> {
    validate_projection(chain)?;
    let width = chain.dealer_claims.inventory.len();
    let width_u32 = u32::try_from(width).map_err(|_| EquityOperatorErrorV3::WidthMismatch)?;
    if output.len() != equity_request_bytes_v3(width_u32)? || obligation_scratch.len() != width {
        return Err(EquityOperatorErrorV3::WidthMismatch);
    }
    let (action, collateral, shares, claims) = match intent {
        EquityRequestIntentV3::Contribute {
            collateral,
            claims,
            minted_shares,
        } if claims.len() == width => (
            EquityRequestActionV3::Contribute,
            collateral,
            minted_shares,
            Some(claims),
        ),
        EquityRequestIntentV3::Redeem { burned_shares } => {
            (EquityRequestActionV3::Redeem, 0, burned_shares, None)
        }
        _ => return Err(EquityOperatorErrorV3::WidthMismatch),
    };
    if shares == 0 {
        return Err(EquityOperatorErrorV3::InvalidIntent);
    }
    if set.selector_offset() != DEALER_EQUITY_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U16
    {
        return Err(EquityOperatorErrorV3::ProgramSelection);
    }
    for (destination, source) in obligation_scratch
        .iter_mut()
        .zip(chain.obligation.obligations())
    {
        *destination = source;
    }
    let equity_action = match (action, claims) {
        (EquityRequestActionV3::Contribute, Some(claims)) => {
            if collateral > chain.collateral.lp_external_balance
                || claims
                    .iter()
                    .zip(chain.lp_claims.inventory.iter())
                    .any(|(supplied, available)| supplied > available)
            {
                return Err(EquityOperatorErrorV3::InvalidIntent);
            }
            PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                collateral,
                claims,
                minted_shares: shares,
            })
        }
        (EquityRequestActionV3::Redeem, None) => {
            if shares > chain.lp_position.equity_shares {
                return Err(EquityOperatorErrorV3::InvalidIntent);
            }
            PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 {
                burned_shares: shares,
            })
        }
        _ => return Err(EquityOperatorErrorV3::InvalidIntent),
    };
    preflight_pool_equity_v3(PoolEquityInputV3 {
        collateral: chain.collateral.principal_balance,
        claims: chain.dealer_claims.inventory,
        obligations: obligation_scratch,
        total_shares: chain.obligation.total_equity_shares(),
        locked_capital_floor: chain.locked_capital_floor,
        action: equity_action,
    })
    .map_err(|_| EquityOperatorErrorV3::InvalidIntent)?;

    let mut selector = [0_u8; 12];
    selector[10..12].copy_from_slice(&(action as u16).to_le_bytes());
    let selected_program = set
        .select(&selector)
        .map_err(|_| EquityOperatorErrorV3::ProgramSelection)?;

    output.fill(0);
    write_bytes(output, 0, &DEALER_EQUITY_REQUEST_MAGIC_V3)?;
    write_bytes(output, 8, &DEALER_EQUITY_REQUEST_VERSION_V3.to_le_bytes())?;
    write_bytes(output, 10, &(action as u16).to_le_bytes())?;
    write_bytes(output, 12, &width_u32.to_le_bytes())?;
    for (offset, identity) in [
        (16, chain.release_set),
        (48, chain.market),
        (80, chain.child_root),
        (112, chain.lp_position_address),
        (144, chain.lp_position.lp_owner),
        (176, chain.obligation_address),
        (208, chain.obligation.state_digest()),
        (
            240,
            solana_program::hash::hash(chain.lp_position_bytes).to_bytes(),
        ),
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
    if let Some(claims) = claims {
        for (index, value) in claims.iter().enumerate() {
            let offset = DEALER_EQUITY_HEADER_BYTES_V3 + index * DEALER_EQUITY_ITEM_BYTES_V3;
            write_bytes(output, offset, &value.to_le_bytes())?;
        }
    }
    let request = DealerEquityRequestV3::decode(output)?;
    authenticate_equity_request_v3(request, chain)?;
    Ok(UnsignedEquityRequestV3 {
        request_bytes: output.len(),
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
        || request.lp_digest != solana_program::hash::hash(chain.lp_position_bytes).to_bytes()
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
    Ok(())
}

/// Materialize the already-authenticated signed choice for the physical planner.
pub fn materialize_equity_intent_v3<'a>(
    request: DealerEquityRequestV3<'_>,
    claims_scratch: &'a mut [u64],
) -> Result<MultiLpIntentV3<'a>, EquityOperatorErrorV3> {
    request.decode_claims(claims_scratch)?;
    match request.action {
        EquityRequestActionV3::Contribute => Ok(MultiLpIntentV3::Contribute {
            collateral: request.collateral,
            claims: claims_scratch,
            minted_shares: request.shares,
            expected_lp_revision: request.lp_revision,
            expected_lp_digest: request.lp_digest,
        }),
        EquityRequestActionV3::Redeem => Ok(MultiLpIntentV3::Redeem {
            burned_shares: request.shares,
            expected_lp_revision: request.lp_revision,
            expected_lp_digest: request.lp_digest,
        }),
    }
}

/// Authenticate one signed request and invoke the sole physical equity planner.
///
/// The common Trading outer supplies the exact current chain projection and
/// derives `context.parent_request_digest` from these request bytes. All
/// Claims, obligation, LP, and Custody poststate candidates remain owned by
/// `prepare_multi_lp_v3`; this adapter creates no alternate transition DTO.
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
        || context.parent_request_digest != solana_program::hash::hash(request.bytes()).to_bytes()
    {
        return Err(EquityOperatorErrorV3::InvalidProjection);
    }
    let intent = materialize_equity_intent_v3(request, request_claims_scratch)?;
    prepare_multi_lp_v3(
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
    .map_err(|_| EquityOperatorErrorV3::Physical)
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
        &frame.principal_vault,
        &frame.principal_balance.to_le_bytes(),
        &frame.hoard_vault,
        &frame.hoard_balance.to_le_bytes(),
    ])
    .to_bytes()
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
        || chain
            .obligation
            .descriptor(chain.locked_capital_floor)
            .market_id
            != chain.market
        || chain
            .obligation
            .descriptor(chain.locked_capital_floor)
            .product_id
            != chain.dealer_claims.product_id
        || chain
            .obligation
            .descriptor(chain.locked_capital_floor)
            .liability_basis_id
            != chain.dealer_claims.liability_basis_id
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
