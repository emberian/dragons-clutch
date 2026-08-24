#![allow(clippy::indexing_slicing)]

use dclutch_realm_contract::PositionV1;

use crate::state::{
    DirectIntentRecordV2, DirectIntentV2, InlineParticipantAccountsV2, MakerReplayRootV2,
    ParticipantAccountsV2, RecordAfterFillV2, Side, VenueFeePolicyV2, position_matches,
    venue_authorized,
};
use crate::{Error, Result, fee, quote, width};

/// Inputs to an immediate signed two-party FOK/IOC execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineOrdinaryMatchV2<const N: usize> {
    /// Current adapter-authenticated Clock slot.
    pub slot: u64,
    /// Seller replay root consuming exactly one nonce.
    pub seller_replay_root: MakerReplayRootV2,
    /// Buyer replay root consuming exactly one nonce.
    pub buyer_replay_root: MakerReplayRootV2,
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
    /// Immediate positive fill.
    pub fill: u64,
    /// Exact execution price.
    pub execution_price: u64,
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV2,
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
    venue_authorized(ask, input.fee_policy, input.fee_recipient_account)?;
    venue_authorized(bid, input.fee_policy, input.fee_recipient_account)?;
    if input.execution_price < ask.limit_price() || input.execution_price > bid.limit_price() {
        return Err(Error::PriceIncompatible);
    }
    let gross = quote(input.fill, input.execution_price)?;
    let venue_fee = fee(gross, input.fee_policy.fee_basis_points())?;
    let total = gross
        .checked_add(venue_fee)
        .ok_or(Error::ArithmeticOverflow)?;
    let seller_replay_root = input.seller_replay_root.consume_inline(ask, input.fill)?;
    let buyer_replay_root = input.buyer_replay_root.consume_inline(bid, input.fill)?;
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

/// Inputs to an immediate N=2 complementary buy or sell execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InlineComplementaryMatchV2<const N: usize> {
    /// Current adapter-authenticated Clock slot.
    pub slot: u64,
    /// Common Buy or Sell direction.
    pub side: Side,
    /// Replay roots in canonical outcome order.
    pub replay_roots: [MakerReplayRootV2; N],
    /// Exact signed intents in canonical outcome order.
    pub intents: [DirectIntentV2; N],
    /// Sealed native authorizations in canonical outcome order.
    pub authorizations: [crate::adapter::Ed25519AuthorizationV2; N],
    /// Exact immediate account triples in canonical outcome order.
    pub accounts: [InlineParticipantAccountsV2; N],
    /// Positions debited or credited atomically.
    pub positions: [PositionV1<N>; N],
    /// Common immediate fill.
    pub fill: u64,
    /// Exact prices summing to one collateral atom.
    pub execution_prices: [u64; N],
    /// Canonical fee policy.
    pub fee_policy: VenueFeePolicyV2,
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
    let first = *input.intents.first().ok_or(Error::InvalidInlineWidth)?;
    let mut roots = input.replay_roots;
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
        venue_authorized(intent, input.fee_policy, input.fee_recipient_account)?;
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
        roots[index] = roots[index].consume_inline(intent, input.fill)?;
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

fn inline_alias(left: InlineParticipantAccountsV2, right: InlineParticipantAccountsV2) -> bool {
    let left_keys = [left.replay_root, left.position, left.collateral];
    let right_keys = [right.replay_root, right.position, right.collateral];
    left_keys.iter().any(|key| right_keys.contains(key))
}

/// Inputs to an ordinary permissionless persisted-record transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrdinaryMatchV2<const N: usize> {
    /// Current adapter-authenticated Clock slot.
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
    /// Matcher-selected positive fill.
    pub fill: u64,
    /// Matcher-selected exact scaled execution price.
    pub execution_price: u64,
    /// Canonical program-owned fee policy.
    pub fee_policy: VenueFeePolicyV2,
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
    distinct_participants(&[input.seller_accounts, input.buyer_accounts])?;
    position_matches(input.seller_position, ask)?;
    position_matches(input.buyer_position, bid)?;
    venue_authorized(ask, input.fee_policy, input.fee_recipient_account)?;
    venue_authorized(bid, input.fee_policy, input.fee_recipient_account)?;
    if input.execution_price < ask.limit_price() || input.execution_price > bid.limit_price() {
        return Err(Error::PriceIncompatible);
    }
    let gross = quote(input.fill, input.execution_price)?;
    let venue_fee = fee(gross, input.fee_policy.fee_basis_points())?;
    let buyer_debit = gross
        .checked_add(venue_fee)
        .ok_or(Error::ArithmeticOverflow)?;
    let (seller_record, seller_replay_root) =
        input
            .seller_record
            .consume(input.seller_replay_root, input.slot, input.fill, 0)?;
    let (buyer_record, buyer_replay_root) =
        input
            .buyer_record
            .consume(input.buyer_replay_root, input.slot, input.fill, buyer_debit)?;
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

/// Inputs to one exhaustive complementary-buy split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementaryBuyMatchV2<const N: usize> {
    /// Current adapter-authenticated Clock slot.
    pub slot: u64,
    /// Maker replay roots in canonical outcome order.
    pub buyer_replay_roots: [MakerReplayRootV2; N],
    /// Program-owned buy records in canonical outcome order.
    pub buyer_records: [DirectIntentRecordV2; N],
    /// Exact participant account quadruples in canonical outcome order.
    pub buyer_accounts: [ParticipantAccountsV2; N],
    /// Buyer Positions in canonical outcome order.
    pub buyer_positions: [PositionV1<N>; N],
    /// Common matcher-selected fill.
    pub fill: u64,
    /// Exact prices summing to [`crate::PRICE_SCALE`].
    pub execution_prices: [u64; N],
    /// Canonical program-owned fee policy.
    pub fee_policy: VenueFeePolicyV2,
    /// Actual fee-recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Exact effects of one complete-set split.
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
pub fn settle_split_v2<const N: usize>(
    input: ComplementaryBuyMatchV2<N>,
) -> Result<SplitSettlementV2<N>> {
    width(N)?;
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
        position_matches(positions[index], intent)?;
        venue_authorized(intent, input.fee_policy, input.fee_recipient_account)?;
        let price = input.execution_prices[index];
        if price > intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        fees[index] = fee(gross[index], input.fee_policy.fee_basis_points())?;
        let debit = gross[index]
            .checked_add(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
        let transition = record.consume(roots[index], input.slot, input.fill, debit)?;
        records[index] = transition.0;
        roots[index] = transition.1;
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComplementarySellMatchV2<const N: usize> {
    /// Current adapter-authenticated Clock slot.
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
    pub fee_policy: VenueFeePolicyV2,
    /// Actual fee-recipient account.
    pub fee_recipient_account: [u8; 32],
}

/// Exact effects of one complete-set merge.
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
pub fn settle_merge_v2<const N: usize>(
    input: ComplementarySellMatchV2<N>,
) -> Result<MergeSettlementV2<N>> {
    width(N)?;
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
        venue_authorized(intent, input.fee_policy, input.fee_recipient_account)?;
        let price = input.execution_prices[index];
        if price < intent.limit_price() {
            return Err(Error::PriceIncompatible);
        }
        price_sum = price_sum
            .checked_add(price)
            .ok_or(Error::ArithmeticOverflow)?;
        gross[index] = quote(input.fill, price)?;
        fees[index] = fee(gross[index], input.fee_policy.fee_basis_points())?;
        net[index] = gross[index]
            .checked_sub(fees[index])
            .ok_or(Error::ArithmeticOverflow)?;
        let transition = record.consume(roots[index], input.slot, input.fill, 0)?;
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
