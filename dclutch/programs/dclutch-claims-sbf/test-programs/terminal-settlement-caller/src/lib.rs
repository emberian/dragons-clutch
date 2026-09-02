#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for family-neutral terminal-settlement evidence.
//!
//! The wrapper signs the exact release-scoped caller-authority PDA, forwards
//! one canonical DCLTSQ03 request, validates the exact typed receipt returned
//! by Claims, and can then deliberately refuse to prove transaction rollback.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::{
    CallerRole,
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3, TerminalSettlementReceiptV3,
        TerminalSettlementRequestV3,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Stable test-wrapper refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TerminalSettlementCallerError {
    /// Wrapper bytes did not contain one flag and one canonical request.
    Instruction = 0x10_6000,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 0x10_6001,
    /// Release-scoped caller authority did not match the request.
    Authority = 0x10_6002,
    /// Production Claims settlement refused or returned no exact typed receipt.
    ClaimsCpi = 0x10_6003,
    /// Deliberate refusal after Claims and every required child returned.
    DeliberateLateFailure = 0x10_6004,
}

impl TerminalSettlementCallerError {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`TerminalSettlementCallerError::ordinal`], whose match is exhaustive: a variant added to
    /// the enum does not compile until its author writes an arm here, and the only arm that
    /// satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 5] = [
        Self::Instruction,
        Self::AccountFrame,
        Self::Authority,
        Self::ClaimsCpi,
        Self::DeliberateLateFailure,
    ];

    /// This refusal's position in [`TerminalSettlementCallerError::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a sixth variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Instruction => 0,
            Self::AccountFrame => 1,
            Self::Authority => 2,
            Self::ClaimsCpi => 3,
            Self::DeliberateLateFailure => 4,
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
        TerminalSettlementCallerError::ALL[0] as u32
            == dclutch_refusal_registry::TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE,
        "TerminalSettlementCallerError must start at its registered refusal band base"
    );
    let mut index: u32 = 0;
    let mut rest = TerminalSettlementCallerError::ALL.as_slice();
    while let [variant, tail @ ..] = rest {
        let variant = *variant;
        assert!(
            variant.ordinal() == index as usize,
            "TerminalSettlementCallerError::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32
                == dclutch_refusal_registry::TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE + index,
            "TerminalSettlementCallerError discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "TerminalSettlementCallerError must not run past its registered refusal band"
        );
        index += 1;
        rest = tail;
    }
};

impl From<TerminalSettlementCallerError> for ProgramError {
    fn from(value: TerminalSettlementCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one exact DCLTSQ03 request and optionally refuse after the complete
/// production Claims/Custody graph returned.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let fail_after = *instruction_data
        .first()
        .ok_or(TerminalSettlementCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(TerminalSettlementCallerError::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(1..)
        .ok_or(TerminalSettlementCallerError::Instruction)?;
    let request = TerminalSettlementRequestV3::decode(request_bytes)
        .map_err(|_| TerminalSettlementCallerError::Instruction)?;
    let claims_program = accounts
        .first()
        .ok_or(TerminalSettlementCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(TerminalSettlementCallerError::AccountFrame)?;
    if !claims_program.executable
        || claims_program.is_signer
        || claims_program.is_writable
        || forwarded.len() != TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3
    {
        return Err(TerminalSettlementCallerError::AccountFrame.into());
    }
    let input = request.input();
    if claims_program.key.to_bytes() != input.claims_program {
        return Err(TerminalSettlementCallerError::AccountFrame.into());
    }
    // This wrapper exists to be an EXTERNAL caller: it signs a release-pinned
    // caller-authority PDA under its own program id. Execution role `Claims`
    // (decision 0008 §8) is the case with no external caller at all -- its
    // authority is the Position owner's signature, which this program cannot
    // produce and must not pretend to. Refused rather than mapped.
    let role = match input.caller_role {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Trading => ExecutionRoleV1::Trading,
        CallerRole::Claims => return Err(TerminalSettlementCallerError::Authority.into()),
    };
    let request_digest = hash(request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(input.release_set).map_err(|_| TerminalSettlementCallerError::Authority)?,
        input.market,
        role,
        input.parent_context,
        request_digest,
    )
    .map_err(|_| TerminalSettlementCallerError::Authority)?;
    let expected_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id).0;
    if forwarded
        .first()
        .ok_or(TerminalSettlementCallerError::AccountFrame)?
        .key
        != &expected_authority
    {
        return Err(TerminalSettlementCallerError::Authority.into());
    }

    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = index == 0 || account.is_signer;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    let bump = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TerminalSettlementCallerError::ClaimsCpi)?;
    let (producer, receipt_bytes) =
        get_return_data().ok_or(TerminalSettlementCallerError::ClaimsCpi)?;
    if producer != *claims_program.key {
        return Err(TerminalSettlementCallerError::ClaimsCpi.into());
    }
    let receipt = TerminalSettlementReceiptV3::decode(&receipt_bytes)
        .map_err(|_| TerminalSettlementCallerError::ClaimsCpi)?;
    if receipt.request() != request || receipt.evidence().request_digest != request_digest {
        return Err(TerminalSettlementCallerError::ClaimsCpi.into());
    }
    if fail_after == 1 {
        return Err(TerminalSettlementCallerError::DeliberateLateFailure.into());
    }
    set_return_data(&receipt_bytes);
    Ok(())
}
