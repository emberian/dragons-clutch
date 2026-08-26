//! Family-neutral Instructions-sysvar adapter for native-signature evidence.

use dclutch_request_profile_contract::v2::{
    NativeEd25519InstructionViewV1, NativeSignatureRegistersV1, RequestProfileV2,
    seed_authenticated_signers_atomic,
};
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use solana_sdk_ids::{ed25519_program, sysvar};

use crate::TradingSbfError;

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
    if instructions.key != &sysvar::instructions::ID
        || instructions.is_signer
        || instructions.is_writable
        || instructions.executable
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let current =
        load_current_index_checked(instructions).map_err(|_| TradingSbfError::NativeSignature)?;
    if current == 0 {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let observed = load_instruction_at_checked(usize::from(current), instructions)
        .map_err(|_| TradingSbfError::NativeSignature)?;
    if observed.program_id != *program_id
        || observed.data.as_slice() != current_instruction_data
        || observed.accounts.len() != current_accounts.len()
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    for (meta, account) in observed.accounts.iter().zip(current_accounts) {
        if meta.pubkey != *account.key
            || meta.is_signer != account.is_signer
            || meta.is_writable != account.is_writable
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
    }
    let preceding = load_instruction_at_checked(usize::from(current - 1), instructions)
        .map_err(|_| TradingSbfError::NativeSignature)?;
    if preceding.program_id != ed25519_program::ID || !preceding.accounts.is_empty() {
        return Err(TradingSbfError::NativeSignature.into());
    }
    seed_authenticated_signers_atomic(
        profile,
        tail_count,
        NativeEd25519InstructionViewV1 {
            ed25519_data: &preceding.data,
            current_instruction_data,
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

    fn ed25519_data(message: &[u8], signer: [u8; 32]) -> Vec<u8> {
        let public_key = ED25519_SIGNATURE_OFFSETS_START + ED25519_SIGNATURE_OFFSETS_BYTES;
        let signature = public_key + ED25519_PUBLIC_KEY_BYTES;
        let message_offset = signature + ED25519_SIGNATURE_BYTES;
        let mut bytes = vec![1_u8, 0];
        for value in [
            u16::try_from(signature).expect("offset"),
            ED25519_SELF_INSTRUCTION_INDEX,
            u16::try_from(public_key).expect("offset"),
            ED25519_SELF_INSTRUCTION_INDEX,
            u16::try_from(message_offset).expect("offset"),
            u16::try_from(message.len()).expect("message"),
            ED25519_SELF_INSTRUCTION_INDEX,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&signer);
        bytes.extend_from_slice(&[0x55; ED25519_SIGNATURE_BYTES]);
        bytes.extend_from_slice(message);
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

    #[test]
    fn reads_adjacent_native_instruction_and_seeds_selected_register() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(b"sig", [7; 32]);
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
    fn wrong_program_current_bytes_or_nonadjacency_refuse_without_commit() {
        let mut current_data = [0_u8; 32];
        current_data
            .get_mut(20..23)
            .expect("message")
            .copy_from_slice(b"sig");
        let profile_bytes = profile_bytes(20, 3);
        let profile = RequestProfileV2::decode(&profile_bytes).expect("profile");
        let native = ed25519_data(b"sig", [7; 32]);
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
