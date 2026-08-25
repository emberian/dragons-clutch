//! Atomic SBF boundary for exact transferable structured portfolio receipts.
//!
//! This module deliberately owns no structured supply or holder ledger.  The
//! receipt Mint supply and ordinary Token-2022 Account amount are parsed from
//! hostile bytes, passed to `dclutch-structured-contract`, changed by one real
//! Token-2022 CPI, reloaded, and checked before native Position candidates are
//! persisted.
//!
//! Exact account frames (all roles are distinct) are:
//!
//! * Activate: Market, Descriptor, Manifest, Manifest cursor, Config, Config
//!   cursor, Product Instance, Instance cursor, PortfolioTemplate, Template
//!   cursor, receipt Mint, receipt controller, Token-2022 program, Rent sysvar,
//!   capability Funding, payer, custody Position, System Program.
//! * Wrap/Unwrap: the common first fourteen roles above, then holder, holder
//!   Position, custody Position, and holder receipt Account.
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
use dclutch_core_contract::ContentId;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    portfolio::{PORTFOLIO_TEMPLATE_CONTENT_DOMAIN_V1, PortfolioTemplateV1},
    product::InstanceV1,
};
use dclutch_realm_contract::{POSITION_PDA_DOMAIN, PositionV1};
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
        ReceiptOperationV1, ReceiptSupplyPlanV1, activate, audit_backing, retire, unwrap, wrap,
    },
};
use dclutch_token_svm::{AccountState, COption, Mint, TOKEN_2022_PROGRAM_ID, TokenAccount};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::Instruction,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;
use spl_token_2022_interface::extension::ExtensionType;

use crate::{AdapterError, authenticate::MARKET_SEED, realm::recognized_program_loader};

const COMMON_ACCOUNTS: usize = 14;
const ACTIVATE_ACCOUNTS: usize = 18;
const QUANTITY_ACCOUNTS: usize = 18;
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
        // Terminal redemption additionally crosses the Realm collateral-vault
        // boundary.  It remains refused until that physical payout slice is
        // integrated; silently persisting only the semantic Market debit would
        // strand or counterfeit collateral.
        StructuredActionV1::RedeemTerminal => Err(AdapterError::InvalidInstruction.into()),
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
        StructuredActionV1::RedeemTerminal => QUANTITY_ACCOUNTS,
        StructuredActionV1::Retire => RETIRE_ACCOUNTS,
    };
    if accounts.len() != expected || accounts.len() < COMMON_ACCOUNTS {
        return Err(AdapterError::AccountFrameLength.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .take(index)
            .any(|prior| prior.key == account.key)
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
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
            // The terminal frame is intentionally not admitted until the Realm
            // payout roles are added; this prevents a misleading partial ABI.
            return Err(AdapterError::InvalidInstruction.into());
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
    use std::{boxed::Box, vec::Vec};

    use super::*;

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
}
