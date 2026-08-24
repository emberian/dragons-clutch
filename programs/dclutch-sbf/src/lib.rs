#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Atomic dClutch categorical-Pyth founding and resolution adapter.
//!
//! The adapter creates content-addressed collateral Realms and atomically
//! founds a fully authenticated Market with its prepaid resolution Fund. It
//! also authenticates one immutable provider release, posts and checks a fully
//! verified Pyth update, folds it through the total kernel, persists a terminal
//! Market receipt, reclaims the temporary update, and closes the Fund in one
//! transaction. The body-free failure route is permissionless strictly after
//! the immutable price window.

extern crate alloc;

#[cfg(test)]
extern crate std;

use dclutch_collateral_contract::{
    INSTRUCTION_MAGIC as COLLATERAL_INSTRUCTION_MAGIC, InstructionV1 as CollateralInstructionV1,
};
use dclutch_pyth_contract::instruction::ResolveCategoricalInstructionV1;
use dclutch_rent_contract::RENT_CREDIT_INSTRUCTION_MAGIC_V1;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

mod authenticate;
mod close_fund;
mod error;
mod found_market;
mod open_vault;
mod position;
mod provider;
mod realm;
mod rent_credit;
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
    if instruction_data.get(..RENT_CREDIT_INSTRUCTION_MAGIC_V1.len())
        == Some(&RENT_CREDIT_INSTRUCTION_MAGIC_V1)
    {
        return rent_credit::dispatch(program_id, accounts, instruction_data);
    }
    match decode_instruction(instruction_data)? {
        RoutedInstruction::Resolve(instruction) => {
            resolution::dispatch(program_id, accounts, instruction)
        }
        RoutedInstruction::CreateRealm(instruction) => {
            realm::process_create_realm(program_id, accounts, instruction)
        }
        RoutedInstruction::FoundMarketAndFund(instruction) => {
            found_market::process_found_market_and_fund(program_id, accounts, instruction)
        }
        RoutedInstruction::OpenCollateralVault(instruction) => {
            open_vault::process_open_collateral_vault(program_id, accounts, instruction)
        }
        RoutedInstruction::CreatePositionAndSplit(instruction) => {
            position::process_create_position_and_split(program_id, accounts, instruction)
        }
        RoutedInstruction::SplitCompleteSet(instruction) => {
            position::process_split_complete_set(program_id, accounts, instruction)
        }
        RoutedInstruction::MergeCompleteSet(instruction) => {
            position::process_merge_complete_set(program_id, accounts, instruction)
        }
        RoutedInstruction::RedeemResolvedOutcome(instruction) => {
            position::process_redeem_resolved_outcome(program_id, accounts, instruction)
        }
        RoutedInstruction::TransferClaims(instruction) => {
            position::process_transfer_claims(program_id, accounts, instruction)
        }
        RoutedInstruction::SweepSurplus(instruction) => {
            position::process_sweep_surplus(program_id, accounts, instruction)
        }
        RoutedInstruction::CloseEmptyPosition(instruction) => {
            position::process_close_empty_position(program_id, accounts, instruction)
        }
    }
}

enum RoutedInstruction<'a> {
    Resolve(ResolveCategoricalInstructionV1<'a>),
    CreateRealm(dclutch_collateral_contract::CreateRealmV1),
    FoundMarketAndFund(dclutch_collateral_contract::FoundMarketAndFundV1),
    OpenCollateralVault(dclutch_collateral_contract::OpenCollateralVaultV1),
    CreatePositionAndSplit(dclutch_collateral_contract::CreatePositionAndSplitV1),
    SplitCompleteSet(dclutch_collateral_contract::SplitCompleteSetV1),
    MergeCompleteSet(dclutch_collateral_contract::MergeCompleteSetV1),
    RedeemResolvedOutcome(dclutch_collateral_contract::RedeemResolvedOutcomeV1),
    TransferClaims(dclutch_collateral_contract::TransferClaimsV1),
    SweepSurplus(dclutch_collateral_contract::SweepSurplusV1),
    CloseEmptyPosition(dclutch_collateral_contract::CloseEmptyPositionV1),
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
            CollateralInstructionV1::FoundMarketAndFund(instruction) => {
                Ok(RoutedInstruction::FoundMarketAndFund(instruction))
            }
            CollateralInstructionV1::OpenCollateralVault(instruction) => {
                Ok(RoutedInstruction::OpenCollateralVault(instruction))
            }
            CollateralInstructionV1::CreatePositionAndSplit(instruction) => {
                Ok(RoutedInstruction::CreatePositionAndSplit(instruction))
            }
            CollateralInstructionV1::SplitCompleteSet(instruction) => {
                Ok(RoutedInstruction::SplitCompleteSet(instruction))
            }
            CollateralInstructionV1::MergeCompleteSet(instruction) => {
                Ok(RoutedInstruction::MergeCompleteSet(instruction))
            }
            CollateralInstructionV1::RedeemResolvedOutcome(instruction) => {
                Ok(RoutedInstruction::RedeemResolvedOutcome(instruction))
            }
            CollateralInstructionV1::TransferClaims(instruction) => {
                Ok(RoutedInstruction::TransferClaims(instruction))
            }
            CollateralInstructionV1::SweepSurplus(instruction) => {
                Ok(RoutedInstruction::SweepSurplus(instruction))
            }
            CollateralInstructionV1::CloseEmptyPosition(instruction) => {
                Ok(RoutedInstruction::CloseEmptyPosition(instruction))
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
        CREATE_REALM_BYTES, CreateRealmV1, FOUND_MARKET_AND_FUND_BYTES, FoundMarketAndFundV1,
        OPEN_COLLATERAL_VAULT_BYTES, OpenCollateralVaultV1,
    };
    use dclutch_core_contract::{ContentId, MarketIdentity};
    use dclutch_pyth_contract::instruction::{RESOLVE_FAILURE_BYTES, ResolveCategoricalFailureV1};
    use dclutch_realm_contract::{
        FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1, RealmV1Input,
    };
    use dclutch_rent_contract::{CreateRentCreditV1, RefundAuthority, RentCreditInstructionV1};

    use super::*;

    fn realm() -> RealmV1 {
        RealmV1::new(RealmV1Input {
            token_program: [2; 32],
            collateral_mint: [3; 32],
            collateral_adapter_release_id: [4; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("valid Realm")
    }

    #[test]
    fn dispatch_preserves_resolution_and_routes_implemented_collateral_slices() {
        let mut create = [0; CREATE_REALM_BYTES];
        CreateRealmV1::new(realm())
            .encode(&mut create)
            .expect("exact encoding");
        assert!(matches!(
            decode_instruction(&create),
            Ok(RoutedInstruction::CreateRealm(_))
        ));

        let identity = MarketIdentity::new(
            ContentId::new([1; 32]).expect("identity"),
            ContentId::new([2; 32]).expect("identity"),
            ContentId::new([3; 32]).expect("identity"),
            ContentId::new([4; 32]).expect("identity"),
            ContentId::new([5; 32]).expect("identity"),
            7,
        );
        let mut found = [0; FOUND_MARKET_AND_FUND_BYTES];
        FoundMarketAndFundV1::new(identity, 2)
            .expect("valid founding")
            .encode(&mut found)
            .expect("exact encoding");
        assert!(matches!(
            decode_instruction(&found),
            Ok(RoutedInstruction::FoundMarketAndFund(_))
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

        let mut open = [0; OPEN_COLLATERAL_VAULT_BYTES];
        OpenCollateralVaultV1::new(1, 2)
            .encode(&mut open)
            .expect("exact encoding");
        assert!(matches!(
            decode_instruction(&open),
            Ok(RoutedInstruction::OpenCollateralVault(_))
        ));
    }

    #[test]
    fn rent_credit_family_has_a_distinct_top_level_domain() {
        let authority = RefundAuthority::new([9; 32]).expect("nonzero authority");
        let create = CreateRentCreditV1::new(authority, 7).to_bytes();
        assert_eq!(
            create.get(..RENT_CREDIT_INSTRUCTION_MAGIC_V1.len()),
            Some(RENT_CREDIT_INSTRUCTION_MAGIC_V1.as_slice())
        );
        assert_ne!(
            RENT_CREDIT_INSTRUCTION_MAGIC_V1.as_slice(),
            COLLATERAL_INSTRUCTION_MAGIC
        );
        assert!(matches!(
            RentCreditInstructionV1::decode(&create),
            Ok(RentCreditInstructionV1::Create(_))
        ));
    }
}
