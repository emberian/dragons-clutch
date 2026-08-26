#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Small SVM refinement of the canonical permanent RentCredit contract.
//!
//! The SDK-free `dclutch-rent-contract` remains the sole owner of instruction
//! bytes, account roles, PDA seed material, and exact balance arithmetic. This
//! adapter owns only SVM account observations, current Rent, PDA derivation,
//! the System create CPI, and commit/post-observation checks. RentCredit has no
//! close action.

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryFrom;

use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_rent_contract::{
    AccountMetaV1, CreateBalancePlanV1, CreateRentCreditFrameV1, CreateRentCreditV1,
    Error as ContractError, RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority,
    RentCreditInstructionV1, RentCreditV1, SystemWalletFactsV1, WithdrawBalancePlanV1,
    WithdrawRentCreditFrameV1, WithdrawRentCreditV1,
    lifecycle_v2::{
        CLOSE_LIFECYCLE_RENT_CREDIT_BYTES_V2, CloseLifecycleRentCreditV2,
        CreateLifecycleRentCreditV2, LIFECYCLE_RENT_CORE_CLOSE_AUTHORITY_DOMAIN_V2,
        LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2, LifecycleAccountIdV2,
        LifecycleAccountMetaV2, LifecycleClosePlanV2, LifecycleRentCreditV2,
        LifecycleRentInstructionV2, LifecycleSweepPlanV2, SweepLifecycleRentCreditV2,
        validate_refund_wallet_v2, validate_sweep_frame_v2,
    },
};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

/// Exact number of accounts in either V1 action frame.
pub const ACCOUNT_COUNT_V1: usize = 4;

/// Stable refusal from the successor RentCredit SBF adapter.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RentSbfError {
    /// Instruction bytes did not decode under the canonical contract.
    Instruction = 0,
    /// Account count, order, privileges, aliases, or wallet facts refused.
    AccountFrame = 1,
    /// System Program or Rent sysvar identity/value refused.
    RuntimeAccount = 2,
    /// RentCredit PDA, owner, vacancy, or immutable binding refused.
    RentCredit = 3,
    /// Exact checked contract balance plan refused.
    Balance = 4,
    /// System Program refused exact PDA creation.
    SystemCpi = 5,
    /// A completed physical effect did not have its exact planned poststate.
    Postcondition = 6,
    /// A required account borrow was unavailable.
    Borrow = 7,
    /// Registry refused the current Core deployment or returned other bytes.
    Release = 8,
    /// Current Core did not sign with the exact lifecycle-close authority.
    Caller = 9,
    /// Complete retirement evidence or the final close plan refused.
    Closure = 10,
}

impl From<RentSbfError> for ProgramError {
    fn from(value: RentSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Decode and execute one exact canonical Create or Withdraw request.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.get(..8) == Some(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2.as_slice()) {
        return match LifecycleRentInstructionV2::decode(instruction_data)
            .map_err(|_| RentSbfError::Instruction)?
        {
            LifecycleRentInstructionV2::Create => process_create_v2(
                program_id,
                accounts,
                CreateLifecycleRentCreditV2::decode(instruction_data)
                    .map_err(|_| RentSbfError::Instruction)?,
            ),
            LifecycleRentInstructionV2::Sweep => process_sweep_v2(
                program_id,
                accounts,
                SweepLifecycleRentCreditV2::decode(instruction_data)
                    .map_err(|_| RentSbfError::Instruction)?,
            ),
            LifecycleRentInstructionV2::Close => process_close_v2(
                program_id,
                accounts,
                CloseLifecycleRentCreditV2::decode(instruction_data)
                    .map_err(|_| RentSbfError::Instruction)?,
            ),
        };
    }
    match RentCreditInstructionV1::decode(instruction_data)
        .map_err(|_| RentSbfError::Instruction)?
    {
        RentCreditInstructionV1::Create(request) => process_create(program_id, accounts, request),
        RentCreditInstructionV1::Withdraw(request) => {
            process_withdraw(program_id, accounts, request)
        }
    }
}

struct CreateAccounts<'a, 'info> {
    payer: &'a AccountInfo<'info>,
    credit: &'a AccountInfo<'info>,
    system: &'a AccountInfo<'info>,
    rent: &'a AccountInfo<'info>,
}

impl<'a, 'info> CreateAccounts<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        require_count(accounts)?;
        let frame = Self {
            payer: account(accounts, 0)?,
            credit: account(accounts, 1)?,
            system: account(accounts, 2)?,
            rent: account(accounts, 3)?,
        };
        CreateRentCreditFrameV1::new([
            meta(frame.payer),
            meta(frame.credit),
            meta(frame.system),
            meta(frame.rent),
        ])
        .map_err(map_frame_error)?;
        Ok(frame)
    }
}

struct WithdrawAccounts<'a, 'info> {
    credit: &'a AccountInfo<'info>,
    authority: &'a AccountInfo<'info>,
    recipient: &'a AccountInfo<'info>,
    rent: &'a AccountInfo<'info>,
}

impl<'a, 'info> WithdrawAccounts<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        require_count(accounts)?;
        let frame = Self {
            credit: account(accounts, 0)?,
            authority: account(accounts, 1)?,
            recipient: account(accounts, 2)?,
            rent: account(accounts, 3)?,
        };
        let recipient_data_len =
            u64::try_from(frame.recipient.data_len()).map_err(|_| RentSbfError::AccountFrame)?;
        let recipient = SystemWalletFactsV1::new(
            frame.recipient.owner == &system_program::ID,
            recipient_data_len,
        )
        .map_err(|_| RentSbfError::AccountFrame)?;
        WithdrawRentCreditFrameV1::new(
            [
                meta(frame.credit),
                meta(frame.authority),
                meta(frame.recipient),
                meta(frame.rent),
            ],
            recipient,
        )
        .map_err(map_frame_error)?;
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
struct CreatePlan {
    state: RentCreditV1,
    rent_minimum: u64,
    balances: CreateBalancePlanV1,
}

/// Create exactly one permanent authority-derived RentCredit PDA.
#[inline(never)]
fn process_create(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CreateRentCreditV1,
) -> ProgramResult {
    let accounts = CreateAccounts::parse(accounts)?;
    let plan = prepare_create(program_id, &accounts, request)?;

    let space = u64::try_from(RENT_CREDIT_BYTES_V1).map_err(|_| RentSbfError::Balance)?;
    let instruction = create_account(
        accounts.payer.key,
        accounts.credit.key,
        plan.rent_minimum,
        space,
        program_id,
    );
    let seeds = plan.state.pda_seeds();
    let authority = seeds.refund_authority().to_bytes();
    let bump = [seeds.bump()];
    let signer = [seeds.domain(), authority.as_slice(), bump.as_slice()];
    let derived = Pubkey::create_program_address(&signer, program_id)
        .map_err(|_| RentSbfError::RentCredit)?;
    if &derived != accounts.credit.key {
        return Err(RentSbfError::RentCredit.into());
    }
    invoke_signed(
        &instruction,
        &[
            accounts.payer.clone(),
            accounts.credit.clone(),
            accounts.system.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| RentSbfError::SystemCpi)?;

    plan.balances
        .validate_post(accounts.payer.lamports(), accounts.credit.lamports())
        .map_err(|_| RentSbfError::Postcondition)?;
    if accounts.credit.owner != program_id
        || accounts.credit.data_len() != RENT_CREDIT_BYTES_V1
        || accounts.credit.executable
    {
        return Err(RentSbfError::Postcondition.into());
    }
    let expected = plan.state.to_bytes();
    {
        let mut data = accounts
            .credit
            .try_borrow_mut_data()
            .map_err(|_| RentSbfError::Borrow)?;
        if data.len() != expected.len() {
            return Err(RentSbfError::Postcondition.into());
        }
        data.copy_from_slice(&expected);
    }
    let data = accounts
        .credit
        .try_borrow_data()
        .map_err(|_| RentSbfError::Borrow)?;
    if RentCreditV1::decode(&data) != Ok(plan.state) || data.as_ref() != expected {
        return Err(RentSbfError::Postcondition.into());
    }
    Ok(())
}

#[inline(never)]
fn prepare_create(
    program_id: &Pubkey,
    accounts: &CreateAccounts<'_, '_>,
    request: CreateRentCreditV1,
) -> Result<CreatePlan, ProgramError> {
    let rent = authenticate_system_and_rent(accounts.system, accounts.rent)?;
    if accounts.payer.owner != &system_program::ID
        || !accounts.payer.data_is_empty()
        || accounts.credit.owner != &system_program::ID
        || !accounts.credit.data_is_empty()
        || accounts.credit.lamports() != 0
    {
        return Err(RentSbfError::RentCredit.into());
    }
    let authority = request.refund_authority().to_bytes();
    let (expected, bump) =
        Pubkey::find_program_address(&[RENT_CREDIT_PDA_DOMAIN_V1, &authority], program_id);
    if accounts.credit.key != &expected || request.pda_bump() != bump {
        return Err(RentSbfError::RentCredit.into());
    }
    let rent_minimum = rent.minimum_balance(RENT_CREDIT_BYTES_V1);
    let balances = CreateBalancePlanV1::new(
        accounts.payer.lamports(),
        accounts.credit.lamports(),
        rent_minimum,
    )
    .map_err(|_| RentSbfError::Balance)?;

    // Refuse unavailable commit targets before the irreversible CPI boundary.
    drop(
        accounts
            .payer
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .credit
            .try_borrow_mut_data()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    Ok(CreatePlan {
        state: request.credit(),
        rent_minimum,
        balances,
    })
}

/// Withdraw one authority-approved exact surplus while preserving current Rent.
#[inline(never)]
fn process_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: WithdrawRentCreditV1,
) -> ProgramResult {
    let accounts = WithdrawAccounts::parse(accounts)?;
    let plan = prepare_withdraw(program_id, &accounts, request)?;
    {
        let mut credit = accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?;
        let mut recipient = accounts
            .recipient
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?;
        **credit = plan.credit_after();
        **recipient = plan.recipient_after();
    }
    plan.validate_post(accounts.credit.lamports(), accounts.recipient.lamports())
        .map_err(|_| RentSbfError::Postcondition)?;
    Ok(())
}

#[inline(never)]
fn prepare_withdraw(
    program_id: &Pubkey,
    accounts: &WithdrawAccounts<'_, '_>,
    request: WithdrawRentCreditV1,
) -> Result<WithdrawBalancePlanV1, ProgramError> {
    let rent = authenticate_rent(accounts.rent)?;
    if accounts.credit.owner != program_id
        || accounts.credit.data_len() != RENT_CREDIT_BYTES_V1
        || accounts.credit.executable
    {
        return Err(RentSbfError::RentCredit.into());
    }
    let data = accounts
        .credit
        .try_borrow_data()
        .map_err(|_| RentSbfError::Borrow)?;
    let state = RentCreditV1::decode(&data).map_err(|_| RentSbfError::RentCredit)?;
    drop(data);
    let authority = RefundAuthority::new(accounts.authority.key.to_bytes())
        .map_err(|_| RentSbfError::RentCredit)?;
    let authority_bytes = authority.to_bytes();
    let (expected, bump) =
        Pubkey::find_program_address(&[RENT_CREDIT_PDA_DOMAIN_V1, &authority_bytes], program_id);
    if accounts.credit.key != &expected {
        return Err(RentSbfError::RentCredit.into());
    }
    state
        .validate_binding(authority, bump)
        .map_err(|_| RentSbfError::RentCredit)?;
    let plan = WithdrawBalancePlanV1::new(
        accounts.credit.lamports(),
        accounts.recipient.lamports(),
        rent.minimum_balance(RENT_CREDIT_BYTES_V1),
        request,
    )
    .map_err(|_| RentSbfError::Balance)?;
    drop(
        accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .recipient
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    Ok(plan)
}

const SWEEP_ACCOUNT_COUNT_V2: usize = 3;
const CLOSE_ACCOUNT_COUNT_V2: usize = 7;

#[derive(Clone, Copy)]
struct CreatePlanV2 {
    state: LifecycleRentCreditV2,
    rent_minimum: u64,
    balances: CreateBalancePlanV1,
}

/// Create one Market-generation-scoped lifecycle RentCredit PDA.
#[inline(never)]
fn process_create_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CreateLifecycleRentCreditV2,
) -> ProgramResult {
    let accounts = CreateAccounts::parse(accounts)?;
    let plan = prepare_create_v2(program_id, &accounts, request)?;
    let state = plan.state;
    let seeds = state.pda_seeds();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let signer = [
        seeds.domain(),
        market.as_slice(),
        generation.as_slice(),
        bump.as_slice(),
    ];
    let space = u64::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).map_err(|_| RentSbfError::Balance)?;
    let instruction = create_account(
        accounts.payer.key,
        accounts.credit.key,
        plan.rent_minimum,
        space,
        program_id,
    );
    invoke_signed(
        &instruction,
        &[
            accounts.payer.clone(),
            accounts.credit.clone(),
            accounts.system.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| RentSbfError::SystemCpi)?;
    plan.balances
        .validate_post(accounts.payer.lamports(), accounts.credit.lamports())
        .map_err(|_| RentSbfError::Postcondition)?;
    if accounts.credit.owner != program_id
        || accounts.credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || accounts.credit.executable
    {
        return Err(RentSbfError::Postcondition.into());
    }
    let expected = state.to_bytes();
    {
        let mut data = accounts
            .credit
            .try_borrow_mut_data()
            .map_err(|_| RentSbfError::Borrow)?;
        if data.len() != expected.len() {
            return Err(RentSbfError::Postcondition.into());
        }
        data.copy_from_slice(&expected);
    }
    let data = accounts
        .credit
        .try_borrow_data()
        .map_err(|_| RentSbfError::Borrow)?;
    if LifecycleRentCreditV2::decode(&data) != Ok(state) || data.as_ref() != expected {
        return Err(RentSbfError::Postcondition.into());
    }
    Ok(())
}

#[inline(never)]
fn prepare_create_v2(
    program_id: &Pubkey,
    accounts: &CreateAccounts<'_, '_>,
    request: CreateLifecycleRentCreditV2,
) -> Result<CreatePlanV2, ProgramError> {
    let rent = authenticate_system_and_rent(accounts.system, accounts.rent)?;
    if accounts.payer.owner != &system_program::ID
        || !accounts.payer.data_is_empty()
        || accounts.credit.owner != &system_program::ID
        || !accounts.credit.data_is_empty()
        || accounts.credit.lamports() != 0
    {
        return Err(RentSbfError::RentCredit.into());
    }
    let state = request.state();
    let seeds = state.pda_seeds();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let (expected, bump) = Pubkey::find_program_address(
        &[seeds.domain(), market.as_slice(), generation.as_slice()],
        program_id,
    );
    if accounts.credit.key != &expected || state.pda_bump() != bump {
        return Err(RentSbfError::RentCredit.into());
    }
    let rent_minimum = rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2);
    let balances = CreateBalancePlanV1::new(
        accounts.payer.lamports(),
        accounts.credit.lamports(),
        rent_minimum,
    )
    .map_err(|_| RentSbfError::Balance)?;
    drop(
        accounts
            .payer
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .credit
            .try_borrow_mut_data()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    Ok(CreatePlanV2 {
        state,
        rent_minimum,
        balances,
    })
}

struct SweepAccountsV2<'a, 'info> {
    credit: &'a AccountInfo<'info>,
    wallet: &'a AccountInfo<'info>,
    rent: &'a AccountInfo<'info>,
}

impl<'a, 'info> SweepAccountsV2<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != SWEEP_ACCOUNT_COUNT_V2 {
            return Err(RentSbfError::AccountFrame.into());
        }
        Ok(Self {
            credit: account(accounts, 0)?,
            wallet: account(accounts, 1)?,
            rent: account(accounts, 2)?,
        })
    }
}

/// Sweep exact surplus only to the immutable lifecycle refund wallet.
#[inline(never)]
fn process_sweep_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: SweepLifecycleRentCreditV2,
) -> ProgramResult {
    let accounts = SweepAccountsV2::parse(accounts)?;
    let rent = authenticate_rent(accounts.rent)?;
    let state = authenticate_lifecycle_credit_v2(program_id, accounts.credit)?;
    let wallet_data_len =
        u64::try_from(accounts.wallet.data_len()).map_err(|_| RentSbfError::AccountFrame)?;
    validate_refund_wallet_v2(accounts.wallet.owner.to_bytes(), wallet_data_len)
        .map_err(|_| RentSbfError::AccountFrame)?;
    validate_sweep_frame_v2(
        state,
        meta_v2(accounts.credit),
        meta_v2(accounts.wallet),
        meta_v2(accounts.rent),
    )
    .map_err(|_| RentSbfError::AccountFrame)?;
    let plan = LifecycleSweepPlanV2::new(
        accounts.credit.lamports(),
        accounts.wallet.lamports(),
        rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2),
        request,
    )
    .map_err(|_| RentSbfError::Balance)?;
    drop(
        accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .wallet
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    {
        let mut credit = accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?;
        let mut wallet = accounts
            .wallet
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?;
        **credit = plan.credit_after();
        **wallet = plan.wallet_after();
    }
    if accounts.credit.lamports() != plan.credit_after()
        || accounts.wallet.lamports() != plan.wallet_after()
    {
        return Err(RentSbfError::Postcondition.into());
    }
    Ok(())
}

struct CloseAccountsV2<'a, 'info> {
    credit: &'a AccountInfo<'info>,
    wallet: &'a AccountInfo<'info>,
    activation_cache: &'a AccountInfo<'info>,
    registry_program: &'a AccountInfo<'info>,
    core_program: &'a AccountInfo<'info>,
    core_programdata: &'a AccountInfo<'info>,
    core_caller: &'a AccountInfo<'info>,
}

impl<'a, 'info> CloseAccountsV2<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CLOSE_ACCOUNT_COUNT_V2 {
            return Err(RentSbfError::AccountFrame.into());
        }
        let value = Self {
            credit: account(accounts, 0)?,
            wallet: account(accounts, 1)?,
            activation_cache: account(accounts, 2)?,
            registry_program: account(accounts, 3)?,
            core_program: account(accounts, 4)?,
            core_programdata: account(accounts, 5)?,
            core_caller: account(accounts, 6)?,
        };
        if !value.credit.is_writable
            || value.credit.is_signer
            || value.credit.executable
            || !value.wallet.is_writable
            || value.wallet.executable
            || value.activation_cache.is_signer
            || value.activation_cache.is_writable
            || value.activation_cache.executable
            || value.registry_program.is_signer
            || value.registry_program.is_writable
            || !value.registry_program.executable
            || value.core_program.is_signer
            || value.core_program.is_writable
            || !value.core_program.executable
            || value.core_programdata.is_signer
            || value.core_programdata.is_writable
            || value.core_programdata.executable
            || !value.core_caller.is_signer
            || value.core_caller.is_writable
            || value.core_caller.executable
        {
            return Err(RentSbfError::AccountFrame.into());
        }
        let keys = [
            value.credit.key,
            value.wallet.key,
            value.activation_cache.key,
            value.registry_program.key,
            value.core_program.key,
            value.core_programdata.key,
            value.core_caller.key,
        ];
        for (index, key) in keys.iter().enumerate() {
            if keys.get(index + 1..).is_some_and(|tail| tail.contains(key)) {
                return Err(RentSbfError::AccountFrame.into());
            }
        }
        Ok(value)
    }
}

/// Close one lifecycle credit after current-Core complete retirement.
#[inline(never)]
fn process_close_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: CloseLifecycleRentCreditV2,
) -> ProgramResult {
    if request.to_bytes().len() != CLOSE_LIFECYCLE_RENT_CREDIT_BYTES_V2 {
        return Err(RentSbfError::Instruction.into());
    }
    let accounts = CloseAccountsV2::parse(accounts)?;
    let state = authenticate_lifecycle_credit_v2(program_id, accounts.credit)?;
    let wallet_data_len =
        u64::try_from(accounts.wallet.data_len()).map_err(|_| RentSbfError::AccountFrame)?;
    validate_refund_wallet_v2(accounts.wallet.owner.to_bytes(), wallet_data_len)
        .map_err(|_| RentSbfError::AccountFrame)?;
    if accounts.wallet.key.to_bytes() != state.refund_wallet().to_bytes() {
        return Err(RentSbfError::AccountFrame.into());
    }
    authenticate_current_core_v2(&accounts, state)?;
    let credit_id = LifecycleAccountIdV2::new(accounts.credit.key.to_bytes())
        .map_err(|_| RentSbfError::Closure)?;
    let core_id = LifecycleAccountIdV2::new(accounts.core_program.key.to_bytes())
        .map_err(|_| RentSbfError::Closure)?;
    let post_resource_digest = request.receipt().input().post_resource_digest;
    let (expected_caller, _) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CORE_CLOSE_AUTHORITY_DOMAIN_V2,
            accounts.credit.key.as_ref(),
            post_resource_digest.as_slice(),
        ],
        accounts.core_program.key,
    );
    if accounts.core_caller.key != &expected_caller {
        return Err(RentSbfError::Caller.into());
    }
    let plan = LifecycleClosePlanV2::new(
        state,
        credit_id,
        core_id,
        true,
        accounts.credit.lamports(),
        accounts.wallet.lamports(),
        request,
    )
    .map_err(|_| RentSbfError::Closure)?;
    let receipt = plan
        .receipt(state, credit_id)
        .map_err(|_| RentSbfError::Closure)?;
    drop(
        accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .credit
            .try_borrow_mut_data()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    drop(
        accounts
            .wallet
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?,
    );
    {
        let mut credit = accounts
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?;
        let mut wallet = accounts
            .wallet
            .try_borrow_mut_lamports()
            .map_err(|_| RentSbfError::Borrow)?;
        **credit = 0;
        **wallet = plan.wallet_after();
    }
    accounts
        .credit
        .resize(0)
        .map_err(|_| RentSbfError::Postcondition)?;
    accounts.credit.assign(&system_program::ID);
    if accounts.credit.lamports() != 0
        || !accounts.credit.data_is_empty()
        || accounts.credit.owner != &system_program::ID
        || accounts.wallet.lamports() != plan.wallet_after()
    {
        return Err(RentSbfError::Postcondition.into());
    }
    set_return_data(&receipt.to_bytes());
    Ok(())
}

#[inline(never)]
fn authenticate_lifecycle_credit_v2(
    program_id: &Pubkey,
    credit: &AccountInfo<'_>,
) -> Result<LifecycleRentCreditV2, ProgramError> {
    if credit.owner != program_id
        || credit.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || credit.executable
    {
        return Err(RentSbfError::RentCredit.into());
    }
    let data = credit.try_borrow_data().map_err(|_| RentSbfError::Borrow)?;
    let state = LifecycleRentCreditV2::decode(&data).map_err(|_| RentSbfError::RentCredit)?;
    drop(data);
    let seeds = state.pda_seeds();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let (expected, bump) = Pubkey::find_program_address(
        &[seeds.domain(), market.as_slice(), generation.as_slice()],
        program_id,
    );
    if credit.key != &expected || state.pda_bump() != bump {
        return Err(RentSbfError::RentCredit.into());
    }
    Ok(state)
}

#[inline(never)]
fn authenticate_current_core_v2(
    accounts: &CloseAccountsV2<'_, '_>,
    state: LifecycleRentCreditV2,
) -> ProgramResult {
    let instruction = Instruction {
        program_id: *accounts.registry_program.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*accounts.activation_cache.key, false),
            AccountMeta::new_readonly(*accounts.core_program.key, false),
            AccountMeta::new_readonly(*accounts.core_programdata.key, false),
        ]),
        data: RegistryInstructionV1::Reauthenticate(ExecutionRoleV1::Core)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            accounts.activation_cache.clone(),
            accounts.core_program.clone(),
            accounts.core_programdata.clone(),
            accounts.registry_program.clone(),
        ],
    )
    .map_err(|_| RentSbfError::Release)?;
    let (producer, bytes) = get_return_data().ok_or(RentSbfError::Release)?;
    if producer != *accounts.registry_program.key {
        return Err(RentSbfError::Release.into());
    }
    let receipt = AuthenticatedRoleReceiptV1::decode(&bytes).map_err(|_| RentSbfError::Release)?;
    if receipt.role() != ExecutionRoleV1::Core
        || receipt.program().as_bytes() != &accounts.core_program.key.to_bytes()
        || receipt.execution_release_set_id().as_bytes() != &state.release_set().to_bytes()
    {
        return Err(RentSbfError::Release.into());
    }
    Ok(())
}

fn meta_v2(account: &AccountInfo<'_>) -> LifecycleAccountMetaV2 {
    LifecycleAccountMetaV2 {
        key: account.key.to_bytes(),
        signer: account.is_signer,
        writable: account.is_writable,
        executable: account.executable,
    }
}

fn authenticate_system_and_rent(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
) -> Result<Rent, ProgramError> {
    if system.key != &system_program::ID || system.owner != &native_loader::ID || !system.executable
    {
        return Err(RentSbfError::RuntimeAccount.into());
    }
    authenticate_rent(rent)
}

fn authenticate_rent(account: &AccountInfo<'_>) -> Result<Rent, ProgramError> {
    if account.key != &sysvar::rent::ID || account.owner != &sysvar::ID || account.executable {
        return Err(RentSbfError::RuntimeAccount.into());
    }
    Rent::from_account_info(account).map_err(|_| RentSbfError::RuntimeAccount.into())
}

fn require_count(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    if accounts.len() == ACCOUNT_COUNT_V1 {
        Ok(())
    } else {
        Err(RentSbfError::AccountFrame.into())
    }
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| RentSbfError::AccountFrame.into())
}

fn meta(account: &AccountInfo<'_>) -> AccountMetaV1 {
    AccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn map_frame_error(error: ContractError) -> ProgramError {
    match error {
        ContractError::InvalidAccountPrivilege | ContractError::AccountAlias => {
            RentSbfError::AccountFrame.into()
        }
        ContractError::InvalidSystemProgram | ContractError::InvalidRentSysvar => {
            RentSbfError::RuntimeAccount.into()
        }
        _ => RentSbfError::AccountFrame.into(),
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use solana_program::sysvar::SysvarSerialize;
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

    fn account_info(
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

    fn rent_account() -> AccountInfo<'static> {
        let mut account = account_info(
            sysvar::rent::ID,
            false,
            false,
            1,
            vec![0; Rent::size_of()],
            sysvar::ID,
            false,
        );
        assert_eq!(Rent::default().to_account_info(&mut account), Some(()));
        account
    }

    fn credit_account(
        program_id: Pubkey,
        authority: Pubkey,
        lamports: u64,
    ) -> AccountInfo<'static> {
        let (key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, authority.as_ref()],
            &program_id,
        );
        let authority = RefundAuthority::new(authority.to_bytes()).expect("authority");
        account_info(
            key,
            false,
            true,
            lamports,
            RentCreditV1::new(authority, bump).to_bytes().to_vec(),
            program_id,
            false,
        )
    }

    fn lifecycle_state(
        program_id: Pubkey,
        market: Pubkey,
        release_set: Pubkey,
        wallet: Pubkey,
    ) -> (Pubkey, LifecycleRentCreditV2) {
        let generation = 7_u64;
        let (credit, bump) = Pubkey::find_program_address(
            &[
                dclutch_rent_contract::lifecycle_v2::LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                market.as_ref(),
                &generation.to_le_bytes(),
            ],
            &program_id,
        );
        let state = LifecycleRentCreditV2::new(
            RefundAuthority::new(wallet.to_bytes()).expect("wallet"),
            LifecycleAccountIdV2::new(market.to_bytes()).expect("market"),
            LifecycleAccountIdV2::new(release_set.to_bytes()).expect("release"),
            generation,
            bump,
        )
        .expect("lifecycle state");
        (credit, state)
    }

    #[test]
    fn hostile_instruction_and_account_width_refuse() {
        let authority = RefundAuthority::new([7; 32]).expect("authority");
        let mut bytes = CreateRentCreditV1::new(authority, 1).to_bytes().to_vec();
        bytes.push(0);
        assert_eq!(
            process_instruction(&Pubkey::new_unique(), &[], &bytes),
            Err(RentSbfError::Instruction.into())
        );
        let request = WithdrawRentCreditV1::new(1).expect("withdraw");
        assert_eq!(
            process_instruction(&Pubkey::new_unique(), &[], &request.to_bytes()),
            Err(RentSbfError::AccountFrame.into())
        );
    }

    #[test]
    fn substituted_authority_and_under_rent_refuse_without_mutation() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let impostor = Pubkey::new_unique();
        let floor = Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1);
        let accounts = [
            credit_account(program_id, authority, floor + 4),
            account_info(impostor, true, false, 1, vec![], system_program::ID, false),
            account_info(
                Pubkey::new_unique(),
                false,
                true,
                10,
                vec![],
                system_program::ID,
                false,
            ),
            rent_account(),
        ];
        assert_eq!(
            process_withdraw(
                &program_id,
                &accounts,
                WithdrawRentCreditV1::new(4).expect("withdraw")
            ),
            Err(RentSbfError::RentCredit.into())
        );
        assert_eq!(accounts.first().expect("credit").lamports(), floor + 4);
        assert_eq!(accounts.get(2).expect("recipient").lamports(), 10);

        let under_rent = [
            credit_account(program_id, authority, floor.saturating_sub(1)),
            account_info(authority, true, false, 1, vec![], system_program::ID, false),
            account_info(
                Pubkey::new_unique(),
                false,
                true,
                10,
                vec![],
                system_program::ID,
                false,
            ),
            rent_account(),
        ];
        assert_eq!(
            process_withdraw(
                &program_id,
                &under_rent,
                WithdrawRentCreditV1::new(1).expect("withdraw")
            ),
            Err(RentSbfError::Balance.into())
        );
        assert_eq!(
            under_rent.first().expect("credit").lamports(),
            floor.saturating_sub(1)
        );
    }

    #[test]
    fn exact_withdrawal_preserves_floor_and_bytes() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let floor = Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1);
        let accounts = [
            credit_account(program_id, authority, floor + 19),
            account_info(authority, true, false, 1, vec![], system_program::ID, false),
            account_info(
                Pubkey::new_unique(),
                false,
                true,
                11,
                vec![],
                system_program::ID,
                false,
            ),
            rent_account(),
        ];
        let before = accounts
            .first()
            .expect("credit")
            .try_borrow_data()
            .expect("data")
            .to_vec();
        process_withdraw(
            &program_id,
            &accounts,
            WithdrawRentCreditV1::new(19).expect("withdraw"),
        )
        .expect("exact withdrawal");
        assert_eq!(accounts.first().expect("credit").lamports(), floor);
        assert_eq!(accounts.get(2).expect("recipient").lamports(), 30);
        assert_eq!(
            accounts
                .first()
                .expect("credit")
                .try_borrow_data()
                .expect("data")
                .as_ref(),
            before
        );
    }

    #[test]
    fn lifecycle_create_derives_market_generation_pda() {
        let program_id = Pubkey::new_unique();
        let payer = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let release_set = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let (credit, state) = lifecycle_state(program_id, market, release_set, wallet);
        let accounts = [
            account_info(
                payer,
                true,
                true,
                10_000_000,
                vec![],
                system_program::ID,
                false,
            ),
            account_info(credit, false, true, 0, vec![], system_program::ID, false),
            account_info(
                system_program::ID,
                false,
                false,
                1,
                vec![],
                native_loader::ID,
                true,
            ),
            rent_account(),
        ];
        let frame = CreateAccounts::parse(&accounts).expect("frame");
        let plan = prepare_create_v2(&program_id, &frame, CreateLifecycleRentCreditV2::new(state))
            .expect("exact create plan");
        assert_eq!(plan.state, state);
        assert_eq!(plan.rent_minimum, Rent::default().minimum_balance(128));

        let wrong_market = LifecycleRentCreditV2::new(
            state.refund_wallet(),
            LifecycleAccountIdV2::new(Pubkey::new_unique().to_bytes()).expect("market"),
            state.release_set(),
            state.generation(),
            state.pda_bump(),
        )
        .expect("state");
        assert_eq!(
            prepare_create_v2(
                &program_id,
                &frame,
                CreateLifecycleRentCreditV2::new(wrong_market)
            )
            .err(),
            Some(RentSbfError::RentCredit.into())
        );
    }

    #[test]
    fn lifecycle_sweep_is_permissionless_and_wallet_fixed() {
        let program_id = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let release_set = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let (credit, state) = lifecycle_state(program_id, market, release_set, wallet);
        let floor = Rent::default().minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2);
        let accounts = [
            account_info(
                credit,
                false,
                true,
                floor + 23,
                state.to_bytes().to_vec(),
                program_id,
                false,
            ),
            account_info(wallet, false, true, 9, vec![], system_program::ID, false),
            rent_account(),
        ];
        process_sweep_v2(
            &program_id,
            &accounts,
            SweepLifecycleRentCreditV2::new(23).expect("sweep"),
        )
        .expect("permissionless sweep");
        assert_eq!(accounts[0].lamports(), floor);
        assert_eq!(accounts[1].lamports(), 32);

        let substituted = [
            account_info(
                credit,
                false,
                true,
                floor + 1,
                state.to_bytes().to_vec(),
                program_id,
                false,
            ),
            account_info(
                Pubkey::new_unique(),
                false,
                true,
                9,
                vec![],
                system_program::ID,
                false,
            ),
            rent_account(),
        ];
        assert_eq!(
            process_sweep_v2(
                &program_id,
                &substituted,
                SweepLifecycleRentCreditV2::new(1).expect("sweep")
            ),
            Err(RentSbfError::AccountFrame.into())
        );
        assert_eq!(substituted[0].lamports(), floor + 1);
        assert_eq!(substituted[1].lamports(), 9);
    }
}
