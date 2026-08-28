//! Wallet-authorized caller for the existing Claims User Position lifecycle.
//!
//! Claims remains the sole writer and validator of Position/admission state.
//! This outer contributes exactly two facts: the wallet owning the unique
//! Position coordinate signed the top-level instruction, and the current
//! Trading program signed Claims' request-bound caller PDA. The wallet signer
//! is deliberately de-escalated before CPI because Claims' child ABI requires
//! an immutable nonsigner owner-identity observation.

use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_user_position_admission_contract::{
    USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1, USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1,
    USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1,
    USER_POSITION_ADMISSION_CHILD_ACCOUNT_OFFSET_V1,
    USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1,
    USER_POSITION_ADMISSION_CLAIMS_PROGRAM_ACCOUNT_V1, USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
    USER_POSITION_ADMISSION_TRADING_PROGRAM_ACCOUNT_V1, UserPositionAdmissionFrameV1,
    UserPositionAdmissionRequestV1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::TradingSbfError;

/// Execute one exact wallet-authorized User Position admission.
#[inline(never)]
pub fn process_user_position_admission_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let outer = UserPositionAdmissionRequestV1::decode(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_outer_frame_v1(program_id, accounts, outer)?;

    let child_data = outer
        .claims_request_bytes()
        .map_err(|_| TradingSbfError::Content)?;
    let request = outer.claims_request();
    let request_digest = hash(&child_data).to_bytes();
    let authority_seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set,
        request.market,
        ExecutionRoleV1::Trading,
        request.position_owner,
        request_digest,
    )
    .map_err(|_| TradingSbfError::Release)?;
    let (authority, bump) = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if accounts
        .get(USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1)
        .is_none_or(|account| account.key != &authority)
    {
        return Err(TradingSbfError::Release.into());
    }

    let child_accounts = accounts
        .get(
            USER_POSITION_ADMISSION_CHILD_ACCOUNT_OFFSET_V1
                ..USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1,
        )
        .ok_or(TradingSbfError::Content)?;
    if child_accounts.len() != USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1 {
        return Err(TradingSbfError::Content.into());
    }
    let frame = UserPositionAdmissionFrameV1;
    let mut metas = std::vec::Vec::with_capacity(USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1);
    for (child_index, account) in child_accounts.iter().enumerate() {
        let outer_index = child_index
            .checked_add(USER_POSITION_ADMISSION_CHILD_ACCOUNT_OFFSET_V1)
            .ok_or(TradingSbfError::Content)?;
        let privileges = frame
            .privileges(outer_index)
            .map_err(|_| TradingSbfError::Content)?;
        // Child coordinate zero is Trading's PDA signer. The sole top-level
        // wallet signer is intentionally absent from every child meta.
        let signer = child_index == 0;
        metas.push(if privileges.writable() {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let claims_program = accounts
        .get(USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: child_data.to_vec(),
    };
    let mut infos = std::vec::Vec::with_capacity(USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1);
    infos.extend_from_slice(child_accounts);
    infos.push(claims_program.clone());
    let bump_seed = [bump];
    let [domain, release, market, role, context, digest] = authority_seeds.as_slices();
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump_seed]],
    )
    .map_err(|_| TradingSbfError::Transition)?;

    let (producer, receipt) = get_return_data().ok_or(TradingSbfError::Transition)?;
    if producer != *claims_program.key {
        return Err(TradingSbfError::Transition.into());
    }
    outer
        .validate_claims_receipt(
            &receipt,
            request_digest,
            claims_program.key.to_bytes(),
            program_id.to_bytes(),
        )
        .map_err(|_| TradingSbfError::Transition)?;
    Ok(())
}

fn authenticate_outer_frame_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: UserPositionAdmissionRequestV1,
) -> Result<(), ProgramError> {
    let frame = UserPositionAdmissionFrameV1;
    if accounts.len() != frame.account_count() {
        return Err(TradingSbfError::Content.into());
    }
    for (index, account) in accounts.iter().enumerate() {
        let expected = frame
            .privileges(index)
            .map_err(|_| TradingSbfError::Content)?;
        if account.is_signer != expected.signer()
            || account.is_writable != expected.writable()
            || account.executable != expected.executable()
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    let claims_callee = accounts
        .get(USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let trading = accounts
        .get(USER_POSITION_ADMISSION_TRADING_PROGRAM_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let claims_alias = accounts
        .get(USER_POSITION_ADMISSION_CLAIMS_PROGRAM_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    let owner = accounts
        .get(USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1)
        .ok_or(TradingSbfError::Content)?;
    if trading.key != program_id
        || claims_callee.key != claims_alias.key
        || owner.key.to_bytes() != request.claims_request().position_owner
        || owner.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_user_position_admission_contract::{
        ProtocolPositionActionV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
        ProtocolPositionRequestV2,
    };

    fn request() -> UserPositionAdmissionRequestV1 {
        UserPositionAdmissionRequestV1::new(ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::User,
            presence: ProtocolPositionPresenceV2::Vacant,
            release_set: [1; 32],
            market: [2; 32],
            position_owner: [3; 32],
            parent_request_digest: [4; 32],
            rent_credit: [5; 32],
            rent_program: [6; 32],
            generation: 7,
            expected_market_revision: 8,
            expected_position_revision: 0,
            observed_position_lamports: 11,
            observed_admission_lamports: 13,
            position_rent_principal: 11,
            admission_rent_principal: 13,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        })
        .expect("request")
    }

    #[test]
    fn caller_authority_is_bound_to_the_exact_child_request() {
        let request = request();
        let child = request.claims_request_bytes().expect("child");
        let selected = request.claims_request();
        let program = Pubkey::new_from_array([9; 32]);
        let seeds = CallerAuthoritySeedsV1::from_bytes(
            selected.release_set,
            selected.market,
            ExecutionRoleV1::Trading,
            selected.position_owner,
            hash(&child).to_bytes(),
        )
        .expect("seeds");
        let first = Pubkey::find_program_address(&seeds.as_slices(), &program).0;
        let mut hostile = child;
        hostile[112] ^= 1;
        let hostile_seeds = CallerAuthoritySeedsV1::from_bytes(
            selected.release_set,
            selected.market,
            ExecutionRoleV1::Trading,
            selected.position_owner,
            hash(&hostile).to_bytes(),
        )
        .expect("hostile seeds");
        assert_ne!(
            first,
            Pubkey::find_program_address(&hostile_seeds.as_slices(), &program).0
        );
    }
}
