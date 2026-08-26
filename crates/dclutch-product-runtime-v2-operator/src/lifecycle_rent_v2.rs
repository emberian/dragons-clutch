//! Chain-derived lifecycle-scoped RentCredit V2 workflows.
//!
//! Creation consumes an already authenticated Core Found projection, so the
//! Market, generation, and release-set identities cannot diverge from the
//! subsequent Found instruction. The refund wallet occurs only in the Rent
//! state. Sweeping derives the complete safe surplus from a finalized account
//! snapshot. Close authority remains exclusively inside Core; this module only
//! validates the typed Rent acknowledgement returned to that outer workflow.

use dclutch_rent_contract::{
    RefundAuthority,
    lifecycle_v2::{
        CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2, CreateLifecycleRentCreditV2,
        LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2, LIFECYCLE_RENT_CREDIT_BYTES_V2,
        LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCloseReceiptV2,
        LifecycleRentCreditV2, LifecycleSweepPlanV2, SweepLifecycleRentCreditV2,
    },
};
use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};

use crate::{AccountObservationV2, found::FoundInstructionPlanV2};

/// Exact V2 Create frame width.
pub const LIFECYCLE_RENT_CREATE_ACCOUNT_COUNT_V2: usize = 4;
/// Exact V2 Sweep frame width.
pub const LIFECYCLE_RENT_SWEEP_ACCOUNT_COUNT_V2: usize = 3;

/// Refusal from a hostile lifecycle-Rent observation or cross-workflow join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleRentOperatorErrorV2 {
    /// Observations did not come from the Found projection's finalized slot.
    ObservationMismatch,
    /// A runtime account had the wrong owner, data, executable bit, or vacancy.
    AccountAuthority,
    /// The lifecycle credit state or PDA was not canonical.
    InvalidCredit,
    /// The requested sweep had no safe positive surplus.
    NoSweepableSurplus,
    /// Lamport arithmetic overflowed or an observed post-balance differed.
    BalanceMismatch,
    /// A close receipt was malformed or did not close the expected lifecycle.
    InvalidCloseReceipt,
}

/// Finalized accounts needed to create the credit selected by one Found plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentCreateStateV2<'a> {
    /// System-owned signer donating the current credit rent minimum.
    pub payer: AccountObservationV2<'a>,
    /// Vacant System-owned exact lifecycle-credit PDA.
    pub credit_destination: AccountObservationV2<'a>,
    /// Sole immutable System wallet that receives sweeps and terminal closure.
    pub refund_wallet: AccountObservationV2<'a>,
    /// Current executable Rent program selected by Found infrastructure.
    pub rent_program: AccountObservationV2<'a>,
    /// Canonical executable System Program.
    pub system_program: AccountObservationV2<'a>,
    /// Canonical Rent sysvar used for the current minimum.
    pub rent: AccountObservationV2<'a>,
}

/// Exact unsigned V2 creation plan joined to one Core Found plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleRentCreatePlanV2 {
    /// Unsigned four-account Rent Create instruction.
    pub instruction: Instruction,
    /// Canonical lifecycle credit address.
    pub credit: Pubkey,
    /// Exact immutable state written by Rent.
    pub state: LifecycleRentCreditV2,
    /// Current rent-minimum debit from the payer.
    pub rent_debit: u64,
    /// Finalized observation slot shared with Found.
    pub observation_slot: u64,
}

/// Finalized state needed to sweep all currently safe surplus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentSweepStateV2<'a> {
    /// Existing writable lifecycle credit.
    pub credit: AccountObservationV2<'a>,
    /// Immutable writable refund wallet named by the credit.
    pub refund_wallet: AccountObservationV2<'a>,
    /// Current executable Rent program.
    pub rent_program: AccountObservationV2<'a>,
    /// Canonical Rent sysvar.
    pub rent: AccountObservationV2<'a>,
}

/// Exact unsigned maximum-safe-surplus sweep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleRentSweepPlanV2 {
    /// Unsigned three-account Rent Sweep instruction.
    pub instruction: Instruction,
    /// Complete balance above the current rent minimum.
    pub amount: u64,
    /// Expected credit post-balance.
    pub credit_after: u64,
    /// Expected refund-wallet post-balance.
    pub wallet_after: u64,
    /// Shared finalized observation slot.
    pub observation_slot: u64,
}

/// Exact pre-close facts retained by the outer Core-retirement operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRentCloseExpectationV2 {
    /// Canonical pre-close state.
    pub state: LifecycleRentCreditV2,
    /// Closed credit address.
    pub credit: Pubkey,
    /// Credit balance transferred at closure.
    pub credit_lamports: u64,
    /// Refund-wallet balance immediately before closure.
    pub wallet_before: u64,
}

/// Construct the sole lifecycle credit selected by an authenticated Found plan.
pub fn build_lifecycle_rent_create_v2(
    found: &FoundInstructionPlanV2,
    state: LifecycleRentCreateStateV2<'_>,
) -> Result<LifecycleRentCreatePlanV2, LifecycleRentOperatorErrorV2> {
    let slot = found.observation_slot;
    for account in [
        state.payer,
        state.credit_destination,
        state.refund_wallet,
        state.rent_program,
        state.system_program,
        state.rent,
    ] {
        if account.slot != slot {
            return Err(LifecycleRentOperatorErrorV2::ObservationMismatch);
        }
    }
    authenticate_system_wallet(state.payer)?;
    authenticate_system_wallet(state.refund_wallet)?;
    if state.credit_destination.key == state.refund_wallet.key
        || state.credit_destination.key == found.market_address
        || state.rent_program.key == state.credit_destination.key
        || !state.credit_destination.data.is_empty()
        || state.credit_destination.owner != system_program::ID
        || state.credit_destination.executable
        || state.credit_destination.lamports != 0
        || !state.rent_program.executable
        || state.rent_program.key == system_program::ID
        || state.system_program.key != system_program::ID
        || state.system_program.owner != native_loader::ID
        || !state.system_program.executable
        || !state.system_program.data.is_empty()
    {
        return Err(LifecycleRentOperatorErrorV2::AccountAuthority);
    }
    let rent = decode_rent(state.rent)?;
    let refund = RefundAuthority::new(state.refund_wallet.key.to_bytes())
        .map_err(|_| LifecycleRentOperatorErrorV2::AccountAuthority)?;
    let market = LifecycleAccountIdV2::new(found.market_address.to_bytes())
        .map_err(|_| LifecycleRentOperatorErrorV2::InvalidCredit)?;
    let release_set =
        LifecycleAccountIdV2::new(found.market_identity.selected_release_set.to_bytes())
            .map_err(|_| LifecycleRentOperatorErrorV2::InvalidCredit)?;
    let generation = found.market_identity.generation;
    let (credit, bump) = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            found.market_address.as_ref(),
            &generation.to_le_bytes(),
        ],
        &state.rent_program.key,
    );
    if credit != state.credit_destination.key {
        return Err(LifecycleRentOperatorErrorV2::InvalidCredit);
    }
    let credit_state = LifecycleRentCreditV2::new(refund, market, release_set, generation, bump)
        .map_err(|_| LifecycleRentOperatorErrorV2::InvalidCredit)?;
    let rent_debit = rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2);
    if state.payer.lamports < rent_debit {
        return Err(LifecycleRentOperatorErrorV2::BalanceMismatch);
    }
    let data = CreateLifecycleRentCreditV2::new(credit_state)
        .to_bytes()
        .to_vec();
    if data.len() != CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2 {
        return Err(LifecycleRentOperatorErrorV2::InvalidCredit);
    }
    Ok(LifecycleRentCreatePlanV2 {
        instruction: Instruction {
            program_id: state.rent_program.key,
            accounts: vec![
                AccountMeta::new(state.payer.key, true),
                AccountMeta::new(credit, false),
                AccountMeta::new_readonly(state.system_program.key, false),
                AccountMeta::new_readonly(state.rent.key, false),
            ],
            data,
        },
        credit,
        state: credit_state,
        rent_debit,
        observation_slot: slot,
    })
}

/// Sweep the complete current surplus while preserving the current rent floor.
pub fn build_lifecycle_rent_sweep_all_v2(
    state: LifecycleRentSweepStateV2<'_>,
) -> Result<LifecycleRentSweepPlanV2, LifecycleRentOperatorErrorV2> {
    let slot = state.credit.slot;
    if [state.refund_wallet, state.rent_program, state.rent]
        .iter()
        .any(|account| account.slot != slot)
    {
        return Err(LifecycleRentOperatorErrorV2::ObservationMismatch);
    }
    let credit = authenticate_credit(state.rent_program, state.credit)?;
    authenticate_system_wallet(state.refund_wallet)?;
    if state.refund_wallet.key.to_bytes() != credit.refund_wallet().to_bytes() {
        return Err(LifecycleRentOperatorErrorV2::AccountAuthority);
    }
    let rent = decode_rent(state.rent)?;
    let rent_minimum = rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2);
    let amount = state
        .credit
        .lamports
        .checked_sub(rent_minimum)
        .filter(|amount| *amount > 0)
        .ok_or(LifecycleRentOperatorErrorV2::NoSweepableSurplus)?;
    let request = SweepLifecycleRentCreditV2::new(amount)
        .map_err(|_| LifecycleRentOperatorErrorV2::NoSweepableSurplus)?;
    let balances = LifecycleSweepPlanV2::new(
        state.credit.lamports,
        state.refund_wallet.lamports,
        rent_minimum,
        request,
    )
    .map_err(|_| LifecycleRentOperatorErrorV2::BalanceMismatch)?;
    Ok(LifecycleRentSweepPlanV2 {
        instruction: Instruction {
            program_id: state.rent_program.key,
            accounts: vec![
                AccountMeta::new(state.credit.key, false),
                AccountMeta::new(state.refund_wallet.key, false),
                AccountMeta::new_readonly(state.rent.key, false),
            ],
            data: request.to_bytes().to_vec(),
        },
        amount,
        credit_after: balances.credit_after(),
        wallet_after: balances.wallet_after(),
        observation_slot: slot,
    })
}

/// Authenticate one lifecycle-credit observation and capture its close facts.
pub fn lifecycle_rent_close_expectation_v2(
    rent_program: AccountObservationV2<'_>,
    credit: AccountObservationV2<'_>,
    refund_wallet: AccountObservationV2<'_>,
) -> Result<LifecycleRentCloseExpectationV2, LifecycleRentOperatorErrorV2> {
    if rent_program.slot != credit.slot || refund_wallet.slot != credit.slot {
        return Err(LifecycleRentOperatorErrorV2::ObservationMismatch);
    }
    let lifecycle = authenticate_credit(rent_program, credit)?;
    authenticate_system_wallet(refund_wallet)?;
    if refund_wallet.key.to_bytes() != lifecycle.refund_wallet().to_bytes() || credit.lamports == 0
    {
        return Err(LifecycleRentOperatorErrorV2::AccountAuthority);
    }
    Ok(LifecycleRentCloseExpectationV2 {
        state: lifecycle,
        credit: credit.key,
        credit_lamports: credit.lamports,
        wallet_before: refund_wallet.lamports,
    })
}

/// Validate the Rent close acknowledgement and exact refund-wallet delta.
///
/// This deliberately does not construct a top-level Close instruction: only
/// the current Core program can sign the close-authority PDA during retirement.
pub fn validate_lifecycle_rent_close_receipt_v2(
    expectation: LifecycleRentCloseExpectationV2,
    rent_program: Pubkey,
    return_data_producer: Pubkey,
    return_data: &[u8],
    wallet_after: u64,
) -> Result<LifecycleRentCloseReceiptV2, LifecycleRentOperatorErrorV2> {
    if return_data_producer != rent_program
        || return_data.len() != LIFECYCLE_RENT_CLOSE_RECEIPT_BYTES_V2
    {
        return Err(LifecycleRentOperatorErrorV2::InvalidCloseReceipt);
    }
    let receipt = LifecycleRentCloseReceiptV2::decode(return_data)
        .map_err(|_| LifecycleRentOperatorErrorV2::InvalidCloseReceipt)?;
    let input = receipt.input();
    let expected_wallet_after = expectation
        .wallet_before
        .checked_add(expectation.credit_lamports)
        .ok_or(LifecycleRentOperatorErrorV2::BalanceMismatch)?;
    if input.credit.to_bytes() != expectation.credit.to_bytes()
        || input.refund_wallet != expectation.state.refund_wallet()
        || input.market != expectation.state.market()
        || input.release_set != expectation.state.release_set()
        || input.generation != expectation.state.generation()
        || input.closed_lamports != expectation.credit_lamports
        || wallet_after != expected_wallet_after
    {
        return Err(LifecycleRentOperatorErrorV2::InvalidCloseReceipt);
    }
    Ok(receipt)
}

fn authenticate_credit(
    rent_program: AccountObservationV2<'_>,
    credit: AccountObservationV2<'_>,
) -> Result<LifecycleRentCreditV2, LifecycleRentOperatorErrorV2> {
    if rent_program.slot != credit.slot
        || !rent_program.executable
        || credit.owner != rent_program.key
        || credit.executable
        || credit.data.len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
    {
        return Err(LifecycleRentOperatorErrorV2::AccountAuthority);
    }
    let state = LifecycleRentCreditV2::decode(credit.data)
        .map_err(|_| LifecycleRentOperatorErrorV2::InvalidCredit)?;
    let seeds = state.pda_seeds();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            bump.as_slice(),
        ],
        &rent_program.key,
    )
    .map_err(|_| LifecycleRentOperatorErrorV2::InvalidCredit)?;
    if expected != credit.key {
        return Err(LifecycleRentOperatorErrorV2::InvalidCredit);
    }
    Ok(state)
}

fn authenticate_system_wallet(
    account: AccountObservationV2<'_>,
) -> Result<(), LifecycleRentOperatorErrorV2> {
    if account.owner != system_program::ID || account.executable || !account.data.is_empty() {
        Err(LifecycleRentOperatorErrorV2::AccountAuthority)
    } else {
        Ok(())
    }
}

fn decode_rent(account: AccountObservationV2<'_>) -> Result<Rent, LifecycleRentOperatorErrorV2> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(LifecycleRentOperatorErrorV2::AccountAuthority);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.to_vec();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Rent::from_account_info(&info).map_err(|_| LifecycleRentOperatorErrorV2::AccountAuthority)
}

#[cfg(test)]
mod tests {
    use solana_program::rent::Rent;

    use super::*;

    fn id(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn observation<'a>(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: &'a [u8],
    ) -> AccountObservationV2<'a> {
        AccountObservationV2 {
            slot: 41,
            key,
            owner,
            lamports,
            executable,
            data,
        }
    }

    #[test]
    fn sweep_is_maximal_and_refuses_wallet_substitution() {
        let rent_program = id(20);
        let market = LifecycleAccountIdV2::new(id(21).to_bytes()).expect("market");
        let release = LifecycleAccountIdV2::new(id(22).to_bytes()).expect("release");
        let refund = RefundAuthority::new(id(23).to_bytes()).expect("refund");
        let generation = 7_u64;
        let (credit_key, bump) = Pubkey::find_program_address(
            &[
                LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                id(21).as_ref(),
                &generation.to_le_bytes(),
            ],
            &rent_program,
        );
        let credit =
            LifecycleRentCreditV2::new(refund, market, release, generation, bump).expect("credit");
        let credit_bytes = credit.to_bytes();
        let rent = Rent::default();
        let mut rent_bytes = vec![0_u8; Rent::size_of()];
        let mut rent_lamports = 1;
        let mut rent_info = AccountInfo::new(
            &sysvar::rent::ID,
            false,
            false,
            &mut rent_lamports,
            &mut rent_bytes,
            &sysvar::ID,
            false,
        );
        rent.to_account_info(&mut rent_info)
            .expect("serialize canonical Rent");
        drop(rent_info);
        let floor = rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2);
        let plan = build_lifecycle_rent_sweep_all_v2(LifecycleRentSweepStateV2 {
            credit: observation(credit_key, rent_program, floor + 19, false, &credit_bytes),
            refund_wallet: observation(id(23), system_program::ID, 5, false, &[]),
            rent_program: observation(rent_program, native_loader::ID, 1, true, &[]),
            rent: observation(sysvar::rent::ID, sysvar::ID, 1, false, &rent_bytes),
        })
        .expect("sweep");
        assert_eq!(
            (plan.amount, plan.credit_after, plan.wallet_after),
            (19, floor, 24)
        );

        let substituted = build_lifecycle_rent_sweep_all_v2(LifecycleRentSweepStateV2 {
            credit: observation(credit_key, rent_program, floor + 19, false, &credit_bytes),
            refund_wallet: observation(id(24), system_program::ID, 5, false, &[]),
            rent_program: observation(rent_program, native_loader::ID, 1, true, &[]),
            rent: observation(sysvar::rent::ID, sysvar::ID, 1, false, &rent_bytes),
        });
        assert_eq!(
            substituted,
            Err(LifecycleRentOperatorErrorV2::AccountAuthority)
        );
    }
}
