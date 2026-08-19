//! Permissionless, typed upload and sealing of immutable protocol artifacts.
//!
//! Terms are 1,656 bytes, so they cannot travel in one Solana transaction.
//! This family creates an uploader-keyed staging PDA at its exact final body
//! size, accepts only the next 192-byte chunk, and seals only after the whole
//! body passes its pre-existing hostile-byte codec and semantic digest check.
//! The final account contains the exact historical raw Policy, PriceGrid, or
//! Terms bytes at its content-derived PDA; it never contains a generic blob
//! wrapper and consumers never read the staging account.
//!
//! The stage's funder is its sole writer and sealer.  It may abort at any
//! time.  After the frozen expiry slot any signer may reap the abandoned stage,
//! but every lamport still returns to the funder stored in the stage header.
//! Hoard principal, collateral, protocol fees, and the reaper are never rent
//! sources or refund destinations.

use crate::accounts::{expect_pda, require, require_count, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_solana_layout::artifact::{
    self, ArtifactBinding, ArtifactKind, ArtifactStageHeader, ARTIFACT_CHUNK_BYTES,
};
#[cfg(target_os = "solana")]
use clutch_solana_layout::{CodecError, TermsAccount, HASH_BYTES};
use clutch_solana_layout::{Hash32, Intent};
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::genesis::{
    read_rent, require_system_program, RentParameters, MAX_PERMITTED_DATA_INCREASE,
    SYSTEM_PROGRAM_ID,
};

/// Shortest stage lifetime admitted at creation.
pub const MIN_UPLOAD_LIFETIME_SLOTS: u64 = 8;
/// Longest stage lifetime admitted at creation.
///
/// This is a resource bound, not a wall-clock promise: Solana slots have no
/// exact duration guarantee.  An uploader that needs longer may abort and
/// restart at the same uploader-keyed PDA after the old stage is closed.
pub const MAX_UPLOAD_LIFETIME_SLOTS: u64 = 432_000;

// `SystemInstruction` is bincode-encoded.  These are the frozen enum variant
// indices and payload widths for Assign, Transfer, and Allocate in
// `solana-system-interface`.  Artifact creation deliberately uses the three
// instructions rather than CreateAccount: a third party can transfer lamports
// to any predictable PDA before it exists, and CreateAccount rejects that
// otherwise harmless prefund.
const SYSTEM_IX_ASSIGN: u32 = 1;
const SYSTEM_IX_TRANSFER: u32 = 2;
const SYSTEM_IX_ALLOCATE: u32 = 8;
const SYSTEM_TRANSFER_DATA_LEN: usize = 4 + 8;
const SYSTEM_ALLOCATE_DATA_LEN: usize = 4 + 8;
const SYSTEM_ASSIGN_DATA_LEN: usize = 4 + 32;

const _: () = {
    assert!(MIN_UPLOAD_LIFETIME_SLOTS > 0);
    assert!(MAX_UPLOAD_LIFETIME_SLOTS >= MIN_UPLOAD_LIFETIME_SLOTS);
};

/// Clock sysvar address, `SysvarC1ock11111111111111111111111111111111`.
pub const CLOCK_SYSVAR_ID: Pubkey = Pubkey::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
/// Bincode length of the Solana Clock sysvar.
pub const CLOCK_SYSVAR_LEN: usize = 8 + 8 + 8 + 8 + 8;

/// Begin account count: funder, stage, System program, Rent, Clock.
pub const BEGIN_ACCOUNT_COUNT: usize = 5;
/// Write account count: funder, stage, Clock.
pub const WRITE_ACCOUNT_COUNT: usize = 3;
/// Seal account count: funder, stage, final, System program, Rent, Clock.
pub const SEAL_ACCOUNT_COUNT: usize = 6;
/// Abort account count: caller, stage, recorded funder, Clock.
pub const ABORT_ACCOUNT_COUNT: usize = 4;

/// Signer that funds and owns an upload lifecycle.
pub const IX_FUNDER: usize = 0;
/// Uploader-keyed staging PDA.
pub const IX_STAGE: usize = 1;
/// System program in Begin and Seal.
pub const IX_BEGIN_SYSTEM: usize = 2;
/// Rent sysvar in Begin and Seal.
pub const IX_BEGIN_RENT: usize = 3;
/// Clock sysvar in Begin.
pub const IX_BEGIN_CLOCK: usize = 4;
/// Clock sysvar in Write.
pub const IX_WRITE_CLOCK: usize = 2;
/// Final content-derived artifact PDA in Seal.
pub const IX_FINAL: usize = 2;
/// System program in Seal.
pub const IX_SEAL_SYSTEM: usize = 3;
/// Rent sysvar in Seal.
pub const IX_SEAL_RENT: usize = 4;
/// Clock sysvar in Seal.
pub const IX_SEAL_CLOCK: usize = 5;
/// Any signer may call Abort after expiry; before expiry it must be the funder.
pub const IX_ABORT_CALLER: usize = 0;
/// Recorded funder and sole rent-refund destination in Abort.
pub const IX_ABORT_REFUND: usize = 2;
/// Clock sysvar in Abort.
pub const IX_ABORT_CLOCK: usize = 3;

/// Decode the current slot from an authenticated Clock sysvar account.
pub fn read_clock_slot(account: &AccountInfo) -> Outcome<u64> {
    require(
        *account.key == CLOCK_SYSVAR_ID,
        ClutchError::WrongClockSysvar,
    )?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        account.data_len() == CLOCK_SYSVAR_LEN,
        ClutchError::WrongClockSysvar,
    )?;
    let data = account.data.borrow();
    let mut slot = [0; 8];
    slot.copy_from_slice(&data[..8]);
    Ok(u64::from_le_bytes(slot))
}

fn require_zero_sequence(sequence: u64) -> Outcome<()> {
    require(sequence == 0, ClutchError::Replay)
}

fn require_artifact_creation_target(target: &AccountInfo) -> Outcome<()> {
    require(target.is_writable, ClutchError::NotWritable)?;
    require(!target.executable, ClutchError::ExecutableAccount)?;
    require(
        target.data_len() == 0 && *target.owner == SYSTEM_PROGRAM_ID,
        ClutchError::AlreadyInitialized,
    )
}

fn system_transfer_data(lamports: u64) -> [u8; SYSTEM_TRANSFER_DATA_LEN] {
    let mut data = [0_u8; SYSTEM_TRANSFER_DATA_LEN];
    data[..4].copy_from_slice(&SYSTEM_IX_TRANSFER.to_le_bytes());
    data[4..].copy_from_slice(&lamports.to_le_bytes());
    data
}

fn system_allocate_data(space: usize) -> [u8; SYSTEM_ALLOCATE_DATA_LEN] {
    let mut data = [0_u8; SYSTEM_ALLOCATE_DATA_LEN];
    data[..4].copy_from_slice(&SYSTEM_IX_ALLOCATE.to_le_bytes());
    data[4..].copy_from_slice(&(space as u64).to_le_bytes());
    data
}

fn system_assign_data(owner: &Pubkey) -> [u8; SYSTEM_ASSIGN_DATA_LEN] {
    let mut data = [0_u8; SYSTEM_ASSIGN_DATA_LEN];
    data[..4].copy_from_slice(&SYSTEM_IX_ASSIGN.to_le_bytes());
    data[4..].copy_from_slice(&owner.to_bytes());
    data
}

/// Allocate and assign an exact artifact PDA even if someone prefunded it.
///
/// Any lamports already present are an unsolicited donation, never identity,
/// authority, a fee credit, or a refund claim.  The payer supplies exactly the
/// rent shortfall.  A persistent final retains any excess; a transient stage
/// follows its one frozen close destination, so all of its lamports eventually
/// go to the recorded funder on seal, abort, or reap.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn create_artifact_pda<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_artifact_creation_target(target)?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;

    let initial_lamports = target.lamports();
    let minimum_balance = rent.minimum_balance(space)?;
    let shortfall = minimum_balance.saturating_sub(initial_lamports);
    let funded_balance = initial_lamports
        .checked_add(shortfall)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;

    if shortfall != 0 {
        let transfer = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &system_transfer_data(shortfall),
            vec![
                AccountMeta::new(*payer.key, true),
                AccountMeta::new(*target.key, false),
            ],
        );
        invoke(
            &transfer,
            &[payer.clone(), target.clone(), system_program.clone()],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
        require(
            target.lamports() == funded_balance
                && target.data_len() == 0
                && *target.owner == SYSTEM_PROGRAM_ID,
            ClutchError::AccountCreationFailed,
        )?;
    }

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &system_allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == funded_balance
            && target.data_len() == space
            && *target.owner == SYSTEM_PROGRAM_ID,
        ClutchError::AccountCreationFailed,
    )?;

    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &system_assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == funded_balance
            && target.data_len() == space
            && target.owner == program_id,
        ClutchError::AccountCreationFailed,
    )
}

fn require_stage_metadata(program_id: &Pubkey, stage: &AccountInfo) -> Outcome<()> {
    require(stage.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(stage.is_writable, ClutchError::NotWritable)?;
    require(!stage.executable, ClutchError::ExecutableAccount)
}

fn require_live(header: ArtifactStageHeader, slot: u64) -> Outcome<()> {
    require(slot <= header.expires_slot, ClutchError::ArtifactExpired)
}

fn require_funder(header: ArtifactStageHeader, funder: &AccountInfo) -> Outcome<()> {
    require_signer(funder)?;
    require(
        funder.key.to_bytes() == header.funder,
        ClutchError::UnauthorizedActor,
    )
}

fn require_stage_pda(
    program_id: &Pubkey,
    stage: &AccountInfo,
    header: ArtifactStageHeader,
) -> Outcome<()> {
    let kind = [header.binding.kind.byte()];
    expect_pda(
        stage.key,
        seeds::artifact_stage_pda(
            program_id,
            &header.funder,
            &kind,
            &header.binding.context.bytes(),
            &header.binding.digest.bytes(),
        ),
        Some(header.stored_bump),
    )
}

fn begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    binding: ArtifactBinding,
    expires_slot: u64,
) -> Outcome<()> {
    require_count(accounts, BEGIN_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require_signer(&accounts[IX_FUNDER])?;
    require(accounts[IX_FUNDER].is_writable, ClutchError::NotWritable)?;
    require(
        accounts[IX_FUNDER].key != accounts[IX_STAGE].key,
        ClutchError::AccountAlias,
    )?;
    binding.validate()?;
    require_system_program(&accounts[IX_BEGIN_SYSTEM])?;
    let rent = read_rent(&accounts[IX_BEGIN_RENT])?;
    let current_slot = read_clock_slot(&accounts[IX_BEGIN_CLOCK])?;
    let lifetime = expires_slot
        .checked_sub(current_slot)
        .ok_or(Refusal::Adapter(ClutchError::InvalidArtifactExpiry))?;
    require(
        (MIN_UPLOAD_LIFETIME_SLOTS..=MAX_UPLOAD_LIFETIME_SLOTS).contains(&lifetime),
        ClutchError::InvalidArtifactExpiry,
    )?;

    let funder = accounts[IX_FUNDER].key.to_bytes();
    let kind = [binding.kind.byte()];
    let context = binding.context.bytes();
    let digest = binding.digest.bytes();
    let (address, bump) = seeds::artifact_stage_pda(program_id, &funder, &kind, &context, &digest);
    expect_pda(accounts[IX_STAGE].key, (address, bump), None)?;
    let header = ArtifactStageHeader {
        binding,
        funder,
        cursor: 0,
        created_slot: current_slot,
        expires_slot,
        stored_bump: bump,
    };
    let space = header.account_len()?;
    create_artifact_pda(
        program_id,
        &accounts[IX_FUNDER],
        &accounts[IX_STAGE],
        &accounts[IX_BEGIN_SYSTEM],
        &rent,
        space,
        &[
            seeds::SEED_ARTIFACT_STAGE,
            &funder,
            &kind,
            &context,
            &digest,
            &[bump],
        ],
    )?;
    let mut data = accounts[IX_STAGE]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    artifact::initialize_stage(&mut data, &header)?;
    require(
        artifact::decode_stage(&data)? == header,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn write(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    binding: ArtifactBinding,
    cursor: u16,
    chunk_len: u16,
    chunk: &[u8; ARTIFACT_CHUNK_BYTES],
) -> Outcome<()> {
    require_count(accounts, WRITE_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require(
        accounts[IX_FUNDER].key != accounts[IX_STAGE].key,
        ClutchError::AccountAlias,
    )?;
    require_stage_metadata(program_id, &accounts[IX_STAGE])?;
    let current_slot = read_clock_slot(&accounts[IX_WRITE_CLOCK])?;
    let mut data = accounts[IX_STAGE]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let header = artifact::decode_stage(&data)?;
    require_funder(header, &accounts[IX_FUNDER])?;
    require_stage_pda(program_id, &accounts[IX_STAGE], header)?;
    require_live(header, current_slot)?;
    artifact::append_chunk(&mut data, binding, cursor, chunk_len, chunk)?;
    Ok(())
}

fn expected_final_pda(program_id: &Pubkey, binding: ArtifactBinding) -> (Pubkey, u8) {
    let context = binding.context.bytes();
    let digest = binding.digest.bytes();
    match binding.kind {
        ArtifactKind::CollateralPolicy => seeds::policy_pda(program_id, &context, &digest),
        ArtifactKind::PriceGrid => seeds::grid_pda(program_id, &context, &digest),
        ArtifactKind::Terms => seeds::terms_pda(program_id, &context, &digest),
    }
}

/// Validate a staged artifact without paying the layout crate's portable
/// software SHA-256 cost on the SBF target.
///
/// `clutch-solana-layout` deliberately stays dependency-free and implements
/// SHA-256 in fixed-array Rust.  That is the correct portable reference, but a
/// full 1,620-byte Terms preimage consumes more than the default Solana
/// transaction budget when interpreted as SBF instructions.  The adapter can
/// use Solana's authenticated SHA-256 syscall for the *same exact preimage*.
/// Every hostile-byte and semantic check still comes from the owning Terms
/// codec through `decode_unchecked_into`; "unchecked" here means only that the
/// portable digest recomputation is replaced immediately below.
#[inline(never)]
fn validate_for_runtime(binding: ArtifactBinding, body: &[u8]) -> Outcome<u8> {
    #[cfg(target_os = "solana")]
    if matches!(binding.kind, ArtifactKind::Terms) {
        const TERMS_DOMAIN: &[u8] = b"dragons-clutch/terms/v2";
        const TERMS_BODY_START: usize = 2 + HASH_BYTES;
        const TERMS_TRAILER_BYTES: usize = 2; // stored bump and flags

        binding.validate()?;
        require(
            body.len() == usize::from(binding.exact_len),
            ClutchError::WrongDataLength,
        )?;
        let mut terms = TermsAccount::ZEROED;
        TermsAccount::decode_unchecked_into(body, &mut terms)?;
        if terms.realm != binding.context || terms.terms != binding.digest {
            return Err(CodecError::MismatchedBinding.into());
        }
        let body_end = body
            .len()
            .checked_sub(TERMS_TRAILER_BYTES)
            .ok_or(Refusal::Codec(CodecError::Truncated))?;
        let preimage = body
            .get(TERMS_BODY_START..body_end)
            .ok_or(Refusal::Codec(CodecError::Truncated))?;
        let observed = solana_sha256_hasher::hashv(&[TERMS_DOMAIN, preimage]);
        if observed.to_bytes() != binding.digest.bytes() {
            return Err(CodecError::NonCanonicalIdentity.into());
        }
        return Ok(terms.stored_bump);
    }

    // Host tests remain an independent oracle over the portable implementation;
    // the other, much smaller artifact families also use it on SBF.
    artifact::validate_artifact(binding, body).map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
fn create_final<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    final_account: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    rent: &super::genesis::RentParameters,
    binding: ArtifactBinding,
    bump: u8,
) -> Outcome<()> {
    let context = binding.context.bytes();
    let digest = binding.digest.bytes();
    match binding.kind {
        ArtifactKind::CollateralPolicy => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[seeds::SEED_POLICY, &context, &digest, &[bump]],
        ),
        ArtifactKind::PriceGrid => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[seeds::SEED_GRID, &context, &digest, &[bump]],
        ),
        ArtifactKind::Terms => create_artifact_pda(
            program_id,
            payer,
            final_account,
            system,
            rent,
            usize::from(binding.exact_len),
            &[seeds::SEED_TERMS, &context, &digest, &[bump]],
        ),
    }
}

fn close_stage(stage: &AccountInfo, funder: &AccountInfo) -> Outcome<()> {
    require(stage.key != funder.key, ClutchError::AccountAlias)?;
    require(funder.is_writable, ClutchError::NotWritable)?;
    require(!funder.executable, ClutchError::ExecutableAccount)?;
    let refund = stage.lamports();
    let next = funder
        .lamports()
        .checked_add(refund)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    {
        let mut destination = funder
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **destination = next;
    }
    {
        let mut source = stage
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        **source = 0;
    }
    let mut data = stage
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.fill(0);
    Ok(())
}

fn seal(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    binding: ArtifactBinding,
) -> Outcome<()> {
    require_count(accounts, SEAL_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require(
        accounts[IX_FUNDER].key != accounts[IX_STAGE].key
            && accounts[IX_FUNDER].key != accounts[IX_FINAL].key
            && accounts[IX_STAGE].key != accounts[IX_FINAL].key,
        ClutchError::AccountAlias,
    )?;
    require(accounts[IX_FUNDER].is_writable, ClutchError::NotWritable)?;
    require_stage_metadata(program_id, &accounts[IX_STAGE])?;
    require(accounts[IX_FINAL].is_writable, ClutchError::NotWritable)?;
    require(
        !accounts[IX_FINAL].executable,
        ClutchError::ExecutableAccount,
    )?;
    require_system_program(&accounts[IX_SEAL_SYSTEM])?;
    let rent = read_rent(&accounts[IX_SEAL_RENT])?;
    let current_slot = read_clock_slot(&accounts[IX_SEAL_CLOCK])?;

    let stage_data = accounts[IX_STAGE].data.borrow();
    let header = artifact::decode_stage(&stage_data)?;
    require_funder(header, &accounts[IX_FUNDER])?;
    require_stage_pda(program_id, &accounts[IX_STAGE], header)?;
    require_live(header, current_slot)?;
    require(
        header.binding == binding,
        ClutchError::EvidenceBufferMismatch,
    )?;
    require(header.is_complete(), ClutchError::ArtifactIncomplete)?;
    let body = artifact::stage_payload(&stage_data)?;
    let encoded_bump = validate_for_runtime(binding, body)?;
    let (final_address, final_bump) = expected_final_pda(program_id, binding);
    expect_pda(accounts[IX_FINAL].key, (final_address, final_bump), None)?;
    if !matches!(binding.kind, ArtifactKind::CollateralPolicy) {
        require(encoded_bump == final_bump, ClutchError::WrongBump)?;
    }

    if accounts[IX_FINAL].owner == program_id {
        require(
            accounts[IX_FINAL].data_len() == usize::from(binding.exact_len),
            ClutchError::WrongDataLength,
        )?;
        let existing = accounts[IX_FINAL].data.borrow();
        validate_for_runtime(binding, &existing)?;
        require(
            existing.as_ref() == body,
            ClutchError::EvidenceBufferMismatch,
        )?;
    } else {
        create_final(
            program_id,
            &accounts[IX_FUNDER],
            &accounts[IX_FINAL],
            &accounts[IX_SEAL_SYSTEM],
            &rent,
            binding,
            final_bump,
        )?;
        let mut target = accounts[IX_FINAL]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(target.len() == body.len(), ClutchError::WrongDataLength)?;
        target.copy_from_slice(body);
        /* `body` was fully validated above, the target length is exact, and
         * this is a byte-for-byte copy in the same atomic instruction.  A
         * second decode/hash would establish no new fact. */
    }
    drop(stage_data);
    close_stage(&accounts[IX_STAGE], &accounts[IX_FUNDER])
}

fn abort(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
) -> Outcome<()> {
    require_count(accounts, ABORT_ACCOUNT_COUNT)?;
    require_zero_sequence(sequence)?;
    require_signer(&accounts[IX_ABORT_CALLER])?;
    require(
        accounts[IX_ABORT_CALLER].key != accounts[IX_STAGE].key
            && accounts[IX_ABORT_REFUND].key != accounts[IX_STAGE].key,
        ClutchError::AccountAlias,
    )?;
    require_stage_metadata(program_id, &accounts[IX_STAGE])?;
    let current_slot = read_clock_slot(&accounts[IX_ABORT_CLOCK])?;
    let data = accounts[IX_STAGE].data.borrow();
    let header = artifact::decode_stage(&data)?;
    require(
        header.binding.kind == kind
            && header.binding.context == context
            && header.binding.digest == digest,
        ClutchError::EvidenceBufferMismatch,
    )?;
    require_stage_pda(program_id, &accounts[IX_STAGE], header)?;
    require(
        accounts[IX_ABORT_REFUND].key.to_bytes() == header.funder,
        ClutchError::ArtifactRefundMismatch,
    )?;
    require(
        current_slot > header.expires_slot
            || accounts[IX_ABORT_CALLER].key.to_bytes() == header.funder,
        ClutchError::UnauthorizedActor,
    )?;
    drop(data);
    close_stage(&accounts[IX_STAGE], &accounts[IX_ABORT_REFUND])
}

/// Route one decoded artifact request.
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        Action::Layout(Intent::BeginArtifact {
            kind,
            context,
            digest,
            exact_len,
            expires_slot,
        }) => begin(
            program_id,
            accounts,
            request.sequence,
            ArtifactBinding {
                kind,
                context,
                digest,
                exact_len,
            },
            expires_slot,
        ),
        Action::Layout(Intent::WriteArtifact {
            kind,
            context,
            digest,
            cursor,
            chunk_len,
            chunk,
        }) => write(
            program_id,
            accounts,
            request.sequence,
            ArtifactBinding {
                kind,
                context,
                digest,
                exact_len: kind.exact_len() as u16,
            },
            cursor,
            chunk_len,
            &chunk,
        ),
        Action::Layout(Intent::SealArtifact {
            kind,
            context,
            digest,
            exact_len,
        }) => seal(
            program_id,
            accounts,
            request.sequence,
            ArtifactBinding {
                kind,
                context,
                digest,
                exact_len,
            },
        ),
        Action::Layout(Intent::AbortArtifact {
            kind,
            context,
            digest,
        }) => abort(
            program_id,
            accounts,
            request.sequence,
            kind,
            context,
            digest,
        ),
        _ => Err(ClutchError::UnsupportedInstruction.into()),
    }
}
