//! The upkeep vault's two routes: found it once, credit it under a class.
//!
//! Decision 0024 item 4. The contract (`dclutch_custody::upkeep_vault_v1`)
//! owns the record, the closed source-class enum, the two operations and the
//! one transition; this adapter owns the accounts: the PDA, the rent, the one
//! System transfer a deposit performs, and the caller-authority conjunct a
//! protocol credit must satisfy. There is no spend route here and the reserved
//! debit tag refuses by name at decode, before an account is touched.
//!
//! # Two credit shapes, one recognition rule
//!
//! Every credit RECOGNIZES `amount` lamports that are already at the address
//! and not yet receipted (`UpkeepVaultV1::unreceipted`). A deposit moves them
//! there first, by a System transfer the depositor signs, inside this route. A
//! protocol credit finds them already there: the calling route debited an
//! account it owns and credited the vault's lamports directly -- a credit to a
//! foreign-owned account the runtime allows -- and then called here so the
//! record says where they came from. Both end at the same conjunct: at least
//! `amount` unreceipted lamports at the address, then `credit`.
//!
//! # Who may say "residue"
//!
//! Only a release-set role, signing with its caller-authority PDA derived
//! exactly as every other Custody route derives it, and named in the
//! activation cache this frame carries. A wallet can only say `deposit`, and
//! the wire refuses the other three from a frame with a signer in it.

use super::*;

use dclutch_custody::upkeep_vault_v1::{
    UPKEEP_DEPOSIT_CREDIT_ACCOUNT_COUNT_V1, UPKEEP_FOUND_ACCOUNT_COUNT_V1,
    UPKEEP_PROTOCOL_CREDIT_ACCOUNT_COUNT_V1, UPKEEP_VAULT_RECEIPT_BYTES_V1,
    UPKEEP_VAULT_RECORD_BYTES_V1, UPKEEP_VAULT_REQUEST_BYTES_V1, UPKEEP_VAULT_REQUEST_MAGIC_V1,
    UpkeepCreditReceiptV1, UpkeepCreditV1, UpkeepOperationV1, UpkeepProtocolCallerV1,
    UpkeepRequestV1, UpkeepSourceClassV1, UpkeepVaultSeedsV1, UpkeepVaultV1,
};

/// Stable refusal from the vault's routes: sub-band `0x6200` of Custody's band.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpkeepVaultSbfErrorV1 {
    /// Instruction bytes did not decode as the one generated request.
    Instruction = 0x6200,
    /// Account count, order, privileges, or aliases were not exact.
    AccountFrame = 0x6201,
    /// The vault PDA, its owner, or its bytes refused, or it is unfounded.
    Vault = 0x6202,
    /// Rent, payer, System program, or account creation refused.
    Create = 0x6203,
    /// The reserved debit tag: there is no spend instruction (decision 0024
    /// item 4, I2). Refused by name so the hostile that tries to spend is told
    /// which charter stopped it.
    NoSpendRoute = 0x6204,
    /// A source class the frame cannot vouch for: a protocol class with a
    /// signer, a deposit with a caller authority, or a byte outside the four.
    SourceClass = 0x6205,
    /// The caller authority was not the release-pinned role PDA signer.
    CallerAuthority = 0x6206,
    /// The activation cache, calling program, or ProgramData refused.
    Release = 0x6207,
    /// Fewer unreceipted lamports at the address than the credit names, or the
    /// deposit's System transfer refused.
    Unreceipted = 0x6208,
    /// The record could not be rewritten after the credit was computed.
    Commit = 0x6209,
    /// The record's totals did not close (a record this program never wrote).
    Legibility = 0x620A,
    /// The calling deployment moved beyond its activated slot pin.
    ReleaseSuperseded = 0x620B,
}

dclutch_refusal_registry::pin_refusal_band!(
    UpkeepVaultSbfErrorV1,
    dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 0x200,
    [
        Instruction,
        AccountFrame,
        Vault,
        Create,
        NoSpendRoute,
        SourceClass,
        CallerAuthority,
        Release,
        Unreceipted,
        Commit,
        Legibility,
        ReleaseSuperseded
    ]
);

impl From<dclutch_registry::activation_auth_v1::ActivationAuthErrorV1> for UpkeepVaultSbfErrorV1 {
    fn from(value: dclutch_registry::activation_auth_v1::ActivationAuthErrorV1) -> Self {
        use dclutch_registry::activation_auth_v1::ActivationAuthErrorV1;
        match value {
            ActivationAuthErrorV1::ReleaseSuperseded => Self::ReleaseSuperseded,
            cause => {
                solana_program::msg!("upkeep caller release: {:?}", cause);
                Self::Release
            }
        }
    }
}

impl From<dclutch_custody::upkeep_vault_v1::Error> for UpkeepVaultSbfErrorV1 {
    fn from(value: dclutch_custody::upkeep_vault_v1::Error) -> Self {
        use dclutch_custody::upkeep_vault_v1::Error;
        match value {
            Error::NoSpendRoute => Self::NoSpendRoute,
            Error::UnknownSourceClass | Error::UnknownCallerRole => Self::SourceClass,
            Error::NotLegible => Self::Legibility,
            Error::ArithmeticOverflow => Self::Commit,
            _ => Self::Instruction,
        }
    }
}

/// Whether `instruction_data` is this family's wire: exact width and magic.
pub(super) fn selects(instruction_data: &[u8]) -> bool {
    instruction_data.len() == UPKEEP_VAULT_REQUEST_BYTES_V1
        && instruction_data.get(..UPKEEP_VAULT_REQUEST_MAGIC_V1.len())
            == Some(UPKEEP_VAULT_REQUEST_MAGIC_V1.as_slice())
}

const VAULT: usize = 0;

/// Dispatch one vault request.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    // The contract's own refusal names, kept: `NoSpendRoute` and the class
    // refusals are economic verdicts over bytes that decode, not "these bytes
    // are not the request".
    let request = UpkeepRequestV1::decode(instruction_data).map_err(UpkeepVaultSbfErrorV1::from)?;
    let request_digest = hash(instruction_data).to_bytes();
    let vault = account(accounts, VAULT).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let (expected, bump) =
        Pubkey::find_program_address(&UpkeepVaultSeedsV1.as_slices(), program_id);
    if vault.key != &expected || !vault.is_writable || vault.is_signer {
        return Err(UpkeepVaultSbfErrorV1::Vault.into());
    }
    match (request.operation, request.credit) {
        (UpkeepOperationV1::Found, None) => {
            found(program_id, accounts, vault, bump, request_digest)
        }
        (UpkeepOperationV1::Credit, Some(credit)) => {
            credit_route(program_id, accounts, vault, credit, request_digest)
        }
        _ => Err(UpkeepVaultSbfErrorV1::Instruction.into()),
    }
}

/// `[vault, payer, system_program, rent_sysvar]`: create the record once,
/// funded to exactly its own rent minimum. Anybody may; nobody gains an
/// authority by doing so, because the record has none.
#[inline(never)]
fn found<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    vault: &AccountInfo<'info>,
    bump: u8,
    request_digest: [u8; 32],
) -> ProgramResult {
    if accounts.len() != UPKEEP_FOUND_ACCOUNT_COUNT_V1 {
        return Err(UpkeepVaultSbfErrorV1::AccountFrame.into());
    }
    let payer = account(accounts, 1).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let system = account(accounts, 2).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let rent_account = account(accounts, 3).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    if !payer.is_signer
        || !payer.is_writable
        || system.key != &system_program::ID
        || rent_account.key != &sysvar::rent::ID
        || payer.key == vault.key
    {
        return Err(UpkeepVaultSbfErrorV1::AccountFrame.into());
    }
    // Founding twice is refused by the account already being ours; a vault
    // holding stray lamports while still System-owned is founded around them
    // (they become unreceipted, which the census reports).
    if vault.owner != &system_program::ID || vault.data_len() != 0 {
        return Err(UpkeepVaultSbfErrorV1::Vault.into());
    }
    let rent = Rent::from_account_info(rent_account).map_err(|_| UpkeepVaultSbfErrorV1::Create)?;
    let minimum = rent.minimum_balance(UPKEEP_VAULT_RECORD_BYTES_V1);
    let shortfall = minimum.saturating_sub(vault.lamports());
    let bump_seed = [bump];
    let [domain] = UpkeepVaultSeedsV1.as_slices();
    let signer_seeds = &[domain, &bump_seed];
    if shortfall != 0 {
        invoke(
            &transfer(payer.key, vault.key, shortfall),
            &[payer.clone(), vault.clone(), system.clone()],
        )
        .map_err(|_| UpkeepVaultSbfErrorV1::Create)?;
    }
    invoke_signed(
        &allocate(
            vault.key,
            u64::try_from(UPKEEP_VAULT_RECORD_BYTES_V1)
                .map_err(|_| UpkeepVaultSbfErrorV1::Create)?,
        ),
        &[vault.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| UpkeepVaultSbfErrorV1::Create)?;
    invoke_signed(
        &assign(vault.key, program_id),
        &[vault.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| UpkeepVaultSbfErrorV1::Create)?;
    if vault.owner != program_id || vault.data_len() != UPKEEP_VAULT_RECORD_BYTES_V1 {
        return Err(UpkeepVaultSbfErrorV1::Create.into());
    }
    let record = UpkeepVaultV1::genesis(bump);
    write_record(vault, record)?;
    set_return_data(
        &UpkeepCreditReceiptV1 {
            operation: UpkeepOperationV1::Found,
            source_class: UpkeepSourceClassV1::Residue,
            request_digest,
            vault: vault.key.to_bytes(),
            amount: 0,
            inflow_total_after: 0,
            outflow_total_after: 0,
            credit_count_after: 0,
        }
        .to_bytes(),
    );
    Ok(())
}

#[inline(never)]
fn credit_route<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    vault: &AccountInfo<'info>,
    credit: UpkeepCreditV1,
    request_digest: [u8; 32],
) -> ProgramResult {
    if vault.owner != program_id || vault.data_len() != UPKEEP_VAULT_RECORD_BYTES_V1 {
        return Err(UpkeepVaultSbfErrorV1::Vault.into());
    }
    let record = read_record(vault)?;
    let rent_account = match credit.caller {
        None => deposit_frame(accounts, vault, credit)?,
        Some(caller) => protocol_frame(accounts, vault, credit, caller, request_digest)?,
    };
    let rent = Rent::from_account_info(rent_account).map_err(|_| UpkeepVaultSbfErrorV1::Create)?;
    let minimum = rent.minimum_balance(UPKEEP_VAULT_RECORD_BYTES_V1);
    // THE RECOGNITION CONJUNCT. Everything above only decided who may name the
    // class; this decides whether the lamports are there to be named.
    let unreceipted = record
        .unreceipted(vault.lamports(), minimum)
        .map_err(|_| UpkeepVaultSbfErrorV1::Legibility)?;
    if unreceipted < credit.amount {
        return Err(UpkeepVaultSbfErrorV1::Unreceipted.into());
    }
    let after = record
        .credit(credit.source_class, credit.amount, request_digest)
        .map_err(UpkeepVaultSbfErrorV1::from)?;
    write_record(vault, after)?;
    set_return_data(
        &UpkeepCreditReceiptV1 {
            operation: UpkeepOperationV1::Credit,
            source_class: credit.source_class,
            request_digest,
            vault: vault.key.to_bytes(),
            amount: credit.amount,
            inflow_total_after: after.inflow_total,
            outflow_total_after: after.outflow_total,
            credit_count_after: after.credit_count,
        }
        .to_bytes(),
    );
    Ok(())
}

/// `[vault, depositor, system_program, rent_sysvar]`: a signer's own lamports,
/// moved here by this route and then receipted. Returns the Rent sysvar.
#[inline(never)]
fn deposit_frame<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    vault: &AccountInfo<'info>,
    credit: UpkeepCreditV1,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    if accounts.len() != UPKEEP_DEPOSIT_CREDIT_ACCOUNT_COUNT_V1 {
        return Err(UpkeepVaultSbfErrorV1::AccountFrame.into());
    }
    if !credit.source_class.is_voluntary() {
        return Err(UpkeepVaultSbfErrorV1::SourceClass.into());
    }
    let depositor = account(accounts, 1).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let system = account(accounts, 2).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let rent_account = account(accounts, 3).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    if !depositor.is_signer
        || !depositor.is_writable
        || depositor.key == vault.key
        || system.key != &system_program::ID
        || rent_account.key != &sysvar::rent::ID
    {
        return Err(UpkeepVaultSbfErrorV1::AccountFrame.into());
    }
    invoke(
        &transfer(depositor.key, vault.key, credit.amount),
        &[depositor.clone(), vault.clone(), system.clone()],
    )
    .map_err(|_| UpkeepVaultSbfErrorV1::Unreceipted)?;
    Ok(rent_account)
}

/// `[vault, caller_authority, activation_cache, registry_program,
/// caller_program, caller_programdata, rent_sysvar]`: a release-set role's
/// word for lamports it already moved here. The caller authority is the
/// role's PDA under `CallerAuthoritySeedsV1` over THIS request's digest, so a
/// stranger cannot replay a program's credit under another amount, and the
/// role is the one the activation cache names for the calling program.
#[inline(never)]
fn protocol_frame<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    vault: &AccountInfo<'info>,
    credit: UpkeepCreditV1,
    caller: UpkeepProtocolCallerV1,
    request_digest: [u8; 32],
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    if accounts.len() != UPKEEP_PROTOCOL_CREDIT_ACCOUNT_COUNT_V1 {
        return Err(UpkeepVaultSbfErrorV1::AccountFrame.into());
    }
    if credit.source_class.is_voluntary() || accounts.iter().skip(2).any(|info| info.is_signer) {
        return Err(UpkeepVaultSbfErrorV1::SourceClass.into());
    }
    let caller_authority = account(accounts, 1).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let cache_account = account(accounts, 2).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let registry = account(accounts, 3).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let caller_program = account(accounts, 4).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let caller_programdata =
        account(accounts, 5).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    let rent_account = account(accounts, 6).map_err(|_| UpkeepVaultSbfErrorV1::AccountFrame)?;
    if !caller_authority.is_signer
        || rent_account.key != &sysvar::rent::ID
        || caller_authority.key == vault.key
    {
        return Err(UpkeepVaultSbfErrorV1::AccountFrame.into());
    }
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(caller.release_set).map_err(|_| UpkeepVaultSbfErrorV1::Release)?,
        caller.market,
        registry_role(caller.caller_role),
        caller.context,
        request_digest,
    )
    .map_err(|_| UpkeepVaultSbfErrorV1::CallerAuthority)?;
    let expected_caller =
        Pubkey::find_program_address(&caller_seeds.as_slices(), caller_program.key).0;
    if caller_authority.key != &expected_caller {
        return Err(UpkeepVaultSbfErrorV1::CallerAuthority.into());
    }
    // The release: the cache names the calling program under the role the
    // request claims. Read, never CPI'd; the same discipline as
    // `authenticate_calling_release`.
    require_cache_account(registry.key, cache_account)
        .map_err(|_| UpkeepVaultSbfErrorV1::Release)?;
    let cache_data = cache_account
        .try_borrow_data()
        .map_err(|_| UpkeepVaultSbfErrorV1::Release)?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache_data)
        .map_err(|_| UpkeepVaultSbfErrorV1::Release)?;
    authenticate_activation_cache_identity_v1(
        registry,
        cache_account,
        &caller.release_set,
        activated,
    )
    .map_err(|_| UpkeepVaultSbfErrorV1::Release)?;
    // The caller PDA continues to sign after an upgrade; authenticate the
    // live deployment here, not only the activation cache's stored keys.
    dclutch_registry::activation_auth_v1::authenticate_activated_role_in_frame_v1(
        cache_account,
        activated,
        registry_role(caller.caller_role),
        caller_program,
        caller_programdata,
    )
    .map_err(UpkeepVaultSbfErrorV1::from)?;
    Ok(rent_account)
}

fn read_record(vault: &AccountInfo<'_>) -> Result<UpkeepVaultV1, ProgramError> {
    let data = vault
        .try_borrow_data()
        .map_err(|_| UpkeepVaultSbfErrorV1::Vault)?;
    let record = UpkeepVaultV1::decode(&data).map_err(UpkeepVaultSbfErrorV1::from)?;
    // The record authenticates its own address through its bump.
    let [domain] = UpkeepVaultSeedsV1.as_slices();
    let bump = [record.bump];
    let expected = Pubkey::create_program_address(&[domain, &bump], vault.owner)
        .map_err(|_| UpkeepVaultSbfErrorV1::Vault)?;
    if vault.key != &expected {
        return Err(UpkeepVaultSbfErrorV1::Vault.into());
    }
    Ok(record)
}

fn write_record(vault: &AccountInfo<'_>, record: UpkeepVaultV1) -> ProgramResult {
    let mut data = vault
        .try_borrow_mut_data()
        .map_err(|_| UpkeepVaultSbfErrorV1::Commit)?;
    if data.len() != UPKEEP_VAULT_RECORD_BYTES_V1 {
        return Err(UpkeepVaultSbfErrorV1::Commit.into());
    }
    data.copy_from_slice(&record.to_bytes());
    Ok(())
}

const _: () = assert!(UPKEEP_VAULT_RECEIPT_BYTES_V1 == 112);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sub_band_starts_where_custody_reserved_it() {
        assert_eq!(
            UpkeepVaultSbfErrorV1::Instruction as u32,
            dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 0x200
        );
        assert_eq!(UpkeepVaultSbfErrorV1::ALL.len(), 11);
    }

    /// The hostile the ruling asked for: a spend is refused by name at the
    /// contract, and the adapter carries that name to the wire.
    #[test]
    fn a_spend_is_refused_by_name_on_the_wire() {
        let mut hostile = UpkeepRequestV1::FOUND.to_bytes().expect("encodes");
        hostile[10] = dclutch_custody::upkeep_vault_v1::UPKEEP_OPERATION_DEBIT_TAG_RESERVED_V1;
        assert!(selects(&hostile));
        assert_eq!(
            UpkeepRequestV1::decode(&hostile).map_err(UpkeepVaultSbfErrorV1::from),
            Err(UpkeepVaultSbfErrorV1::NoSpendRoute)
        );
    }
}

#[cfg(test)]
mod release_continuity_tests {
    use super::*;
    use crate::release_continuity_tests::{CASES, Case, Fixture, info};

    #[test]
    fn upkeep_calling_release_requires_live_loader_continuity() {
        let mut outcomes = Vec::new();
        let mut expected_outcomes = Vec::new();
        for case in CASES {
            let mut fixture = Fixture::new();
            fixture.mutate(case);
            let caller = UpkeepProtocolCallerV1 {
                caller_role: CallerRoleV1::Trading,
                release_set: fixture.release_set,
                market: [0x12; 32],
                context: [0x14; 32],
            };
            let digest = [0x15; 32];
            let seeds = CallerAuthoritySeedsV1::new(
                ContentId::new(caller.release_set).expect("valid continuity fixture"),
                caller.market,
                registry_role(caller.caller_role),
                caller.context,
                digest,
            )
            .expect("valid continuity fixture");
            let caller_key =
                Pubkey::find_program_address(&seeds.as_slices(), fixture.program.key).0;
            let mut authority = info(caller_key, system_program::ID, alloc::vec![], false);
            authority.is_signer = true;
            let vault = info(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                alloc::vec![],
                false,
            );
            let rent = info(sysvar::rent::ID, sysvar::ID, alloc::vec![], false);
            let accounts = [
                vault.clone(),
                authority,
                fixture.cache,
                fixture.registry,
                fixture.program,
                fixture.programdata,
                rent,
            ];
            let credit = UpkeepCreditV1 {
                source_class: UpkeepSourceClassV1::Residue,
                caller: Some(caller),
                receipt_digest: [0x16; 32],
                amount: 1,
            };
            let expected = match case {
                Case::Accepted => Ok(()),
                Case::Slot => Err(UpkeepVaultSbfErrorV1::ReleaseSuperseded.into()),
                _ => Err(UpkeepVaultSbfErrorV1::Release.into()),
            };
            outcomes.push(protocol_frame(&accounts, &vault, credit, caller, digest).map(|_| ()));
            expected_outcomes.push(expected);
        }
        assert_eq!(outcomes, expected_outcomes, "case order: {CASES:?}");
    }
}
