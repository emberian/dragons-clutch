#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for the composed Claims sparse-transfer chain.
//!
//! The program owns no protocol state and no production ABI. It exists to drive
//! the one composition `crates/dclutch-claims-svm/src/composition_v3.rs`
//! describes and no live route can reach yet: Admit the destination Position,
//! carry its exact typed receipt into a SparseNativeTransfer, carry THAT
//! receipt into the Close of the drained source Position, all inside one
//! transaction. It signs the three release-scoped caller-authority PDAs, and it
//! can deliberately refuse afterward so ProgramTest can prove that a completed
//! three-route chain rolls back whole.
//!
//! The receipts are never reconstructed here: each is taken verbatim from the
//! child's return data and appended to the next request, which is exactly the
//! dependency the composition requires.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::{
    CallerRole,
    frame_spec_v1::{ClaimsFrameSpecV1, SparseNativeTransferFrameSpecV1},
    protocol_position_v2::{
        PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_REQUEST_BYTES_V2,
        ProtocolPositionActionV2, ProtocolPositionPresenceV2, ProtocolPositionRequestLayoutV2,
    },
    sparse_native_transfer_v1::{
        SPARSE_NATIVE_TRANSFER_BYTES_V1, SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1,
        SparseNativeTransferLayoutV1,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Exact Admit frame width this wrapper forwards.
pub const ADMIT_ACCOUNT_COUNT: usize =
    dclutch_claims_svm::frame_spec_v1::PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1 as usize;
/// Exact SparseNativeTransfer frame width this wrapper forwards.
pub const SPARSE_ACCOUNT_COUNT: usize =
    dclutch_claims_svm::frame_spec_v1::SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1 as usize;
/// Exact Close frame width this wrapper forwards.
pub const CLOSE_ACCOUNT_COUNT: usize =
    dclutch_claims_svm::frame_spec_v1::PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V1 as usize;

/// Chain the Close stage after the transfer.
pub const FLAG_WITH_CLOSE: u8 = 0b0000_0001;
/// Canonical offset of the sparse request's caller-role byte.
const SPARSE_CALLER_ROLE_OFFSET: usize = 10;

/// Exact width of the Close stage's rent tail: four little-endian `u64`s.
///
/// The Close request is DERIVED here rather than carried whole. Sending three
/// 320-byte requests inline puts the composed chain at 1,261 bytes, past
/// Solana's 1,232-byte packet maximum, and a chain no validator can accept is
/// not evidence that the chain runs. Everything the Close needs is already on
/// the wire except the SOURCE Position's four rent facts -- the admit request
/// carries the DESTINATION's -- so only those four travel, and the rest is
/// patched from the two requests the composition already binds.
pub const CLOSE_RENT_TAIL_BYTES: usize = 32;

/// Refuse after every stage returned, to prove whole-chain rollback.
pub const FLAG_FAIL_AFTER: u8 = 0b0000_0010;
/// Carry an admission receipt whose Position owner is not the transfer's.
///
/// The adapter treats the receipt suffix as OPTIONAL -- omitting it simply
/// skips the join -- so the hostile case that reaches
/// `validate_sparse_admission_receipt_v3` has to present a receipt that decodes
/// and does not join, not no receipt at all.
pub const FLAG_SUBSTITUTE_ADMISSION_OWNER: u8 = 0b0000_0100;

/// Canonical offset of the admitted Position owner inside the receipt.
const ADMISSION_POSITION_OWNER_OFFSET: usize = 80;

/// Stable test-wrapper refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SparseChainCallerError {
    /// Wrapper bytes did not carry one flag byte and the exact stage requests.
    Instruction = 0,
    /// Claims program or a forwarded stage frame was malformed.
    AccountFrame = 1,
    /// A stage refused, or returned no producer-authenticated receipt.
    ClaimsCpi = 2,
    /// Deliberate refusal after the complete chain returned.
    DeliberateLateFailure = 3,
}

impl From<SparseChainCallerError> for ProgramError {
    fn from(value: SparseChainCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Drive Admit -> SparseNativeTransfer -> optional Close as one chain.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let flags = *instruction_data
        .first()
        .ok_or(SparseChainCallerError::Instruction)?;
    if flags & !(FLAG_WITH_CLOSE | FLAG_FAIL_AFTER | FLAG_SUBSTITUTE_ADMISSION_OWNER) != 0 {
        return Err(SparseChainCallerError::Instruction.into());
    }
    let with_close = flags & FLAG_WITH_CLOSE != 0;
    let body = instruction_data
        .get(1..)
        .ok_or(SparseChainCallerError::Instruction)?;
    let expected_body = PROTOCOL_POSITION_REQUEST_BYTES_V2
        .checked_add(SPARSE_NATIVE_TRANSFER_BYTES_V1)
        .and_then(|held| {
            held.checked_add(if with_close { CLOSE_RENT_TAIL_BYTES } else { 0 })
        })
        .ok_or(SparseChainCallerError::Instruction)?;
    if body.len() != expected_body {
        return Err(SparseChainCallerError::Instruction.into());
    }
    let admit_bytes = body
        .get(..PROTOCOL_POSITION_REQUEST_BYTES_V2)
        .ok_or(SparseChainCallerError::Instruction)?;
    let sparse_end = PROTOCOL_POSITION_REQUEST_BYTES_V2
        .checked_add(SPARSE_NATIVE_TRANSFER_BYTES_V1)
        .ok_or(SparseChainCallerError::Instruction)?;
    let sparse_bytes = body
        .get(PROTOCOL_POSITION_REQUEST_BYTES_V2..sparse_end)
        .ok_or(SparseChainCallerError::Instruction)?;
    let close_bytes = if with_close {
        Some(derive_close_request(
            admit_bytes,
            sparse_bytes,
            body.get(sparse_end..)
                .ok_or(SparseChainCallerError::Instruction)?,
        )?)
    } else {
        None
    };

    let claims_program = accounts
        .first()
        .ok_or(SparseChainCallerError::AccountFrame)?;
    if !claims_program.executable || claims_program.is_signer || claims_program.is_writable {
        return Err(SparseChainCallerError::AccountFrame.into());
    }
    let forwarded = accounts
        .get(1..)
        .ok_or(SparseChainCallerError::AccountFrame)?;
    let expected_accounts = ADMIT_ACCOUNT_COUNT
        .checked_add(SPARSE_ACCOUNT_COUNT)
        .and_then(|held| held.checked_add(if with_close { CLOSE_ACCOUNT_COUNT } else { 0 }))
        .ok_or(SparseChainCallerError::AccountFrame)?;
    if forwarded.len() != expected_accounts {
        return Err(SparseChainCallerError::AccountFrame.into());
    }
    let admit_frame = forwarded
        .get(..ADMIT_ACCOUNT_COUNT)
        .ok_or(SparseChainCallerError::AccountFrame)?;
    let sparse_end_account = ADMIT_ACCOUNT_COUNT
        .checked_add(SPARSE_ACCOUNT_COUNT)
        .ok_or(SparseChainCallerError::AccountFrame)?;
    let sparse_frame = forwarded
        .get(ADMIT_ACCOUNT_COUNT..sparse_end_account)
        .ok_or(SparseChainCallerError::AccountFrame)?;
    let close_frame = forwarded
        .get(sparse_end_account..)
        .ok_or(SparseChainCallerError::AccountFrame)?;

    let admission = stage(
        program_id,
        claims_program,
        accounts,
        admit_frame,
        Stage::Admit,
        admit_bytes,
        position_seeds(admit_bytes)?,
    )?;
    if admission.len() != PROTOCOL_POSITION_ADMISSION_BYTES_V2 {
        return Err(SparseChainCallerError::ClaimsCpi.into());
    }

    let mut transfer_data = Vec::with_capacity(
        SPARSE_NATIVE_TRANSFER_BYTES_V1
            .checked_add(PROTOCOL_POSITION_ADMISSION_BYTES_V2)
            .ok_or(SparseChainCallerError::Instruction)?,
    );
    transfer_data.extend_from_slice(sparse_bytes);
    transfer_data.extend_from_slice(&admission);
    if flags & FLAG_SUBSTITUTE_ADMISSION_OWNER != 0 {
        let offset = SPARSE_NATIVE_TRANSFER_BYTES_V1
            .checked_add(ADMISSION_POSITION_OWNER_OFFSET)
            .ok_or(SparseChainCallerError::Instruction)?;
        let byte = transfer_data
            .get_mut(offset)
            .ok_or(SparseChainCallerError::Instruction)?;
        *byte ^= 0xff;
    }
    let transfer_receipt = stage(
        program_id,
        claims_program,
        accounts,
        sparse_frame,
        Stage::Transfer,
        &transfer_data,
        sparse_seeds(sparse_bytes)?,
    )?;
    if transfer_receipt.len() != SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1 {
        return Err(SparseChainCallerError::ClaimsCpi.into());
    }

    if let Some(ref close_bytes) = close_bytes {
        let mut close_data = Vec::with_capacity(
            PROTOCOL_POSITION_REQUEST_BYTES_V2
                .checked_add(SPARSE_NATIVE_TRANSFER_RECEIPT_BYTES_V1)
                .ok_or(SparseChainCallerError::Instruction)?,
        );
        close_data.extend_from_slice(close_bytes);
        close_data.extend_from_slice(&transfer_receipt);
        stage(
            program_id,
            claims_program,
            accounts,
            close_frame,
            Stage::Close,
            &close_data,
            position_seeds(close_bytes)?,
        )?;
    }

    if flags & FLAG_FAIL_AFTER != 0 {
        return Err(SparseChainCallerError::DeliberateLateFailure.into());
    }
    Ok(())
}

/// Which canonical Claims frame one stage carries.
#[derive(Clone, Copy)]
enum Stage {
    /// ProtocolPosition Admit, 26 accounts.
    Admit,
    /// SparseNativeTransfer, 22 accounts.
    Transfer,
    /// ProtocolPosition Close, 15 accounts.
    Close,
}

impl Stage {
    /// Exact writability the canonical frame gives one coordinate.
    fn writable(self, index: u16) -> Result<bool, ProgramError> {
        let privileges = match self {
            Self::Admit => ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Admit)
                .account(index)
                .map_err(|_| SparseChainCallerError::AccountFrame)?
                .privileges(),
            Self::Close => ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Close)
                .account(index)
                .map_err(|_| SparseChainCallerError::AccountFrame)?
                .privileges(),
            Self::Transfer => SparseNativeTransferFrameSpecV1
                .account(index)
                .map_err(|_| SparseChainCallerError::AccountFrame)?
                .privileges(),
        };
        Ok(privileges.writable())
    }
}

/// One signed child invocation, returning the child's verbatim receipt.
///
/// The stages of this chain disagree about writability: the Claims aggregate is
/// readonly to Admit and writable to the transfer, and the lifecycle RentCredit
/// is readonly to Admit and writable to Close. A transaction carries ONE
/// writability bit per address, so the message necessarily grants the union and
/// each stage must be handed the exact frame it authenticates. Copying the
/// observed bit into every CPI would make the whole composition unreachable in
/// one transaction, which is what the frame spec's per-coordinate privileges
/// are for.
fn stage(
    program_id: &Pubkey,
    claims_program: &AccountInfo<'_>,
    all: &[AccountInfo<'_>],
    frame: &[AccountInfo<'_>],
    kind: Stage,
    data: &[u8],
    seeds: CallerAuthoritySeedsV1,
) -> Result<Vec<u8>, ProgramError> {
    let bump = [Pubkey::find_program_address(&seeds.as_slices(), program_id).1];
    let mut metas = Vec::with_capacity(frame.len());
    for (index, account) in frame.iter().enumerate() {
        let coordinate =
            u16::try_from(index).map_err(|_| SparseChainCallerError::AccountFrame)?;
        // Coordinate zero is the release-scoped caller-authority PDA this
        // program signs for. Nothing else is ever signed, and no coordinate is
        // ever granted more than the transaction already carries.
        let signer = index == 0;
        let writable = kind.writable(coordinate)? && account.is_writable;
        metas.push(if writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: data.to_vec(),
    };
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    invoke_signed(
        &instruction,
        all,
        &[&[domain, release, market, role, context, digest, &bump]],
    )
    .map_err(|_| SparseChainCallerError::ClaimsCpi)?;
    let (producer, receipt) = get_return_data().ok_or(SparseChainCallerError::ClaimsCpi)?;
    if producer != *claims_program.key || receipt.is_empty() {
        return Err(SparseChainCallerError::ClaimsCpi.into());
    }
    Ok(receipt)
}

/// Build the Close request the composition's close join requires.
///
/// Every field but the source Position's four rent facts is already bound by
/// the admit and transfer requests: `require_sparse_close_join` demands the
/// same release set, Market, generation and parent request, the transfer's
/// SOURCE owner, and both post-revisions. Patching the admit request rather
/// than encoding a fresh one keeps this wrapper free of the request codec, so
/// hostile bytes still reach the adapter that owes the refusal.
fn derive_close_request(
    admit_bytes: &[u8],
    sparse_bytes: &[u8],
    rent_tail: &[u8],
) -> Result<Vec<u8>, ProgramError> {
    if rent_tail.len() != CLOSE_RENT_TAIL_BYTES {
        return Err(SparseChainCallerError::Instruction.into());
    }
    let mut close = admit_bytes.to_vec();
    put(&mut close, ProtocolPositionRequestLayoutV2::ACTION, &[
        ProtocolPositionActionV2::Close as u8,
    ])?;
    put(&mut close, ProtocolPositionRequestLayoutV2::PRESENCE, &[
        ProtocolPositionPresenceV2::Existing as u8,
    ])?;
    let source_owner: [u8; 32] = array(sparse_bytes, SparseNativeTransferLayoutV1::SOURCE_OWNER)?;
    let request_id: [u8; 32] = array(sparse_bytes, SparseNativeTransferLayoutV1::REQUEST_ID)?;
    put(
        &mut close,
        ProtocolPositionRequestLayoutV2::POSITION_OWNER,
        &source_owner,
    )?;
    put(
        &mut close,
        ProtocolPositionRequestLayoutV2::PARENT_REQUEST_DIGEST,
        &request_id,
    )?;
    // The transfer advances both revisions exactly once, and the close must
    // name the post-revisions its receipt will carry.
    for (source, target) in [
        (
            SparseNativeTransferLayoutV1::MARKET_REVISION,
            ProtocolPositionRequestLayoutV2::EXPECTED_MARKET_REVISION,
        ),
        (
            SparseNativeTransferLayoutV1::SOURCE_REVISION,
            ProtocolPositionRequestLayoutV2::EXPECTED_POSITION_REVISION,
        ),
    ] {
        let value = revision(sparse_bytes, source)?
            .checked_add(1)
            .ok_or(SparseChainCallerError::Instruction)?;
        put(&mut close, target, &value.to_le_bytes())?;
    }
    for (index, target) in [
        ProtocolPositionRequestLayoutV2::OBSERVED_POSITION_LAMPORTS,
        ProtocolPositionRequestLayoutV2::OBSERVED_ADMISSION_LAMPORTS,
        ProtocolPositionRequestLayoutV2::POSITION_RENT_PRINCIPAL,
        ProtocolPositionRequestLayoutV2::ADMISSION_RENT_PRINCIPAL,
    ]
    .into_iter()
    .enumerate()
    {
        let start = index
            .checked_mul(8)
            .ok_or(SparseChainCallerError::Instruction)?;
        let end = start
            .checked_add(8)
            .ok_or(SparseChainCallerError::Instruction)?;
        let value = rent_tail
            .get(start..end)
            .ok_or(SparseChainCallerError::Instruction)?;
        put(&mut close, target, value)?;
    }
    Ok(close)
}

fn put(target: &mut [u8], offset: usize, value: &[u8]) -> Result<(), ProgramError> {
    let end = offset
        .checked_add(value.len())
        .ok_or(SparseChainCallerError::Instruction)?;
    target
        .get_mut(offset..end)
        .ok_or(SparseChainCallerError::Instruction)?
        .copy_from_slice(value);
    Ok(())
}

fn revision(input: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let end = offset
        .checked_add(8)
        .ok_or(SparseChainCallerError::Instruction)?;
    let bytes: [u8; 8] = input
        .get(offset..end)
        .ok_or(SparseChainCallerError::Instruction)?
        .try_into()
        .map_err(|_| SparseChainCallerError::Instruction)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Caller-authority seeds for one ProtocolPosition stage.
///
/// The fields are read at their canonical offsets rather than decoded. A
/// test-only wrapper that re-validated the request would refuse hostile bytes
/// itself and hide the refusal the ADAPTER owes: a campaign could never see
/// which Claims code rejects a malformed request.
fn position_seeds(request_bytes: &[u8]) -> Result<CallerAuthoritySeedsV1, ProgramError> {
    seeds(
        request_bytes,
        ExecutionRoleV1::Trading,
        ProtocolPositionRequestLayoutV2::RELEASE_SET,
        ProtocolPositionRequestLayoutV2::MARKET,
        ProtocolPositionRequestLayoutV2::POSITION_OWNER,
    )
}

/// Caller-authority seeds for the transfer stage.
fn sparse_seeds(request_bytes: &[u8]) -> Result<CallerAuthoritySeedsV1, ProgramError> {
    let role = match *request_bytes
        .get(SPARSE_CALLER_ROLE_OFFSET)
        .ok_or(SparseChainCallerError::Instruction)?
    {
        role if role == CallerRole::Core as u8 => ExecutionRoleV1::Core,
        role if role == CallerRole::Trading as u8 => ExecutionRoleV1::Trading,
        _ => return Err(SparseChainCallerError::Instruction.into()),
    };
    seeds(
        request_bytes,
        role,
        SparseNativeTransferLayoutV1::RELEASE_SET,
        SparseNativeTransferLayoutV1::MARKET,
        SparseNativeTransferLayoutV1::REQUEST_ID,
    )
}

fn seeds(
    request_bytes: &[u8],
    role: ExecutionRoleV1,
    release_offset: usize,
    market_offset: usize,
    context_offset: usize,
) -> Result<CallerAuthoritySeedsV1, ProgramError> {
    CallerAuthoritySeedsV1::new(
        ContentId::new(array(request_bytes, release_offset)?)
            .map_err(|_| SparseChainCallerError::Instruction)?,
        array(request_bytes, market_offset)?,
        role,
        array(request_bytes, context_offset)?,
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| SparseChainCallerError::Instruction.into())
}

fn array(input: &[u8], offset: usize) -> Result<[u8; 32], ProgramError> {
    let end = offset
        .checked_add(32)
        .ok_or(SparseChainCallerError::Instruction)?;
    input
        .get(offset..end)
        .ok_or(SparseChainCallerError::Instruction)?
        .try_into()
        .map_err(|_| SparseChainCallerError::Instruction.into())
}
