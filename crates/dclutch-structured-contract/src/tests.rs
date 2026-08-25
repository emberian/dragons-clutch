use dclutch_bearer_contract::state::{
    BEARER_MINT_BYTES, BEARER_SEMANTIC_RELEASE_ID, BEARER_TOKEN_ACCOUNT_BYTES, MintObservationV1,
    TokenAccountObservationV1, TokenAccountStateV1,
};
use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::CapacityProfileId,
    portfolio::{PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, PortfolioTemplateV1},
    product::{InstanceV1, InstanceV1Input},
    terminal::ResolutionKind,
};
use dclutch_realm_contract::PositionV1;
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;
use sha2::{Digest, Sha256};

use crate::descriptor::*;
use crate::instruction::*;
use crate::transition::*;
use crate::{Error, Result};

const GENERATION: u64 = 7;
const MARKET_KEY: [u8; 32] = [10; 32];
const OWNER: [u8; 32] = [20; 32];
const RECIPIENT: [u8; 32] = [21; 32];
const DESCRIPTOR_KEY: [u8; 32] = [30; 32];
const INSTANCE_ID: [u8; 32] = [31; 32];
const TEMPLATE_ID: [u8; 32] = [32; 32];
const CONFIG_ID: [u8; 32] = [33; 32];
const RECEIPT_MINT: [u8; 32] = [34; 32];
const RECEIPT_AUTHORITY: [u8; 32] = [35; 32];
const CUSTODY_POSITION: [u8; 32] = [36; 32];
const CUSTODY_OWNER: [u8; 32] = [37; 32];
const RENT_CREDIT: [u8; 32] = [38; 32];
const OWNER_TOKEN_ACCOUNT: [u8; 32] = [39; 32];
const RECIPIENT_TOKEN_ACCOUNT: [u8; 32] = [40; 32];

fn core_id(fill: u8) -> CoreContentId {
    CoreContentId::new([fill; 32]).expect("nonzero core identity")
}

fn product_id(fill: u8) -> ProductContentId {
    ProductContentId::new([fill; 32]).expect("nonzero Product identity")
}

fn product<const N: usize>(coefficients: [u64; N], denominator: u64) -> ProductBindingV1<N> {
    let basis = product_id(3);
    let domain = product_id(4);
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id(1),
        occurrence_id: product_id(2),
        claim_basis_id: basis,
        result_domain_id: domain,
        capacity_profile_id: CapacityProfileId::new(product_id(5)),
        partition_cell_count: u32::try_from(N).expect("test width fits u32"),
    })
    .expect("valid Product instance");
    let template =
        PortfolioTemplateV1::new(basis, domain, coefficients, denominator).expect("valid template");
    ProductBindingV1::new(
        ProductContentId::new(INSTANCE_ID).expect("instance ID"),
        instance,
        ProductContentId::new(TEMPLATE_ID).expect("template ID"),
        template,
    )
    .expect("valid Product binding")
}

fn open_market<const N: usize>(supply: [u64; N], hoard: u64) -> CategoricalMarketV1<N> {
    let mut root = MarketRoot::founding(
        MarketIdentity::new(
            core_id(1),
            CoreContentId::new(INSTANCE_ID).expect("instance ID"),
            core_id(3),
            core_id(4),
            core_id(5),
            GENERATION,
        ),
        [90; 32],
    )
    .expect("founding root");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("open root");
    CategoricalMarketV1::new(root, hoard, supply, CategoricalSettlementSummaryV1::empty())
        .expect("valid market")
}

fn config() -> StructuredConfigV1 {
    StructuredConfigV1::new(
        TOKEN_2022_PROGRAM_ID,
        BEARER_SEMANTIC_RELEASE_ID,
        RENT_CREDIT,
    )
    .expect("structured config")
}

fn descriptor<const N: usize>() -> StructuredDescriptorV1 {
    StructuredDescriptorV1::new::<N>(StructuredDescriptorInputV1 {
        market: MARKET_KEY,
        generation: GENERATION,
        manifest_entry_index: 4,
        portfolio_template_id: TEMPLATE_ID,
        capability_config_id: CONFIG_ID,
        capability_release_id: STRUCTURED_SEMANTIC_RELEASE_ID_V1,
        receipt_adapter_release_id: BEARER_SEMANTIC_RELEASE_ID,
        receipt_mint: RECEIPT_MINT,
        receipt_authority: RECEIPT_AUTHORITY,
        custody_position: CUSTODY_POSITION,
        custody_owner: CUSTODY_OWNER,
        rent_credit: RENT_CREDIT,
    })
    .expect("structured descriptor")
}

fn context<const N: usize>(
    market: &CategoricalMarketV1<N>,
    product: ProductBindingV1<N>,
) -> StructuredContextV1<N> {
    StructuredContextV1::new(
        DESCRIPTOR_KEY,
        descriptor::<N>(),
        MARKET_KEY,
        market,
        product,
        CONFIG_ID,
        config(),
    )
    .expect("joined context")
}

fn mint(supply: u64) -> MintObservationV1 {
    MintObservationV1 {
        key: RECEIPT_MINT,
        program_owner: TOKEN_2022_PROGRAM_ID,
        data_len: BEARER_MINT_BYTES,
        supply,
        decimals: 0,
        initialized: true,
        mint_authority: Some(RECEIPT_AUTHORITY),
        freeze_authority: None,
        close_authority: Some(RECEIPT_AUTHORITY),
        permissioned_burn_authority: Some(RECEIPT_AUTHORITY),
        extension_count: 2,
    }
}

fn token_account(key: [u8; 32], authority: [u8; 32], amount: u64) -> TokenAccountObservationV1 {
    TokenAccountObservationV1 {
        key,
        program_owner: TOKEN_2022_PROGRAM_ID,
        data_len: BEARER_TOKEN_ACCOUNT_BYTES,
        mint: RECEIPT_MINT,
        authority,
        amount,
        state: TokenAccountStateV1::Initialized,
        has_native_reserve: false,
        extension_count: 0,
    }
}

fn activate_three(
    market: &mut CategoricalMarketV1<3>,
    product: ProductBindingV1<3>,
) -> (StructuredContextV1<3>, PositionV1<3>) {
    let joined = context(market, product);
    let plan = activate(joined, MARKET_KEY, market, 0).expect("activate");
    assert_eq!(plan.market_child_count_after(), 1);
    assert_eq!(plan.receipt_mint().mint(), RECEIPT_MINT);
    assert_eq!(plan.receipt_mint().controller(), RECEIPT_AUTHORITY);
    assert_eq!(plan.receipt_mint().data_len(), BEARER_MINT_BYTES);
    assert_eq!(plan.receipt_mint().decimals(), 0);
    assert!(plan.receipt_mint().permissioned_burn_required());
    assert!(plan.receipt_mint().close_authority_required());
    (joined, plan.custody_position())
}

fn resolve_three(market: &mut CategoricalMarketV1<3>, winner: usize) {
    let settlement = CategoricalSettlementSummaryV1::resolved::<3>(
        product_id(99),
        ResolutionKind::Occurrence,
        winner,
        1,
    )
    .expect("settlement");
    market
        .resolve_with_summary(GENERATION, settlement)
        .expect("resolve market");
}

#[test]
fn release_id_preimages_and_product_namespace_are_exact() {
    for (preimage, expected) in [
        (
            STRUCTURED_CAPABILITY_KIND_PREIMAGE_V1,
            STRUCTURED_CAPABILITY_KIND_ID_V1,
        ),
        (
            STRUCTURED_SEMANTIC_RELEASE_PREIMAGE_V1,
            STRUCTURED_SEMANTIC_RELEASE_ID_V1,
        ),
        (STRUCTURED_CAPACITY_PREIMAGE_V1, STRUCTURED_CAPACITY_ID_V1),
        (
            STRUCTURED_CHILD_SCHEMA_PREIMAGE_V1,
            STRUCTURED_CHILD_SCHEMA_ID_V1,
        ),
        (
            STRUCTURED_CHILD_DERIVATION_PREIMAGE_V1,
            STRUCTURED_CHILD_DERIVATION_ID_V1,
        ),
    ] {
        let actual: [u8; 32] = Sha256::digest(preimage).into();
        assert_eq!(actual, expected);
    }
    let binding = product([1, 2, 3], 4);
    assert_eq!(
        binding.portfolio_template_content_domain(),
        PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1
    );
}

#[test]
fn config_and_descriptor_exact_codecs_refuse_hostile_bytes() {
    let config = config();
    let config_bytes = config.to_bytes();
    assert_eq!(StructuredConfigV1::decode(&config_bytes), Ok(config));
    for length in 0..STRUCTURED_CONFIG_BYTES {
        assert_eq!(
            StructuredConfigV1::decode(config_bytes.get(..length).expect("bounded config prefix")),
            Err(Error::InvalidLength)
        );
    }
    let mut config_long = [0; STRUCTURED_CONFIG_BYTES + 1];
    config_long
        .get_mut(..STRUCTURED_CONFIG_BYTES)
        .expect("config target")
        .copy_from_slice(&config_bytes);
    assert_eq!(
        StructuredConfigV1::decode(&config_long),
        Err(Error::InvalidLength)
    );
    for offset in [0, 8, 10, 11, 12, 16, 48, 80] {
        let mut hostile = config_bytes;
        *hostile.get_mut(offset).expect("config offset") ^= 1;
        assert_ne!(StructuredConfigV1::decode(&hostile), Ok(config));
    }

    let descriptor = descriptor::<3>();
    let bytes = descriptor.to_bytes();
    assert_eq!(bytes.len(), STRUCTURED_DESCRIPTOR_BYTES);
    assert_eq!(StructuredDescriptorV1::decode(&bytes), Ok(descriptor));
    for length in 0..STRUCTURED_DESCRIPTOR_BYTES {
        assert_eq!(
            StructuredDescriptorV1::decode(bytes.get(..length).expect("descriptor prefix")),
            Err(Error::InvalidLength)
        );
    }
    let mut long = [0; STRUCTURED_DESCRIPTOR_BYTES + 1];
    long.get_mut(..STRUCTURED_DESCRIPTOR_BYTES)
        .expect("descriptor target")
        .copy_from_slice(&bytes);
    assert_eq!(
        StructuredDescriptorV1::decode(&long),
        Err(Error::InvalidLength)
    );
    for offset in [0, 8, 10, 11, 14, 15, 56] {
        let mut hostile = bytes;
        *hostile.get_mut(offset).expect("header offset") ^= 1;
        assert_ne!(StructuredDescriptorV1::decode(&hostile), Ok(descriptor));
    }
    for offset in [16, 64, 96, 128, 160, 192, 224, 256, 288, 320] {
        let mut hostile = bytes;
        hostile
            .get_mut(offset..offset + 32)
            .expect("identity field")
            .fill(0);
        assert_eq!(
            StructuredDescriptorV1::decode(&hostile),
            Err(Error::ZeroIdentifier)
        );
    }
}

#[test]
fn instruction_codec_is_exact_and_quantity_refusing() {
    let values = [
        StructuredInstructionV1::activate(3, GENERATION, 0).expect("activate ix"),
        StructuredInstructionV1::wrap(3, GENERATION, 2).expect("wrap ix"),
        StructuredInstructionV1::unwrap(3, GENERATION, 2).expect("unwrap ix"),
        StructuredInstructionV1::redeem_terminal(3, GENERATION, 2).expect("redeem ix"),
        StructuredInstructionV1::retire(3, GENERATION, 1).expect("retire ix"),
    ];
    for value in values {
        let mut bytes = [0; STRUCTURED_INSTRUCTION_BYTES];
        value.encode(&mut bytes).expect("encode instruction");
        assert_eq!(StructuredInstructionV1::decode(&bytes), Ok(value));
        for length in 0..STRUCTURED_INSTRUCTION_BYTES {
            assert_eq!(
                StructuredInstructionV1::decode(bytes.get(..length).expect("ix prefix")),
                Err(Error::InvalidLength)
            );
        }
    }
    assert_eq!(
        StructuredInstructionV1::wrap(3, GENERATION, 0),
        Err(Error::ZeroInstructionQuantity)
    );
    assert_eq!(
        StructuredInstructionV1::retire(3, GENERATION, 0),
        Err(Error::InvalidChildCount)
    );
    let mut canonical = [0; STRUCTURED_INSTRUCTION_BYTES];
    values
        .get(1)
        .copied()
        .expect("wrap")
        .encode(&mut canonical)
        .expect("encode");
    for offset in [0, 8, 10, 11, 12] {
        let mut hostile = canonical;
        *hostile.get_mut(offset).expect("ix offset") ^= 0xff;
        assert!(StructuredInstructionV1::decode(&hostile).is_err());
    }
}

#[test]
fn canonical_denominator_is_the_minimum_integral_lot() {
    let binding = product([2, 3, 0], 6);
    assert_eq!(binding.recipe().minimum_realization_lot(), 6);
    assert_eq!(binding.recipe().coefficients(), &[2, 3, 0]);

    let normalized = product([2, 4, 0], 6);
    assert_eq!(normalized.recipe().minimum_realization_lot(), 3);
    assert_eq!(normalized.recipe().coefficients(), &[1, 2, 0]);
}

#[test]
fn same_width_template_basis_domain_and_config_substitution_refuse() {
    let market = open_market([10, 20, 30], 30);
    let canonical = product([1, 2, 3], 4);
    let foreign_template = PortfolioTemplateV1::new(
        canonical.instance().claim_basis_id(),
        canonical.instance().result_domain_id(),
        [1, 3, 2],
        4,
    )
    .expect("foreign template");
    let foreign = ProductBindingV1::new(
        canonical.instance_id(),
        canonical.instance(),
        product_id(88),
        foreign_template,
    )
    .expect("same-width foreign binding");
    assert_eq!(
        StructuredContextV1::new(
            DESCRIPTOR_KEY,
            descriptor::<3>(),
            MARKET_KEY,
            &market,
            foreign,
            CONFIG_ID,
            config(),
        ),
        Err(Error::PortfolioTemplateMismatch)
    );

    let wrong_domain_template = PortfolioTemplateV1::new(
        canonical.instance().claim_basis_id(),
        product_id(77),
        [1, 2, 3],
        4,
    )
    .expect("foreign domain template");
    assert_eq!(
        ProductBindingV1::new(
            canonical.instance_id(),
            canonical.instance(),
            canonical.portfolio_template_id(),
            wrong_domain_template,
        ),
        Err(Error::ResultDomainMismatch)
    );

    let wrong_config =
        StructuredConfigV1::new(TOKEN_2022_PROGRAM_ID, BEARER_SEMANTIC_RELEASE_ID, [91; 32])
            .expect("foreign config");
    assert_eq!(
        StructuredContextV1::new(
            DESCRIPTOR_KEY,
            descriptor::<3>(),
            MARKET_KEY,
            &market,
            canonical,
            CONFIG_ID,
            wrong_config,
        ),
        Err(Error::RentCreditMismatch)
    );
}

#[test]
fn wrap_and_unwrap_preserve_exact_native_and_receipt_conservation() {
    let product_binding = product([1, 2, 3], 4);
    let mut market = open_market([10, 20, 30], 30);
    let (joined, mut custody) = activate_three(&mut market, product_binding);
    let mut owner_position =
        PositionV1::new(MARKET_KEY, OWNER, GENERATION, [4, 8, 12]).expect("owner Position");
    let before_visible = [4, 8, 12];
    let plan = wrap(
        joined,
        MARKET_KEY,
        &market,
        OWNER,
        &mut owner_position,
        CUSTODY_POSITION,
        &mut custody,
        mint(0),
        token_account(OWNER_TOKEN_ACCOUNT, OWNER, 0),
        2,
    )
    .expect("wrap exact basket");
    assert_eq!(plan.backing(), &[2, 4, 6]);
    assert_eq!(plan.minimum_realization_lot(), 4);
    assert_eq!(owner_position.balances(), &[2, 4, 6]);
    assert_eq!(custody.balances(), &[2, 4, 6]);
    assert_eq!(plan.receipt().mint_supply_before(), 0);
    assert_eq!(plan.receipt().mint_supply_after(), 2);
    assert_eq!(plan.receipt().account_balance_after(), 2);
    for (index, initial) in before_visible.iter().copied().enumerate() {
        let visible = owner_position
            .balances()
            .get(index)
            .copied()
            .expect("owner")
            + custody.balances().get(index).copied().expect("custody");
        assert_eq!(visible, initial);
    }
    audit_backing(
        joined,
        MARKET_KEY,
        &market,
        CUSTODY_POSITION,
        &custody,
        mint(2),
    )
    .expect("backing audit");

    let plan = unwrap(
        joined,
        MARKET_KEY,
        &market,
        OWNER,
        &mut owner_position,
        CUSTODY_POSITION,
        &mut custody,
        mint(2),
        token_account(OWNER_TOKEN_ACCOUNT, OWNER, 2),
        2,
    )
    .expect("unwrap exact basket");
    assert_eq!(plan.receipt().operation(), ReceiptOperationV1::Burn);
    assert_eq!(plan.receipt().mint_supply_after(), 0);
    assert_eq!(owner_position.balances(), &before_visible);
    assert!(custody.is_empty());
}

#[test]
fn ordinary_token_transfer_preserves_backing_and_recipient_can_unwrap_and_redeem() {
    let product = product([1, 2, 3], 4);
    let mut market = open_market([20, 30, 40], 40);
    let (context, mut custody) = activate_three(&mut market, product);
    let mut owner_position =
        PositionV1::new(MARKET_KEY, OWNER, GENERATION, [10, 20, 30]).expect("owner Position");
    wrap(
        context,
        MARKET_KEY,
        &market,
        OWNER,
        &mut owner_position,
        CUSTODY_POSITION,
        &mut custody,
        mint(0),
        token_account(OWNER_TOKEN_ACCOUNT, OWNER, 0),
        3,
    )
    .expect("wrap three");
    assert_eq!(custody.balances(), &[3, 6, 9]);

    // An ordinary external Token-2022 transfer moves two receipt atoms from
    // OWNER to RECIPIENT. Structured state is untouched and Mint supply stays 3.
    let transferred_source = token_account(OWNER_TOKEN_ACCOUNT, OWNER, 1);
    let transferred_recipient = token_account(RECIPIENT_TOKEN_ACCOUNT, RECIPIENT, 2);
    assert_eq!(transferred_source.amount + transferred_recipient.amount, 3);
    audit_backing(
        context,
        MARKET_KEY,
        &market,
        CUSTODY_POSITION,
        &custody,
        mint(3),
    )
    .expect("transfer leaves sole supply and backing unchanged");

    let mut recipient_position =
        PositionV1::empty(MARKET_KEY, RECIPIENT, GENERATION).expect("recipient Position");
    let unwrap_plan = unwrap(
        context,
        MARKET_KEY,
        &market,
        RECIPIENT,
        &mut recipient_position,
        CUSTODY_POSITION,
        &mut custody,
        mint(3),
        transferred_recipient,
        1,
    )
    .expect("recipient unwraps transferred receipt");
    assert_eq!(unwrap_plan.receipt().mint_supply_after(), 2);
    assert_eq!(unwrap_plan.receipt().account_balance_after(), 1);
    assert_eq!(recipient_position.balances(), &[1, 2, 3]);
    assert_eq!(custody.balances(), &[2, 4, 6]);

    resolve_three(&mut market, 1);
    let supply_before = *market.supply();
    let hoard_before = market.hoard_atoms();
    let redeem_plan = redeem_terminal(
        context,
        MARKET_KEY,
        &mut market,
        RECIPIENT,
        CUSTODY_POSITION,
        &mut custody,
        mint(2),
        token_account(RECIPIENT_TOKEN_ACCOUNT, RECIPIENT, 1),
        1,
    )
    .expect("recipient redeems remaining transferred receipt");
    assert_eq!(redeem_plan.backing(), &[1, 2, 3]);
    assert_eq!(redeem_plan.collateral_payout_atoms(), 2);
    assert_eq!(redeem_plan.receipt().mint_supply_after(), 1);
    assert_eq!(redeem_plan.receipt().account_balance_after(), 0);
    assert_eq!(custody.balances(), &[1, 2, 3]);
    assert_eq!(
        market.supply(),
        &[
            supply_before[0] - 1,
            supply_before[1] - 2,
            supply_before[2] - 3
        ]
    );
    assert_eq!(market.hoard_atoms(), hoard_before - 2);
}

#[test]
fn terminal_redeem_consumes_winner_and_every_losing_coefficient_then_retires() {
    let product = product([1, 2, 3], 4);
    let mut market = open_market([10, 20, 30], 30);
    let (context, mut custody) = activate_three(&mut market, product);
    let mut owner_position =
        PositionV1::new(MARKET_KEY, OWNER, GENERATION, [1, 2, 3]).expect("owner Position");
    wrap(
        context,
        MARKET_KEY,
        &market,
        OWNER,
        &mut owner_position,
        CUSTODY_POSITION,
        &mut custody,
        mint(0),
        token_account(OWNER_TOKEN_ACCOUNT, OWNER, 0),
        1,
    )
    .expect("wrap");
    resolve_three(&mut market, 1);
    let supplies = *market.supply();
    let plan = redeem_terminal(
        context,
        MARKET_KEY,
        &mut market,
        OWNER,
        CUSTODY_POSITION,
        &mut custody,
        mint(1),
        token_account(OWNER_TOKEN_ACCOUNT, OWNER, 1),
        1,
    )
    .expect("terminal redeem");
    assert_eq!(plan.collateral_payout_atoms(), 2);
    assert!(custody.is_empty());
    assert_eq!(
        market.supply(),
        &[supplies[0] - 1, supplies[1] - 2, supplies[2] - 3]
    );
    market
        .transition_phase(GENERATION, Phase::Retiring)
        .expect("retiring");
    let retirement = retire(
        context,
        MARKET_KEY,
        &mut market,
        CUSTODY_POSITION,
        &custody,
        mint(0),
        1,
    )
    .expect("retire structured child");
    assert_eq!(retirement.receipt_mint(), RECEIPT_MINT);
    assert_eq!(retirement.rent_credit(), RENT_CREDIT);
    assert_eq!(retirement.market_child_count_after(), 0);
}

#[test]
fn mint_profile_substitution_backing_mismatch_overflow_and_refusal_are_atomic() {
    let product_binding = product([1, 2, 3], 4);
    let mut market = open_market([10, 20, 30], 30);
    let (joined, mut custody) = activate_three(&mut market, product_binding);
    let mut owner_position =
        PositionV1::new(MARKET_KEY, OWNER, GENERATION, [1, 2, 3]).expect("owner Position");
    let owner_before = owner_position;
    let custody_before = custody;
    let mut foreign_mint = mint(0);
    foreign_mint.key = [88; 32];
    assert!(
        wrap(
            joined,
            MARKET_KEY,
            &market,
            OWNER,
            &mut owner_position,
            CUSTODY_POSITION,
            &mut custody,
            foreign_mint,
            token_account(OWNER_TOKEN_ACCOUNT, OWNER, 0),
            1,
        )
        .is_err()
    );
    assert_eq!(owner_position, owner_before);
    assert_eq!(custody, custody_before);

    assert_eq!(
        audit_backing(
            joined,
            MARKET_KEY,
            &market,
            CUSTODY_POSITION,
            &custody,
            mint(1),
        ),
        Err(Error::BackingMismatch)
    );
    assert_eq!(owner_position, owner_before);
    assert_eq!(custody, custody_before);

    assert!(
        wrap(
            joined,
            MARKET_KEY,
            &market,
            OWNER,
            &mut owner_position,
            CUSTODY_POSITION,
            &mut custody,
            mint(0),
            token_account(OWNER_TOKEN_ACCOUNT, OWNER, 0),
            2,
        )
        .is_err()
    );
    assert_eq!(owner_position, owner_before);
    assert_eq!(custody, custody_before);

    let huge_product = product([u64::MAX, 1], 1);
    let mut huge_market = open_market([u64::MAX, u64::MAX], u64::MAX);
    let huge_context = context(&huge_market, huge_product);
    let huge_plan = activate(huge_context, MARKET_KEY, &mut huge_market, 0).expect("activate huge");
    let mut huge_custody = huge_plan.custody_position();
    let mut huge_owner =
        PositionV1::new(MARKET_KEY, OWNER, GENERATION, [u64::MAX, u64::MAX]).expect("huge owner");
    let huge_owner_before = huge_owner;
    let huge_custody_before = huge_custody;
    assert_eq!(
        wrap(
            huge_context,
            MARKET_KEY,
            &huge_market,
            OWNER,
            &mut huge_owner,
            CUSTODY_POSITION,
            &mut huge_custody,
            mint(0),
            token_account(OWNER_TOKEN_ACCOUNT, OWNER, 0),
            2,
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(huge_owner, huge_owner_before);
    assert_eq!(huge_custody, huge_custody_before);
}

#[test]
fn retirement_refuses_any_mint_supply_or_native_custody() {
    let product = product([1, 2, 3], 4);
    let mut market = open_market([10, 20, 30], 30);
    let (context, mut custody) = activate_three(&mut market, product);
    let mut owner_position =
        PositionV1::new(MARKET_KEY, OWNER, GENERATION, [1, 2, 3]).expect("owner Position");
    wrap(
        context,
        MARKET_KEY,
        &market,
        OWNER,
        &mut owner_position,
        CUSTODY_POSITION,
        &mut custody,
        mint(0),
        token_account(OWNER_TOKEN_ACCOUNT, OWNER, 0),
        1,
    )
    .expect("wrap");
    resolve_three(&mut market, 1);
    market
        .transition_phase(GENERATION, Phase::Retiring)
        .expect("retiring");
    let market_before = market;
    assert_eq!(
        retire(
            context,
            MARKET_KEY,
            &mut market,
            CUSTODY_POSITION,
            &custody,
            mint(1),
            1,
        ),
        Err(Error::OutstandingStructuredBacking)
    );
    assert_eq!(market, market_before);
}

#[test]
fn fixed_layout_types_remain_copyable_and_allocation_free_by_construction() -> Result<()> {
    let descriptor = descriptor::<16>();
    let copy = descriptor;
    assert_eq!(copy, descriptor);
    let mut output = [7; STRUCTURED_DESCRIPTOR_BYTES];
    descriptor.encode(&mut output)?;
    assert_eq!(StructuredDescriptorV1::decode(&output), Ok(descriptor));
    Ok(())
}
