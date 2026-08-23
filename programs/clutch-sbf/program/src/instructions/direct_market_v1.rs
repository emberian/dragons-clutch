//! Disabled current Direct `80/1` account authentication and writeback plane.
//!
//! This module is intentionally not routed by `dispatch` or admitted by a
//! capability profile. It owns the hostile Solana boundary for the fresh
//! `0xb1..=0xb4/v1` family while economic state and transition identities stay
//! exclusively in `clutch-direct-market-runtime`.

use crate::accounts::{
    expect_pda, require, require_count, require_distinct, require_signer, Outcome,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::SYSTEM_PROGRAM_ID;
use crate::seeds;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, MAX_PERMITTED_DATA_INCREASE,
};
use clutch_direct_market_runtime::codec_v1::{
    decode_direct_action_replay_body_v1, decode_direct_market_root_body_v1,
    decode_direct_reservation_body_v1, decode_direct_selection_body_v1,
    encode_direct_action_replay_body_v1, encode_direct_market_root_body_v1,
    encode_direct_reservation_body_v1, encode_direct_selection_body_v1,
    DIRECT_ACTION_REPLAY_BODY_BYTES_V1 as RUNTIME_REPLAY_BODY_BYTES,
    DIRECT_MARKET_ROOT_BODY_BYTES_V1 as RUNTIME_ROOT_BODY_BYTES,
    DIRECT_RESERVATION_BODY_BYTES_V1 as RUNTIME_RESERVATION_BODY_BYTES,
    DIRECT_SELECTION_BODY_BYTES_V1 as RUNTIME_SELECTION_BODY_BYTES,
};
use clutch_direct_market_runtime::reservation_v1::DirectReservationV1;
use clutch_direct_market_runtime::reservation_v1::AuthenticatedDirectReservationAdmissionV1;
use clutch_direct_market_runtime::selection_v1::DirectSelectionV1;
use clutch_direct_market_runtime::selection_v1::{
    begin_direct_candidate_verification_v1, finalize_direct_selection_v1,
    prepare_direct_selection_freeze_v1, submit_direct_candidate_v1,
    verify_next_direct_candidate_v1, AuthenticatedDirectSelectionFreezeV1,
};
use clutch_direct_market_runtime::{
    DirectActionReplayV1, DirectHashBackendV1, DirectMarketErrorV1, DirectMarketRootV1,
    DirectRentOwnerV1, DirectRootReplayPostV1,
};
use clutch_direct_market_runtime::settlement_v1::{
    prepare_direct_reservation_admission_with_replay_v1,
    DirectReservationOrderInputV1,
};
use clutch_collateral_adapter_v2::{
    refine_market_collateral_v2, BoundCollateralProfileV2, Id as CollateralId,
    MarketCollateralBindingV2,
};
use clutch_general_v2_contract::GeneralReplayTransitionPlanV1;
use clutch_owner_settlement::{AuthenticatedPositionV3, PositionSettlementPoststateV3};
use clutch_retirement::{PositionV3Sha256Backend, ReplayV3HashBackend};
use clutch_batch::relation_v2::{
    price_semantics_digest_v2, EconomicDomainV2, PricePreconditionV2,
    ECONOMIC_RELATION_VERSION_V2,
};
use clutch_price_measure::PriceVectorV3;
use clutch_product_series::{
    CompiledProductSeriesBundleV5, ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2,
    NativeClaimBasisV1, PriceMeasurePolicyV1,
};
use clutch_solana_layout::direct_market_v1::{
    DirectActionReplayAccountV1, DirectMarketRootAccountV1, DirectReservationAccountV1,
    DirectSelectionAccountV1, DIRECT_ACTION_REPLAY_BODY_BYTES_V1,
    DIRECT_MARKET_ROOT_BODY_BYTES_V1, DIRECT_RESERVATION_BODY_BYTES_V1,
    DIRECT_SELECTION_BODY_BYTES_V1, decode_direct_empty_payload_v1,
    DirectAdmitOrderPayloadV1, DirectFreezeBookPayloadV1, DirectSubmitCandidatePayloadV1,
};
use clutch_solana_layout::registry::DirectMarketAction;
use clutch_solana_layout::{account_len, PriceGridAccount};
use clutch_solana_layout::registry::{
    DIRECT_ACTION_REPLAY_ACCOUNT_BYTES, DIRECT_MARKET_ROOT_ACCOUNT_BYTES,
    DIRECT_RESERVATION_ACCOUNT_BYTES, DIRECT_SELECTION_ACCOUNT_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::product_artifact::authenticate_product_artifact_v1;
use super::artifact::read_clock_slot;
use super::collateral_position_v3::authenticate_general_market_v2;
use super::general_v2_position_replay::authenticate_current_general_position_replay_v2;

const DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/account-authentication/v1\0";
const DIRECT_PRICE_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/price-authentication/v1\0";

const _: () = assert!(DIRECT_MARKET_ROOT_BODY_BYTES_V1 == RUNTIME_ROOT_BODY_BYTES);
const _: () = assert!(DIRECT_SELECTION_BODY_BYTES_V1 == RUNTIME_SELECTION_BODY_BYTES);
const _: () = assert!(DIRECT_ACTION_REPLAY_BODY_BYTES_V1 == RUNTIME_REPLAY_BODY_BYTES);
const _: () = assert!(DIRECT_RESERVATION_BODY_BYTES_V1 == RUNTIME_RESERVATION_BODY_BYTES);

/// Runtime SHA-256 implementation for all current Direct semantic identities.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DirectRuntimeSha256V1;

impl DirectHashBackendV1 for DirectRuntimeSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

impl PositionV3Sha256Backend for DirectRuntimeSha256V1 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for DirectRuntimeSha256V1 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Exact authenticated `0xb1/1` Direct root prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectMarketRootV1 {
    account: Pubkey,
    value: DirectMarketRootV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectMarketRootV1 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> DirectMarketRootV1 { self.value }
    pub(crate) const fn bump(self) -> u8 { self.bump }
    pub(crate) const fn data_id(self) -> [u8; 32] { self.data_id }
    pub(crate) const fn semantic_id(self) -> [u8; 32] { self.semantic_id }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
}

/// Exact authenticated permanent `0xb3/1` Direct replay/receipt prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectActionReplayV1 {
    account: Pubkey,
    value: DirectActionReplayV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectActionReplayV1 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> DirectActionReplayV1 { self.value }
    pub(crate) const fn bump(self) -> u8 { self.bump }
    pub(crate) const fn data_id(self) -> [u8; 32] { self.data_id }
    pub(crate) const fn semantic_id(self) -> [u8; 32] { self.semantic_id }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
}

/// Exact authenticated `0xb2/1` Selection prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectSelectionV1 {
    account: Pubkey,
    value: DirectSelectionV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectSelectionV1 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> DirectSelectionV1 { self.value }
    pub(crate) const fn bump(self) -> u8 { self.bump }
    pub(crate) const fn data_id(self) -> [u8; 32] { self.data_id }
    pub(crate) const fn semantic_id(self) -> [u8; 32] { self.semantic_id }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
}

/// Exact authenticated `0xb4/1` funded Reservation prestate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectReservationV1 {
    account: Pubkey,
    value: DirectReservationV1,
    bump: u8,
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
}

impl AuthenticatedDirectReservationV1 {
    pub(crate) const fn account(self) -> Pubkey { self.account }
    pub(crate) const fn value(self) -> DirectReservationV1 { self.value }
    pub(crate) const fn bump(self) -> u8 { self.bump }
    pub(crate) const fn data_id(self) -> [u8; 32] { self.data_id }
    pub(crate) const fn semantic_id(self) -> [u8; 32] { self.semantic_id }
    pub(crate) const fn observed_lamports(self) -> u64 { self.observed_lamports }
}

/// Private Product/PriceGrid-authenticated action-4 input. Construction owns
/// the complete immutable graph join; callers receive only the exact Relation
/// domain and price precondition persisted by b2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDirectPricePreconditionV1 {
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    authentication_id: [u8; 32],
}

impl AuthenticatedDirectPricePreconditionV1 {
    pub(crate) const fn domain(self) -> EconomicDomainV2 { self.domain }
    pub(crate) const fn price(self) -> PricePreconditionV2 { self.price }
    pub(crate) const fn authentication_id(self) -> [u8; 32] { self.authentication_id }
}

/// Authenticate the current Product bundle, native basis, price policy,
/// Genesis V2, and immutable venue grid before b2 may own a price vector.
/// Every active component must be an exact grid tick; every inactive component
/// must be zero; Product independently checks width, scale, and simplex sum.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn authenticate_direct_price_precondition_v1(
    program_id: &Pubkey,
    root: AuthenticatedDirectMarketRootV1,
    bundle_account: &AccountInfo<'_>,
    basis_account: &AccountInfo<'_>,
    price_policy_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    price_grid_account: &AccountInfo<'_>,
    prices: [u64; 16],
    reservation_limits: [Option<u128>; 2],
) -> Outcome<AuthenticatedDirectPricePreconditionV1> {
    let binding = root.value().binding;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        bundle_account,
        ContentId::from_bytes(binding.compiler_bundle_v5_id),
    )?;
    let basis = authenticate_product_artifact_v1::<NativeClaimBasisV1>(
        program_id,
        basis_account,
        bundle.value().native_claim_basis_id.content_id(),
    )?;
    let price_policy = authenticate_product_artifact_v1::<PriceMeasurePolicyV1>(
        program_id,
        price_policy_account,
        bundle.value().price_measure_policy_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle.value().market_genesis_profile_id.content_id(),
    )?;
    require(
        bundle.value().price_measure_policy_id.content_id().bytes() == binding.price_policy_id
            && bundle.value().series_plan_id.bytes() == binding.founder_series_plan_id
            && genesis.value().realm_id.bytes() == binding.realm_id
            && genesis.value().profile_id.bytes() == binding.collateral_profile_id
            && genesis.value().relation_policy_id.bytes() == binding.relation_policy_id
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == binding.price_policy_id
            && basis.value().outcome_count == binding.outcome_count,
        ClutchError::MismatchedState,
    )?;

    require(
        price_grid_account.owner == program_id
            && !price_grid_account.is_signer
            && !price_grid_account.executable
            && !price_grid_account.is_writable
            && price_grid_account.data_len() == account_len::PRICE_GRID,
        ClutchError::MismatchedState,
    )?;
    let grid_data = price_grid_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let grid = PriceGridAccount::decode(&grid_data)?;
    expect_pda(
        price_grid_account.key,
        seeds::grid_pda(program_id, &grid.realm.0, &grid.grid.0),
        Some(grid.stored_bump),
    )?;
    require(
        grid.realm.0 == binding.realm_id
            && grid.grid.0 == genesis.value().price_grid_id.bytes()
            && grid.price_scale == binding.price_scale,
        ClutchError::MismatchedState,
    )?;
    let active = usize::from(binding.outcome_count);
    let mut index = 0usize;
    while index < prices.len() {
        if index < active {
            grid.tick_of(prices[index])?;
        } else {
            require(prices[index] == 0, ClutchError::NonCanonical)?;
        }
        index += 1;
    }
    let mut encoded_limits = [[0u8; 16]; 2];
    index = 0;
    while index < reservation_limits.len() {
        if let Some(limit) = reservation_limits[index] {
            let grid_limit = u64::try_from(limit)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            grid.tick_of(grid_limit)?;
            encoded_limits[index] = limit.to_le_bytes();
        }
        index += 1;
    }
    let price_vector = PriceVectorV3 {
        basis_degree: basis.value().basis_degree,
        native_outcome_count: binding.outcome_count,
        price_scale: grid.price_scale,
        prices,
    };
    price_policy
        .value()
        .validate_candidate_price_contract(basis.value(), &price_vector, grid.price_scale)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let grid_data_id = solana_sha256_hasher::hashv(&[&grid_data[..]]).to_bytes();
    drop(grid_data);

    let domain = EconomicDomainV2 {
        relation_version: ECONOMIC_RELATION_VERSION_V2,
        market_semantics_digest: binding.market_instance_id,
        epoch_semantics_digest: binding.resolution_semantic_id,
        relation_policy_digest: binding.relation_policy_id,
        price_policy_digest: binding.price_policy_id,
        epoch_index: binding.generation,
        outcome_count: binding.outcome_count,
        price_scale: binding.price_scale,
    };
    let semantic_price_digest = price_semantics_digest_v2(&domain, &prices)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let price = PricePreconditionV2 {
        policy_digest: binding.price_policy_id,
        semantic_price_digest,
        prices,
    };
    price
        .validate(&domain)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authentication_id = solana_sha256_hasher::hashv(&[
        DIRECT_PRICE_AUTHENTICATION_DOMAIN_V1,
        &root.semantic_id(),
        bundle_account.key.as_ref(),
        basis_account.key.as_ref(),
        price_policy_account.key.as_ref(),
        genesis_account.key.as_ref(),
        price_grid_account.key.as_ref(),
        &grid_data_id,
        &encoded_limits[0],
        &encoded_limits[1],
        &semantic_price_digest,
    ])
    .to_bytes();
    require_live_id_v1(authentication_id)?;
    Ok(AuthenticatedDirectPricePreconditionV1 {
        domain,
        price,
        authentication_id,
    })
}

/// Execute the complete persisted-selection sublifecycle (actions 5..=8).
///
/// Account order is exact and bounded: writable b1 root, writable permanent b3
/// replay, writable b2 Selection, read-only Clock. No signer or caller index
/// chooses a candidate during verification or finalization; b2's canonical
/// cursor is the only traversal coordinate.
pub(crate) fn process_direct_selection_lifecycle_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DirectMarketAction,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, 4)?;
    require_distinct(accounts)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    let replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        root,
    )?;
    let selection = authenticate_direct_selection_writable_v1(
        program_id,
        &accounts[2],
        root,
    )?;
    let observed_slot = read_clock_slot(&accounts[3])?;
    let state = DirectRootReplayPostV1 {
        root: root.value(),
        replay: replay.value(),
    };
    let plan = match action {
        DirectMarketAction::SubmitCandidate => {
            let candidate = DirectSubmitCandidatePayloadV1::decode(payload)?.candidate;
            submit_direct_candidate_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                candidate,
                &DirectRuntimeSha256V1,
            )
        }
        DirectMarketAction::BeginVerification => {
            decode_direct_empty_payload_v1(payload)?;
            begin_direct_candidate_verification_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                &DirectRuntimeSha256V1,
            )
        }
        DirectMarketAction::VerifyCandidate => {
            decode_direct_empty_payload_v1(payload)?;
            verify_next_direct_candidate_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                &DirectRuntimeSha256V1,
            )
        }
        DirectMarketAction::FinalizeSelection => {
            decode_direct_empty_payload_v1(payload)?;
            finalize_direct_selection_v1(
                state,
                selection.value(),
                sequence,
                observed_slot,
                &DirectRuntimeSha256V1,
            )
        }
        _ => return Err(Refusal::Adapter(ClutchError::UnsupportedInstruction)),
    }
    .map_err(map_direct_error_v1)?;

    // All hostile reads and all pure checks precede the first write. SVM
    // transaction atomicity makes these three postimages one transition.
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        replay.bump(),
        plan.state.replay,
        plan.state.root,
    )?;
    write_direct_selection_v1(
        &accounts[2],
        selection.bump(),
        plan.selection,
        plan.state.root,
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectSelectionFreezeAuthoritySbfV1 {
    root: DirectMarketRootV1,
    selection_account: [u8; 32],
    rent: DirectRentOwnerV1,
    reservation_accounts: [[u8; 32]; 2],
    reservation_semantic_ids: [[u8; 32]; 2],
    reservation_count: u8,
    price: AuthenticatedDirectPricePreconditionV1,
}

impl AuthenticatedDirectSelectionFreezeV1 for DirectSelectionFreezeAuthoritySbfV1 {
    fn authenticate_freeze(
        &self,
        root: DirectMarketRootV1,
        selection_account: [u8; 32],
        rent: DirectRentOwnerV1,
        reservations: &[Option<DirectReservationV1>; 2],
        reservation_semantic_ids: &[[u8; 32]; 2],
        domain: &EconomicDomainV2,
        price: &PricePreconditionV2,
    ) -> Result<(), DirectMarketErrorV1> {
        if root != self.root
            || selection_account != self.selection_account
            || rent != self.rent
            || *domain != self.price.domain()
            || *price != self.price.price()
            || self.price.authentication_id() == [0; 32]
            || *reservation_semantic_ids != self.reservation_semantic_ids
        {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let mut index = 0usize;
        while index < 2 {
            if index < usize::from(self.reservation_count) {
                let reservation = reservations[index]
                    .ok_or(DirectMarketErrorV1::UnauthenticatedAuthority)?;
                if reservation.account() != self.reservation_accounts[index] {
                    return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
                }
            } else if reservations[index].is_some()
                || self.reservation_accounts[index] != [0; 32]
                || self.reservation_semantic_ids[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
            }
            index += 1;
        }
        Ok(())
    }
}

/// Execute action 4 with an exhaustive account-derived Reservation prefix.
///
/// Fixed accounts 0..=11 are root, replay, fresh Selection, payer, System,
/// Rent, Clock, BundleV5, NativeClaimBasis, PriceMeasurePolicy, GenesisV2, and
/// PriceGrid. Exactly `root.live_reservations` read-only b4 accounts follow.
/// No packet count or order index is accepted.
pub(crate) fn process_direct_freeze_book_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(accounts.len() >= 12, ClutchError::AccountCount)?;
    require_distinct(accounts)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    let reservation_count = usize::from(root.value().live_reservations());
    let expected_count = 12usize
        .checked_add(reservation_count)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require_count(accounts, expected_count)?;
    let replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[4])?;
    let rent_parameters = read_rent(&accounts[5])?;
    let observed_slot = read_clock_slot(&accounts[6])?;
    let payload = DirectFreezeBookPayloadV1::decode(payload)?;
    let (selection_pda, selection_bump) =
        seeds::direct_selection_v1_pda(program_id, &root.account());
    let (_, donation_floor_lamports) = authenticate_fresh_direct_pda_v1(
        &accounts[2],
        (selection_pda, selection_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let selection_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    selection_rent.validate().map_err(map_direct_error_v1)?;

    let mut authenticated = [None; 2];
    let mut index = 0usize;
    while index < reservation_count {
        authenticated[index] = Some(authenticate_direct_reservation_readonly_v1(
            program_id,
            &accounts[12 + index],
            root,
        )?);
        index += 1;
    }
    if reservation_count == 2 {
        let left = authenticated[0]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        let right = authenticated[1]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        if right.value().order_id() < left.value().order_id() {
            authenticated = [Some(right), Some(left)];
        }
    }
    let mut reservation_limits = [None; 2];
    index = 0;
    while index < reservation_count {
        let current = authenticated[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        reservation_limits[index] = Some(current.value().limit_price_units_per_egg());
        index += 1;
    }
    let price = authenticate_direct_price_precondition_v1(
        program_id,
        root,
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        payload.prices,
        reservation_limits,
    )?;
    let mut reservations = [None; 2];
    let mut reservation_accounts = [[0u8; 32]; 2];
    let mut reservation_semantic_ids = [[0u8; 32]; 2];
    index = 0;
    while index < reservation_count {
        let current = authenticated[index]
            .ok_or_else(|| Refusal::Adapter(ClutchError::MismatchedState))?;
        reservations[index] = Some(current.value());
        reservation_accounts[index] = current.account().to_bytes();
        reservation_semantic_ids[index] = current.semantic_id();
        index += 1;
    }
    let reservation_count_u8 = u8::try_from(reservation_count)
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    let authority = DirectSelectionFreezeAuthoritySbfV1 {
        root: root.value(),
        selection_account: accounts[2].key.to_bytes(),
        rent: selection_rent,
        reservation_accounts,
        reservation_semantic_ids,
        reservation_count: reservation_count_u8,
        price,
    };
    let plan = prepare_direct_selection_freeze_v1(
        &authority,
        DirectRootReplayPostV1 {
            root: root.value(),
            replay: replay.value(),
        },
        sequence,
        observed_slot,
        accounts[2].key.to_bytes(),
        selection_rent,
        reservations,
        price.domain(),
        price.price(),
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;

    let root_bytes = root.account().to_bytes();
    let bump_seed = [selection_bump];
    let signer_seeds: [&[u8]; 3] = [
        seeds::SEED_DIRECT_SELECTION_V1,
        &root_bytes,
        &bump_seed,
    ];
    create_current_direct_account_v1(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[4],
        &rent_parameters,
        DIRECT_SELECTION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        replay.bump(),
        plan.state.replay,
        plan.state.root,
    )?;
    write_direct_selection_v1(
        &accounts[2],
        selection_bump,
        plan.selection,
        plan.state.root,
    )
}

#[derive(Clone, Copy, Debug)]
struct DirectReservationAdmissionAuthoritySbfV1 {
    root: DirectMarketRootV1,
    position: AuthenticatedPositionV3,
    reservation_account: [u8; 32],
    order_id: [u8; 32],
    side: clutch_batch::Side,
    outcome: u8,
    quantity: u64,
    minimum_fill: u64,
    partial_policy: clutch_batch::PartialPolicy,
    expiry_epoch: u64,
    limit_price_units_per_egg: u128,
    rent: DirectRentOwnerV1,
}

impl AuthenticatedDirectReservationAdmissionV1 for DirectReservationAdmissionAuthoritySbfV1 {
    fn authenticate_admission(
        &self,
        root: DirectMarketRootV1,
        position: AuthenticatedPositionV3,
        reservation_account: [u8; 32],
        order_id: [u8; 32],
        side: clutch_batch::Side,
        outcome: u8,
        quantity: u64,
        minimum_fill: u64,
        partial_policy: clutch_batch::PartialPolicy,
        expiry_epoch: u64,
        limit_price_units_per_egg: u128,
        rent: DirectRentOwnerV1,
    ) -> Result<(), DirectMarketErrorV1> {
        if root == self.root
            && position == self.position
            && reservation_account == self.reservation_account
            && order_id == self.order_id
            && side == self.side
            && outcome == self.outcome
            && quantity == self.quantity
            && minimum_fill == self.minimum_fill
            && partial_policy == self.partial_policy
            && expiry_epoch == self.expiry_epoch
            && limit_price_units_per_egg == self.limit_price_units_per_egg
            && rent == self.rent
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

/// Execute action 2 across b1/b3, a fresh b4, PositionV3, and GEN1.
///
/// The nineteen-account frame is frozen: root, Direct replay, fresh
/// Reservation, owner/payer, Position, GEN1, Realm, Profile, collateral
/// policy, token program, General MarketBindingV2, General runtime,
/// MarketInstanceV2 artifact, System, Rent, Clock, BundleV5, GenesisV2, and
/// PriceGrid. All order funding is derived from authenticated Position state.
pub(crate) fn process_direct_admit_order_v1(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require_count(accounts, 19)?;
    require_distinct(accounts)?;
    let request = DirectAdmitOrderPayloadV1::decode(payload)?;
    let root = authenticate_direct_market_root_writable_v1(program_id, &accounts[0])?;
    let direct_replay = authenticate_direct_action_replay_writable_v1(
        program_id,
        &accounts[1],
        root,
    )?;
    require_signer(&accounts[3])?;
    require(accounts[3].is_writable, ClutchError::NotWritable)?;
    require_system_program(&accounts[13])?;
    let rent_parameters = read_rent(&accounts[14])?;
    let observed_slot = read_clock_slot(&accounts[15])?;
    authenticate_direct_order_limit_v1(
        program_id,
        root,
        &accounts[16],
        &accounts[17],
        &accounts[18],
        request.limit_price_units_per_egg,
    )?;

    let bound = authenticate_direct_general_market_v1(
        program_id,
        root,
        &accounts[6],
        &accounts[7],
        &accounts[8],
        &accounts[9],
        &accounts[10],
        &accounts[11],
        &accounts[12],
        &accounts[17],
    )?;
    let position_replay = authenticate_current_general_position_replay_v2(
        program_id,
        bound,
        &accounts[10],
        &accounts[11],
        &accounts[4],
        &accounts[5],
        accounts[3].key.to_bytes(),
    )?;
    let (reservation_pda, reservation_bump) = seeds::direct_reservation_v1_pda(
        program_id,
        &root.account(),
        &request.order_id,
    );
    let (_, donation_floor_lamports) = authenticate_fresh_direct_pda_v1(
        &accounts[2],
        (reservation_pda, reservation_bump),
    )?;
    let principal_lamports = rent_parameters.minimum_balance(DIRECT_RESERVATION_ACCOUNT_BYTES)?;
    let reservation_rent = DirectRentOwnerV1 {
        payer: accounts[3].key.to_bytes(),
        principal_lamports,
        donation_floor_lamports,
    };
    let authority = DirectReservationAdmissionAuthoritySbfV1 {
        root: root.value(),
        position: position_replay.position,
        reservation_account: accounts[2].key.to_bytes(),
        order_id: request.order_id,
        side: request.side,
        outcome: request.outcome,
        quantity: request.quantity,
        minimum_fill: request.minimum_fill,
        partial_policy: request.partial_policy,
        expiry_epoch: request.expiry_epoch,
        limit_price_units_per_egg: request.limit_price_units_per_egg,
        rent: reservation_rent,
    };
    let plan = prepare_direct_reservation_admission_with_replay_v1(
        &authority,
        DirectRootReplayPostV1 {
            root: root.value(),
            replay: direct_replay.value(),
        },
        position_replay.replay,
        sequence,
        observed_slot,
        DirectReservationOrderInputV1 {
            reservation_account: accounts[2].key.to_bytes(),
            order_id: request.order_id,
            side: request.side,
            outcome: request.outcome,
            quantity: request.quantity,
            minimum_fill: request.minimum_fill,
            partial_policy: request.partial_policy,
            expiry_epoch: request.expiry_epoch,
            limit_price_units_per_egg: request.limit_price_units_per_egg,
            rent: reservation_rent,
        },
        &DirectRuntimeSha256V1,
    )
    .map_err(map_direct_error_v1)?;

    let root_bytes = root.account().to_bytes();
    let bump_seed = [reservation_bump];
    let signer_seeds: [&[u8]; 4] = [
        seeds::SEED_DIRECT_RESERVATION_V1,
        &root_bytes,
        &request.order_id,
        &bump_seed,
    ];
    create_current_direct_account_v1(
        program_id,
        &accounts[3],
        &accounts[2],
        &accounts[13],
        &rent_parameters,
        DIRECT_RESERVATION_ACCOUNT_BYTES,
        principal_lamports,
        donation_floor_lamports,
        &signer_seeds,
    )?;
    write_position_post_v1(&accounts[4], &plan.position_poststate)?;
    write_general_replay_post_v1(&accounts[5], &plan.replay_transition)?;
    write_direct_market_root_v1(&accounts[0], root.bump(), plan.state.root)?;
    write_direct_action_replay_v1(
        &accounts[1],
        direct_replay.bump(),
        plan.state.replay,
        plan.state.root,
    )?;
    write_direct_reservation_v1(
        &accounts[2],
        reservation_bump,
        plan.reservation,
        plan.state.root,
    )
}

#[allow(clippy::too_many_arguments)]
fn authenticate_direct_general_market_v1(
    program_id: &Pubkey,
    root: AuthenticatedDirectMarketRootV1,
    realm_account: &AccountInfo<'_>,
    profile_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    market_binding_account: &AccountInfo<'_>,
    market_runtime_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
) -> Outcome<BoundCollateralProfileV2> {
    let realm = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let (market_binding, market_runtime) = authenticate_general_market_v2(
        program_id,
        market_binding_account,
        market_runtime_account,
    )?;
    let market = market_binding.base();
    let market_instance = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        market.market_instance_v2_id.content_id(),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        market_instance.value().market_genesis_profile_id.content_id(),
    )?;
    let binding = root.value().binding;
    let release_id = realm
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        market.market_instance_v2_id.bytes() == binding.market_instance_id
            && market.outcome_count == binding.outcome_count
            && market.series_plan_v5_id.bytes() == binding.founder_series_plan_id
            && market.relation_policy_id.bytes() == binding.relation_policy_id
            && market.price_measure_policy_v1_id.bytes() == binding.price_policy_id
            && market.neutral_sink.bytes() == binding.neutral_lamport_sink
            && market.price_scale == binding.price_scale
            && genesis.value().realm_id.bytes() == binding.realm_id
            && genesis.value().profile_id.bytes() == binding.collateral_profile_id
            && genesis.value().relation_policy_id.bytes() == binding.relation_policy_id
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == binding.price_policy_id
            && realm.policy_id().bytes() == binding.collateral_policy_id
            && release_id.bytes() == binding.collateral_release_id
            && market_runtime_account.key.to_bytes() == binding.general_market_runtime
            && market_runtime.market_instance_v2_id == market.market_instance_v2_id
            && market_instance.value().id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes() == binding.market_instance_id,
        ClutchError::MismatchedState,
    )?;
    let market_bytes = binding.market_instance_id;
    refine_market_collateral_v2(
        realm,
        MarketCollateralBindingV2 {
            market: CollateralId::from_bytes(market_bytes),
            realm: CollateralId::from_bytes(binding.realm_id),
            profile: CollateralId::from_bytes(binding.collateral_profile_id),
            collateral_cap_atoms: market_instance.value().collateral_cap,
            hoard_authority: CollateralId::from_bytes(
                seeds::hoard_authority_v2_pda(program_id, &market_bytes).0.to_bytes(),
            ),
            hoard_token_account: CollateralId::from_bytes(
                seeds::hoard_token_v2_pda(program_id, &market_bytes).0.to_bytes(),
            ),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))
}

fn authenticate_direct_order_limit_v1(
    program_id: &Pubkey,
    root: AuthenticatedDirectMarketRootV1,
    bundle_account: &AccountInfo<'_>,
    genesis_account: &AccountInfo<'_>,
    price_grid_account: &AccountInfo<'_>,
    limit: u128,
) -> Outcome<()> {
    let binding = root.value().binding;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        bundle_account,
        ContentId::from_bytes(binding.compiler_bundle_v5_id),
    )?;
    let genesis = authenticate_product_artifact_v1::<MarketGenesisProfileV2>(
        program_id,
        genesis_account,
        bundle.value().market_genesis_profile_id.content_id(),
    )?;
    require(
        genesis.value().realm_id.bytes() == binding.realm_id
            && genesis.value().profile_id.bytes() == binding.collateral_profile_id
            && genesis.value().price_measure_policy_id.content_id().bytes()
                == binding.price_policy_id,
        ClutchError::MismatchedState,
    )?;
    require(
        price_grid_account.owner == program_id
            && !price_grid_account.is_signer
            && !price_grid_account.executable
            && !price_grid_account.is_writable
            && price_grid_account.data_len() == account_len::PRICE_GRID,
        ClutchError::MismatchedState,
    )?;
    let grid = PriceGridAccount::decode(&price_grid_account.data.borrow())?;
    expect_pda(
        price_grid_account.key,
        seeds::grid_pda(program_id, &grid.realm.0, &grid.grid.0),
        Some(grid.stored_bump),
    )?;
    let limit = u64::try_from(limit)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    grid.tick_of(limit)?;
    require(
        grid.realm.0 == binding.realm_id
            && grid.grid.0 == genesis.value().price_grid_id.bytes()
            && grid.price_scale == binding.price_scale,
        ClutchError::MismatchedState,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectAccountAccessV1 { ReadOnly, Writable }

impl DirectAccountAccessV1 {
    const fn writable(self) -> bool { matches!(self, Self::Writable) }
}

#[inline(never)]
fn authenticate_root_with_access_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    access: DirectAccountAccessV1,
) -> Outcome<AuthenticatedDirectMarketRootV1> {
    require_program_state_v1(program_id, account, access, DIRECT_MARKET_ROOT_ACCOUNT_BYTES)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_MARKET_ROOT_ACCOUNT_BYTES>(&data)?;
    let frame = DirectMarketRootAccountV1::decode(&bytes)?;
    let value = decode_direct_market_root_body_v1(frame.semantic_body())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_market_root_v1_pda(
        program_id,
        &value.binding.market_instance_id,
        value.binding.generation,
    );
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.binding.direct_root_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.root_rent.principal_lamports,
        value.root_rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(&DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    require_live_id_v1(data_id)?;
    require_live_id_v1(semantic_id)?;
    Ok(AuthenticatedDirectMarketRootV1 {
        account: *account.key, value, bump, data_id, semantic_id, observed_lamports,
    })
}

pub(crate) fn authenticate_direct_market_root_readonly_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedDirectMarketRootV1> {
    authenticate_root_with_access_v1(program_id, account, DirectAccountAccessV1::ReadOnly)
}

pub(crate) fn authenticate_direct_market_root_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedDirectMarketRootV1> {
    authenticate_root_with_access_v1(program_id, account, DirectAccountAccessV1::Writable)
}

#[inline(never)]
pub(crate) fn authenticate_direct_action_replay_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectActionReplayV1> {
    require_program_state_v1(
        program_id,
        account,
        DirectAccountAccessV1::Writable,
        DIRECT_ACTION_REPLAY_ACCOUNT_BYTES,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_ACTION_REPLAY_ACCOUNT_BYTES>(&data)?;
    let frame = DirectActionReplayAccountV1::decode(&bytes)?;
    let value = decode_direct_action_replay_body_v1(frame.semantic_body(), root.value())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_action_replay_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.replay_account == account.key.to_bytes()
            && value.direct_root_account == root.account().to_bytes()
            && root.value().binding.action_replay_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.rent.principal_lamports,
        value.rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(root.value(), &DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    Ok(AuthenticatedDirectActionReplayV1 {
        account: *account.key, value, bump, data_id, semantic_id, observed_lamports,
    })
}

#[inline(never)]
fn authenticate_selection_with_access_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedDirectMarketRootV1,
    access: DirectAccountAccessV1,
) -> Outcome<AuthenticatedDirectSelectionV1> {
    require_program_state_v1(program_id, account, access, DIRECT_SELECTION_ACCOUNT_BYTES)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_SELECTION_ACCOUNT_BYTES>(&data)?;
    let frame = DirectSelectionAccountV1::decode(&bytes)?;
    let value = decode_direct_selection_body_v1(frame.semantic_body(), root.value())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_selection_v1_pda(program_id, &root.account());
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.account() == account.key.to_bytes()
            && root.value().selection_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.rent().principal_lamports,
        value.rent().donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(root.value(), &DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    Ok(AuthenticatedDirectSelectionV1 {
        account: *account.key, value, bump, data_id, semantic_id, observed_lamports,
    })
}

pub(crate) fn authenticate_direct_selection_readonly_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectSelectionV1> {
    authenticate_selection_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::ReadOnly,
    )
}

pub(crate) fn authenticate_direct_selection_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectSelectionV1> {
    authenticate_selection_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::Writable,
    )
}

#[inline(never)]
fn authenticate_reservation_with_access_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedDirectMarketRootV1,
    access: DirectAccountAccessV1,
) -> Outcome<AuthenticatedDirectReservationV1> {
    require_program_state_v1(program_id, account, access, DIRECT_RESERVATION_ACCOUNT_BYTES)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let bytes = exact_array::<DIRECT_RESERVATION_ACCOUNT_BYTES>(&data)?;
    let frame = DirectReservationAccountV1::decode(&bytes)?;
    let value = decode_direct_reservation_body_v1(frame.semantic_body(), root.value())
        .map_err(map_direct_error_v1)?;
    let (expected, bump) = seeds::direct_reservation_v1_pda(
        program_id,
        &root.account(),
        &value.order_id,
    );
    expect_pda(account.key, (expected, bump), Some(frame.bump()))?;
    require(
        value.reservation_account == account.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let observed_lamports = account.lamports();
    require_rent_coverage_v1(
        value.rent.principal_lamports,
        value.rent.donation_floor_lamports,
        observed_lamports,
    )?;
    let data_id = solana_sha256_hasher::hashv(&[&data[..]]).to_bytes();
    drop(data);
    let semantic_id = value
        .semantic_id(&DirectRuntimeSha256V1)
        .map_err(map_direct_error_v1)?;
    Ok(AuthenticatedDirectReservationV1 {
        account: *account.key, value, bump, data_id, semantic_id, observed_lamports,
    })
}

pub(crate) fn authenticate_direct_reservation_readonly_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectReservationV1> {
    authenticate_reservation_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::ReadOnly,
    )
}

pub(crate) fn authenticate_direct_reservation_writable_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    root: AuthenticatedDirectMarketRootV1,
) -> Outcome<AuthenticatedDirectReservationV1> {
    authenticate_reservation_with_access_v1(
        program_id, account, root, DirectAccountAccessV1::Writable,
    )
}

/// Authenticate a fresh System-owned, zero-data current Direct PDA and return
/// its hostile prefund donation floor. The caller still must calculate rent,
/// fund principal without discounting the prefund, allocate, assign, and write.
pub(crate) fn authenticate_fresh_direct_pda_v1(
    account: &AccountInfo<'_>,
    expected: (Pubkey, u8),
) -> Outcome<(u8, u64)> {
    expect_pda(account.key, expected, None)?;
    require(
        !account.is_signer
            && account.is_writable
            && !account.executable
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.data_len() == 0,
        ClutchError::AlreadyInitialized,
    )?;
    Ok((expected.1, account.lamports()))
}

pub(crate) fn write_direct_market_root_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_market_root_body_v1(value).map_err(map_direct_error_v1)?;
    let frame = DirectMarketRootAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_MARKET_ROOT_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

pub(crate) fn write_direct_action_replay_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectActionReplayV1,
    root: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_action_replay_body_v1(value, root).map_err(map_direct_error_v1)?;
    let frame = DirectActionReplayAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_ACTION_REPLAY_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

pub(crate) fn write_direct_selection_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectSelectionV1,
    root: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_selection_body_v1(value, root).map_err(map_direct_error_v1)?;
    let frame = DirectSelectionAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_SELECTION_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

pub(crate) fn write_direct_reservation_v1(
    account: &AccountInfo<'_>,
    bump: u8,
    value: DirectReservationV1,
    root: DirectMarketRootV1,
) -> Outcome<()> {
    let body = encode_direct_reservation_body_v1(value, root).map_err(map_direct_error_v1)?;
    let frame = DirectReservationAccountV1::new(bump, body)?;
    let mut output = [0u8; DIRECT_RESERVATION_ACCOUNT_BYTES];
    frame.encode_into(&mut output)?;
    write_exact_v1(account, &output)
}

fn require_program_state_v1(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    access: DirectAccountAccessV1,
    expected_len: usize,
) -> Outcome<()> {
    require(
        !account.is_signer
            && !account.executable
            && account.owner == program_id
            && account.is_writable == access.writable()
            && account.data_len() == expected_len,
        ClutchError::MismatchedState,
    )
}

fn require_rent_coverage_v1(
    principal: u64,
    donation_floor: u64,
    observed: u64,
) -> Outcome<()> {
    let floor = principal
        .checked_add(donation_floor)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(principal != 0 && observed >= floor, ClutchError::MismatchedState)
}

fn require_live_id_v1(id: [u8; 32]) -> Outcome<()> {
    require(id != [0; 32], ClutchError::MismatchedState)
}

fn exact_array<const N: usize>(input: &[u8]) -> Outcome<[u8; N]> {
    require(input.len() == N, ClutchError::WrongDataLength)?;
    let mut output = [0u8; N];
    output.copy_from_slice(input);
    Ok(output)
}

fn write_exact_v1<const N: usize>(account: &AccountInfo<'_>, value: &[u8; N]) -> Outcome<()> {
    require(
        account.is_writable && !account.executable && account.data_len() == N,
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(value);
    Ok(())
}

#[inline(never)]
fn write_position_post_v1(
    account: &AccountInfo<'_>,
    post: &PositionSettlementPoststateV3,
) -> Outcome<()> {
    let body = post
        .semantic
        .encode()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    write_exact_v1(account, &body)
}

#[inline(never)]
fn write_general_replay_post_v1(
    account: &AccountInfo<'_>,
    post: &GeneralReplayTransitionPlanV1,
) -> Outcome<()> {
    let body = post.replay_poststate_body();
    require(
        account.is_writable && account.data_len() == body.len(),
        ClutchError::MismatchedState,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.copy_from_slice(body);
    Ok(())
}

/// Fund the exact persisted principal in full even when a hostile caller
/// prefunded the PDA, then allocate and assign without changing that donation.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn create_current_direct_account_v1<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    principal_lamports: u64,
    donation_floor_lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_creatable(target)?;
    require_system_program(system_program)?;
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(
        space <= MAX_PERMITTED_DATA_INCREASE
            && principal_lamports == rent.minimum_balance(space)?
            && target.lamports() == donation_floor_lamports,
        ClutchError::MismatchedState,
    )?;
    let expected_balance = donation_floor_lamports
        .checked_add(principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(principal_lamports),
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
        target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(space),
        vec![AccountMeta::new(*target.key, false)],
    );
    invoke_signed(
        &allocate,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*target.key, false)],
    );
    invoke_signed(
        &assign,
        &[target.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        target.data_len() == space
            && target.owner == program_id
            && target.lamports() == expected_balance,
        ClutchError::AccountCreationFailed,
    )
}

fn map_direct_error_v1(error: DirectMarketErrorV1) -> Refusal {
    let adapter = match error {
        DirectMarketErrorV1::Arithmetic => ClutchError::Arithmetic,
        DirectMarketErrorV1::Replay => ClutchError::Replay,
        DirectMarketErrorV1::WrongPhase => ClutchError::NotActive,
        DirectMarketErrorV1::UnauthenticatedAuthority => ClutchError::AuthorizationUnavailable,
        _ => ClutchError::MismatchedState,
    };
    Refusal::Adapter(adapter)
}

fn direct_account_authentication_id_v1(
    account: [u8; 32],
    data_id: [u8; 32],
    semantic_id: [u8; 32],
    observed_lamports: u64,
) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[
        DIRECT_ACCOUNT_AUTHENTICATION_DOMAIN_V1,
        &account,
        &data_id,
        &semantic_id,
        &observed_lamports.to_le_bytes(),
    ])
    .to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_authentication_transcript_is_role_sensitive() {
        let root = direct_account_authentication_id_v1([1; 32], [2; 32], [3; 32], 4);
        let changed_account = direct_account_authentication_id_v1([9; 32], [2; 32], [3; 32], 4);
        let changed_data = direct_account_authentication_id_v1([1; 32], [9; 32], [3; 32], 4);
        let changed_semantic = direct_account_authentication_id_v1([1; 32], [2; 32], [9; 32], 4);
        let changed_lamports = direct_account_authentication_id_v1([1; 32], [2; 32], [3; 32], 9);
        assert_ne!(root, changed_account);
        assert_ne!(root, changed_data);
        assert_ne!(root, changed_semantic);
        assert_ne!(root, changed_lamports);
    }
}
