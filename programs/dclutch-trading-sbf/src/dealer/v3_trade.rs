//! Runtime-width exact Dealer trade requests and physical composition.
//!
//! The signed family request carries the complete portfolio and quote intent,
//! while every identity, candidate digest, revision, generation, and expiry is
//! copied from authenticated chain state. The adapter rejoins those bytes to
//! the one Trading-owned obligation projection and both canonical Claims
//! Positions before calling the scenario-solvent physical composer.

use dclutch_capability_program_contract::set_v1::{CapabilityProgramSetV1, SelectorWidthV1};
use dclutch_core_contract::ContentId;
use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
use solana_program::{hash::hash, pubkey::Pubkey};

use super::{
    v3_composer::{
        ScenarioAtomicPlanV3, ScenarioCollateralFrameV3, ScenarioComposerContextV3,
        ScenarioComposerErrorV3, ScenarioFillInputV3, ScenarioQuoteDirectionV3, ScenarioQuoteLegV3,
        prepare_scenario_atomic_v3,
    },
    v3_obligation::{DEALER_OBLIGATION_PDA_DOMAIN_V3, DealerObligationProjectionV3},
};

/// Canonical exact-fill request magic.
pub const DEALER_SCENARIO_TRADE_MAGIC_V3: [u8; 8] = *b"DCLDST03";
/// Canonical exact-fill request version.
pub const DEALER_SCENARIO_TRADE_VERSION_V3: u16 = 1;
/// Family-neutral CapabilityProgramSet selector offset.
pub const DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3: u32 = 10;
/// Exact fixed header before two runtime-width `u64` vectors.
pub const DEALER_SCENARIO_TRADE_HEADER_BYTES_V3: usize = 384;
/// One acquired and one delivered quantity per outcome.
pub const DEALER_SCENARIO_TRADE_ITEM_BYTES_V3: usize = 16;
/// Sole exact-fill action in the global Dealer selector space.
pub const DEALER_SCENARIO_TRADE_ACTION_V3: u16 = 9;

/// Stable refusal from request decoding, construction, or execution joining.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioTradeErrorV3 {
    /// Count-derived bytes, reserved fields, or action encoding refused.
    InvalidRequest,
    /// Authenticated chain identities, PDAs, revisions, or digests differed.
    InvalidProjection,
    /// Portfolio or quote intent was empty, noncanonical, or lifecycle-disabled.
    InvalidIntent,
    /// Family-neutral CapabilityProgramSet did not select the exact action.
    ProgramSelection,
    /// Runtime caller-owned scratch width differed.
    WidthMismatch,
    /// Scenario-solvent physical composition refused.
    Composition,
}

impl From<ScenarioComposerErrorV3> for ScenarioTradeErrorV3 {
    fn from(_: ScenarioComposerErrorV3) -> Self {
        Self::Composition
    }
}

/// Exact user-selected principal direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioTradeDirectionV3 {
    /// Counterparty pays principal plus the separately realized fee.
    CounterpartyPaysDealer,
    /// Dealer pays net principal after moving the realized fee to FeeVault.
    DealerPaysCounterparty,
}

/// Borrowed hostile-decoded exact trade request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerScenarioTradeRequestV3<'a> {
    bytes: &'a [u8],
    /// Runtime Product outcome width.
    pub width: u32,
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Canonical Trading-owned obligation PDA.
    pub obligation: [u8; 32],
    /// Exact current obligation-state digest.
    pub current_obligation_digest: [u8; 32],
    /// Exact candidate obligation-state digest.
    pub candidate_obligation_digest: [u8; 32],
    /// Canonical Dealer Claims Position owner.
    pub dealer_owner: [u8; 32],
    /// Exact counterparty Claims Position owner.
    pub counterparty_owner: [u8; 32],
    /// Exact counterparty external collateral account.
    pub counterparty_account: [u8; 32],
    /// Current obligation revision.
    pub current_obligation_revision: u64,
    /// Candidate obligation revision.
    pub candidate_obligation_revision: u64,
    /// Current Dealer Position revision.
    pub dealer_position_revision: u64,
    /// Current counterparty Position revision.
    pub counterparty_position_revision: u64,
    /// Current Claims aggregate revision.
    pub claims_revision: u64,
    /// Current Core Market generation.
    pub generation: u64,
    /// Last admitted slot/time coordinate.
    pub expires_at: u64,
    /// Positive exact quote principal atoms.
    pub principal: u64,
    /// Exact separately realized fee atoms.
    pub realized_fee: u64,
    /// Principal direction.
    pub direction: ScenarioTradeDirectionV3,
}

impl<'a> DealerScenarioTradeRequestV3<'a> {
    /// Hostile-decode the exact count-derived request.
    pub fn decode(bytes: &'a [u8]) -> Result<Self, ScenarioTradeErrorV3> {
        if bytes.len() < DEALER_SCENARIO_TRADE_HEADER_BYTES_V3
            || bytes.get(..8) != Some(DEALER_SCENARIO_TRADE_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != DEALER_SCENARIO_TRADE_VERSION_V3
            || read_u16(bytes, 10)? != DEALER_SCENARIO_TRADE_ACTION_V3
            || bytes.get(377..384).is_none_or(|value| value != [0; 7])
        {
            return Err(ScenarioTradeErrorV3::InvalidRequest);
        }
        let width = read_u32(bytes, 12)?;
        let expected = scenario_trade_request_bytes_v3(width)?;
        if width == 0 || bytes.len() != expected {
            return Err(ScenarioTradeErrorV3::InvalidRequest);
        }
        let direction = match byte(bytes, 376)? {
            0 => ScenarioTradeDirectionV3::CounterpartyPaysDealer,
            1 => ScenarioTradeDirectionV3::DealerPaysCounterparty,
            _ => return Err(ScenarioTradeErrorV3::InvalidRequest),
        };
        let value = Self {
            bytes,
            width,
            release_set: read_identity(bytes, 16)?,
            market: read_identity(bytes, 48)?,
            child_root: read_identity(bytes, 80)?,
            obligation: read_identity(bytes, 112)?,
            current_obligation_digest: read_identity(bytes, 144)?,
            candidate_obligation_digest: read_identity(bytes, 176)?,
            dealer_owner: read_identity(bytes, 208)?,
            counterparty_owner: read_identity(bytes, 240)?,
            counterparty_account: read_identity(bytes, 272)?,
            current_obligation_revision: read_u64(bytes, 304)?,
            candidate_obligation_revision: read_u64(bytes, 312)?,
            dealer_position_revision: read_u64(bytes, 320)?,
            counterparty_position_revision: read_u64(bytes, 328)?,
            claims_revision: read_u64(bytes, 336)?,
            generation: read_u64(bytes, 344)?,
            expires_at: read_u64(bytes, 352)?,
            principal: read_u64(bytes, 360)?,
            realized_fee: read_u64(bytes, 368)?,
            direction,
        };
        if value.dealer_owner == value.counterparty_owner
            || value.current_obligation_revision == 0
            || value.candidate_obligation_revision
                != value
                    .current_obligation_revision
                    .checked_add(1)
                    .unwrap_or(0)
            || value.dealer_position_revision == 0
            || value.counterparty_position_revision == 0
            || value.generation == 0
            || value.principal == 0
            || (direction == ScenarioTradeDirectionV3::DealerPaysCounterparty
                && value.realized_fee > value.principal)
        {
            return Err(ScenarioTradeErrorV3::InvalidRequest);
        }
        let mut nonzero = false;
        let mut index = 0_u32;
        while index < width {
            let acquired = value.acquired(index)?;
            let delivered = value.delivered(index)?;
            if acquired != 0 && delivered != 0 {
                return Err(ScenarioTradeErrorV3::InvalidRequest);
            }
            nonzero |= acquired != 0 || delivered != 0;
            index = index
                .checked_add(1)
                .ok_or(ScenarioTradeErrorV3::InvalidRequest)?;
        }
        if !nonzero {
            return Err(ScenarioTradeErrorV3::InvalidRequest);
        }
        Ok(value)
    }

    /// Borrow the exact bytes whose digest is the common parent request digest.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Decode one acquired quantity.
    pub fn acquired(self, index: u32) -> Result<u64, ScenarioTradeErrorV3> {
        self.item(index, 0)
    }

    /// Decode one delivered quantity.
    pub fn delivered(self, index: u32) -> Result<u64, ScenarioTradeErrorV3> {
        self.item(index, 8)
    }

    /// Materialize exact runtime legs into caller-owned scratch.
    pub fn decode_legs(
        self,
        acquired: &mut [u64],
        delivered: &mut [u64],
    ) -> Result<(), ScenarioTradeErrorV3> {
        let width = usize::try_from(self.width).map_err(|_| ScenarioTradeErrorV3::WidthMismatch)?;
        if acquired.len() != width || delivered.len() != width {
            return Err(ScenarioTradeErrorV3::WidthMismatch);
        }
        for (index, (acquired_output, delivered_output)) in
            acquired.iter_mut().zip(delivered.iter_mut()).enumerate()
        {
            let coordinate =
                u32::try_from(index).map_err(|_| ScenarioTradeErrorV3::WidthMismatch)?;
            *acquired_output = self.acquired(coordinate)?;
            *delivered_output = self.delivered(coordinate)?;
        }
        Ok(())
    }

    fn item(self, index: u32, field: usize) -> Result<u64, ScenarioTradeErrorV3> {
        if index >= self.width {
            return Err(ScenarioTradeErrorV3::InvalidRequest);
        }
        let offset = usize::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(DEALER_SCENARIO_TRADE_ITEM_BYTES_V3))
            .and_then(|value| DEALER_SCENARIO_TRADE_HEADER_BYTES_V3.checked_add(value))
            .and_then(|value| value.checked_add(field))
            .ok_or(ScenarioTradeErrorV3::InvalidRequest)?;
        read_u64(self.bytes, offset)
    }
}

/// Authenticated chain snapshot for unsigned construction and execution rejoin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioTradeChainProjectionV3<'a> {
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// Canonical obligation PDA address.
    pub obligation_address: [u8; 32],
    /// Current authenticated obligation state.
    pub current_obligation: DealerObligationProjectionV3<'a>,
    /// Exact candidate obligation state proposed for write-last replacement.
    pub candidate_obligation: DealerObligationProjectionV3<'a>,
    /// Canonical Dealer Claims Position.
    pub dealer_position: ClaimsInventoryObservation<'a>,
    /// Canonical counterparty Claims Position.
    pub counterparty_position: ClaimsInventoryObservation<'a>,
    /// Exact counterparty external collateral account.
    pub counterparty_account: [u8; 32],
    /// Current Claims aggregate revision.
    pub claims_revision: u64,
    /// Current Core Market generation.
    pub generation: u64,
    /// Current slot/time coordinate.
    pub now: u64,
    /// Last admitted slot/time coordinate copied into the request.
    pub expires_at: u64,
    /// Whether the Market has entered terminal settlement.
    pub terminal: bool,
}

/// User-selected economic intent; all other request fields are chain-derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioTradeIntentV3<'a> {
    /// Principal direction.
    pub direction: ScenarioTradeDirectionV3,
    /// Positive exact quote principal atoms.
    pub principal: u64,
    /// Exact separately realized fee atoms.
    pub realized_fee: u64,
    /// Claims transferred from counterparty to Dealer.
    pub acquired: &'a [u64],
    /// Claims transferred from Dealer to counterparty.
    pub delivered: &'a [u64],
}

/// Metadata for one caller-buffer-backed unsigned request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsignedScenarioTradeRequestV3 {
    /// Exact initialized request width in the caller-owned output.
    pub request_bytes: usize,
    /// Exact CapabilityProgramV3 selected from the authenticated set.
    pub selected_program: ContentId,
}

/// Count-derived exact request width.
pub fn scenario_trade_request_bytes_v3(width: u32) -> Result<usize, ScenarioTradeErrorV3> {
    usize::try_from(width)
        .ok()
        .and_then(|value| value.checked_mul(DEALER_SCENARIO_TRADE_ITEM_BYTES_V3))
        .and_then(|value| DEALER_SCENARIO_TRADE_HEADER_BYTES_V3.checked_add(value))
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)
}

/// Build a chain-derived unsigned exact-fill request into caller-owned bytes.
pub fn build_scenario_trade_request_v3(
    chain: ScenarioTradeChainProjectionV3<'_>,
    intent: ScenarioTradeIntentV3<'_>,
    set: CapabilityProgramSetV1<'_>,
    output: &mut [u8],
) -> Result<UnsignedScenarioTradeRequestV3, ScenarioTradeErrorV3> {
    validate_projection(chain)?;
    validate_intent(chain, intent)?;
    let width =
        u32::try_from(intent.acquired.len()).map_err(|_| ScenarioTradeErrorV3::WidthMismatch)?;
    let expected = scenario_trade_request_bytes_v3(width)?;
    if output.len() != expected
        || set.selector_offset() != DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3
        || set.selector_width() != SelectorWidthV1::U16
    {
        return Err(ScenarioTradeErrorV3::ProgramSelection);
    }
    let mut selector = [0_u8; 12];
    selector[10..12].copy_from_slice(&DEALER_SCENARIO_TRADE_ACTION_V3.to_le_bytes());
    let selected_program = set
        .select(&selector)
        .map_err(|_| ScenarioTradeErrorV3::ProgramSelection)?;

    output.fill(0);
    write_bytes(output, 0, &DEALER_SCENARIO_TRADE_MAGIC_V3)?;
    write_bytes(output, 8, &DEALER_SCENARIO_TRADE_VERSION_V3.to_le_bytes())?;
    write_bytes(output, 10, &DEALER_SCENARIO_TRADE_ACTION_V3.to_le_bytes())?;
    write_bytes(output, 12, &width.to_le_bytes())?;
    for (offset, identity) in [
        (16, chain.release_set),
        (48, chain.market),
        (80, chain.child_root),
        (112, chain.obligation_address),
        (144, chain.current_obligation.state_digest()),
        (176, chain.candidate_obligation.state_digest()),
        (208, chain.dealer_position.position_owner),
        (240, chain.counterparty_position.position_owner),
        (272, chain.counterparty_account),
    ] {
        write_bytes(output, offset, &identity)?;
    }
    for (offset, value) in [
        (304, chain.current_obligation.revision()),
        (312, chain.candidate_obligation.revision()),
        (320, chain.dealer_position.revision),
        (328, chain.counterparty_position.revision),
        (336, chain.claims_revision),
        (344, chain.generation),
        (352, chain.expires_at),
        (360, intent.principal),
        (368, intent.realized_fee),
    ] {
        write_bytes(output, offset, &value.to_le_bytes())?;
    }
    *output
        .get_mut(376)
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)? = match intent.direction {
        ScenarioTradeDirectionV3::CounterpartyPaysDealer => 0,
        ScenarioTradeDirectionV3::DealerPaysCounterparty => 1,
    };
    for (index, (acquired, delivered)) in intent
        .acquired
        .iter()
        .zip(intent.delivered.iter())
        .enumerate()
    {
        let offset = DEALER_SCENARIO_TRADE_HEADER_BYTES_V3 + index * 16;
        write_bytes(output, offset, &acquired.to_le_bytes())?;
        write_bytes(output, offset + 8, &delivered.to_le_bytes())?;
    }
    let request = DealerScenarioTradeRequestV3::decode(output)?;
    authenticate_scenario_trade_request_v3(request, chain)?;
    Ok(UnsignedScenarioTradeRequestV3 {
        request_bytes: expected,
        selected_program,
    })
}

/// Rejoin one exact request to current authenticated chain projections.
pub fn authenticate_scenario_trade_request_v3(
    request: DealerScenarioTradeRequestV3<'_>,
    chain: ScenarioTradeChainProjectionV3<'_>,
) -> Result<(), ScenarioTradeErrorV3> {
    validate_projection(chain)?;
    if chain.terminal
        || chain.now > request.expires_at
        || request.expires_at != chain.expires_at
        || request.release_set != chain.release_set
        || request.market != chain.market
        || request.child_root != chain.child_root
        || request.obligation != chain.obligation_address
        || request.current_obligation_digest != chain.current_obligation.state_digest()
        || request.candidate_obligation_digest != chain.candidate_obligation.state_digest()
        || request.current_obligation_revision != chain.current_obligation.revision()
        || request.candidate_obligation_revision != chain.candidate_obligation.revision()
        || request.dealer_owner != chain.dealer_position.position_owner
        || request.counterparty_owner != chain.counterparty_position.position_owner
        || request.counterparty_account != chain.counterparty_account
        || request.dealer_position_revision != chain.dealer_position.revision
        || request.counterparty_position_revision != chain.counterparty_position.revision
        || request.claims_revision != chain.claims_revision
        || request.generation != chain.generation
        || usize::try_from(request.width).ok() != Some(chain.dealer_position.inventory.len())
    {
        return Err(ScenarioTradeErrorV3::InvalidProjection);
    }
    Ok(())
}

/// Authenticate, materialize runtime legs, and invoke the sole physical composer.
#[allow(clippy::too_many_arguments)]
pub fn prepare_scenario_trade_v3(
    request: DealerScenarioTradeRequestV3<'_>,
    chain: ScenarioTradeChainProjectionV3<'_>,
    context: ScenarioComposerContextV3,
    frame: ScenarioCollateralFrameV3,
    acquired: &mut [u64],
    delivered: &mut [u64],
    obligations_before: &mut [u64],
    obligations_after: &mut [u64],
    post_inventory: &mut [u64],
    post_equity: &mut [i128],
    custody_output: &mut [Option<super::v3_composer::ScenarioCustodyEffectV3>],
) -> Result<ScenarioAtomicPlanV3, ScenarioTradeErrorV3> {
    authenticate_scenario_trade_request_v3(request, chain)?;
    if context.trading_program != chain.trading_program
        || context.release_set != request.release_set
        || context.market != request.market
        || context.child_root != request.child_root
        || context.obligation_account != request.obligation
        || context.generation != request.generation
        || context.parent_request_digest != hash(request.bytes()).to_bytes()
        || frame.counterparty_owner != request.counterparty_owner
        || frame.counterparty_account != request.counterparty_account
    {
        return Err(ScenarioTradeErrorV3::InvalidProjection);
    }
    request.decode_legs(acquired, delivered)?;
    let direction = match request.direction {
        ScenarioTradeDirectionV3::CounterpartyPaysDealer => {
            ScenarioQuoteDirectionV3::CounterpartyPaysDealer
        }
        ScenarioTradeDirectionV3::DealerPaysCounterparty => {
            ScenarioQuoteDirectionV3::DealerPaysCounterparty
        }
    };
    prepare_scenario_atomic_v3(
        context,
        frame,
        chain.current_obligation,
        chain.candidate_obligation,
        request.candidate_obligation_digest,
        chain.claims_revision,
        ScenarioFillInputV3 {
            dealer_position: chain.dealer_position,
            counterparty_position_revision: chain.counterparty_position.revision,
            acquired,
            delivered,
            quote: ScenarioQuoteLegV3 {
                direction,
                principal: request.principal,
                realized_fee: request.realized_fee,
            },
        },
        obligations_before,
        obligations_after,
        post_inventory,
        post_equity,
        custody_output,
    )
    .map_err(ScenarioTradeErrorV3::from)
}

fn validate_projection(
    chain: ScenarioTradeChainProjectionV3<'_>,
) -> Result<(), ScenarioTradeErrorV3> {
    for identity in [
        chain.trading_program,
        chain.release_set,
        chain.market,
        chain.child_root,
        chain.obligation_address,
        chain.counterparty_account,
    ] {
        if identity == [0; 32] {
            return Err(ScenarioTradeErrorV3::InvalidProjection);
        }
    }
    let current = chain.current_obligation;
    let candidate = chain.candidate_obligation;
    let dealer = chain.dealer_position;
    let counterparty = chain.counterparty_position;
    let expected_obligation = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &chain.child_root],
        &Pubkey::new_from_array(chain.trading_program),
    )
    .0
    .to_bytes();
    if chain.terminal
        || chain.now > chain.expires_at
        || chain.generation == 0
        || chain.obligation_address != expected_obligation
        || current.child_root() != chain.child_root
        || candidate.child_root() != chain.child_root
        || current.descriptor(0) != candidate.descriptor(0)
        || current.revision().checked_add(1) != Some(candidate.revision())
        || current.width() != candidate.width()
        || current.total_equity_shares() != candidate.total_equity_shares()
        || current.position_owner() != dealer.position_owner
        || dealer.position_owner == counterparty.position_owner
        || dealer.market_id != chain.market
        || counterparty.market_id != chain.market
        || dealer.product_id != counterparty.product_id
        || dealer.liability_basis_id != counterparty.liability_basis_id
        || current.descriptor(0).product_id != dealer.product_id
        || current.descriptor(0).liability_basis_id != dealer.liability_basis_id
        || dealer.revision == 0
        || counterparty.revision == 0
        || dealer.inventory.len() != counterparty.inventory.len()
        || usize::try_from(current.width()).ok() != Some(dealer.inventory.len())
    {
        return Err(ScenarioTradeErrorV3::InvalidProjection);
    }
    Ok(())
}

fn validate_intent(
    chain: ScenarioTradeChainProjectionV3<'_>,
    intent: ScenarioTradeIntentV3<'_>,
) -> Result<(), ScenarioTradeErrorV3> {
    let width = chain.dealer_position.inventory.len();
    if intent.principal == 0
        || intent.acquired.len() != width
        || intent.delivered.len() != width
        || (intent.direction == ScenarioTradeDirectionV3::DealerPaysCounterparty
            && intent.realized_fee > intent.principal)
    {
        return Err(ScenarioTradeErrorV3::InvalidIntent);
    }
    let mut nonzero = false;
    for (acquired, delivered) in intent.acquired.iter().zip(intent.delivered.iter()) {
        if *acquired != 0 && *delivered != 0 {
            return Err(ScenarioTradeErrorV3::InvalidIntent);
        }
        nonzero |= *acquired != 0 || *delivered != 0;
    }
    if !nonzero {
        return Err(ScenarioTradeErrorV3::InvalidIntent);
    }
    Ok(())
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8, ScenarioTradeErrorV3> {
    bytes
        .get(offset)
        .copied()
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ScenarioTradeErrorV3> {
    bytes
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ScenarioTradeErrorV3> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, ScenarioTradeErrorV3> {
    bytes
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)
}

fn read_identity(bytes: &[u8], offset: usize) -> Result<[u8; 32], ScenarioTradeErrorV3> {
    let value = bytes
        .get(offset..offset + 32)
        .and_then(|value| value.try_into().ok())
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)?;
    if value == [0; 32] {
        Err(ScenarioTradeErrorV3::InvalidRequest)
    } else {
        Ok(value)
    }
}

fn write_bytes(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<(), ScenarioTradeErrorV3> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)?;
    bytes
        .get_mut(offset..end)
        .ok_or(ScenarioTradeErrorV3::InvalidRequest)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dealer::v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3, DEALER_OBLIGATION_VERSION_V3,
    };

    fn obligation_bytes(
        child_root: [u8; 32],
        revision: u64,
        obligations: &[u64],
    ) -> std::vec::Vec<u8> {
        let mut bytes = std::vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + obligations.len() * 8];
        bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
        bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
        bytes[12..16].copy_from_slice(&(obligations.len() as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(&revision.to_le_bytes());
        for (offset, identity) in [
            (24, [2; 32]),
            (56, [3; 32]),
            (88, [4; 32]),
            (120, [5; 32]),
            (152, child_root),
        ] {
            bytes[offset..offset + 32].copy_from_slice(&identity);
        }
        bytes[184..192].copy_from_slice(&10_u64.to_le_bytes());
        for (index, obligation) in obligations.iter().enumerate() {
            let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
            bytes[offset..offset + 8].copy_from_slice(&obligation.to_le_bytes());
        }
        bytes
    }

    fn program_set() -> std::vec::Vec<u8> {
        let mut bytes = std::vec![0; 72];
        bytes[..8].copy_from_slice(b"DCLTCPS1");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&DEALER_SCENARIO_TRADE_SELECTOR_OFFSET_V3.to_le_bytes());
        bytes[16] = 2;
        bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
        bytes[32..36].copy_from_slice(&u32::from(DEALER_SCENARIO_TRADE_ACTION_V3).to_le_bytes());
        bytes[36..68].copy_from_slice(&[42; 32]);
        bytes
    }

    #[test]
    fn runtime_width_request_is_chain_derived_and_exact() {
        let trading_program = [1; 32];
        let child_root = [7; 32];
        let dealer_inventory = [2, 10, 0];
        let counterparty_inventory = [20, 5, 9];
        let current_bytes = obligation_bytes(child_root, 7, &[12, 20, 10]);
        let candidate_bytes = obligation_bytes(child_root, 8, &[10, 19, 13]);
        let current = DealerObligationProjectionV3::decode(&current_bytes).expect("current");
        let candidate = DealerObligationProjectionV3::decode(&candidate_bytes).expect("candidate");
        let obligation_address = Pubkey::find_program_address(
            &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &child_root],
            &Pubkey::new_from_array(trading_program),
        )
        .0
        .to_bytes();
        let chain = ScenarioTradeChainProjectionV3 {
            trading_program,
            release_set: [6; 32],
            market: [2; 32],
            child_root,
            obligation_address,
            current_obligation: current,
            candidate_obligation: candidate,
            dealer_position: ClaimsInventoryObservation {
                market_id: [2; 32],
                product_id: [3; 32],
                liability_basis_id: [4; 32],
                position_owner: [5; 32],
                revision: 9,
                inventory: &dealer_inventory,
            },
            counterparty_position: ClaimsInventoryObservation {
                market_id: [2; 32],
                product_id: [3; 32],
                liability_basis_id: [4; 32],
                position_owner: [8; 32],
                revision: 11,
                inventory: &counterparty_inventory,
            },
            counterparty_account: [9; 32],
            claims_revision: 0,
            generation: 17,
            now: 20,
            expires_at: 25,
            terminal: false,
        };
        let intent = ScenarioTradeIntentV3 {
            direction: ScenarioTradeDirectionV3::CounterpartyPaysDealer,
            principal: 10,
            realized_fee: 1,
            acquired: &[3, 0, 4],
            delivered: &[0, 1, 0],
        };
        let set_bytes = program_set();
        let set = CapabilityProgramSetV1::decode(&set_bytes).expect("set");
        let mut output = std::vec![0; scenario_trade_request_bytes_v3(3).expect("width")];
        let unsigned =
            build_scenario_trade_request_v3(chain, intent, set, &mut output).expect("request");
        assert_eq!(unsigned.request_bytes, 432);
        assert_eq!(unsigned.selected_program.to_bytes(), [42; 32]);
        let request = DealerScenarioTradeRequestV3::decode(&output).expect("decode");
        assert_eq!(request.width, 3);
        assert_eq!(request.acquired(2), Ok(4));
        assert_eq!(request.delivered(1), Ok(1));
        assert_eq!(
            authenticate_scenario_trade_request_v3(request, chain),
            Ok(())
        );

        let mut substituted = request;
        substituted.candidate_obligation_digest[0] ^= 1;
        assert_eq!(
            authenticate_scenario_trade_request_v3(substituted, chain),
            Err(ScenarioTradeErrorV3::InvalidProjection)
        );
    }

    #[test]
    fn noncanonical_round_trip_and_expired_projection_refuse() {
        let width = 2_u32;
        let mut bytes = std::vec![0; scenario_trade_request_bytes_v3(width).expect("width")];
        bytes[..8].copy_from_slice(&DEALER_SCENARIO_TRADE_MAGIC_V3);
        bytes[8..10].copy_from_slice(&DEALER_SCENARIO_TRADE_VERSION_V3.to_le_bytes());
        bytes[10..12].copy_from_slice(&DEALER_SCENARIO_TRADE_ACTION_V3.to_le_bytes());
        bytes[12..16].copy_from_slice(&width.to_le_bytes());
        for offset in [16, 48, 80, 112, 144, 176, 208, 240, 272] {
            bytes[offset..offset + 32].copy_from_slice(&[u8::try_from(offset).unwrap_or(1); 32]);
        }
        for (offset, value) in [
            (304, 1_u64),
            (312, 2),
            (320, 1),
            (328, 1),
            (336, 1),
            (344, 1),
            (352, 1),
            (360, 1),
            (368, 0),
        ] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[384..392].copy_from_slice(&1_u64.to_le_bytes());
        bytes[392..400].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            DealerScenarioTradeRequestV3::decode(&bytes),
            Err(ScenarioTradeErrorV3::InvalidRequest)
        );
        bytes[392..400].copy_from_slice(&0_u64.to_le_bytes());
        assert!(DealerScenarioTradeRequestV3::decode(&bytes).is_ok());
        bytes[377] = 1;
        assert_eq!(
            DealerScenarioTradeRequestV3::decode(&bytes),
            Err(ScenarioTradeErrorV3::InvalidRequest)
        );
    }
}
