//! Permissionless creation of Direct's seller and venue-fee Token-2022 accounts.
//!
//! The seller owner is not trusted instruction data: it must own the canonical
//! Claims Position under the Claims release selected by the open Market. The
//! fee owner comes only from the finalized Direct config, which this release
//! pins to 50 basis points. Trading derives both token accounts as distinct
//! role-separated PDAs and initializes zero balances; this route performs no
//! claim, collateral, fee, nonce, or other economic Direct movement.

extern crate alloc;

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_claims::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2;
use dclutch_claims::{
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_trading::{
    direct_root_admission_v1::DIRECT_ROOT_OPEN_ADMISSIBLE_STATES_V1,
    execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3,
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1, DirectExecutionConfigV1,
        DirectRootStateV1,
    },
    token_setup_v1::{
        DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1, DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1,
        DirectTokenAccountRoleV1, DirectTokenAccountSeedsV1, DirectTokenRentNormalizationV1,
        DirectTokenSetupReceiptV1, DirectTokenSetupRequestV1, direct_token_rent_normalization_v1,
        direct_token_setup_frame_digest_v1,
    },
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_market::realm::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry::release_set::ExecutionRoleV1;
use dclutch_custody::token_svm::{
    ACCOUNT_BYTES, COption, PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID, TokenAccount,
    initialize_account3,
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::market_admission_v1::TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1;
use crate::{
    TradingSbfError, child_refused_v1,
    execution_strategy_v2::authenticate_activated_current_deployment,
};

const MARKET: usize = 0;
const CORE_PROGRAM: usize = 1;
const REGISTRY: usize = 2;
const ACTIVATION_CACHE: usize = 3;
const TRADING_PROGRAM: usize = 4;
const TRADING_PROGRAMDATA: usize = 5;
const CLAIMS_PROGRAM: usize = 6;
const CLAIMS_PROGRAMDATA: usize = 7;
const DIRECT_ROOT: usize = 8;
const REALM_RAW: usize = 9;
const REALM_STAGING: usize = 10;
const CONFIG_RAW: usize = 11;
const CONFIG_STAGING: usize = 12;
const CLAIMS_AGGREGATE: usize = 13;
const SELLER_POSITION: usize = 14;
const COLLATERAL_MINT: usize = 15;
const SELLER_TOKEN: usize = 16;
const FEE_TOKEN: usize = 17;
const PAYER: usize = 18;
const RENT_REFUND: usize = 19;
const RENT: usize = 20;
const SYSTEM_PROGRAM: usize = 21;
const TOKEN_PROGRAM: usize = 22;

const EXPECTED_PRIVILEGES: [u8; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1] = [
    0, 4, 4, 0, 4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 3, 2, 0, 4, 4,
];

#[derive(Clone, Copy)]
struct AuthenticatedSetupV1 {
    release_set: [u8; 32],
    realm_id: [u8; 32],
    config_id: [u8; 32],
    claims_aggregate: [u8; 32],
    seller_position: [u8; 32],
    collateral_mint: [u8; 32],
    token_program: [u8; 32],
    seller_owner: [u8; 32],
    fee_recipient: [u8; 32],
    /// The venue rate this market's finalized config states.
    ///
    /// FROM THE CONFIG, like `fee_recipient` two lines up. This route already
    /// authenticates that record and already trusts one of its fields; pinning
    /// its neighbour to a literal made the same record half-authoritative, and
    /// four markets were founded dead because of it.
    fee_basis_points: u16,
    seller_token: [u8; 32],
    fee_token: [u8; 32],
    rent_refund: [u8; 32],
}

/// Execute one exact Direct Token-2022 account setup request.
#[inline(never)]
pub fn process_direct_token_setup_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = DirectTokenSetupRequestV1::decode(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_top_frame(program_id, accounts)?;
    let rent =
        Rent::from_account_info(account(accounts, RENT)?).map_err(|_| TradingSbfError::Content)?;
    let request_digest = hash(instruction_data).to_bytes();
    let frame_digest = authenticated_frame_digest_v1(accounts)?;
    let authenticated = authenticate_semantics(program_id, accounts, request)?;

    // Preflight every checked delta before the first CPI. Transaction rollback
    // remains the outer atomicity boundary, but arithmetic is never discovered
    // only after one account has already been normalized.
    let exact_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let seller_observed = account(accounts, SELLER_TOKEN)?.lamports();
    let fee_observed = account(accounts, FEE_TOKEN)?.lamports();
    let payer_before = account(accounts, PAYER)?.lamports();
    let refund_before = account(accounts, RENT_REFUND)?.lamports();
    let seller_normalization =
        direct_token_rent_normalization_v1(seller_observed, exact_rent, refund_before)
            .map_err(|_| TradingSbfError::Transition)?;
    let refund_after_seller = refund_before
        .checked_add(seller_normalization.refunded_excess)
        .ok_or(TradingSbfError::Transition)?;
    let fee_normalization =
        direct_token_rent_normalization_v1(fee_observed, exact_rent, refund_after_seller)
            .map_err(|_| TradingSbfError::Transition)?;
    let total_top_up = seller_normalization
        .payer_top_up
        .checked_add(fee_normalization.payer_top_up)
        .ok_or(TradingSbfError::Transition)?;
    let total_refund = seller_normalization
        .refunded_excess
        .checked_add(fee_normalization.refunded_excess)
        .ok_or(TradingSbfError::Transition)?;
    let expected_payer_after = payer_before
        .checked_sub(total_top_up)
        .ok_or(TradingSbfError::Transition)?;
    let expected_refund_after = refund_before
        .checked_add(total_refund)
        .ok_or(TradingSbfError::Transition)?;

    let immutable_before = immutable_digests(accounts)?;
    normalize_and_initialize_token(
        program_id,
        accounts,
        SELLER_TOKEN,
        authenticated.seller_owner,
        DirectTokenAccountRoleV1::Seller,
        seller_normalization,
        request,
    )?;
    normalize_and_initialize_token(
        program_id,
        accounts,
        FEE_TOKEN,
        authenticated.fee_recipient,
        DirectTokenAccountRoleV1::Fee,
        fee_normalization,
        request,
    )?;

    if account(accounts, PAYER)?.lamports() != expected_payer_after
        || account(accounts, RENT_REFUND)?.lamports() != expected_refund_after
        || immutable_digests(accounts)? != immutable_before
    {
        return Err(TradingSbfError::Transition.into());
    }
    emit_token_setup_receipt_v1(
        accounts,
        request,
        &authenticated,
        request_digest,
        frame_digest,
        exact_rent,
        seller_normalization,
        fee_normalization,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn emit_token_setup_receipt_v1(
    accounts: &[AccountInfo<'_>],
    request: DirectTokenSetupRequestV1,
    authenticated: &AuthenticatedSetupV1,
    request_digest: [u8; 32],
    frame_digest: [u8; 32],
    exact_rent: u64,
    seller_normalization: DirectTokenRentNormalizationV1,
    fee_normalization: DirectTokenRentNormalizationV1,
) -> ProgramResult {
    let seller_poststate_digest = authenticate_token_poststate(
        accounts,
        SELLER_TOKEN,
        authenticated.seller_owner,
        exact_rent,
    )?;
    let fee_poststate_digest =
        authenticate_token_poststate(accounts, FEE_TOKEN, authenticated.fee_recipient, exact_rent)?;
    let receipt = DirectTokenSetupReceiptV1 {
        request_digest,
        frame_digest,
        market: request.market,
        release_set: authenticated.release_set,
        realm: authenticated.realm_id,
        direct_config: authenticated.config_id,
        claims_aggregate: authenticated.claims_aggregate,
        seller_position: authenticated.seller_position,
        collateral_mint: authenticated.collateral_mint,
        token_program: authenticated.token_program,
        seller_owner: authenticated.seller_owner,
        fee_recipient: authenticated.fee_recipient,
        seller_token: authenticated.seller_token,
        fee_token: authenticated.fee_token,
        rent_refund: authenticated.rent_refund,
        payer: account(accounts, PAYER)?.key.to_bytes(),
        seller_poststate_digest,
        fee_poststate_digest,
        fee_basis_points: authenticated.fee_basis_points,
        seller_normalization,
        fee_normalization,
    }
    .to_bytes()
    .map_err(|_| TradingSbfError::Width)?;
    set_return_data(&receipt);
    Ok(())
}

#[inline(never)]
fn authenticate_top_frame(program_id: &Pubkey, accounts: &[AccountInfo<'_>]) -> ProgramResult {
    if accounts.len() != DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    for (index, info) in accounts.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .is_some_and(|suffix| suffix.iter().any(|other| other.key == info.key))
            || privilege_byte(info) != EXPECTED_PRIVILEGES[index]
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    if account(accounts, TRADING_PROGRAM)?.key != program_id
        || account(accounts, CORE_PROGRAM)?.key == program_id
        || account(accounts, CLAIMS_PROGRAM)?.key == program_id
        || account(accounts, REGISTRY)?.key == program_id
        || account(accounts, RENT)?.key != &sysvar::rent::ID
        || account(accounts, RENT)?.owner != &sysvar::ID
        || account(accounts, SYSTEM_PROGRAM)?.key != &system_program::ID
        || account(accounts, TOKEN_PROGRAM)?.key.to_bytes() != TOKEN_2022_PROGRAM_ID
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_semantics(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: DirectTokenSetupRequestV1,
) -> Result<AuthenticatedSetupV1, ProgramError> {
    let market = authenticate_market(accounts, request)?;
    let (trading_release, claims_release) = authenticate_activation(accounts, market)?;
    authenticate_activated_current_deployment(
        trading_release,
        account(accounts, TRADING_PROGRAM)?,
        account(accounts, TRADING_PROGRAMDATA)?,
    )
    .map_err(ProgramError::from)?;
    authenticate_activated_current_deployment(
        claims_release,
        account(accounts, CLAIMS_PROGRAM)?,
        account(accounts, CLAIMS_PROGRAMDATA)?,
    )
    .map_err(ProgramError::from)?;
    let (config_id, config) = authenticate_root_and_config(program_id, accounts, request, market)?;
    let realm = authenticate_realm(accounts, market)?;
    authenticate_mint(accounts, realm)?;
    let (_aggregate, position) = authenticate_seller_position(accounts, request, market)?;
    let seller_seeds = DirectTokenAccountSeedsV1::new(
        request.market,
        request.generation,
        position.owner,
        DirectTokenAccountRoleV1::Seller,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let fee_seeds = DirectTokenAccountSeedsV1::new(
        request.market,
        request.generation,
        config.fee_recipient(),
        DirectTokenAccountRoleV1::Fee,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let seller_token = Pubkey::find_program_address(&seller_seeds.as_slices(), program_id).0;
    let fee_token = Pubkey::find_program_address(&fee_seeds.as_slices(), program_id).0;
    let seller_account = account(accounts, SELLER_TOKEN)?;
    let fee_account = account(accounts, FEE_TOKEN)?;
    if seller_token == fee_token
        || seller_account.key != &seller_token
        || fee_account.key != &fee_token
        || seller_account.owner != &system_program::ID
        || seller_account.data_len() != 0
        || fee_account.owner != &system_program::ID
        || fee_account.data_len() != 0
        || account(accounts, RENT_REFUND)?.key.to_bytes() != market.rent_beneficiary.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(AuthenticatedSetupV1 {
        fee_basis_points: config.fee_basis_points(),
        release_set: market.identity.selected_release_set.to_bytes(),
        realm_id: market.identity.realm_id.to_bytes(),
        config_id,
        claims_aggregate: account(accounts, CLAIMS_AGGREGATE)?.key.to_bytes(),
        seller_position: account(accounts, SELLER_POSITION)?.key.to_bytes(),
        collateral_mint: *realm.collateral_mint(),
        token_program: *realm.token_program(),
        seller_owner: position.owner,
        fee_recipient: config.fee_recipient(),
        seller_token: seller_token.to_bytes(),
        fee_token: fee_token.to_bytes(),
        rent_refund: market.rent_beneficiary.to_bytes(),
    })
}

#[inline(never)]
fn authenticate_market(
    accounts: &[AccountInfo<'_>],
    request: DirectTokenSetupRequestV1,
) -> Result<CoreState, ProgramError> {
    let market = account(accounts, MARKET)?;
    let data = market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let state = CoreState::decode(&data).map_err(|_| TradingSbfError::Content)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        account(accounts, CORE_PROGRAM)?.key,
    )
    .0;
    if market.owner != account(accounts, CORE_PROGRAM)?.key
        || market.key != &expected
        || market.key.to_bytes() != request.market
        || data.len() != STATE_BYTES
        || state
            .encode()
            .map_err(|_| TradingSbfError::Content)?
            .as_slice()
            != data.as_ref()
        || hash(&data).to_bytes() != request.expected_market_digest
        || !TRADING_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(state.phase)
        || state.identity.market_id.to_bytes() != request.market
        || state.identity.generation != request.generation
        || state.identity.registry_program.to_bytes() != account(accounts, REGISTRY)?.key.to_bytes()
        || state.rent_beneficiary.to_bytes() != account(accounts, RENT_REFUND)?.key.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(state)
}

#[inline(never)]
fn authenticate_activation(
    accounts: &[AccountInfo<'_>],
    market: CoreState,
) -> Result<
    (
        dclutch_registry::ArtifactReleaseV1,
        dclutch_registry::ArtifactReleaseV1,
    ),
    ProgramError,
> {
    let cache = account(accounts, ACTIVATION_CACHE)?;
    let release_set = market.identity.selected_release_set.to_bytes();
    let expected = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, release_set.as_slice()],
        account(accounts, REGISTRY)?.key,
    )
    .0;
    let data = cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| TradingSbfError::Release)?;
    let claims = activated
        .role(ExecutionRoleV1::Claims)
        .map_err(|_| TradingSbfError::Release)?;
    if cache.key != &expected
        || cache.owner != account(accounts, REGISTRY)?.key
        || activated
            .execution_release_set_id()
            .map_err(|_| TradingSbfError::Release)?
            .to_bytes()
            != release_set
        || trading.release().program().to_bytes()
            != account(accounts, TRADING_PROGRAM)?.key.to_bytes()
        || claims.release().program().to_bytes()
            != account(accounts, CLAIMS_PROGRAM)?.key.to_bytes()
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok((trading.release(), claims.release()))
}

#[inline(never)]
fn authenticate_root_and_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: DirectTokenSetupRequestV1,
    market: CoreState,
) -> Result<([u8; 32], DirectExecutionConfigV1), ProgramError> {
    let root = account(accounts, DIRECT_ROOT)?;
    let data = root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let expected_width = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or(TradingSbfError::Content)?;
    if data.len() != expected_width || hash(&data).to_bytes() != request.expected_root_digest {
        return Err(TradingSbfError::Content.into());
    }
    let header = CapabilityRootHeaderV1::decode(
        data.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let state = DirectRootStateV1::decode(
        data.get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let expected_root = Pubkey::find_program_address(&header.seeds().as_slices(), program_id).0;
    let selection = header.selection();
    let config_id = selection.config().to_bytes();
    if root.owner != program_id
        || root.key != &expected_root
        || !funded_rent_persists_v1(root.lamports())
        || header.market() != request.market
        || header.generation() != request.generation
        || header.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || selection.kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3
        || !DIRECT_ROOT_OPEN_ADMISSIBLE_STATES_V1.admits(state.phase())
    {
        return Err(TradingSbfError::Content.into());
    }
    drop(data);
    let config_data = borrow_finalized_record(
        accounts,
        CONFIG_RAW,
        CONFIG_STAGING,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        config_id,
    )?;
    let config = DirectExecutionConfigV1::decode_selected(
        config_id,
        hash(&config_data).to_bytes(),
        &config_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    // THE RATE IS THE CONFIG'S, AND THE BAND IS ALREADY ENFORCED ABOVE.
    // `decode_selected` calls `DirectExecutionConfigV1::new`, which refuses any
    // rate over `DIRECT_MAX_FEE_BASIS_POINTS_V1`. This route used to additionally
    // demand exactly 50, which is neither the protocol band nor anything the
    // rest of the release believes: `settle_inline_ordinary_v2` computes both
    // fees with `fee_floor_v2(gross, execution.config.fee_basis_points)`, from
    // the record, at whatever rate it states.
    //
    // The cost of the extra conjunct was total and silent. This route is the SOLE
    // creator of the seller's and the venue's Direct token accounts and every
    // fill needs them, so a market founded at any other rate could never trade --
    // and the config record is finalized and immutable, so it could never be
    // repaired either. Four markets were founded that way, cohort-11's included
    // (`62kFf7i2vRkG...`, 30 basis points), before anything refused.
    Ok((config_id, config))
}

#[inline(never)]
fn authenticate_realm(
    accounts: &[AccountInfo<'_>],
    market: CoreState,
) -> Result<RealmV1, ProgramError> {
    let realm_id = market.identity.realm_id.to_bytes();
    let data = borrow_finalized_record(
        accounts,
        REALM_RAW,
        REALM_STAGING,
        REALM_SCHEMA_RELEASE_ID_V1,
        realm_id,
    )?;
    let realm = RealmV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    if realm.to_bytes().as_slice() != data.as_ref()
        || realm.token_program() != &TOKEN_2022_PROGRAM_ID
        || realm.collateral_mint() != &account(accounts, COLLATERAL_MINT)?.key.to_bytes()
        || realm.token_program() != &account(accounts, TOKEN_PROGRAM)?.key.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    let mut selected_profile = None;
    for release in PRODUCTION_ADAPTER_RELEASES {
        if hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id() {
            selected_profile = Some(release.profile());
        }
    }
    let profile = selected_profile.ok_or(TradingSbfError::Content)?;
    if profile.program_id() != TOKEN_2022_PROGRAM_ID {
        return Err(TradingSbfError::Content.into());
    }
    Ok(realm)
}

#[inline(never)]
fn authenticate_mint(accounts: &[AccountInfo<'_>], realm: RealmV1) -> ProgramResult {
    let mint_account = account(accounts, COLLATERAL_MINT)?;
    let data = mint_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let profile = PRODUCTION_ADAPTER_RELEASES
        .iter()
        .find(|release| hash(&release.to_bytes()).as_ref() == realm.collateral_adapter_release_id())
        .map(|release| release.profile())
        .ok_or(TradingSbfError::Content)?;
    let mint = profile
        .check_mint(account(accounts, TOKEN_PROGRAM)?.key.to_bytes(), &data)
        .map_err(|_| TradingSbfError::Content)?;
    let mint_authority_ok = realm.mint_authority_policy()
        == MintAuthorityPolicy::AdmitIssuerControl
        || matches!(mint.mint_authority, COption::None);
    let freeze_authority_ok = realm.freeze_authority_policy()
        == FreezeAuthorityPolicy::AdmitIssuerControl
        || matches!(mint.freeze_authority, COption::None);
    if mint_account.owner != account(accounts, TOKEN_PROGRAM)?.key
        || !mint_authority_ok
        || !freeze_authority_ok
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_seller_position(
    accounts: &[AccountInfo<'_>],
    request: DirectTokenSetupRequestV1,
    market: CoreState,
) -> Result<(LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2), ProgramError> {
    let claims_program = account(accounts, CLAIMS_PROGRAM)?;
    let aggregate_account = account(accounts, CLAIMS_AGGREGATE)?;
    let aggregate_data = aggregate_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_data)
        .map_err(|_| TradingSbfError::Content)?;
    let expected_aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, request.market.as_slice()],
        claims_program.key,
    )
    .0;
    if aggregate_account.owner != claims_program.key
        || aggregate_account.key != &expected_aggregate
        || hash(&aggregate_data).to_bytes() != request.expected_claims_aggregate_digest
        || aggregate.logical_market != request.market
        || aggregate.release_set != market.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != market.identity.registry_program.to_bytes()
        || aggregate.realm_id != market.identity.realm_id.to_bytes()
        || aggregate.product_instance_id != market.identity.product_id.to_bytes()
        || aggregate.generation != request.generation
    {
        return Err(TradingSbfError::Content.into());
    }
    let position_account = account(accounts, SELLER_POSITION)?;
    let position_data = position_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let position = LiabilityBasisPositionViewV2::decode(&position_data)
        .map_err(|_| TradingSbfError::Content)?;
    let position_seeds =
        ProtocolPositionSeedsV2::new(aggregate_account.key.to_bytes(), request.seller_owner)
            .map_err(|_| TradingSbfError::Content)?;
    let expected_position =
        Pubkey::find_program_address(&position_seeds.as_slices(), claims_program.key).0;
    if position_account.owner != claims_program.key
        || position_account.key != &expected_position
        || hash(&position_data).to_bytes() != request.expected_seller_position_digest
        || position.market_account != aggregate_account.key.to_bytes()
        || position.owner != request.seller_owner
        || position.basis_id != aggregate.basis_id
        || position.claim_count != aggregate.claim_count
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok((aggregate, position))
}

#[inline(never)]
fn borrow_finalized_record<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    raw_index: usize,
    staging_index: usize,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<core::cell::Ref<'a, [u8]>, ProgramError> {
    let raw = account(accounts, raw_index)?;
    let staging = account(accounts, staging_index)?;
    let registry = account(accounts, REGISTRY)?;
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        registry.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        registry.key,
    )
    .0;
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if raw.key != &expected_raw
        || raw.owner != registry.key
        || hash(&data).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(core::cell::Ref::map(data, |bytes| &**bytes))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn normalize_and_initialize_token(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    resource_index: usize,
    owner: [u8; 32],
    role: DirectTokenAccountRoleV1,
    normalization: DirectTokenRentNormalizationV1,
    request: DirectTokenSetupRequestV1,
) -> ProgramResult {
    let resource = account(accounts, resource_index)?;
    let payer = account(accounts, PAYER)?;
    let refund = account(accounts, RENT_REFUND)?;
    let system = account(accounts, SYSTEM_PROGRAM)?;
    let seeds = DirectTokenAccountSeedsV1::new(request.market, request.generation, owner, role)
        .map_err(|_| TradingSbfError::Content)?;
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, market, generation, token_owner, role] = seeds.as_slices();
    let signer_seeds = [domain, market, generation, token_owner, role, &bump_seed];
    if normalization.refunded_excess != 0 {
        invoke_signed(
            &transfer(resource.key, refund.key, normalization.refunded_excess),
            &[resource.clone(), refund.clone(), system.clone()],
            &[&signer_seeds],
        )
        .map_err(child_refused_v1)?;
    } else if normalization.payer_top_up != 0 {
        invoke(
            &transfer(payer.key, resource.key, normalization.payer_top_up),
            &[payer.clone(), resource.clone(), system.clone()],
        )
        .map_err(child_refused_v1)?;
    }
    for instruction in [
        allocate(
            resource.key,
            u64::try_from(ACCOUNT_BYTES).map_err(|_| TradingSbfError::Width)?,
        ),
        assign(resource.key, account(accounts, TOKEN_PROGRAM)?.key),
    ] {
        invoke_signed(
            &Instruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts,
                data: instruction.data,
            },
            &[resource.clone(), system.clone()],
            &[&signer_seeds],
        )
        .map_err(|_| TradingSbfError::Transition)?;
    }
    let specification = initialize_account3(
        account(accounts, TOKEN_PROGRAM)?.key.to_bytes(),
        resource.key.to_bytes(),
        account(accounts, COLLATERAL_MINT)?.key.to_bytes(),
        owner,
    )
    .map_err(|_| TradingSbfError::Width)?;
    let token_instruction = Instruction {
        program_id: Pubkey::new_from_array(*specification.program_id()),
        accounts: specification
            .accounts()
            .iter()
            .map(|meta| {
                if meta.is_writable() {
                    AccountMeta::new(Pubkey::new_from_array(*meta.address()), meta.is_signer())
                } else {
                    AccountMeta::new_readonly(
                        Pubkey::new_from_array(*meta.address()),
                        meta.is_signer(),
                    )
                }
            })
            .collect(),
        data: specification.data().to_vec(),
    };
    invoke(
        &token_instruction,
        &[
            resource.clone(),
            account(accounts, COLLATERAL_MINT)?.clone(),
            account(accounts, TOKEN_PROGRAM)?.clone(),
        ],
    )
    .map_err(|_| TradingSbfError::Transition)?;
    Ok(())
}

#[inline(never)]
fn authenticate_token_poststate(
    accounts: &[AccountInfo<'_>],
    index: usize,
    owner: [u8; 32],
    exact_rent: u64,
) -> Result<[u8; 32], ProgramError> {
    let token_account = account(accounts, index)?;
    let data = token_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AccountData)?;
    let expected = TokenAccount::initialized_base_bytes(
        account(accounts, COLLATERAL_MINT)?.key.to_bytes(),
        owner,
    )
    .map_err(|_| TradingSbfError::Width)?;
    if token_account.owner != account(accounts, TOKEN_PROGRAM)?.key
        || token_account.lamports() != exact_rent
        || data.as_ref() != expected.as_slice()
    {
        return Err(TradingSbfError::Transition.into());
    }
    Ok(hash(&data).to_bytes())
}

#[inline(never)]
fn immutable_digests(accounts: &[AccountInfo<'_>]) -> Result<[[u8; 32]; 8], ProgramError> {
    let mut output = [[0; 32]; 8];
    for (slot, index) in [
        MARKET,
        DIRECT_ROOT,
        REALM_RAW,
        CONFIG_RAW,
        CLAIMS_AGGREGATE,
        SELLER_POSITION,
        COLLATERAL_MINT,
        ACTIVATION_CACHE,
    ]
    .into_iter()
    .enumerate()
    {
        let data = account(accounts, index)?
            .try_borrow_data()
            .map_err(|_| TradingSbfError::AccountData)?;
        output[slot] = hash(&data).to_bytes();
    }
    Ok(output)
}

#[inline(never)]
fn frame_addresses(
    accounts: &[AccountInfo<'_>],
) -> Result<[[u8; 32]; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1], ProgramError> {
    let mut output = [[0; 32]; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1];
    for (destination, source) in output.iter_mut().zip(accounts) {
        *destination = source.key.to_bytes();
    }
    if accounts.len() != output.len() {
        return Err(TradingSbfError::Content.into());
    }
    Ok(output)
}

#[inline(never)]
fn authenticated_frame_digest_v1(accounts: &[AccountInfo<'_>]) -> Result<[u8; 32], ProgramError> {
    Ok(direct_token_setup_frame_digest_v1(frame_addresses(
        accounts,
    )?))
}

fn privilege_byte(info: &AccountInfo<'_>) -> u8 {
    u8::from(info.is_signer) | (u8::from(info.is_writable) << 1) | (u8::from(info.executable) << 2)
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use dclutch_trading::successor::DIRECT_MAX_FEE_BASIS_POINTS_V1;

    use dclutch_claims::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
        encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
        liability_basis_vector_width_v2,
    };
    use std::vec;

    use super::*;

    #[derive(Clone, Copy)]
    struct SemanticFixtureV1 {
        market: [u8; 32],
        generation: u64,
        release: [u8; 32],
        realm: [u8; 32],
        registry: [u8; 32],
        config: [u8; 32],
        mint: [u8; 32],
        token_program: [u8; 32],
        fee_recipient: [u8; 32],
        seller_owner: [u8; 32],
        aggregate: [u8; 32],
        position: [u8; 32],
        expected_aggregate: [u8; 32],
        expected_position: [u8; 32],
        seller_token: [u8; 32],
        fee_token: [u8; 32],
        expected_seller_token: [u8; 32],
        expected_fee_token: [u8; 32],
        fee_basis_points: u16,
    }

    impl SemanticFixtureV1 {
        fn validate(self) -> Result<(), TradingSbfError> {
            if self.market == [0; 32]
                || self.release == [0; 32]
                || self.realm == [0; 32]
                || self.registry == [0; 32]
                || self.config == [0; 32]
                || self.mint == [0; 32]
                || self.token_program != TOKEN_2022_PROGRAM_ID
                || self.fee_recipient == [0; 32]
                || self.seller_owner == [0; 32]
                || self.aggregate != self.expected_aggregate
                || self.position != self.expected_position
                || self.seller_token != self.expected_seller_token
                || self.fee_token != self.expected_fee_token
                || self.seller_token == self.fee_token
                // THE BAND, NOT THE POINT. This mirror demanded exactly 50 while
                // the route it models did, and updating one without the other is
                // how a test keeps a removed law alive in the only place a reader
                // would look for it. The protocol bound is
                // `DIRECT_MAX_FEE_BASIS_POINTS_V1`, enforced once, by
                // `DirectExecutionConfigV1::new`.
                || self.fee_basis_points > DIRECT_MAX_FEE_BASIS_POINTS_V1
                || self.generation == u64::MAX
            {
                Err(TradingSbfError::Content)
            } else {
                Ok(())
            }
        }
    }

    fn id(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn semantic_fixture() -> SemanticFixtureV1 {
        SemanticFixtureV1 {
            market: id(1),
            generation: 2,
            release: id(3),
            realm: id(4),
            registry: id(5),
            config: id(6),
            mint: id(7),
            token_program: TOKEN_2022_PROGRAM_ID,
            fee_recipient: id(8),
            seller_owner: id(9),
            aggregate: id(10),
            position: id(11),
            expected_aggregate: id(10),
            expected_position: id(11),
            seller_token: id(12),
            fee_token: id(13),
            expected_seller_token: id(12),
            expected_fee_token: id(13),
            fee_basis_points: DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1,
        }
    }

    #[test]
    fn semantic_join_refuses_every_foreign_axis_and_alias() {
        let base = semantic_fixture();
        assert!(base.validate().is_ok());
        for hostile in [
            SemanticFixtureV1 {
                market: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                release: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                realm: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                registry: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                config: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                mint: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                token_program: id(90),
                ..base
            },
            SemanticFixtureV1 {
                fee_recipient: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                seller_owner: [0; 32],
                ..base
            },
            SemanticFixtureV1 {
                aggregate: id(91),
                ..base
            },
            SemanticFixtureV1 {
                position: id(92),
                ..base
            },
            SemanticFixtureV1 {
                seller_token: id(93),
                ..base
            },
            SemanticFixtureV1 {
                fee_token: id(94),
                ..base
            },
            SemanticFixtureV1 {
                fee_token: base.seller_token,
                expected_fee_token: base.seller_token,
                ..base
            },
            // OUT OF BAND, not "not fifty". `e7cbedee` gave the venue fee rate
            // one author -- the config record -- and moved this mirror's
            // conjunct from `== 50` to the protocol band, but left this hostile
            // behind at 49, where it asserted a law the route no longer has and
            // had been red on main ever since. The conjunct that exists is the
            // band, so this is the value that exercises it.
            SemanticFixtureV1 {
                fee_basis_points: DIRECT_MAX_FEE_BASIS_POINTS_V1
                    .checked_add(1)
                    .expect("one basis point past the band"),
                ..base
            },
        ] {
            assert_eq!(hostile.validate(), Err(TradingSbfError::Content));
        }
    }

    /// The other half of the same move, and the half a hostile cannot state:
    /// every rate inside the band is admitted, INCLUDING the ones a route that
    /// pinned fifty refused.
    ///
    /// Without this the mirror says only "501 is refused", and a future edit
    /// that quietly restored the point would still pass every case above. The
    /// rates are the ones `e7cbedee` proved on the route itself -- 0, 25, 49,
    /// 50 and 500 -- so the mirror and the route agree on the same list rather
    /// than on a bound each interprets alone.
    #[test]
    fn every_rate_the_band_admits_joins_including_the_ones_fifty_refused() {
        let base = semantic_fixture();
        for fee_basis_points in [
            0,
            25,
            49,
            DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1,
            DIRECT_MAX_FEE_BASIS_POINTS_V1,
        ] {
            assert!(fee_basis_points <= DIRECT_MAX_FEE_BASIS_POINTS_V1);
            assert_eq!(
                SemanticFixtureV1 {
                    fee_basis_points,
                    ..base
                }
                .validate(),
                Ok(()),
                "the config is the sole author of the rate, at {fee_basis_points} basis points",
            );
        }
    }

    #[test]
    fn claims_position_context_is_the_seller_owner_authority() {
        let claim_count = 2;
        let aggregate_width =
            liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, claim_count)
                .expect("aggregate width");
        let position_width =
            liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, claim_count)
                .expect("position width");
        let mut aggregate = vec![0; aggregate_width];
        let mut position = vec![0; position_width];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 1,
                logical_market: id(1),
                release_set: id(2),
                registry_program: id(3),
                product_instance_id: id(4),
                basis_id: id(5),
                realm_id: id(6),
                custody_context: id(7),
                generation: 8,
            },
            &[10, 20],
            &mut aggregate,
        )
        .expect("aggregate");
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision: 9,
                market_account: id(10),
                owner: id(11),
                basis_id: id(5),
            },
            &[3, 4],
            &mut position,
        )
        .expect("position");
        let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate).expect("aggregate view");
        let position = LiabilityBasisPositionViewV2::decode(&position).expect("position view");
        assert_eq!(position.owner, id(11));
        assert_eq!(position.basis_id, aggregate.basis_id);
        assert_eq!(position.claim_count, aggregate.claim_count);
        assert_ne!(position.owner, id(12));
    }

    #[test]
    fn pda_roles_and_frame_order_cannot_be_substituted() {
        let program = Pubkey::new_unique();
        let seller =
            DirectTokenAccountSeedsV1::new(id(1), 2, id(3), DirectTokenAccountRoleV1::Seller)
                .expect("seller seeds");
        let fee = DirectTokenAccountSeedsV1::new(id(1), 2, id(3), DirectTokenAccountRoleV1::Fee)
            .expect("fee seeds");
        assert_ne!(
            Pubkey::find_program_address(&seller.as_slices(), &program).0,
            Pubkey::find_program_address(&fee.as_slices(), &program).0
        );
        let mut frame = [[0; 32]; DIRECT_TOKEN_SETUP_ACCOUNT_COUNT_V1];
        for (index, address) in frame.iter_mut().enumerate() {
            *address = [u8::try_from(index + 1).expect("small index"); 32];
        }
        let expected = direct_token_setup_frame_digest_v1(frame);
        frame.swap(SELLER_TOKEN, FEE_TOKEN);
        assert_ne!(direct_token_setup_frame_digest_v1(frame), expected);
    }

    #[test]
    fn exact_token_poststate_refuses_every_wrong_byte() {
        let expected = TokenAccount::initialized_base_bytes(id(1), id(2)).expect("poststate");
        let parsed = TokenAccount::parse(&expected).expect("token");
        assert_eq!(parsed.mint, id(1));
        assert_eq!(parsed.owner, id(2));
        assert_eq!(parsed.amount, 0);
        for index in [0, 32, 64, 72, 108, 109, 121, 129] {
            let mut hostile = expected;
            hostile[index] ^= 1;
            assert_ne!(hostile, expected);
        }
    }
}
