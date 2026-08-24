//! Permanent native-rent-credit Create and Withdraw adapter boundary.
//!
//! This module intentionally has no close operation.  The contract owns the
//! hostile wire grammar and balance plans; this adapter owns SVM account,
//! current-Rent, PDA, System-CPI, and post-observation facts.

use dclutch_rent_contract::{
    AccountMetaV1, CreateBalancePlanV1, CreateRentCreditFrameV1, CreateRentCreditV1,
    Error as RentError, RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority,
    RentCreditInstructionV1, RentCreditV1, SystemWalletFactsV1, WithdrawBalancePlanV1,
    WithdrawRentCreditFrameV1, WithdrawRentCreditV1,
};
use solana_program::{
    account_info::AccountInfo, program::invoke_signed, program_error::ProgramError, pubkey::Pubkey,
    rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::AdapterError;

const RENT_CREDIT_ACCOUNTS: usize = 4;

/// Decode one owned exact wire and execute its account-frame-specific route.
///
/// The program's top-level router must select this module only for the
/// rent-credit instruction family.  There is deliberately no action-id route
/// here: V1's own canonical wire discriminator is decoded by the contract.
pub(crate) fn dispatch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    match RentCreditInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction)?
    {
        RentCreditInstructionV1::Create(instruction) => {
            process_create(program_id, accounts, instruction)
        }
        RentCreditInstructionV1::Withdraw(instruction) => {
            process_withdraw(program_id, accounts, instruction)
        }
    }
}

struct CreateFrame<'a, 'info> {
    payer: &'a AccountInfo<'info>,
    credit: &'a AccountInfo<'info>,
    system_program: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> CreateFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != RENT_CREDIT_ACCOUNTS {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            payer: account(accounts, 0)?,
            credit: account(accounts, 1)?,
            system_program: account(accounts, 2)?,
            rent_sysvar: account(accounts, 3)?,
        };
        CreateRentCreditFrameV1::new([
            meta(frame.payer),
            meta(frame.credit),
            meta(frame.system_program),
            meta(frame.rent_sysvar),
        ])
        .map_err(map_frame_error)?;
        Ok(frame)
    }
}

struct WithdrawFrame<'a, 'info> {
    credit: &'a AccountInfo<'info>,
    authority: &'a AccountInfo<'info>,
    recipient: &'a AccountInfo<'info>,
    rent_sysvar: &'a AccountInfo<'info>,
}

impl<'a, 'info> WithdrawFrame<'a, 'info> {
    fn parse(accounts: &'a [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != RENT_CREDIT_ACCOUNTS {
            return Err(AdapterError::AccountFrameLength.into());
        }
        let frame = Self {
            credit: account(accounts, 0)?,
            authority: account(accounts, 1)?,
            recipient: account(accounts, 2)?,
            rent_sysvar: account(accounts, 3)?,
        };
        let recipient_facts = SystemWalletFactsV1::new(
            frame.recipient.owner == &system_program::ID,
            u64::try_from(frame.recipient.data_len()).map_err(|_| AdapterError::Arithmetic)?,
        )
        .map_err(|_| AdapterError::RentCreditAuthentication)?;
        WithdrawRentCreditFrameV1::new(
            [
                meta(frame.credit),
                meta(frame.authority),
                meta(frame.recipient),
                meta(frame.rent_sysvar),
            ],
            recipient_facts,
        )
        .map_err(map_frame_error)?;
        Ok(frame)
    }
}

#[derive(Clone, Copy)]
struct CreatePlan {
    credit: RentCreditV1,
    bump: u8,
    rent_lamports: u64,
    balances: CreateBalancePlanV1,
}

/// Permissionlessly create one vacant authority-derived permanent credit.
pub(crate) fn process_create(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: CreateRentCreditV1,
) -> Result<(), ProgramError> {
    let frame = CreateFrame::parse(accounts)?;
    let plan = authenticate_create(program_id, &frame, instruction)?;

    let space = u64::try_from(RENT_CREDIT_BYTES_V1).map_err(|_| AdapterError::Arithmetic)?;
    let create = create_account(
        frame.payer.key,
        frame.credit.key,
        plan.rent_lamports,
        space,
        program_id,
    );
    let authority = plan.credit.refund_authority().to_bytes();
    let bump = [plan.bump];
    let signer = [
        RENT_CREDIT_PDA_DOMAIN_V1,
        authority.as_slice(),
        bump.as_slice(),
    ];
    invoke_signed(
        &create,
        &[
            frame.payer.clone(),
            frame.credit.clone(),
            frame.system_program.clone(),
        ],
        &[&signer],
    )
    .map_err(|_| AdapterError::RentCreditCreateCpi)?;

    plan.balances
        .validate_post(frame.payer.lamports(), frame.credit.lamports())
        .map_err(|_| AdapterError::RentCreditPostcondition)?;
    if frame.credit.owner != program_id || frame.credit.data_len() != RENT_CREDIT_BYTES_V1 {
        return Err(AdapterError::RentCreditPostcondition.into());
    }
    let mut data = frame
        .credit
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::RentCreditPostcondition)?;
    data.copy_from_slice(&plan.credit.to_bytes());
    if RentCreditV1::decode(&data) != Ok(plan.credit) {
        return Err(AdapterError::RentCreditPostcondition.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_create(
    program_id: &Pubkey,
    frame: &CreateFrame<'_, '_>,
    instruction: CreateRentCreditV1,
) -> Result<CreatePlan, ProgramError> {
    authenticate_system_and_rent(frame.system_program, frame.rent_sysvar)?;
    if frame.payer.owner != &system_program::ID
        || !frame.payer.data_is_empty()
        || frame.credit.owner != &system_program::ID
        || !frame.credit.data_is_empty()
        || frame.credit.lamports() != 0
    {
        return Err(AdapterError::RentCreditAuthentication.into());
    }

    let authority = instruction.refund_authority();
    let authority_bytes = authority.to_bytes();
    let (expected_credit, bump) =
        Pubkey::find_program_address(&[RENT_CREDIT_PDA_DOMAIN_V1, &authority_bytes], program_id);
    if frame.credit.key != &expected_credit || instruction.pda_bump() != bump {
        return Err(AdapterError::RentCreditAuthentication.into());
    }
    let rent_lamports = current_rent_minimum(frame.rent_sysvar)?;
    let balances = CreateBalancePlanV1::new(
        frame.payer.lamports(),
        frame.credit.lamports(),
        rent_lamports,
    )
    .map_err(|_| AdapterError::RentCreditAuthentication)?;

    // Verify every mutable post-CPI target is borrowable before the System CPI.
    drop(
        frame
            .payer
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RentCreditAuthentication)?,
    );
    drop(
        frame
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RentCreditAuthentication)?,
    );
    drop(
        frame
            .credit
            .try_borrow_mut_data()
            .map_err(|_| AdapterError::RentCreditAuthentication)?,
    );

    Ok(CreatePlan {
        credit: instruction.credit(),
        bump,
        rent_lamports,
        balances,
    })
}

/// Withdraw one authority-authorized exact surplus while preserving Rent floor.
pub(crate) fn process_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction: WithdrawRentCreditV1,
) -> Result<(), ProgramError> {
    let frame = WithdrawFrame::parse(accounts)?;
    let plan = authenticate_withdraw(program_id, &frame, instruction)?;

    {
        let mut credit_lamports = frame
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RentCreditPostcondition)?;
        let mut recipient_lamports = frame
            .recipient
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RentCreditPostcondition)?;
        **credit_lamports = plan.credit_after();
        **recipient_lamports = plan.recipient_after();
    }
    plan.validate_post(frame.credit.lamports(), frame.recipient.lamports())
        .map_err(|_| AdapterError::RentCreditPostcondition)?;
    Ok(())
}

#[inline(never)]
fn authenticate_withdraw(
    program_id: &Pubkey,
    frame: &WithdrawFrame<'_, '_>,
    instruction: WithdrawRentCreditV1,
) -> Result<WithdrawBalancePlanV1, ProgramError> {
    authenticate_rent(frame.rent_sysvar)?;
    if frame.credit.owner != program_id || frame.credit.data_len() != RENT_CREDIT_BYTES_V1 {
        return Err(AdapterError::RentCreditAuthentication.into());
    }
    let data = frame
        .credit
        .try_borrow_data()
        .map_err(|_| AdapterError::RentCreditAuthentication)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| AdapterError::RentCreditAuthentication)?;
    drop(data);

    let authority = RefundAuthority::new(frame.authority.key.to_bytes())
        .map_err(|_| AdapterError::RentCreditAuthentication)?;
    let authority_bytes = authority.to_bytes();
    let (expected_credit, bump) =
        Pubkey::find_program_address(&[RENT_CREDIT_PDA_DOMAIN_V1, &authority_bytes], program_id);
    if frame.credit.key != &expected_credit {
        return Err(AdapterError::RentCreditAuthentication.into());
    }
    credit
        .validate_binding(authority, bump)
        .map_err(|_| AdapterError::RentCreditAuthentication)?;

    let rent_lamports = current_rent_minimum(frame.rent_sysvar)?;
    let plan = WithdrawBalancePlanV1::new(
        frame.credit.lamports(),
        frame.recipient.lamports(),
        rent_lamports,
        instruction,
    )
    .map_err(|_| AdapterError::RentCreditAuthentication)?;

    // Credit is distinct from recipient by the validated contract frame.
    drop(
        frame
            .credit
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RentCreditAuthentication)?,
    );
    drop(
        frame
            .recipient
            .try_borrow_mut_lamports()
            .map_err(|_| AdapterError::RentCreditAuthentication)?,
    );
    Ok(plan)
}

fn authenticate_system_and_rent(
    system: &AccountInfo<'_>,
    rent: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if system.key != &system_program::ID || system.owner != &native_loader::ID || !system.executable
    {
        return Err(AdapterError::RentCreditAuthentication.into());
    }
    authenticate_rent(rent)
}

fn authenticate_rent(rent: &AccountInfo<'_>) -> Result<(), ProgramError> {
    if rent.key != &sysvar::rent::ID || rent.owner != &sysvar::ID {
        return Err(AdapterError::RentCreditAuthentication.into());
    }
    Rent::from_account_info(rent)
        .map(|_| ())
        .map_err(|_| AdapterError::RentCreditAuthentication.into())
}

fn current_rent_minimum(rent: &AccountInfo<'_>) -> Result<u64, ProgramError> {
    let rent = Rent::from_account_info(rent).map_err(|_| AdapterError::RentCreditAuthentication)?;
    Ok(rent.minimum_balance(RENT_CREDIT_BYTES_V1))
}

fn meta(account: &AccountInfo<'_>) -> AccountMetaV1 {
    AccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        is_executable: account.executable,
    }
}

fn map_frame_error(error: RentError) -> ProgramError {
    match error {
        RentError::InvalidAccountPrivilege => AdapterError::AccountPrivilege.into(),
        RentError::AccountAlias => AdapterError::AccountIdentity.into(),
        _ => AdapterError::RentCreditAuthentication.into(),
    }
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
    use solana_sdk_ids::bpf_loader;
    use std::{boxed::Box, vec, vec::Vec};

    use super::*;

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

    fn rent_account() -> AccountInfo<'static> {
        let mut account = leak_account(
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
    ) -> (AccountInfo<'static>, u8) {
        let (key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, authority.as_ref()],
            &program_id,
        );
        let authority = RefundAuthority::new(authority.to_bytes()).expect("nonzero authority");
        let data = RentCreditV1::new(authority, bump).to_bytes().to_vec();
        (
            leak_account(key, false, true, lamports, data, program_id, false),
            bump,
        )
    }

    fn withdraw_accounts(
        program_id: Pubkey,
        authority_key: Pubkey,
        credit_lamports: u64,
        authority_executable: bool,
    ) -> [AccountInfo<'static>; RENT_CREDIT_ACCOUNTS] {
        let (credit, _) = credit_account(program_id, authority_key, credit_lamports);
        [
            credit,
            leak_account(
                authority_key,
                true,
                false,
                1,
                vec![],
                if authority_executable {
                    bpf_loader::ID
                } else {
                    system_program::ID
                },
                authority_executable,
            ),
            leak_account(
                Pubkey::new_unique(),
                false,
                true,
                11,
                vec![],
                system_program::ID,
                false,
            ),
            rent_account(),
        ]
    }

    #[test]
    fn decoder_refuses_trailing_bytes() {
        let authority = RefundAuthority::new(Pubkey::new_unique().to_bytes()).expect("authority");
        let request = CreateRentCreditV1::new(authority, 7);
        let mut bytes = request.to_bytes().to_vec();
        bytes.push(0);
        assert_eq!(
            RentCreditInstructionV1::decode(&bytes),
            Err(RentError::InvalidLength)
        );
    }

    #[test]
    fn create_authentication_refuses_wrong_pda_and_nonvacant_owner() {
        let program_id = Pubkey::new_unique();
        let authority = RefundAuthority::new(Pubkey::new_unique().to_bytes()).expect("authority");
        let payer = leak_account(
            Pubkey::new_unique(),
            true,
            true,
            1_000_000,
            vec![],
            system_program::ID,
            false,
        );
        let credit = leak_account(
            Pubkey::new_unique(),
            false,
            true,
            0,
            vec![],
            system_program::ID,
            false,
        );
        let system = leak_account(
            system_program::ID,
            false,
            false,
            1,
            vec![],
            native_loader::ID,
            true,
        );
        let accounts = [payer, credit, system, rent_account()];
        let frame = CreateFrame::parse(&accounts).expect("valid frame shape");
        assert_eq!(
            authenticate_create(&program_id, &frame, CreateRentCreditV1::new(authority, 0)).err(),
            Some(ProgramError::from(AdapterError::RentCreditAuthentication))
        );

        let authority_bytes = authority.to_bytes();
        let (key, bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, &authority_bytes],
            &program_id,
        );
        let occupied = [
            leak_account(
                Pubkey::new_unique(),
                true,
                true,
                1_000_000,
                vec![],
                system_program::ID,
                false,
            ),
            leak_account(key, false, true, 0, vec![], program_id, false),
            leak_account(
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
        let occupied_frame = CreateFrame::parse(&occupied).expect("valid frame shape");
        assert_eq!(
            authenticate_create(
                &program_id,
                &occupied_frame,
                CreateRentCreditV1::new(authority, bump)
            )
            .err(),
            Some(ProgramError::from(AdapterError::RentCreditAuthentication))
        );
    }

    #[test]
    fn executable_readonly_authority_is_admitted() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let floor = Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1);
        let accounts = withdraw_accounts(program_id, authority, floor + 9, true);
        let frame = WithdrawFrame::parse(&accounts).expect("executable readonly signer is valid");
        assert!(
            authenticate_withdraw(
                &program_id,
                &frame,
                WithdrawRentCreditV1::new(9).expect("nonzero request")
            )
            .is_ok()
        );
    }

    #[test]
    fn authority_recipient_alias_requires_and_accepts_privilege_union() {
        let program_id = Pubkey::new_unique();
        let authority_key = Pubkey::new_unique();
        let floor = Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1);
        let (credit, _) = credit_account(program_id, authority_key, floor + 3);
        let authority_recipient = leak_account(
            authority_key,
            true,
            true,
            11,
            vec![],
            system_program::ID,
            false,
        );
        let accounts = [
            credit,
            authority_recipient.clone(),
            authority_recipient,
            rent_account(),
        ];
        let frame = WithdrawFrame::parse(&accounts).expect("alias privilege union");
        assert!(
            authenticate_withdraw(
                &program_id,
                &frame,
                WithdrawRentCreditV1::new(3).expect("nonzero request")
            )
            .is_ok()
        );
    }

    #[test]
    fn withdrawal_refuses_under_rent_and_preserves_exact_post_balances() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let floor = Rent::default().minimum_balance(RENT_CREDIT_BYTES_V1);
        let under_rent = withdraw_accounts(program_id, authority, floor.saturating_sub(1), false);
        let under_rent_frame = WithdrawFrame::parse(&under_rent).expect("valid frame");
        assert_eq!(
            authenticate_withdraw(
                &program_id,
                &under_rent_frame,
                WithdrawRentCreditV1::new(1).expect("nonzero request")
            )
            .err(),
            Some(ProgramError::from(AdapterError::RentCreditAuthentication))
        );

        let balances = withdraw_accounts(program_id, authority, floor + 19, false);
        process_withdraw(
            &program_id,
            &balances,
            WithdrawRentCreditV1::new(19).expect("nonzero request"),
        )
        .expect("exact surplus withdrawal");
        assert_eq!(balances[0].lamports(), floor);
        assert_eq!(balances[2].lamports(), 30);
    }
}
