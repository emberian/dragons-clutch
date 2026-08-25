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

use dclutch_capability_contract::readiness_instruction::READINESS_INSTRUCTION_MAGIC;
use dclutch_collateral_contract::{
    CloseEmptyPositionV1, CompactTerminalMarketV1, CreatePositionAndSplitV1, CreateRealmV1,
    FoundMarketAndFundV1, INSTRUCTION_MAGIC as COLLATERAL_INSTRUCTION_MAGIC, InstructionTag,
    MergeCompleteSetV1, OpenCollateralVaultV1, RedeemResolvedOutcomeV1, SplitCompleteSetV1,
    SweepSurplusV1, TransferClaimsV1, decode_instruction_tag,
};
use dclutch_general_contract::GENERAL_INSTRUCTION_MAGIC_V1;
use dclutch_pyth_contract::instruction::ResolveCategoricalInstructionV1;
use dclutch_record_contract::RECORD_INSTRUCTION_MAGIC_V1;
use dclutch_rent_contract::RENT_CREDIT_INSTRUCTION_MAGIC_V1;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey};

mod authenticate;
mod bearer;
mod close_fund;
mod dealer;
mod direct;
mod error;
mod found_market;
mod general;
mod open_vault;
mod position;
mod provider;
mod readiness;
mod realm;
mod records;
mod rent_credit;
mod resolution;
mod series;
mod source;
#[cfg(feature = "non-production-real-pyth-lab")]
mod synthetic_release;
mod terminal;

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
    if instruction_data.get(..GENERAL_INSTRUCTION_MAGIC_V1.len())
        == Some(&GENERAL_INSTRUCTION_MAGIC_V1)
    {
        return general::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..dclutch_bearer_contract::instruction::BEARER_INSTRUCTION_MAGIC.len())
        == Some(&dclutch_bearer_contract::instruction::BEARER_INSTRUCTION_MAGIC)
    {
        return bearer::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..dclutch_source_contract::SOURCE_INSTRUCTION_MAGIC.len())
        == Some(&dclutch_source_contract::SOURCE_INSTRUCTION_MAGIC)
    {
        return source::dispatch(program_id, accounts, instruction_data);
    }
    if series::is_routable_instruction(instruction_data) {
        return series::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..dclutch_dealer_contract::instruction::DEALER_INSTRUCTION_MAGIC.len())
        == Some(&dclutch_dealer_contract::instruction::DEALER_INSTRUCTION_MAGIC)
    {
        return dealer::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..dclutch_direct_contract::adapter::DIRECT_ADAPTER_MAGIC_V2.len())
        == Some(&dclutch_direct_contract::adapter::DIRECT_ADAPTER_MAGIC_V2)
    {
        return direct::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..RENT_CREDIT_INSTRUCTION_MAGIC_V1.len())
        == Some(&RENT_CREDIT_INSTRUCTION_MAGIC_V1)
    {
        return rent_credit::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..READINESS_INSTRUCTION_MAGIC.len())
        == Some(&READINESS_INSTRUCTION_MAGIC)
    {
        return readiness::dispatch(program_id, accounts, instruction_data);
    }
    if instruction_data.get(..RECORD_INSTRUCTION_MAGIC_V1.len())
        == Some(&RECORD_INSTRUCTION_MAGIC_V1)
    {
        return records::dispatch(program_id, accounts, instruction_data);
    }
    dispatch_collateral_or_resolution(program_id, accounts, instruction_data)
}

#[inline(never)]
fn dispatch_collateral_or_resolution(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.get(..COLLATERAL_INSTRUCTION_MAGIC.len())
        == Some(&COLLATERAL_INSTRUCTION_MAGIC)
    {
        return dispatch_collateral(program_id, accounts, instruction_data);
    }
    ResolveCategoricalInstructionV1::decode(instruction_data)
        .map_err(|_| AdapterError::InvalidInstruction.into())
        .and_then(|instruction| resolution::dispatch(program_id, accounts, instruction))
}

#[inline(never)]
fn dispatch_collateral(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    match decode_instruction_tag(instruction_data).map_err(|_| AdapterError::InvalidInstruction)? {
        InstructionTag::CreateRealm => {
            dispatch_create_realm(program_id, accounts, instruction_data)
        }
        InstructionTag::FoundMarketAndFund => {
            dispatch_found_market(program_id, accounts, instruction_data)
        }
        InstructionTag::OpenCollateralVault => {
            dispatch_open_vault(program_id, accounts, instruction_data)
        }
        InstructionTag::CreatePositionAndSplit => {
            dispatch_create_position(program_id, accounts, instruction_data)
        }
        InstructionTag::SplitCompleteSet => dispatch_split(program_id, accounts, instruction_data),
        InstructionTag::MergeCompleteSet => dispatch_merge(program_id, accounts, instruction_data),
        InstructionTag::RedeemResolvedOutcome => {
            dispatch_redeem(program_id, accounts, instruction_data)
        }
        InstructionTag::SweepSurplus => dispatch_sweep(program_id, accounts, instruction_data),
        InstructionTag::TransferClaims => dispatch_transfer(program_id, accounts, instruction_data),
        InstructionTag::CloseEmptyPosition => {
            dispatch_close_position(program_id, accounts, instruction_data)
        }
        InstructionTag::CompactTerminalMarket => {
            dispatch_compact_terminal(program_id, accounts, instruction_data)
        }
        InstructionTag::RetireEmptyVault => Err(AdapterError::InvalidInstruction.into()),
    }
}

macro_rules! collateral_dispatch {
    ($name:ident, $instruction:ty, $processor:path) => {
        #[inline(never)]
        fn $name(
            program_id: &Pubkey,
            accounts: &[AccountInfo<'_>],
            instruction_data: &[u8],
        ) -> ProgramResult {
            let instruction = <$instruction>::decode(instruction_data)
                .map_err(|_| AdapterError::InvalidInstruction)?;
            $processor(program_id, accounts, instruction)
        }
    };
}

collateral_dispatch!(
    dispatch_create_realm,
    CreateRealmV1,
    realm::process_create_realm
);
collateral_dispatch!(
    dispatch_found_market,
    FoundMarketAndFundV1,
    found_market::process_found_market_and_fund
);
collateral_dispatch!(
    dispatch_open_vault,
    OpenCollateralVaultV1,
    open_vault::process_open_collateral_vault
);
collateral_dispatch!(
    dispatch_create_position,
    CreatePositionAndSplitV1,
    position::process_create_position_and_split
);
collateral_dispatch!(
    dispatch_split,
    SplitCompleteSetV1,
    position::process_split_complete_set
);
collateral_dispatch!(
    dispatch_merge,
    MergeCompleteSetV1,
    position::process_merge_complete_set
);
collateral_dispatch!(
    dispatch_redeem,
    RedeemResolvedOutcomeV1,
    position::process_redeem_resolved_outcome
);
collateral_dispatch!(
    dispatch_sweep,
    SweepSurplusV1,
    position::process_sweep_surplus
);
collateral_dispatch!(
    dispatch_transfer,
    TransferClaimsV1,
    position::process_transfer_claims
);
collateral_dispatch!(
    dispatch_close_position,
    CloseEmptyPositionV1,
    position::process_close_empty_position
);
collateral_dispatch!(
    dispatch_compact_terminal,
    CompactTerminalMarketV1,
    terminal::process_compact_terminal_market
);

#[cfg(test)]
mod tests {
    use dclutch_collateral_contract::{
        COMPACT_TERMINAL_MARKET_BYTES, CREATE_REALM_BYTES, CompactTerminalMarketV1, CreateRealmV1,
        FOUND_MARKET_AND_FUND_BYTES, FoundMarketAndFundV1,
        InstructionV1 as CollateralInstructionV1, OPEN_COLLATERAL_VAULT_BYTES,
        OpenCollateralVaultV1,
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
            CollateralInstructionV1::decode(&create),
            Ok(CollateralInstructionV1::CreateRealm(_))
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
            CollateralInstructionV1::decode(&found),
            Ok(CollateralInstructionV1::FoundMarketAndFund(_))
        ));

        let mut failure = [0; RESOLVE_FAILURE_BYTES];
        ResolveCategoricalFailureV1::new(1, 2)
            .encode(&mut failure)
            .expect("exact encoding");
        assert!(matches!(
            ResolveCategoricalInstructionV1::decode(&failure),
            Ok(ResolveCategoricalInstructionV1::Failure(_))
        ));

        let mut open = [0; OPEN_COLLATERAL_VAULT_BYTES];
        OpenCollateralVaultV1::new(1, 2)
            .encode(&mut open)
            .expect("exact encoding");
        assert!(matches!(
            CollateralInstructionV1::decode(&open),
            Ok(CollateralInstructionV1::OpenCollateralVault(_))
        ));

        let mut compact = [0; COMPACT_TERMINAL_MARKET_BYTES];
        CompactTerminalMarketV1::new(7)
            .encode(&mut compact)
            .expect("terminal compaction encoding");
        assert!(matches!(
            CollateralInstructionV1::decode(&compact),
            Ok(CollateralInstructionV1::CompactTerminalMarket(_))
        ));
    }

    #[test]
    fn routed_instruction_families_have_distinct_top_level_domains() {
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
        assert_ne!(
            RENT_CREDIT_INSTRUCTION_MAGIC_V1,
            READINESS_INSTRUCTION_MAGIC
        );
        assert_ne!(
            RENT_CREDIT_INSTRUCTION_MAGIC_V1,
            RECORD_INSTRUCTION_MAGIC_V1
        );
        assert_ne!(READINESS_INSTRUCTION_MAGIC, RECORD_INSTRUCTION_MAGIC_V1);
        assert_ne!(READINESS_INSTRUCTION_MAGIC, COLLATERAL_INSTRUCTION_MAGIC);
        assert_ne!(RECORD_INSTRUCTION_MAGIC_V1, COLLATERAL_INSTRUCTION_MAGIC);
        assert!(matches!(
            RentCreditInstructionV1::decode(&create),
            Ok(RentCreditInstructionV1::Create(_))
        ));
    }
}
