//! Pure host construction and verification for Direct Token-2022 setup.
//!
//! This module owns no RPC, filesystem, key, journal, or retry behavior. It
//! hostile-decodes the complete semantic prestates, derives all 23 accounts,
//! emits the sole admitted Trading instruction, and verifies return data plus
//! every full token and immutable poststate.

use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_claims::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_trading::{
    execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3,
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectExecutionConfigV1,
        DirectRootPhaseV1, DirectRootStateV1,
    },
    token_setup_v1::{
        DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1, DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1,
        DIRECT_TOKEN_SETUP_RECEIPT_BYTES_V1, DIRECT_TOKEN_SETUP_REQUEST_BYTES_V1,
        DirectTokenAccountRoleV1, DirectTokenAccountSeedsV1, DirectTokenRentNormalizationV1,
        DirectTokenSetupReceiptV1, DirectTokenSetupRequestV1, direct_token_rent_normalization_v1,
        direct_token_setup_frame_digest_v1,
    },
};
use dclutch_market::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, StateBumpsV1,
};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_custody::token_svm::{
    ACCOUNT_BYTES, COption, PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, TokenAccount,
};
use solana_program::{hash::hash, rent::Rent};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{Error, Result};

/// Exact ordered coordinates of the onchain 23-account route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectTradeTokenSetupCoordinatesV1 {
    pub(crate) market: Pubkey,
    pub(crate) core_program: Pubkey,
    pub(crate) registry_program: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) trading_program: Pubkey,
    pub(crate) trading_programdata: Pubkey,
    pub(crate) claims_program: Pubkey,
    pub(crate) claims_programdata: Pubkey,
    pub(crate) direct_root: Pubkey,
    pub(crate) realm_raw: Pubkey,
    pub(crate) realm_staging: Pubkey,
    pub(crate) config_raw: Pubkey,
    pub(crate) config_staging: Pubkey,
    pub(crate) claims_aggregate: Pubkey,
    pub(crate) seller_position: Pubkey,
    pub(crate) collateral_mint: Pubkey,
    pub(crate) seller_token: Pubkey,
    pub(crate) fee_token: Pubkey,
    pub(crate) payer: Pubkey,
    pub(crate) rent_refund: Pubkey,
    pub(crate) rent_sysvar: Pubkey,
    pub(crate) system_program: Pubkey,
    pub(crate) token_program: Pubkey,
}

impl DirectTradeTokenSetupCoordinatesV1 {
    fn ordered(self) -> [Pubkey; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1] {
        [
            self.market,
            self.core_program,
            self.registry_program,
            self.activation_cache,
            self.trading_program,
            self.trading_programdata,
            self.claims_program,
            self.claims_programdata,
            self.direct_root,
            self.realm_raw,
            self.realm_staging,
            self.config_raw,
            self.config_staging,
            self.claims_aggregate,
            self.seller_position,
            self.collateral_mint,
            self.seller_token,
            self.fee_token,
            self.payer,
            self.rent_refund,
            self.rent_sysvar,
            self.system_program,
            self.token_program,
        ]
    }
}

/// Complete semantic prestates required to construct the route.
#[derive(Clone, Debug)]
pub(crate) struct DirectTradeTokenSetupBuildInputV1<'a> {
    pub(crate) market_bytes: &'a [u8],
    pub(crate) root_bytes: &'a [u8],
    pub(crate) realm_bytes: &'a [u8],
    pub(crate) config_bytes: &'a [u8],
    pub(crate) claims_aggregate_bytes: &'a [u8],
    pub(crate) seller_position_bytes: &'a [u8],
    pub(crate) collateral_mint_bytes: &'a [u8],
    pub(crate) generation: u64,
    pub(crate) seller_owner: Pubkey,
    pub(crate) coordinates: DirectTradeTokenSetupCoordinatesV1,
    pub(crate) rent: Rent,
    pub(crate) observed_seller_lamports: u64,
    pub(crate) observed_fee_lamports: u64,
    pub(crate) observed_payer_lamports: u64,
    pub(crate) observed_refund_lamports: u64,
}

/// One exact full account observation supplied after finalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectTradeTokenSetupObservedAccountV1<'a> {
    pub(crate) address: Pubkey,
    pub(crate) owner: Pubkey,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data: &'a [u8],
}

/// Complete poststate observation for pure verification.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectTradeTokenSetupPoststateV1<'a> {
    pub(crate) return_program: Pubkey,
    pub(crate) return_data: &'a [u8],
    pub(crate) seller_token: DirectTradeTokenSetupObservedAccountV1<'a>,
    pub(crate) fee_token: DirectTradeTokenSetupObservedAccountV1<'a>,
    pub(crate) market_bytes: &'a [u8],
    pub(crate) root_bytes: &'a [u8],
    pub(crate) realm_bytes: &'a [u8],
    pub(crate) config_bytes: &'a [u8],
    pub(crate) claims_aggregate_bytes: &'a [u8],
    pub(crate) seller_position_bytes: &'a [u8],
    pub(crate) collateral_mint_bytes: &'a [u8],
}

/// Complete pure setup plan and exact expected poststate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectTradeTokenSetupPlanV1 {
    pub(crate) instruction: Instruction,
    pub(crate) request: DirectTokenSetupRequestV1,
    pub(crate) request_bytes: [u8; DIRECT_TOKEN_SETUP_REQUEST_BYTES_V1],
    pub(crate) request_digest: [u8; 32],
    pub(crate) frame_digest: [u8; 32],
    pub(crate) coordinates: DirectTradeTokenSetupCoordinatesV1,
    pub(crate) seller_owner: Pubkey,
    pub(crate) fee_recipient: Pubkey,
    pub(crate) exact_account_rent: u64,
    pub(crate) seller_normalization: DirectTokenRentNormalizationV1,
    pub(crate) fee_normalization: DirectTokenRentNormalizationV1,
    /// This instruction's own effect on the two shared wallets, excluding the
    /// runtime fee. A projection for the record, never a chain expectation.
    pub(crate) projected_payer_lamports: u64,
    pub(crate) projected_refund_lamports: u64,
    pub(crate) expected_seller_bytes: [u8; ACCOUNT_BYTES],
    pub(crate) expected_fee_bytes: [u8; ACCOUNT_BYTES],
    pub(crate) expected_receipt: DirectTokenSetupReceiptV1,
    pub(crate) expected_receipt_bytes: [u8; DIRECT_TOKEN_SETUP_RECEIPT_BYTES_V1],
    pub(crate) expected_immutable_bytes: [Vec<u8>; 7],
    immutable_digests: [[u8; 32]; 7],
}

/// Fully verified return and token poststate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedDirectTradeTokenSetupV1 {
    pub(crate) receipt: DirectTokenSetupReceiptV1,
    pub(crate) seller: TokenAccount,
    pub(crate) fee: TokenAccount,
}

/// Build and internally verify the one exact Trading token-setup instruction.
pub(crate) fn build_direct_trade_token_setup_v1(
    input: DirectTradeTokenSetupBuildInputV1<'_>,
) -> Result<DirectTradeTokenSetupPlanV1> {
    let coordinates = input.coordinates;
    require_coordinates(coordinates)?;
    let market = CoreState::decode(input.market_bytes)
        .map_err(|error| refusal(format!("Direct token setup Market: {error:?}")))?;
    let canonical_market = market
        .encode()
        .map_err(|error| refusal(format!("encode Direct token setup Market: {error:?}")))?;
    let expected_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market.identity).as_slices(),
        &coordinates.core_program,
    )
    .0;
    if canonical_market.as_slice() != input.market_bytes
        || market.phase != CorePhase::Open
        || market.identity.market_id.to_bytes() != coordinates.market.to_bytes()
        || coordinates.market != expected_market
        || market.identity.generation != input.generation
        || market.identity.registry_program.to_bytes() != coordinates.registry_program.to_bytes()
        || market.rent_beneficiary.to_bytes() != coordinates.rent_refund.to_bytes()
    {
        return Err(refusal("Direct token setup had a foreign Open Market"));
    }
    let release_set = market.identity.selected_release_set.to_bytes();
    let expected_activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_slice()],
        &coordinates.registry_program,
    )
    .0;
    if coordinates.activation_cache != expected_activation {
        return Err(refusal("Direct token setup activation cache was foreign"));
    }

    let expected_root_width = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or_else(|| refusal("Direct token setup root width overflowed"))?;
    if input.root_bytes.len() != expected_root_width {
        return Err(refusal("Direct token setup root had another width"));
    }
    let header = CapabilityRootHeaderV1::decode(
        input
            .root_bytes
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or_else(|| refusal("Direct token setup root header missing"))?,
    )
    .map_err(|error| refusal(format!("Direct token setup root header: {error:?}")))?;
    let root_state = DirectRootStateV1::decode(
        input
            .root_bytes
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or_else(|| refusal("Direct token setup root tail missing"))?,
    )
    .map_err(|error| refusal(format!("Direct token setup root state: {error:?}")))?;
    let expected_root =
        Pubkey::find_program_address(&header.seeds().as_slices(), &coordinates.trading_program).0;
    let selection = header.selection();
    let config_id = selection.config().to_bytes();
    if coordinates.direct_root != expected_root
        || header.market() != coordinates.market.to_bytes()
        || header.generation() != input.generation
        || header.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || selection.kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
        || root_state.phase() != DirectRootPhaseV1::Open
    {
        return Err(refusal("Direct token setup root selection was foreign"));
    }

    let realm_id = hash(input.realm_bytes).to_bytes();
    let realm = RealmV1::decode(input.realm_bytes)
        .map_err(|error| refusal(format!("Direct token setup Realm: {error:?}")))?;
    let config_digest = hash(input.config_bytes).to_bytes();
    let config =
        DirectExecutionConfigV1::decode_selected(config_id, config_digest, input.config_bytes)
            .map_err(|error| refusal(format!("Direct token setup config: {error:?}")))?;
    if realm.to_bytes().as_slice() != input.realm_bytes
        || realm_id != market.identity.realm_id.to_bytes()
        || realm.collateral_mint() != &coordinates.collateral_mint.to_bytes()
        || realm.token_program() != &coordinates.token_program.to_bytes()
        || coordinates.token_program.to_bytes() != TOKEN_2022_PROGRAM_ID
        || config_digest != config_id
        || config.fee_basis_points() != DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1
    {
        return Err(refusal(
            "Direct token setup Realm/config selection was foreign",
        ));
    }
    let profile = PRODUCTION_ADAPTER_RELEASES
        .iter()
        .find(|release| hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id())
        .map(|release| release.profile())
        .ok_or_else(|| refusal("Direct token setup Realm selected a foreign adapter release"))?;
    let mint = profile
        .check_mint(
            coordinates.token_program.to_bytes(),
            input.collateral_mint_bytes,
        )
        .map_err(|error| refusal(format!("Direct token setup collateral Mint: {error:?}")))?;
    if profile.program_id() != TOKEN_2022_PROGRAM_ID
        || (realm.mint_authority_policy() == MintAuthorityPolicy::RequireAbsent
            && !matches!(mint.mint_authority, COption::None))
        || (realm.freeze_authority_policy() == FreezeAuthorityPolicy::RequireAbsent
            && !matches!(mint.freeze_authority, COption::None))
    {
        return Err(refusal(
            "Direct token setup collateral Mint violated its Realm policy",
        ));
    }
    authenticate_finalized_record_coordinates(
        coordinates.registry_program,
        coordinates.realm_raw,
        coordinates.realm_staging,
        REALM_SCHEMA_RELEASE_ID_V1,
        realm_id,
    )?;
    authenticate_finalized_record_coordinates(
        coordinates.registry_program,
        coordinates.config_raw,
        coordinates.config_staging,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        config_id,
    )?;

    let aggregate = LiabilityBasisMarketViewV2::decode(input.claims_aggregate_bytes)
        .map_err(|error| refusal(format!("Direct token setup Claims aggregate: {error:?}")))?;
    let expected_aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, coordinates.market.as_ref()],
        &coordinates.claims_program,
    )
    .0;
    let position = LiabilityBasisPositionViewV2::decode(input.seller_position_bytes)
        .map_err(|error| refusal(format!("Direct token setup seller Position: {error:?}")))?;
    let position_seeds = ProtocolPositionSeedsV2::new(
        coordinates.claims_aggregate.to_bytes(),
        input.seller_owner.to_bytes(),
    )
    .map_err(|error| refusal(format!("Direct token setup Position seeds: {error:?}")))?;
    let expected_position =
        Pubkey::find_program_address(&position_seeds.as_slices(), &coordinates.claims_program).0;
    if coordinates.claims_aggregate != expected_aggregate
        || aggregate.logical_market != coordinates.market.to_bytes()
        || aggregate.release_set != market.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != coordinates.registry_program.to_bytes()
        || aggregate.realm_id != realm_id
        || aggregate.product_instance_id != market.identity.product_id.to_bytes()
        || aggregate.generation != input.generation
        || coordinates.seller_position != expected_position
        || position.market_account != coordinates.claims_aggregate.to_bytes()
        || position.owner != input.seller_owner.to_bytes()
        || position.basis_id != aggregate.basis_id
        || position.claim_count != aggregate.claim_count
    {
        return Err(refusal(
            "Direct token setup Claims/Position join was foreign",
        ));
    }

    let seller_seeds = DirectTokenAccountSeedsV1::new(
        coordinates.market.to_bytes(),
        input.generation,
        input.seller_owner.to_bytes(),
        DirectTokenAccountRoleV1::Seller,
    )
    .map_err(|error| refusal(format!("Direct seller-token seeds: {error:?}")))?;
    let fee_recipient = Pubkey::new_from_array(config.fee_recipient());
    let fee_seeds = DirectTokenAccountSeedsV1::new(
        coordinates.market.to_bytes(),
        input.generation,
        fee_recipient.to_bytes(),
        DirectTokenAccountRoleV1::Fee,
    )
    .map_err(|error| refusal(format!("Direct fee-token seeds: {error:?}")))?;
    let expected_seller =
        Pubkey::find_program_address(&seller_seeds.as_slices(), &coordinates.trading_program).0;
    let expected_fee =
        Pubkey::find_program_address(&fee_seeds.as_slices(), &coordinates.trading_program).0;
    if coordinates.seller_token != expected_seller || coordinates.fee_token != expected_fee {
        return Err(refusal("Direct token setup received a foreign token PDA"));
    }

    let request = DirectTokenSetupRequestV1 {
        market: coordinates.market.to_bytes(),
        expected_market_digest: hash(input.market_bytes).to_bytes(),
        expected_root_digest: hash(input.root_bytes).to_bytes(),
        expected_claims_aggregate_digest: hash(input.claims_aggregate_bytes).to_bytes(),
        seller_owner: input.seller_owner.to_bytes(),
        expected_seller_position_digest: hash(input.seller_position_bytes).to_bytes(),
        generation: input.generation,
    };
    let request_bytes = request
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct token setup request: {error:?}")))?;
    let request_digest = hash(&request_bytes).to_bytes();
    let ordered = coordinates.ordered();
    let frame_digest = direct_token_setup_frame_digest_v1(ordered.map(|key| key.to_bytes()));
    let exact_account_rent = input.rent.minimum_balance(ACCOUNT_BYTES);
    let seller_normalization = direct_token_rent_normalization_v1(
        input.observed_seller_lamports,
        exact_account_rent,
        input.observed_refund_lamports,
    )
    .map_err(|error| refusal(format!("Direct seller-token normalization: {error:?}")))?;
    let refund_after_seller = input
        .observed_refund_lamports
        .checked_add(seller_normalization.refunded_excess)
        .ok_or_else(|| refusal("Direct token setup seller refund overflowed"))?;
    let fee_normalization = direct_token_rent_normalization_v1(
        input.observed_fee_lamports,
        exact_account_rent,
        refund_after_seller,
    )
    .map_err(|error| refusal(format!("Direct fee-token normalization: {error:?}")))?;
    let total_top_up = seller_normalization
        .payer_top_up
        .checked_add(fee_normalization.payer_top_up)
        .ok_or_else(|| refusal("Direct token setup payer debit overflowed"))?;
    let total_refund = seller_normalization
        .refunded_excess
        .checked_add(fee_normalization.refunded_excess)
        .ok_or_else(|| refusal("Direct token setup refund credit overflowed"))?;
    // These two are the projection of THIS INSTRUCTION's effect on the payer
    // and the rent refund, and nothing more. They deliberately exclude the
    // runtime fee, which the payer also pays but which no instruction moves.
    // A chain balance for either wallet therefore never equals its projection,
    // and must never be compared against one: both are shared accounts that go
    // on moving with every other transaction they sign or receive. What the
    // chain IS held to is the exact delta, by the landed transaction's own
    // pre/post balance vectors. Computing them here also preflights payer
    // solvency and refund overflow.
    let projected_payer_lamports = input
        .observed_payer_lamports
        .checked_sub(total_top_up)
        .ok_or_else(|| refusal("Direct token setup payer lacked exact shortfall"))?;
    let projected_refund_lamports = input
        .observed_refund_lamports
        .checked_add(total_refund)
        .ok_or_else(|| refusal("Direct token setup refund balance overflowed"))?;
    let expected_seller_bytes = TokenAccount::initialized_base_bytes(
        coordinates.collateral_mint.to_bytes(),
        input.seller_owner.to_bytes(),
    )
    .map_err(|error| refusal(format!("Direct seller-token poststate: {error:?}")))?;
    let expected_fee_bytes = TokenAccount::initialized_base_bytes(
        coordinates.collateral_mint.to_bytes(),
        fee_recipient.to_bytes(),
    )
    .map_err(|error| refusal(format!("Direct fee-token poststate: {error:?}")))?;
    let expected_receipt = DirectTokenSetupReceiptV1 {
        request_digest,
        frame_digest,
        market: coordinates.market.to_bytes(),
        release_set: market.identity.selected_release_set.to_bytes(),
        realm: realm_id,
        direct_config: config_id,
        claims_aggregate: coordinates.claims_aggregate.to_bytes(),
        seller_position: coordinates.seller_position.to_bytes(),
        collateral_mint: coordinates.collateral_mint.to_bytes(),
        token_program: coordinates.token_program.to_bytes(),
        seller_owner: input.seller_owner.to_bytes(),
        fee_recipient: fee_recipient.to_bytes(),
        seller_token: coordinates.seller_token.to_bytes(),
        fee_token: coordinates.fee_token.to_bytes(),
        rent_refund: coordinates.rent_refund.to_bytes(),
        payer: coordinates.payer.to_bytes(),
        seller_poststate_digest: hash(&expected_seller_bytes).to_bytes(),
        fee_poststate_digest: hash(&expected_fee_bytes).to_bytes(),
        fee_basis_points: DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1,
        seller_normalization,
        fee_normalization,
    };
    let expected_receipt_bytes = expected_receipt
        .to_bytes()
        .map_err(|error| refusal(format!("encode Direct token setup receipt: {error:?}")))?;
    let instruction = Instruction {
        program_id: coordinates.trading_program,
        accounts: account_metas(coordinates),
        data: request_bytes.to_vec(),
    };
    let plan = DirectTradeTokenSetupPlanV1 {
        instruction,
        request,
        request_bytes,
        request_digest,
        frame_digest,
        coordinates,
        seller_owner: input.seller_owner,
        fee_recipient,
        exact_account_rent,
        seller_normalization,
        fee_normalization,
        projected_payer_lamports,
        projected_refund_lamports,
        expected_seller_bytes,
        expected_fee_bytes,
        expected_receipt,
        expected_receipt_bytes,
        expected_immutable_bytes: [
            input.market_bytes.to_vec(),
            input.root_bytes.to_vec(),
            input.realm_bytes.to_vec(),
            input.config_bytes.to_vec(),
            input.claims_aggregate_bytes.to_vec(),
            input.seller_position_bytes.to_vec(),
            input.collateral_mint_bytes.to_vec(),
        ],
        immutable_digests: [
            hash(input.market_bytes).to_bytes(),
            hash(input.root_bytes).to_bytes(),
            hash(input.realm_bytes).to_bytes(),
            hash(input.config_bytes).to_bytes(),
            hash(input.claims_aggregate_bytes).to_bytes(),
            hash(input.seller_position_bytes).to_bytes(),
            hash(input.collateral_mint_bytes).to_bytes(),
        ],
    };
    verify_direct_trade_token_setup_instruction_v1(&plan, &plan.instruction)?;
    Ok(plan)
}

/// Verify the complete instruction rather than trusting a caller-built vector.
pub(crate) fn verify_direct_trade_token_setup_instruction_v1(
    plan: &DirectTradeTokenSetupPlanV1,
    instruction: &Instruction,
) -> Result<()> {
    let expected = Instruction {
        program_id: plan.coordinates.trading_program,
        accounts: account_metas(plan.coordinates),
        data: plan.request_bytes.to_vec(),
    };
    if instruction != &expected {
        return Err(refusal(
            "Direct token setup instruction data, account order, or privileges changed",
        ));
    }
    Ok(())
}

/// Verify return provenance, exact receipt, immutable bytes, and both complete
/// Token-2022 account poststates.
///
/// The receipt carries the exact `payer_top_up` and `refunded_excess` this
/// instruction moved; the landed transaction's own balance vectors are what
/// hold the chain to them. The payer and rent refund are shared wallets whose
/// absolute balances belong to no single transaction, so none is asserted here.
pub(crate) fn verify_direct_trade_token_setup_poststate_v1(
    plan: &DirectTradeTokenSetupPlanV1,
    observed: DirectTradeTokenSetupPoststateV1<'_>,
) -> Result<VerifiedDirectTradeTokenSetupV1> {
    if observed.return_program != plan.coordinates.trading_program
        || observed.return_data != plan.expected_receipt_bytes
    {
        return Err(refusal(
            "Direct token setup return provenance or receipt was wrong",
        ));
    }
    let receipt = DirectTokenSetupReceiptV1::decode(observed.return_data)
        .map_err(|error| refusal(format!("Direct token setup receipt: {error:?}")))?;
    if receipt != plan.expected_receipt {
        return Err(refusal("Direct token setup receipt facts changed"));
    }
    let seller = verify_token(
        observed.seller_token,
        plan.coordinates.seller_token,
        plan.coordinates.token_program,
        plan.exact_account_rent,
        &plan.expected_seller_bytes,
        "seller",
    )?;
    let fee = verify_token(
        observed.fee_token,
        plan.coordinates.fee_token,
        plan.coordinates.token_program,
        plan.exact_account_rent,
        &plan.expected_fee_bytes,
        "fee",
    )?;
    let post_digests = [
        hash(observed.market_bytes).to_bytes(),
        hash(observed.root_bytes).to_bytes(),
        hash(observed.realm_bytes).to_bytes(),
        hash(observed.config_bytes).to_bytes(),
        hash(observed.claims_aggregate_bytes).to_bytes(),
        hash(observed.seller_position_bytes).to_bytes(),
        hash(observed.collateral_mint_bytes).to_bytes(),
    ];
    if post_digests != plan.immutable_digests {
        return Err(refusal(
            "Direct token setup changed a Market, root, record, Claims, or Mint prestate",
        ));
    }
    Ok(VerifiedDirectTradeTokenSetupV1 {
        receipt,
        seller,
        fee,
    })
}

fn authenticate_finalized_record_coordinates(
    registry: Pubkey,
    raw: Pubkey,
    staging: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<()> {
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    if raw != expected_raw || staging != expected_staging {
        return Err(refusal(
            "Direct token setup finalized-record PDA was foreign",
        ));
    }
    Ok(())
}

fn require_coordinates(coordinates: DirectTradeTokenSetupCoordinatesV1) -> Result<()> {
    let accounts = coordinates.ordered();
    if coordinates.rent_sysvar != sysvar::rent::ID
        || coordinates.system_program != system_program::ID
        || coordinates.token_program.to_bytes() != TOKEN_2022_PROGRAM_ID
        || coordinates.trading_program == coordinates.claims_program
        || coordinates.trading_program == coordinates.core_program
        || coordinates.trading_program == coordinates.registry_program
    {
        return Err(refusal(
            "Direct token setup frame contained a foreign program",
        ));
    }
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .is_some_and(|suffix| suffix.contains(account))
        {
            return Err(refusal("Direct token setup frame aliased two accounts"));
        }
    }
    Ok(())
}

fn account_metas(coordinates: DirectTradeTokenSetupCoordinatesV1) -> Vec<AccountMeta> {
    let ordered = coordinates.ordered();
    const PRIVILEGES: [u8; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1] = [
        0, 4, 4, 0, 4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 3, 2, 0, 4, 4,
    ];
    ordered
        .into_iter()
        .zip(PRIVILEGES)
        .map(|(pubkey, privileges)| AccountMeta {
            pubkey,
            is_signer: privileges & 1 != 0,
            is_writable: privileges & 2 != 0,
        })
        .collect()
}

fn verify_token(
    observed: DirectTradeTokenSetupObservedAccountV1<'_>,
    expected_address: Pubkey,
    token_program: Pubkey,
    exact_rent: u64,
    expected_bytes: &[u8; ACCOUNT_BYTES],
    label: &str,
) -> Result<TokenAccount> {
    let token = TokenAccount::parse(observed.data)
        .map_err(|error| refusal(format!("Direct {label} token poststate: {error:?}")))?;
    if observed.address != expected_address
        || observed.owner != token_program
        || observed.lamports != exact_rent
        || observed.executable
        || observed.data != expected_bytes
    {
        return Err(refusal(format!(
            "Direct {label} token account was not the exact InitializeAccount3 poststate"
        )));
    }
    Ok(token)
}

fn refusal(message: impl Into<String>) -> Error {
    Error::new(message)
}

#[cfg(test)]
mod tests {
    use dclutch_market::capability_program::SelectedRecordBumpsV1;
    use dclutch_core_contract::ContentId;
    use dclutch_market::{Identity, MarketIdentity, Readiness};
    use dclutch_market::realm::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};
    use dclutch_registry::release_set::CapabilityExecutionSelectionV1;

    use super::*;

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn key(tag: u8) -> Pubkey {
        Pubkey::new_from_array(id(tag))
    }

    fn content(tag: u8) -> ContentId {
        ContentId::new(id(tag)).expect("content")
    }

    struct Fixture {
        market: Vec<u8>,
        root: Vec<u8>,
        realm: Vec<u8>,
        config: Vec<u8>,
        aggregate: Vec<u8>,
        position: Vec<u8>,
        mint: Vec<u8>,
        coordinates: DirectTradeTokenSetupCoordinatesV1,
        seller: Pubkey,
        rent: Rent,
    }

    impl Fixture {
        fn input(&self) -> DirectTradeTokenSetupBuildInputV1<'_> {
            DirectTradeTokenSetupBuildInputV1 {
                market_bytes: &self.market,
                root_bytes: &self.root,
                realm_bytes: &self.realm,
                config_bytes: &self.config,
                claims_aggregate_bytes: &self.aggregate,
                seller_position_bytes: &self.position,
                collateral_mint_bytes: &self.mint,
                generation: 9,
                seller_owner: self.seller,
                coordinates: self.coordinates,
                rent: self.rent.clone(),
                observed_seller_lamports: 1,
                observed_fee_lamports: self.rent.minimum_balance(ACCOUNT_BYTES) + 1,
                observed_payer_lamports: 10_000_000,
                observed_refund_lamports: 100,
            }
        }
    }

    fn fixture() -> Fixture {
        let core_program = key(20);
        let registry = key(21);
        let trading = key(22);
        let claims = key(23);
        let generation = 9;
        let config_value =
            DirectExecutionConfigV1::new(1_000_000, DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1, id(24))
                .expect("config");
        let config = config_value.encode().to_vec();
        let config_id = hash(&config).to_bytes();
        let adapter = PRODUCTION_ADAPTER_RELEASES
            .iter()
            .find(|release| release.profile().program_id() == TOKEN_2022_PROGRAM_ID)
            .copied()
            .expect("Token-2022 adapter");
        let realm_value = RealmV1::new(RealmV1Input {
            token_program: TOKEN_2022_PROGRAM_ID,
            collateral_mint: key(25).to_bytes(),
            collateral_adapter_release_id: hash(&adapter.to_bytes()).to_bytes(),
            mint_authority_policy: MintAuthorityPolicy::AdmitIssuerControl,
            freeze_authority_policy: FreezeAuthorityPolicy::AdmitIssuerControl,
        })
        .expect("realm");
        let realm = realm_value.to_bytes().to_vec();
        let realm_id = hash(&realm).to_bytes();
        let mut identity = MarketIdentity {
            market_id: Identity::new(id(1)).expect("market placeholder"),
            realm_id: Identity::new(realm_id).expect("realm"),
            product_record: Identity::new(id(3)).expect("product record"),
            product_id: Identity::new(id(4)).expect("product"),
            resolution_policy: Identity::new(id(5)).expect("resolution"),
            capability_manifest: Identity::new(id(6)).expect("manifest"),
            selected_release_set: Identity::new(id(7)).expect("release"),
            registry_program: Identity::new(registry.to_bytes()).expect("registry"),
            generation,
        };
        let market_key = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(identity).as_slices(),
            &core_program,
        )
        .0;
        identity.market_id = Identity::new(market_key.to_bytes()).expect("market");
        let state = CoreState {
            phase: CorePhase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity,
            outstanding_capabilities: 1,
            principal_cap_sets: 1,
            rent_beneficiary: Identity::new(id(8)).expect("refund"),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        };
        let market = state.encode().expect("market").to_vec();
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            content(6),
            ContentId::new(DIRECT_SUCCESSOR_KIND_ID_V3).expect("kind"),
            content(10),
            ContentId::new(config_id).expect("config"),
        )
        .expect("selection");
        let header = CapabilityRootHeaderV1::new(
            content(7),
            market_key.to_bytes(),
            generation,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("header");
        let mut root = header.to_bytes().to_vec();
        root.extend_from_slice(&DirectRootStateV1::new().encode());
        let direct_root = Pubkey::find_program_address(&header.seeds().as_slices(), &trading).0;
        let aggregate_key = Pubkey::find_program_address(
            &[LIABILITY_BASIS_MARKET_SEED_V2, market_key.as_ref()],
            &claims,
        )
        .0;
        let seller = key(27);
        let position_key = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(aggregate_key.to_bytes(), seller.to_bytes())
                .expect("position seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let mut aggregate = vec![0; 272];
        dclutch_claims::liability_basis_state_v2::encode_liability_basis_market_into_v2(
            dclutch_claims::liability_basis_state_v2::LiabilityBasisMarketInputV2 {
                revision: 1,
                logical_market: market_key.to_bytes(),
                release_set: id(7),
                registry_program: registry.to_bytes(),
                product_instance_id: id(4),
                basis_id: id(11),
                realm_id,
                custody_context: id(12),
                generation,
            },
            &[1, 1],
            &mut aggregate,
        )
        .expect("aggregate");
        let mut position = vec![0; 144];
        dclutch_claims::liability_basis_state_v2::encode_liability_basis_position_into_v2(
            dclutch_claims::liability_basis_state_v2::LiabilityBasisPositionInputV2 {
                revision: 1,
                market_account: aggregate_key.to_bytes(),
                owner: seller.to_bytes(),
                basis_id: id(11),
            },
            &[1, 0],
            &mut position,
        )
        .expect("position");
        let seller_seeds = DirectTokenAccountSeedsV1::new(
            market_key.to_bytes(),
            generation,
            seller.to_bytes(),
            DirectTokenAccountRoleV1::Seller,
        )
        .expect("seller seeds");
        let fee_seeds = DirectTokenAccountSeedsV1::new(
            market_key.to_bytes(),
            generation,
            id(24),
            DirectTokenAccountRoleV1::Fee,
        )
        .expect("fee seeds");
        let realm_raw = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &REALM_SCHEMA_RELEASE_ID_V1,
                &realm_id,
            ],
            &registry,
        )
        .0;
        let realm_staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &REALM_SCHEMA_RELEASE_ID_V1,
                &realm_id,
            ],
            &registry,
        )
        .0;
        let config_raw = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
                &config_id,
            ],
            &registry,
        )
        .0;
        let config_staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
                &config_id,
            ],
            &registry,
        )
        .0;
        let activation_cache =
            Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &id(7)], &registry).0;
        let mut mint = vec![0; dclutch_custody::token_svm::MINT_BYTES];
        mint[45] = 1;
        Fixture {
            market,
            root,
            realm,
            config,
            aggregate,
            position,
            mint,
            coordinates: DirectTradeTokenSetupCoordinatesV1 {
                market: market_key,
                core_program,
                registry_program: registry,
                activation_cache,
                trading_program: trading,
                trading_programdata: key(30),
                claims_program: claims,
                claims_programdata: key(31),
                direct_root,
                realm_raw,
                realm_staging,
                config_raw,
                config_staging,
                claims_aggregate: aggregate_key,
                seller_position: position_key,
                collateral_mint: key(25),
                seller_token: Pubkey::find_program_address(&seller_seeds.as_slices(), &trading).0,
                fee_token: Pubkey::find_program_address(&fee_seeds.as_slices(), &trading).0,
                payer: key(32),
                rent_refund: key(8),
                rent_sysvar: sysvar::rent::ID,
                system_program: system_program::ID,
                token_program: Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            },
            seller,
            rent: Rent::default(),
        }
    }

    fn poststate<'a>(
        fixture: &'a Fixture,
        plan: &'a DirectTradeTokenSetupPlanV1,
        return_data: &'a [u8],
    ) -> DirectTradeTokenSetupPoststateV1<'a> {
        DirectTradeTokenSetupPoststateV1 {
            return_program: plan.coordinates.trading_program,
            return_data,
            seller_token: DirectTradeTokenSetupObservedAccountV1 {
                address: plan.coordinates.seller_token,
                owner: plan.coordinates.token_program,
                lamports: plan.exact_account_rent,
                executable: false,
                data: &plan.expected_seller_bytes,
            },
            fee_token: DirectTradeTokenSetupObservedAccountV1 {
                address: plan.coordinates.fee_token,
                owner: plan.coordinates.token_program,
                lamports: plan.exact_account_rent,
                executable: false,
                data: &plan.expected_fee_bytes,
            },
            market_bytes: &fixture.market,
            root_bytes: &fixture.root,
            realm_bytes: &fixture.realm,
            config_bytes: &fixture.config,
            claims_aggregate_bytes: &fixture.aggregate,
            seller_position_bytes: &fixture.position,
            collateral_mint_bytes: &fixture.mint,
        }
    }

    #[test]
    fn builds_exact_23_account_instruction_and_verifies_full_poststate() {
        let fixture = fixture();
        let plan = build_direct_trade_token_setup_v1(fixture.input()).expect("plan");
        assert_eq!(
            plan.instruction.accounts.len(),
            DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1
        );
        assert_eq!(
            plan.instruction
                .accounts
                .iter()
                .filter(|meta| meta.is_signer)
                .count(),
            1
        );
        let verified = verify_direct_trade_token_setup_poststate_v1(
            &plan,
            poststate(&fixture, &plan, &plan.expected_receipt_bytes),
        )
        .expect("verified");
        assert_eq!(verified.receipt, plan.expected_receipt);
    }

    #[test]
    fn refuses_reordering_alias_foreign_owner_pda_and_receipt() {
        let fixture = fixture();
        let plan = build_direct_trade_token_setup_v1(fixture.input()).expect("plan");
        let mut reordered = plan.instruction.clone();
        reordered.accounts.swap(16, 17);
        assert!(verify_direct_trade_token_setup_instruction_v1(&plan, &reordered).is_err());
        let mut alias = fixture.input();
        alias.coordinates.fee_token = alias.coordinates.seller_token;
        assert!(build_direct_trade_token_setup_v1(alias).is_err());
        let mut foreign_owner = fixture.input();
        foreign_owner.seller_owner = key(99);
        assert!(build_direct_trade_token_setup_v1(foreign_owner).is_err());
        let mut foreign_root = fixture.input();
        foreign_root.coordinates.direct_root = key(98);
        assert!(build_direct_trade_token_setup_v1(foreign_root).is_err());
        let mut receipt = plan.expected_receipt_bytes;
        receipt[16] ^= 1;
        assert!(
            verify_direct_trade_token_setup_poststate_v1(
                &plan,
                poststate(&fixture, &plan, &receipt),
            )
            .is_err()
        );
        let mut foreign_mint = fixture.input();
        let hostile_mint = vec![0; dclutch_custody::token_svm::MINT_BYTES];
        foreign_mint.collateral_mint_bytes = &hostile_mint;
        assert!(build_direct_trade_token_setup_v1(foreign_mint).is_err());
    }

    /// The two wallet balances are preflights, not expectations: they gate the
    /// build and are never recorded, so these are the only assertions that
    /// still hold them to anything.
    #[test]
    fn refuses_an_insolvent_payer_and_an_overflowing_refund() {
        let fixture = fixture();
        let rent = fixture.rent.minimum_balance(ACCOUNT_BYTES);
        let shortfall = |payer: u64| {
            let mut input = fixture.input();
            // Both token PDAs sit one lamport under rent, so the instruction
            // moves exactly two lamports out of the payer.
            input.observed_seller_lamports = rent - 1;
            input.observed_fee_lamports = rent - 1;
            input.observed_payer_lamports = payer;
            build_direct_trade_token_setup_v1(input)
        };
        assert!(shortfall(1).is_err());
        assert!(shortfall(2).is_ok());
        let mut overflow = fixture.input();
        overflow.observed_seller_lamports = rent + 1;
        overflow.observed_refund_lamports = u64::MAX;
        assert!(build_direct_trade_token_setup_v1(overflow).is_err());
    }

    #[test]
    fn normalization_matches_every_dust_boundary() {
        let fixture = fixture();
        let rent = fixture.rent.minimum_balance(ACCOUNT_BYTES);
        for observed in [0, 1, rent - 1, rent, rent + 1, u64::MAX] {
            let mut input = fixture.input();
            input.observed_seller_lamports = observed;
            input.observed_fee_lamports = rent;
            input.observed_payer_lamports = u64::MAX;
            input.observed_refund_lamports = 0;
            let plan = build_direct_trade_token_setup_v1(input).expect("dust-normalized plan");
            assert_eq!(
                plan.seller_normalization,
                direct_token_rent_normalization_v1(observed, rent, 0).expect("normalization")
            );
        }
    }
}
