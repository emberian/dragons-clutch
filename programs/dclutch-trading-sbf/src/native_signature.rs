//! Family-neutral Instructions-sysvar adapter for native-signature evidence.
//!
//! The sysvar is read in place. Its serialized form is a byte-offset table over
//! self-describing instruction records, so every fact this adapter authenticates
//! — program identity, instruction data, and the account metas with their
//! privileges — can be compared against the borrowed sysvar account data
//! directly. Materialising an owned `Instruction` instead cost a
//! `Vec<AccountMeta>` per read (2,856 bytes for the canonical 84-account Direct
//! frame) plus a verbatim copy of instruction data that was already in the
//! caller's hands, all of it charged for the whole instruction against a
//! 32,768-byte heap whose allocator never frees.
//!
//! Borrowing is sound here for a reason narrower than "the sysvar does not
//! change": every accessor below reads the same borrowed slice the check
//! consumed, under one `RefCell` guard held across the comparison, and no
//! accessor hands out a reference that outlives that guard. The runtime writes
//! the trailing current-index field between top-level instructions and never
//! during one, and this adapter performs no CPI while a guard is live.

use core::cell::Ref;

use dclutch_request_profile_contract::v2::{
    NativeEd25519InstructionViewV1, NativeSignatureRegistersV1, RequestProfileV2,
    seed_authenticated_signers_atomic,
};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use solana_sdk_ids::{ed25519_program, sysvar};

use crate::TradingSbfError;

/// Width of the sysvar's leading instruction count and of one offset entry.
const SYSVAR_U16_BYTES: usize = 2;
/// One serialized account meta: a privilege byte followed by a 32-byte address.
const SYSVAR_META_BYTES: usize = 33;
/// Width of a serialized address.
const SYSVAR_PUBKEY_BYTES: usize = 32;
/// Privilege bit 0 marks a signer.
const SYSVAR_META_SIGNER_BIT: u8 = 1;
/// Privilege bit 1 marks a writable account.
const SYSVAR_META_WRITABLE_BIT: u8 = 1 << 1;

fn read_u16(data: &[u8], at: usize) -> Result<u16, ProgramError> {
    let end = at
        .checked_add(SYSVAR_U16_BYTES)
        .ok_or(TradingSbfError::NativeSignature)?;
    let bytes = data.get(at..end).ok_or(TradingSbfError::NativeSignature)?;
    let value =
        <[u8; SYSVAR_U16_BYTES]>::try_from(bytes).map_err(|_| TradingSbfError::NativeSignature)?;
    Ok(u16::from_le_bytes(value))
}

fn read_pubkey(data: &[u8], at: usize) -> Result<&[u8; SYSVAR_PUBKEY_BYTES], ProgramError> {
    let end = at
        .checked_add(SYSVAR_PUBKEY_BYTES)
        .ok_or(TradingSbfError::NativeSignature)?;
    let bytes = data.get(at..end).ok_or(TradingSbfError::NativeSignature)?;
    <&[u8; SYSVAR_PUBKEY_BYTES]>::try_from(bytes)
        .map_err(|_| TradingSbfError::NativeSignature.into())
}

/// One borrowed account meta from the Instructions sysvar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SysvarAccountMetaV1<'a> {
    /// Exact account address.
    pub(crate) pubkey: &'a [u8; SYSVAR_PUBKEY_BYTES],
    /// Whether the transaction presented this account as a signer.
    pub(crate) is_signer: bool,
    /// Whether the transaction presented this account as writable.
    pub(crate) is_writable: bool,
}

/// A borrowed run of serialized account metas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SysvarAccountMetasV1<'a> {
    bytes: &'a [u8],
}

impl<'a> SysvarAccountMetasV1<'a> {
    /// Exact meta count in this run.
    pub(crate) const fn len(self) -> usize {
        self.bytes.len() / SYSVAR_META_BYTES
    }

    /// The meta at one position in this run.
    pub(crate) fn get(self, index: usize) -> Option<SysvarAccountMetaV1<'a>> {
        let start = index.checked_mul(SYSVAR_META_BYTES)?;
        let end = start.checked_add(SYSVAR_META_BYTES)?;
        Self::decode(self.bytes.get(start..end)?)
    }

    /// Walk the run in order.
    pub(crate) fn iter(self) -> impl Iterator<Item = SysvarAccountMetaV1<'a>> {
        self.bytes
            .chunks_exact(SYSVAR_META_BYTES)
            .filter_map(Self::decode)
    }

    fn decode(bytes: &'a [u8]) -> Option<SysvarAccountMetaV1<'a>> {
        let privileges = *bytes.first()?;
        let pubkey =
            <&[u8; SYSVAR_PUBKEY_BYTES]>::try_from(bytes.get(1..SYSVAR_META_BYTES)?).ok()?;
        Some(SysvarAccountMetaV1 {
            pubkey,
            is_signer: privileges & SYSVAR_META_SIGNER_BIT != 0,
            is_writable: privileges & SYSVAR_META_WRITABLE_BIT != 0,
        })
    }
}

/// A borrowed view of one instruction inside the Instructions sysvar.
///
/// Every field points into the sysvar account data borrowed by the caller, so
/// the view cannot outlive that borrow and no accessor copies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SysvarInstructionV1<'a> {
    metas: SysvarAccountMetasV1<'a>,
    program_id: &'a [u8; SYSVAR_PUBKEY_BYTES],
    data: &'a [u8],
}

impl<'a> SysvarInstructionV1<'a> {
    /// Read the instruction at `index` from borrowed sysvar account data.
    ///
    /// Every offset is bounds-checked against the same slice and every width is
    /// checked arithmetic, so a truncated, overlong, or self-referential sysvar
    /// is a refusal rather than a panic or a short read.
    pub(crate) fn read(index: u16, sysvar: &'a [u8]) -> Result<Self, ProgramError> {
        if index >= read_u16(sysvar, 0)? {
            return Err(TradingSbfError::NativeSignature.into());
        }
        let offset_at = SYSVAR_U16_BYTES
            .checked_add(
                usize::from(index)
                    .checked_mul(SYSVAR_U16_BYTES)
                    .ok_or(TradingSbfError::NativeSignature)?,
            )
            .ok_or(TradingSbfError::NativeSignature)?;
        let mut cursor = usize::from(read_u16(sysvar, offset_at)?);
        let account_count = usize::from(read_u16(sysvar, cursor)?);
        cursor = cursor
            .checked_add(SYSVAR_U16_BYTES)
            .ok_or(TradingSbfError::NativeSignature)?;
        let meta_bytes = account_count
            .checked_mul(SYSVAR_META_BYTES)
            .ok_or(TradingSbfError::NativeSignature)?;
        let meta_end = cursor
            .checked_add(meta_bytes)
            .ok_or(TradingSbfError::NativeSignature)?;
        let metas = SysvarAccountMetasV1 {
            bytes: sysvar
                .get(cursor..meta_end)
                .ok_or(TradingSbfError::NativeSignature)?,
        };
        let program_id = read_pubkey(sysvar, meta_end)?;
        cursor = meta_end
            .checked_add(SYSVAR_PUBKEY_BYTES)
            .ok_or(TradingSbfError::NativeSignature)?;
        let data_len = usize::from(read_u16(sysvar, cursor)?);
        cursor = cursor
            .checked_add(SYSVAR_U16_BYTES)
            .ok_or(TradingSbfError::NativeSignature)?;
        let data_end = cursor
            .checked_add(data_len)
            .ok_or(TradingSbfError::NativeSignature)?;
        let data = sysvar
            .get(cursor..data_end)
            .ok_or(TradingSbfError::NativeSignature)?;
        Ok(Self {
            metas,
            program_id,
            data,
        })
    }

    /// Exact executing program identity.
    pub(crate) const fn program_id(self) -> &'a [u8; SYSVAR_PUBKEY_BYTES] {
        self.program_id
    }

    /// Exact instruction data bytes.
    pub(crate) const fn data(self) -> &'a [u8] {
        self.data
    }

    /// Exact account count presented by this instruction.
    pub(crate) const fn account_count(self) -> usize {
        self.metas.len()
    }

    /// Every account meta, in transaction order.
    pub(crate) const fn metas(self) -> SysvarAccountMetasV1<'a> {
        self.metas
    }

    /// Every meta from `start` onward, refusing a start this instruction does
    /// not reach.
    pub(crate) fn metas_from(self, start: usize) -> Result<SysvarAccountMetasV1<'a>, ProgramError> {
        let from = start
            .checked_mul(SYSVAR_META_BYTES)
            .ok_or(TradingSbfError::NativeSignature)?;
        Ok(SysvarAccountMetasV1 {
            bytes: self
                .metas
                .bytes
                .get(from..)
                .ok_or(TradingSbfError::NativeSignature)?,
        })
    }

    /// The `len` metas starting at `start`, refusing a run this instruction
    /// does not wholly cover.
    pub(crate) fn metas_range(
        self,
        start: usize,
        len: usize,
    ) -> Result<SysvarAccountMetasV1<'a>, ProgramError> {
        let from = start
            .checked_mul(SYSVAR_META_BYTES)
            .ok_or(TradingSbfError::NativeSignature)?;
        let to = len
            .checked_mul(SYSVAR_META_BYTES)
            .and_then(|width| from.checked_add(width))
            .ok_or(TradingSbfError::NativeSignature)?;
        Ok(SysvarAccountMetasV1 {
            bytes: self
                .metas
                .bytes
                .get(from..to)
                .ok_or(TradingSbfError::NativeSignature)?,
        })
    }
}

/// Borrow the canonical Instructions sysvar and read its current index.
///
/// The account shape is authenticated before a single byte is parsed: the
/// canonical sysvar address, and none of signer, writable, or executable. The
/// returned guard keeps the account data borrowed for as long as any view read
/// from it is alive, which is what makes those views observe exactly the bytes
/// this check consumed.
pub(crate) fn borrow_authenticated_instructions_v1<'a>(
    instructions: &'a AccountInfo<'_>,
) -> Result<(u16, Ref<'a, &'a mut [u8]>), ProgramError> {
    if instructions.key != &sysvar::instructions::ID
        || instructions.is_signer
        || instructions.is_writable
        || instructions.executable
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let data = instructions
        .try_borrow_data()
        .map_err(|_| TradingSbfError::NativeSignature)?;
    let end = data
        .len()
        .checked_sub(SYSVAR_U16_BYTES)
        .ok_or(TradingSbfError::NativeSignature)?;
    let current = read_u16(&data, end)?;
    Ok((current, data))
}

/// Authenticate the exact current top-level Trading instruction and account metas.
///
/// The common hot outer calls this for every action, including unsigned ones,
/// so the fixed Instructions-sysvar account is an observed authority rather
/// than a dummy slot. The returned nonsemantic instruction index may only be
/// used to inspect native evidence immediately preceding this instruction.
pub fn authenticate_current_top_level_instruction(
    program_id: &Pubkey,
    current_accounts: &[AccountInfo<'_>],
    current_instruction_data: &[u8],
    instructions: &AccountInfo<'_>,
) -> Result<u16, ProgramError> {
    let (current, sysvar) = borrow_authenticated_instructions_v1(instructions)?;
    let observed = SysvarInstructionV1::read(current, &sysvar)?;
    if observed.program_id() != program_id.as_array()
        || observed.data() != current_instruction_data
        || observed.account_count() != current_accounts.len()
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    for (meta, account) in observed.metas().iter().zip(current_accounts) {
        if meta.pubkey != account.key.as_array()
            || meta.is_signer != account.is_signer
            || meta.is_writable != account.is_writable
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
    }
    Ok(current)
}

/// Authenticate one exact top-level Trading instruction and its immediately
/// preceding canonical native-Ed25519 batch, then seed signer identities.
///
/// The selected RequestProfile V2 owns every absolute message slice and
/// destination register.  This adapter introduces no family discriminator.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_and_seed_native_signatures(
    program_id: &Pubkey,
    current_accounts: &[AccountInfo<'_>],
    current_instruction_data: &[u8],
    instructions: &AccountInfo<'_>,
    profile: RequestProfileV2<'_>,
    tail_count: u32,
    registers: NativeSignatureRegistersV1<'_>,
) -> Result<(), ProgramError> {
    let current = authenticate_current_top_level_instruction(
        program_id,
        current_accounts,
        current_instruction_data,
        instructions,
    )?;
    seed_native_signatures_at_authenticated_instruction(
        current,
        current_instruction_data,
        0,
        instructions,
        profile,
        tail_count,
        registers,
    )
}

/// Seed native-signature identities after the enclosing adapter has already
/// authenticated the exact current top-level invocation.
///
/// Registry continuation mode needs this narrower seam because the current
/// top-level program is Registry while the request-profile message coordinates
/// remain relative to the exact nested Trading Hot bytes. The caller owns
/// authentication of `current`, the byte-exact nested bytes, and the canonical
/// top-level offset at which those bytes begin.
///
/// The ed25519 batch is read in place and the sysvar borrow is held across
/// seeding, which is exactly the window in which the seeded identities are
/// derived from it. `seed_authenticated_signers_atomic` is a pure kernel over
/// the bytes and the authenticated profile: it performs no CPI and touches no
/// account, so nothing can write the sysvar between the adjacency check and the
/// signatures those bytes authorize.
pub(crate) fn seed_native_signatures_at_authenticated_instruction(
    current: u16,
    authenticated_message_data: &[u8],
    message_offset_bias: u16,
    instructions: &AccountInfo<'_>,
    profile: RequestProfileV2<'_>,
    tail_count: u32,
    registers: NativeSignatureRegistersV1<'_>,
) -> Result<(), ProgramError> {
    let preceding_index = current
        .checked_sub(1)
        .ok_or(TradingSbfError::NativeSignature)?;
    let (_, sysvar) = borrow_authenticated_instructions_v1(instructions)?;
    let preceding = SysvarInstructionV1::read(preceding_index, &sysvar)?;
    if preceding.program_id() != ed25519_program::ID.as_array() || preceding.account_count() != 0 {
        return Err(TradingSbfError::NativeSignature.into());
    }
    seed_authenticated_signers_atomic(
        profile,
        tail_count,
        NativeEd25519InstructionViewV1 {
            ed25519_data: preceding.data(),
            authenticated_message_data,
            message_instruction_index: current,
            message_offset_bias,
        },
        registers,
    )
    .map_err(|_| TradingSbfError::NativeSignature.into())
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::{vec, vec::Vec};

    use dclutch_request_profile_contract::v2::{
        ED25519_PUBLIC_KEY_BYTES, ED25519_SELF_INSTRUCTION_INDEX, ED25519_SIGNATURE_BYTES,
        ED25519_SIGNATURE_OFFSETS_BYTES, ED25519_SIGNATURE_OFFSETS_START,
        NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1, REQUEST_PROFILE_V2_ARTIFACT_PROFILE,
        REQUEST_PROFILE_V2_HEADER_BYTES, REQUEST_PROFILE_V2_MAGIC,
        REQUEST_PROFILE_V2_SCHEMA_VERSION,
    };
    use solana_instructions_sysvar::construct_instructions_data;
    use solana_program::{
        account_info::AccountInfo,
        sysvar::instructions::{BorrowedAccountMeta, BorrowedInstruction},
    };

    use super::*;

    const CURRENT_PROGRAM: Pubkey = Pubkey::new_from_array([91; 32]);
    const CURRENT_ACCOUNT: Pubkey = Pubkey::new_from_array([92; 32]);
    const SYSVAR_OWNER: Pubkey = Pubkey::new_from_array([93; 32]);

    fn request_profile_v1() -> Vec<u8> {
        let mut bytes = vec![0_u8; 56];
        bytes
            .get_mut(..8)
            .expect("magic")
            .copy_from_slice(&dclutch_request_profile_contract::MAGIC);
        bytes
            .get_mut(8..10)
            .expect("version")
            .copy_from_slice(&dclutch_request_profile_contract::VERSION.to_le_bytes());
        bytes
            .get_mut(10..12)
            .expect("artifact profile")
            .copy_from_slice(&dclutch_request_profile_contract::ARTIFACT_PROFILE.to_le_bytes());
        bytes
            .get_mut(12..16)
            .expect("request width")
            .copy_from_slice(&8_u32.to_le_bytes());
        bytes
            .get_mut(20..22)
            .expect("operation count")
            .copy_from_slice(&1_u16.to_le_bytes());
        bytes
            .get_mut(28..30)
            .expect("identity count")
            .copy_from_slice(&1_u16.to_le_bytes());
        // One fixed require-zero-range operation covering the eight request bytes.
        *bytes.get_mut(32).expect("opcode") = 4;
        bytes
            .get_mut(44..52)
            .expect("zero-range width")
            .copy_from_slice(&8_u64.to_le_bytes());
        bytes
    }

    fn profile_bytes(message_offset: u16, message_bytes: u16) -> Vec<u8> {
        let embedded = request_profile_v1();
        let mut bytes = Vec::with_capacity(
            REQUEST_PROFILE_V2_HEADER_BYTES
                + embedded.len()
                + NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1,
        );
        bytes.extend_from_slice(&REQUEST_PROFILE_V2_MAGIC);
        bytes.extend_from_slice(&REQUEST_PROFILE_V2_SCHEMA_VERSION.to_le_bytes());
        bytes.extend_from_slice(&REQUEST_PROFILE_V2_ARTIFACT_PROFILE.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(embedded.len())
                .expect("fixture width")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&embedded);
        bytes.extend_from_slice(&message_offset.to_le_bytes());
        bytes.extend_from_slice(&message_bytes.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes
    }

    fn ed25519_data(
        message_offset: u16,
        message_bytes: u16,
        message_instruction_index: u16,
        signer: [u8; 32],
    ) -> Vec<u8> {
        let public_key = ED25519_SIGNATURE_OFFSETS_START + ED25519_SIGNATURE_OFFSETS_BYTES;
        let signature = public_key + ED25519_PUBLIC_KEY_BYTES;
        let mut bytes = vec![1_u8, 0];
        for value in [
            u16::try_from(signature).expect("offset"),
            ED25519_SELF_INSTRUCTION_INDEX,
            u16::try_from(public_key).expect("offset"),
            ED25519_SELF_INSTRUCTION_INDEX,
            message_offset,
            message_bytes,
            message_instruction_index,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&signer);
        bytes.extend_from_slice(&[0x55; ED25519_SIGNATURE_BYTES]);
        bytes
    }

    fn sysvar_account<'a>(
        preceding_program: &'a Pubkey,
        preceding_data: &'a [u8],
        current_data: &'a [u8],
        account_key: &'a Pubkey,
        lamports: &'a mut u64,
        data: &'a mut Vec<u8>,
    ) -> AccountInfo<'a> {
        let current_accounts = vec![BorrowedAccountMeta {
            pubkey: account_key,
            is_signer: false,
            is_writable: false,
        }];
        let borrowed = [
            BorrowedInstruction {
                program_id: preceding_program,
                accounts: Vec::new(),
                data: preceding_data,
            },
            BorrowedInstruction {
                program_id: &CURRENT_PROGRAM,
                accounts: current_accounts,
                data: current_data,
            },
        ];
        *data = construct_instructions_data(&borrowed);
        let end = data.len();
        data.get_mut(end - 2..)
            .expect("current index")
            .copy_from_slice(&1_u16.to_le_bytes());
        AccountInfo::new(
            &sysvar::instructions::ID,
            false,
            false,
            lamports,
            data.as_mut_slice(),
            &SYSVAR_OWNER,
            false,
        )
    }

    /// The same two-instruction sysvar `sysvar_account` builds, as raw bytes a
    /// test may corrupt before an `AccountInfo` is wrapped around them.
    fn sysvar_bytes(
        preceding_program: &Pubkey,
        preceding_data: &[u8],
        current_data: &[u8],
        account_key: &Pubkey,
    ) -> Vec<u8> {
        let current_accounts = vec![BorrowedAccountMeta {
            pubkey: account_key,
            is_signer: false,
            is_writable: false,
        }];
        let borrowed = [
            BorrowedInstruction {
                program_id: preceding_program,
                accounts: Vec::new(),
                data: preceding_data,
            },
            BorrowedInstruction {
                program_id: &CURRENT_PROGRAM,
                accounts: current_accounts,
                data: current_data,
            },
        ];
        let mut data = construct_instructions_data(&borrowed);
        let end = data.len();
        data.get_mut(end - 2..)
            .expect("current index")
            .copy_from_slice(&1_u16.to_le_bytes());
        data
    }

    fn sysvar_info<'a>(bytes: &'a mut [u8], lamports: &'a mut u64) -> AccountInfo<'a> {
        AccountInfo::new(
            &sysvar::instructions::ID,
            false,
            false,
            lamports,
            bytes,
            &SYSVAR_OWNER,
            false,
        )
    }

    /// Rewrite the trailing current-index field.
    fn set_current_index(bytes: &mut [u8], index: u16) {
        let end = bytes.len() - 2;
        bytes
            .get_mut(end..)
            .expect("current index")
            .copy_from_slice(&index.to_le_bytes());
    }

    /// Rewrite entry `instruction` of the leading byte-offset table.
    fn set_offset_entry(bytes: &mut [u8], instruction: usize, offset: u16) {
        let at = 2 + instruction * 2;
        bytes
            .get_mut(at..at + 2)
            .expect("offset entry")
            .copy_from_slice(&offset.to_le_bytes());
    }

    fn offset_entry(bytes: &[u8], instruction: usize) -> usize {
        let at = 2 + instruction * 2;
        let entry: [u8; 2] = bytes
            .get(at..at + 2)
            .expect("offset entry")
            .try_into()
            .expect("offset entry");
        usize::from(u16::from_le_bytes(entry))
    }

    /// Drive the whole continuation-admission read: authenticate the current
    /// top-level record against the presented invocation, then read the
    /// preceding ed25519 batch and seed from it.
    ///
    /// The corpus below drives this rather than
    /// `seed_native_signatures_at_authenticated_instruction`, because that
    /// narrower seam authenticates only the preceding record by contract — its
    /// caller owns `current`. Corrupting the current record proves nothing
    /// against it, and would silently pass.
    fn admit_over(
        instructions: &AccountInfo<'_>,
        profile: RequestProfileV2<'_>,
        current_data: &[u8],
    ) -> bool {
        let mut current_lamports = 1;
        let mut current_account_data = [];
        let owner = Pubkey::new_from_array([93; 32]);
        let current_account = AccountInfo::new(
            &CURRENT_ACCOUNT,
            false,
            false,
            &mut current_lamports,
            &mut current_account_data,
            &owner,
            false,
        );
        let input = [[0_u8; 32]];
        let mut scratch = [[9_u8; 32]];
        let mut output = [[8_u8; 32]];
        authenticate_and_seed_native_signatures(
            &CURRENT_PROGRAM,
            &[current_account],
            current_data,
            instructions,
            profile,
            0,
            NativeSignatureRegistersV1 {
                input_identities: &input,
                scratch_identities: &mut scratch,
                output_identities: &mut output,
            },
        )
        .is_ok()
    }

    /// The borrowed view reports exactly what the owning reader materialises.
    ///
    /// This is the whole justification for reading the sysvar in place: for the
    /// realized record shapes, program identity, instruction data and every
    /// account meta with its privileges agree field for field with
    /// `load_instruction_at_checked`, which is the reader this adapter replaced.
    #[test]
    fn borrowed_view_agrees_with_the_owning_sysvar_reader() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        let mut bytes = sysvar_bytes(
            &ed25519_program::ID,
            &native,
            &current_data,
            &CURRENT_ACCOUNT,
        );
        let mut lamports = 1;
        let instructions = sysvar_info(&mut bytes, &mut lamports);
        for index in 0..2_u16 {
            let owned = solana_instructions_sysvar::load_instruction_at_checked(
                usize::from(index),
                &instructions,
            )
            .expect("owning reader");
            let data = instructions.try_borrow_data().expect("borrow");
            let view = SysvarInstructionV1::read(index, &data).expect("borrowed reader");
            assert_eq!(view.program_id(), owned.program_id.as_array());
            assert_eq!(view.data(), owned.data.as_slice());
            assert_eq!(view.account_count(), owned.accounts.len());
            for (meta, expected) in view.metas().iter().zip(&owned.accounts) {
                assert_eq!(meta.pubkey, expected.pubkey.as_array());
                assert_eq!(meta.is_signer, expected.is_signer);
                assert_eq!(meta.is_writable, expected.is_writable);
            }
        }
    }

    /// Every truncation of the sysvar refuses, and none panics.
    ///
    /// The adapter parses the sysvar itself now, so a short record is its
    /// problem rather than the SDK's. Each prefix is driven through the whole
    /// admission read.
    #[test]
    fn every_truncated_sysvar_prefix_refuses() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        let full = sysvar_bytes(
            &ed25519_program::ID,
            &native,
            &current_data,
            &CURRENT_ACCOUNT,
        );
        for length in 0..full.len() {
            let mut bytes = full.get(..length).expect("prefix").to_vec();
            let mut lamports = 1;
            let instructions = sysvar_info(&mut bytes, &mut lamports);
            assert!(
                !admit_over(&instructions, profile, &current_data),
                "truncation to {length} bytes was admitted"
            );
        }
    }

    /// A substituted current-instruction index is refused, never followed.
    #[test]
    fn substituted_current_instruction_index_refuses() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        // 0 points at the ed25519 batch itself, 2 and 9 past the last record.
        for index in [0_u16, 2, 9, u16::MAX] {
            let mut bytes = sysvar_bytes(
                &ed25519_program::ID,
                &native,
                &current_data,
                &CURRENT_ACCOUNT,
            );
            set_current_index(&mut bytes, index);
            let mut lamports = 1;
            let instructions = sysvar_info(&mut bytes, &mut lamports);
            let mut current_lamports = 1;
            let mut current_account_data = [];
            let owner = Pubkey::new_from_array([93; 32]);
            let current_account = AccountInfo::new(
                &CURRENT_ACCOUNT,
                false,
                false,
                &mut current_lamports,
                &mut current_account_data,
                &owner,
                false,
            );
            assert!(
                authenticate_current_top_level_instruction(
                    &CURRENT_PROGRAM,
                    &[current_account],
                    &current_data,
                    &instructions,
                )
                .is_err(),
                "current index {index} was admitted"
            );
        }
    }

    /// Crafted byte-offset table entries refuse instead of reading elsewhere.
    ///
    /// A record offset is the one field that can point anywhere in the sysvar,
    /// so it is the natural place to try to make the adapter authenticate one
    /// instruction while the runtime executes another: past the end, at the
    /// count field, into the middle of the preceding record, and at the trailing
    /// index field.
    #[test]
    fn crafted_offset_table_entries_refuse() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        let reference = sysvar_bytes(
            &ed25519_program::ID,
            &native,
            &current_data,
            &CURRENT_ACCOUNT,
        );
        let preceding_start = offset_entry(&reference, 0);
        let end = u16::try_from(reference.len()).expect("sysvar width");
        for offset in [
            u16::MAX,
            end,
            end - 1,
            0,
            2,
            4,
            u16::try_from(preceding_start + 1).expect("mid record"),
        ] {
            for instruction in 0..2 {
                let mut bytes = reference.clone();
                set_offset_entry(&mut bytes, instruction, offset);
                let mut lamports = 1;
                let instructions = sysvar_info(&mut bytes, &mut lamports);
                assert!(
                    !admit_over(&instructions, profile, &current_data),
                    "offset {offset} at entry {instruction} was admitted"
                );
            }
        }
    }

    /// A declared account count the record cannot cover refuses.
    ///
    /// The count scales a 33-byte stride, so this is the field that would
    /// overflow a hand-written cursor. It is checked arithmetic and a bounds
    /// check, and the widths below span both failure modes.
    #[test]
    fn oversized_declared_account_count_refuses() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        let reference = sysvar_bytes(
            &ed25519_program::ID,
            &native,
            &current_data,
            &CURRENT_ACCOUNT,
        );
        for count in [2_u16, 64, 1024, u16::MAX] {
            for instruction in 0..2 {
                let mut bytes = reference.clone();
                let start = offset_entry(&bytes, instruction);
                bytes
                    .get_mut(start..start + 2)
                    .expect("account count")
                    .copy_from_slice(&count.to_le_bytes());
                let mut lamports = 1;
                let instructions = sysvar_info(&mut bytes, &mut lamports);
                assert!(
                    !admit_over(&instructions, profile, &current_data),
                    "account count {count} at entry {instruction} was admitted"
                );
            }
        }
    }

    /// A privilege the sysvar reports but the invocation does not present is
    /// refused.
    ///
    /// This is the shape a nested self-CPI would have to forge: the sysvar
    /// record describes the top-level instruction, so an inner invocation that
    /// presented a different signer set than the transaction signed for is
    /// caught by the per-meta privilege comparison rather than by the data
    /// comparison alone.
    #[test]
    fn substituted_meta_privileges_refuse() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        for privileges in [1_u8, 2, 3] {
            let mut bytes = sysvar_bytes(
                &ed25519_program::ID,
                &native,
                &current_data,
                &CURRENT_ACCOUNT,
            );
            let start = offset_entry(&bytes, 1);
            *bytes.get_mut(start + 2).expect("privilege byte") = privileges;
            let mut lamports = 1;
            let instructions = sysvar_info(&mut bytes, &mut lamports);
            let mut current_lamports = 1;
            let mut current_account_data = [];
            let owner = Pubkey::new_from_array([93; 32]);
            let current_account = AccountInfo::new(
                &CURRENT_ACCOUNT,
                false,
                false,
                &mut current_lamports,
                &mut current_account_data,
                &owner,
                false,
            );
            assert!(
                authenticate_current_top_level_instruction(
                    &CURRENT_PROGRAM,
                    &[current_account],
                    &current_data,
                    &instructions,
                )
                .is_err(),
                "privilege byte {privileges} was admitted"
            );
        }
    }

    /// The signed message slice must lie wholly inside the authenticated bytes.
    ///
    /// The ed25519 batch names an absolute offset and width into the top-level
    /// message; the adapter rebases it by the authenticated nested offset. These
    /// are the boundary cases either side of that window.
    #[test]
    fn message_slice_boundaries_refuse_outside_the_authenticated_bytes() {
        let mut nested_hot = [0_u8; 32];
        nested_hot
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let mut registry_outer = vec![0xa5; 128];
        registry_outer.extend_from_slice(&nested_hot);
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        // (message offset in the outer instruction, whether it may seed)
        // 148 is the exact authenticated position of the three signed bytes;
        // 147 and 149 straddle it, and 157 puts the three-byte window exactly at
        // the end of the nested bytes while 158 runs one past it.
        for (offset, admitted) in [(148_u16, true), (147, false), (149, false), (158, false)] {
            let native = ed25519_data(offset, 3, 1, [7; 32]);
            let mut bytes = sysvar_bytes(
                &ed25519_program::ID,
                &native,
                &registry_outer,
                &CURRENT_ACCOUNT,
            );
            let mut lamports = 1;
            let instructions = sysvar_info(&mut bytes, &mut lamports);
            let input = [[0_u8; 32]];
            let mut scratch = [[9_u8; 32]];
            let mut output = [[8_u8; 32]];
            let seeded = seed_native_signatures_at_authenticated_instruction(
                1,
                &nested_hot,
                128,
                &instructions,
                profile,
                0,
                NativeSignatureRegistersV1 {
                    input_identities: &input,
                    scratch_identities: &mut scratch,
                    output_identities: &mut output,
                },
            )
            .is_ok();
            assert_eq!(seeded, admitted, "message offset {offset}");
        }
    }

    #[test]
    fn reads_adjacent_native_instruction_and_seeds_selected_register() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        let mut sysvar_lamports = 1;
        let mut sysvar_data = Vec::new();
        let instructions = sysvar_account(
            &ed25519_program::ID,
            &native,
            &current_data,
            &CURRENT_ACCOUNT,
            &mut sysvar_lamports,
            &mut sysvar_data,
        );
        let mut current_lamports = 1;
        let mut current_account_data = [];
        let owner = Pubkey::new_from_array([93; 32]);
        let current_account = AccountInfo::new(
            &CURRENT_ACCOUNT,
            false,
            false,
            &mut current_lamports,
            &mut current_account_data,
            &owner,
            false,
        );
        let input = [[0_u8; 32]];
        let mut scratch = [[9_u8; 32]];
        let mut output = [[8_u8; 32]];
        authenticate_and_seed_native_signatures(
            &CURRENT_PROGRAM,
            &[current_account],
            &current_data,
            &instructions,
            profile,
            0,
            NativeSignatureRegistersV1 {
                input_identities: &input,
                scratch_identities: &mut scratch,
                output_identities: &mut output,
            },
        )
        .expect("native evidence");
        assert_eq!(output, [[7; 32]]);
    }

    #[test]
    fn registry_outer_binds_detached_message_to_authenticated_nested_hot_bytes() {
        let mut nested_hot = [0_u8; 32];
        nested_hot
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let mut registry_outer = vec![0xa5; 128];
        registry_outer.extend_from_slice(&nested_hot);
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(148, 3, 1, [7; 32]);
        let mut sysvar_lamports = 1;
        let mut sysvar_data = Vec::new();
        let instructions = sysvar_account(
            &ed25519_program::ID,
            &native,
            &registry_outer,
            &CURRENT_ACCOUNT,
            &mut sysvar_lamports,
            &mut sysvar_data,
        );
        let input = [[0_u8; 32]];
        let mut scratch = [[9_u8; 32]];
        let mut output = [[8_u8; 32]];
        seed_native_signatures_at_authenticated_instruction(
            1,
            &nested_hot,
            128,
            &instructions,
            profile,
            0,
            NativeSignatureRegistersV1 {
                input_identities: &input,
                scratch_identities: &mut scratch,
                output_identities: &mut output,
            },
        )
        .expect("detached signature over authenticated Registry bytes");
        assert_eq!(output, [[7; 32]]);

        assert!(
            seed_native_signatures_at_authenticated_instruction(
                1,
                &nested_hot,
                127,
                &instructions,
                profile,
                0,
                NativeSignatureRegistersV1 {
                    input_identities: &input,
                    scratch_identities: &mut scratch,
                    output_identities: &mut output,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn wrong_program_current_bytes_or_nonadjacency_refuse_without_commit() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(20, 3, 1, [7; 32]);
        for case in 0..3 {
            let preceding_program = if case == 0 {
                Pubkey::new_from_array([4; 32])
            } else {
                ed25519_program::ID
            };
            let observed_current = if case == 1 { [3_u8; 32] } else { current_data };
            let mut sysvar_lamports = 1;
            let mut sysvar_data = Vec::new();
            let instructions = sysvar_account(
                &preceding_program,
                &native,
                &observed_current,
                &CURRENT_ACCOUNT,
                &mut sysvar_lamports,
                &mut sysvar_data,
            );
            if case == 2 {
                let data_len = instructions.data_len();
                instructions
                    .data
                    .borrow_mut()
                    .get_mut(data_len - 2..)
                    .expect("current index")
                    .copy_from_slice(&0_u16.to_le_bytes());
            }
            let mut current_lamports = 1;
            let mut current_account_data = [];
            let owner = Pubkey::new_from_array([93; 32]);
            let current_account = AccountInfo::new(
                &CURRENT_ACCOUNT,
                false,
                false,
                &mut current_lamports,
                &mut current_account_data,
                &owner,
                false,
            );
            let input = [[0_u8; 32]];
            let mut scratch = [[9_u8; 32]];
            let mut output = [[8_u8; 32]];
            let before = output;
            assert!(
                authenticate_and_seed_native_signatures(
                    &CURRENT_PROGRAM,
                    &[current_account],
                    &current_data,
                    &instructions,
                    profile,
                    0,
                    NativeSignatureRegistersV1 {
                        input_identities: &input,
                        scratch_identities: &mut scratch,
                        output_identities: &mut output,
                    },
                )
                .is_err()
            );
            assert_eq!(output, before);
        }
    }
}
