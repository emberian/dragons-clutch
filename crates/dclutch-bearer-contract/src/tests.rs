use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, FundingAmountsV1, FundingCustodyObservationV1, FundingQuoteV1,
    FundingStateV1, FundingStatus, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::terminal::ResolutionKind;
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, PositionV1, RealmV1, RealmV1Input,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

use super::frame::{AccountMetaV1, validate_account_frame};
use super::instruction::{
    ACTIVATE_INSTRUCTION_BYTES, ActionV1, InstructionV1, OUTCOME_INSTRUCTION_BYTES,
};
use super::state::{
    BEARER_CAPABILITY_KIND_ID, BEARER_CHILD_DERIVATION_ID, BEARER_CHILD_SCHEMA_ID,
    BEARER_CONFIG_BYTES, BEARER_SEMANTIC_RELEASE_ID, BEARER_STATE_BASE_BYTES, BearerCapabilityV1,
    BearerConfigV1, BearerMintDerivationV1, MintObservationV1, TokenAccountObservationV1,
    TokenAccountStateV1,
};
use super::transition::{
    RealmBindingV1, activate, audit_mints, dematerialize, materialize, merge_from_bearer,
    redeem_bearer, redeem_native, retire, split_to_bearer, split_to_position, transfer,
};
use super::{Error, Result};

const MARKET_KEY: [u8; 32] = [41; 32];
const OWNER: [u8; 32] = [42; 32];
const OTHER_OWNER: [u8; 32] = [43; 32];
const CONTROLLER: [u8; 32] = [44; 32];
const GENERATION: u64 = 9;
const MANIFEST_ID_BYTE: u8 = 5;
const REALM_ID_BYTE: u8 = 1;

fn id(byte: u8) -> ContentId {
    match ContentId::new([byte; 32]) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    }
}

#[cold]
fn spin_forever() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

fn open_market<const N: usize>() -> CategoricalMarketV1<N> {
    market_with_parts(0, [0; N], true)
}

fn founding_market<const N: usize>() -> CategoricalMarketV1<N> {
    market_with_parts(0, [0; N], false)
}

fn market_with_parts<const N: usize>(
    hoard: u64,
    supply: [u64; N],
    open: bool,
) -> CategoricalMarketV1<N> {
    let identity = MarketIdentity::new(
        id(REALM_ID_BYTE),
        id(2),
        id(3),
        id(4),
        id(MANIFEST_ID_BYTE),
        GENERATION,
    );
    let mut root = match MarketRoot::founding(identity, [90; 32]) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    };
    if open && root.transition_phase(GENERATION, Phase::Open).is_err() {
        spin_forever();
    }
    match CategoricalMarketV1::new(root, hoard, supply, CategoricalSettlementSummaryV1::empty()) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    }
}

fn realm() -> RealmBindingV1 {
    let record = match RealmV1::new(RealmV1Input {
        token_program: [61; 32],
        collateral_mint: [62; 32],
        collateral_adapter_release_id: [63; 32],
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    }) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    };
    RealmBindingV1 {
        content_id: id(REALM_ID_BYTE),
        realm: record,
    }
}

fn mint_keys<const N: usize>() -> [[u8; 32]; N] {
    core::array::from_fn(|index| {
        let byte = match u8::try_from(index) {
            Ok(value) => value.saturating_add(100),
            Err(_) => 255,
        };
        [byte; 32]
    })
}

fn key_at<const N: usize>(keys: &[[u8; 32]; N], index: usize) -> [u8; 32] {
    match keys.get(index) {
        Some(value) => *value,
        None => spin_forever(),
    }
}

fn mint_observation(key: [u8; 32], supply: u64) -> MintObservationV1 {
    MintObservationV1 {
        key,
        program_owner: TOKEN_2022_PROGRAM_ID,
        data_len: super::state::BEARER_MINT_BYTES,
        supply,
        decimals: 0,
        initialized: true,
        mint_authority: Some(CONTROLLER),
        freeze_authority: None,
        close_authority: Some(CONTROLLER),
        permissioned_burn_authority: Some(CONTROLLER),
        extension_count: 2,
    }
}

fn token_account(
    key_byte: u8,
    mint: [u8; 32],
    authority: [u8; 32],
    amount: u64,
) -> TokenAccountObservationV1 {
    TokenAccountObservationV1 {
        key: [key_byte; 32],
        program_owner: TOKEN_2022_PROGRAM_ID,
        data_len: super::state::BEARER_TOKEN_ACCOUNT_BYTES,
        mint,
        authority,
        amount,
        state: TokenAccountStateV1::Initialized,
        has_native_reserve: false,
        extension_count: 0,
    }
}

fn resolve<const N: usize>(market: &mut CategoricalMarketV1<N>, winner: usize) -> Result<()> {
    let evidence_id = match dclutch_product_contract::ContentId::new([77; 32]) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    };
    let summary = CategoricalSettlementSummaryV1::resolved::<N>(
        evidence_id,
        ResolutionKind::Occurrence,
        winner,
        1,
    )
    .map_err(|error| Error::MarketContract { error })?;
    market
        .resolve_with_summary(GENERATION, summary)
        .map_err(|error| Error::MarketContract { error })
}

#[test]
fn config_and_state_are_exact_fixed_layouts() -> Result<()> {
    let config = BearerConfigV1::new(TOKEN_2022_PROGRAM_ID, [70; 32])?;
    let bytes = config.to_bytes();
    assert_eq!(bytes.len(), BEARER_CONFIG_BYTES);
    assert_eq!(BearerConfigV1::decode(&bytes), Ok(config));
    let mut noncanonical = bytes;
    noncanonical[10] = 1;
    assert_eq!(
        BearerConfigV1::decode(&noncanonical),
        Err(Error::NonCanonicalReservedBytes)
    );
    assert_eq!(
        BearerConfigV1::new([1; 32], [70; 32]),
        Err(Error::WrongTokenProgram)
    );

    let state = BearerCapabilityV1::<2>::activated(MARKET_KEY, GENERATION, 3)?;
    let mut state_bytes = [0u8; BEARER_STATE_BASE_BYTES + 16];
    state.encode(&mut state_bytes)?;
    assert_eq!(BearerCapabilityV1::<2>::decode(&state_bytes), Ok(state));
    let before = [9u8; BEARER_STATE_BASE_BYTES + 16];
    let mut wrong_output = before;
    assert_eq!(
        state.encode(&mut wrong_output[..7]),
        Err(Error::OutputLength)
    );
    assert_eq!(wrong_output, before);
    assert_eq!(
        BearerCapabilityV1::<1>::encoded_len(),
        Err(Error::InvalidOutcomeCount)
    );
    assert_eq!(
        BearerCapabilityV1::<17>::encoded_len(),
        Err(Error::InvalidOutcomeCount)
    );
    assert_eq!(
        BearerMintDerivationV1::new::<2>(MARKET_KEY, GENERATION, 2),
        Err(Error::InvalidOutcome)
    );
    Ok(())
}

fn width_round_trip<const N: usize>() -> Result<()> {
    let market = open_market::<N>();
    let state = BearerCapabilityV1::<N>::activated(MARKET_KEY, GENERATION, 0)?;
    assert_eq!(BearerCapabilityV1::<N>::encoded_len()?, 64 + 8 * N);
    state.validate_market(MARKET_KEY, &market)?;
    let keys = mint_keys::<N>();
    let observations = core::array::from_fn(|index| mint_observation(key_at(&keys, index), 0));
    audit_mints(&state, MARKET_KEY, &market, CONTROLLER, keys, observations)
}

#[test]
fn provisional_profile_executes_every_width_two_through_sixteen() -> Result<()> {
    width_round_trip::<2>()?;
    width_round_trip::<3>()?;
    width_round_trip::<4>()?;
    width_round_trip::<5>()?;
    width_round_trip::<6>()?;
    width_round_trip::<7>()?;
    width_round_trip::<8>()?;
    width_round_trip::<9>()?;
    width_round_trip::<10>()?;
    width_round_trip::<11>()?;
    width_round_trip::<12>()?;
    width_round_trip::<13>()?;
    width_round_trip::<14>()?;
    width_round_trip::<15>()?;
    width_round_trip::<16>()
}

#[test]
fn materialization_is_a_conservative_swap_and_supply_drift_refuses() -> Result<()> {
    let mut market = open_market::<2>();
    let mut position = PositionV1::<2>::empty(MARKET_KEY, OWNER, GENERATION)
        .map_err(|error| Error::RealmContract { error })?;
    split_to_position(MARKET_KEY, &mut market, &mut position, OWNER, realm(), 10)?;
    let mut state = BearerCapabilityV1::<2>::activated(MARKET_KEY, GENERATION, 0)?;
    let key = mint_keys::<2>()[0];
    let mint = mint_observation(key, 0);
    let destination = token_account(81, key, OWNER, 0);
    let plan = materialize(
        MARKET_KEY,
        &market,
        &mut state,
        &mut position,
        OWNER,
        0,
        6,
        CONTROLLER,
        key,
        mint,
        destination,
    )?;
    assert_eq!(market.supply(), &[10, 10]);
    assert_eq!(position.balances(), &[4, 10]);
    assert_eq!(state.accounted_supply(), &[6, 0]);
    assert_eq!(plan.mint_supply_after, 6);

    let position_before = position;
    let state_before = state;
    assert_eq!(
        materialize(
            MARKET_KEY,
            &market,
            &mut state,
            &mut position,
            OWNER,
            0,
            1,
            CONTROLLER,
            key,
            mint,
            destination,
        ),
        Err(Error::UnaccountedMintSupply)
    );
    assert_eq!(position, position_before);
    assert_eq!(state, state_before);

    let keys = mint_keys::<2>();
    let unchanged_after_external_transfer =
        [mint_observation(keys[0], 6), mint_observation(keys[1], 0)];
    assert_eq!(
        audit_mints(
            &state,
            MARKET_KEY,
            &market,
            CONTROLLER,
            keys,
            unchanged_after_external_transfer,
        ),
        Ok(())
    );
    let external_burn = [mint_observation(keys[0], 5), mint_observation(keys[1], 0)];
    assert_eq!(
        audit_mints(&state, MARKET_KEY, &market, CONTROLLER, keys, external_burn),
        Err(Error::UnaccountedMintSupply)
    );

    let burn = dematerialize(
        MARKET_KEY,
        &market,
        &mut state,
        &mut position,
        OWNER,
        0,
        6,
        CONTROLLER,
        key,
        mint_observation(key, 6),
        token_account(81, key, OWNER, 6),
    )?;
    assert_eq!(burn.mint_supply_after, 0);
    assert_eq!(position.balances(), &[10, 10]);
    assert_eq!(state.accounted_supply(), &[0, 0]);
    Ok(())
}

#[test]
fn transfer_requires_initialized_base_accounts_and_preserves_supply() -> Result<()> {
    let mut market = open_market::<2>();
    let mut position = PositionV1::<2>::new(MARKET_KEY, OWNER, GENERATION, [8, 0])
        .map_err(|error| Error::RealmContract { error })?;
    market
        .split_complete_set(8)
        .map_err(|error| Error::MarketContract { error })?;
    let mut state = BearerCapabilityV1::<2>::activated(MARKET_KEY, GENERATION, 0)?;
    let key = mint_keys::<2>()[0];
    materialize(
        MARKET_KEY,
        &market,
        &mut state,
        &mut position,
        OWNER,
        0,
        8,
        CONTROLLER,
        key,
        mint_observation(key, 0),
        token_account(81, key, OWNER, 0),
    )?;
    let plan = transfer(
        MARKET_KEY,
        &market,
        &state,
        0,
        3,
        CONTROLLER,
        OWNER,
        key,
        mint_observation(key, 8),
        token_account(81, key, OWNER, 8),
        token_account(82, key, OTHER_OWNER, 1),
    )?;
    assert_eq!(plan.source_balance_after, 5);
    assert_eq!(plan.destination_balance_after, 4);
    assert_eq!(plan.unchanged_mint_supply, 8);
    let mut frozen = token_account(81, key, OWNER, 8);
    frozen.state = TokenAccountStateV1::Frozen;
    assert_eq!(
        transfer(
            MARKET_KEY,
            &market,
            &state,
            0,
            3,
            CONTROLLER,
            OWNER,
            key,
            mint_observation(key, 8),
            frozen,
            token_account(82, key, OTHER_OWNER, 1),
        ),
        Err(Error::TokenAccountNotTransferable)
    );
    Ok(())
}

#[test]
fn bearer_and_native_redemption_cannot_redeem_the_same_atoms_twice() -> Result<()> {
    let mut market = open_market::<2>();
    let mut position = PositionV1::<2>::empty(MARKET_KEY, OWNER, GENERATION)
        .map_err(|error| Error::RealmContract { error })?;
    split_to_position(MARKET_KEY, &mut market, &mut position, OWNER, realm(), 10)?;
    let mut state = BearerCapabilityV1::<2>::activated(MARKET_KEY, GENERATION, 0)?;
    let key = mint_keys::<2>()[1];
    materialize(
        MARKET_KEY,
        &market,
        &mut state,
        &mut position,
        OWNER,
        1,
        7,
        CONTROLLER,
        key,
        mint_observation(key, 0),
        token_account(81, key, OWNER, 0),
    )?;
    resolve(&mut market, 1)?;
    let bearer = redeem_bearer(
        MARKET_KEY,
        &mut market,
        &mut state,
        OWNER,
        realm(),
        1,
        7,
        CONTROLLER,
        key,
        mint_observation(key, 7),
        token_account(81, key, OWNER, 7),
    )?;
    assert_eq!(bearer.payout.amount(), 7);
    assert_eq!(market.supply(), &[10, 3]);
    assert_eq!(market.hoard_atoms(), 3);
    assert_eq!(state.accounted_supply(), &[0, 0]);
    assert_eq!(
        redeem_bearer(
            MARKET_KEY,
            &mut market,
            &mut state,
            OWNER,
            realm(),
            1,
            1,
            CONTROLLER,
            key,
            mint_observation(key, 0),
            token_account(81, key, OWNER, 0),
        ),
        Err(Error::InsufficientTokenBalance)
    );
    let native = redeem_native(MARKET_KEY, &mut market, &mut position, OWNER, realm(), 1, 3)?;
    assert_eq!(native.payout.amount(), 3);
    assert_eq!(market.hoard_atoms(), 0);
    assert_eq!(
        redeem_native(MARKET_KEY, &mut market, &mut position, OWNER, realm(), 1, 1,),
        Err(Error::RealmContract {
            error: dclutch_realm_contract::Error::InsufficientBalance
        })
    );
    Ok(())
}

#[test]
fn direct_bearer_split_and_merge_update_all_representations_atomically() -> Result<()> {
    let mut market = open_market::<3>();
    let mut state = BearerCapabilityV1::<3>::activated(MARKET_KEY, GENERATION, 0)?;
    let keys = mint_keys::<3>();
    let mints = core::array::from_fn(|index| mint_observation(key_at(&keys, index), 0));
    let destinations = core::array::from_fn(|index| {
        let byte = match u8::try_from(index) {
            Ok(value) => value.saturating_add(80),
            Err(_) => 255,
        };
        token_account(byte, key_at(&keys, index), OWNER, 0)
    });
    let (deposit, mint_plans) = split_to_bearer(
        MARKET_KEY,
        &mut market,
        &mut state,
        OWNER,
        realm(),
        12,
        CONTROLLER,
        keys,
        mints,
        destinations,
    )?;
    assert_eq!(deposit.amount(), 12);
    assert_eq!(market.supply(), &[12, 12, 12]);
    assert_eq!(state.accounted_supply(), &[12, 12, 12]);
    assert!(mint_plans.iter().all(|plan| plan.mint_supply_after == 12));

    let mints = core::array::from_fn(|index| mint_observation(key_at(&keys, index), 12));
    let sources = core::array::from_fn(|index| {
        let byte = match u8::try_from(index) {
            Ok(value) => value.saturating_add(80),
            Err(_) => 255,
        };
        token_account(byte, key_at(&keys, index), OWNER, 12)
    });
    let (withdrawal, burn_plans) = merge_from_bearer(
        MARKET_KEY,
        &mut market,
        &mut state,
        OWNER,
        realm(),
        5,
        CONTROLLER,
        keys,
        mints,
        sources,
    )?;
    assert_eq!(withdrawal.amount(), 5);
    assert_eq!(market.supply(), &[7, 7, 7]);
    assert_eq!(state.accounted_supply(), &[7, 7, 7]);
    assert!(burn_plans.iter().all(|plan| plan.mint_supply_after == 7));
    Ok(())
}

#[test]
fn wrong_mint_authority_extension_key_and_arithmetic_refuse_atomically() -> Result<()> {
    let market = market_with_parts::<2>(u64::MAX, [u64::MAX; 2], true);
    let mut state = BearerCapabilityV1::<2>::activated(MARKET_KEY, GENERATION, 0)?;
    state.credit(0, u64::MAX)?;
    let key = mint_keys::<2>()[0];
    let mut wrong = mint_observation(key, u64::MAX);
    wrong.extension_count = 3;
    assert_eq!(
        audit_mints(
            &state,
            MARKET_KEY,
            &market,
            CONTROLLER,
            mint_keys::<2>(),
            [wrong, mint_observation(mint_keys::<2>()[1], 0)],
        ),
        Err(Error::WrongMintExtensions)
    );
    wrong = mint_observation(key, u64::MAX);
    wrong.mint_authority = Some([99; 32]);
    assert_eq!(
        materialize(
            MARKET_KEY,
            &market,
            &mut state,
            &mut PositionV1::<2>::new(MARKET_KEY, OWNER, GENERATION, [1, 0])
                .map_err(|error| Error::RealmContract { error })?,
            OWNER,
            0,
            1,
            CONTROLLER,
            key,
            wrong,
            token_account(81, key, OWNER, 0),
        ),
        Err(Error::WrongAuthority)
    );
    let state_before = state;
    let mut position = PositionV1::<2>::new(MARKET_KEY, OWNER, GENERATION, [1, 0])
        .map_err(|error| Error::RealmContract { error })?;
    assert_eq!(
        materialize(
            MARKET_KEY,
            &market,
            &mut state,
            &mut position,
            OWNER,
            0,
            1,
            CONTROLLER,
            key,
            mint_observation(key, u64::MAX),
            token_account(81, key, OWNER, 0),
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(state, state_before);
    assert_eq!(position.balances(), &[1, 0]);
    let wrong_key = [199; 32];
    assert_eq!(
        transfer(
            MARKET_KEY,
            &market,
            &state,
            0,
            1,
            CONTROLLER,
            OWNER,
            wrong_key,
            mint_observation(key, u64::MAX),
            token_account(81, key, OWNER, 1),
            token_account(82, key, OTHER_OWNER, 0),
        ),
        Err(Error::WrongMint)
    );
    Ok(())
}

fn bearer_entry(config_id: ContentId) -> CapabilityEntryV1 {
    let rent = match CompartmentFundingV1::native_lamports(100) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    };
    let creation = match CompartmentFundingV1::native_lamports(20) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    };
    let amounts = match FundingAmountsV1::new(
        rent,
        creation,
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
        CompartmentFundingV1::not_applicable(),
    ) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    };
    let quote = match FundingQuoteV1::new(amounts, None) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    };
    match CapabilityEntryV1::new(
        id_from_bytes(BEARER_CAPABILITY_KIND_ID),
        id_from_bytes(BEARER_SEMANTIC_RELEASE_ID),
        config_id,
        id(12),
        id_from_bytes(BEARER_CHILD_SCHEMA_ID),
        id_from_bytes(BEARER_CHILD_DERIVATION_ID),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote,
    ) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    }
}

fn id_from_bytes(bytes: [u8; 32]) -> ContentId {
    match ContentId::new(bytes) {
        Ok(value) => value,
        Err(_) => spin_forever(),
    }
}

#[test]
fn activation_and_retirement_are_manifest_funded_child_counted_and_replay_bound() -> Result<()> {
    let config_id = id(11);
    let config = BearerConfigV1::new(TOKEN_2022_PROGRAM_ID, [70; 32])?;
    let entry = bearer_entry(config_id);
    let mut manifest_bytes = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest = CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes)
        .map_err(|error| Error::CapabilityContract { error })?;
    let custody = FundingCustodyObservationV1::native_only(150, 30)
        .map_err(|error| Error::CapabilityContract { error })?;
    let mut funding = FundingStateV1::new(id(MANIFEST_ID_BYTE), manifest, 0, custody)
        .map_err(|error| Error::CapabilityContract { error })?;
    let mut market = founding_market::<2>();
    let keys = mint_keys::<2>();
    let (state, plan) = activate(
        MARKET_KEY,
        &mut market,
        id(MANIFEST_ID_BYTE),
        manifest,
        config_id,
        config,
        &mut funding,
        custody,
        7,
        100,
        20,
        0,
        CONTROLLER,
        keys,
    )?;
    assert_eq!(market.root().outstanding_children(), 1);
    assert_eq!(funding.status(), FundingStatus::Active);
    assert_eq!(plan.rent_lamports, 100);
    assert_eq!(plan.mints[1].derivation.outcome(), 1);

    let stale = BearerCapabilityV1::<2>::activated(MARKET_KEY, GENERATION + 1, 0)?;
    assert_eq!(
        stale.validate_market(MARKET_KEY, &market),
        Err(Error::GenerationMismatch)
    );

    market
        .transition_phase(GENERATION, Phase::Retiring)
        .map_err(|error| Error::MarketContract { error })?;
    let observations = [mint_observation(keys[0], 0), mint_observation(keys[1], 0)];
    let retirement = retire(
        MARKET_KEY,
        &mut market,
        state,
        id(MANIFEST_ID_BYTE),
        manifest,
        config_id,
        config,
        1,
        CONTROLLER,
        keys,
        observations,
    )?;
    assert_eq!(retirement.market_child_count_after, 0);
    assert_eq!(retirement.rent_refund, [70; 32]);
    assert_eq!(
        retire(
            MARKET_KEY,
            &mut market,
            state,
            id(MANIFEST_ID_BYTE),
            manifest,
            config_id,
            config,
            1,
            CONTROLLER,
            keys,
            observations,
        ),
        Err(Error::MarketContract {
            error: dclutch_market_contract::Error::InvalidMarketRoot {
                error: dclutch_core_contract::Error::ChildCountMismatch
            }
        })
    );
    Ok(())
}

#[test]
fn instruction_codecs_refuse_trailing_reserved_and_out_of_range_data() -> Result<()> {
    let activate = InstructionV1::Activate {
        outcome_count: 16,
        generation: GENERATION,
        expected_prior_child_count: 3,
    };
    let mut bytes = [0u8; ACTIVATE_INSTRUCTION_BYTES];
    activate.encode(&mut bytes)?;
    assert_eq!(InstructionV1::decode(&bytes), Ok(activate));
    let mut reserved = bytes;
    reserved[12] = 1;
    assert_eq!(
        InstructionV1::decode(&reserved),
        Err(Error::NonCanonicalReservedBytes)
    );
    let mut trailing = [0u8; ACTIVATE_INSTRUCTION_BYTES + 1];
    trailing[..ACTIVATE_INSTRUCTION_BYTES].copy_from_slice(&bytes);
    assert_eq!(InstructionV1::decode(&trailing), Err(Error::InvalidLength));

    let outcome = InstructionV1::Outcome {
        action: ActionV1::RedeemBearer,
        outcome_count: 2,
        generation: GENERATION,
        quantity: 7,
        outcome: 1,
    };
    let mut outcome_bytes = [0u8; OUTCOME_INSTRUCTION_BYTES];
    outcome.encode(&mut outcome_bytes)?;
    assert_eq!(InstructionV1::decode(&outcome_bytes), Ok(outcome));
    outcome_bytes[32] = 2;
    assert_eq!(
        InstructionV1::decode(&outcome_bytes),
        Err(Error::InvalidOutcome)
    );
    Ok(())
}

#[test]
fn frame_enforces_exact_privileges_and_aliases() -> Result<()> {
    let mut accounts = [AccountMetaV1 {
        key: [0; 32],
        is_signer: false,
        is_writable: false,
        is_executable: false,
    }; 12];
    for (index, account) in accounts.iter_mut().enumerate() {
        let byte = match u8::try_from(index) {
            Ok(value) => value.saturating_add(1),
            Err(_) => 255,
        };
        account.key = [byte; 32];
    }
    for index in [0usize, 1, 4, 6, 10, 11] {
        if let Some(account) = accounts.get_mut(index) {
            account.is_writable = true;
        }
    }
    accounts[6].is_signer = true;
    accounts[7].is_executable = true;
    accounts[8].is_executable = true;
    assert_eq!(
        validate_account_frame::<2>(ActionV1::Activate, &accounts),
        Ok(())
    );
    let exact = accounts;
    accounts[2].is_writable = true;
    assert_eq!(
        validate_account_frame::<2>(ActionV1::Activate, &accounts),
        Err(Error::InvalidAccountFrame)
    );
    accounts = exact;
    accounts[11].key = accounts[10].key;
    assert_eq!(
        validate_account_frame::<2>(ActionV1::Activate, &accounts),
        Err(Error::AccountAlias)
    );
    Ok(())
}

#[test]
fn retirement_refund_is_writable_for_token_close_and_root_refund() -> Result<()> {
    let mut accounts = [AccountMetaV1 {
        key: [0; 32],
        is_signer: false,
        is_writable: false,
        is_executable: false,
    }; 10];
    for (index, account) in accounts.iter_mut().enumerate() {
        let byte = u8::try_from(index).map_err(|_| Error::ArithmeticOverflow)?;
        account.key = [byte.saturating_add(1); 32];
    }
    for index in [0usize, 1, 4, 8, 9] {
        accounts
            .get_mut(index)
            .ok_or(Error::InvalidAccountFrame)?
            .is_writable = true;
    }
    accounts[5].is_executable = true;
    accounts[6].is_executable = true;
    assert_eq!(validate_account_frame::<2>(ActionV1::Retire, &accounts), Ok(()));
    accounts[4].is_writable = false;
    assert_eq!(
        validate_account_frame::<2>(ActionV1::Retire, &accounts),
        Err(Error::InvalidAccountFrame)
    );
    Ok(())
}
