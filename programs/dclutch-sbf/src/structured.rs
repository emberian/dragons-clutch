//! Atomic SBF boundary for exact transferable structured portfolio receipts.
//!
//! This module deliberately owns no structured supply or holder ledger.  The
//! receipt Mint supply and ordinary Token-2022 Account amount are parsed from
//! hostile bytes, passed to `dclutch-structured-contract`, changed by one real
//! Token-2022 CPI, reloaded, and checked before native Position candidates are
//! persisted.
//!
//! Exact account frames keep all data/authority roles distinct; RedeemTerminal
//! alone permits its receipt and collateral program roles to name the same
//! authenticated Token-2022 executable:
//!
//! * Activate: Market, Descriptor, Manifest, Manifest cursor, Config, Config
//!   cursor, Product Instance, Instance cursor, PortfolioTemplate, Template
//!   cursor, receipt Mint, receipt controller, Token-2022 program, Rent sysvar,
//!   capability Funding, payer, custody Position, System Program.
//! * Wrap/Unwrap: the common first fourteen roles above, then holder, holder
//!   Position, custody Position, and holder receipt Account.
//! * RedeemTerminal: the common first fourteen roles above, then holder,
//!   custody Position, holder receipt Account, Realm, collateral vault, holder
//!   collateral Account, collateral Mint, and collateral token program.
//! * Retire: the common first fourteen roles above, then custody Position and
//!   immutable RentCredit.
//!
//! A PortfolioTemplate and StructuredConfig use finalized raw records whose
//! raw-record PDA is keyed by the ordinary SHA-256 of their canonical bytes.
//! Their semantic IDs remain the required domain-separated hashes.  This is
//! the same separation between record storage identity and Product semantic
//! identity used by the existing record boundary.

use alloc::vec::Vec;

use dclutch_bearer_contract::state::{
    BEARER_MINT_BYTES, BEARER_TOKEN_ACCOUNT_BYTES, MintObservationV1, TokenAccountObservationV1,
    TokenAccountStateV1,
};
use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, FUNDING_STATE_BYTES,
    FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_collateral_contract::COLLATERAL_VAULT_PDA_DOMAIN;
use dclutch_core_contract::ContentId;
use dclutch_core_contract::MarketRoot;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    portfolio::{PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, PortfolioTemplateV1},
    product::InstanceV1,
};
use dclutch_realm_contract::{POSITION_PDA_DOMAIN, PositionV1, REALM_PDA_DOMAIN, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RentCreditV1};
use dclutch_structured_contract::{
    descriptor::{
        ProductBindingV1, STRUCTURED_CONFIG_CONTENT_DOMAIN_V1, STRUCTURED_DESCRIPTOR_BYTES,
        StructuredConfigV1, StructuredContextV1, StructuredDescriptorDerivationV1,
        StructuredDescriptorInputV1, StructuredDescriptorV1, custody_owner_derivation_v1,
        receipt_authority_derivation_v1, receipt_mint_derivation_v1,
        validate_structured_capability_entry_v1,
    },
    instruction::{StructuredActionV1, StructuredInstructionV1},
    transition::{
        ReceiptOperationV1, ReceiptSupplyPlanV1, activate, audit_backing, redeem_terminal, retire,
        unwrap, wrap,
    },
};
use dclutch_token_svm::{
    AccountState, AuthorityRole, COption, CollateralAdapterReleaseV1, ExactTransferInput, Mint,
    TOKEN_2022_PROGRAM_ID, TokenAccount, transfer_checked,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;
use spl_token_2022_interface::extension::ExtensionType;

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    realm::{
        recognized_program_loader, require_authority_policy, require_freeze_policy,
        select_adapter_release,
    },
};

const COMMON_ACCOUNTS: usize = 14;
const ACTIVATE_ACCOUNTS: usize = 18;
const QUANTITY_ACCOUNTS: usize = 18;
const TERMINAL_ACCOUNTS: usize = 22;
const RETIRE_ACCOUNTS: usize = 16;

const MARKET: usize = 0;
const DESCRIPTOR: usize = 1;
const MANIFEST: usize = 2;
const MANIFEST_CURSOR: usize = 3;
const CONFIG: usize = 4;
const CONFIG_CURSOR: usize = 5;
const INSTANCE: usize = 6;
const INSTANCE_CURSOR: usize = 7;
const TEMPLATE: usize = 8;
const TEMPLATE_CURSOR: usize = 9;
const RECEIPT_MINT: usize = 10;
const RECEIPT_AUTHORITY: usize = 11;
const RECEIPT_TOKEN_PROGRAM: usize = 12;
const RENT_SYSVAR: usize = 13;

const ACTIVATE_FUNDING: usize = 14;
const ACTIVATE_PAYER: usize = 15;
const ACTIVATE_CUSTODY: usize = 16;
const ACTIVATE_SYSTEM: usize = 17;

const QUANTITY_HOLDER: usize = 14;
const QUANTITY_OWNER_POSITION: usize = 15;
const QUANTITY_CUSTODY: usize = 16;
const QUANTITY_RECEIPT_ACCOUNT: usize = 17;

const TERMINAL_HOLDER: usize = 14;
const TERMINAL_CUSTODY: usize = 15;
const TERMINAL_RECEIPT_ACCOUNT: usize = 16;
const TERMINAL_REALM: usize = 17;
const TERMINAL_COLLATERAL_VAULT: usize = 18;
const TERMINAL_COLLATERAL_DESTINATION: usize = 19;
const TERMINAL_COLLATERAL_MINT: usize = 20;
const TERMINAL_COLLATERAL_PROGRAM: usize = 21;

const RETIRE_CUSTODY: usize = 14;
const RETIRE_RENT_CREDIT: usize = 15;

/// SHA-256 of `dclutch/schema/structured-config-v1`.
const STRUCTURED_CONFIG_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x90, 0x4e, 0xb9, 0x4e, 0x2d, 0xdd, 0x36, 0x42, 0x4b, 0xd9, 0xc6, 0xbc, 0xd8, 0x9a, 0xba, 0x61,
    0x1a, 0x0f, 0x2b, 0x7b, 0x99, 0x56, 0xf3, 0xfb, 0x7c, 0x65, 0x19, 0x2d, 0xf1, 0x55, 0xdc, 0x0e,
];

/// SHA-256 of `dclutch/schema/product-portfolio-template-v1`.
const PORTFOLIO_TEMPLATE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x46, 0xcf, 0x1f, 0xb2, 0xb5, 0x1d, 0x5d, 0x75, 0x50, 0xc2, 0x2a, 0xc6, 0x20, 0xdb, 0xad, 0x14,
    0x45, 0xb2, 0x75, 0xbb, 0xcb, 0x37, 0x74, 0xd5, 0x3e, 0x7f, 0x7d, 0x4d, 0xb8, 0x84, 0xa4, 0xf2,
];

struct RecordSnapshots {
    manifest: Vec<u8>,
    config: Vec<u8>,
    instance: Vec<u8>,
    template: Vec<u8>,
}

struct ExistingContext<const N: usize> {
    market: CategoricalMarketV1<N>,
    descriptor: StructuredDescriptorV1,
    context: StructuredContextV1<N>,
}

#[derive(Clone, Copy)]
struct RealmFacts {
    realm: RealmV1,
    release: CollateralAdapterReleaseV1,
    mint: Mint,
}

#[derive(Clone, Copy)]
struct CollateralTransferFacts {
    source: TokenAccount,
    destination: TokenAccount,
    source_lamports: u64,
    destination_lamports: u64,
    mint_lamports: u64,
}

/// Decode and route one exact Structured V1 instruction.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let instruction = StructuredInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?;
    let market = account(accounts, MARKET)?;
    let market_data = market
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let width = decode_market_outcome_count(&market_data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    drop(market_data);
    if width != instruction.outcome_count() {
        return Err(AdapterError::ReplayMismatch.into());
    }
    macro_rules! route {
        ($n:literal) => {
            process::<$n>(program_id, accounts, instruction)
        };
    }
    match width {
        2 => route!(2),
        3 => route!(3),
        4 => route!(4),
        5 => route!(5),
        6 => route!(6),
        7 => route!(7),
        8 => route!(8),
        9 => route!(9),
        10 => route!(10),
        11 => route!(11),
        12 => route!(12),
        13 => route!(13),
        14 => route!(14),
        15 => route!(15),
        16 => route!(16),
        _ => Err(AdapterError::BearerAuthentication.into()),
    }
}

fn process<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: StructuredInstructionV1,
) -> Result<(), ProgramError> {
    validate_frame(accounts, instruction.action())?;
    match instruction.action() {
        StructuredActionV1::Activate => activate_receipt::<N>(
            program_id,
            accounts,
            instruction.generation(),
            instruction.value(),
        ),
        StructuredActionV1::Wrap => quantity_transition::<N>(
            program_id,
            accounts,
            instruction.generation(),
            instruction.value(),
            ReceiptOperationV1::Mint,
        ),
        StructuredActionV1::Unwrap => quantity_transition::<N>(
            program_id,
            accounts,
            instruction.generation(),
            instruction.value(),
            ReceiptOperationV1::Burn,
        ),
        StructuredActionV1::RedeemTerminal => terminal_redeem_receipt::<N>(
            program_id,
            accounts,
            instruction.generation(),
            instruction.value(),
        ),
        StructuredActionV1::Retire => retire_receipt::<N>(
            program_id,
            accounts,
            instruction.generation(),
            instruction.value(),
        ),
    }
}

fn activate_receipt<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    expected_prior_child_count: u64,
) -> Result<(), ProgramError> {
    let market_account = account(accounts, MARKET)?;
    let descriptor_account = account(accounts, DESCRIPTOR)?;
    let mint_account = account(accounts, RECEIPT_MINT)?;
    let controller_account = account(accounts, RECEIPT_AUTHORITY)?;
    let token_program = account(accounts, RECEIPT_TOKEN_PROGRAM)?;
    let rent_sysvar = account(accounts, RENT_SYSVAR)?;
    let funding_account = account(accounts, ACTIVATE_FUNDING)?;
    let payer = account(accounts, ACTIVATE_PAYER)?;
    let custody_account = account(accounts, ACTIVATE_CUSTODY)?;
    let system = account(accounts, ACTIVATE_SYSTEM)?;

    authenticate_fixed_programs(token_program, rent_sysvar, Some(system))?;
    if !payer.is_signer
        || payer.owner != &system_program::ID
        || payer.executable
        || !payer.data_is_empty()
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    for vacant in [descriptor_account, mint_account, custody_account] {
        require_vacant(vacant)?;
    }
    require_ephemeral_controller(controller_account)?;

    let mut market = authenticate_market::<N>(program_id, market_account, generation)?;
    let records = load_records(program_id, accounts, rent_sysvar, &market)?;
    let manifest = CapabilityManifestV1::decode(&records.manifest)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let manifest_id = content_id(&records.manifest)?;
    let config = StructuredConfigV1::decode(&records.config)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let config_id = domain_content_id(STRUCTURED_CONFIG_CONTENT_DOMAIN_V1, &records.config)?;
    let instance =
        InstanceV1::decode(&records.instance).map_err(|_| AdapterError::BearerAuthentication)?;
    let template = PortfolioTemplateV1::<N>::decode(&records.template)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let template_id = domain_content_id(PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, &records.template)?;

    let funding = authenticate_funding(
        program_id,
        funding_account,
        market_account,
        generation,
        manifest_id,
        manifest,
    )?;
    let entry = manifest
        .entry(funding.entry_index())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    validate_structured_capability_entry_v1(entry, config_id)
        .map_err(|_| AdapterError::BearerAuthentication)?;

    let generation_le = generation.to_le_bytes();
    let descriptor_seeds = [
        dclutch_structured_contract::descriptor::STRUCTURED_DESCRIPTOR_PDA_DOMAIN_V1,
        market_account.key.as_ref(),
        generation_le.as_slice(),
        template_id.as_slice(),
        config_id.as_slice(),
        dclutch_structured_contract::descriptor::STRUCTURED_SEMANTIC_RELEASE_ID_V1.as_slice(),
    ];
    let (expected_descriptor, descriptor_bump) =
        Pubkey::find_program_address(&descriptor_seeds, program_id);
    if descriptor_account.key != &expected_descriptor {
        return Err(AdapterError::AccountIdentity.into());
    }
    let (expected_mint, mint_bump) = Pubkey::find_program_address(
        &[
            dclutch_structured_contract::descriptor::STRUCTURED_RECEIPT_MINT_PDA_DOMAIN_V1,
            descriptor_account.key.as_ref(),
        ],
        program_id,
    );
    let (expected_controller, _) = Pubkey::find_program_address(
        &[
            dclutch_structured_contract::descriptor::STRUCTURED_RECEIPT_AUTHORITY_PDA_DOMAIN_V1,
            descriptor_account.key.as_ref(),
        ],
        program_id,
    );
    let (custody_owner, _) = Pubkey::find_program_address(
        &[
            dclutch_structured_contract::descriptor::STRUCTURED_CUSTODY_OWNER_PDA_DOMAIN_V1,
            descriptor_account.key.as_ref(),
        ],
        program_id,
    );
    let (expected_custody, custody_bump) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            market_account.key.as_ref(),
            custody_owner.as_ref(),
        ],
        program_id,
    );
    if mint_account.key != &expected_mint
        || controller_account.key != &expected_controller
        || custody_account.key != &expected_custody
    {
        return Err(AdapterError::AccountIdentity.into());
    }

    let descriptor = StructuredDescriptorV1::new::<N>(StructuredDescriptorInputV1 {
        market: market_account.key.to_bytes(),
        generation,
        manifest_entry_index: funding.entry_index(),
        portfolio_template_id: template_id,
        capability_config_id: config_id,
        capability_release_id:
            dclutch_structured_contract::descriptor::STRUCTURED_SEMANTIC_RELEASE_ID_V1,
        receipt_adapter_release_id: config.receipt_adapter_release_id(),
        receipt_mint: mint_account.key.to_bytes(),
        receipt_authority: controller_account.key.to_bytes(),
        custody_position: custody_account.key.to_bytes(),
        custody_owner: custody_owner.to_bytes(),
        rent_credit: config.rent_credit(),
    })
    .map_err(|_| AdapterError::BearerAuthentication)?;
    let product = ProductBindingV1::new(
        ProductContentId::new(market.root().identity().product_instance_id().to_bytes())
            .map_err(|_| AdapterError::BearerAuthentication)?,
        instance,
        ProductContentId::new(template_id).map_err(|_| AdapterError::BearerAuthentication)?,
        template,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    let context = StructuredContextV1::new(
        descriptor_account.key.to_bytes(),
        descriptor,
        market_account.key.to_bytes(),
        &market,
        product,
        config_id,
        config,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    let plan = activate(
        context,
        market_account.key.to_bytes(),
        &mut market,
        expected_prior_child_count,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    if plan.descriptor() != descriptor
        || plan.receipt_mint().mint() != mint_account.key.to_bytes()
        || plan.receipt_mint().controller() != controller_account.key.to_bytes()
        || plan.custody_position_key() != custody_account.key.to_bytes()
    {
        return Err(AdapterError::BearerPostcondition.into());
    }

    let rent =
        Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::BearerAuthentication)?;
    let descriptor_rent = rent.minimum_balance(STRUCTURED_DESCRIPTOR_BYTES);
    let mint_rent = rent.minimum_balance(BEARER_MINT_BYTES);
    let position_len = PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    let position_rent = rent.minimum_balance(position_len);
    let creation_rent = mint_rent
        .checked_add(position_rent)
        .ok_or(AdapterError::Arithmetic)?;
    let total_debit = descriptor_rent
        .checked_add(creation_rent)
        .ok_or(AdapterError::Arithmetic)?;
    let funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
    let custody =
        FundingCustodyObservationV1::native_only(funding_account.lamports(), funding_rent)
            .map_err(|_| AdapterError::BearerAuthentication)?;
    let mut funding_after = funding;
    let debit = funding_after
        .activate(
            manifest_id,
            manifest,
            custody,
            solana_program::clock::Clock::get()
                .map_err(|_| AdapterError::BearerAuthentication)?
                .slot,
        )
        .map_err(|_| AdapterError::BearerTransition)?;
    if debit.rent_lamports() != descriptor_rent
        || debit.creation_lamports() != creation_rent
        || funding_after.remaining().native_lamports_total() != 0
        || funding_after.remaining().realm_collateral_total() != 0
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let payer_before = payer.lamports();
    payer_before
        .checked_sub(total_debit)
        .ok_or(AdapterError::BearerCreateCpi)?;

    create_pda_account(
        payer,
        descriptor_account,
        system,
        descriptor_rent,
        STRUCTURED_DESCRIPTOR_BYTES,
        program_id,
        &descriptor_seeds,
        descriptor_bump,
    )?;
    let mint_seeds = [
        dclutch_structured_contract::descriptor::STRUCTURED_RECEIPT_MINT_PDA_DOMAIN_V1,
        descriptor_account.key.as_ref(),
    ];
    create_pda_account(
        payer,
        mint_account,
        system,
        mint_rent,
        BEARER_MINT_BYTES,
        token_program.key,
        &mint_seeds,
        mint_bump,
    )?;
    let custody_seeds = [
        POSITION_PDA_DOMAIN,
        market_account.key.as_ref(),
        custody_owner.as_ref(),
    ];
    create_pda_account(
        payer,
        custody_account,
        system,
        position_rent,
        position_len,
        program_id,
        &custody_seeds,
        custody_bump,
    )?;
    initialize_receipt_mint(mint_account, controller_account, token_program)?;
    let observed = parse_receipt_mint(mint_account, token_program)?;
    observed
        .validate_profile(
            mint_account.key.to_bytes(),
            controller_account.key.to_bytes(),
        )
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if observed.supply != 0 {
        return Err(AdapterError::BearerPostcondition.into());
    }

    move_lamports(funding_account, payer, total_debit)?;
    persist_market(market_account, market)?;
    persist_funding(funding_account, funding_after)?;
    persist_descriptor(descriptor_account, descriptor)?;
    persist_position(custody_account, plan.custody_position())?;
    if payer.lamports() != payer_before || funding_account.lamports() != funding_rent {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn quantity_transition<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    units: u64,
    operation: ReceiptOperationV1,
) -> Result<(), ProgramError> {
    let holder = account(accounts, QUANTITY_HOLDER)?;
    if !holder.is_signer || holder.executable {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let token_program = account(accounts, RECEIPT_TOKEN_PROGRAM)?;
    let rent_sysvar = account(accounts, RENT_SYSVAR)?;
    authenticate_fixed_programs(token_program, rent_sysvar, None)?;
    let loaded = authenticate_existing_context::<N>(program_id, accounts, generation)?;
    let owner_account = account(accounts, QUANTITY_OWNER_POSITION)?;
    let custody_account = account(accounts, QUANTITY_CUSTODY)?;
    let mint_account = account(accounts, RECEIPT_MINT)?;
    let controller = account(accounts, RECEIPT_AUTHORITY)?;
    let receipt_account = account(accounts, QUANTITY_RECEIPT_ACCOUNT)?;
    let mut owner_position = authenticate_position::<N>(
        program_id,
        owner_account,
        account(accounts, MARKET)?,
        holder.key,
        generation,
    )?;
    let mut custody_position = authenticate_position_for_owner::<N>(
        program_id,
        custody_account,
        account(accounts, MARKET)?,
        &Pubkey::new_from_array(loaded.descriptor.custody_owner()),
        generation,
    )?;
    let mint_before = parse_receipt_mint(mint_account, token_program)?;
    let token_before = parse_receipt_account(receipt_account, token_program)?;
    let plan = match operation {
        ReceiptOperationV1::Mint => wrap(
            loaded.context,
            account(accounts, MARKET)?.key.to_bytes(),
            &loaded.market,
            holder.key.to_bytes(),
            &mut owner_position,
            custody_account.key.to_bytes(),
            &mut custody_position,
            mint_before,
            token_before,
            units,
        ),
        ReceiptOperationV1::Burn => unwrap(
            loaded.context,
            account(accounts, MARKET)?.key.to_bytes(),
            &loaded.market,
            holder.key.to_bytes(),
            &mut owner_position,
            custody_account.key.to_bytes(),
            &mut custody_position,
            mint_before,
            token_before,
            units,
        ),
    }
    .map_err(|_| AdapterError::BearerTransition)?;
    if plan.receipt().operation() != operation {
        return Err(AdapterError::BearerPostcondition.into());
    }
    execute_receipt_plan(
        program_id,
        account(accounts, DESCRIPTOR)?,
        mint_account,
        receipt_account,
        controller,
        holder,
        token_program,
        plan.receipt(),
        mint_before,
        token_before,
    )?;
    let mint_after = parse_receipt_mint(mint_account, token_program)?;
    audit_backing(
        loaded.context,
        account(accounts, MARKET)?.key.to_bytes(),
        &loaded.market,
        custody_account.key.to_bytes(),
        &custody_position,
        mint_after,
    )
    .map_err(|_| AdapterError::BearerPostcondition)?;
    persist_position(owner_account, owner_position)?;
    persist_position(custody_account, custody_position)?;
    Ok(())
}

fn terminal_redeem_receipt<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    units: u64,
) -> Result<(), ProgramError> {
    let holder = account(accounts, TERMINAL_HOLDER)?;
    let receipt_program = account(accounts, RECEIPT_TOKEN_PROGRAM)?;
    let rent_sysvar = account(accounts, RENT_SYSVAR)?;
    authenticate_fixed_programs(receipt_program, rent_sysvar, None)?;
    let mut loaded = authenticate_existing_context::<N>(program_id, accounts, generation)?;
    let market_account = account(accounts, MARKET)?;
    let descriptor_account = account(accounts, DESCRIPTOR)?;
    let custody_account = account(accounts, TERMINAL_CUSTODY)?;
    let receipt_mint_account = account(accounts, RECEIPT_MINT)?;
    let receipt_controller = account(accounts, RECEIPT_AUTHORITY)?;
    let receipt_account = account(accounts, TERMINAL_RECEIPT_ACCOUNT)?;
    let realm_account = account(accounts, TERMINAL_REALM)?;
    let collateral_vault = account(accounts, TERMINAL_COLLATERAL_VAULT)?;
    let collateral_destination = account(accounts, TERMINAL_COLLATERAL_DESTINATION)?;
    let collateral_mint = account(accounts, TERMINAL_COLLATERAL_MINT)?;
    let collateral_program = account(accounts, TERMINAL_COLLATERAL_PROGRAM)?;

    let realm = authenticate_realm(
        program_id,
        realm_account,
        collateral_mint,
        collateral_program,
        loaded.market.root(),
    )?;
    let mut custody = authenticate_position_for_owner::<N>(
        program_id,
        custody_account,
        market_account,
        &Pubkey::new_from_array(loaded.descriptor.custody_owner()),
        generation,
    )?;
    let receipt_mint_before = parse_receipt_mint(receipt_mint_account, receipt_program)?;
    let receipt_account_before = parse_receipt_account(receipt_account, receipt_program)?;
    let vault_before = authenticate_collateral_vault(
        program_id,
        market_account,
        collateral_vault,
        collateral_mint,
        collateral_program,
        realm,
        loaded.market.hoard_atoms(),
    )?;
    let destination_before = authenticate_holder_collateral_account(
        collateral_destination,
        collateral_program,
        realm,
        holder.key,
    )?;
    let hoard_before = loaded.market.hoard_atoms();
    let plan = redeem_terminal(
        loaded.context,
        market_account.key.to_bytes(),
        &mut loaded.market,
        holder.key.to_bytes(),
        custody_account.key.to_bytes(),
        &mut custody,
        receipt_mint_before,
        receipt_account_before,
        units,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    validate_terminal_economic_delta(
        plan.receipt().operation(),
        plan.collateral_payout_atoms(),
        hoard_before,
        loaded.market.hoard_atoms(),
    )?;
    let collateral_transfer = authenticate_collateral_transfer(
        collateral_vault,
        collateral_destination,
        collateral_mint,
        collateral_program,
        realm,
        market_account.key,
        plan.collateral_payout_atoms(),
        vault_before,
        destination_before,
    )?;
    preflight_terminal_mutable(&[
        market_account,
        custody_account,
        receipt_mint_account,
        receipt_account,
        collateral_vault,
        collateral_destination,
    ])?;

    // Both external effects precede program-account persistence. A failure in
    // the later collateral CPI returns an error after the receipt burn; SVM
    // transaction rollback is therefore the authority that restores the burn.
    execute_receipt_plan(
        program_id,
        descriptor_account,
        receipt_mint_account,
        receipt_account,
        receipt_controller,
        holder,
        receipt_program,
        plan.receipt(),
        receipt_mint_before,
        receipt_account_before,
    )?;
    execute_collateral_payout(
        program_id,
        market_account,
        collateral_vault,
        collateral_destination,
        collateral_mint,
        collateral_program,
        realm,
        collateral_transfer,
        plan.collateral_payout_atoms(),
        loaded.market.root(),
    )?;

    let receipt_mint_after = parse_receipt_mint(receipt_mint_account, receipt_program)?;
    audit_backing(
        loaded.context,
        market_account.key.to_bytes(),
        &loaded.market,
        custody_account.key.to_bytes(),
        &custody,
        receipt_mint_after,
    )
    .map_err(|_| AdapterError::BearerPostcondition)?;
    persist_market(market_account, loaded.market)?;
    persist_position(custody_account, custody)?;
    Ok(())
}

fn retire_receipt<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
    expected_prior_child_count: u64,
) -> Result<(), ProgramError> {
    let token_program = account(accounts, RECEIPT_TOKEN_PROGRAM)?;
    let rent_sysvar = account(accounts, RENT_SYSVAR)?;
    authenticate_fixed_programs(token_program, rent_sysvar, None)?;
    let mut loaded = authenticate_existing_context::<N>(program_id, accounts, generation)?;
    let market_account = account(accounts, MARKET)?;
    let descriptor_account = account(accounts, DESCRIPTOR)?;
    let custody_account = account(accounts, RETIRE_CUSTODY)?;
    let mint_account = account(accounts, RECEIPT_MINT)?;
    let controller = account(accounts, RECEIPT_AUTHORITY)?;
    let rent_credit_account = account(accounts, RETIRE_RENT_CREDIT)?;
    let custody = authenticate_position_for_owner::<N>(
        program_id,
        custody_account,
        market_account,
        &Pubkey::new_from_array(loaded.descriptor.custody_owner()),
        generation,
    )?;
    let mint = parse_receipt_mint(mint_account, token_program)?;
    let plan = retire(
        loaded.context,
        market_account.key.to_bytes(),
        &mut loaded.market,
        custody_account.key.to_bytes(),
        &custody,
        mint,
        expected_prior_child_count,
    )
    .map_err(|_| AdapterError::BearerTransition)?;
    if plan.descriptor_key() != descriptor_account.key.to_bytes()
        || plan.receipt_mint() != mint_account.key.to_bytes()
        || plan.custody_position() != custody_account.key.to_bytes()
        || plan.rent_credit() != rent_credit_account.key.to_bytes()
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    let rent_credit = authenticate_named_rent_credit(
        program_id,
        rent_credit_account,
        loaded.descriptor.rent_credit(),
    )?;
    let mint_lamports = mint_account.lamports();
    let descriptor_lamports = descriptor_account.lamports();
    let custody_lamports = custody_account.lamports();
    let credit_before = rent_credit_account.lamports();
    let expected_credit = credit_before
        .checked_add(mint_lamports)
        .and_then(|value| value.checked_add(descriptor_lamports))
        .and_then(|value| value.checked_add(custody_lamports))
        .ok_or(AdapterError::Arithmetic)?;

    let close = spl_token_2022_interface::instruction::close_account(
        token_program.key,
        mint_account.key,
        rent_credit_account.key,
        controller.key,
        &[],
    )?;
    invoke_controller_signed(
        program_id,
        descriptor_account,
        &close,
        &[
            mint_account.clone(),
            rent_credit_account.clone(),
            controller.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| AdapterError::BearerTokenCpi)?;
    if mint_account.lamports() != 0 || !mint_account.data_is_empty() {
        return Err(AdapterError::BearerClose.into());
    }
    persist_market(market_account, loaded.market)?;
    close_program_account(descriptor_account, rent_credit_account)?;
    close_program_account(custody_account, rent_credit_account)?;
    require_unchanged_rent_credit(program_id, rent_credit_account, rent_credit)?;
    if rent_credit_account.lamports() != expected_credit
        || plan.market_child_count_after().checked_add(1) != Some(plan.market_child_count_before())
    {
        return Err(AdapterError::BearerClose.into());
    }
    Ok(())
}

fn authenticate_existing_context<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    generation: u64,
) -> Result<ExistingContext<N>, ProgramError> {
    let market_account = account(accounts, MARKET)?;
    let descriptor_account = account(accounts, DESCRIPTOR)?;
    let market = authenticate_market::<N>(program_id, market_account, generation)?;
    let descriptor = decode_descriptor(program_id, descriptor_account)?;
    if descriptor.generation() != generation
        || descriptor.market() != market_account.key.to_bytes()
        || usize::from(descriptor.outcome_count()) != N
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    authenticate_descriptor_children(program_id, accounts, descriptor)?;
    let records = load_records(
        program_id,
        accounts,
        account(accounts, RENT_SYSVAR)?,
        &market,
    )?;
    let manifest = CapabilityManifestV1::decode(&records.manifest)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let config = StructuredConfigV1::decode(&records.config)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let config_id = domain_content_id(STRUCTURED_CONFIG_CONTENT_DOMAIN_V1, &records.config)?;
    let entry = manifest
        .entry(descriptor.manifest_entry_index())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    validate_structured_capability_entry_v1(entry, config_id)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let instance =
        InstanceV1::decode(&records.instance).map_err(|_| AdapterError::BearerAuthentication)?;
    let template = PortfolioTemplateV1::<N>::decode(&records.template)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let template_id = domain_content_id(PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, &records.template)?;
    let product = ProductBindingV1::new(
        ProductContentId::new(market.root().identity().product_instance_id().to_bytes())
            .map_err(|_| AdapterError::BearerAuthentication)?,
        instance,
        ProductContentId::new(template_id).map_err(|_| AdapterError::BearerAuthentication)?,
        template,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    let context = StructuredContextV1::new(
        descriptor_account.key.to_bytes(),
        descriptor,
        market_account.key.to_bytes(),
        &market,
        product,
        config_id,
        config,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    Ok(ExistingContext {
        market,
        descriptor,
        context,
    })
}

fn load_records<const N: usize>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    rent_sysvar: &AccountInfo<'_>,
    market: &CategoricalMarketV1<N>,
) -> Result<RecordSnapshots, ProgramError> {
    let rent =
        Rent::from_account_info(rent_sysvar).map_err(|_| AdapterError::BearerAuthentication)?;
    let manifest = finalized_record(
        program_id,
        account(accounts, MANIFEST)?,
        account(accounts, MANIFEST_CURSOR)?,
        crate::records::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        Some(market.root().identity().capability_manifest_id().to_bytes()),
        &rent,
    )?;
    CapabilityManifestV1::decode(&manifest).map_err(|_| AdapterError::BearerAuthentication)?;
    let config = finalized_record(
        program_id,
        account(accounts, CONFIG)?,
        account(accounts, CONFIG_CURSOR)?,
        STRUCTURED_CONFIG_SCHEMA_RELEASE_ID_V1,
        None,
        &rent,
    )?;
    StructuredConfigV1::decode(&config).map_err(|_| AdapterError::BearerAuthentication)?;
    let instance = finalized_record(
        program_id,
        account(accounts, INSTANCE)?,
        account(accounts, INSTANCE_CURSOR)?,
        crate::records::PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        Some(market.root().identity().product_instance_id().to_bytes()),
        &rent,
    )?;
    InstanceV1::decode(&instance).map_err(|_| AdapterError::BearerAuthentication)?;
    let template = finalized_record(
        program_id,
        account(accounts, TEMPLATE)?,
        account(accounts, TEMPLATE_CURSOR)?,
        PORTFOLIO_TEMPLATE_SCHEMA_RELEASE_ID_V1,
        None,
        &rent,
    )?;
    PortfolioTemplateV1::<N>::decode(&template).map_err(|_| AdapterError::BearerAuthentication)?;
    Ok(RecordSnapshots {
        manifest,
        config,
        instance,
        template,
    })
}

fn finalized_record(
    program_id: &Pubkey,
    raw: &AccountInfo<'_>,
    cursor: &AccountInfo<'_>,
    schema: [u8; 32],
    expected_digest: Option<[u8; 32]>,
    rent: &Rent,
) -> Result<Vec<u8>, ProgramError> {
    if raw.owner != program_id
        || raw.executable
        || raw.is_signer
        || raw.is_writable
        || cursor.owner != &system_program::ID
        || cursor.executable
        || cursor.is_signer
        || cursor.is_writable
        || !cursor.data_is_empty()
        || raw.lamports() < rent.minimum_balance(raw.data_len())
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let digest = hash(&data).to_bytes();
    if expected_digest.is_some_and(|expected| expected != digest) {
        return Err(AdapterError::ContentIdentity.into());
    }
    let (expected_raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        program_id,
    );
    let (expected_cursor, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        program_id,
    );
    if raw.key != &expected_raw || cursor.key != &expected_cursor {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(Vec::from(data.as_ref()))
}

fn authenticate_market<const N: usize>(
    program_id: &Pubkey,
    market_account: &AccountInfo<'_>,
    generation: u64,
) -> Result<CategoricalMarketV1<N>, ProgramError> {
    if market_account.owner != program_id || market_account.executable {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = market_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let market =
        CategoricalMarketV1::<N>::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if market.root().identity().generation() != generation {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let identity_digest = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) =
        Pubkey::find_program_address(&[MARKET_SEED, identity_digest.as_slice()], program_id);
    if market_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    let canonical = encode_market(market)?;
    if canonical.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(market)
}

fn authenticate_realm(
    program_id: &Pubkey,
    realm_account: &AccountInfo<'_>,
    mint_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    root: MarketRoot,
) -> Result<RealmFacts, ProgramError> {
    if realm_account.owner != program_id
        || realm_account.executable
        || mint_account.owner != token_program.key
        || !token_program.executable
        || !recognized_program_loader(token_program.owner)
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = realm_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let realm = RealmV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if realm.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let realm_digest = hash(&data).to_bytes();
    let (expected_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, realm_digest.as_slice()], program_id);
    if root.identity().realm_id().to_bytes() != realm_digest
        || realm_account.key != &expected_realm
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint_account.key.as_ref()
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if release.token_program() != token_program.key.to_bytes() {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let mint = release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint.mint_authority)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    require_freeze_policy(realm.freeze_authority_policy(), &mint.freeze_authority)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    Ok(RealmFacts {
        realm,
        release,
        mint,
    })
}

fn authenticate_collateral_vault(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    vault: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    hoard_atoms: u64,
) -> Result<TokenAccount, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, market.key.as_ref()],
        program_id,
    );
    if vault.key != &expected || vault.owner != token_program.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = vault
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let account = realm
        .release
        .profile()
        .check_custody_account(
            token_program.key.to_bytes(),
            &data,
            mint.key.to_bytes(),
            market.key.to_bytes(),
        )
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if account.amount < hoard_atoms {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(account)
}

fn authenticate_holder_collateral_account(
    destination: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    holder: &Pubkey,
) -> Result<TokenAccount, ProgramError> {
    if destination.owner != token_program.key {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let account = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &data)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if account.mint != *realm.realm.collateral_mint() || account.owner != holder.to_bytes() {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(account)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_collateral_transfer(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    authority: &Pubkey,
    amount: u64,
    expected_source: TokenAccount,
    expected_destination: TokenAccount,
) -> Result<CollateralTransferFacts, ProgramError> {
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let transfer = realm
        .release
        .profile()
        .check_transfer(ExactTransferInput {
            program_id: token_program.key.to_bytes(),
            mint_address: mint.key.to_bytes(),
            mint_data: &mint_data,
            source_data: &source_data,
            destination_data: &destination_data,
            authority: authority.to_bytes(),
            amount,
            decimals: realm.mint.decimals,
        })
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if transfer.mint() != realm.mint
        || transfer.source() != expected_source
        || transfer.destination() != expected_destination
        || transfer.authority_role() != AuthorityRole::Owner
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    Ok(CollateralTransferFacts {
        source: transfer.source(),
        destination: transfer.destination(),
        source_lamports: source
            .try_lamports()
            .map_err(|_| AdapterError::BearerAuthentication)?,
        destination_lamports: destination
            .try_lamports()
            .map_err(|_| AdapterError::BearerAuthentication)?,
        mint_lamports: mint
            .try_lamports()
            .map_err(|_| AdapterError::BearerAuthentication)?,
    })
}

fn validate_terminal_economic_delta(
    operation: ReceiptOperationV1,
    payout: u64,
    hoard_before: u64,
    hoard_after: u64,
) -> Result<(), ProgramError> {
    if operation != ReceiptOperationV1::Burn
        || hoard_before.checked_sub(hoard_after) != Some(payout)
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_collateral_payout<'a>(
    program_id: &Pubkey,
    market: &AccountInfo<'a>,
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    realm: RealmFacts,
    before: CollateralTransferFacts,
    amount: u64,
    root: MarketRoot,
) -> Result<(), ProgramError> {
    if amount != 0 {
        let instruction = checked_collateral_transfer_instruction(
            realm.release,
            source.key,
            mint.key,
            destination.key,
            market.key,
            amount,
            realm.mint.decimals,
        )?;
        let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
        let (expected_market, bump) =
            Pubkey::find_program_address(&[MARKET_SEED, identity_digest.as_slice()], program_id);
        if market.key != &expected_market {
            return Err(AdapterError::AccountIdentity.into());
        }
        let bump_seed = [bump];
        invoke_signed(
            &instruction,
            &[
                source.clone(),
                mint.clone(),
                destination.clone(),
                market.clone(),
                token_program.clone(),
            ],
            &[&[
                MARKET_SEED,
                identity_digest.as_slice(),
                bump_seed.as_slice(),
            ]],
        )
        .map_err(|_| AdapterError::CollateralTransferCpi)?;
    }
    authenticate_collateral_transfer_post(
        source,
        destination,
        mint,
        token_program,
        realm,
        before,
        amount,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_collateral_transfer_post(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    realm: RealmFacts,
    before: CollateralTransferFacts,
    amount: u64,
) -> Result<(), ProgramError> {
    if source.owner != token_program.key
        || destination.owner != token_program.key
        || mint.owner != token_program.key
        || source.lamports() != before.source_lamports
        || destination.lamports() != before.destination_lamports
        || mint.lamports() != before.mint_lamports
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    let source_data = source
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    let mint_after = realm
        .release
        .profile()
        .check_mint(token_program.key.to_bytes(), &mint_data)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    let source_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &source_data)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    let destination_after = realm
        .release
        .profile()
        .check_transfer_account(token_program.key.to_bytes(), &destination_data)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    let mut expected_source = before.source;
    expected_source.amount = expected_source
        .amount
        .checked_sub(amount)
        .ok_or(AdapterError::BearerPostcondition)?;
    let mut expected_destination = before.destination;
    expected_destination.amount = expected_destination
        .amount
        .checked_add(amount)
        .ok_or(AdapterError::BearerPostcondition)?;
    if mint_after != realm.mint
        || source_after != expected_source
        || destination_after != expected_destination
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn checked_collateral_transfer_instruction(
    release: CollateralAdapterReleaseV1,
    source: &Pubkey,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Result<Instruction, ProgramError> {
    let spec = transfer_checked(
        release.token_program(),
        source.to_bytes(),
        mint.to_bytes(),
        destination.to_bytes(),
        authority.to_bytes(),
        amount,
        decimals,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    if spec.program_id() != &release.token_program() {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let expected = [
        (source.to_bytes(), false, true),
        (mint.to_bytes(), false, false),
        (destination.to_bytes(), false, true),
        (authority.to_bytes(), true, false),
    ];
    for (actual, (address, signer, writable)) in spec.accounts().iter().zip(expected) {
        if actual.address() != &address
            || actual.is_signer() != signer
            || actual.is_writable() != writable
        {
            return Err(AdapterError::BearerAuthentication.into());
        }
    }
    Ok(Instruction {
        program_id: Pubkey::new_from_array(release.token_program()),
        accounts: Vec::from([
            AccountMeta::new(*source, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(*destination, false),
            AccountMeta::new_readonly(*authority, true),
        ]),
        data: Vec::from(*spec.data()),
    })
}

fn preflight_terminal_mutable(accounts: &[&AccountInfo<'_>]) -> Result<(), ProgramError> {
    for account in accounts {
        drop(
            account
                .try_borrow_mut_lamports()
                .map_err(|_| AdapterError::BearerAuthentication)?,
        );
        drop(
            account
                .try_borrow_mut_data()
                .map_err(|_| AdapterError::BearerAuthentication)?,
        );
    }
    Ok(())
}

fn decode_descriptor(
    program_id: &Pubkey,
    descriptor_account: &AccountInfo<'_>,
) -> Result<StructuredDescriptorV1, ProgramError> {
    if descriptor_account.owner != program_id
        || descriptor_account.executable
        || descriptor_account.data_len() != STRUCTURED_DESCRIPTOR_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = descriptor_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let descriptor =
        StructuredDescriptorV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if descriptor.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let derivation = StructuredDescriptorDerivationV1::new(descriptor)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let (expected, _) = Pubkey::find_program_address(&derivation.seeds(), program_id);
    if descriptor_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(descriptor)
}

fn authenticate_descriptor_children(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    descriptor: StructuredDescriptorV1,
) -> Result<(), ProgramError> {
    let descriptor_key = account(accounts, DESCRIPTOR)?.key.to_bytes();
    let mint = receipt_mint_derivation_v1(descriptor_key)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let authority = receipt_authority_derivation_v1(descriptor_key)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let owner = custody_owner_derivation_v1(descriptor_key)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let (expected_mint, _) = Pubkey::find_program_address(&mint.seeds(), program_id);
    let (expected_authority, _) = Pubkey::find_program_address(&authority.seeds(), program_id);
    let (expected_owner, _) = Pubkey::find_program_address(&owner.seeds(), program_id);
    if expected_mint.to_bytes() != descriptor.receipt_mint()
        || expected_authority.to_bytes() != descriptor.receipt_authority()
        || expected_owner.to_bytes() != descriptor.custody_owner()
        || account(accounts, RECEIPT_MINT)?.key != &expected_mint
        || account(accounts, RECEIPT_AUTHORITY)?.key != &expected_authority
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    require_ephemeral_controller(account(accounts, RECEIPT_AUTHORITY)?)?;
    let (expected_custody, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            descriptor.market().as_slice(),
            descriptor.custody_owner().as_slice(),
        ],
        program_id,
    );
    if expected_custody.to_bytes() != descriptor.custody_position() {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn authenticate_funding(
    program_id: &Pubkey,
    funding_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    generation: u64,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
) -> Result<FundingStateV1, ProgramError> {
    if funding_account.owner != program_id
        || funding_account.executable
        || funding_account.data_len() != FUNDING_STATE_BYTES
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = funding_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let funding = FundingStateV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if funding.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let derivation = CapabilityFundingDerivationV1::new(
        market_account.key.to_bytes(),
        generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| AdapterError::BearerAuthentication)?;
    let (expected, _) = Pubkey::find_program_address(&derivation.seed_components(), program_id);
    if funding_account.key != &expected {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(funding)
}

fn authenticate_position<const N: usize>(
    program_id: &Pubkey,
    position_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    owner: &Pubkey,
    generation: u64,
) -> Result<PositionV1<N>, ProgramError> {
    authenticate_position_for_owner(
        program_id,
        position_account,
        market_account,
        owner,
        generation,
    )
}

fn authenticate_position_for_owner<const N: usize>(
    program_id: &Pubkey,
    position_account: &AccountInfo<'_>,
    market_account: &AccountInfo<'_>,
    owner: &Pubkey,
    generation: u64,
) -> Result<PositionV1<N>, ProgramError> {
    let (expected, _) = Pubkey::find_program_address(
        &[
            POSITION_PDA_DOMAIN,
            market_account.key.as_ref(),
            owner.as_ref(),
        ],
        program_id,
    );
    if position_account.key != &expected || position_account.owner != program_id {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = position_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let position =
        PositionV1::<N>::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    if position.market() != market_account.key.as_ref()
        || position.owner() != owner.as_ref()
        || position.generation() != generation
    {
        return Err(AdapterError::ReplayMismatch.into());
    }
    let canonical = encode_position(position)?;
    if canonical.as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(position)
}

fn parse_receipt_mint(
    mint_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<MintObservationV1, ProgramError> {
    if mint_account.owner != token_program.key || mint_account.data_len() != BEARER_MINT_BYTES {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = mint_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let base = Mint::parse(data.get(..82).ok_or(AdapterError::BearerAuthentication)?)
        .map_err(|_| AdapterError::BearerAuthentication)?;
    if data.get(82..165) != Some(&[0; 83]) || data.get(165) != Some(&1) {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let mut close = None;
    let mut permissioned_burn = None;
    let mut count = 0u16;
    let mut offset = 166usize;
    while offset < data.len() {
        let kind = read_u16(&data, offset)?;
        let length = usize::from(read_u16(&data, offset + 2)?);
        let next = offset
            .checked_add(4)
            .and_then(|value| value.checked_add(length))
            .ok_or(AdapterError::Arithmetic)?;
        let authority: [u8; 32] = data
            .get(offset + 4..next)
            .ok_or(AdapterError::BearerAuthentication)?
            .try_into()
            .map_err(|_| AdapterError::BearerAuthentication)?;
        if length != 32 {
            return Err(AdapterError::BearerAuthentication.into());
        }
        match kind {
            value
                if value == ExtensionType::MintCloseAuthority as u16
                    && close.replace(authority).is_none() => {}
            value
                if value == ExtensionType::PermissionedBurn as u16
                    && permissioned_burn.replace(authority).is_none() => {}
            _ => return Err(AdapterError::BearerAuthentication.into()),
        }
        count = count.checked_add(1).ok_or(AdapterError::Arithmetic)?;
        offset = next;
    }
    Ok(MintObservationV1 {
        key: mint_account.key.to_bytes(),
        program_owner: mint_account.owner.to_bytes(),
        data_len: data.len(),
        supply: base.supply,
        decimals: base.decimals,
        initialized: base.is_initialized,
        mint_authority: coption_address(base.mint_authority),
        freeze_authority: coption_address(base.freeze_authority),
        close_authority: close,
        permissioned_burn_authority: permissioned_burn,
        extension_count: count,
    })
}

fn parse_receipt_account(
    token_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<TokenAccountObservationV1, ProgramError> {
    if token_account.owner != token_program.key
        || token_account.data_len() != BEARER_TOKEN_ACCOUNT_BYTES
    {
        return Err(AdapterError::BearerAuthentication.into());
    }
    let data = token_account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let token = TokenAccount::parse(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    let state = match token.state {
        AccountState::Uninitialized => TokenAccountStateV1::Uninitialized,
        AccountState::Initialized => TokenAccountStateV1::Initialized,
        AccountState::Frozen => TokenAccountStateV1::Frozen,
    };
    Ok(TokenAccountObservationV1 {
        key: token_account.key.to_bytes(),
        program_owner: token_account.owner.to_bytes(),
        data_len: data.len(),
        mint: token.mint,
        authority: token.owner,
        amount: token.amount,
        state,
        has_native_reserve: !token.native_reserve.is_none(),
        extension_count: 0,
    })
}

#[allow(clippy::too_many_arguments)]
fn execute_receipt_plan<'a>(
    program_id: &Pubkey,
    descriptor: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token: &AccountInfo<'a>,
    controller: &AccountInfo<'a>,
    holder: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    plan: ReceiptSupplyPlanV1,
    mint_before: MintObservationV1,
    token_before: TokenAccountObservationV1,
) -> Result<(), ProgramError> {
    if plan.mint() != mint.key.to_bytes()
        || plan.receipt_controller() != controller.key.to_bytes()
        || plan.token_account() != token.key.to_bytes()
        || plan.holder() != holder.key.to_bytes()
        || plan.mint_supply_before() != mint_before.supply
        || plan.account_balance_before() != token_before.amount
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    let instruction = match plan.operation() {
        ReceiptOperationV1::Mint => spl_token_2022_interface::instruction::mint_to_checked(
            token_program.key,
            mint.key,
            token.key,
            controller.key,
            &[],
            plan.amount(),
            0,
        )?,
        ReceiptOperationV1::Burn => {
            spl_token_2022_interface::extension::permissioned_burn::instruction::burn_checked(
                token_program.key,
                token.key,
                mint.key,
                controller.key,
                holder.key,
                &[],
                plan.amount(),
                0,
            )?
        }
    };
    let infos = match plan.operation() {
        ReceiptOperationV1::Mint => Vec::from([
            mint.clone(),
            token.clone(),
            controller.clone(),
            token_program.clone(),
        ]),
        ReceiptOperationV1::Burn => Vec::from([
            token.clone(),
            mint.clone(),
            controller.clone(),
            holder.clone(),
            token_program.clone(),
        ]),
    };
    invoke_controller_signed(program_id, descriptor, &instruction, &infos)
        .map_err(|_| AdapterError::BearerTokenCpi)?;
    let mint_after = parse_receipt_mint(mint, token_program)?;
    let token_after = parse_receipt_account(token, token_program)?;
    let mut expected_mint = mint_before;
    expected_mint.supply = plan.mint_supply_after();
    let mut expected_token = token_before;
    expected_token.amount = plan.account_balance_after();
    if mint_after != expected_mint || token_after != expected_token {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn invoke_controller_signed<'a>(
    program_id: &Pubkey,
    descriptor: &AccountInfo<'a>,
    instruction: &Instruction,
    infos: &[AccountInfo<'a>],
) -> Result<(), ProgramError> {
    let (_, bump) = Pubkey::find_program_address(
        &[
            dclutch_structured_contract::descriptor::STRUCTURED_RECEIPT_AUTHORITY_PDA_DOMAIN_V1,
            descriptor.key.as_ref(),
        ],
        program_id,
    );
    invoke_signed(
        instruction,
        infos,
        &[&[
            dclutch_structured_contract::descriptor::STRUCTURED_RECEIPT_AUTHORITY_PDA_DOMAIN_V1,
            descriptor.key.as_ref(),
            &[bump],
        ]],
    )
}

fn initialize_receipt_mint<'a>(
    mint: &AccountInfo<'a>,
    controller: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Result<(), ProgramError> {
    let close = spl_token_2022_interface::instruction::initialize_mint_close_authority(
        token_program.key,
        mint.key,
        Some(controller.key),
    )?;
    invoke(&close, &[mint.clone(), token_program.clone()])
        .map_err(|_| AdapterError::BearerTokenCpi)?;
    let burn = spl_token_2022_interface::extension::permissioned_burn::instruction::initialize(
        token_program.key,
        mint.key,
        controller.key,
    )?;
    invoke(&burn, &[mint.clone(), token_program.clone()])
        .map_err(|_| AdapterError::BearerTokenCpi)?;
    let initialize = spl_token_2022_interface::instruction::initialize_mint2(
        token_program.key,
        mint.key,
        controller.key,
        None,
        0,
    )?;
    invoke(&initialize, &[mint.clone(), token_program.clone()])
        .map_err(|_| AdapterError::BearerTokenCpi)?;
    Ok(())
}

fn validate_frame(
    accounts: &[AccountInfo<'_>],
    action: StructuredActionV1,
) -> Result<(), ProgramError> {
    let expected = match action {
        StructuredActionV1::Activate => ACTIVATE_ACCOUNTS,
        StructuredActionV1::Wrap | StructuredActionV1::Unwrap => QUANTITY_ACCOUNTS,
        StructuredActionV1::RedeemTerminal => TERMINAL_ACCOUNTS,
        StructuredActionV1::Retire => RETIRE_ACCOUNTS,
    };
    if accounts.len() != expected || accounts.len() < COMMON_ACCOUNTS {
        return Err(AdapterError::AccountFrameLength.into());
    }
    require_frame_alias_policy(accounts, action)?;
    for index in [
        MANIFEST,
        MANIFEST_CURSOR,
        CONFIG,
        CONFIG_CURSOR,
        INSTANCE,
        INSTANCE_CURSOR,
        TEMPLATE,
        TEMPLATE_CURSOR,
        RECEIPT_AUTHORITY,
        RENT_SYSVAR,
    ] {
        require_privilege(account(accounts, index)?, false, false, false)?;
    }
    require_privilege(
        account(accounts, RECEIPT_TOKEN_PROGRAM)?,
        false,
        false,
        true,
    )?;
    match action {
        StructuredActionV1::Activate => {
            for index in [
                MARKET,
                DESCRIPTOR,
                RECEIPT_MINT,
                ACTIVATE_FUNDING,
                ACTIVATE_PAYER,
                ACTIVATE_CUSTODY,
            ] {
                let signer = index == ACTIVATE_PAYER;
                require_privilege(account(accounts, index)?, signer, true, false)?;
            }
            require_privilege(account(accounts, ACTIVATE_SYSTEM)?, false, false, true)?;
        }
        StructuredActionV1::Wrap | StructuredActionV1::Unwrap => {
            require_privilege(account(accounts, MARKET)?, false, false, false)?;
            require_privilege(account(accounts, DESCRIPTOR)?, false, false, false)?;
            require_privilege(account(accounts, RECEIPT_MINT)?, false, true, false)?;
            require_privilege(account(accounts, QUANTITY_HOLDER)?, true, false, false)?;
            for index in [
                QUANTITY_OWNER_POSITION,
                QUANTITY_CUSTODY,
                QUANTITY_RECEIPT_ACCOUNT,
            ] {
                require_privilege(account(accounts, index)?, false, true, false)?;
            }
        }
        StructuredActionV1::RedeemTerminal => {
            require_privilege(account(accounts, MARKET)?, false, true, false)?;
            require_privilege(account(accounts, DESCRIPTOR)?, false, false, false)?;
            require_privilege(account(accounts, RECEIPT_MINT)?, false, true, false)?;
            require_privilege(account(accounts, TERMINAL_HOLDER)?, true, false, false)?;
            for index in [
                TERMINAL_CUSTODY,
                TERMINAL_RECEIPT_ACCOUNT,
                TERMINAL_COLLATERAL_VAULT,
                TERMINAL_COLLATERAL_DESTINATION,
            ] {
                require_privilege(account(accounts, index)?, false, true, false)?;
            }
            for index in [TERMINAL_REALM, TERMINAL_COLLATERAL_MINT] {
                require_privilege(account(accounts, index)?, false, false, false)?;
            }
            require_privilege(
                account(accounts, TERMINAL_COLLATERAL_PROGRAM)?,
                false,
                false,
                true,
            )?;
        }
        StructuredActionV1::Retire => {
            for index in [
                MARKET,
                DESCRIPTOR,
                RECEIPT_MINT,
                RETIRE_CUSTODY,
                RETIRE_RENT_CREDIT,
            ] {
                require_privilege(account(accounts, index)?, false, true, false)?;
            }
        }
    }
    Ok(())
}

fn require_frame_alias_policy(
    accounts: &[AccountInfo<'_>],
    action: StructuredActionV1,
) -> Result<(), ProgramError> {
    for (index, current) in accounts.iter().enumerate() {
        for (prior_index, prior) in accounts.iter().take(index).enumerate() {
            if prior.key == current.key
                && !(action == StructuredActionV1::RedeemTerminal
                    && prior_index == RECEIPT_TOKEN_PROGRAM
                    && index == TERMINAL_COLLATERAL_PROGRAM)
            {
                return Err(AdapterError::AccountIdentity.into());
            }
        }
    }
    Ok(())
}

fn require_privilege(
    account: &AccountInfo<'_>,
    signer: bool,
    writable: bool,
    executable: bool,
) -> Result<(), ProgramError> {
    if account.is_signer != signer
        || account.is_writable != writable
        || account.executable != executable
    {
        Err(AdapterError::AccountPrivilege.into())
    } else {
        Ok(())
    }
}

fn authenticate_fixed_programs(
    token_program: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
    system: Option<&AccountInfo<'_>>,
) -> Result<(), ProgramError> {
    if token_program.key.to_bytes() != TOKEN_2022_PROGRAM_ID
        || !token_program.executable
        || !recognized_program_loader(token_program.owner)
        || rent.key != &sysvar::rent::ID
        || rent.owner != &sysvar::ID
        || system.is_some_and(|value| {
            value.key != &system_program::ID
                || value.owner != &native_loader::ID
                || !value.executable
        })
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn require_ephemeral_controller(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || account.executable || !account.data_is_empty() {
        Err(AdapterError::AccountIdentity.into())
    } else {
        Ok(())
    }
}

fn require_vacant(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if account.owner != &system_program::ID || !account.data_is_empty() || account.lamports() != 0 {
        Err(AdapterError::AccountIdentity.into())
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    new_account: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    lamports: u64,
    space: usize,
    owner: &Pubkey,
    seeds: &[&[u8]],
    bump: u8,
) -> Result<(), ProgramError> {
    let instruction = create_account(
        payer.key,
        new_account.key,
        lamports,
        u64::try_from(space).map_err(|_| AdapterError::Arithmetic)?,
        owner,
    );
    let bump_seed = [bump];
    let mut signer = Vec::new();
    signer
        .try_reserve_exact(seeds.len() + 1)
        .map_err(|_| AdapterError::Arithmetic)?;
    signer.extend_from_slice(seeds);
    signer.push(&bump_seed);
    invoke_signed(
        &instruction,
        &[payer.clone(), new_account.clone(), system.clone()],
        &[signer.as_slice()],
    )
    .map_err(|_| AdapterError::BearerCreateCpi)?;
    if new_account.lamports() != lamports
        || new_account.owner != owner
        || new_account.data_len() != space
    {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn persist_market<const N: usize>(
    account: &AccountInfo<'_>,
    market: CategoricalMarketV1<N>,
) -> Result<(), ProgramError> {
    let bytes = encode_market(market)?;
    write_exact(account, &bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if CategoricalMarketV1::<N>::decode(&data) != Ok(market) {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn persist_position<const N: usize>(
    account: &AccountInfo<'_>,
    position: PositionV1<N>,
) -> Result<(), ProgramError> {
    let bytes = encode_position(position)?;
    write_exact(account, &bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if PositionV1::<N>::decode(&data) != Ok(position) {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn persist_descriptor(
    account: &AccountInfo<'_>,
    descriptor: StructuredDescriptorV1,
) -> Result<(), ProgramError> {
    let bytes = descriptor.to_bytes();
    write_exact(account, &bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if StructuredDescriptorV1::decode(&data) != Ok(descriptor) {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn persist_funding(account: &AccountInfo<'_>, funding: FundingStateV1) -> Result<(), ProgramError> {
    let bytes = funding.to_bytes();
    write_exact(account, &bytes)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if FundingStateV1::decode(&data) != Ok(funding) {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn write_exact(account: &AccountInfo<'_>, bytes: &[u8]) -> Result<(), ProgramError> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::BearerPostcondition)?;
    if data.len() != bytes.len() {
        return Err(AdapterError::BearerPostcondition.into());
    }
    data.copy_from_slice(bytes);
    Ok(())
}

fn encode_market<const N: usize>(market: CategoricalMarketV1<N>) -> Result<Vec<u8>, ProgramError> {
    let length = CategoricalMarketV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    let mut bytes = zeroed(length)?;
    market
        .encode(&mut bytes)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    Ok(bytes)
}

fn encode_position<const N: usize>(position: PositionV1<N>) -> Result<Vec<u8>, ProgramError> {
    let length = PositionV1::<N>::encoded_len().map_err(|_| AdapterError::Arithmetic)?;
    let mut bytes = zeroed(length)?;
    position
        .encode(&mut bytes)
        .map_err(|_| AdapterError::BearerPostcondition)?;
    Ok(bytes)
}

fn zeroed(length: usize) -> Result<Vec<u8>, ProgramError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| AdapterError::Arithmetic)?;
    output.resize(length, 0);
    Ok(output)
}

fn move_lamports(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
    amount: u64,
) -> Result<(), ProgramError> {
    let source_after = source
        .lamports()
        .checked_sub(amount)
        .ok_or(AdapterError::Arithmetic)?;
    let destination_after = destination
        .lamports()
        .checked_add(amount)
        .ok_or(AdapterError::Arithmetic)?;
    {
        let mut source_lamports = source
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::BearerPostcondition)?;
        let mut destination_lamports = destination
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::BearerPostcondition)?;
        **source_lamports = source_after;
        **destination_lamports = destination_after;
    }
    if source.lamports() != source_after || destination.lamports() != destination_after {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn authenticate_named_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_key: [u8; 32],
) -> Result<RentCreditV1, ProgramError> {
    if account.key.to_bytes() != expected_key
        || account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::BearerAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::BearerAuthentication)?;
    let authority = credit.refund_authority().to_bytes();
    let (derived, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority.as_slice()],
        program_id,
    );
    if account.key != &derived
        || credit.pda_bump() != bump
        || credit.to_bytes().as_slice() != &data[..]
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(credit)
}

fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    credit: RentCreditV1,
) -> Result<(), ProgramError> {
    let observed = authenticate_named_rent_credit(program_id, account, account.key.to_bytes())?;
    if observed != credit {
        return Err(AdapterError::BearerPostcondition.into());
    }
    Ok(())
}

fn close_program_account(
    source: &AccountInfo<'_>,
    destination: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    let amount = source.lamports();
    move_lamports(source, destination, amount)?;
    source.resize(0).map_err(|_| AdapterError::BearerClose)?;
    source.assign(&system_program::ID);
    if source.lamports() != 0 || !source.data_is_empty() || source.owner != &system_program::ID {
        return Err(AdapterError::BearerClose.into());
    }
    Ok(())
}

fn domain_content_id(domain: &[u8], bytes: &[u8]) -> Result<[u8; 32], ProgramError> {
    let value = hashv(&[domain, &[0], bytes]).to_bytes();
    if value.iter().all(|byte| *byte == 0) {
        Err(AdapterError::ContentIdentity.into())
    } else {
        Ok(value)
    }
}

fn content_id(bytes: &[u8]) -> Result<ContentId, ProgramError> {
    ContentId::new(hash(bytes).to_bytes()).map_err(|_| AdapterError::ContentIdentity.into())
}

fn coption_address(value: COption<[u8; 32]>) -> Option<[u8; 32]> {
    match value {
        COption::None => None,
        COption::Some(address) => Some(address),
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ProgramError> {
    let end = offset.checked_add(2).ok_or(AdapterError::Arithmetic)?;
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(AdapterError::BearerAuthentication)?
            .try_into()
            .map_err(|_| AdapterError::BearerAuthentication)?,
    ))
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or(AdapterError::AccountFrameLength.into())
}

#[cfg(test)]
mod tests {
    use std::{boxed::Box, sync::Mutex, vec::Vec};

    use dclutch_core_contract::{ContentId as CoreContentId, MarketIdentity, MarketRoot};
    use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};
    use solana_program::{
        entrypoint::ProgramResult,
        program_stubs::{SyscallStubs, set_syscall_stubs},
    };

    use super::*;

    static CPI_STUB_LOCK: Mutex<()> = Mutex::new(());

    struct RejectCpi;

    impl SyscallStubs for RejectCpi {
        fn sol_invoke_signed(
            &self,
            _instruction: &Instruction,
            _account_infos: &[AccountInfo<'_>],
            _signers_seeds: &[&[&[u8]]],
        ) -> ProgramResult {
            Err(ProgramError::Custom(0x51_52_53))
        }
    }

    struct ApplyTransferCpi;

    impl SyscallStubs for ApplyTransferCpi {
        fn sol_invoke_signed(
            &self,
            instruction: &Instruction,
            account_infos: &[AccountInfo<'_>],
            _signers_seeds: &[&[&[u8]]],
        ) -> ProgramResult {
            let amount = u64::from_le_bytes(
                instruction
                    .data
                    .get(1..9)
                    .ok_or(ProgramError::InvalidInstructionData)?
                    .try_into()
                    .map_err(|_| ProgramError::InvalidInstructionData)?,
            );
            let source = account_infos
                .first()
                .ok_or(ProgramError::NotEnoughAccountKeys)?;
            let destination = account_infos
                .get(2)
                .ok_or(ProgramError::NotEnoughAccountKeys)?;
            let mut source_data = source
                .try_borrow_mut_data()
                .map_err(|_| ProgramError::AccountBorrowFailed)?;
            let source_amount = u64::from_le_bytes(
                source_data
                    .get(64..72)
                    .ok_or(ProgramError::InvalidAccountData)?
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?,
            );
            source_data
                .get_mut(64..72)
                .ok_or(ProgramError::InvalidAccountData)?
                .copy_from_slice(
                    &source_amount
                        .checked_sub(amount)
                        .ok_or(ProgramError::InsufficientFunds)?
                        .to_le_bytes(),
                );
            drop(source_data);
            let mut destination_data = destination
                .try_borrow_mut_data()
                .map_err(|_| ProgramError::AccountBorrowFailed)?;
            let destination_amount = u64::from_le_bytes(
                destination_data
                    .get(64..72)
                    .ok_or(ProgramError::InvalidAccountData)?
                    .try_into()
                    .map_err(|_| ProgramError::InvalidAccountData)?,
            );
            destination_data
                .get_mut(64..72)
                .ok_or(ProgramError::InvalidAccountData)?
                .copy_from_slice(
                    &destination_amount
                        .checked_add(amount)
                        .ok_or(ProgramError::InvalidArgument)?
                        .to_le_bytes(),
                );
            Ok(())
        }
    }

    fn test_account(key: Pubkey, data: Vec<u8>, owner: Pubkey) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            true,
            Box::leak(Box::new(1)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            false,
        )
    }

    fn token_program() -> AccountInfo<'static> {
        test_account(
            Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            Vec::new(),
            native_loader::ID,
        )
    }

    fn mint_bytes(controller: Pubkey, supply: u64) -> Vec<u8> {
        let mut bytes = std::vec![0u8; BEARER_MINT_BYTES];
        bytes
            .get_mut(0..4)
            .expect("mint option")
            .copy_from_slice(&1u32.to_le_bytes());
        bytes
            .get_mut(4..36)
            .expect("mint authority")
            .copy_from_slice(controller.as_ref());
        bytes
            .get_mut(36..44)
            .expect("supply")
            .copy_from_slice(&supply.to_le_bytes());
        *bytes.get_mut(45).expect("initialized") = 1;
        *bytes.get_mut(165).expect("account type") = 1;
        bytes
            .get_mut(166..168)
            .expect("close kind")
            .copy_from_slice(&(ExtensionType::MintCloseAuthority as u16).to_le_bytes());
        bytes
            .get_mut(168..170)
            .expect("close width")
            .copy_from_slice(&32u16.to_le_bytes());
        bytes
            .get_mut(170..202)
            .expect("close authority")
            .copy_from_slice(controller.as_ref());
        bytes
            .get_mut(202..204)
            .expect("burn kind")
            .copy_from_slice(&(ExtensionType::PermissionedBurn as u16).to_le_bytes());
        bytes
            .get_mut(204..206)
            .expect("burn width")
            .copy_from_slice(&32u16.to_le_bytes());
        bytes
            .get_mut(206..238)
            .expect("burn authority")
            .copy_from_slice(controller.as_ref());
        bytes
    }

    fn token_bytes(mint: Pubkey, holder: Pubkey, amount: u64) -> Vec<u8> {
        let mut bytes = std::vec![0u8; BEARER_TOKEN_ACCOUNT_BYTES];
        bytes
            .get_mut(0..32)
            .expect("mint")
            .copy_from_slice(mint.as_ref());
        bytes
            .get_mut(32..64)
            .expect("holder")
            .copy_from_slice(holder.as_ref());
        bytes
            .get_mut(64..72)
            .expect("amount")
            .copy_from_slice(&amount.to_le_bytes());
        *bytes.get_mut(108).expect("state") = 1;
        bytes
    }

    fn collateral_mint_bytes(supply: u64, decimals: u8) -> Vec<u8> {
        let mut bytes = std::vec![0u8; 82];
        bytes
            .get_mut(36..44)
            .expect("collateral supply")
            .copy_from_slice(&supply.to_le_bytes());
        *bytes.get_mut(44).expect("collateral decimals") = decimals;
        *bytes.get_mut(45).expect("collateral initialized") = 1;
        bytes
    }

    fn realm_facts(mint: Pubkey, mint_bytes: &[u8]) -> RealmFacts {
        let release = CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer();
        let realm = RealmV1::new(RealmV1Input {
            token_program: TOKEN_2022_PROGRAM_ID,
            collateral_mint: mint.to_bytes(),
            collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("test Realm");
        let parsed_mint = release
            .profile()
            .check_mint(TOKEN_2022_PROGRAM_ID, mint_bytes)
            .expect("test collateral Mint");
        RealmFacts {
            realm,
            release,
            mint: parsed_mint,
        }
    }

    fn core_id(fill: u8) -> CoreContentId {
        CoreContentId::new([fill; 32]).expect("core identity")
    }

    fn market_root() -> MarketRoot {
        MarketRoot::founding(
            MarketIdentity::new(
                core_id(1),
                core_id(2),
                core_id(3),
                core_id(4),
                core_id(5),
                7,
            ),
            [9; 32],
        )
        .expect("Market root")
    }

    #[test]
    fn schema_release_constants_match_owned_preimages() {
        assert_eq!(
            hash(b"dclutch/schema/structured-config-v1").to_bytes(),
            STRUCTURED_CONFIG_SCHEMA_RELEASE_ID_V1
        );
        assert_eq!(
            hash(b"dclutch/schema/product-portfolio-template-v1").to_bytes(),
            PORTFOLIO_TEMPLATE_SCHEMA_RELEASE_ID_V1
        );
    }

    #[test]
    fn content_namespaces_are_domain_separated() {
        let bytes = [7u8; 112];
        assert_ne!(
            hash(&bytes).to_bytes(),
            domain_content_id(STRUCTURED_CONFIG_CONTENT_DOMAIN_V1, &bytes).expect("content ID")
        );
    }

    #[test]
    fn hostile_mint_parser_consumes_exact_base_padding_and_two_tlvs() {
        let token_program = token_program();
        let controller = Pubkey::new_unique();
        let mint_key = Pubkey::new_unique();
        let valid = mint_bytes(controller, 9);
        let mint = test_account(mint_key, valid.clone(), *token_program.key);
        let observation = parse_receipt_mint(&mint, &token_program).expect("exact receipt Mint");
        assert_eq!(observation.supply, 9);
        assert_eq!(observation.close_authority, Some(controller.to_bytes()));
        assert_eq!(
            observation.permissioned_burn_authority,
            Some(controller.to_bytes())
        );

        for offset in [82usize, 165, 166, 168, 202, 204] {
            let mut hostile = valid.clone();
            *hostile.get_mut(offset).expect("hostile offset") ^= 0xff;
            let account = test_account(Pubkey::new_unique(), hostile, *token_program.key);
            assert!(parse_receipt_mint(&account, &token_program).is_err());
        }
        let mut duplicate = valid;
        duplicate
            .get_mut(202..204)
            .expect("second kind")
            .copy_from_slice(&(ExtensionType::MintCloseAuthority as u16).to_le_bytes());
        let account = test_account(Pubkey::new_unique(), duplicate, *token_program.key);
        assert!(parse_receipt_mint(&account, &token_program).is_err());
    }

    #[test]
    fn hostile_holder_parser_refuses_bad_option_and_state_tags() {
        let token_program = token_program();
        let mint = Pubkey::new_unique();
        let holder = Pubkey::new_unique();
        let valid = token_bytes(mint, holder, 7);
        let account = test_account(Pubkey::new_unique(), valid.clone(), *token_program.key);
        let observation =
            parse_receipt_account(&account, &token_program).expect("exact holder Account");
        assert_eq!(observation.amount, 7);
        assert_eq!(observation.authority, holder.to_bytes());

        for (offset, value) in [(72usize, 2u8), (108, 9), (109, 2), (129, 2)] {
            let mut hostile = valid.clone();
            *hostile.get_mut(offset).expect("hostile tag") = value;
            let account = test_account(Pubkey::new_unique(), hostile, *token_program.key);
            assert!(parse_receipt_account(&account, &token_program).is_err());
        }
    }

    #[test]
    fn terminal_delta_refuses_wrong_winner_payout_or_nonburn_receipt_effect() {
        assert_eq!(
            validate_terminal_economic_delta(ReceiptOperationV1::Burn, 3, 11, 8),
            Ok(())
        );
        assert!(
            validate_terminal_economic_delta(ReceiptOperationV1::Burn, 2, 11, 8).is_err(),
            "a payout from any noncanonical winner coefficient must refuse"
        );
        assert!(
            validate_terminal_economic_delta(ReceiptOperationV1::Mint, 3, 11, 8).is_err(),
            "terminal settlement must burn the sole receipt supply"
        );
        assert!(
            validate_terminal_economic_delta(ReceiptOperationV1::Burn, 3, 8, 11).is_err(),
            "terminal settlement cannot increase Hoard principal"
        );
    }

    #[test]
    fn terminal_frame_allows_only_the_shared_token_program_alias() {
        let mut accounts = Vec::new();
        for _ in 0..TERMINAL_ACCOUNTS {
            accounts.push(test_account(
                Pubkey::new_unique(),
                Vec::new(),
                system_program::ID,
            ));
        }
        let shared_program = token_program();
        *accounts
            .get_mut(RECEIPT_TOKEN_PROGRAM)
            .expect("receipt program role") = shared_program.clone();
        *accounts
            .get_mut(TERMINAL_COLLATERAL_PROGRAM)
            .expect("collateral program role") = shared_program;
        assert_eq!(
            require_frame_alias_policy(&accounts, StructuredActionV1::RedeemTerminal),
            Ok(())
        );

        let market_key = *accounts.first().expect("market role").key;
        let duplicate = test_account(market_key, Vec::new(), system_program::ID);
        *accounts.get_mut(DESCRIPTOR).expect("descriptor role") = duplicate;
        assert!(require_frame_alias_policy(&accounts, StructuredActionV1::RedeemTerminal).is_err());
    }

    #[test]
    fn terminal_collateral_authentication_refuses_wrong_vault_mint_authority_and_balance() {
        let program_id = Pubkey::new_unique();
        let market = test_account(Pubkey::new_unique(), Vec::new(), program_id);
        let holder = Pubkey::new_unique();
        let collateral_mint_key = Pubkey::new_unique();
        let collateral_mint_data = collateral_mint_bytes(100, 6);
        let collateral_program = token_program();
        let collateral_mint = test_account(
            collateral_mint_key,
            collateral_mint_data.clone(),
            *collateral_program.key,
        );
        let realm = realm_facts(collateral_mint_key, &collateral_mint_data);
        let (vault_key, _) = Pubkey::find_program_address(
            &[COLLATERAL_VAULT_PDA_DOMAIN, market.key.as_ref()],
            &program_id,
        );
        let valid_vault = test_account(
            vault_key,
            token_bytes(collateral_mint_key, *market.key, 12),
            *collateral_program.key,
        );
        let valid_destination = test_account(
            Pubkey::new_unique(),
            token_bytes(collateral_mint_key, holder, 4),
            *collateral_program.key,
        );
        let vault_facts = authenticate_collateral_vault(
            &program_id,
            &market,
            &valid_vault,
            &collateral_mint,
            &collateral_program,
            realm,
            10,
        )
        .expect("canonical collateral vault");
        let destination_facts = authenticate_holder_collateral_account(
            &valid_destination,
            &collateral_program,
            realm,
            &holder,
        )
        .expect("holder collateral account");
        assert!(
            authenticate_collateral_transfer(
                &valid_vault,
                &valid_destination,
                &collateral_mint,
                &collateral_program,
                realm,
                market.key,
                3,
                vault_facts,
                destination_facts,
            )
            .is_ok()
        );

        let wrong_vault = test_account(
            Pubkey::new_unique(),
            token_bytes(collateral_mint_key, *market.key, 12),
            *collateral_program.key,
        );
        assert!(
            authenticate_collateral_vault(
                &program_id,
                &market,
                &wrong_vault,
                &collateral_mint,
                &collateral_program,
                realm,
                10,
            )
            .is_err()
        );

        let foreign_mint_vault = test_account(
            vault_key,
            token_bytes(Pubkey::new_unique(), *market.key, 12),
            *collateral_program.key,
        );
        assert!(
            authenticate_collateral_vault(
                &program_id,
                &market,
                &foreign_mint_vault,
                &collateral_mint,
                &collateral_program,
                realm,
                10,
            )
            .is_err()
        );

        let wrong_authority_vault = test_account(
            vault_key,
            token_bytes(collateral_mint_key, Pubkey::new_unique(), 12),
            *collateral_program.key,
        );
        assert!(
            authenticate_collateral_vault(
                &program_id,
                &market,
                &wrong_authority_vault,
                &collateral_mint,
                &collateral_program,
                realm,
                10,
            )
            .is_err()
        );

        let underfunded_vault = test_account(
            vault_key,
            token_bytes(collateral_mint_key, *market.key, 9),
            *collateral_program.key,
        );
        assert!(
            authenticate_collateral_vault(
                &program_id,
                &market,
                &underfunded_vault,
                &collateral_mint,
                &collateral_program,
                realm,
                10,
            )
            .is_err()
        );

        let foreign_holder_destination = test_account(
            Pubkey::new_unique(),
            token_bytes(collateral_mint_key, Pubkey::new_unique(), 4),
            *collateral_program.key,
        );
        assert!(
            authenticate_holder_collateral_account(
                &foreign_holder_destination,
                &collateral_program,
                realm,
                &holder,
            )
            .is_err()
        );

        let low_transfer_vault = test_account(
            vault_key,
            token_bytes(collateral_mint_key, *market.key, 2),
            *collateral_program.key,
        );
        assert!(
            authenticate_collateral_transfer(
                &low_transfer_vault,
                &valid_destination,
                &collateral_mint,
                &collateral_program,
                realm,
                market.key,
                3,
                vault_facts,
                destination_facts,
            )
            .is_err()
        );
    }

    #[test]
    fn late_collateral_cpi_failure_returns_before_any_local_persistence() {
        let _guard = CPI_STUB_LOCK.lock().expect("CPI stub lock");
        let program_id = Pubkey::new_unique();
        let root = market_root();
        let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
        let (market_key, _) =
            Pubkey::find_program_address(&[MARKET_SEED, identity_digest.as_slice()], &program_id);
        let market = test_account(market_key, Vec::from([0xa5; 8]), program_id);
        let holder = Pubkey::new_unique();
        let mint_key = Pubkey::new_unique();
        let mint_data = collateral_mint_bytes(100, 6);
        let token_program = token_program();
        let mint = test_account(mint_key, mint_data.clone(), *token_program.key);
        let source = test_account(
            Pubkey::new_unique(),
            token_bytes(mint_key, market_key, 12),
            *token_program.key,
        );
        let destination = test_account(
            Pubkey::new_unique(),
            token_bytes(mint_key, holder, 4),
            *token_program.key,
        );
        let realm = realm_facts(mint_key, &mint_data);
        let source_data_before =
            Vec::from(source.try_borrow_data().expect("source snapshot").as_ref());
        let destination_data_before = Vec::from(
            destination
                .try_borrow_data()
                .expect("destination snapshot")
                .as_ref(),
        );
        let market_data_before =
            Vec::from(market.try_borrow_data().expect("Market snapshot").as_ref());
        let source_facts = realm
            .release
            .profile()
            .check_custody_account(
                token_program.key.to_bytes(),
                &source_data_before,
                mint_key.to_bytes(),
                market_key.to_bytes(),
            )
            .expect("source facts");
        let destination_facts = realm
            .release
            .profile()
            .check_transfer_account(token_program.key.to_bytes(), &destination_data_before)
            .expect("destination facts");
        let transfer = authenticate_collateral_transfer(
            &source,
            &destination,
            &mint,
            &token_program,
            realm,
            &market_key,
            3,
            source_facts,
            destination_facts,
        )
        .expect("transfer preflight");

        let previous = set_syscall_stubs(Box::new(RejectCpi));
        let result = execute_collateral_payout(
            &program_id,
            &market,
            &source,
            &destination,
            &mint,
            &token_program,
            realm,
            transfer,
            3,
            root,
        );
        set_syscall_stubs(previous);

        assert!(result.is_err());
        assert_eq!(
            source.try_borrow_data().expect("source after").as_ref(),
            source_data_before
        );
        assert_eq!(
            destination
                .try_borrow_data()
                .expect("destination after")
                .as_ref(),
            destination_data_before
        );
        assert_eq!(
            market.try_borrow_data().expect("Market after").as_ref(),
            market_data_before
        );
    }

    #[test]
    fn collateral_cpi_applies_exact_raw_atoms_and_passes_strict_postchecks() {
        let _guard = CPI_STUB_LOCK.lock().expect("CPI stub lock");
        let program_id = Pubkey::new_unique();
        let root = market_root();
        let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
        let (market_key, _) =
            Pubkey::find_program_address(&[MARKET_SEED, identity_digest.as_slice()], &program_id);
        let market = test_account(market_key, Vec::new(), program_id);
        let holder = Pubkey::new_unique();
        let mint_key = Pubkey::new_unique();
        let mint_data = collateral_mint_bytes(100, 6);
        let token_program = token_program();
        let mint = test_account(mint_key, mint_data.clone(), *token_program.key);
        let source = test_account(
            Pubkey::new_unique(),
            token_bytes(mint_key, market_key, 12),
            *token_program.key,
        );
        let destination = test_account(
            Pubkey::new_unique(),
            token_bytes(mint_key, holder, 4),
            *token_program.key,
        );
        let realm = realm_facts(mint_key, &mint_data);
        let source_facts = realm
            .release
            .profile()
            .check_custody_account(
                token_program.key.to_bytes(),
                &source.try_borrow_data().expect("source bytes"),
                mint_key.to_bytes(),
                market_key.to_bytes(),
            )
            .expect("source facts");
        let destination_facts = realm
            .release
            .profile()
            .check_transfer_account(
                token_program.key.to_bytes(),
                &destination.try_borrow_data().expect("destination bytes"),
            )
            .expect("destination facts");
        let transfer = authenticate_collateral_transfer(
            &source,
            &destination,
            &mint,
            &token_program,
            realm,
            &market_key,
            3,
            source_facts,
            destination_facts,
        )
        .expect("transfer preflight");

        let previous = set_syscall_stubs(Box::new(ApplyTransferCpi));
        let result = execute_collateral_payout(
            &program_id,
            &market,
            &source,
            &destination,
            &mint,
            &token_program,
            realm,
            transfer,
            3,
            root,
        );
        set_syscall_stubs(previous);

        assert_eq!(result, Ok(()));
        let source_after = realm
            .release
            .profile()
            .check_transfer_account(
                token_program.key.to_bytes(),
                &source.try_borrow_data().expect("source after"),
            )
            .expect("source post facts");
        let destination_after = realm
            .release
            .profile()
            .check_transfer_account(
                token_program.key.to_bytes(),
                &destination.try_borrow_data().expect("destination after"),
            )
            .expect("destination post facts");
        assert_eq!(source_after.amount, 9);
        assert_eq!(destination_after.amount, 7);
    }
}
