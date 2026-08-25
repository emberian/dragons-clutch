#![allow(clippy::indexing_slicing)]

use dclutch_realm_contract::PositionV1;

use crate::state::{
    DirectIntentRecordV2, DirectIntentV2, InlineParticipantAccountsV2, MakerReplayRootV2,
    ParticipantAccountsV2, RecordAfterFillV2, ReplayRootStateV2, Side, VenueFeePolicyV3,
    position_matches, runtime_position_matches, venue_authorized,
};
use crate::{DirectPositionV2, Error, Result, fee, quote, width};

/// Inputs to an immediate signed two-party FOK/IOC execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineOrdinaryMatchV2<const N: usize> {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted `Clock::get()` slot.
    pub slot: u64,
    /// Seller replay root, or canonical first-use absence.
    pub seller_replay_root: ReplayRootStateV2,
    /// Buyer replay root, or canonical first-use absence.
    pub buyer_replay_root: ReplayRootStateV2,
    /// Separate System payer for any first-use replay-root creation.
    pub root_creation_payer: [u8; 32],
    /// Exact signed seller intent embedded in adapter data.
    pub seller_intent: DirectIntentV2,
    /// Exact signed buyer intent embedded in adapter data.
    pub buyer_intent: DirectIntentV2,
    /// Sealed preceding-native-program seller authorization.
    pub seller_authorization: crate::adapter::Ed25519AuthorizationV2,
    /// Sealed preceding-native-program buyer authorization.
    pub buyer_authorization: crate::adapter::Ed25519AuthorizationV2,
    /// Exact seller root/Position/collateral accounts.
    pub seller_accounts: InlineParticipantAccountsV2,
    /// Exact buyer root/Position/collateral accounts.
    pub buyer_accounts: InlineParticipantAccountsV2,
    /// Seller Position debited atomically.
    pub seller_position: PositionV1<N>,
    /// Buyer Position credited atomically.
    pub buyer_position: PositionV1<N>,
    /// Realm-selected collateral mint.
    pub collateral_mint: [u8; 32],
    /// Authenticated buyer source delegate projection.
    pub buyer_debit_authority: crate::adapter::BuyDebitAuthorityV2,
    /// Immediate positive fill.
    pub fill: u64,
    /// Exact execution price.
    pub execution_price: u64,
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated SHA-256 digest of the exact policy bytes.
    pub fee_config_digest: [u8; 32],
    /// Actual fee recipient token account.
    pub fee_recipient_account: [u8; 32],
}

/// Exact effects of immediate ordinary execution without live accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineOrdinarySettlementV2<const N: usize> {
    /// Seller replay root after nonce consumption; live count unchanged.
    pub seller_replay_root: MakerReplayRootV2,
    /// Buyer replay root after nonce consumption; live count unchanged.
    pub buyer_replay_root: MakerReplayRootV2,
    /// Seller Position after direct claim debit.
    pub seller_position: PositionV1<N>,
    /// Buyer Position after direct claim credit.
    pub buyer_position: PositionV1<N>,
    /// Gross buyer collateral debit and seller credit.
    pub gross_collateral_transfer: u64,
    /// Direct buyer fee debit.
    pub venue_fee_transfer: u64,
    /// Total direct buyer collateral debit.
    pub buyer_total_collateral_debit: u64,
}

/// Check an atomic immediate ordinary FOK/IOC execution.
pub fn settle_inline_ordinary_v2<const N: usize>(
    input: InlineOrdinaryMatchV2<N>,
) -> Result<InlineOrdinarySettlementV2<N>> {
    width(N)?;
    crate::adapter::require_market_phase_v2(
        crate::adapter::AdapterActionV2::InlineOrdinary,
        input.phase,
    )?;
    let ask = input.seller_intent;
    let bid = input.buyer_intent;
    inline_common(
        ask,
        input.seller_authorization,
        input.seller_accounts,
        input.seller_position,
        input.slot,
    )?;
    inline_common(
        bid,
        input.buyer_authorization,
        input.buyer_accounts,
        input.buyer_position,
        input.slot,
    )?;
    if ask.side() != Side::Sell
        || bid.side() != Side::Buy
        || ask.market() != bid.market()
        || ask.generation() != bid.generation()
        || ask.outcome() != bid.outcome()
    {
        return Err(Error::IncompatibleSides);
    }
    if ask.maker() == bid.maker() || inline_alias(input.seller_accounts, input.buyer_accounts) {
        return Err(Error::Alias);
    }
    venue_authorized(
        ask,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    venue_authorized(
        bid,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    if input.execution_price < ask.limit_price() || input.execution_price > bid.limit_price() {
        return Err(Error::PriceIncompatible);
    }
    let gross = quote(input.fill, input.execution_price)?;
    let venue_fee = fee(gross, input.fee_policy.fee_basis_points())?;
    let total = gross
        .checked_add(venue_fee)
        .ok_or(Error::ArithmeticOverflow)?;
    crate::adapter::validate_inline_buy_debit_authority_v2(
        input.buyer_debit_authority,
        bid,
        input.buyer_accounts.replay_root,
        input.collateral_mint,
        total,
    )?;
    let seller_replay_root = input
        .seller_replay_root
        .open_for_intent(ask, input.root_creation_payer)?
        .consume_inline(ask, input.fill)?;
    let buyer_replay_root = input
        .buyer_replay_root
        .open_for_intent(bid, input.root_creation_payer)?
        .consume_inline(bid, input.fill)?;
    let outcome = usize::from(ask.outcome());
    if outcome >= N {
        return Err(Error::InvalidOutcome);
    }
    let mut seller_position = input.seller_position;
    let mut buyer_position = input.buyer_position;
    seller_position
        .debit_outcome(outcome, input.fill)
        .map_err(crate::position_error)?;
    buyer_position
        .credit_outcome(outcome, input.fill)
        .map_err(crate::position_error)?;
    Ok(InlineOrdinarySettlementV2 {
        seller_replay_root,
        buyer_replay_root,
        seller_position,
        buyer_position,
        gross_collateral_transfer: gross,
        venue_fee_transfer: venue_fee,
        buyer_total_collateral_debit: total,
    })
}

/// Runtime-width inputs to an immediate signed two-party FOK/IOC execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeInlineOrdinaryMatchV2 {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted slot.
    pub slot: u64,
    /// Seller replay root, or canonical first-use absence.
    pub seller_replay_root: ReplayRootStateV2,
    /// Buyer replay root, or canonical first-use absence.
    pub buyer_replay_root: ReplayRootStateV2,
    /// Separate System payer for first-use replay-root creation.
    pub root_creation_payer: [u8; 32],
    /// Exact signed seller intent.
    pub seller_intent: DirectIntentV2,
    /// Exact signed buyer intent.
    pub buyer_intent: DirectIntentV2,
    /// Sealed seller authorization.
    pub seller_authorization: crate::adapter::Ed25519AuthorizationV2,
    /// Sealed buyer authorization.
    pub buyer_authorization: crate::adapter::Ed25519AuthorizationV2,
    /// Exact seller accounts.
    pub seller_accounts: InlineParticipantAccountsV2,
    /// Exact buyer accounts.
    pub buyer_accounts: InlineParticipantAccountsV2,
    /// Runtime-width seller Position.
    pub seller_position: DirectPositionV2,
    /// Runtime-width buyer Position.
    pub buyer_position: DirectPositionV2,
    /// Realm collateral mint.
    pub collateral_mint: [u8; 32],
    /// Authenticated buyer source delegate.
    pub buyer_debit_authority: crate::adapter::BuyDebitAuthorityV2,
    /// Immediate positive fill.
    pub fill: u64,
    /// Exact execution price.
    pub execution_price: u64,
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated policy digest.
    pub fee_config_digest: [u8; 32],
    /// Actual fee recipient token account.
    pub fee_recipient_account: [u8; 32],
}

/// Runtime-width effects of immediate ordinary execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeInlineOrdinarySettlementV2 {
    /// Seller replay root after nonce consumption.
    pub seller_replay_root: MakerReplayRootV2,
    /// Buyer replay root after nonce consumption.
    pub buyer_replay_root: MakerReplayRootV2,
    /// Seller Position after claim debit.
    pub seller_position: DirectPositionV2,
    /// Buyer Position after claim credit.
    pub buyer_position: DirectPositionV2,
    /// Gross buyer debit and seller credit.
    pub gross_collateral_transfer: u64,
    /// Buyer fee debit.
    pub venue_fee_transfer: u64,
    /// Total buyer collateral debit.
    pub buyer_total_collateral_debit: u64,
}

/// Check immediate ordinary execution without width specialization.
pub fn settle_inline_ordinary_runtime_v2(
    input: RuntimeInlineOrdinaryMatchV2,
) -> Result<RuntimeInlineOrdinarySettlementV2> {
    crate::adapter::require_market_phase_v2(
        crate::adapter::AdapterActionV2::InlineOrdinary,
        input.phase,
    )?;
    let ask = input.seller_intent;
    let bid = input.buyer_intent;
    inline_common_runtime(
        ask,
        input.seller_authorization,
        input.seller_accounts,
        input.seller_position,
        input.slot,
    )?;
    inline_common_runtime(
        bid,
        input.buyer_authorization,
        input.buyer_accounts,
        input.buyer_position,
        input.slot,
    )?;
    if input.seller_position.outcome_count() != input.buyer_position.outcome_count()
        || ask.side() != Side::Sell
        || bid.side() != Side::Buy
        || ask.market() != bid.market()
        || ask.generation() != bid.generation()
        || ask.outcome() != bid.outcome()
    {
        return Err(Error::IncompatibleSides);
    }
    if ask.maker() == bid.maker() || inline_alias(input.seller_accounts, input.buyer_accounts) {
        return Err(Error::Alias);
    }
    venue_authorized(
        ask,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    venue_authorized(
        bid,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    if input.execution_price < ask.limit_price() || input.execution_price > bid.limit_price() {
        return Err(Error::PriceIncompatible);
    }
    let gross = quote(input.fill, input.execution_price)?;
    let venue_fee = fee(gross, input.fee_policy.fee_basis_points())?;
    let total = gross
        .checked_add(venue_fee)
        .ok_or(Error::ArithmeticOverflow)?;
    crate::adapter::validate_inline_buy_debit_authority_v2(
        input.buyer_debit_authority,
        bid,
        input.buyer_accounts.replay_root,
        input.collateral_mint,
        total,
    )?;
    let seller_replay_root = input
        .seller_replay_root
        .open_for_intent(ask, input.root_creation_payer)?
        .consume_inline(ask, input.fill)?;
    let buyer_replay_root = input
        .buyer_replay_root
        .open_for_intent(bid, input.root_creation_payer)?
        .consume_inline(bid, input.fill)?;
    let outcome = usize::from(ask.outcome());
    if outcome >= usize::from(input.seller_position.outcome_count()) {
        return Err(Error::InvalidOutcome);
    }
    let mut seller_position = input.seller_position;
    let mut buyer_position = input.buyer_position;
    seller_position.debit_outcome(outcome, input.fill)?;
    buyer_position.credit_outcome(outcome, input.fill)?;
    Ok(RuntimeInlineOrdinarySettlementV2 {
        seller_replay_root,
        buyer_replay_root,
        seller_position,
        buyer_position,
        gross_collateral_transfer: gross,
        venue_fee_transfer: venue_fee,
        buyer_total_collateral_debit: total,
    })
}

/// Inputs to an immediate N=2 complementary buy or sell execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineComplementaryMatchV2<const N: usize> {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted `Clock::get()` slot.
    pub slot: u64,
    /// Common Buy or Sell direction.
    pub side: Side,
    /// Replay roots in canonical outcome order.
    pub replay_roots: [ReplayRootStateV2; N],
    /// Separate System payer for any first-use replay-root creation.
    pub root_creation_payer: [u8; 32],
    /// Exact signed intents in canonical outcome order.
    pub intents: [DirectIntentV2; N],
    /// Sealed native authorizations in canonical outcome order.
    pub authorizations: [crate::adapter::Ed25519AuthorizationV2; N],
    /// Exact immediate account triples in canonical outcome order.
    pub accounts: [InlineParticipantAccountsV2; N],
    /// Positions debited or credited atomically.
    pub positions: [PositionV1<N>; N],
    /// Realm-selected collateral mint.
    pub collateral_mint: [u8; 32],
    /// Authenticated Buy source delegates; all absent for a Sell merge.
    pub buy_debit_authorities: [Option<crate::adapter::BuyDebitAuthorityV2>; N],
    /// Common immediate fill.
    pub fill: u64,
    /// Exact prices summing to one collateral atom.
    pub execution_prices: [u64; N],
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated SHA-256 digest of the exact policy bytes.
    pub fee_config_digest: [u8; 32],
    /// Actual fee recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Exact effects of immediate N=2 complementary execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineComplementarySettlementV2<const N: usize> {
    /// Replay roots after nonce consumption.
    pub replay_roots: [MakerReplayRootV2; N],
    /// Positions after indexed direct claim movement.
    pub positions: [PositionV1<N>; N],
    /// Gross collateral debits for buys or credits for sells.
    pub gross_collateral: [u64; N],
    /// Fee debits in canonical order.
    pub fees: [u64; N],
    /// Net seller credits; zero for a buy split.
    pub net_seller_credits: [u64; N],
    /// Market vault credit for Buy or debit for Sell.
    pub market_vault_transfer: u64,
    /// Aggregate venue fee transfer.
    pub venue_fee_transfer: u64,
}

/// Check an atomic immediate complementary execution. The measured packet
/// profile admits exactly N=2; N>=3 refuses before semantic mutation.
pub fn settle_inline_complementary_v2<const N: usize>(
    input: InlineComplementaryMatchV2<N>,
) -> Result<InlineComplementarySettlementV2<N>> {
    width(N)?;
    if N != 2 {
        return Err(Error::InvalidInlineWidth);
    }
    if input.fill == 0 {
        return Err(Error::ZeroQuantity);
    }
    let action = match input.side {
        Side::Buy => crate::adapter::AdapterActionV2::InlineSplit,
        Side::Sell => crate::adapter::AdapterActionV2::InlineMerge,
    };
    crate::adapter::require_market_phase_v2(action, input.phase)?;
    let first = *input.intents.first().ok_or(Error::InvalidInlineWidth)?;
    let seed_root = input
        .replay_roots
        .first()
        .copied()
        .ok_or(Error::InvalidInlineWidth)?
        .open_for_intent(first, input.root_creation_payer)?;
    let mut roots = [seed_root; N];
    let mut positions = input.positions;
    let mut gross = [0; N];
    let mut fees = [0; N];
    let mut net = [0; N];
    let mut price_sum = 0_u64;
    let mut gross_sum = 0_u64;
    let mut fee_sum = 0_u64;
    for index in 0..N {
        let intent = input.intents[index];
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        inline_common(
            intent,
            input.authorizations[index],
            input.accounts[index],
            positions[index],
            input.slot,
        )?;
        if intent.side() != input.side
            || intent.market() != first.market()
            || intent.generation() != first.generation()
            || intent.outcome() != expected
        {
            return Err(Error::NonCanonicalComplement);
        }
        for prior in 0..index {
            if intent.maker() == input.intents[prior].maker()
                || inline_alias(input.accounts[index], input.accounts[prior])
            {
                return Err(Error::Alias);
            }
        }
        venue_authorized(
            intent,
            input.fee_policy,
            input.fee_config_digest,
            input.fee_recipient_account,
        )?;
        let price = input.execution_prices[index];
        match input.side {
            Side::Buy if price > intent.limit_price() => return Err(Error::PriceIncompatible),
            Side::Sell if price < intent.limit_price() => return Err(Error::PriceIncompatible),
            Side::Buy | Side::Sell => {}
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        fees[index] = fee(gross[index], input.fee_policy.fee_basis_points())?;
        let exact_debit = gross[index]
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
        match input.side {
            Side::Buy => crate::adapter::validate_inline_buy_debit_authority_v2(
                input.buy_debit_authorities[index].ok_or(Error::InvalidBuyDebitAuthority)?,
                intent,
                input.accounts[index].replay_root,
                input.collateral_mint,
                exact_debit,
            )?,
            Side::Sell if input.buy_debit_authorities[index].is_some() => {
                return Err(Error::InvalidBuyDebitAuthority);
            }
            Side::Sell => {}
        }
        roots[index] = input.replay_roots[index]
            .open_for_intent(intent, input.root_creation_payer)?
            .consume_inline(intent, input.fill)?;
        match input.side {
            Side::Buy => positions[index].credit_outcome(index, input.fill),
            Side::Sell => positions[index].debit_outcome(index, input.fill),
        }
        .map_err(crate::position_error)?;
        if input.side == Side::Sell {
            net[index] = gross[index]
                .checked_sub(fees[index])
                .ok_or(Error::ArithmeticOverflow)?;
        }
        gross_sum = gross_sum
            .checked_add(gross[index])
            .ok_or(Error::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != crate::PRICE_SCALE || gross_sum != input.fill {
        return Err(Error::SplitFundingMismatch);
    }
    Ok(InlineComplementarySettlementV2 {
        replay_roots: roots,
        positions,
        gross_collateral: gross,
        fees,
        net_seller_credits: net,
        market_vault_transfer: input.fill,
        venue_fee_transfer: fee_sum,
    })
}

fn inline_common<const N: usize>(
    intent: DirectIntentV2,
    authorization: crate::adapter::Ed25519AuthorizationV2,
    accounts: InlineParticipantAccountsV2,
    position: PositionV1<N>,
    slot: u64,
) -> Result<()> {
    authorization.authorizes_inline(intent)?;
    accounts.validate(intent)?;
    position_matches(position, intent)?;
    if slot < intent.valid_from_slot() || slot > intent.valid_through_slot() {
        return Err(Error::IntentExpired);
    }
    match intent.lifecycle() {
        crate::IntentLifecycleV2::InlineFillOrKill
        | crate::IntentLifecycleV2::InlineImmediateOrCancel => Ok(()),
        crate::IntentLifecycleV2::Registered => Err(Error::IntentLifecycleMismatch),
    }
}

fn inline_common_runtime(
    intent: DirectIntentV2,
    authorization: crate::adapter::Ed25519AuthorizationV2,
    accounts: InlineParticipantAccountsV2,
    position: DirectPositionV2,
    slot: u64,
) -> Result<()> {
    authorization.authorizes_inline(intent)?;
    accounts.validate(intent)?;
    runtime_position_matches(position, intent)?;
    if slot < intent.valid_from_slot() || slot > intent.valid_through_slot() {
        return Err(Error::IntentExpired);
    }
    match intent.lifecycle() {
        crate::IntentLifecycleV2::InlineFillOrKill
        | crate::IntentLifecycleV2::InlineImmediateOrCancel => Ok(()),
        crate::IntentLifecycleV2::Registered => Err(Error::IntentLifecycleMismatch),
    }
}

fn inline_alias(left: InlineParticipantAccountsV2, right: InlineParticipantAccountsV2) -> bool {
    let left_keys = [left.replay_root, left.position, left.collateral];
    let right_keys = [right.replay_root, right.position, right.collateral];
    left_keys.iter().any(|key| right_keys.contains(key))
}

/// Inputs to an ordinary permissionless persisted-record transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinaryMatchV2<const N: usize> {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted `Clock::get()` slot.
    pub slot: u64,
    /// Seller maker replay root.
    pub seller_replay_root: MakerReplayRootV2,
    /// Buyer maker replay root.
    pub buyer_replay_root: MakerReplayRootV2,
    /// Program-owned seller live record.
    pub seller_record: DirectIntentRecordV2,
    /// Program-owned buyer live record.
    pub buyer_record: DirectIntentRecordV2,
    /// Exact seller physical accounts.
    pub seller_accounts: ParticipantAccountsV2,
    /// Exact buyer physical accounts.
    pub buyer_accounts: ParticipantAccountsV2,
    /// Seller Position; claims were already reserved at registration.
    pub seller_position: PositionV1<N>,
    /// Buyer Position to credit.
    pub buyer_position: PositionV1<N>,
    /// Realm-selected collateral mint.
    pub collateral_mint: [u8; 32],
    /// Authenticated registered Buy escrow authority projection.
    pub buyer_escrow_authority: crate::adapter::EscrowAuthorityV2,
    /// Matcher-selected positive fill.
    pub fill: u64,
    /// Matcher-selected exact scaled execution price.
    pub execution_price: u64,
    /// Canonical program-owned fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated SHA-256 digest of the exact policy bytes.
    pub fee_config_digest: [u8; 32],
    /// Actual fee-recipient token account at canonical role.
    pub fee_recipient_account: [u8; 32],
}

/// Exact effects of one ordinary transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinarySettlementV2<const N: usize> {
    /// Updated seller replay root.
    pub seller_replay_root: MakerReplayRootV2,
    /// Updated buyer replay root.
    pub buyer_replay_root: MakerReplayRootV2,
    /// Seller live record replacement or close effects.
    pub seller_record: RecordAfterFillV2,
    /// Buyer live record replacement or close effects.
    pub buyer_record: RecordAfterFillV2,
    /// Seller Position unchanged after registration reservation.
    pub seller_position: PositionV1<N>,
    /// Buyer Position after claim credit.
    pub buyer_position: PositionV1<N>,
    /// Selected outcome transfer.
    pub outcome_quantity: u64,
    /// Gross buyer-escrow release to seller account.
    pub seller_collateral_credit: u64,
    /// Buyer-escrow fee release to venue.
    pub venue_fee_transfer: u64,
    /// Total buyer escrow debit.
    pub buyer_escrow_debit: u64,
}

/// Check one atomic ordinary permissionless match.
pub fn settle_ordinary_v2<const N: usize>(
    input: OrdinaryMatchV2<N>,
) -> Result<OrdinarySettlementV2<N>> {
    width(N)?;
    crate::adapter::require_market_phase_v2(
        crate::adapter::AdapterActionV2::Ordinary,
        input.phase,
    )?;
    let ask = input.seller_record.intent();
    let bid = input.buyer_record.intent();
    if ask.side() != Side::Sell
        || bid.side() != Side::Buy
        || ask.market() != bid.market()
        || ask.generation() != bid.generation()
        || ask.outcome() != bid.outcome()
    {
        return Err(Error::IncompatibleSides);
    }
    if ask.maker() == bid.maker() {
        return Err(Error::Alias);
    }
    input.seller_accounts.validate(ask)?;
    input.buyer_accounts.validate(bid)?;
    crate::adapter::validate_registered_escrow_authority_v2(
        input.buyer_escrow_authority,
        input.buyer_record,
        input.buyer_accounts.record,
        input.buyer_accounts.escrow,
        input.collateral_mint,
    )?;
    distinct_participants(&[input.seller_accounts, input.buyer_accounts])?;
    position_matches(input.seller_position, ask)?;
    position_matches(input.buyer_position, bid)?;
    venue_authorized(
        ask,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    venue_authorized(
        bid,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    if input.execution_price < ask.limit_price() || input.execution_price > bid.limit_price() {
        return Err(Error::PriceIncompatible);
    }
    let gross = quote(input.fill, input.execution_price)?;
    let (seller_record, seller_replay_root, seller_fee) =
        input
            .seller_record
            .consume(input.seller_replay_root, input.slot, input.fill, 0, 0)?;
    if seller_fee != 0 {
        return Err(Error::InvalidReservation);
    }
    let (buyer_record, buyer_replay_root, venue_fee) = input.buyer_record.consume(
        input.buyer_replay_root,
        input.slot,
        input.fill,
        gross,
        gross,
    )?;
    let buyer_debit = gross
        .checked_add(venue_fee)
        .ok_or(Error::ArithmeticOverflow)?;
    let outcome = usize::from(ask.outcome());
    if outcome >= N {
        return Err(Error::InvalidOutcome);
    }
    let mut buyer_position = input.buyer_position;
    buyer_position
        .credit_outcome(outcome, input.fill)
        .map_err(crate::position_error)?;
    Ok(OrdinarySettlementV2 {
        seller_replay_root,
        buyer_replay_root,
        seller_record,
        buyer_record,
        seller_position: input.seller_position,
        buyer_position,
        outcome_quantity: input.fill,
        seller_collateral_credit: gross,
        venue_fee_transfer: venue_fee,
        buyer_escrow_debit: buyer_debit,
    })
}

/// Runtime-width inputs to one persisted-record ordinary transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeOrdinaryMatchV2 {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted slot.
    pub slot: u64,
    /// Seller maker replay root.
    pub seller_replay_root: MakerReplayRootV2,
    /// Buyer maker replay root.
    pub buyer_replay_root: MakerReplayRootV2,
    /// Program-owned seller record.
    pub seller_record: DirectIntentRecordV2,
    /// Program-owned buyer record.
    pub buyer_record: DirectIntentRecordV2,
    /// Exact seller physical accounts.
    pub seller_accounts: ParticipantAccountsV2,
    /// Exact buyer physical accounts.
    pub buyer_accounts: ParticipantAccountsV2,
    /// Runtime-width seller Position.
    pub seller_position: DirectPositionV2,
    /// Runtime-width buyer Position.
    pub buyer_position: DirectPositionV2,
    /// Realm collateral mint.
    pub collateral_mint: [u8; 32],
    /// Authenticated buyer escrow authority.
    pub buyer_escrow_authority: crate::adapter::EscrowAuthorityV2,
    /// Matcher-selected positive fill.
    pub fill: u64,
    /// Matcher-selected execution price.
    pub execution_price: u64,
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated policy digest.
    pub fee_config_digest: [u8; 32],
    /// Actual fee-recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Runtime-width effects of one ordinary transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeOrdinarySettlementV2 {
    /// Updated seller replay root.
    pub seller_replay_root: MakerReplayRootV2,
    /// Updated buyer replay root.
    pub buyer_replay_root: MakerReplayRootV2,
    /// Seller record replacement or close.
    pub seller_record: RecordAfterFillV2,
    /// Buyer record replacement or close.
    pub buyer_record: RecordAfterFillV2,
    /// Seller Position unchanged after registration reservation.
    pub seller_position: DirectPositionV2,
    /// Buyer Position after claim credit.
    pub buyer_position: DirectPositionV2,
    /// Selected outcome transfer.
    pub outcome_quantity: u64,
    /// Gross seller collateral credit.
    pub seller_collateral_credit: u64,
    /// Venue fee transfer.
    pub venue_fee_transfer: u64,
    /// Total buyer escrow debit.
    pub buyer_escrow_debit: u64,
}

/// Check one ordinary transfer without outcome-width specialization.
pub fn settle_ordinary_runtime_v2(
    input: RuntimeOrdinaryMatchV2,
) -> Result<RuntimeOrdinarySettlementV2> {
    crate::adapter::require_market_phase_v2(
        crate::adapter::AdapterActionV2::Ordinary,
        input.phase,
    )?;
    let ask = input.seller_record.intent();
    let bid = input.buyer_record.intent();
    if input.seller_position.outcome_count() != input.buyer_position.outcome_count()
        || ask.side() != Side::Sell
        || bid.side() != Side::Buy
        || ask.market() != bid.market()
        || ask.generation() != bid.generation()
        || ask.outcome() != bid.outcome()
    {
        return Err(Error::IncompatibleSides);
    }
    if ask.maker() == bid.maker() {
        return Err(Error::Alias);
    }
    input.seller_accounts.validate(ask)?;
    input.buyer_accounts.validate(bid)?;
    crate::adapter::validate_registered_escrow_authority_v2(
        input.buyer_escrow_authority,
        input.buyer_record,
        input.buyer_accounts.record,
        input.buyer_accounts.escrow,
        input.collateral_mint,
    )?;
    distinct_participants(&[input.seller_accounts, input.buyer_accounts])?;
    runtime_position_matches(input.seller_position, ask)?;
    runtime_position_matches(input.buyer_position, bid)?;
    venue_authorized(
        ask,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    venue_authorized(
        bid,
        input.fee_policy,
        input.fee_config_digest,
        input.fee_recipient_account,
    )?;
    if input.execution_price < ask.limit_price() || input.execution_price > bid.limit_price() {
        return Err(Error::PriceIncompatible);
    }
    let gross = quote(input.fill, input.execution_price)?;
    let (seller_record, seller_replay_root, seller_fee) =
        input
            .seller_record
            .consume(input.seller_replay_root, input.slot, input.fill, 0, 0)?;
    if seller_fee != 0 {
        return Err(Error::InvalidReservation);
    }
    let (buyer_record, buyer_replay_root, venue_fee) = input.buyer_record.consume(
        input.buyer_replay_root,
        input.slot,
        input.fill,
        gross,
        gross,
    )?;
    let buyer_debit = gross
        .checked_add(venue_fee)
        .ok_or(Error::ArithmeticOverflow)?;
    let outcome = usize::from(ask.outcome());
    if outcome >= usize::from(input.buyer_position.outcome_count()) {
        return Err(Error::InvalidOutcome);
    }
    let mut buyer_position = input.buyer_position;
    buyer_position.credit_outcome(outcome, input.fill)?;
    Ok(RuntimeOrdinarySettlementV2 {
        seller_replay_root,
        buyer_replay_root,
        seller_record,
        buyer_record,
        seller_position: input.seller_position,
        buyer_position,
        outcome_quantity: input.fill,
        seller_collateral_credit: gross,
        venue_fee_transfer: venue_fee,
        buyer_escrow_debit: buyer_debit,
    })
}

/// Inputs to one exhaustive complementary-buy split.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementaryBuyMatchV2<const N: usize> {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted `Clock::get()` slot.
    pub slot: u64,
    /// Maker replay roots in canonical outcome order.
    pub buyer_replay_roots: [MakerReplayRootV2; N],
    /// Program-owned buy records in canonical outcome order.
    pub buyer_records: [DirectIntentRecordV2; N],
    /// Exact participant account quadruples in canonical outcome order.
    pub buyer_accounts: [ParticipantAccountsV2; N],
    /// Buyer Positions in canonical outcome order.
    pub buyer_positions: [PositionV1<N>; N],
    /// Realm-selected collateral mint.
    pub collateral_mint: [u8; 32],
    /// Authenticated escrow authorities in canonical outcome order.
    pub escrow_authorities: [crate::adapter::EscrowAuthorityV2; N],
    /// Common matcher-selected fill.
    pub fill: u64,
    /// Exact prices summing to [`crate::PRICE_SCALE`].
    pub execution_prices: [u64; N],
    /// Canonical program-owned fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated SHA-256 digest of the exact policy bytes.
    pub fee_config_digest: [u8; 32],
    /// Actual fee-recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Exact effects of one complete-set split.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitSettlementV2<const N: usize> {
    /// Updated replay roots in canonical outcome order.
    pub buyer_replay_roots: [MakerReplayRootV2; N],
    /// Record replacements or close effects in canonical outcome order.
    pub buyer_records: [RecordAfterFillV2; N],
    /// Buyer Positions after indexed claim credits.
    pub buyer_positions: [PositionV1<N>; N],
    /// Gross escrow debits in canonical outcome order.
    pub buyer_gross_collateral_debits: [u64; N],
    /// Fee escrow debits in canonical outcome order.
    pub buyer_fee_debits: [u64; N],
    /// Exact Market-vault collateral credit.
    pub market_vault_collateral_credit: u64,
    /// Aggregate fee transfer.
    pub venue_fee_transfer: u64,
}

/// Check one atomic permissionless complementary-buy split.
#[cfg(test)]
pub fn settle_split_v2<const N: usize>(
    input: ComplementaryBuyMatchV2<N>,
) -> Result<SplitSettlementV2<N>> {
    width(N)?;
    crate::adapter::require_market_phase_v2(crate::adapter::AdapterActionV2::Split, input.phase)?;
    if input.fill == 0 {
        return Err(Error::ZeroQuantity);
    }
    distinct_participants(&input.buyer_accounts)?;
    let first = input
        .buyer_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?
        .intent();
    let mut roots = input.buyer_replay_roots;
    let mut positions = input.buyer_positions;
    let seed_record = *input
        .buyer_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?;
    let mut records = [RecordAfterFillV2::live(seed_record); N];
    let mut gross = [0; N];
    let mut fees = [0; N];
    let mut makers = [[0; 32]; N];
    let mut price_sum = 0_u64;
    let mut gross_sum = 0_u64;
    let mut fee_sum = 0_u64;
    for index in 0..N {
        let record = input.buyer_records[index];
        let intent = record.intent();
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side() != Side::Buy
            || intent.market() != first.market()
            || intent.generation() != first.generation()
            || intent.outcome() != expected
        {
            return Err(Error::NonCanonicalComplement);
        }
        if makers[..index].iter().any(|maker| maker == intent.maker()) {
            return Err(Error::Alias);
        }
        makers[index] = *intent.maker();
        input.buyer_accounts[index].validate(intent)?;
        crate::adapter::validate_registered_escrow_authority_v2(
            input.escrow_authorities[index],
            record,
            input.buyer_accounts[index].record,
            input.buyer_accounts[index].escrow,
            input.collateral_mint,
        )?;
        position_matches(positions[index], intent)?;
        venue_authorized(
            intent,
            input.fee_policy,
            input.fee_config_digest,
            input.fee_recipient_account,
        )?;
        let price = input.execution_prices[index];
        if price > intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        let transition = record.consume(
            roots[index],
            input.slot,
            input.fill,
            gross[index],
            gross[index],
        )?;
        records[index] = transition.0;
        roots[index] = transition.1;
        fees[index] = transition.2;
        positions[index]
            .credit_outcome(index, input.fill)
            .map_err(crate::position_error)?;
        gross_sum = gross_sum
            .checked_add(gross[index])
            .ok_or(Error::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != crate::PRICE_SCALE || gross_sum != input.fill {
        return Err(Error::SplitFundingMismatch);
    }
    Ok(SplitSettlementV2 {
        buyer_replay_roots: roots,
        buyer_records: records,
        buyer_positions: positions,
        buyer_gross_collateral_debits: gross,
        buyer_fee_debits: fees,
        market_vault_collateral_credit: input.fill,
        venue_fee_transfer: fee_sum,
    })
}

/// Inputs to one exhaustive complementary-sell merge.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementarySellMatchV2<const N: usize> {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted `Clock::get()` slot.
    pub slot: u64,
    /// Maker replay roots in canonical outcome order.
    pub seller_replay_roots: [MakerReplayRootV2; N],
    /// Program-owned sell records in canonical outcome order.
    pub seller_records: [DirectIntentRecordV2; N],
    /// Exact participant account quadruples in canonical outcome order.
    pub seller_accounts: [ParticipantAccountsV2; N],
    /// Positions at signed accounts; claims were reserved at registration.
    pub seller_positions: [PositionV1<N>; N],
    /// Common matcher-selected fill.
    pub fill: u64,
    /// Exact prices summing to [`crate::PRICE_SCALE`].
    pub execution_prices: [u64; N],
    /// Canonical program-owned fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated SHA-256 digest of the exact policy bytes.
    pub fee_config_digest: [u8; 32],
    /// Actual fee-recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Exact effects of one complete-set merge.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeSettlementV2<const N: usize> {
    /// Updated replay roots in canonical outcome order.
    pub seller_replay_roots: [MakerReplayRootV2; N],
    /// Record replacements or close effects in canonical outcome order.
    pub seller_records: [RecordAfterFillV2; N],
    /// Positions unchanged after registration reservation.
    pub seller_positions: [PositionV1<N>; N],
    /// Gross collateral credits in canonical outcome order.
    pub seller_gross_collateral_credits: [u64; N],
    /// Per-seller fees retained from gross credits.
    pub seller_fee_debits: [u64; N],
    /// Net credits to signed collateral accounts.
    pub seller_net_collateral_credits: [u64; N],
    /// Exact Market-vault debit.
    pub market_vault_collateral_debit: u64,
    /// Aggregate fee transfer.
    pub venue_fee_transfer: u64,
}

/// Check one atomic permissionless complementary-sell merge.
#[cfg(test)]
pub fn settle_merge_v2<const N: usize>(
    input: ComplementarySellMatchV2<N>,
) -> Result<MergeSettlementV2<N>> {
    width(N)?;
    crate::adapter::require_market_phase_v2(crate::adapter::AdapterActionV2::Merge, input.phase)?;
    if input.fill == 0 {
        return Err(Error::ZeroQuantity);
    }
    distinct_participants(&input.seller_accounts)?;
    let first = input
        .seller_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?
        .intent();
    let mut roots = input.seller_replay_roots;
    let seed_record = *input
        .seller_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?;
    let mut records = [RecordAfterFillV2::live(seed_record); N];
    let mut gross = [0; N];
    let mut fees = [0; N];
    let mut net = [0; N];
    let mut makers = [[0; 32]; N];
    let mut price_sum = 0_u64;
    let mut gross_sum = 0_u64;
    let mut fee_sum = 0_u64;
    for index in 0..N {
        let record = input.seller_records[index];
        let intent = record.intent();
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side() != Side::Sell
            || intent.market() != first.market()
            || intent.generation() != first.generation()
            || intent.outcome() != expected
        {
            return Err(Error::NonCanonicalComplement);
        }
        if makers[..index].iter().any(|maker| maker == intent.maker()) {
            return Err(Error::Alias);
        }
        makers[index] = *intent.maker();
        input.seller_accounts[index].validate(intent)?;
        position_matches(input.seller_positions[index], intent)?;
        venue_authorized(
            intent,
            input.fee_policy,
            input.fee_config_digest,
            input.fee_recipient_account,
        )?;
        let price = input.execution_prices[index];
        if price < intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        let transition = record.consume(roots[index], input.slot, input.fill, gross[index], 0)?;
        fees[index] = transition.2;
        net[index] = gross[index]
            .checked_sub(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
        records[index] = transition.0;
        roots[index] = transition.1;
        gross_sum = gross_sum
            .checked_add(gross[index])
            .ok_or(Error::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != crate::PRICE_SCALE || gross_sum != input.fill {
        return Err(Error::SplitFundingMismatch);
    }
    Ok(MergeSettlementV2 {
        seller_replay_roots: roots,
        seller_records: records,
        seller_positions: input.seller_positions,
        seller_gross_collateral_credits: gross,
        seller_fee_debits: fees,
        seller_net_collateral_credits: net,
        market_vault_collateral_debit: input.fill,
        venue_fee_transfer: fee_sum,
    })
}

/// Borrowed inputs and caller-owned outputs for one complete-set Buy split.
///
/// The bounded SBF adapter keeps these slices on its heap. This pure owner
/// validates the whole complement before changing any root, record, Position,
/// or close slot, avoiding an N=16 by-value stack frame.
pub struct ComplementaryBuyMatchInPlaceV2<'a, const N: usize> {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted `Clock::get()` slot.
    pub slot: u64,
    /// Replay roots, replaced in canonical outcome order on success.
    pub buyer_replay_roots: &'a mut [MakerReplayRootV2],
    /// Records, replaced only where the corresponding fill remains live.
    pub buyer_records: &'a mut [DirectIntentRecordV2],
    /// Exact participant account tuples in canonical outcome order.
    pub buyer_accounts: &'a [ParticipantAccountsV2],
    /// Positions, replaced in canonical outcome order on success.
    pub buyer_positions: &'a mut [PositionV1<N>],
    /// Realm-selected collateral mint.
    pub collateral_mint: [u8; 32],
    /// Authenticated escrow authorities in canonical outcome order.
    pub escrow_authorities: &'a [crate::adapter::EscrowAuthorityV2],
    /// Caller-owned close slots; `Some` means that record closes.
    pub record_closes: &'a mut [Option<crate::LiveRecordCloseV2>],
    /// Common matcher-selected fill.
    pub fill: u64,
    /// Exact prices summing to [`crate::PRICE_SCALE`].
    pub execution_prices: &'a [u64],
    /// Canonical program-owned fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated SHA-256 digest of the exact policy bytes.
    pub fee_config_digest: [u8; 32],
    /// Actual fee-recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Small transfer effects of one in-place complete-set Buy split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitSettlementEffectsV2<const N: usize> {
    /// Gross escrow debits in canonical outcome order.
    pub buyer_gross_collateral_debits: [u64; N],
    /// Cumulative-difference fee debits in canonical outcome order.
    pub buyer_fee_debits: [u64; N],
    /// Exact Market-vault collateral credit.
    pub market_vault_collateral_credit: u64,
    /// Aggregate fee transfer.
    pub venue_fee_transfer: u64,
}

/// Check and atomically project one permissionless complementary-buy split.
pub fn settle_split_in_place_v2<const N: usize>(
    input: ComplementaryBuyMatchInPlaceV2<'_, N>,
) -> Result<SplitSettlementEffectsV2<N>> {
    width(N)?;
    crate::adapter::require_market_phase_v2(crate::adapter::AdapterActionV2::Split, input.phase)?;
    if input.fill == 0 {
        return Err(Error::ZeroQuantity);
    }
    require_complementary_lengths(
        N,
        &[
            input.buyer_replay_roots.len(),
            input.buyer_records.len(),
            input.buyer_accounts.len(),
            input.buyer_positions.len(),
            input.escrow_authorities.len(),
            input.record_closes.len(),
            input.execution_prices.len(),
        ],
    )?;
    distinct_participants(input.buyer_accounts)?;
    let first = input
        .buyer_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?
        .intent();
    let mut gross = [0; N];
    let mut fees = [0; N];
    let mut price_sum = 0_u64;
    let mut gross_sum = 0_u64;
    let mut fee_sum = 0_u64;

    for index in 0..N {
        let record = input.buyer_records[index];
        let intent = record.intent();
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side() != Side::Buy
            || intent.market() != first.market()
            || intent.generation() != first.generation()
            || intent.outcome() != expected
        {
            return Err(Error::NonCanonicalComplement);
        }
        if input.buyer_records[..index]
            .iter()
            .any(|prior| prior.intent().maker() == intent.maker())
        {
            return Err(Error::Alias);
        }
        input.buyer_accounts[index].validate(intent)?;
        crate::adapter::validate_registered_escrow_authority_v2(
            input.escrow_authorities[index],
            record,
            input.buyer_accounts[index].record,
            input.buyer_accounts[index].escrow,
            input.collateral_mint,
        )?;
        position_matches(input.buyer_positions[index], intent)?;
        venue_authorized(
            intent,
            input.fee_policy,
            input.fee_config_digest,
            input.fee_recipient_account,
        )?;
        let price = input.execution_prices[index];
        if price > intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        let transition = record.consume(
            input.buyer_replay_roots[index],
            input.slot,
            input.fill,
            gross[index],
            gross[index],
        )?;
        fees[index] = transition.2;
        let mut position = input.buyer_positions[index];
        position
            .credit_outcome(index, input.fill)
            .map_err(crate::position_error)?;
        gross_sum = gross_sum
            .checked_add(gross[index])
            .ok_or(Error::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != crate::PRICE_SCALE || gross_sum != input.fill {
        return Err(Error::SplitFundingMismatch);
    }

    for (index, gross_value) in gross.iter().copied().enumerate() {
        let transition = input.buyer_records[index].consume(
            input.buyer_replay_roots[index],
            input.slot,
            input.fill,
            gross_value,
            gross_value,
        )?;
        input.buyer_replay_roots[index] = transition.1;
        input.record_closes[index] = transition.0.close;
        if let Some(record) = transition.0.live_record {
            input.buyer_records[index] = record;
        }
        input.buyer_positions[index]
            .credit_outcome(index, input.fill)
            .map_err(crate::position_error)?;
    }
    Ok(SplitSettlementEffectsV2 {
        buyer_gross_collateral_debits: gross,
        buyer_fee_debits: fees,
        market_vault_collateral_credit: input.fill,
        venue_fee_transfer: fee_sum,
    })
}

/// Borrowed inputs and caller-owned outputs for one complete-set Sell merge.
pub struct ComplementarySellMatchInPlaceV2<'a, const N: usize> {
    /// Canonical Market phase authenticated from program-owned state.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted `Clock::get()` slot.
    pub slot: u64,
    /// Replay roots, replaced in canonical outcome order on success.
    pub seller_replay_roots: &'a mut [MakerReplayRootV2],
    /// Records, replaced only where the corresponding fill remains live.
    pub seller_records: &'a mut [DirectIntentRecordV2],
    /// Exact participant account tuples in canonical outcome order.
    pub seller_accounts: &'a [ParticipantAccountsV2],
    /// Positions authenticated in canonical outcome order; registration
    /// already removed the reserved claims, so merge leaves them unchanged.
    pub seller_positions: &'a [PositionV1<N>],
    /// Caller-owned close slots; `Some` means that record closes.
    pub record_closes: &'a mut [Option<crate::LiveRecordCloseV2>],
    /// Common matcher-selected fill.
    pub fill: u64,
    /// Exact prices summing to [`crate::PRICE_SCALE`].
    pub execution_prices: &'a [u64],
    /// Canonical program-owned fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated SHA-256 digest of the exact policy bytes.
    pub fee_config_digest: [u8; 32],
    /// Actual fee-recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Small transfer effects of one in-place complete-set Sell merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeSettlementEffectsV2<const N: usize> {
    /// Gross collateral credits in canonical outcome order.
    pub seller_gross_collateral_credits: [u64; N],
    /// Cumulative-difference seller fees in canonical outcome order.
    pub seller_fee_debits: [u64; N],
    /// Net credits to signed collateral accounts.
    pub seller_net_collateral_credits: [u64; N],
    /// Exact Market-vault debit.
    pub market_vault_collateral_debit: u64,
    /// Aggregate fee transfer.
    pub venue_fee_transfer: u64,
}

/// Check and atomically project one permissionless complementary-sell merge.
pub fn settle_merge_in_place_v2<const N: usize>(
    input: ComplementarySellMatchInPlaceV2<'_, N>,
) -> Result<MergeSettlementEffectsV2<N>> {
    width(N)?;
    crate::adapter::require_market_phase_v2(crate::adapter::AdapterActionV2::Merge, input.phase)?;
    if input.fill == 0 {
        return Err(Error::ZeroQuantity);
    }
    require_complementary_lengths(
        N,
        &[
            input.seller_replay_roots.len(),
            input.seller_records.len(),
            input.seller_accounts.len(),
            input.seller_positions.len(),
            input.record_closes.len(),
            input.execution_prices.len(),
        ],
    )?;
    distinct_participants(input.seller_accounts)?;
    let first = input
        .seller_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?
        .intent();
    let mut gross = [0; N];
    let mut fees = [0; N];
    let mut net = [0; N];
    let mut price_sum = 0_u64;
    let mut gross_sum = 0_u64;
    let mut fee_sum = 0_u64;

    for index in 0..N {
        let record = input.seller_records[index];
        let intent = record.intent();
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side() != Side::Sell
            || intent.market() != first.market()
            || intent.generation() != first.generation()
            || intent.outcome() != expected
        {
            return Err(Error::NonCanonicalComplement);
        }
        if input.seller_records[..index]
            .iter()
            .any(|prior| prior.intent().maker() == intent.maker())
        {
            return Err(Error::Alias);
        }
        input.seller_accounts[index].validate(intent)?;
        position_matches(input.seller_positions[index], intent)?;
        venue_authorized(
            intent,
            input.fee_policy,
            input.fee_config_digest,
            input.fee_recipient_account,
        )?;
        let price = input.execution_prices[index];
        if price < intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        let transition = record.consume(
            input.seller_replay_roots[index],
            input.slot,
            input.fill,
            gross[index],
            0,
        )?;
        fees[index] = transition.2;
        net[index] = gross[index]
            .checked_sub(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
        gross_sum = gross_sum
            .checked_add(gross[index])
            .ok_or(Error::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != crate::PRICE_SCALE || gross_sum != input.fill {
        return Err(Error::SplitFundingMismatch);
    }

    for (index, gross_value) in gross.iter().copied().enumerate() {
        let transition = input.seller_records[index].consume(
            input.seller_replay_roots[index],
            input.slot,
            input.fill,
            gross_value,
            0,
        )?;
        input.seller_replay_roots[index] = transition.1;
        input.record_closes[index] = transition.0.close;
        if let Some(record) = transition.0.live_record {
            input.seller_records[index] = record;
        }
    }
    Ok(MergeSettlementEffectsV2 {
        seller_gross_collateral_credits: gross,
        seller_fee_debits: fees,
        seller_net_collateral_credits: net,
        market_vault_collateral_debit: input.fill,
        venue_fee_transfer: fee_sum,
    })
}

/// Borrowed runtime-width inputs and caller-owned outputs for one Buy split.
pub struct RuntimeComplementaryBuyMatchInPlaceV2<'a> {
    /// Canonical Market phase.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted slot.
    pub slot: u64,
    /// Runtime outcome width.
    pub outcome_count: u8,
    /// Replay roots replaced on success.
    pub buyer_replay_roots: &'a mut [MakerReplayRootV2],
    /// Records replaced where the fill remains live.
    pub buyer_records: &'a mut [DirectIntentRecordV2],
    /// Participant accounts in canonical outcome order.
    pub buyer_accounts: &'a [ParticipantAccountsV2],
    /// Runtime-width Positions replaced on success.
    pub buyer_positions: &'a mut [DirectPositionV2],
    /// Realm collateral mint.
    pub collateral_mint: [u8; 32],
    /// Escrow authorities in canonical order.
    pub escrow_authorities: &'a [crate::adapter::EscrowAuthorityV2],
    /// Caller-owned close slots.
    pub record_closes: &'a mut [Option<crate::LiveRecordCloseV2>],
    /// Common positive fill.
    pub fill: u64,
    /// Exact execution prices.
    pub execution_prices: &'a [u64],
    /// Caller-owned gross-debit output.
    pub gross_debits: &'a mut [u64],
    /// Caller-owned fee-debit output.
    pub fee_debits: &'a mut [u64],
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated policy digest.
    pub fee_config_digest: [u8; 32],
    /// Actual fee recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Aggregate effects of a runtime-width Buy split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSplitSettlementEffectsV2 {
    /// Exact Market-vault collateral credit.
    pub market_vault_collateral_credit: u64,
    /// Aggregate fee transfer.
    pub venue_fee_transfer: u64,
}

/// Check and atomically project one Buy split without width specialization.
#[allow(clippy::needless_range_loop)]
pub fn settle_split_runtime_in_place_v2(
    input: RuntimeComplementaryBuyMatchInPlaceV2<'_>,
) -> Result<RuntimeSplitSettlementEffectsV2> {
    let count = usize::from(input.outcome_count);
    width(count)?;
    crate::adapter::require_market_phase_v2(crate::adapter::AdapterActionV2::Split, input.phase)?;
    if input.fill == 0 {
        return Err(Error::ZeroQuantity);
    }
    require_complementary_lengths(
        count,
        &[
            input.buyer_replay_roots.len(),
            input.buyer_records.len(),
            input.buyer_accounts.len(),
            input.buyer_positions.len(),
            input.escrow_authorities.len(),
            input.record_closes.len(),
            input.execution_prices.len(),
            input.gross_debits.len(),
            input.fee_debits.len(),
        ],
    )?;
    distinct_participants(input.buyer_accounts)?;
    let first = input
        .buyer_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?
        .intent();
    let mut gross = [0u64; dclutch_realm_contract::MAX_OUTCOMES];
    let mut fees = [0u64; dclutch_realm_contract::MAX_OUTCOMES];
    let mut price_sum = 0u64;
    let mut gross_sum = 0u64;
    let mut fee_sum = 0u64;

    for index in 0..count {
        let record = input.buyer_records[index];
        let intent = record.intent();
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side() != Side::Buy
            || intent.market() != first.market()
            || intent.generation() != first.generation()
            || intent.outcome() != expected
        {
            return Err(Error::NonCanonicalComplement);
        }
        if input.buyer_records[..index]
            .iter()
            .any(|prior| prior.intent().maker() == intent.maker())
        {
            return Err(Error::Alias);
        }
        input.buyer_accounts[index].validate(intent)?;
        crate::adapter::validate_registered_escrow_authority_v2(
            input.escrow_authorities[index],
            record,
            input.buyer_accounts[index].record,
            input.buyer_accounts[index].escrow,
            input.collateral_mint,
        )?;
        if input.buyer_positions[index].outcome_count() != input.outcome_count {
            return Err(Error::InvalidOutcomeWidth);
        }
        runtime_position_matches(input.buyer_positions[index], intent)?;
        venue_authorized(
            intent,
            input.fee_policy,
            input.fee_config_digest,
            input.fee_recipient_account,
        )?;
        let price = input.execution_prices[index];
        if price > intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        let transition = record.consume(
            input.buyer_replay_roots[index],
            input.slot,
            input.fill,
            gross[index],
            gross[index],
        )?;
        fees[index] = transition.2;
        let mut position = input.buyer_positions[index];
        position.credit_outcome(index, input.fill)?;
        gross_sum = gross_sum
            .checked_add(gross[index])
            .ok_or(Error::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != crate::PRICE_SCALE || gross_sum != input.fill {
        return Err(Error::SplitFundingMismatch);
    }

    for index in 0..count {
        let transition = input.buyer_records[index].consume(
            input.buyer_replay_roots[index],
            input.slot,
            input.fill,
            gross[index],
            gross[index],
        )?;
        input.buyer_replay_roots[index] = transition.1;
        input.record_closes[index] = transition.0.close;
        if let Some(record) = transition.0.live_record {
            input.buyer_records[index] = record;
        }
        input.buyer_positions[index].credit_outcome(index, input.fill)?;
    }
    input.gross_debits.copy_from_slice(&gross[..count]);
    input.fee_debits.copy_from_slice(&fees[..count]);
    Ok(RuntimeSplitSettlementEffectsV2 {
        market_vault_collateral_credit: input.fill,
        venue_fee_transfer: fee_sum,
    })
}

/// Borrowed runtime-width inputs and outputs for one Sell merge.
pub struct RuntimeComplementarySellMatchInPlaceV2<'a> {
    /// Canonical Market phase.
    pub phase: crate::adapter::MarketPhaseV2,
    /// Current trusted slot.
    pub slot: u64,
    /// Runtime outcome width.
    pub outcome_count: u8,
    /// Replay roots replaced on success.
    pub seller_replay_roots: &'a mut [MakerReplayRootV2],
    /// Records replaced where the fill remains live.
    pub seller_records: &'a mut [DirectIntentRecordV2],
    /// Participant accounts in canonical order.
    pub seller_accounts: &'a [ParticipantAccountsV2],
    /// Runtime-width Positions authenticated in canonical order.
    pub seller_positions: &'a [DirectPositionV2],
    /// Caller-owned close slots.
    pub record_closes: &'a mut [Option<crate::LiveRecordCloseV2>],
    /// Common positive fill.
    pub fill: u64,
    /// Exact execution prices.
    pub execution_prices: &'a [u64],
    /// Caller-owned gross-credit output.
    pub gross_credits: &'a mut [u64],
    /// Caller-owned fee-debit output.
    pub fee_debits: &'a mut [u64],
    /// Caller-owned net-credit output.
    pub net_credits: &'a mut [u64],
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV3,
    /// Manifest-authenticated policy digest.
    pub fee_config_digest: [u8; 32],
    /// Actual fee recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Aggregate effects of a runtime-width Sell merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeMergeSettlementEffectsV2 {
    /// Exact Market-vault collateral debit.
    pub market_vault_collateral_debit: u64,
    /// Aggregate fee transfer.
    pub venue_fee_transfer: u64,
}

/// Check and atomically project one Sell merge without width specialization.
#[allow(clippy::needless_range_loop)]
pub fn settle_merge_runtime_in_place_v2(
    input: RuntimeComplementarySellMatchInPlaceV2<'_>,
) -> Result<RuntimeMergeSettlementEffectsV2> {
    let count = usize::from(input.outcome_count);
    width(count)?;
    crate::adapter::require_market_phase_v2(crate::adapter::AdapterActionV2::Merge, input.phase)?;
    if input.fill == 0 {
        return Err(Error::ZeroQuantity);
    }
    require_complementary_lengths(
        count,
        &[
            input.seller_replay_roots.len(),
            input.seller_records.len(),
            input.seller_accounts.len(),
            input.seller_positions.len(),
            input.record_closes.len(),
            input.execution_prices.len(),
            input.gross_credits.len(),
            input.fee_debits.len(),
            input.net_credits.len(),
        ],
    )?;
    distinct_participants(input.seller_accounts)?;
    let first = input
        .seller_records
        .first()
        .ok_or(Error::InvalidOutcomeWidth)?
        .intent();
    let mut gross = [0u64; dclutch_realm_contract::MAX_OUTCOMES];
    let mut fees = [0u64; dclutch_realm_contract::MAX_OUTCOMES];
    let mut net = [0u64; dclutch_realm_contract::MAX_OUTCOMES];
    let mut price_sum = 0u64;
    let mut gross_sum = 0u64;
    let mut fee_sum = 0u64;

    for index in 0..count {
        let record = input.seller_records[index];
        let intent = record.intent();
        let expected = u8::try_from(index).map_err(|_| Error::InvalidOutcome)?;
        if intent.side() != Side::Sell
            || intent.market() != first.market()
            || intent.generation() != first.generation()
            || intent.outcome() != expected
        {
            return Err(Error::NonCanonicalComplement);
        }
        if input.seller_records[..index]
            .iter()
            .any(|prior| prior.intent().maker() == intent.maker())
        {
            return Err(Error::Alias);
        }
        input.seller_accounts[index].validate(intent)?;
        if input.seller_positions[index].outcome_count() != input.outcome_count {
            return Err(Error::InvalidOutcomeWidth);
        }
        runtime_position_matches(input.seller_positions[index], intent)?;
        venue_authorized(
            intent,
            input.fee_policy,
            input.fee_config_digest,
            input.fee_recipient_account,
        )?;
        let price = input.execution_prices[index];
        if price < intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        let transition = record.consume(
            input.seller_replay_roots[index],
            input.slot,
            input.fill,
            gross[index],
            0,
        )?;
        fees[index] = transition.2;
        net[index] = gross[index]
            .checked_sub(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
        gross_sum = gross_sum
            .checked_add(gross[index])
            .ok_or(Error::ArithmeticOverflow)?;
        fee_sum = fee_sum
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if price_sum != crate::PRICE_SCALE || gross_sum != input.fill {
        return Err(Error::SplitFundingMismatch);
    }

    for index in 0..count {
        let transition = input.seller_records[index].consume(
            input.seller_replay_roots[index],
            input.slot,
            input.fill,
            gross[index],
            0,
        )?;
        input.seller_replay_roots[index] = transition.1;
        input.record_closes[index] = transition.0.close;
        if let Some(record) = transition.0.live_record {
            input.seller_records[index] = record;
        }
    }
    input.gross_credits.copy_from_slice(&gross[..count]);
    input.fee_debits.copy_from_slice(&fees[..count]);
    input.net_credits.copy_from_slice(&net[..count]);
    Ok(RuntimeMergeSettlementEffectsV2 {
        market_vault_collateral_debit: input.fill,
        venue_fee_transfer: fee_sum,
    })
}

fn require_complementary_lengths(expected: usize, lengths: &[usize]) -> Result<()> {
    if lengths.iter().all(|length| *length == expected) {
        Ok(())
    } else {
        Err(Error::InvalidOutcomeWidth)
    }
}

fn distinct_participants(accounts: &[ParticipantAccountsV2]) -> Result<()> {
    for (index, account) in accounts.iter().enumerate() {
        let keys = [
            account.replay_root,
            account.record,
            account.position,
            account.collateral,
            account.escrow,
        ];
        for (key_index, key) in keys.iter().enumerate() {
            if *key == [0; 32] {
                continue;
            }
            if keys[..key_index].contains(key) {
                return Err(Error::Alias);
            }
            for prior in &accounts[..index] {
                if [
                    prior.replay_root,
                    prior.record,
                    prior.position,
                    prior.collateral,
                    prior.escrow,
                ]
                .contains(key)
                {
                    return Err(Error::Alias);
                }
            }
        }
    }
    Ok(())
}
