//! The governed protocol-parameters record's four routes.
//!
//! Decision 0024, ember's amendment: one record with a named authority, a
//! change delay and a census-readable receipt. The contract
//! (`dclutch_market::protocol_parameters`) owns the bands, the procedure and
//! every refusal about a VALUE; this adapter owns the accounts: the record's
//! PDA, who is the authority (a signer compared against the record), what
//! slot it is (`Clock::get`), and where a receipt goes.
//!
//! # Founding, and who may
//!
//! The record is born holding today's economics with the founder as the
//! placeholder authority (decision 0024 section 3). The founder must be the
//! Custody program's own current upgrade authority, read off this program's
//! ProgramData: that key IS "the deployer key, named as a placeholder", and
//! reading it there makes the founding race-free -- a stranger who founds
//! first would otherwise be governance. A program with no upgrade authority
//! founds a FROZEN record (authority zero): immutable code, immutable
//! economics.
//!
//! # The receipt stream
//!
//! Every applied change writes one 112-byte receipt at
//! `[receipt-domain, generation]`, rent paid by the applier, so a census walks
//! generations 1..n without a scan and a gap is a gap it can name.

use super::*;

use dclutch_market::protocol_parameters::{
    GovernanceActV1, PROTOCOL_PARAMETERS_APPLY_ACCOUNT_COUNT_V1,
    PROTOCOL_PARAMETERS_AUTHORITY_ACCOUNT_COUNT_V1, PROTOCOL_PARAMETERS_FOUND_ACCOUNT_COUNT_V1,
    PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1, PROTOCOL_PARAMETERS_RECORD_BYTES_V1,
    PROTOCOL_PARAMETERS_REQUEST_BYTES_V1, PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1, PendingChangeV1,
    ProtocolParametersReceiptSeedsV1, ProtocolParametersRecordSeedsV1, ProtocolParametersRecordV1,
    ProtocolParametersRequestV1, ProtocolParametersV1,
};
use dclutch_registry::svm::ProgramDataMetadataV3View;
use solana_program::{clock::Clock, sysvar::Sysvar};
use solana_sdk_ids::bpf_loader_upgradeable;

/// Stable refusal from the record's routes: sub-band `0x6100` of Custody's band.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolParametersSbfErrorV1 {
    /// Instruction bytes did not decode as the one generated request.
    Instruction = 0x6100,
    /// Account count, order, privileges, or aliases were not exact.
    AccountFrame = 0x6101,
    /// The record PDA, its owner, or its bytes refused, or it is unfounded.
    Record = 0x6102,
    /// Rent, payer, System program, or account creation refused.
    Create = 0x6103,
    /// The founder is not this program's current upgrade authority, or the
    /// founding body is not the genesis body under that authority.
    FoundingAuthority = 0x6104,
    /// The authority is the zero key: this record is frozen forever.
    GovernanceFrozen = 0x6105,
    /// The signer is not this record's governance authority.
    UnauthorizedGovernance = 0x6106,
    /// A proposal already stands; withdraw it or wait it out.
    ProposalOutstanding = 0x6107,
    /// The proposed value is outside a constitutional band.
    ParameterOutOfBand = 0x6108,
    /// Nothing has been proposed.
    NoPendingProposal = 0x6109,
    /// The change delay has not elapsed.
    ProposalNotMatured = 0x610A,
    /// The bytes offered are not the bytes the proposal pinned.
    ProposalDigestMismatch = 0x610B,
    /// A nonzero protocol take in a release that admits none (decision 0024
    /// item 1, lifted only by the mainnet ruling's release).
    TakeBeforeMainnet = 0x610C,
    /// The receipt PDA, its creation, or its bytes refused.
    Receipt = 0x610D,
    /// The record could not be rewritten after the act was computed.
    Commit = 0x610E,
    /// A slot or generation sum did not fit `u64`.
    Arithmetic = 0x610F,
}

dclutch_refusal_registry::pin_refusal_band!(
    ProtocolParametersSbfErrorV1,
    dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 0x100,
    [
        Instruction,
        AccountFrame,
        Record,
        Create,
        FoundingAuthority,
        GovernanceFrozen,
        UnauthorizedGovernance,
        ProposalOutstanding,
        ParameterOutOfBand,
        NoPendingProposal,
        ProposalNotMatured,
        ProposalDigestMismatch,
        TakeBeforeMainnet,
        Receipt,
        Commit,
        Arithmetic
    ]
);

impl From<dclutch_market::protocol_parameters::Error> for ProtocolParametersSbfErrorV1 {
    fn from(value: dclutch_market::protocol_parameters::Error) -> Self {
        use dclutch_market::protocol_parameters::Error;
        match value {
            Error::GovernanceFrozen => Self::GovernanceFrozen,
            Error::UnauthorizedGovernance => Self::UnauthorizedGovernance,
            Error::ProposalOutstanding => Self::ProposalOutstanding,
            Error::ParameterOutOfBand => Self::ParameterOutOfBand,
            Error::NoPendingProposal => Self::NoPendingProposal,
            Error::ProposalNotMatured => Self::ProposalNotMatured,
            Error::ProposalDigestMismatch => Self::ProposalDigestMismatch,
            Error::TakeBeforeMainnet => Self::TakeBeforeMainnet,
            Error::ArithmeticOverflow => Self::Arithmetic,
            Error::InvalidLength
            | Error::InvalidHeader
            | Error::NonCanonical
            | Error::UnknownAct => Self::Instruction,
        }
    }
}

/// Whether `instruction_data` is this family's wire: exact width and magic.
pub(super) fn selects(instruction_data: &[u8]) -> bool {
    instruction_data.len() == PROTOCOL_PARAMETERS_REQUEST_BYTES_V1
        && instruction_data.get(..PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1.len())
            == Some(PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1.as_slice())
}

const RECORD: usize = 0;

/// Dispatch one governance act.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let request = ProtocolParametersRequestV1::decode(instruction_data)
        .map_err(ProtocolParametersSbfErrorV1::from)?;
    let record_account =
        account(accounts, RECORD).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    let (expected, bump) =
        Pubkey::find_program_address(&ProtocolParametersRecordSeedsV1.as_slices(), program_id);
    if record_account.key != &expected || !record_account.is_writable || record_account.is_signer {
        return Err(ProtocolParametersSbfErrorV1::Record.into());
    }
    match request.act {
        GovernanceActV1::Found => found(program_id, accounts, record_account, bump, request.body),
        GovernanceActV1::Propose => propose(program_id, accounts, record_account, request.body),
        GovernanceActV1::Withdraw => withdraw(program_id, accounts, record_account),
        GovernanceActV1::Apply => apply(program_id, accounts, record_account, request.body),
    }
}

/// `[record, founder, custody_programdata, system_program, rent_sysvar]`.
#[inline(never)]
fn found<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    record_account: &AccountInfo<'info>,
    bump: u8,
    body: ProtocolParametersV1,
) -> ProgramResult {
    if accounts.len() != PROTOCOL_PARAMETERS_FOUND_ACCOUNT_COUNT_V1 {
        return Err(ProtocolParametersSbfErrorV1::AccountFrame.into());
    }
    let founder = account(accounts, 1).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    let programdata =
        account(accounts, 2).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    let system = account(accounts, 3).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    let rent_account =
        account(accounts, 4).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    if !founder.is_signer
        || !founder.is_writable
        || system.key != &system_program::ID
        || rent_account.key != &sysvar::rent::ID
        || founder.key == record_account.key
    {
        return Err(ProtocolParametersSbfErrorV1::AccountFrame.into());
    }
    if record_account.owner != &system_program::ID || record_account.data_len() != 0 {
        return Err(ProtocolParametersSbfErrorV1::Record.into());
    }
    // THIS program's ProgramData, and its current upgrade authority.
    let expected_programdata =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::ID).0;
    if programdata.key != &expected_programdata || programdata.owner != &bpf_loader_upgradeable::ID
    {
        return Err(ProtocolParametersSbfErrorV1::FoundingAuthority.into());
    }
    let metadata = {
        let data = programdata
            .try_borrow_data()
            .map_err(|_| ProtocolParametersSbfErrorV1::FoundingAuthority)?;
        ProgramDataMetadataV3View::parse(&data)
            .map_err(|_| ProtocolParametersSbfErrorV1::FoundingAuthority)?
    };
    // An immutable program founds a frozen record: nobody may propose, and
    // the one-way door is shut from the first byte.
    let authority = match metadata.upgrade_authority() {
        Some(authority) => {
            if authority != founder.key.to_bytes() {
                return Err(ProtocolParametersSbfErrorV1::FoundingAuthority.into());
            }
            authority
        }
        None => [0; 32],
    };
    // The founding body is the genesis body under that authority and nothing
    // else: a founding cannot smuggle a value past the change delay.
    if body != ProtocolParametersV1::genesis(authority) {
        return Err(ProtocolParametersSbfErrorV1::FoundingAuthority.into());
    }
    let rent =
        Rent::from_account_info(rent_account).map_err(|_| ProtocolParametersSbfErrorV1::Create)?;
    let minimum = rent.minimum_balance(PROTOCOL_PARAMETERS_RECORD_BYTES_V1);
    let shortfall = minimum.saturating_sub(record_account.lamports());
    let bump_seed = [bump];
    let [domain] = ProtocolParametersRecordSeedsV1.as_slices();
    let signer_seeds = &[domain, &bump_seed];
    if shortfall != 0 {
        invoke(
            &transfer(founder.key, record_account.key, shortfall),
            &[founder.clone(), record_account.clone(), system.clone()],
        )
        .map_err(|_| ProtocolParametersSbfErrorV1::Create)?;
    }
    invoke_signed(
        &allocate(
            record_account.key,
            u64::try_from(PROTOCOL_PARAMETERS_RECORD_BYTES_V1)
                .map_err(|_| ProtocolParametersSbfErrorV1::Create)?,
        ),
        &[record_account.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| ProtocolParametersSbfErrorV1::Create)?;
    invoke_signed(
        &assign(record_account.key, program_id),
        &[record_account.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| ProtocolParametersSbfErrorV1::Create)?;
    if record_account.owner != program_id
        || record_account.data_len() != PROTOCOL_PARAMETERS_RECORD_BYTES_V1
    {
        return Err(ProtocolParametersSbfErrorV1::Create.into());
    }
    write_record(
        record_account,
        ProtocolParametersRecordV1 {
            bump,
            parameters: ProtocolParametersV1::genesis(authority),
            pending: PendingChangeV1::NONE,
        },
    )
}

/// `[record, authority]`: the authority's act.
#[inline(never)]
fn propose<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    record_account: &AccountInfo<'info>,
    body: ProtocolParametersV1,
) -> ProgramResult {
    let (record, signer_is_authority) = authority_frame(program_id, accounts, record_account)?;
    let slot = Clock::get()
        .map_err(|_| ProtocolParametersSbfErrorV1::Arithmetic)?
        .slot;
    let after = record
        .propose(signer_is_authority, body, slot)
        .map_err(ProtocolParametersSbfErrorV1::from)?;
    write_record(record_account, after)
}

/// `[record, authority]`: the authority's act, and nobody else's.
#[inline(never)]
fn withdraw<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    record_account: &AccountInfo<'info>,
) -> ProgramResult {
    let (record, signer_is_authority) = authority_frame(program_id, accounts, record_account)?;
    let after = record
        .withdraw(signer_is_authority)
        .map_err(ProtocolParametersSbfErrorV1::from)?;
    write_record(record_account, after)
}

/// `[record, receipt, payer, system_program, rent_sysvar]`: anybody's act.
///
/// A governed change is still a crank: an authority that had to show up twice
/// could propose and then decline to finish. The payer funds the receipt's
/// rent and gains nothing but the receipt.
#[inline(never)]
fn apply<'info>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'info>],
    record_account: &AccountInfo<'info>,
    body: ProtocolParametersV1,
) -> ProgramResult {
    if accounts.len() != PROTOCOL_PARAMETERS_APPLY_ACCOUNT_COUNT_V1 {
        return Err(ProtocolParametersSbfErrorV1::AccountFrame.into());
    }
    let receipt_account =
        account(accounts, 1).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    let payer = account(accounts, 2).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    let system = account(accounts, 3).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    let rent_account =
        account(accounts, 4).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    if !payer.is_signer
        || !payer.is_writable
        || !receipt_account.is_writable
        || receipt_account.is_signer
        || system.key != &system_program::ID
        || rent_account.key != &sysvar::rent::ID
        || receipt_account.key == record_account.key
        || payer.key == receipt_account.key
    {
        return Err(ProtocolParametersSbfErrorV1::AccountFrame.into());
    }
    let record = read_record(program_id, record_account)?;
    let slot = Clock::get()
        .map_err(|_| ProtocolParametersSbfErrorV1::Arithmetic)?
        .slot;
    let (after, receipt) = record
        .apply_change(body, slot)
        .map_err(ProtocolParametersSbfErrorV1::from)?;
    // The receipt at its generation's address, created here so a change that
    // could not be receipted is a change that did not happen.
    let seeds = ProtocolParametersReceiptSeedsV1::new(receipt.generation);
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), program_id);
    if receipt_account.key != &expected
        || receipt_account.owner != &system_program::ID
        || receipt_account.data_len() != 0
    {
        return Err(ProtocolParametersSbfErrorV1::Receipt.into());
    }
    let rent =
        Rent::from_account_info(rent_account).map_err(|_| ProtocolParametersSbfErrorV1::Create)?;
    let minimum = rent.minimum_balance(PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1);
    let shortfall = minimum.saturating_sub(receipt_account.lamports());
    let bump_seed = [bump];
    let [domain, generation] = seeds.as_slices();
    let signer_seeds = &[domain, generation, &bump_seed];
    if shortfall != 0 {
        invoke(
            &transfer(payer.key, receipt_account.key, shortfall),
            &[payer.clone(), receipt_account.clone(), system.clone()],
        )
        .map_err(|_| ProtocolParametersSbfErrorV1::Receipt)?;
    }
    invoke_signed(
        &allocate(
            receipt_account.key,
            u64::try_from(PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1)
                .map_err(|_| ProtocolParametersSbfErrorV1::Receipt)?,
        ),
        &[receipt_account.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| ProtocolParametersSbfErrorV1::Receipt)?;
    invoke_signed(
        &assign(receipt_account.key, program_id),
        &[receipt_account.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| ProtocolParametersSbfErrorV1::Receipt)?;
    {
        let mut data = receipt_account
            .try_borrow_mut_data()
            .map_err(|_| ProtocolParametersSbfErrorV1::Receipt)?;
        if data.len() != PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1 {
            return Err(ProtocolParametersSbfErrorV1::Receipt.into());
        }
        data.copy_from_slice(&receipt.to_bytes());
    }
    write_record(record_account, after)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

/// `[record, authority]`, shared by propose and withdraw. Whether the signer
/// IS the authority is decided here and handed to the contract as a bool,
/// exactly as its docstring asks; a frame with no signer at all is a frame
/// with no authority, and the contract refuses it by name.
fn authority_frame<'a>(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'a>],
    record_account: &AccountInfo<'a>,
) -> Result<(ProtocolParametersRecordV1, bool), ProgramError> {
    if accounts.len() != PROTOCOL_PARAMETERS_AUTHORITY_ACCOUNT_COUNT_V1 {
        return Err(ProtocolParametersSbfErrorV1::AccountFrame.into());
    }
    let signer = account(accounts, 1).map_err(|_| ProtocolParametersSbfErrorV1::AccountFrame)?;
    if signer.key == record_account.key {
        return Err(ProtocolParametersSbfErrorV1::AccountFrame.into());
    }
    let record = read_record(program_id, record_account)?;
    let signer_is_authority =
        signer.is_signer && signer.key.to_bytes() == record.parameters.governance_authority;
    Ok((record, signer_is_authority))
}

fn read_record(
    program_id: &Pubkey,
    record_account: &AccountInfo<'_>,
) -> Result<ProtocolParametersRecordV1, ProgramError> {
    if record_account.owner != program_id
        || record_account.data_len() != PROTOCOL_PARAMETERS_RECORD_BYTES_V1
    {
        return Err(ProtocolParametersSbfErrorV1::Record.into());
    }
    let data = record_account
        .try_borrow_data()
        .map_err(|_| ProtocolParametersSbfErrorV1::Record)?;
    let record = ProtocolParametersRecordV1::decode(&data).map_err(|error| {
        // A persisted take is the one decode refusal that is a verdict.
        match error {
            dclutch_market::protocol_parameters::Error::TakeBeforeMainnet => {
                ProtocolParametersSbfErrorV1::TakeBeforeMainnet
            }
            dclutch_market::protocol_parameters::Error::ParameterOutOfBand => {
                ProtocolParametersSbfErrorV1::ParameterOutOfBand
            }
            _ => ProtocolParametersSbfErrorV1::Record,
        }
    })?;
    let [domain] = ProtocolParametersRecordSeedsV1.as_slices();
    let bump = [record.bump];
    let expected = Pubkey::create_program_address(&[domain, &bump], program_id)
        .map_err(|_| ProtocolParametersSbfErrorV1::Record)?;
    if record_account.key != &expected {
        return Err(ProtocolParametersSbfErrorV1::Record.into());
    }
    Ok(record)
}

fn write_record(
    record_account: &AccountInfo<'_>,
    record: ProtocolParametersRecordV1,
) -> ProgramResult {
    let mut data = record_account
        .try_borrow_mut_data()
        .map_err(|_| ProtocolParametersSbfErrorV1::Commit)?;
    if data.len() != PROTOCOL_PARAMETERS_RECORD_BYTES_V1 {
        return Err(ProtocolParametersSbfErrorV1::Commit.into());
    }
    data.copy_from_slice(&record.to_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sub_band_starts_where_custody_reserved_it_and_names_the_take() {
        assert_eq!(
            ProtocolParametersSbfErrorV1::Instruction as u32,
            dclutch_refusal_registry::CUSTODY_REFUSAL_BASE + 0x100
        );
        assert_eq!(ProtocolParametersSbfErrorV1::ALL.len(), 16);
        assert_eq!(
            ProtocolParametersSbfErrorV1::from(
                dclutch_market::protocol_parameters::Error::TakeBeforeMainnet
            ),
            ProtocolParametersSbfErrorV1::TakeBeforeMainnet
        );
    }
}
