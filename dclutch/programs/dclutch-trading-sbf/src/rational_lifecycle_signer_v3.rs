//! Trading's delegated protocol-Position signature for a Claims coordinate.
use crate::TradingSbfError;
use dclutch_claims::{
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionClaimsCapabilitySeedsV2,
        ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    },
    rational_lifecycle::{
        LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2, LifecycleActionV2, LifecycleRequestV2,
    },
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo, hash::hash, instruction::AccountMeta, program_error::ProgramError,
    pubkey::Pubkey,
};

#[inline(never)]
pub(super) fn rational_lifecycle_signer_v3(
    trading: &Pubkey,
    wire: &[u8],
    accounts: &[AccountInfo<'_>],
    metas: &mut [AccountMeta],
) -> Result<Option<(CallerAuthoritySeedsV1, u8)>, ProgramError> {
    let request = LifecycleRequestV2::decode(wire).map_err(|_| TradingSbfError::Content)?;
    let header = request.header();
    let action = match header.action {
        LifecycleActionV2::ActivateCoordinate => ProtocolPositionActionV2::Admit,
        LifecycleActionV2::RetireCoordinate => ProtocolPositionActionV2::Close,
        LifecycleActionV2::ActivateReceipt | LifecycleActionV2::RetireReceipt => return Ok(None),
    };
    if accounts.len() != LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2 || metas.len() != accounts.len() {
        return Err(TradingSbfError::Content.into());
    }
    let row = request
        .coordinates()
        .next()
        .ok_or(TradingSbfError::Content)?
        .map_err(|_| TradingSbfError::Content)?;
    let claims = accounts.get(3).ok_or(TradingSbfError::Content)?;
    let owner = accounts.get(25).ok_or(TradingSbfError::Content)?;
    let owner_seeds =
        ProtocolPositionClaimsCapabilitySeedsV2::new(header.descriptor_id, row.outcome)
            .map_err(|_| TradingSbfError::Content)?;
    let expected_owner = Pubkey::find_program_address(&owner_seeds.as_slices(), claims.key).0;
    if owner.key != &expected_owner || row.claims_custody_owner != expected_owner.to_bytes() {
        return Err(TradingSbfError::Content.into());
    }
    // This is the exact request Claims executes internally after authenticating
    // the coordinate. Its digest covers the specialized V2 wire, never V6.
    let position = ProtocolPositionRequestV2 {
        action,
        owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
        presence: if action == ProtocolPositionActionV2::Admit {
            ProtocolPositionPresenceV2::Vacant
        } else {
            ProtocolPositionPresenceV2::Existing
        },
        release_set: header.release_set,
        market: header.market,
        position_owner: expected_owner.to_bytes(),
        parent_request_digest: hash(wire).to_bytes(),
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
    .map_err(|_| TradingSbfError::Content)?;
    let bytes = position.to_bytes().map_err(|_| TradingSbfError::Content)?;
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        header.release_set,
        header.market,
        ExecutionRoleV1::Trading,
        expected_owner.to_bytes(),
        hash(&bytes).to_bytes(),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), trading);
    let account = accounts.get(20).ok_or(TradingSbfError::Content)?;
    let meta = metas.get_mut(20).ok_or(TradingSbfError::Content)?;
    if account.key != &expected
        || account.is_writable
        || account.executable
        || meta.pubkey != expected
        || meta.is_writable
    {
        return Err(TradingSbfError::Content.into());
    }
    meta.is_signer = true;
    Ok(Some((seeds, bump)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::rational_lifecycle::{
        LIFECYCLE_COORDINATE_BYTES_V2, LIFECYCLE_HEADER_BYTES_V2, LifecycleCoordinateV2,
        LifecycleHeaderV2,
    };
    use std::{boxed::Box, vec, vec::Vec};

    fn account(key: Pubkey) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            false,
            false,
            Box::leak(Box::new(0)),
            Box::leak(vec![].into_boxed_slice()),
            Box::leak(Box::new(Pubkey::new_unique())),
            false,
        )
    }

    fn fixture(
        action: LifecycleActionV2,
    ) -> (Pubkey, Vec<u8>, Vec<AccountInfo<'static>>, Vec<AccountMeta>) {
        let trading = Pubkey::new_unique();
        let claims = Pubkey::new_unique();
        let descriptor = [4; 32];
        let owner = Pubkey::find_program_address(
            &ProtocolPositionClaimsCapabilitySeedsV2::new(descriptor, 0)
                .unwrap()
                .as_slices(),
            &claims,
        )
        .0;
        let row = LifecycleCoordinateV2 {
            outcome: 0,
            coefficient: 1,
            shard_mint: [20; 32],
            structured_custody_account: [21; 32],
            claims_custody_owner: owner.to_bytes(),
            claims_custody_position: [23; 32],
            position_admission: [24; 32],
            observed_shard_lamports: 100,
            observed_structured_lamports: 100,
            observed_position_lamports: 100,
            observed_admission_lamports: 100,
            shard_rent_principal: 100,
            structured_rent_principal: 100,
            position_rent_principal: 100,
            admission_rent_principal: 100,
            expected_shard_supply: 0,
            expected_structured_amount: 0,
            expected_position_revision: 0,
        };
        let header = LifecycleHeaderV2 {
            action,
            release_set: [1; 32],
            market: [2; 32],
            graph_id: [3; 32],
            descriptor_id: descriptor,
            parent_context: [5; 32],
            representation_authority: [6; 32],
            receipt_mint: [7; 32],
            token_program: dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID,
            rent_credit: [8; 32],
            rent_program: [9; 32],
            generation: 1,
            expected_claims_market_revision: 0,
            observed_receipt_lamports: 100,
            receipt_rent_principal: 100,
            expected_receipt_supply: 0,
            outcome_count: 3,
            coordinate_count: 1,
            rent_credit_before: 1000,
            rent_credit_after: if action.retires() { 1400 } else { 1000 },
        };
        let mut row_bytes = [0; LIFECYCLE_COORDINATE_BYTES_V2];
        row.encode_into(&mut row_bytes).unwrap();
        let lifecycle = LifecycleRequestV2::new(header, &row_bytes).unwrap();
        let mut wire = vec![0; LIFECYCLE_HEADER_BYTES_V2 + LIFECYCLE_COORDINATE_BYTES_V2];
        lifecycle.encode_into(&mut wire).unwrap();
        let position = ProtocolPositionRequestV2 {
            action: if action.activates() {
                ProtocolPositionActionV2::Admit
            } else {
                ProtocolPositionActionV2::Close
            },
            owner_kind: ProtocolPositionOwnerKindV2::ClaimsCapability,
            presence: if action.activates() {
                ProtocolPositionPresenceV2::Vacant
            } else {
                ProtocolPositionPresenceV2::Existing
            },
            release_set: [1; 32],
            market: [2; 32],
            position_owner: owner.to_bytes(),
            parent_request_digest: hash(&wire).to_bytes(),
            rent_credit: [8; 32],
            rent_program: [9; 32],
            generation: 1,
            expected_market_revision: 0,
            expected_position_revision: 0,
            observed_position_lamports: 100,
            observed_admission_lamports: 100,
            position_rent_principal: 100,
            admission_rent_principal: 100,
            capability_descriptor: descriptor,
            capability_outcome: 0,
        }
        .new()
        .unwrap();
        let authority = CallerAuthoritySeedsV1::from_bytes(
            [1; 32],
            [2; 32],
            ExecutionRoleV1::Trading,
            owner.to_bytes(),
            hash(&position.to_bytes().unwrap()).to_bytes(),
        )
        .unwrap();
        let mut accounts: Vec<_> = (0..LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2)
            .map(|_| account(Pubkey::new_unique()))
            .collect();
        accounts[3] = account(claims);
        accounts[20] = account(Pubkey::find_program_address(&authority.as_slices(), &trading).0);
        accounts[25] = account(owner);
        let metas = accounts
            .iter()
            .map(|a| AccountMeta::new_readonly(*a.key, false))
            .collect();
        (trading, wire, accounts, metas)
    }

    #[test]
    fn rational_coordinate_signs_exact_nested_position_authority_for_both_actions() {
        for action in [
            LifecycleActionV2::ActivateCoordinate,
            LifecycleActionV2::RetireCoordinate,
        ] {
            let (trading, wire, accounts, mut metas) = fixture(action);
            let (seeds, bump) =
                rational_lifecycle_signer_v3(&trading, &wire, &accounts, &mut metas)
                    .unwrap()
                    .expect("coordinate signer");
            let mut signed_seeds = seeds.as_slices().to_vec();
            let bump_bytes = [bump];
            signed_seeds.push(&bump_bytes);
            assert_eq!(
                Pubkey::create_program_address(&signed_seeds, &trading).unwrap(),
                *accounts[20].key
            );
            assert_eq!(
                metas
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.is_signer)
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>(),
                vec![20]
            );
        }
    }

    #[test]
    fn rational_coordinate_refuses_substituted_authority_owner_wire_and_privileges() {
        for mutation in 0..6 {
            let (trading, mut wire, mut accounts, mut metas) =
                fixture(LifecycleActionV2::ActivateCoordinate);
            match mutation {
                0 => {
                    accounts[20] = account(Pubkey::new_unique());
                    metas[20].pubkey = *accounts[20].key;
                }
                1 => {
                    accounts[25] = account(Pubkey::new_unique());
                }
                2 => {
                    wire[144] ^= 1;
                }
                3 => {
                    accounts[20].is_writable = true;
                }
                4 => {
                    metas[20].is_writable = true;
                }
                5 => {
                    metas[20].pubkey = Pubkey::new_unique();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                rational_lifecycle_signer_v3(&trading, &wire, &accounts, &mut metas),
                Err(TradingSbfError::Content.into()),
                "mutation {mutation}"
            );
            assert!(!metas.iter().any(|meta| meta.is_signer));
        }
    }
}
