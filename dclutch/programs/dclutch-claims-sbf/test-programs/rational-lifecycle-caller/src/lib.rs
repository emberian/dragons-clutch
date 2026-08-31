#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only Trading caller for real Rational lifecycle rollback evidence.

extern crate alloc;

use alloc::vec::Vec;

use dclutch_claims_svm::protocol_position_v2::{
    ProtocolPositionActionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_RECEIPT_BYTES_V2, LifecycleActionV2, LifecycleReceiptV2, LifecycleRequestV2,
};
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

const CHILD_AUTHORITY_ACCOUNT: usize = 20;

/// Stable test caller refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RationalLifecycleCallerErrorV2 {
    /// Wrapper bytes or the lifecycle request refused.
    Instruction = 0x10_3000,
    /// Claims program or forwarded frame refused.
    Accounts = 0x10_3001,
    /// A release-scoped caller PDA did not join the request.
    Authority = 0x10_3002,
    /// Claims CPI or its exact lifecycle receipt refused.
    Claims = 0x10_3003,
    /// Deliberate refusal after the full Claims route returned.
    DeliberateLateFailure = 0x10_3004,
}

impl RationalLifecycleCallerErrorV2 {
    /// Every refusal this program can raise, in discriminant order.
    ///
    /// This is what the band assertions below read. It is kept honest by
    /// [`RationalLifecycleCallerErrorV2::ordinal`], whose match is exhaustive: a variant added to
    /// the enum does not compile until its author writes an arm here, and the only arm that
    /// satisfies the assertions is its own index in this array.
    pub const ALL: [Self; 5] = [
        Self::Instruction,
        Self::Accounts,
        Self::Authority,
        Self::Claims,
        Self::DeliberateLateFailure,
    ];

    /// This refusal's position in [`RationalLifecycleCallerErrorV2::ALL`].
    ///
    /// The match is exhaustive on purpose, and that is the whole mechanism: a sixth variant is a
    /// COMPILE ERROR here rather than a discriminant no assertion ever looks at.
    const fn ordinal(self) -> usize {
        match self {
            Self::Instruction => 0,
            Self::Accounts => 1,
            Self::Authority => 2,
            Self::Claims => 3,
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
        RationalLifecycleCallerErrorV2::ALL[0] as u32
            == dclutch_refusal_registry::TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE,
        "RationalLifecycleCallerErrorV2 must start at its registered refusal band base"
    );
    let mut index = 0;
    while index < RationalLifecycleCallerErrorV2::ALL.len() {
        let variant = RationalLifecycleCallerErrorV2::ALL[index];
        assert!(
            variant.ordinal() == index,
            "RationalLifecycleCallerErrorV2::ALL repeats a variant, skips one, or is out of discriminant order"
        );
        assert!(
            variant as u32
                == dclutch_refusal_registry::TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE
                    + index as u32,
            "RationalLifecycleCallerErrorV2 discriminants are not the contiguous run from the band base that ALL claims"
        );
        assert!(
            (variant as u32)
                < dclutch_refusal_registry::TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE
                    + dclutch_refusal_registry::BAND_SPAN,
            "RationalLifecycleCallerErrorV2 must not run past its registered refusal band"
        );
        index += 1;
    }
};

impl From<RationalLifecycleCallerErrorV2> for ProgramError {
    fn from(value: RationalLifecycleCallerErrorV2) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Sign every exact Trading authority required by one lifecycle action,
/// invoke the production Claims program, and optionally fail after return.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let fail_after = *instruction_data
        .first()
        .ok_or(RationalLifecycleCallerErrorV2::Instruction)?;
    if fail_after > 1 {
        return Err(RationalLifecycleCallerErrorV2::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(1..)
        .ok_or(RationalLifecycleCallerErrorV2::Instruction)?;
    let request = LifecycleRequestV2::decode(request_bytes)
        .map_err(|_| RationalLifecycleCallerErrorV2::Instruction)?;
    let claims = accounts
        .first()
        .ok_or(RationalLifecycleCallerErrorV2::Accounts)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(RationalLifecycleCallerErrorV2::Accounts)?;
    if !claims.executable || claims.is_signer || claims.is_writable || forwarded.is_empty() {
        return Err(RationalLifecycleCallerErrorV2::Accounts.into());
    }
    let header = request.header();
    let request_digest = hash(request_bytes).to_bytes();
    let outer = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        header.parent_context,
        request_digest,
    )
    .map_err(|_| RationalLifecycleCallerErrorV2::Authority)?;
    let expected_outer = Pubkey::find_program_address(&outer.as_slices(), program_id).0;
    if forwarded
        .first()
        .ok_or(RationalLifecycleCallerErrorV2::Accounts)?
        .key
        != &expected_outer
    {
        return Err(RationalLifecycleCallerErrorV2::Authority.into());
    }

    let coordinate = matches!(
        header.action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    );
    let child = if coordinate {
        Some(child_authority(
            request,
            request_digest,
            program_id,
            forwarded,
        )?)
    } else {
        None
    };
    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer =
            account.is_signer || index == 0 || (coordinate && index == CHILD_AUTHORITY_ACCOUNT);
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };
    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims.clone());

    let outer_bump = [Pubkey::find_program_address(&outer.as_slices(), program_id).1];
    let [
        outer_domain,
        outer_release,
        outer_market,
        outer_role,
        outer_context,
        outer_digest,
    ] = outer.as_slices();
    let outer_signer = [
        outer_domain,
        outer_release,
        outer_market,
        outer_role,
        outer_context,
        outer_digest,
        outer_bump.as_slice(),
    ];
    if let Some(child) = child {
        let child_bump = [Pubkey::find_program_address(&child.as_slices(), program_id).1];
        let [
            child_domain,
            child_release,
            child_market,
            child_role,
            child_context,
            child_digest,
        ] = child.as_slices();
        let child_signer = [
            child_domain,
            child_release,
            child_market,
            child_role,
            child_context,
            child_digest,
            child_bump.as_slice(),
        ];
        invoke_signed(&instruction, &infos, &[&outer_signer, &child_signer])
            .map_err(|_| RationalLifecycleCallerErrorV2::Claims)?;
    } else {
        invoke_signed(&instruction, &infos, &[&outer_signer])
            .map_err(|_| RationalLifecycleCallerErrorV2::Claims)?;
    }
    let (producer, receipt_bytes) =
        get_return_data().ok_or(RationalLifecycleCallerErrorV2::Claims)?;
    if producer != *claims.key || receipt_bytes.len() != LIFECYCLE_RECEIPT_BYTES_V2 {
        return Err(RationalLifecycleCallerErrorV2::Claims.into());
    }
    let receipt = LifecycleReceiptV2::decode(&receipt_bytes)
        .map_err(|_| RationalLifecycleCallerErrorV2::Claims)?;
    if receipt.action() != header.action || receipt.request_digest() != request_digest {
        return Err(RationalLifecycleCallerErrorV2::Claims.into());
    }
    if fail_after == 1 {
        return Err(RationalLifecycleCallerErrorV2::DeliberateLateFailure.into());
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

fn child_authority(
    lifecycle: LifecycleRequestV2<'_>,
    lifecycle_digest: [u8; 32],
    program_id: &Pubkey,
    forwarded: &[AccountInfo<'_>],
) -> Result<CallerAuthoritySeedsV1, ProgramError> {
    let row = lifecycle
        .coordinates()
        .next()
        .ok_or(RationalLifecycleCallerErrorV2::Instruction)?
        .map_err(|_| RationalLifecycleCallerErrorV2::Instruction)?;
    let header = lifecycle.header();
    let owner = forwarded
        .get(25)
        .ok_or(RationalLifecycleCallerErrorV2::Accounts)?
        .key
        .to_bytes();
    let action = if header.action == LifecycleActionV2::ActivateCoordinate {
        ProtocolPositionActionV2::Admit
    } else {
        ProtocolPositionActionV2::Close
    };
    let child_request = ProtocolPositionRequestV2 {
        action,
        owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
        presence: if action == ProtocolPositionActionV2::Admit {
            ProtocolPositionPresenceV2::Vacant
        } else {
            ProtocolPositionPresenceV2::Existing
        },
        release_set: header.release_set,
        market: header.market,
        position_owner: owner,
        parent_request_digest: lifecycle_digest,
        rent_credit: header.rent_credit,
        rent_program: header.rent_program,
        generation: header.generation,
        expected_market_revision: header.expected_claims_market_revision,
        expected_position_revision: row.expected_position_revision,
        observed_position_lamports: row.observed_position_lamports,
        observed_admission_lamports: row.observed_admission_lamports,
        position_rent_principal: row.position_rent_principal,
        admission_rent_principal: row.admission_rent_principal,
        capability_descriptor: header.descriptor_id,
        capability_outcome: row.outcome,
    }
    .new()
    .map_err(|_| RationalLifecycleCallerErrorV2::Instruction)?;
    let child_bytes = child_request
        .to_bytes()
        .map_err(|_| RationalLifecycleCallerErrorV2::Instruction)?;
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        owner,
        hash(&child_bytes).to_bytes(),
    )
    .map_err(|_| RationalLifecycleCallerErrorV2::Authority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    if forwarded
        .get(CHILD_AUTHORITY_ACCOUNT)
        .ok_or(RationalLifecycleCallerErrorV2::Accounts)?
        .key
        != &expected
    {
        return Err(RationalLifecycleCallerErrorV2::Authority.into());
    }
    Ok(seeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_errors() {
        assert_eq!(
            ProgramError::from(RationalLifecycleCallerErrorV2::DeliberateLateFailure),
            ProgramError::Custom(
                dclutch_refusal_registry::TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE + 4
            )
        );
    }
}
