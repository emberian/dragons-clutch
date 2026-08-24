extern crate std;

use std::{vec, vec::Vec};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    FundingQuoteV1, FundingStateV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};

use crate::{
    LiquidityAttachment, LiquidityConfigV1, ParentPool, RentCreditTerms,
    activation::{ActivationError, activate_pool},
    frame::{
        ConfigPdaSeedsV1, DEALER_CONFIG_PDA_DOMAIN_V1, DEALER_LP_PDA_DOMAIN_V1,
        DEALER_POOL_PDA_DOMAIN_V1, DEALER_RENT_SYSVAR_ID, DEALER_SYSTEM_PROGRAM_ID,
        DealerAccountMetaV1, DealerAccountRoleV1, DealerFrameV1, FrameError, LpPositionPdaSeedsV1,
        PoolPdaSeedsV1, dealer_account_count, dealer_account_privileges, dealer_account_role,
        validate_market_phase,
    },
    instruction::{
        ActivatePoolV1, AddLiquidityV1, CloseLpPositionV1, CreateLpPositionV1, DealerActionV1,
        DealerInstructionV1, InstructionError, RemoveLiquidityV1, instruction_len,
    },
};

const MARKET_ADDRESS: [u8; 32] = [90; 32];
const POOL_ADDRESS: [u8; 32] = [91; 32];
const LP_ADDRESS: [u8; 32] = [92; 32];
const OWNER: [u8; 32] = [93; 32];

fn id(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).expect("nonzero content ID")
}

fn rent(amount: u64) -> RentCreditTerms {
    RentCreditTerms::new(OWNER, amount).expect("rent")
}

fn config<const N: usize, const B: usize>() -> LiquidityConfigV1<N, B> {
    let mut bids = [[0u64; B]; N];
    let mut asks = [[0u64; B]; N];
    let capacity = [[10_000u64; B]; N];
    let count = u64::try_from(N).expect("bounded N");
    for (bid_row, ask_row) in bids.iter_mut().zip(asks.iter_mut()) {
        for (index, (bid, ask)) in bid_row.iter_mut().zip(ask_row.iter_mut()).enumerate() {
            let step = u64::try_from(index).expect("bounded B");
            *bid = 8_000 / count - step;
            *ask = 12_000_u64.div_ceil(count) + step;
        }
    }
    LiquidityConfigV1::new(
        id(7),
        ParentPool::new(POOL_ADDRESS, 9).expect("parent"),
        rent(55),
        10_000,
        25,
        1_000,
        100,
        bids,
        asks,
        capacity,
        capacity,
    )
    .expect("config")
}

#[test]
fn instruction_round_trips_and_refuses_hostile_envelopes() {
    let limits = crate::LiquidityAmounts::new(1_000, 50, [700, 800]).expect("limits");
    let add = DealerInstructionV1::AddLiquidity(
        AddLiquidityV1::new(4, 8, 12, [44; 32], limits).expect("add"),
    );
    let mut bytes = vec![0u8; add.encoded_len().expect("width")];
    add.encode_into(&mut bytes).expect("encode");
    assert_eq!(DealerInstructionV1::<2>::decode(&bytes), Ok(add));
    assert_eq!(bytes.len(), 104);
    assert_eq!(instruction_len::<16>(DealerActionV1::AddLiquidity), Ok(216));

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        DealerInstructionV1::<2>::decode(&trailing),
        Err(InstructionError::InvalidLength)
    );
    let mut reserved = bytes.clone();
    *reserved.get_mut(12).expect("header byte") = 1;
    assert_eq!(
        DealerInstructionV1::<2>::decode(&reserved),
        Err(InstructionError::NonCanonicalReservedBytes)
    );
    let mut magic = bytes;
    *magic.first_mut().expect("magic byte") ^= 1;
    assert_eq!(
        DealerInstructionV1::<2>::decode(&magic),
        Err(InstructionError::InvalidMagic)
    );

    let trade = DealerInstructionV1::<2>::Trade(
        crate::TradeRequest::new(2, 7, crate::TradeSide::BuyClaimFromPool, 1, 4, 99)
            .expect("trade"),
    );
    let mut trade_bytes = [0u8; 56];
    trade.encode_into(&mut trade_bytes).expect("trade encode");
    assert_eq!(DealerInstructionV1::<2>::decode(&trade_bytes), Ok(trade));
    trade_bytes[34] = 1;
    assert_eq!(
        DealerInstructionV1::<2>::decode(&trade_bytes),
        Err(InstructionError::NonCanonicalReservedBytes)
    );

    let remove = DealerInstructionV1::RemoveLiquidity(
        RemoveLiquidityV1::new(5, 9, 3, [44; 32], limits).expect("remove"),
    );
    let values = [
        DealerInstructionV1::ActivatePool(
            ActivatePoolV1::new(9, 2, [44; 32], 100, 50).expect("activate"),
        ),
        DealerInstructionV1::CreateLpPosition(
            CreateLpPositionV1::new(4, [44; 32]).expect("create"),
        ),
        remove,
        DealerInstructionV1::ResetLadder {
            expected_pool_sequence: 11,
        },
        DealerInstructionV1::CloseLpPosition(
            CloseLpPositionV1::new(12, 3, [44; 32]).expect("close"),
        ),
        DealerInstructionV1::RetirePool {
            expected_pool_sequence: 13,
            expected_market_child_count: 1,
        },
    ];
    for value in values {
        let mut exact = vec![0u8; value.encoded_len().expect("action width")];
        value.encode_into(&mut exact).expect("action encode");
        assert_eq!(DealerInstructionV1::<2>::decode(&exact), Ok(value));
    }
}

fn frame<const N: usize>(action: DealerActionV1) -> Vec<DealerAccountMetaV1> {
    let count = dealer_account_count::<N>(action).expect("count");
    (0..count)
        .map(|index| {
            let role = dealer_account_role::<N>(action, index).expect("role");
            let (is_signer, is_writable, is_executable) = dealer_account_privileges(action, role);
            let key = match role {
                DealerAccountRoleV1::SystemProgram => DEALER_SYSTEM_PROGRAM_ID,
                DealerAccountRoleV1::RentSysvar => DEALER_RENT_SYSVAR_ID,
                _ => [u8::try_from(index + 1).expect("bounded account count"); 32],
            };
            DealerAccountMetaV1 {
                key,
                is_signer,
                is_writable,
                is_executable,
            }
        })
        .collect()
}

#[test]
fn exact_frames_enforce_counts_privileges_and_explicit_aliases() {
    for action in [
        DealerActionV1::ActivatePool,
        DealerActionV1::CreateLpPosition,
        DealerActionV1::AddLiquidity,
        DealerActionV1::RemoveLiquidity,
        DealerActionV1::Trade,
        DealerActionV1::ResetLadder,
        DealerActionV1::CloseLpPosition,
        DealerActionV1::RetirePool,
    ] {
        let accounts = frame::<16>(action);
        let result = DealerFrameV1::<16>::new(action, &accounts);
        assert!(result.is_ok(), "frame failed for {action:?}: {result:?}");
    }
    assert_eq!(
        dealer_account_count::<2>(DealerActionV1::ActivatePool),
        Ok(23)
    );
    assert_eq!(
        dealer_account_count::<16>(DealerActionV1::ActivatePool),
        Ok(37)
    );
    assert_eq!(
        dealer_account_count::<16>(DealerActionV1::AddLiquidity),
        Ok(28)
    );
    assert_eq!(
        dealer_account_count::<16>(DealerActionV1::RetirePool),
        Ok(29)
    );

    let mut hostile = frame::<2>(DealerActionV1::Trade);
    let market_key = hostile.get(2).expect("Market role").key;
    hostile.get_mut(3).expect("Pool role").key = market_key;
    assert_eq!(
        DealerFrameV1::<2>::new(DealerActionV1::Trade, &hostile),
        Err(FrameError::UnsafeAlias)
    );
    let mut privilege = frame::<2>(DealerActionV1::Trade);
    privilege.get_mut(3).expect("Pool role").is_signer = true;
    assert_eq!(
        DealerFrameV1::<2>::new(DealerActionV1::Trade, &privilege),
        Err(FrameError::InvalidPrivilege)
    );

    let mut safe = frame::<2>(DealerActionV1::ActivatePool);
    let activator_key = safe.first().expect("activator role").key;
    safe.get_mut(1).expect("LP owner role").key = activator_key;
    // Pool/config/LP RentCredits may be the same permanent beneficiary credit.
    let rent_credit_key = safe.get(16).expect("Pool RentCredit role").key;
    safe.get_mut(17).expect("config RentCredit role").key = rent_credit_key;
    safe.get_mut(18).expect("LP RentCredit role").key = rent_credit_key;
    assert!(DealerFrameV1::<2>::new(DealerActionV1::ActivatePool, &safe).is_ok());
}

#[test]
fn clock_is_not_a_meta_and_phase_contract_is_exact() {
    assert_eq!(
        dealer_account_count::<2>(DealerActionV1::ResetLadder),
        Ok(3)
    );
    let reset = frame::<2>(DealerActionV1::ResetLadder);
    assert_eq!(
        reset
            .iter()
            .map(|account| account.key)
            .collect::<Vec<_>>()
            .len(),
        3
    );
    assert_eq!(
        validate_market_phase(DealerActionV1::Trade, Phase::Open),
        Ok(())
    );
    assert_eq!(
        validate_market_phase(DealerActionV1::Trade, Phase::Resolved),
        Err(FrameError::InvalidMarketPhase)
    );
    assert_eq!(
        validate_market_phase(DealerActionV1::RemoveLiquidity, Phase::Retiring),
        Ok(())
    );
    assert_eq!(
        validate_market_phase(DealerActionV1::RetirePool, Phase::Retiring),
        Ok(())
    );
    assert_eq!(
        validate_market_phase(DealerActionV1::RetirePool, Phase::Open),
        Err(FrameError::InvalidMarketPhase)
    );
}

fn short_vec_width(value: usize) -> usize {
    if value < 128 {
        1
    } else if value < 16_384 {
        2
    } else {
        3
    }
}

fn legacy_single_instruction_bytes(accounts: usize, data: usize, frame_has_signer: bool) -> usize {
    // One signature, one Dealer instruction, one Dealer program key, and a
    // separate static fee payer only when the exact frame has no signer.
    let separate_payer = usize::from(!frame_has_signer);
    136 + 32 * separate_payer + 33 * accounts + short_vec_width(data) + data
}

#[test]
fn exact_n16_account_and_legacy_packet_risk_is_locked() {
    let activate_accounts =
        dealer_account_count::<16>(DealerActionV1::ActivatePool).expect("Activate account count");
    let add_accounts =
        dealer_account_count::<16>(DealerActionV1::AddLiquidity).expect("Add account count");
    let retire_accounts =
        dealer_account_count::<16>(DealerActionV1::RetirePool).expect("Retire account count");
    let activate_bytes = legacy_single_instruction_bytes(activate_accounts, 80, true);
    let add_bytes = legacy_single_instruction_bytes(add_accounts, 216, true);
    let retire_bytes = legacy_single_instruction_bytes(retire_accounts, 32, false);
    assert_eq!(activate_bytes, 1_438);
    assert_eq!(add_bytes, 1_278);
    assert_eq!(retire_bytes, 1_158);
    assert!(activate_bytes > crate::frame::SOLANA_PACKET_DATA_SIZE_V1);
    assert!(add_bytes > crate::frame::SOLANA_PACKET_DATA_SIZE_V1);
    assert!(retire_bytes < crate::frame::SOLANA_PACKET_DATA_SIZE_V1);
    assert!(activate_accounts < crate::frame::SOLANA_ACCOUNT_LOCK_LIMIT_V1);
}

#[test]
fn pda_preimages_are_domain_separated_and_substitution_sensitive() {
    let pool = PoolPdaSeedsV1::new(MARKET_ADDRESS, 9, id(7)).expect("pool seeds");
    let config = ConfigPdaSeedsV1::new(MARKET_ADDRESS, 9, id(7)).expect("config seeds");
    let lp = LpPositionPdaSeedsV1::new(MARKET_ADDRESS, 9, id(7), [44; 32]).expect("LP seeds");
    assert_eq!(pool.seed_components()[0], DEALER_POOL_PDA_DOMAIN_V1);
    assert_eq!(config.seed_components()[0], DEALER_CONFIG_PDA_DOMAIN_V1);
    assert_eq!(lp.seed_components()[0], DEALER_LP_PDA_DOMAIN_V1);
    assert_eq!(pool.seed_components()[1], MARKET_ADDRESS.as_slice());
    assert_eq!(pool.seed_components()[2], 9u64.to_le_bytes().as_slice());
    assert_eq!(pool.seed_components()[3], id(7).as_bytes());
    assert_ne!(pool.seed_components()[0], config.seed_components()[0]);
    assert_ne!(
        lp.seed_components(),
        LpPositionPdaSeedsV1::new(MARKET_ADDRESS, 9, id(7), [45; 32])
            .expect("substitution")
            .seed_components()
    );
    assert_eq!(
        PoolPdaSeedsV1::new([0; 32], 9, id(7)),
        Err(FrameError::ZeroIdentity)
    );
}

#[test]
fn activation_uses_shared_funding_authority_and_chain_derived_amounts() {
    let quote = FundingQuoteV1::new(300, 10, 0, 0, 0, 100_000, 5_000).expect("quote");
    let entry = CapabilityEntryV1::new(
        id(6),
        id(21),
        id(7),
        id(23),
        id(24),
        id(25),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote,
    )
    .expect("entry");
    let mut manifest_storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest =
        CapabilityManifestV1::encode_into(&[entry], &mut manifest_storage).expect("manifest");
    let market_identity = MarketIdentity::new(id(1), id(2), id(3), id(4), id(5), 9);
    let market = MarketRoot::founding(market_identity, OWNER).expect("market");
    let funding =
        FundingStateV1::new(id(5), manifest, 0, quote.total_principal()).expect("funding");
    let attachment =
        LiquidityAttachment::new(market_identity, id(21), id(7), OWNER).expect("attachment");
    let request = ActivatePoolV1::new(9, 0, [44; 32], 1_000, 1_000).expect("open wire");
    let config = config::<2, 2>();
    let plan = activate_pool(
        market,
        MARKET_ADDRESS,
        manifest,
        funding,
        quote.total_principal(),
        attachment,
        &config,
        POOL_ADDRESS,
        LP_ADDRESS,
        OWNER,
        rent(100),
        rent(200),
        request,
        50,
    )
    .expect("activation plan");
    assert_eq!(plan.market().outstanding_children(), 1);
    assert_eq!(plan.funding().remaining().total_principal(), 0);
    assert_eq!(plan.funding_debit().liquidity_principal(), 100_000);
    assert_eq!(plan.funding_debit().service_principal(), 5_000);
    assert_eq!(plan.pool().liquidity().claim_reserves(), [1_000; 2]);
    assert_eq!(plan.capability_funding_seeds().config_id(), [7; 32]);
    assert_eq!(plan.lp_seeds().lp_id(), [44; 32]);

    assert_eq!(
        activate_pool(
            market,
            MARKET_ADDRESS,
            manifest,
            funding,
            quote.total_principal(),
            attachment,
            &config,
            POOL_ADDRESS,
            LP_ADDRESS,
            [94; 32],
            rent(100),
            rent(200),
            request,
            50,
        ),
        Err(ActivationError::AuthorityMismatch)
    );
}
