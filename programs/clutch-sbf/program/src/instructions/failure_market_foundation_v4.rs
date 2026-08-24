// SPDX-License-Identifier: AGPL-3.0-or-later

//! Physical owners for the six current Failure foundation slots.
//!
//! Product moves the exact ScheduleV4 principal before entering this module.
//! A writer consumes that move-only debit, allocates and assigns the canonical
//! PDA, writes the real owner state, hostile-reopens it, and returns the only
//! postwrite accepted by Product's RootV3 foundation cursor. No signer-funded
//! fallback and no system-owned retained placeholder exists here.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, require_system_program, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_market_lifecycle_v3_current::{
    AuthenticatedProductMarketFoundationDebitV4,
    AuthenticatedProductMarketFoundationStepPostwriteV4,
};
use crate::seeds;
use clutch_collateral_adapter_v2::{
    Id as CollateralId, ResolutionPayoutUnitBoundaryV5, ResolutionStateV5, ResolutionV5,
    RESOLUTION_V5_BYTES,
};
use clutch_product_series::{ContentId, MarketFoundationSlotV4, MarketInstanceV2Id};
use clutch_retirement::DeletableRentOwnerV1;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const FAILURE_FOUNDATION_POSTWRITE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/failure-foundation-postwrite/v4\0";
const INACTIVE_RESOLUTION_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/inactive-resolution-authentication/v5\0";

/// Exact one-use Product postwrite for one of Failure slots 5 through 10.
#[derive(Debug)]
pub(crate) struct AuthenticatedFailureFoundationPostwriteV4 {
    debit_id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    slot: MarketFoundationSlotV4,
    root_transition_sequence_after: u64,
    account: Pubkey,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    destination_donation_floor_lamports: u64,
    destination_balance_after_lamports: u64,
    vault_donation_before_lamports: u64,
    vault_donation_after_lamports: u64,
    foundation_vault_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    poststate_data_id: ContentId,
    poststate_authentication_id: ContentId,
    accepted_poststate_receipt_id: ContentId,
}

impl AuthenticatedProductMarketFoundationStepPostwriteV4
    for AuthenticatedFailureFoundationPostwriteV4
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v4(
        self,
        debit_id: ContentId,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        slot: MarketFoundationSlotV4,
        root_transition_sequence_after: u64,
        account_id: ContentId,
        principal_lamports: u64,
        principal_before_lamports: u64,
        principal_after_lamports: u64,
        destination_donation_floor_lamports: u64,
        destination_balance_after_lamports: u64,
        vault_donation_before_lamports: u64,
        vault_donation_after_lamports: u64,
        foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey,
        neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        require(
            debit_id == self.debit_id
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && slot == self.slot
                && root_transition_sequence_after == self.root_transition_sequence_after
                && account_id.bytes() == self.account.to_bytes()
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && destination_donation_floor_lamports
                    == self.destination_donation_floor_lamports
                && destination_balance_after_lamports == self.destination_balance_after_lamports
                && vault_donation_before_lamports == self.vault_donation_before_lamports
                && vault_donation_after_lamports == self.vault_donation_after_lamports
                && foundation_vault_account == self.foundation_vault_account
                && rent_refund_owner == self.rent_refund_owner
                && neutral_lamport_sink == self.neutral_lamport_sink
                && self.poststate_data_id != ContentId::ZERO
                && self.poststate_authentication_id != ContentId::ZERO
                && self.accepted_poststate_receipt_id != ContentId::ZERO,
            ClutchError::MismatchedState,
        )?;
        Ok((
            self.accepted_poststate_receipt_id,
            self.vault_donation_after_lamports,
        ))
    }
}

/// Validate a post-Product-debit system-owned destination before allocation.
pub(super) fn authenticate_prefunded_failure_destination_v4(
    debit: &AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
    slot: MarketFoundationSlotV4,
    expected_account: Pubkey,
    expected_bytes: usize,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_generation: u64,
    expected_neutral_sink: Pubkey,
) -> Outcome<()> {
    require_system_program(system_program)?;
    let principal = rent.minimum_balance(expected_bytes)?;
    require(
        debit.id() != ContentId::ZERO
            && debit.slot() == slot
            && debit.root_transition_sequence_after() != 0
            && debit.destination_account() == expected_account
            && *account.key == expected_account
            && debit.market_instance_id() == expected_market_instance_id
            && debit.generation() == expected_generation
            && debit.principal_lamports() == principal
            && debit.principal_before_lamports()
                == debit
                    .principal_after_lamports()
                    .checked_add(principal)
                    .ok_or(ClutchError::Arithmetic)?
            && debit.destination_balance_after_lamports()
                == debit
                    .destination_donation_floor_lamports()
                    .checked_add(principal)
                    .ok_or(ClutchError::Arithmetic)?
            && debit.vault_donation_after_lamports()
                == debit.vault_donation_before_lamports()
            && account.lamports() == debit.destination_balance_after_lamports()
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == 0
            && debit.foundation_vault_account() != *account.key
            && debit.rent_refund_owner() != *account.key
            && debit.neutral_lamport_sink() != *account.key
            && debit.foundation_vault_account() != debit.rent_refund_owner()
            && debit.foundation_vault_account() != debit.neutral_lamport_sink()
            && debit.rent_refund_owner() != debit.neutral_lamport_sink()
            && debit.neutral_lamport_sink() == expected_neutral_sink,
        ClutchError::MismatchedState,
    )
}

/// Consume a debit only after an owner-specific hostile reopen.
pub(super) fn finish_failure_foundation_postwrite_v4(
    program_id: &Pubkey,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    expected_slot: MarketFoundationSlotV4,
    poststate_data_id: ContentId,
    poststate_authentication_id: ContentId,
) -> Outcome<AuthenticatedFailureFoundationPostwriteV4> {
    require(
        debit.slot() == expected_slot
            && debit.destination_account() == *account.key
            && account.owner == program_id
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() != 0
            && account.lamports() == debit.destination_balance_after_lamports()
            && poststate_data_id != ContentId::ZERO
            && poststate_authentication_id != ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    let slot_index = u8::try_from(
        expected_slot
            .index()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let accepted_poststate_receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FAILURE_FOUNDATION_POSTWRITE_DOMAIN_V4,
            program_id.as_ref(),
            &debit.id().bytes(),
            &debit.founder_creation_receipt_id().bytes(),
            &debit.founder_preauthorization_id().bytes(),
            &debit.foundation_steps_id().bytes(),
            &debit.market_binding_id().bytes(),
            &debit.foundation_schedule_id().bytes(),
            &debit.foundation_graph_id().bytes(),
            &debit.market_instance_id().bytes(),
            &debit.generation().to_le_bytes(),
            &[slot_index],
            &debit.root_transition_sequence_after().to_le_bytes(),
            account.key.as_ref(),
            &poststate_data_id.bytes(),
            &poststate_authentication_id.bytes(),
            &debit.principal_lamports().to_le_bytes(),
            &debit.destination_donation_floor_lamports().to_le_bytes(),
            &debit.destination_balance_after_lamports().to_le_bytes(),
            debit.rent_refund_owner().as_ref(),
            debit.neutral_lamport_sink().as_ref(),
        ])
        .to_bytes(),
    );
    require(
        accepted_poststate_receipt_id != ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedFailureFoundationPostwriteV4 {
        debit_id: debit.id(),
        founder_creation_receipt_id: debit.founder_creation_receipt_id(),
        founder_preauthorization_id: debit.founder_preauthorization_id(),
        foundation_steps_id: debit.foundation_steps_id(),
        market_binding_id: debit.market_binding_id(),
        foundation_schedule_id: debit.foundation_schedule_id(),
        foundation_graph_id: debit.foundation_graph_id(),
        market_instance_id: debit.market_instance_id(),
        generation: debit.generation(),
        slot: expected_slot,
        root_transition_sequence_after: debit.root_transition_sequence_after(),
        account: *account.key,
        principal_lamports: debit.principal_lamports(),
        principal_before_lamports: debit.principal_before_lamports(),
        principal_after_lamports: debit.principal_after_lamports(),
        destination_donation_floor_lamports: debit.destination_donation_floor_lamports(),
        destination_balance_after_lamports: debit.destination_balance_after_lamports(),
        vault_donation_before_lamports: debit.vault_donation_before_lamports(),
        vault_donation_after_lamports: debit.vault_donation_after_lamports(),
        foundation_vault_account: debit.foundation_vault_account(),
        rent_refund_owner: debit.rent_refund_owner(),
        neutral_lamport_sink: debit.neutral_lamport_sink(),
        poststate_data_id,
        poststate_authentication_id,
        accepted_poststate_receipt_id,
    })
}

fn allocate_assign_failure_pda<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    account_bytes: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_bytes),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &assign,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl clutch_retirement::PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

/// Hostile authentication of a Product-founded, not-yet-finalized payout
/// owner. Redemption paths do not accept this type.
#[derive(Debug)]
pub(crate) struct AuthenticatedInactiveFailureResolutionV5 {
    account: Pubkey,
    resolution: ResolutionV5,
    semantic_id: ContentId,
    data_id: ContentId,
    observed_lamports: u64,
    authentication_id: ContentId,
}

impl AuthenticatedInactiveFailureResolutionV5 {
    pub(crate) const fn account(&self) -> Pubkey { self.account }
    pub(crate) const fn resolution(&self) -> ResolutionV5 { self.resolution }
    pub(crate) const fn semantic_id(&self) -> ContentId { self.semantic_id }
    pub(crate) const fn data_id(&self) -> ContentId { self.data_id }
    pub(crate) const fn observed_lamports(&self) -> u64 { self.observed_lamports }
    pub(crate) const fn authentication_id(&self) -> ContentId { self.authentication_id }
}

/// Reconstruct the exact inactive ResolutionV5 from current chain state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_inactive_failure_resolution_v5(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_market_instance_id: MarketInstanceV2Id,
    expected_native_claim_basis_id: ContentId,
    expected_outcome_count: u8,
    expected_generation: u64,
    expected_rent_refund_owner: Pubkey,
    expected_donation_floor_lamports: u64,
    require_writable: bool,
) -> Outcome<AuthenticatedInactiveFailureResolutionV5> {
    require(
        account.owner == program_id
            && account.is_writable == require_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == RESOLUTION_V5_BYTES,
        ClutchError::MismatchedState,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let resolution = ResolutionV5::decode(&data)
        .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    expect_pda(
        account.key,
        seeds::resolution_v5_pda(program_id, &expected_market_instance_id.bytes()),
        Some(resolution.stored_bump),
    )?;
    require(
        resolution.state == ResolutionStateV5::Inactive
            && resolution.facts.market_instance_id.bytes() == expected_market_instance_id.bytes()
            && resolution.facts.native_claim_basis_id.bytes()
                == expected_native_claim_basis_id.bytes()
            && resolution.facts.outcome_count == expected_outcome_count
            && resolution.facts.generation == expected_generation
            && resolution.facts.finalization_evidence_id.is_zero()
            && resolution.facts.payout_denominator == 0
            && resolution.facts.payout_weights.iter().all(|weight| *weight == 0)
            && resolution.facts.payout_unit_boundary
                == ResolutionPayoutUnitBoundaryV5::ExactWholeCollateralAtoms
            && resolution.rent.payer.bytes() == expected_rent_refund_owner.to_bytes()
            && resolution.rent.donation_floor == expected_donation_floor_lamports
            && account.lamports()
                >= resolution
                    .rent
                    .refundable_principal
                    .checked_add(resolution.rent.donation_floor)
                    .ok_or(ClutchError::Arithmetic)?,
        ClutchError::MismatchedState,
    )?;
    let semantic_id = ContentId::from_bytes(
        resolution
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
            .bytes(),
    );
    let data_id = ContentId::from_bytes(
        resolution
            .data_id(CollateralId::from_bytes(account.key.to_bytes()))
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?
            .bytes(),
    );
    let observed_lamports = account.lamports();
    let authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            INACTIVE_RESOLUTION_AUTHENTICATION_DOMAIN_V5,
            program_id.as_ref(),
            account.key.as_ref(),
            &semantic_id.bytes(),
            &data_id.bytes(),
            &observed_lamports.to_le_bytes(),
            &expected_market_instance_id.bytes(),
            &expected_native_claim_basis_id.bytes(),
            &expected_generation.to_le_bytes(),
            &[expected_outcome_count],
        ])
        .to_bytes(),
    );
    require(
        semantic_id != ContentId::ZERO
            && data_id != ContentId::ZERO
            && authentication_id != ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    drop(data);
    Ok(AuthenticatedInactiveFailureResolutionV5 {
        account: *account.key,
        resolution,
        semantic_id,
        data_id,
        observed_lamports,
        authentication_id,
    })
}

/// Product-funded ScheduleV4 slot 10 writer for inactive ResolutionV5.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn create_inactive_failure_resolution_from_product_foundation_debit_v4<'info>(
    program_id: &Pubkey,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    rent: &RentParameters,
    native_claim_basis_id: ContentId,
    outcome_count: u8,
) -> Outcome<AuthenticatedFailureFoundationPostwriteV4> {
    let market_instance_id = debit.market_instance_id();
    let generation = debit.generation();
    let (expected_account, bump) =
        seeds::resolution_v5_pda(program_id, &market_instance_id.bytes());
    authenticate_prefunded_failure_destination_v4(
        &debit,
        account,
        system_program,
        rent,
        MarketFoundationSlotV4::ResolutionV5,
        expected_account,
        RESOLUTION_V5_BYTES,
        market_instance_id,
        generation,
        debit.neutral_lamport_sink(),
    )?;
    let resolution = ResolutionV5::inactive(
        CollateralId::from_bytes(market_instance_id.bytes()),
        CollateralId::from_bytes(native_claim_basis_id.bytes()),
        outcome_count,
        generation,
        bump,
        DeletableRentOwnerV1 {
            payer: CollateralId::from_bytes(debit.rent_refund_owner().to_bytes()),
            refundable_principal: debit.principal_lamports(),
            donation_floor: debit.destination_donation_floor_lamports(),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = market_instance_id.bytes();
    let bump_seed = [bump];
    allocate_assign_failure_pda(
        program_id,
        account,
        system_program,
        RESOLUTION_V5_BYTES,
        &[seeds::SEED_RESOLUTION_V5, &market, &bump_seed],
    )?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        require(data.iter().all(|byte| *byte == 0), ClutchError::AlreadyInitialized)?;
        resolution
            .encode(&mut data)
            .map_err(|_| Refusal::Adapter(ClutchError::NonCanonical))?;
    }
    let authenticated = authenticate_inactive_failure_resolution_v5(
        program_id,
        account,
        market_instance_id,
        native_claim_basis_id,
        outcome_count,
        generation,
        debit.rent_refund_owner(),
        debit.destination_donation_floor_lamports(),
        true,
    )?;
    require(
        authenticated.resolution == resolution
            && authenticated.observed_lamports == debit.destination_balance_after_lamports(),
        ClutchError::MismatchedState,
    )?;
    finish_failure_foundation_postwrite_v4(
        program_id,
        debit,
        account,
        MarketFoundationSlotV4::ResolutionV5,
        authenticated.data_id,
        authenticated.authentication_id,
    )
}
