#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for RationalRepresentationV2 rollback evidence.
//!
//! The wrapper derives and signs the exact release-scoped caller-authority PDA,
//! forwards one canonical shared request, and can deliberately refuse only
//! after the complete production Claims child graph returned.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_core_contract::ContentId;
use dclutch_rational_representation_v2_contract::{CallerRoleV2, RepresentationRequestV2};
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
pub enum RationalV2CallerError {
    /// Wrapper bytes did not contain one flag and one canonical request.
    Instruction = 0x10_4000,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 0x10_4001,
    /// Production Claims composition refused or returned no exact receipt.
    ClaimsCpi = 0x10_4002,
    /// Deliberate refusal after the complete production composition returned.
    DeliberateLateFailure = 0x10_4003,
    /// Caller-authority seed material was not canonical.
    Authority = 0x10_4004,
}

impl RationalV2CallerError {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`RationalV2CallerError::ordinal`], whose match is exhaustive: a variant added to the enum
    /// does not compile until its author writes an arm here, and the only arm that satisfies the
    /// assertions is its own index in this array.
    pub const ALL: [Self; 5] = [
        Self::Instruction,
        Self::AccountFrame,
        Self::ClaimsCpi,
        Self::DeliberateLateFailure,
        Self::Authority,
    ];

    /// This refusal's position in [`RationalV2CallerError::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a sixth variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Instruction => 0,
            Self::AccountFrame => 1,
            Self::ClaimsCpi => 2,
            Self::DeliberateLateFailure => 3,
            Self::Authority => 4,
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
        RationalV2CallerError::ALL[0] as u32
            == dclutch_refusal_registry::TEST_CLAIMS_RATIONAL_V2_CALLER_BASE,
        "RationalV2CallerError must start at its registered refusal band base"
    );
    let mut index = 0;
    while index < RationalV2CallerError::ALL.len() {
        let variant = RationalV2CallerError::ALL[index];
        assert!(
            variant.ordinal() == index,
            "RationalV2CallerError::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32
                == dclutch_refusal_registry::TEST_CLAIMS_RATIONAL_V2_CALLER_BASE + index as u32,
            "RationalV2CallerError discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::TEST_CLAIMS_RATIONAL_V2_CALLER_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "RationalV2CallerError must not run past its registered refusal band"
        );
        index += 1;
    }
};

impl From<RationalV2CallerError> for ProgramError {
    fn from(value: RationalV2CallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one exact RationalRepresentationV2 request and optionally refuse
/// after its complete Claims/Token/Custody graph returns.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let fail_after = *instruction_data
        .first()
        .ok_or(RationalV2CallerError::Instruction)?;
    if fail_after > 1 {
        return Err(RationalV2CallerError::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(1..)
        .ok_or(RationalV2CallerError::Instruction)?;
    let request = RepresentationRequestV2::decode(request_bytes)
        .map_err(|_| RationalV2CallerError::Instruction)?;
    let claims_program = accounts
        .first()
        .ok_or(RationalV2CallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(RationalV2CallerError::AccountFrame)?;
    if !claims_program.executable
        || claims_program.is_signer
        || claims_program.is_writable
        || forwarded.is_empty()
    {
        return Err(RationalV2CallerError::AccountFrame.into());
    }
    let header = request.header();
    let role = match header.caller_role {
        CallerRoleV2::Core => ExecutionRoleV1::Core,
        CallerRoleV2::Trading => ExecutionRoleV1::Trading,
    };
    let request_digest = hash(request_bytes).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(header.release_set).map_err(|_| RationalV2CallerError::Authority)?,
        header.market,
        role,
        header.parent_context,
        request_digest,
    )
    .map_err(|_| RationalV2CallerError::Authority)?;
    let expected_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), program_id).0;
    if forwarded
        .first()
        .ok_or(RationalV2CallerError::AccountFrame)?
        .key
        != &expected_authority
    {
        return Err(RationalV2CallerError::Authority.into());
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
    .map_err(|_| RationalV2CallerError::ClaimsCpi)?;
    let (producer, receipt) = get_return_data().ok_or(RationalV2CallerError::ClaimsCpi)?;
    if producer != *claims_program.key {
        return Err(RationalV2CallerError::ClaimsCpi.into());
    }
    if fail_after == 1 {
        return Err(RationalV2CallerError::DeliberateLateFailure.into());
    }
    set_return_data(&receipt);
    Ok(())
}
