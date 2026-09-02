#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Readonly ProgramTest caller for the General admitted accelerator.
//!
//! The caller exists only to provide the real instructions-sysvar relationship
//! of a top-level Trading-shaped request invoking the accelerator by CPI. It
//! reads an exact accelerator request from account zero, forwards the remaining
//! frame WITH THE PRIVILEGES IT WAS HANDED, signs only the canonical
//! caller-authority PDA, and relays the accelerator's typed return data. It owns
//! no protocol semantics or state.
//!
//! It forwarded the frame flattened to read-only until the accelerator acquired
//! an output page, which is the one account any admitted accelerator is handed
//! writable. Flattening was never the caller deciding anything: real Trading
//! decides privileges and this program relays them, so relaying them is the
//! honest shape and a hard-coded `false` was a second author of a decision made
//! one frame up.

extern crate alloc;

use alloc::vec::Vec;

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// PDA seed used only by the readonly real-SBF test caller.
pub const GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1: &[u8] =
    b"general-accelerator-test-caller";

/// Stable refusal from the test-only caller.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAcceleratorTestCallerErrorV1 {
    /// The request/program/frame accounts were missing or malformed.
    Frame = 0x10_9000,
    /// The canonical caller-authority PDA differed.
    Authority = 0x10_9001,
    /// The accelerator returned no typed bytes or another producer returned.
    ReturnData = 0x10_9002,
}

impl GeneralAcceleratorTestCallerErrorV1 {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`GeneralAcceleratorTestCallerErrorV1::ordinal`], whose match is exhaustive: a variant added
    /// to the enum does not compile until its author writes an arm here, and the only arm that
    /// satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 3] = [Self::Frame, Self::Authority, Self::ReturnData];

    /// This refusal's position in [`GeneralAcceleratorTestCallerErrorV1::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a fourth variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Frame => 0,
            Self::Authority => 1,
            Self::ReturnData => 2,
        }
    }
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
//
// WHY THIS IS A LIST AND NOT TWO ENDPOINTS. The ceiling assertion used to name
// one variant BY HAND as "the last one". A hand-named ceiling says nothing
// about the variants after it and goes stale silently every single time the
// enum grows -- the failure is not that the name is wrong, it is that nothing
// can notice. Claims proved it the expensive way: its bound went on naming
// `ReleaseSuperseded` after a later variant landed, so for as long as that
// stood, the newest refusal in the program was checked by nothing.
//
// So the band is now checked over `ALL`, element by element, and `ALL` is
// welded to the enum by the exhaustive `ordinal` match. A new variant cannot
// join quietly: it does not compile until its author answers for it, and the
// answer they must give is its index here.
const _: () = {
    assert!(
        GeneralAcceleratorTestCallerErrorV1::ALL[0] as u32
            == dclutch_refusal_registry::TEST_GENERAL_ACCELERATOR_CALLER_BASE,
        "GeneralAcceleratorTestCallerErrorV1 must start at its registered refusal band base"
    );
    let mut index: u32 = 0;
    let mut rest = GeneralAcceleratorTestCallerErrorV1::ALL.as_slice();
    while let [variant, tail @ ..] = rest {
        let variant = *variant;
        assert!(
            variant.ordinal() == index as usize,
            "GeneralAcceleratorTestCallerErrorV1::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32
                == dclutch_refusal_registry::TEST_GENERAL_ACCELERATOR_CALLER_BASE + index,
            "GeneralAcceleratorTestCallerErrorV1 discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::TEST_GENERAL_ACCELERATOR_CALLER_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "GeneralAcceleratorTestCallerErrorV1 must not run past its registered refusal band"
        );
        index += 1;
        rest = tail;
    }
};

impl From<GeneralAcceleratorTestCallerErrorV1> for ProgramError {
    fn from(value: GeneralAcceleratorTestCallerErrorV1) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Invoke one admitted accelerator request without granting state or CPI authority.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    _instruction_data: &[u8],
) -> ProgramResult {
    let request_account = accounts
        .first()
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let accelerator = accounts
        .get(1)
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let frame = accounts
        .get(2..)
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let authority = frame
        .first()
        .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let (expected_authority, bump) = Pubkey::find_program_address(
        &[GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1],
        program_id,
    );
    if authority.key != &expected_authority
        || authority.is_signer
        || authority.is_writable
        || authority.executable
        || !accelerator.executable
        || request_account.is_signer
        || request_account.is_writable
        || request_account.executable
    {
        return Err(GeneralAcceleratorTestCallerErrorV1::Authority.into());
    }
    let request = request_account
        .try_borrow_data()
        .map_err(|_| GeneralAcceleratorTestCallerErrorV1::Frame)?;
    let metas = frame
        .iter()
        .enumerate()
        .map(|(index, account)| AccountMeta {
            pubkey: *account.key,
            is_signer: index == 0,
            is_writable: account.is_writable,
        })
        .collect::<Vec<_>>();
    let instruction = Instruction {
        program_id: *accelerator.key,
        accounts: metas,
        data: request.to_vec(),
    };
    let mut infos = Vec::with_capacity(
        frame
            .len()
            .checked_add(1)
            .ok_or(GeneralAcceleratorTestCallerErrorV1::Frame)?,
    );
    infos.extend(frame.iter().cloned());
    infos.push(accelerator.clone());
    invoke_signed(
        &instruction,
        &infos,
        &[&[GENERAL_ACCELERATOR_TEST_CALLER_AUTHORITY_SEED_V1, &[bump]]],
    )?;
    let (producer, bytes) =
        get_return_data().ok_or(GeneralAcceleratorTestCallerErrorV1::ReturnData)?;
    if producer != *accelerator.key || bytes.is_empty() {
        return Err(GeneralAcceleratorTestCallerErrorV1::ReturnData.into());
    }
    set_return_data(&bytes);
    Ok(())
}
