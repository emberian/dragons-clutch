//! Atomic collateral-custody creation and Market opening.

use alloc::vec::Vec;

use dclutch_capability_contract::{
    CapabilityManifestV1, MARKET_OPENING_READINESS_BYTES, MARKET_OPENING_READINESS_PDA_DOMAIN,
    MarketOpeningReadinessV1,
};
use dclutch_collateral_contract::{
    AccountPrivilege, COLLATERAL_CUSTODY_BYTES, COLLATERAL_CUSTODY_PDA_DOMAIN,
    COLLATERAL_VAULT_PDA_DOMAIN, CollateralCustodyV1, InstructionTag, OpenCollateralVaultV1,
    validate_account_frame,
};
use dclutch_core_contract::{ContentId, Phase as RootPhase};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_realm_contract::{REALM_PDA_DOMAIN, RealmV1};
use dclutch_rent_contract::{
    RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1,
    SourceCloseCreditPlanV1,
};
use dclutch_token_svm::{ACCOUNT_BYTES, CollateralAdapterReleaseV1, initialize_account3};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::{
    AdapterError,
    authenticate::MARKET_SEED,
    realm::{
        recognized_program_loader, require_authority_policy, require_freeze_policy,
        select_adapter_release,
    },
};

const OPEN_ACCOUNTS: usize = 12;
const REQUIRED_FUND_CHILD_COUNT: u64 = 1;
const REQUIRED_PREOPEN_CHILD_COUNT: u64 = 2;
const OPENED_CHILD_COUNT: u64 = 2;
const MIN_OUTCOMES: u8 = 2;
const MAX_OUTCOMES: u8 = 16;

struct OpenFrame<'a, 'info> {
    sponsor: &'a AccountInfo<'info>,
    market: &'a AccountInfo<'info>,
    readiness: &'a AccountInfo<'info>,
    rent_credit: &'a AccountInfo<'info>,
    capability_manifest: &'a AccountInfo<'info>,
    realm: &'a AccountInfo<'info>,
    custody: &'a AccountInfo<'info>,
    vault: &'a AccountInfo<'info>,
    mint: &'a AccountInfo<'info>,
    token_program: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> OpenFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != OPEN_ACCOUNTS {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            sponsor: account(accounts, 0)?,
            market: account(accounts, 1)?,
            readiness: account(accounts, 2)?,
            rent_credit: account(accounts, 3)?,
            capability_manifest: account(accounts, 4)?,
            realm: account(accounts, 5)?,
            custody: account(accounts, 6)?,
            vault: account(accounts, 7)?,
            mint: account(accounts, 8)?,
            token_program: account(accounts, 9)?,
            system_program: account(accounts, 10)?,
            rent_sysvar: account(accounts, 11)?,
        };
        let privileges = [
            privilege(frame.sponsor),
            privilege(frame.market),
            privilege(frame.readiness),
            privilege(frame.rent_credit),
            privilege(frame.capability_manifest),
            privilege(frame.realm),
            privilege(frame.custody),
            privilege(frame.vault),
            privilege(frame.mint),
            privilege(frame.token_program),
            privilege(frame.system_program),
            privilege(frame.rent_sysvar),
        ];
        validate_account_frame(InstructionTag::OpenCollateralVault, &privileges)
            .map_err(|_| AdapterError::AccountPrivilege)?;
        require_distinct(accounts)?;
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
struct OpenPlan {
    outcome_count: u8,
    custody_bump: u8,
    vault_bump: u8,
    custody_rent: u64,
    vault_rent: u64,
    sponsor_before: u64,
    readiness_rent: u64,
    rent_credit: RentCreditV1,
    rent_credit_lamports: u64,
    market_lamports: u64,
    mint_lamports: u64,
    mint_digest: [u8; 32],
    release: CollateralAdapterReleaseV1,
}

/// Create exact custody accounts and transition one founded Market to `Open`.
pub(crate) fn process_open_collateral_vault(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: OpenCollateralVaultV1,
) -> Result<(), ProgramError> {
    let frame = OpenFrame::parse(accounts)?;
    let plan = authenticate_open(program_id, &frame, instruction)?;

    let custody_space =
        u64::try_from(COLLATERAL_CUSTODY_BYTES).map_err(|_| AdapterError::Arithmetic)?;
    let create_custody = create_account(
        frame.sponsor.key,
        frame.custody.key,
        plan.custody_rent,
        custody_space,
        program_id,
    );
    let custody_bump = [plan.custody_bump];
    let custody_signer = [
        COLLATERAL_CUSTODY_PDA_DOMAIN,
        frame.market.key.as_ref(),
        custody_bump.as_slice(),
    ];
    invoke_signed(
        &create_custody,
        &[
            frame.sponsor.clone(),
            frame.custody.clone(),
            frame.system_program.clone(),
        ],
        &[&custody_signer],
    )
    .map_err(|_| AdapterError::CustodyCreateCpi)?;
    let sponsor_after_custody = plan
        .sponsor_before
        .checked_sub(plan.custody_rent)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != sponsor_after_custody
        || frame.custody.lamports() != plan.custody_rent
        || frame.custody.owner != program_id
        || frame.custody.data_len() != COLLATERAL_CUSTODY_BYTES
    {
        return Err(AdapterError::VaultPostcondition.into());
    }

    let vault_space = u64::try_from(ACCOUNT_BYTES).map_err(|_| AdapterError::Arithmetic)?;
    let create_vault = create_account(
        frame.sponsor.key,
        frame.vault.key,
        plan.vault_rent,
        vault_space,
        frame.token_program.key,
    );
    let vault_bump = [plan.vault_bump];
    let vault_signer = [
        COLLATERAL_VAULT_PDA_DOMAIN,
        frame.market.key.as_ref(),
        vault_bump.as_slice(),
    ];
    invoke_signed(
        &create_vault,
        &[
            frame.sponsor.clone(),
            frame.vault.clone(),
            frame.system_program.clone(),
        ],
        &[&vault_signer],
    )
    .map_err(|_| AdapterError::VaultCreateCpi)?;

    let initialize = initialize_vault_instruction(
        plan.release,
        *frame.vault.key,
        *frame.mint.key,
        *frame.market.key,
    )?;
    invoke(
        &initialize,
        &[
            frame.vault.clone(),
            frame.mint.clone(),
            frame.token_program.clone(),
        ],
    )
    .map_err(|_| AdapterError::VaultInitializeCpi)?;

    persist_open(program_id, &frame, instruction, plan)
}

#[inline(never)]
fn authenticate_open(
    program_id: &Pubkey,
    frame: &OpenFrame<'_, '_>,
    instruction: OpenCollateralVaultV1,
) -> Result<OpenPlan, ProgramError> {
    if instruction.child_count() != REQUIRED_PREOPEN_CHILD_COUNT {
        return Err(AdapterError::ReplayMismatch.into());
    }
    authenticate_account_identities(program_id, frame)?;
    let (realm, realm_digest) = authenticate_realm(program_id, frame)?;
    let release = authenticate_token(frame, realm)?;

    let manifest_data = frame
        .capability_manifest
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| AdapterError::VaultAuthentication)?;
    if manifest.as_bytes() != &manifest_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let manifest_digest = hash(manifest.as_bytes()).to_bytes();
    let manifest_id =
        ContentId::new(manifest_digest).map_err(|_| AdapterError::VaultAuthentication)?;

    let readiness_data = frame
        .readiness
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let readiness = MarketOpeningReadinessV1::decode(&readiness_data)
        .map_err(|_| AdapterError::VaultAuthentication)?;
    if readiness.to_bytes().as_slice() != &readiness_data[..]
        || readiness.sponsor_rent_refund() != frame.sponsor.key.as_ref()
    {
        return Err(AdapterError::ContentIdentity.into());
    }
    readiness
        .require_ready_for_open(
            frame.market.key.to_bytes(),
            instruction.generation(),
            manifest_id,
            manifest,
        )
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let rent_credit = authenticate_rent_credit(program_id, frame.rent_credit, frame.sponsor.key)?;
    let rent_credit_lamports = frame
        .rent_credit
        .try_lamports()
        .map_err(|_| AdapterError::VaultAuthentication)?;

    let generation_seed = instruction.generation().to_le_bytes();
    let (expected_readiness, _) = Pubkey::find_program_address(
        &[
            MARKET_OPENING_READINESS_PDA_DOMAIN,
            frame.market.key.as_ref(),
            generation_seed.as_slice(),
        ],
        program_id,
    );
    if frame.readiness.key != &expected_readiness {
        return Err(AdapterError::AccountIdentity.into());
    }

    let market_data = frame
        .market
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let outcome_count =
        decode_market_outcome_count(&market_data).map_err(|_| AdapterError::VaultAuthentication)?;
    validate_selected_market(
        program_id,
        frame.market.key,
        &market_data,
        realm_digest,
        manifest_digest,
        instruction,
    )?;
    drop(market_data);
    drop(readiness_data);
    drop(manifest_data);

    let (expected_custody, custody_bump) = Pubkey::find_program_address(
        &[COLLATERAL_CUSTODY_PDA_DOMAIN, frame.market.key.as_ref()],
        program_id,
    );
    let (expected_vault, vault_bump) = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, frame.market.key.as_ref()],
        program_id,
    );
    if frame.custody.key != &expected_custody || frame.vault.key != &expected_vault {
        return Err(AdapterError::AccountIdentity.into());
    }

    let rent = Rent::from_account_info(frame.rent_sysvar)
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let market_rent = rent.minimum_balance(frame.market.data_len());
    if frame.market.lamports() < market_rent {
        return Err(AdapterError::VaultAuthentication.into());
    }
    let custody_rent = rent.minimum_balance(COLLATERAL_CUSTODY_BYTES);
    let vault_rent = rent.minimum_balance(ACCOUNT_BYTES);
    let readiness_rent = rent.minimum_balance(MARKET_OPENING_READINESS_BYTES);
    if frame.readiness.lamports() != readiness_rent
        || frame.readiness.data_len() != MARKET_OPENING_READINESS_BYTES
    {
        return Err(AdapterError::VaultAuthentication.into());
    }
    let total_debit = custody_rent
        .checked_add(vault_rent)
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() < total_debit {
        return Err(AdapterError::FundUnderfunded.into());
    }
    frame
        .sponsor
        .lamports()
        .checked_sub(total_debit)
        .and_then(|value| value.checked_add(readiness_rent))
        .ok_or(AdapterError::Arithmetic)?;

    // Construct the exact token instruction before either account exists.
    initialize_vault_instruction(
        release,
        *frame.vault.key,
        *frame.mint.key,
        *frame.market.key,
    )?;
    preflight_mutable(frame.sponsor)?;
    preflight_mutable(frame.market)?;
    preflight_mutable(frame.readiness)?;
    preflight_mutable(frame.rent_credit)?;
    preflight_mutable(frame.custody)?;
    preflight_mutable(frame.vault)?;
    drop(
        frame
            .market
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::VaultAuthentication)?,
    );
    drop(
        frame
            .readiness
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::VaultAuthentication)?,
    );
    drop(
        frame
            .custody
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::VaultAuthentication)?,
    );
    drop(
        frame
            .vault
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::VaultAuthentication)?,
    );

    let mint_data = frame
        .mint
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let mint_digest = hash(&mint_data).to_bytes();
    drop(mint_data);
    Ok(OpenPlan {
        outcome_count,
        custody_bump,
        vault_bump,
        custody_rent,
        vault_rent,
        sponsor_before: frame.sponsor.lamports(),
        readiness_rent,
        rent_credit,
        rent_credit_lamports,
        market_lamports: frame.market.lamports(),
        mint_lamports: frame.mint.lamports(),
        mint_digest,
        release,
    })
}

fn authenticate_account_identities(
    program_id: &Pubkey,
    frame: &OpenFrame<'_, '_>,
) -> Result<(), ProgramError> {
    if frame.sponsor.owner != &system_program::ID
        || !frame.sponsor.data_is_empty()
        || frame.market.owner != program_id
        || frame.readiness.owner != program_id
        || frame.capability_manifest.owner != program_id
        || frame.realm.owner != program_id
        || frame.custody.owner != &system_program::ID
        || !frame.custody.data_is_empty()
        || frame.custody.lamports() != 0
        || frame.vault.owner != &system_program::ID
        || !frame.vault.data_is_empty()
        || frame.vault.lamports() != 0
        || frame.system_program.key != &system_program::ID
        || frame.system_program.owner != &native_loader::ID
        || frame.rent_sysvar.key != &sysvar::rent::ID
        || frame.rent_sysvar.owner != &sysvar::ID
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok(())
}

fn authenticate_realm(
    program_id: &Pubkey,
    frame: &OpenFrame<'_, '_>,
) -> Result<(RealmV1, [u8; 32]), ProgramError> {
    let realm_data = frame
        .realm
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| AdapterError::VaultAuthentication)?;
    let realm_digest = hash(&realm_data).to_bytes();
    if realm.to_bytes().as_slice() != &realm_data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    let (expected_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], program_id);
    if frame.realm.key != &expected_realm {
        return Err(AdapterError::AccountIdentity.into());
    }
    Ok((realm, realm_digest))
}

fn authenticate_token(
    frame: &OpenFrame<'_, '_>,
    realm: RealmV1,
) -> Result<CollateralAdapterReleaseV1, ProgramError> {
    let token_program = frame.token_program.key.to_bytes();
    if realm.token_program() != &token_program
        || realm.collateral_mint() != frame.mint.key.as_ref()
        || frame.mint.owner != frame.token_program.key
        || !recognized_program_loader(frame.token_program.owner)
    {
        return Err(AdapterError::VaultAuthentication.into());
    }
    let release = select_adapter_release(*realm.collateral_adapter_release_id())?;
    if release.token_program() != token_program {
        return Err(AdapterError::VaultAuthentication.into());
    }
    let mint_data = frame
        .mint
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let mint = release
        .profile()
        .check_mint(token_program, &mint_data)
        .map_err(|_| AdapterError::VaultAuthentication)?;
    require_authority_policy(realm.mint_authority_policy(), &mint.mint_authority)?;
    require_freeze_policy(realm.freeze_authority_policy(), &mint.freeze_authority)?;
    Ok(release)
}

fn persist_open(
    program_id: &Pubkey,
    frame: &OpenFrame<'_, '_>,
    instruction: OpenCollateralVaultV1,
    plan: OpenPlan,
) -> Result<(), ProgramError> {
    let sponsor_after_debits = plan
        .sponsor_before
        .checked_sub(plan.custody_rent)
        .and_then(|value| value.checked_sub(plan.vault_rent))
        .ok_or(AdapterError::Arithmetic)?;
    if frame.sponsor.lamports() != sponsor_after_debits
        || frame.market.lamports() != plan.market_lamports
        || frame.market.owner != program_id
        || frame.readiness.lamports() != plan.readiness_rent
        || frame.readiness.owner != program_id
        || frame.readiness.data_len() != MARKET_OPENING_READINESS_BYTES
        || frame.custody.lamports() != plan.custody_rent
        || frame.custody.owner != program_id
        || frame.custody.data_len() != COLLATERAL_CUSTODY_BYTES
        || frame.vault.lamports() != plan.vault_rent
        || frame.vault.owner != frame.token_program.key
        || frame.vault.data_len() != ACCOUNT_BYTES
        || frame.mint.lamports() != plan.mint_lamports
        || frame.mint.owner != frame.token_program.key
    {
        return Err(AdapterError::VaultPostcondition.into());
    }
    let mint_data = frame
        .mint
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultPostcondition)?;
    if hash(&mint_data).to_bytes() != plan.mint_digest {
        return Err(AdapterError::VaultPostcondition.into());
    }
    drop(mint_data);
    authenticate_initialized_vault(&plan, frame)?;

    let custody = CollateralCustodyV1::new(
        frame.market.key.to_bytes(),
        instruction.generation(),
        frame.sponsor.key.to_bytes(),
    )
    .map_err(|_| AdapterError::VaultPostcondition)?;
    let mut custody_data = frame
        .custody
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::VaultPostcondition)?;
    custody
        .encode(&mut custody_data)
        .map_err(|_| AdapterError::VaultPostcondition)?;
    if CollateralCustodyV1::decode(&custody_data) != Ok(custody) {
        return Err(AdapterError::VaultPostcondition.into());
    }
    drop(custody_data);

    let mut market_data = frame
        .market
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::VaultPostcondition)?;
    open_selected_market(plan.outcome_count, &mut market_data, instruction)?;
    drop(market_data);

    close_readiness(program_id, frame, plan.readiness_rent, plan.rent_credit)?;
    if frame.sponsor.lamports() != sponsor_after_debits
        || frame.rent_credit.lamports()
            != plan
                .rent_credit_lamports
                .checked_add(plan.readiness_rent)
                .ok_or(AdapterError::Arithmetic)?
        || frame.readiness.lamports() != 0
        || frame.readiness.owner != &system_program::ID
        || !frame
            .readiness
            .try_data_is_empty()
            .map_err(|_| AdapterError::VaultPostcondition)?
    {
        return Err(AdapterError::VaultPostcondition.into());
    }
    Ok(())
}

fn close_readiness(
    program_id: &Pubkey,
    frame: &OpenFrame<'_, '_>,
    readiness_rent: u64,
    rent_credit: RentCreditV1,
) -> Result<(), ProgramError> {
    if frame.readiness.owner != program_id
        || frame.readiness.lamports() != readiness_rent
        || frame.readiness.data_len() != MARKET_OPENING_READINESS_BYTES
    {
        return Err(AdapterError::VaultPostcondition.into());
    }
    let credit_plan = SourceCloseCreditPlanV1::new(
        frame.readiness.lamports(),
        frame.rent_credit.lamports(),
        readiness_rent,
    )
    .map_err(|_| AdapterError::Arithmetic)?;
    {
        let mut credit_lamports = frame
            .rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::VaultPostcondition)?;
        let mut readiness_lamports = frame
            .readiness
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::VaultPostcondition)?;
        **credit_lamports = credit_plan.credit_after();
        **readiness_lamports = 0;
    }
    frame
        .readiness
        .resize(0)
        .map_err(|_| AdapterError::VaultPostcondition)?;
    frame.readiness.assign(&system_program::ID);
    credit_plan
        .validate_post(frame.readiness.lamports(), frame.rent_credit.lamports())
        .map_err(|_| AdapterError::VaultPostcondition)?;
    require_unchanged_rent_credit(program_id, frame.rent_credit, rent_credit)?;
    Ok(())
}

fn authenticate_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    authority_key: &Pubkey,
) -> Result<RentCreditV1, ProgramError> {
    let authority = RefundAuthority::new(authority_key.to_bytes())
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let authority_bytes = authority.to_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, authority_bytes.as_slice()],
        program_id,
    );
    if account.key != &expected
        || account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::AccountIdentity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::VaultAuthentication)?;
    credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::VaultAuthentication)?;
    if credit.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::ContentIdentity.into());
    }
    Ok(credit)
}

fn require_unchanged_rent_credit(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: RentCreditV1,
) -> Result<(), ProgramError> {
    if account.owner != program_id
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
    {
        return Err(AdapterError::VaultPostcondition.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultPostcondition)?;
    if RentCreditV1::decode(&data) != Ok(expected) || expected.to_bytes().as_slice() != &data[..] {
        return Err(AdapterError::VaultPostcondition.into());
    }
    Ok(())
}

fn authenticate_initialized_vault(
    plan: &OpenPlan,
    frame: &OpenFrame<'_, '_>,
) -> Result<(), ProgramError> {
    let vault_data = frame
        .vault
        .try_borrow_data()
        .map_err(|_| AdapterError::VaultPostcondition)?;
    let account = plan
        .release
        .profile()
        .check_custody_account(
            frame.token_program.key.to_bytes(),
            &vault_data,
            frame.mint.key.to_bytes(),
            frame.market.key.to_bytes(),
        )
        .map_err(|_| AdapterError::VaultPostcondition)?;
    if account.amount != 0 {
        return Err(AdapterError::VaultPostcondition.into());
    }
    Ok(())
}

fn initialize_vault_instruction(
    release: CollateralAdapterReleaseV1,
    vault: Pubkey,
    mint: Pubkey,
    market_authority: Pubkey,
) -> Result<Instruction, ProgramError> {
    let spec = initialize_account3(
        release.token_program(),
        vault.to_bytes(),
        mint.to_bytes(),
        market_authority.to_bytes(),
    )
    .map_err(|_| AdapterError::VaultAuthentication)?;
    if spec.program_id() != &release.token_program()
        || spec.accounts().first().map(|meta| meta.address()) != Some(&vault.to_bytes())
        || spec.accounts().get(1).map(|meta| meta.address()) != Some(&mint.to_bytes())
    {
        return Err(AdapterError::VaultAuthentication.into());
    }
    let mut accounts = Vec::new();
    accounts
        .try_reserve_exact(2)
        .map_err(|_| AdapterError::Arithmetic)?;
    accounts.push(AccountMeta::new(vault, false));
    accounts.push(AccountMeta::new_readonly(mint, false));
    Ok(Instruction {
        program_id: Pubkey::new_from_array(release.token_program()),
        accounts,
        data: Vec::from(*spec.data()),
    })
}

type MarketValidator = fn(
    &Pubkey,
    &Pubkey,
    &[u8],
    [u8; 32],
    [u8; 32],
    OpenCollateralVaultV1,
) -> Result<(), ProgramError>;
type MarketOpener = fn(&mut [u8], OpenCollateralVaultV1) -> Result<(), ProgramError>;

const MARKET_VALIDATORS: [MarketValidator; 15] = [
    validate_market::<2>,
    validate_market::<3>,
    validate_market::<4>,
    validate_market::<5>,
    validate_market::<6>,
    validate_market::<7>,
    validate_market::<8>,
    validate_market::<9>,
    validate_market::<10>,
    validate_market::<11>,
    validate_market::<12>,
    validate_market::<13>,
    validate_market::<14>,
    validate_market::<15>,
    validate_market::<16>,
];
const MARKET_OPENERS: [MarketOpener; 15] = [
    open_market::<2>,
    open_market::<3>,
    open_market::<4>,
    open_market::<5>,
    open_market::<6>,
    open_market::<7>,
    open_market::<8>,
    open_market::<9>,
    open_market::<10>,
    open_market::<11>,
    open_market::<12>,
    open_market::<13>,
    open_market::<14>,
    open_market::<15>,
    open_market::<16>,
];

fn validate_selected_market(
    program_id: &Pubkey,
    market_key: &Pubkey,
    bytes: &[u8],
    realm_digest: [u8; 32],
    manifest_digest: [u8; 32],
    instruction: OpenCollateralVaultV1,
) -> Result<(), ProgramError> {
    let outcome_count =
        decode_market_outcome_count(bytes).map_err(|_| AdapterError::VaultAuthentication)?;
    let validator = MARKET_VALIDATORS
        .get(outcome_index(outcome_count)?)
        .copied()
        .ok_or(AdapterError::VaultAuthentication)?;
    validator(
        program_id,
        market_key,
        bytes,
        realm_digest,
        manifest_digest,
        instruction,
    )
}

fn open_selected_market(
    outcome_count: u8,
    bytes: &mut [u8],
    instruction: OpenCollateralVaultV1,
) -> Result<(), ProgramError> {
    let opener = MARKET_OPENERS
        .get(outcome_index(outcome_count)?)
        .copied()
        .ok_or(AdapterError::VaultPostcondition)?;
    opener(bytes, instruction)
}

fn validate_market<const N: usize>(
    program_id: &Pubkey,
    market_key: &Pubkey,
    bytes: &[u8],
    realm_digest: [u8; 32],
    manifest_digest: [u8; 32],
    instruction: OpenCollateralVaultV1,
) -> Result<(), ProgramError> {
    let market =
        CategoricalMarketV1::<N>::decode(bytes).map_err(|_| AdapterError::VaultAuthentication)?;
    let root = market.root();
    if root.phase() != RootPhase::Founding
        || root.identity().generation() != instruction.generation()
        || root.outstanding_children() != instruction.child_count()
        || root.identity().realm_id().to_bytes() != realm_digest
        || root.identity().capability_manifest_id().to_bytes() != manifest_digest
    {
        return Err(AdapterError::VaultAuthentication.into());
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], program_id);
    if market_key != &expected_market {
        return Err(AdapterError::AccountIdentity.into());
    }
    candidate_open_market(market, instruction)?;
    Ok(())
}

fn open_market<const N: usize>(
    bytes: &mut [u8],
    instruction: OpenCollateralVaultV1,
) -> Result<(), ProgramError> {
    let market =
        CategoricalMarketV1::<N>::decode(bytes).map_err(|_| AdapterError::VaultPostcondition)?;
    let opened = candidate_open_market(market, instruction)?;
    opened
        .encode(bytes)
        .map_err(|_| AdapterError::VaultPostcondition)?;
    let persisted =
        CategoricalMarketV1::<N>::decode(bytes).map_err(|_| AdapterError::VaultPostcondition)?;
    if persisted != opened
        || persisted.root().phase() != RootPhase::Open
        || persisted.root().outstanding_children() != OPENED_CHILD_COUNT
    {
        return Err(AdapterError::VaultPostcondition.into());
    }
    Ok(())
}

fn candidate_open_market<const N: usize>(
    market: CategoricalMarketV1<N>,
    instruction: OpenCollateralVaultV1,
) -> Result<CategoricalMarketV1<N>, ProgramError> {
    let mut opened = market;
    opened
        .retire_child(instruction.generation(), instruction.child_count())
        .map_err(|_| AdapterError::MarketTransition)?;
    opened
        .register_child(instruction.generation(), REQUIRED_FUND_CHILD_COUNT)
        .map_err(|_| AdapterError::MarketTransition)?;
    opened
        .transition_phase(instruction.generation(), RootPhase::Open)
        .map_err(|_| AdapterError::MarketTransition)?;
    Ok(opened)
}

fn outcome_index(outcome_count: u8) -> Result<usize, ProgramError> {
    if !(MIN_OUTCOMES..=MAX_OUTCOMES).contains(&outcome_count) {
        return Err(AdapterError::VaultAuthentication.into());
    }
    Ok(usize::from(outcome_count.saturating_sub(MIN_OUTCOMES)))
}

fn preflight_mutable(account: &AccountInfo<'_>) -> Result<(), ProgramError> {
    drop(
        account
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::VaultAuthentication)?,
    );
    Ok(())
}

fn privilege(account: &AccountInfo<'_>) -> AccountPrivilege {
    AccountPrivilege {
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn require_distinct(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(AdapterError::AccountIdentity.into());
        }
    }
    Ok(())
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
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, FundingQuoteV1,
        MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_core_contract::{MarketIdentity, MarketRoot};
    use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
    use dclutch_pyth_contract::funding::{FUNDING_BYTES, construct_required_resolution_funding};
    use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1Input};
    use dclutch_token_svm::{
        LEGACY_TOKEN_PROGRAM_ID, PRODUCTION_ADAPTER_RELEASES, TOKEN_2022_PROGRAM_ID,
        state::MINT_BYTES,
    };
    use solana_sdk_ids::bpf_loader;
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

    const GENERATION: u64 = 7;
    const POLICY_ID: [u8; 32] = [4; 32];

    struct Fixture {
        program_id: Pubkey,
        instruction: OpenCollateralVaultV1,
        accounts: Vec<AccountInfo<'static>>,
    }

    impl Fixture {
        fn new(token_program: [u8; 32], ready: bool) -> Self {
            let program_id = Pubkey::new_unique();
            let sponsor_key = Pubkey::new_unique();
            let mint_key = Pubkey::new_unique();
            let release = PRODUCTION_ADAPTER_RELEASES
                .iter()
                .find(|candidate| candidate.token_program() == token_program)
                .copied()
                .expect("production token release");
            let realm = RealmV1::new(RealmV1Input {
                token_program,
                collateral_mint: mint_key.to_bytes(),
                collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
                mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
                freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
            })
            .expect("Realm");
            let realm_bytes = realm.to_bytes();
            let realm_digest = hash(&realm_bytes).to_bytes();
            let (realm_key, _) =
                Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], &program_id);

            let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
            let entry = CapabilityEntryV1::new(
                core_id([11; 32]),
                core_id([12; 32]),
                core_id(POLICY_ID),
                core_id([13; 32]),
                core_id([14; 32]),
                core_id([15; 32]),
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                FundingQuoteV1::new(fund_rent, 0, 0, 3, 5, 0, 0).expect("quote"),
            )
            .expect("entry");
            let mut manifest_bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
            let manifest =
                CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes).expect("manifest");
            let manifest_id = core_id(hash(manifest.as_bytes()).to_bytes());
            let identity = MarketIdentity::new(
                core_id(realm_digest),
                core_id([2; 32]),
                core_id([3; 32]),
                core_id(POLICY_ID),
                manifest_id,
                GENERATION,
            );
            let identity_digest = hash(&identity.to_bytes()).to_bytes();
            let (market_key, _) =
                Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], &program_id);

            let selected = manifest
                .required_founding_entry_for_config(core_id(POLICY_ID))
                .expect("selected");
            let funding = construct_required_resolution_funding(
                manifest_id,
                manifest,
                selected,
                fund_rent,
                44,
            )
            .expect("funding");
            let mut readiness = MarketOpeningReadinessV1::begin(
                market_key.to_bytes(),
                GENERATION,
                manifest_id,
                manifest,
                sponsor_key.to_bytes(),
            )
            .expect("readiness");
            if ready {
                readiness
                    .advance(
                        market_key.to_bytes(),
                        GENERATION,
                        manifest_id,
                        manifest,
                        0,
                        funding,
                        funding.remaining().total_principal(),
                        44,
                    )
                    .expect("ready");
            }
            let generation_seed = GENERATION.to_le_bytes();
            let (readiness_key, _) = Pubkey::find_program_address(
                &[
                    MARKET_OPENING_READINESS_PDA_DOMAIN,
                    market_key.as_ref(),
                    generation_seed.as_slice(),
                ],
                &program_id,
            );
            let (custody_key, _) = Pubkey::find_program_address(
                &[COLLATERAL_CUSTODY_PDA_DOMAIN, market_key.as_ref()],
                &program_id,
            );
            let (vault_key, _) = Pubkey::find_program_address(
                &[COLLATERAL_VAULT_PDA_DOMAIN, market_key.as_ref()],
                &program_id,
            );
            let (rent_credit_key, rent_credit_bump) = Pubkey::find_program_address(
                &[RENT_CREDIT_PDA_DOMAIN_V1, sponsor_key.as_ref()],
                &program_id,
            );
            let rent_credit = RentCreditV1::new(
                RefundAuthority::new(sponsor_key.to_bytes()).expect("authority"),
                rent_credit_bump,
            );

            let mut root = MarketRoot::founding(identity, sponsor_key.to_bytes()).expect("root");
            root.register_child(GENERATION, 0).expect("Fund child");
            root.register_child(GENERATION, 1).expect("readiness child");
            let market = CategoricalMarketV1::<2>::new(
                root,
                0,
                [0; 2],
                CategoricalSettlementSummaryV1::empty(),
            )
            .expect("Market");
            let mut market_bytes = vec![0; CategoricalMarketV1::<2>::encoded_len().expect("width")];
            market.encode(&mut market_bytes).expect("Market bytes");

            let mut accounts = vec![
                leak_account(
                    sponsor_key,
                    true,
                    true,
                    100_000_000,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(
                    market_key,
                    false,
                    true,
                    Rent::default().minimum_balance(market_bytes.len()),
                    market_bytes,
                    program_id,
                    false,
                ),
                leak_account(
                    readiness_key,
                    false,
                    true,
                    Rent::default().minimum_balance(MARKET_OPENING_READINESS_BYTES),
                    readiness.to_bytes().to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    rent_credit_key,
                    false,
                    true,
                    1,
                    rent_credit.to_bytes().to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    Pubkey::new_unique(),
                    false,
                    false,
                    1,
                    manifest_bytes,
                    program_id,
                    false,
                ),
                leak_account(
                    realm_key,
                    false,
                    false,
                    1,
                    realm_bytes.to_vec(),
                    program_id,
                    false,
                ),
                leak_account(
                    custody_key,
                    false,
                    true,
                    0,
                    vec![],
                    system_program::ID,
                    false,
                ),
                leak_account(vault_key, false, true, 0, vec![], system_program::ID, false),
                leak_account(
                    mint_key,
                    false,
                    false,
                    1,
                    mint_bytes(),
                    Pubkey::new_from_array(token_program),
                    false,
                ),
                leak_account(
                    Pubkey::new_from_array(token_program),
                    false,
                    false,
                    1,
                    vec![],
                    bpf_loader::ID,
                    true,
                ),
                leak_account(
                    system_program::ID,
                    false,
                    false,
                    1,
                    vec![],
                    native_loader::ID,
                    true,
                ),
                leak_account(
                    sysvar::rent::ID,
                    false,
                    false,
                    1,
                    vec![0; Rent::size_of()],
                    sysvar::ID,
                    false,
                ),
            ];
            let rent = accounts.get_mut(11).expect("rent");
            assert_eq!(Rent::default().to_account_info(rent), Some(()));
            Self {
                program_id,
                instruction: OpenCollateralVaultV1::new(GENERATION, 2),
                accounts,
            }
        }

        fn authenticate(&self) -> Result<OpenPlan, ProgramError> {
            let frame = OpenFrame::parse(&self.accounts)?;
            authenticate_open(&self.program_id, &frame, self.instruction)
        }
    }

    fn core_id(bytes: [u8; 32]) -> ContentId {
        ContentId::new(bytes).expect("content ID")
    }

    fn mint_bytes() -> Vec<u8> {
        let mut bytes = vec![0; MINT_BYTES];
        *bytes.get_mut(44).expect("decimals") = 6;
        *bytes.get_mut(45).expect("initialized") = 1;
        bytes
    }

    fn leak_account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        lamports: u64,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn account(fixture: &Fixture, index: usize) -> &AccountInfo<'static> {
        fixture.accounts.get(index).expect("fixture account")
    }

    fn account_mut(fixture: &mut Fixture, index: usize) -> &mut AccountInfo<'static> {
        fixture.accounts.get_mut(index).expect("fixture account")
    }

    #[test]
    fn both_token_profiles_authenticate_exact_ready_opening() {
        for token_program in [LEGACY_TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID] {
            let fixture = Fixture::new(token_program, true);
            let plan = fixture.authenticate().expect("authenticated Open");
            assert_eq!(plan.outcome_count, 2);
            assert_eq!(plan.readiness_rent, Rent::default().minimum_balance(128));
            let instruction = initialize_vault_instruction(
                plan.release,
                *account(&fixture, 7).key,
                *account(&fixture, 8).key,
                *account(&fixture, 1).key,
            )
            .expect("initialize instruction");
            assert_eq!(instruction.program_id.to_bytes(), token_program);
            assert_eq!(instruction.accounts.len(), 2);
        }
    }

    #[test]
    fn hostile_frame_privilege_alias_and_destination_state_refuse() {
        let canonical = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        assert_eq!(
            OpenFrame::parse(canonical.accounts.get(..10).expect("prefix")).err(),
            Some(ProgramError::from(AdapterError::AccountFrameLength))
        );

        let mut missing_signer = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        account_mut(&mut missing_signer, 0).is_signer = false;
        assert_eq!(
            missing_signer.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountPrivilege))
        );

        let mut aliased = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        *account_mut(&mut aliased, 7) = account(&aliased, 6).clone();
        assert_eq!(
            aliased.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );

        let existing = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        **account(&existing, 6)
            .try_borrow_mut_lamports()
            .expect("custody lamports") = 1;
        assert_eq!(
            existing.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );
    }

    #[test]
    fn incomplete_readiness_wrong_pda_manifest_and_replay_refuse() {
        assert_eq!(
            Fixture::new(LEGACY_TOKEN_PROGRAM_ID, false)
                .authenticate()
                .err(),
            Some(ProgramError::from(AdapterError::VaultAuthentication))
        );

        let mut wrong_pda = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        let readiness = account(&wrong_pda, 2).clone();
        *account_mut(&mut wrong_pda, 2) = leak_account(
            Pubkey::new_unique(),
            false,
            true,
            readiness.lamports(),
            readiness.try_borrow_data().expect("readiness").to_vec(),
            wrong_pda.program_id,
            false,
        );
        assert_eq!(
            wrong_pda.authenticate().err(),
            Some(ProgramError::from(AdapterError::AccountIdentity))
        );

        let malformed = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        *account(&malformed, 3)
            .try_borrow_mut_data()
            .expect("manifest")
            .get_mut(0)
            .expect("magic") ^= 1;
        assert_eq!(
            malformed.authenticate().err(),
            Some(ProgramError::from(AdapterError::VaultAuthentication))
        );

        let mut replay = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        replay.instruction = OpenCollateralVaultV1::new(GENERATION, 1);
        assert_eq!(
            replay.authenticate().err(),
            Some(ProgramError::from(AdapterError::ReplayMismatch))
        );
    }

    #[test]
    fn exact_child_replacement_opens_without_changing_economics() {
        let fixture = Fixture::new(LEGACY_TOKEN_PROGRAM_ID, true);
        let market_data = account(&fixture, 1).try_borrow_data().expect("Market");
        let market = CategoricalMarketV1::<2>::decode(&market_data).expect("Market");
        let opened = candidate_open_market(market, fixture.instruction).expect("opened");
        assert_eq!(opened.root().phase(), RootPhase::Open);
        assert_eq!(opened.root().outstanding_children(), 2);
        assert_eq!(opened.hoard_atoms(), 0);
        assert_eq!(opened.supply(), &[0; 2]);
        assert!(opened.settlement().is_empty());
    }
}
