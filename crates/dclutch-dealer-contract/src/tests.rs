extern crate std;

use std::vec;

use super::*;

const POOL_ADDRESS: [u8; 32] = [8; 32];
const INITIAL_POSITION_ID: [u8; 32] = [40; 32];

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero test content ID")
}

fn attachment() -> LiquidityAttachment {
    LiquidityAttachment::new(
        MarketIdentity::new(id(1), id(2), id(3), id(4), id(5), 7),
        id(6),
        id(7),
        [9; 32],
    )
    .expect("fixture attachment")
}

fn rent(seed: u8) -> RentCreditTerms {
    RentCreditTerms::new([seed; 32], 1_000_000).expect("fixture rent credit")
}

fn config<const N: usize, const B: usize>() -> LiquidityConfigV1<N, B> {
    config_with_identity(id(7))
}

fn config_with_identity<const N: usize, const B: usize>(
    content_id: ContentId,
) -> LiquidityConfigV1<N, B> {
    let mut bids = [[0u64; B]; N];
    let mut asks = [[0u64; B]; N];
    let capacities = [[1_000u64; B]; N];
    let claim_count = u64::try_from(N).expect("bounded claim count");
    let best_bid = 8_000 / claim_count;
    let best_ask = 12_000_u64.div_ceil(claim_count);
    let price_step = core::cmp::max(1, best_bid / 40);
    for (bid_row, ask_row) in bids.iter_mut().zip(asks.iter_mut()) {
        for (bin, (bid, ask)) in bid_row.iter_mut().zip(ask_row.iter_mut()).enumerate() {
            let bin_u64 = u64::try_from(bin).expect("bounded bin");
            *bid = best_bid - price_step * bin_u64;
            *ask = best_ask + price_step * bin_u64;
        }
    }
    LiquidityConfigV1::new(
        content_id, [20; 32], 10_000, 25, 2_500, 100, bids, asks, capacities, capacities,
    )
    .expect("fixture config")
}

#[allow(clippy::too_many_arguments)]
fn open_for_test<const N: usize, const B: usize>(
    attachment: LiquidityAttachment,
    pool_address: [u8; 32],
    config: &LiquidityConfigV1<N, B>,
    pool_rent: RentCreditTerms,
    opened_at_slot: u64,
    liquidity: LiquidityAmounts<N>,
    service_funding: u64,
    position_id: [u8; 32],
    owner: [u8; 32],
    position_rent: RentCreditTerms,
    shares: u64,
) -> Result<(PoolState<N, B>, LpPosition, LiquidityChangeReceipt<N>)> {
    let profile = runtime::LiquidityProfileV1::new(N, B)?;
    let mut config_bytes = vec![0; LiquidityConfigV1::<N, B>::encoded_len()?];
    config.encode_into(&mut config_bytes)?;
    let config_view =
        runtime::LiquidityConfigViewV1::new(config.content_id(), profile, &config_bytes)?;
    let mut pool_bytes = vec![0; profile.pool_len()?];
    let (position, receipt) = runtime::initialize_pool(
        &mut pool_bytes,
        profile,
        attachment,
        pool_address,
        config_view,
        pool_rent,
        opened_at_slot,
        liquidity,
        service_funding,
        position_id,
        owner,
        position_rent,
        shares,
    )?;
    Ok((PoolState::decode(&pool_bytes)?, position, receipt))
}

fn opened<const N: usize, const B: usize>() -> (LiquidityConfigV1<N, B>, PoolState<N, B>, LpPosition)
{
    let config = config();
    let liquidity = LiquidityAmounts::new(100_000, 0, [10_000; N]).expect("liquidity");
    let (pool, position, receipt) = open_for_test(
        attachment(),
        POOL_ADDRESS,
        &config,
        rent(30),
        1_000,
        liquidity,
        5_000,
        INITIAL_POSITION_ID,
        [41; 32],
        rent(42),
        1_000,
    )
    .expect("open Pool");
    receipt.validate().expect("open receipt");
    (config, pool, position)
}

fn maximum<const N: usize>() -> LiquidityAmounts<N> {
    LiquidityAmounts::new(u64::MAX, u64::MAX, [u64::MAX; N]).expect("maximum")
}

fn zero<const N: usize>() -> LiquidityAmounts<N> {
    LiquidityAmounts::new(0, 0, [0; N]).expect("zero")
}

#[test]
fn n2_n16_widths_are_exact_and_compact_codecs_round_trip() {
    assert_eq!(LIQUIDITY_ATTACHMENT_BYTES, 264);
    assert_eq!(RENT_CREDIT_TERMS_BYTES, 40);
    assert_eq!(LP_POSITION_BYTES, 152);
    assert_eq!(LiquidityConfigV1::<2, 2>::encoded_len(), Ok(208));
    assert_eq!(PoolState::<2, 2>::encoded_len(), Ok(472));
    assert_eq!(ExecutionReceipt::<2>::encoded_len(), Ok(216));
    assert_eq!(LiquidityConfigV1::<16, 8>::encoded_len(), Ok(4_176));
    assert_eq!(PoolState::<16, 8>::encoded_len(), Ok(2_568));
    assert_eq!(ExecutionReceipt::<8>::encoded_len(), Ok(312));

    let (config, pool, position) = opened::<2, 2>();
    let mut config_bytes = vec![0; LiquidityConfigV1::<2, 2>::encoded_len().expect("width")];
    config
        .encode_into(&mut config_bytes)
        .expect("encode config");
    assert_eq!(
        LiquidityConfigV1::<2, 2>::decode(id(7), &config_bytes),
        Ok(config)
    );
    let mut pool_bytes = vec![0; PoolState::<2, 2>::encoded_len().expect("width")];
    pool.encode_into(&mut pool_bytes).expect("encode Pool");
    assert_eq!(PoolState::<2, 2>::decode(&pool_bytes), Ok(pool));
    let position_bytes = position.to_bytes().expect("encode position");
    assert_eq!(LpPosition::decode(&position_bytes), Ok(position));

    assert!(!pool_bytes.windows(32).any(|window| window == POOL_ADDRESS));
    assert!(
        !config_bytes
            .windows(32)
            .any(|window| window == POOL_ADDRESS)
    );
    assert!(
        !position_bytes
            .windows(32)
            .any(|window| window == INITIAL_POSITION_ID)
    );

    let (config16, pool16, _) = opened::<16, 8>();
    let mut config16_bytes = vec![0; LiquidityConfigV1::<16, 8>::encoded_len().expect("width")];
    config16
        .encode_into(&mut config16_bytes)
        .expect("encode max config");
    assert_eq!(
        LiquidityConfigV1::<16, 8>::decode(id(7), &config16_bytes),
        Ok(config16)
    );
    let mut pool16_bytes = vec![0; PoolState::<16, 8>::encoded_len().expect("width")];
    pool16
        .encode_into(&mut pool16_bytes)
        .expect("encode max Pool");
    assert_eq!(PoolState::<16, 8>::decode(&pool16_bytes), Ok(pool16));
}

#[test]
fn buy_sell_conservation_uses_present_separate_fee_collateral() {
    let (config, mut pool, _) = opened::<2, 2>();
    let service_before = pool.service_funding();
    let buy =
        TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 1_500, 908).expect("buy request");
    let receipt = pool
        .execute(POOL_ADDRESS, &config, buy)
        .expect("covered buy");
    assert_eq!(receipt.notional_collateral(), 905);
    assert_eq!(receipt.trader_fee_collateral(), 3);
    assert_eq!(receipt.trader_collateral_debit(), 908);
    assert_eq!(receipt.trader_claim_credit(), 1_500);
    assert_eq!(pool.liquidity().principal_collateral(), 100_905);
    assert_eq!(pool.liquidity().realized_fee_collateral(), 3);
    assert_eq!(
        pool.liquidity().claim_reserves().first().copied(),
        Some(8_500)
    );
    assert_eq!(pool.service_funding(), service_before);

    let sell =
        TradeRequest::new(0, 2, TradeSide::SellClaimToPool, 0, 1_500, 590).expect("sell request");
    let receipt = pool
        .execute(POOL_ADDRESS, &config, sell)
        .expect("covered sell");
    assert_eq!(receipt.notional_collateral(), 595);
    assert_eq!(receipt.trader_fee_collateral(), 2);
    assert_eq!(receipt.trader_collateral_debit(), 2);
    assert_eq!(receipt.trader_collateral_credit(), 595);
    assert_eq!(receipt.trader_claim_debit(), 1_500);
    assert_eq!(pool.liquidity().realized_fee_collateral(), 5);
    assert_eq!(pool.service_funding(), service_before);

    let mut bytes = vec![0; ExecutionReceipt::<2>::encoded_len().expect("width")];
    receipt.encode_into(&mut bytes).expect("encode receipt");
    assert_eq!(ExecutionReceipt::<2>::decode(&bytes), Ok(receipt));
}

#[test]
fn quote_refuses_inventory_cash_depth_limits_and_zero_rounding() {
    let (config, mut pool, _) = opened::<2, 2>();
    *pool.claim_reserves.first_mut().expect("first claim") = 1;
    let buy = TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 2, 10).expect("buy request");
    assert_eq!(
        pool.quote(POOL_ADDRESS, &config, buy),
        Err(Error::InsufficientClaimInventory)
    );
    *pool.claim_reserves.first_mut().expect("first claim") = 10_000;
    let too_deep = TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 2_001, u64::MAX)
        .expect("depth request");
    assert_eq!(
        pool.quote(POOL_ADDRESS, &config, too_deep),
        Err(Error::InsufficientBinDepth)
    );
    let too_expensive =
        TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 1_000, 1).expect("limit request");
    assert_eq!(
        pool.quote(POOL_ADDRESS, &config, too_expensive),
        Err(Error::LimitExceeded)
    );

    pool.principal_collateral = 1;
    let sell = TradeRequest::new(0, 1, TradeSide::SellClaimToPool, 0, 10, 0).expect("sell request");
    assert_eq!(
        pool.quote(POOL_ADDRESS, &config, sell),
        Err(Error::InsufficientPrincipalCollateral)
    );

    let tiny_config = LiquidityConfigV1::new(
        id(7),
        [20; 32],
        10_000,
        1,
        10,
        100,
        [[1], [1]],
        [[5_000], [5_000]],
        [[10], [10]],
        [[10], [10]],
    )
    .expect("tiny config");
    let (tiny_pool, _, _) = open_for_test(
        attachment(),
        POOL_ADDRESS,
        &tiny_config,
        rent(30),
        1_000,
        LiquidityAmounts::new(10, 0, [10, 10]).expect("liquidity"),
        0,
        INITIAL_POSITION_ID,
        [41; 32],
        rent(42),
        10,
    )
    .expect("tiny Pool");
    let dust = TradeRequest::new(0, 1, TradeSide::SellClaimToPool, 0, 1, 0).expect("dust request");
    assert_eq!(
        tiny_pool.quote(POOL_ADDRESS, &tiny_config, dust),
        Err(Error::ZeroNotional)
    );
}

#[test]
fn replay_config_identity_and_overflow_refusals_are_atomic() {
    let (config, mut pool, _) = opened::<2, 2>();
    let before = pool;
    let stale = TradeRequest::new(0, 0, TradeSide::BuyClaimFromPool, 0, 1, 10).expect("request");
    assert_eq!(
        pool.execute(POOL_ADDRESS, &config, stale),
        Err(Error::SequenceMismatch)
    );
    assert_eq!(pool, before);

    let wrong_id = config_with_identity::<2, 2>(id(70));
    let request = TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 1, 10).expect("request");
    assert_eq!(
        pool.quote(POOL_ADDRESS, &wrong_id, request),
        Err(Error::ConfigurationMismatch)
    );
    pool.principal_collateral = u64::MAX;
    let before_overflow = pool;
    assert_eq!(
        pool.execute(POOL_ADDRESS, &config, request),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(pool, before_overflow);
}

#[test]
fn timed_reset_preserves_depth_and_only_reopens_identical_config() {
    let (config, mut pool, _) = opened::<2, 2>();
    let trade =
        TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 1_000, 1_000).expect("trade");
    pool.execute(POOL_ADDRESS, &config, trade).expect("trade");
    assert_eq!(pool.ask_filled().first().copied(), Some([1_000, 0]));

    let before = pool;
    assert_eq!(
        pool.reset_ladder(POOL_ADDRESS, &config, 2, 1_099),
        Err(Error::ResetTooEarly)
    );
    assert_eq!(pool, before);
    let reset = pool
        .reset_ladder(POOL_ADDRESS, &config, 2, 1_100)
        .expect("timed reset");
    assert_eq!(reset.old_reset_number(), 0);
    assert_eq!(reset.new_reset_number(), 1);
    assert_eq!(reset.next_reset_slot(), 1_200);
    assert_eq!(pool.ask_filled(), [[0; 2]; 2]);

    let stale_reset = TradeRequest::new(0, 3, TradeSide::BuyClaimFromPool, 0, 1, 10)
        .expect("stale reset request");
    assert_eq!(
        pool.quote(POOL_ADDRESS, &config, stale_reset),
        Err(Error::InvalidReset)
    );
    assert_eq!(
        pool.reset_ladder(POOL_ADDRESS, &config, 3, 1_100),
        Err(Error::ResetTooEarly)
    );

    let overflowing_interval = LiquidityConfigV1::new(
        id(7),
        [20; 32],
        10_000,
        25,
        10,
        100,
        [[4_000], [4_000]],
        [[6_000], [6_000]],
        [[10], [10]],
        [[10], [10]],
    )
    .expect("config");
    assert_eq!(
        open_for_test(
            attachment(),
            POOL_ADDRESS,
            &overflowing_interval,
            rent(30),
            u64::MAX - 50,
            LiquidityAmounts::new(10, 0, [10, 10]).expect("liquidity"),
            0,
            INITIAL_POSITION_ID,
            [41; 32],
            rent(42),
            10,
        ),
        Err(Error::InvalidResetInterval)
    );
}

#[test]
fn multi_lp_ceil_add_prevents_dilution_and_floor_remove_keeps_dust() {
    let (config, mut pool, _) = opened::<2, 2>();
    let buy = TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 1, 10).expect("request");
    pool.execute(POOL_ADDRESS, &config, buy)
        .expect("accrue fee");
    let original = pool.liquidity();
    let (mut newcomer, _) = pool
        .create_position(POOL_ADDRESS, &config, 2, [50; 32], [51; 32], rent(52))
        .expect("position");
    let add = AddLiquidityRequest::new(3, 1, 500, maximum()).expect("add request");
    let added = pool
        .add_liquidity(POOL_ADDRESS, &config, [50; 32], &mut newcomer, add)
        .expect("add liquidity");
    assert_eq!(added.amounts_transferred().realized_fee_collateral(), 1);
    let remove = RemoveLiquidityRequest::new(4, 2, 500, zero()).expect("remove request");
    pool.remove_liquidity(POOL_ADDRESS, &config, [50; 32], &mut newcomer, remove)
        .expect("remove newcomer");
    assert_eq!(newcomer.status(), PositionStatus::Empty);
    assert!(pool.liquidity().principal_collateral() >= original.principal_collateral());
    assert!(pool.liquidity().realized_fee_collateral() >= original.realized_fee_collateral());
    for (after, before) in pool
        .liquidity()
        .claim_reserves()
        .iter()
        .zip(original.claim_reserves().iter())
    {
        assert!(after >= before);
    }
}

#[test]
fn last_lp_exact_sweep_then_rentcredit_and_service_retirement() {
    let (config, mut pool, mut position) = opened::<2, 2>();
    let before = pool.liquidity();
    let service = pool.service_funding();
    let remove = RemoveLiquidityRequest::new(1, 1, 1_000, before).expect("remove request");
    let receipt = pool
        .remove_liquidity(
            POOL_ADDRESS,
            &config,
            INITIAL_POSITION_ID,
            &mut position,
            remove,
        )
        .expect("last LP withdrawal");
    assert_eq!(receipt.amounts_transferred(), before);
    assert!(pool.liquidity().is_zero());
    assert_eq!(pool.service_funding(), service);
    assert_eq!(pool.status(), PoolStatus::Retiring);
    let close = pool
        .close_position(POOL_ADDRESS, INITIAL_POSITION_ID, &mut position, 2, 2)
        .expect("close last position");
    assert_eq!(close.rent_credit(), rent(42));
    let mut pool_bytes = vec![0; PoolState::<2, 2>::encoded_len().expect("Pool width")];
    pool.encode_into(&mut pool_bytes).expect("encode Pool");
    let mut config_bytes = vec![0; LiquidityConfigV1::<2, 2>::encoded_len().expect("config width")];
    config
        .encode_into(&mut config_bytes)
        .expect("encode config");
    let profile = runtime::LiquidityProfileV1::new(2, 2).expect("profile");
    let config_view =
        runtime::LiquidityConfigViewV1::new(config.content_id(), profile, &config_bytes)
            .expect("config view");
    let retired = runtime::retire_pool(&mut pool_bytes, profile, POOL_ADDRESS, config_view, 3)
        .expect("retire Pool");
    pool = PoolState::decode(&pool_bytes).expect("decode retired Pool");
    assert_eq!(retired.service_refund_collateral(), service);
    assert_eq!(retired.service_refund_beneficiary(), [9; 32]);
    assert_eq!(retired.pool_rent_credit(), rent(30));
    assert_eq!(pool.status(), PoolStatus::Retired);
}

#[test]
fn dust_share_burn_and_limits_refuse_without_mutation() {
    let tiny_config = config::<2, 1>();
    let (mut pool, mut position, _) = open_for_test(
        attachment(),
        POOL_ADDRESS,
        &tiny_config,
        rent(30),
        1_000,
        LiquidityAmounts::new(1, 0, [1, 1]).expect("tiny liquidity"),
        0,
        INITIAL_POSITION_ID,
        [41; 32],
        rent(42),
        1_000,
    )
    .expect("open");
    let pool_before = pool;
    let position_before = position;
    let request = RemoveLiquidityRequest::new(1, 1, 1, zero()).expect("remove");
    assert_eq!(
        pool.remove_liquidity(
            POOL_ADDRESS,
            &tiny_config,
            INITIAL_POSITION_ID,
            &mut position,
            request,
        ),
        Err(Error::ZeroNotional)
    );
    assert_eq!(pool, pool_before);
    assert_eq!(position, position_before);
}

#[test]
fn service_funding_never_becomes_lp_or_trade_principal() {
    let (config, mut pool, _) = opened::<2, 2>();
    let lp_before = pool.liquidity();
    pool.fund_service(POOL_ADDRESS, &config, 1, [60; 32], 100)
        .expect("fund service");
    assert_eq!(pool.liquidity(), lp_before);
    pool.spend_service(POOL_ADDRESS, &config, 2, [61; 32], 5_050)
        .expect("spend service");
    assert_eq!(pool.service_funding(), 50);
    assert_eq!(pool.liquidity(), lp_before);
    let before = pool;
    assert_eq!(
        pool.spend_service(POOL_ADDRESS, &config, 3, [61; 32], 51),
        Err(Error::InsufficientServiceFunding)
    );
    assert_eq!(pool, before);
    pool.validate_against(POOL_ADDRESS, &config)
        .expect("still valid");
}

#[test]
fn malformed_ladder_alias_profile_and_reserved_bytes_refuse() {
    assert_eq!(ParentPool::new([0; 32], 7), Err(Error::ZeroIdentity));
    assert_eq!(
        LiquidityAmounts::<1>::new(1, 0, [1]),
        Err(Error::UnsupportedProfile)
    );
    assert_eq!(
        LiquidityConfigV1::new(
            id(7),
            [20; 32],
            100,
            1,
            10,
            100,
            [[60], [60]],
            [[50], [50]],
            [[10], [10]],
            [[10], [10]],
        ),
        Err(Error::InvalidLadder)
    );
    let (_, _, position) = opened::<2, 2>();
    let mut bytes = position.to_bytes().expect("position bytes");
    *bytes
        .get_mut(POSITION_RESERVED_OFFSET)
        .expect("reserved byte") = 1;
    assert_eq!(
        LpPosition::decode(&bytes),
        Err(Error::NonCanonicalReservedBytes)
    );
}

#[test]
fn complete_set_top_of_book_no_arbitrage_is_exact_checked_and_extremal() {
    let capacities = [[10u64]; 2];
    let equality = LiquidityConfigV1::new(
        id(7),
        [20; 32],
        10_000,
        1,
        10,
        100,
        [[4_000], [6_000]],
        [[4_000], [6_000]],
        capacities,
        capacities,
    );
    assert!(equality.is_ok());

    let bid_cross_by_one = LiquidityConfigV1::new(
        id(7),
        [20; 32],
        10_000,
        10_000,
        10,
        100,
        [[5_000], [5_001]],
        [[6_000], [6_000]],
        capacities,
        capacities,
    );
    assert_eq!(bid_cross_by_one, Err(Error::CompleteSetArbitrage));

    let ask_cross_by_one = LiquidityConfigV1::new(
        id(7),
        [20; 32],
        10_000,
        1,
        10,
        100,
        [[4_000], [4_000]],
        [[4_999], [5_000]],
        capacities,
        capacities,
    );
    assert_eq!(ask_cross_by_one, Err(Error::CompleteSetArbitrage));

    let overflow = LiquidityConfigV1::new(
        id(7),
        [20; 32],
        u64::MAX,
        1,
        10,
        100,
        [[u64::MAX], [u64::MAX]],
        [[u64::MAX], [u64::MAX]],
        capacities,
        capacities,
    );
    assert_eq!(overflow, Err(Error::ArithmeticOverflow));

    let n16 = LiquidityConfigV1::new(
        id(7),
        [20; 32],
        16_000,
        1,
        10,
        100,
        [[1_000, 999]; 16],
        [[1_000, 1_001]; 16],
        [[10, 10]; 16],
        [[10, 10]; 16],
    )
    .expect("N16 equality config");
    let mut bid_sums = [0u64; 2];
    let mut ask_sums = [0u64; 2];
    for row in n16.bid_prices().iter() {
        for (sum, price) in bid_sums.iter_mut().zip(row.iter()) {
            *sum = sum.checked_add(*price).expect("bounded checked sum");
        }
    }
    for row in n16.ask_prices().iter() {
        for (sum, price) in ask_sums.iter_mut().zip(row.iter()) {
            *sum = sum.checked_add(*price).expect("bounded checked sum");
        }
    }
    assert_eq!(bid_sums, [16_000, 15_984]);
    assert_eq!(ask_sums, [16_000, 16_016]);
    assert!(bid_sums.iter().all(|sum| *sum <= n16.price_scale()));
    assert!(ask_sums.iter().all(|sum| *sum >= n16.price_scale()));
}

#[test]
fn tampered_trade_and_lp_receipts_fail_conservation() {
    let (config, mut pool, _) = opened::<2, 2>();
    let request = TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 0, 10, 20).expect("request");
    let receipt = pool
        .execute(POOL_ADDRESS, &config, request)
        .expect("execution");
    let mut bad_fee = receipt;
    bad_fee.fees_after = bad_fee.fees_after.checked_add(1).expect("fixture addition");
    assert_eq!(bad_fee.validate(), Err(Error::ConservationMismatch));
    let mut bad_fill = receipt;
    let first_fill = bad_fill.bin_after.first_mut().expect("first bin");
    *first_fill = first_fill.checked_add(1).expect("fixture addition");
    assert_eq!(bad_fill.validate(), Err(Error::ConservationMismatch));

    let (mut newcomer, _) = pool
        .create_position(POOL_ADDRESS, &config, 2, [50; 32], [51; 32], rent(52))
        .expect("position");
    let add = AddLiquidityRequest::new(3, 1, 10, maximum()).expect("add request");
    let receipt = pool
        .add_liquidity(POOL_ADDRESS, &config, [50; 32], &mut newcomer, add)
        .expect("add liquidity");
    let mut bad_lp = receipt;
    bad_lp.amounts_after.principal_collateral = bad_lp
        .amounts_after
        .principal_collateral
        .checked_add(1)
        .expect("fixture addition");
    assert_eq!(bad_lp.validate(), Err(Error::ConservationMismatch));
}

#[test]
fn n16_uses_same_bounded_execution_path() {
    let (config, mut pool, _) = opened::<16, 8>();
    let request =
        TradeRequest::new(0, 1, TradeSide::BuyClaimFromPool, 15, 1_500, 1_000).expect("request");
    let receipt = pool
        .execute(POOL_ADDRESS, &config, request)
        .expect("N16 execution");
    assert_eq!(receipt.claim_index(), 15);
    assert_eq!(receipt.quantity(), 1_500);
    assert_eq!(
        pool.liquidity().claim_reserves().last().copied(),
        Some(8_500)
    );
    pool.validate_against(POOL_ADDRESS, &config)
        .expect("N16 invariant");
}
