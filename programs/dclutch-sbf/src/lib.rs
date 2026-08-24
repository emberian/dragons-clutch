#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Atomic dClutch categorical-Pyth resolution adapter.
//!
//! The adapter authenticates one immutable provider release, posts and checks
//! a fully verified Pyth update, folds it through the total kernel, persists a
//! terminal Market receipt, reclaims the temporary update, and closes the
//! prepaid resolution Fund in one transaction.  The body-free failure route is
//! permissionless strictly after the immutable price window.

extern crate alloc;

#[cfg(test)]
extern crate std;

use dclutch_collateral_contract::{
    INSTRUCTION_MAGIC as COLLATERAL_INSTRUCTION_MAGIC, InstructionV1 as CollateralInstructionV1,
};
use dclutch_pyth_contract::instruction::ResolveCategoricalInstructionV1;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

mod authenticate;
mod close_fund;
mod error;
mod provider;
mod realm;
mod resolution;
#[cfg(feature = "non-production-real-pyth-lab")]
mod synthetic_release;

pub use error::AdapterError;

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Decode and execute one supported dClutch protocol request.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    match decode_instruction(instruction_data)? {
        RoutedInstruction::Resolve(instruction) => {
            resolution::dispatch(program_id, accounts, instruction)
        }
        RoutedInstruction::CreateRealm(instruction) => {
            realm::process_create_realm(program_id, accounts, instruction)
        }
    }
}

enum RoutedInstruction<'a> {
    Resolve(ResolveCategoricalInstructionV1<'a>),
    CreateRealm(dclutch_collateral_contract::CreateRealmV1),
}

fn decode_instruction(instruction_data: &[u8]) -> Result<RoutedInstruction<'_>, ProgramError> {
    if instruction_data.get(..COLLATERAL_INSTRUCTION_MAGIC.len())
        == Some(&COLLATERAL_INSTRUCTION_MAGIC)
    {
        return match CollateralInstructionV1::decode(instruction_data)
            .map_err(|_| AdapterError::InvalidInstruction)?
        {
            CollateralInstructionV1::CreateRealm(instruction) => {
                Ok(RoutedInstruction::CreateRealm(instruction))
            }
            _ => Err(AdapterError::InvalidInstruction.into()),
        };
    }
    ResolveCategoricalInstructionV1::decode(instruction_data)
        .map(RoutedInstruction::Resolve)
        .map_err(|_| AdapterError::InvalidInstruction.into())
}

#[cfg(test)]
mod tests {
    use dclutch_collateral_contract::{
        CREATE_REALM_BYTES, CreateRealmV1, OPEN_COLLATERAL_VAULT_BYTES, OpenCollateralVaultV1,
    };
    use dclutch_pyth_contract::instruction::{RESOLVE_FAILURE_BYTES, ResolveCategoricalFailureV1};
    use dclutch_realm_contract::{
        FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1, RealmV1Input,
    };

    use super::*;

    fn realm() -> RealmV1 {
        RealmV1::new(RealmV1Input {
            collateral_semantic_id: [1; 32],
            token_program: [2; 32],
            collateral_mint: [3; 32],
            collateral_adapter_release_id: [4; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("valid Realm")
    }

    #[test]
    fn dispatch_preserves_resolution_and_adds_only_create_realm() {
        let mut create = [0; CREATE_REALM_BYTES];
        CreateRealmV1::new(realm())
            .encode(&mut create)
            .expect("exact encoding");
        assert!(matches!(
            decode_instruction(&create),
            Ok(RoutedInstruction::CreateRealm(_))
        ));

        let mut failure = [0; RESOLVE_FAILURE_BYTES];
        ResolveCategoricalFailureV1::new(1, 2)
            .encode(&mut failure)
            .expect("exact encoding");
        assert!(matches!(
            decode_instruction(&failure),
            Ok(RoutedInstruction::Resolve(
                ResolveCategoricalInstructionV1::Failure(_)
            ))
        ));

        let mut unsupported = [0; OPEN_COLLATERAL_VAULT_BYTES];
        OpenCollateralVaultV1::new(1, 2)
            .encode(&mut unsupported)
            .expect("exact encoding");
        assert_eq!(
            decode_instruction(&unsupported).err(),
            Some(ProgramError::from(AdapterError::InvalidInstruction))
        );
    }
}
