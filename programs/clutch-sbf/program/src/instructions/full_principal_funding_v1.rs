//! Shared full-principal predictable-PDA funding.
//!
//! This is not Direct lifecycle state. It is the small construction primitive
//! used by current General/Realm accounts so hostile PDA prefunds remain an
//! independently owned donation floor instead of discounting payer principal.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, require_creatable, transfer_data, RentParameters,
    MAX_PERMITTED_DATA_INCREASE, SYSTEM_PROGRAM_ID,
};
use clutch_solana_layout::Hash32;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::incinerator;

/// Current neutral destination for unowned lamport surplus in this substrate.
pub const FULL_PRINCIPAL_NEUTRAL_SINK_V1: Pubkey = incinerator::ID;

/// Transient construction facts; never persisted as a second account truth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FullPrincipalFundingV1 {
    pub(crate) payer: Hash32,
    pub(crate) payer_principal_lamports: u64,
    pub(crate) prior_donation_lamports: u64,
}

impl FullPrincipalFundingV1 {
    fn validate(self, neutral_sink: Pubkey) -> Outcome<()> {
        require(
            self.payer.bytes() != [0; 32]
                && self.payer.bytes() != neutral_sink.to_bytes()
                && self.payer_principal_lamports != 0,
            ClutchError::MismatchedState,
        )
    }
}

/// Derive exact payer principal and pre-existing donation compartments.
pub(crate) fn full_principal_creation_funding(
    payer: &AccountInfo,
    target: &AccountInfo,
    rent: &RentParameters,
    space: usize,
    neutral_sink: Pubkey,
) -> Outcome<FullPrincipalFundingV1> {
    let funding = FullPrincipalFundingV1 {
        payer: Hash32::from_bytes(payer.key.to_bytes()),
        payer_principal_lamports: rent.minimum_balance(space)?,
        prior_donation_lamports: target.lamports(),
    };
    funding.validate(neutral_sink)?;
    Ok(funding)
}

/// Refresh only the donation compartment of one surviving funded account.
pub(crate) fn observe_full_principal_funding(
    funding: FullPrincipalFundingV1,
    live_lamports: u64,
    neutral_sink: Pubkey,
) -> Outcome<FullPrincipalFundingV1> {
    funding.validate(neutral_sink)?;
    let accounted = funding
        .payer_principal_lamports
        .checked_add(funding.prior_donation_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(live_lamports >= accounted, ClutchError::MismatchedState)?;
    let observed = FullPrincipalFundingV1 {
        prior_donation_lamports: live_lamports
            .checked_sub(funding.payer_principal_lamports)
            .ok_or(ClutchError::Arithmetic)?,
        ..funding
    };
    observed.validate(neutral_sink)?;
    Ok(observed)
}

/// Transfer exact principal, then allocate and assign one predictable PDA.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_pda_account_full_principal<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    funding: FullPrincipalFundingV1,
    extra_deposit: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE,
        ClutchError::AccountCreationFailed,
    )?;
    let principal = rent.minimum_balance(space)?;
    let balance_before = target.lamports();
    require(
        funding.payer == Hash32::from_bytes(payer.key.to_bytes())
            && funding.payer_principal_lamports == principal
            && funding.prior_donation_lamports == balance_before,
        ClutchError::MismatchedState,
    )?;
    let deposit = principal
        .checked_add(extra_deposit)
        .ok_or(ClutchError::Arithmetic)?;
    let balance_after = balance_before
        .checked_add(deposit)
        .ok_or(ClutchError::Arithmetic)?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(deposit),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*target.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[payer.clone(), target.clone(), system_program.clone()],
        &[],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == balance_after,
        ClutchError::AccountCreationFailed,
    )?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == balance_after
            && target.data_len() == space
            && *target.owner == SYSTEM_PROGRAM_ID,
        ClutchError::AccountCreationFailed,
    )?;

    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, true)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.lamports() == balance_after
            && target.data_len() == space
            && target.owner == program_id,
        ClutchError::AccountCreationFailed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_never_spends_principal() {
        let funding = FullPrincipalFundingV1 {
            payer: Hash32::from_bytes([7; 32]),
            payer_principal_lamports: 10,
            prior_donation_lamports: 3,
        };
        assert_eq!(
            observe_full_principal_funding(funding, 17, FULL_PRINCIPAL_NEUTRAL_SINK_V1),
            Ok(FullPrincipalFundingV1 {
                prior_donation_lamports: 7,
                ..funding
            })
        );
        assert!(observe_full_principal_funding(
            funding,
            12,
            FULL_PRINCIPAL_NEUTRAL_SINK_V1
        )
        .is_err());
    }
}
