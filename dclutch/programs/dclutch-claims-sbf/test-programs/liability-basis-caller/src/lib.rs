#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF wrapper for late LiabilityBasisV2 rollback evidence.
//!
//! This program has no protocol authority. It forwards the caller's complete
//! account frame and opaque instruction bytes to Claims, requires Claims to
//! return successfully, and can then deliberately refuse so ProgramTest can
//! prove transaction rollback across Claims, Custody, and token state.

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryInto;

use dclutch_core_contract::ContentId;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3, FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3,
    FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3, FractionalRetirementRequestV3,
    decode_fractional_capability_root_v4,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

const PROTOCOL_POSITION_MAGIC_V2: &[u8] = b"DCLPPR02";
const PROTOCOL_POSITION_BYTES_V2: usize = 320;

/// Stable test-wrapper refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LiabilityBasisTestCallerError {
    /// Wrapper bytes were malformed.
    Instruction = 0x10_2000,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 0x10_2001,
    /// Production Claims/Custody composition refused or returned no receipt.
    ClaimsCpi = 0x10_2002,
    /// Deliberate refusal after the complete production composition returned.
    DeliberateLateFailure = 0x10_2003,
}

impl LiabilityBasisTestCallerError {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`LiabilityBasisTestCallerError::ordinal`], whose match is exhaustive: a variant added to
    /// the enum does not compile until its author writes an arm here, and the only arm that
    /// satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 4] = [
        Self::Instruction,
        Self::AccountFrame,
        Self::ClaimsCpi,
        Self::DeliberateLateFailure,
    ];

    /// This refusal's position in [`LiabilityBasisTestCallerError::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a fifth variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Instruction => 0,
            Self::AccountFrame => 1,
            Self::ClaimsCpi => 2,
            Self::DeliberateLateFailure => 3,
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
        LiabilityBasisTestCallerError::ALL[0] as u32
            == dclutch_refusal_registry::TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE,
        "LiabilityBasisTestCallerError must start at its registered refusal band base"
    );
    let mut index = 0;
    while index < LiabilityBasisTestCallerError::ALL.len() {
        let variant = LiabilityBasisTestCallerError::ALL[index];
        assert!(
            variant.ordinal() == index,
            "LiabilityBasisTestCallerError::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32
                == dclutch_refusal_registry::TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE + index as u32,
            "LiabilityBasisTestCallerError discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "LiabilityBasisTestCallerError must not run past its registered refusal band"
        );
        index += 1;
    }
};

impl From<LiabilityBasisTestCallerError> for ProgramError {
    fn from(value: LiabilityBasisTestCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one opaque Claims request and optionally refuse after its return.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let fail_after = *instruction_data
        .first()
        .ok_or(LiabilityBasisTestCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(LiabilityBasisTestCallerError::Instruction.into());
    }
    let claims_program = accounts
        .first()
        .ok_or(LiabilityBasisTestCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(LiabilityBasisTestCallerError::AccountFrame)?;
    let request = instruction_data
        .get(1..)
        .ok_or(LiabilityBasisTestCallerError::Instruction)?;
    if !claims_program.executable || claims_program.is_signer || claims_program.is_writable {
        return Err(LiabilityBasisTestCallerError::AccountFrame.into());
    }

    let protocol_position = request.len() == PROTOCOL_POSITION_BYTES_V2
        && request.get(..PROTOCOL_POSITION_MAGIC_V2.len()) == Some(PROTOCOL_POSITION_MAGIC_V2);
    let fractional_retirement = request.len() == FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3
        && request.get(..FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3.len())
            == Some(FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3.as_slice());
    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = account.is_signer
            || protocol_position && index == 0
            || fractional_retirement
                && (index == 0 || index == FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3);
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: request.to_vec(),
    };
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    if fractional_retirement {
        let retirement = FractionalRetirementRequestV3::decode(request)
            .map_err(|_| LiabilityBasisTestCallerError::Instruction)?;
        let input = retirement.input();
        let caller = CallerAuthoritySeedsV1::new(
            ContentId::new(input.release_set)
                .map_err(|_| LiabilityBasisTestCallerError::Instruction)?,
            input.market,
            ExecutionRoleV1::Trading,
            input.terms,
            hash(request).to_bytes(),
        )
        .map_err(|_| LiabilityBasisTestCallerError::Instruction)?;
        let caller_bump = [Pubkey::find_program_address(&caller.as_slices(), program_id).1];
        let [caller_domain, release, market, role, context, digest] = caller.as_slices();
        let root_account = forwarded
            .get(FRACTIONAL_RETIREMENT_COORDINATE_ROOT_V3)
            .ok_or(LiabilityBasisTestCallerError::AccountFrame)?;
        let root_data = root_account
            .try_borrow_data()
            .map_err(|_| LiabilityBasisTestCallerError::AccountFrame)?;
        let root = decode_fractional_capability_root_v4(&root_data)
            .ok_or(LiabilityBasisTestCallerError::AccountFrame)?;
        let root_seeds = root.header().seeds();
        let root_bump = [root.state().input().bump];
        let [
            root_domain,
            root_market,
            generation,
            manifest,
            entry,
            kind,
            root_release,
            config,
        ] = root_seeds.as_slices();
        drop(root_data);
        invoke_signed(
            &instruction,
            &infos,
            &[
                &[
                    caller_domain,
                    release,
                    market,
                    role,
                    context,
                    digest,
                    &caller_bump,
                ],
                &[
                    root_domain,
                    root_market,
                    generation,
                    manifest,
                    entry,
                    kind,
                    root_release,
                    config,
                    &root_bump,
                ],
            ],
        )
        .map_err(|_| LiabilityBasisTestCallerError::ClaimsCpi)?;
    } else if protocol_position {
        let release_set = array::<32>(request, 16)?;
        let market = array::<32>(request, 48)?;
        let position_owner = array::<32>(request, 80)?;
        let seeds = CallerAuthoritySeedsV1::new(
            ContentId::new(release_set).map_err(|_| LiabilityBasisTestCallerError::Instruction)?,
            market,
            ExecutionRoleV1::Trading,
            position_owner,
            hash(request).to_bytes(),
        )
        .map_err(|_| LiabilityBasisTestCallerError::Instruction)?;
        let bump = [Pubkey::find_program_address(&seeds.as_slices(), program_id).1];
        let [domain, release, market, role, context, digest] = seeds.as_slices();
        invoke_signed(
            &instruction,
            &infos,
            &[&[domain, release, market, role, context, digest, &bump]],
        )
        .map_err(|_| LiabilityBasisTestCallerError::ClaimsCpi)?;
    } else {
        invoke(&instruction, &infos).map_err(|_| LiabilityBasisTestCallerError::ClaimsCpi)?;
    }
    let (producer, receipt) = get_return_data().ok_or(LiabilityBasisTestCallerError::ClaimsCpi)?;
    if producer != *claims_program.key || receipt.is_empty() {
        return Err(LiabilityBasisTestCallerError::ClaimsCpi.into());
    }
    if fail_after == 1 {
        return Err(LiabilityBasisTestCallerError::DeliberateLateFailure.into());
    }
    Ok(())
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], ProgramError> {
    let end = offset
        .checked_add(N)
        .ok_or(LiabilityBasisTestCallerError::Instruction)?;
    input
        .get(offset..end)
        .ok_or(LiabilityBasisTestCallerError::Instruction)?
        .try_into()
        .map_err(|_| LiabilityBasisTestCallerError::Instruction.into())
}
