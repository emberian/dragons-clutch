// SPDX-License-Identifier: AGPL-3.0-or-later

//! Executable non-production Dealer facility transitions.
//!
//! This module owns the Solana trust boundary for the small set of facility
//! actions admitted by the laboratory profile. It authenticates physical
//! account ownership, exact codecs, PDA seeds, rent, Clock, Replay, and the
//! separately owned General/Position/liveness state before applying one atomic
//! postimage. Pure Dealer economics remain in `clutch-dealer-runtime-contract`.

use crate::accounts::{expect_pda, require, require_count, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_dealer_runtime_contract::{
    dealer_runtime_liveness_policy_id_v1,
    prepare_bind_epoch_v3, prepare_dealer_sponsor_funding_transfer_v1,
    prepare_facility_initialization_v3, DealerActionReceiptV1, DealerChildCountsV2,
    DealerEpochBindingV2, DealerFacilityGenesisV1, DealerFacilityReplayV1,
    DealerFundedBudgetDependenciesV1, DealerFundedDependenciesV2, DealerGeneralEpochEvidenceV3,
    DealerLivenessCompartmentV1, DealerLivenessScheduleV1, DealerPhaseV2,
    DealerPositionMarketJoinV1, DealerPositionObservationV3, DealerReplayAccountBindingV1,
    DealerRuntimeActionV1, DealerRuntimeLivenessBindingV1, DealerStateV2, DealerTransferPositionV3,
    DeletableRentOwnerV1, FacilityPositionBindingV2, FixedCodec, Id, RootRentOwnerV1,
    SponsorCapitalDispositionV1,
};
use clutch_general_v2_contract::{
    CandidateWindowV4AccountV1, EconomicDomainV2AccountV1, GeneralEpochV6AccountV1,
    ECONOMIC_DOMAIN_ACCOUNT_BYTES, GENERAL_EPOCH_ACCOUNT_BYTES, WINDOW_ACCOUNT_BYTES,
};
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimePersistedAccountViewV1, RuntimeTransferRoleV1,
};
use clutch_liveness::runtime_v1::{
    PresentFundingSourceV1, PresentFundingV1, RuntimeCompartmentAdmissionV1,
    RuntimeCompartmentIdentityV1, RuntimeCompartmentKindV1, RuntimeCompartmentV1,
    RuntimeLivenessBundleV1, RuntimeLivenessPolicyV1, RUNTIME_COMPARTMENT_COUNT_V1,
    RUNTIME_COMPARTMENT_ORDER_V1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_retirement::{
    project_dealer_position_v3, project_general_position_v3, AdapterPositionMarketBindingV3,
    AdapterPositionPurposeBindingV3, DeletableRentOwnerV1 as ReplayRentOwnerV1, Identity32V1,
    PositionAccountV3, PositionLifecycleV3, PositionPurposeV3, PositionV3Fields,
    PositionV3Sha256Backend, RentSplitV2, POSITION_TOMBSTONE_V3_BYTES, POSITION_V3_BYTES,
};
use clutch_solana_layout::registry::{
    DealerFacilityAction, DEALER_ACTION_RECEIPT_ACCOUNT_BYTES, DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
    DEALER_ACTION_RECEIPT_ACCOUNT_VERSION, DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
    DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG, DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES, DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
    DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION, DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG, DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
    DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES, DEALER_STATE_V2_ACCOUNT_BYTES,
    DEALER_STATE_V2_ACCOUNT_TAG, DEALER_STATE_V2_ACCOUNT_VERSION,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::artifact::read_clock_slot;
use super::collateral_position_v3::RuntimeSha256;
use super::dealer_policy::{
    authenticate_catalog_policy, create_exact_payer_debit_pda, create_full_principal_pda,
    dealer_fault,
};
use super::dealer_runtime::{
    decode_dealer_account_body_v1, encode_dealer_account_body_v1, DealerRuntimePayloadV1,
};
use super::genesis::{read_rent, require_creatable, require_system_program};

const INITIALIZE_ACCOUNT_COUNT: usize = 22;
const BIND_EPOCH_ACCOUNT_COUNT: usize = 24;

fn id(key: &Pubkey) -> Id {
    Id::from_bytes(key.to_bytes())
}

fn retirement_id(value: Id) -> Outcome<Identity32V1> {
    Identity32V1::new(value.bytes()).map_err(|_| ClutchError::MismatchedState.into())
}

fn liveness_id(value: Id) -> clutch_liveness::Id {
    clutch_liveness::Id::from_bytes(value.bytes())
}

const fn runtime_kind_seed(kind: RuntimeCompartmentKindV1) -> u8 {
    match kind {
        RuntimeCompartmentKindV1::Source => 0,
        RuntimeCompartmentKindV1::Candidate => 1,
        RuntimeCompartmentKindV1::Clearing => 2,
        RuntimeCompartmentKindV1::Settlement => 3,
        RuntimeCompartmentKindV1::Resolution => 4,
        RuntimeCompartmentKindV1::Retirement => 5,
        RuntimeCompartmentKindV1::Recovery => 6,
    }
}

#[inline(never)]
fn prepare_runtime_compartment_admission(
    program_id: &Pubkey,
    facility_id: Id,
    state_account_id: Id,
    payer: &AccountInfo<'_>,
    account: &AccountInfo<'_>,
    policy: RuntimeLivenessPolicyV1,
    kind: RuntimeCompartmentKindV1,
    required_rent_principal: u64,
) -> Outcome<(RuntimeCompartmentV1, u8)> {
    require_creatable(account)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        !account.is_signer && !account.executable,
        ClutchError::MismatchedState,
    )?;
    let compartment_policy = policy.compartment(kind);
    require(
        compartment_policy.account_rent_principal_lamports == required_rent_principal,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let (address, bump) = seeds::dealer_runtime_liveness_account_pda(
        program_id,
        &facility_id.bytes(),
        runtime_kind_seed(kind),
    );
    expect_pda(account.key, (address, bump), None)?;
    let payer_debit = compartment_policy
        .total_payer_debit_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let balance_before = account.lamports();
    let balance_after = balance_before
        .checked_add(payer_debit)
        .ok_or(ClutchError::Arithmetic)?;
    let semantic_owner = if kind == RuntimeCompartmentKindV1::Source {
        compartment_policy.receipt_program_id
    } else {
        liveness_id(state_account_id)
    };
    let state = RuntimeCompartmentV1::admit(
        policy,
        RuntimeCompartmentAdmissionV1 {
            kind,
            identity: RuntimeCompartmentIdentityV1 {
                policy_id: policy.policy_id,
                lifecycle_id: liveness_id(facility_id),
                account_id: clutch_liveness::Id::from_bytes(account.key.to_bytes()),
                owner: semantic_owner,
                payer: clutch_liveness::Id::from_bytes(payer.key.to_bytes()),
                neutral_sink: policy.neutral_sink,
                generation: 0,
            },
            funding: PresentFundingV1 {
                payer: clutch_liveness::Id::from_bytes(payer.key.to_bytes()),
                source: PresentFundingSourceV1::ExternalSignerNativeLamports,
                payer_debit_lamports: payer_debit,
                account_balance_before: balance_before,
                account_balance_after: balance_after,
            },
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((state, bump))
}

#[inline(never)]
fn dealer_body<T: FixedCodec>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    tag: u8,
    version: u8,
    account_bytes: usize,
) -> Outcome<(u8, T)> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_signer, ClutchError::MismatchedState)?;
    require(
        account.is_writable == writable,
        if writable {
            ClutchError::NotWritable
        } else {
            ClutchError::UnexpectedWritable
        },
    )?;
    require(
        account.data_len() == account_bytes,
        ClutchError::WrongDataLength,
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let (envelope, body) =
        decode_dealer_account_body_v1::<T>(&data, tag, version).map_err(dealer_fault)?;
    Ok((envelope.bump, body))
}

fn authenticate_state(program_id: &Pubkey, account: &AccountInfo<'_>) -> Outcome<DealerStateV2> {
    let (bump, state) = dealer_body::<DealerStateV2>(
        program_id,
        account,
        true,
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        DEALER_STATE_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_state_v2_pda(program_id, &state.facility_id.bytes()),
        Some(bump),
    )?;
    let floor = state
        .rent
        .refundable_live_principal
        .checked_add(state.rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(state.rent.donation_floor))
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok(state)
}

fn authenticate_dependency(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    facility_id: Id,
) -> Outcome<DealerFundedDependenciesV2> {
    let (bump, dependency) = dealer_body::<DealerFundedDependenciesV2>(
        program_id,
        account,
        false,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
    )?;
    expect_pda(
        account.key,
        seeds::dealer_funded_v2_pda(program_id, &facility_id.bytes()),
        Some(bump),
    )?;
    let floor = dependency
        .rent
        .refundable_principal
        .checked_add(dependency.rent.donation_floor)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        account.lamports() >= floor,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    Ok(dependency)
}

fn authenticate_schedule(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
) -> Outcome<DealerLivenessScheduleV1> {
    let (bump, schedule) = dealer_body::<DealerLivenessScheduleV1>(
        program_id,
        account,
        false,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_TAG,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_VERSION,
        DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES,
    )?;
    let schedule_id = schedule.schedule_id().map_err(dealer_fault)?.untyped();
    expect_pda(
        account.key,
        seeds::dealer_liveness_schedule_pda(program_id, &schedule_id.bytes()),
        Some(bump),
    )?;
    Ok(schedule)
}

#[inline(never)]
fn authenticate_position_and_replay(
    program_id: &Pubkey,
    state_account: &AccountInfo<'_>,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
    state: &DealerStateV2,
) -> Outcome<(
    FacilityPositionBindingV2,
    DealerPositionObservationV3,
    DealerFacilityReplayV1,
    DealerReplayAccountBindingV1,
)> {
    require(
        position_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        replay_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        !position_account.is_writable,
        ClutchError::UnexpectedWritable,
    )?;
    require(replay_account.is_writable, ClutchError::NotWritable)?;
    require(
        !position_account.executable && !replay_account.executable,
        ClutchError::ExecutableAccount,
    )?;
    require(
        position_account.data_len() == POSITION_V3_BYTES
            && replay_account.data_len()
                == clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let position = PositionAccountV3::decode(&position_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let binding = FacilityPositionBindingV2 {
        facility_id: state.facility_id,
        policy_id: state.policy_id,
        market_instance_v2_id: policy.market_instance_v2_id,
        collateral_policy_id: Id::from_bytes(position.collateral_policy_id().bytes()),
        collateral_release_id: Id::from_bytes(position.collateral_release_id().bytes()),
        dealer_state_account_id: id(state_account.key),
        initial_position_generation: 1,
    };
    let binding_id = binding.binding_id().map_err(dealer_fault)?;
    require(
        binding_id == state.facility_position_binding_id,
        ClutchError::MismatchedState,
    )?;
    let position_seeds = position.pda_seeds();
    expect_pda(
        position_account.key,
        seeds::position_v3_pda(
            program_id,
            &position_seeds.market_instance_id().bytes(),
            &position_seeds.owner().bytes(),
            position_seeds.purpose(),
            &position_seeds.purpose_binding_id().bytes(),
        ),
        Some(position_seeds.stored_bump()),
    )?;
    let replay =
        DealerFacilityReplayV1::decode(&replay_account.data.borrow()).map_err(dealer_fault)?;
    let replay_seeds = replay.pda_seeds();
    expect_pda(
        replay_account.key,
        seeds::purpose_replay_v3_pda(
            program_id,
            &replay_seeds.position_account().bytes(),
            replay_seeds.purpose(),
            &replay_seeds.purpose_binding_id().bytes(),
        ),
        Some(replay_seeds.stored_bump()),
    )?;
    require(
        position_account.key.to_bytes() == state.facility_position_account_id.bytes()
            && replay_account.key.to_bytes() == state.facility_replay_account_id.bytes()
            && position.replay_account().bytes() == replay_account.key.to_bytes()
            && position.purpose() == PositionPurposeV3::DealerFacility,
        ClutchError::MismatchedState,
    )?;
    let projection = project_dealer_position_v3(
        position,
        AdapterPositionMarketBindingV3 {
            market_instance_id: position.market_instance_id(),
            outcome_count: position.outcome_count(),
            realm_id: position.realm_id(),
            collateral_policy_id: position.collateral_policy_id(),
            collateral_release_id: position.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: retirement_id(state.facility_id)?,
            controller: retirement_id(binding.dealer_state_account_id)?,
            purpose_binding_id: retirement_id(binding_id)?,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let semantic_id = Id::from_bytes(
        position
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let observation = DealerPositionObservationV3 {
        account_id: id(position_account.key),
        semantic_id,
        projection,
    };
    observation
        .validate_current(state, &binding, policy)
        .map_err(dealer_fault)?;
    Ok((
        binding,
        observation,
        replay,
        DealerReplayAccountBindingV1 {
            replay_account_id: id(replay_account.key),
            position_replay_account_id: Id::from_bytes(position.replay_account().bytes()),
        },
    ))
}

#[inline(never)]
fn authenticate_runtime_bundle(
    program_id: &Pubkey,
    dependency: &DealerFundedDependenciesV2,
    policy_account: &AccountInfo<'_>,
    compartments: &[AccountInfo<'_>],
    writable_index: usize,
) -> Outcome<(
    RuntimeLivenessPolicyV1,
    [RuntimeCompartmentV1; RUNTIME_COMPARTMENT_COUNT_V1],
    DealerRuntimeLivenessBindingV1,
)> {
    require(
        dependency.bindings.runtime_liveness_program_id.bytes() == program_id.to_bytes(),
        ClutchError::WrongProgramOwner,
    )?;
    require(
        policy_account.key.to_bytes()
            == dependency
                .bindings
                .runtime_liveness_policy_account_id
                .bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        policy_account.owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(!policy_account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        policy_account.data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    let runtime_policy = RuntimeLivenessPolicyV1::decode(&policy_account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_policy_id = dealer_runtime_liveness_policy_id_v1(runtime_policy)
        .map_err(dealer_fault)?;
    expect_pda(
        policy_account.key,
        seeds::dealer_runtime_liveness_policy_pda(program_id, &runtime_policy_id.bytes()),
        None,
    )?;
    require(
        runtime_policy.policy_id.bytes() == dependency.bindings.runtime_liveness_policy_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        compartments.len() == RUNTIME_COMPARTMENT_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let first = RuntimeCompartmentV1::decode(&compartments[0].data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut states = [first; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut index = 0usize;
    while index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let account = &compartments[index];
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(
            !account.executable && !account.is_signer,
            ClutchError::MismatchedState,
        )?;
        require(
            account.is_writable == (index == writable_index),
            if index == writable_index {
                ClutchError::NotWritable
            } else {
                ClutchError::UnexpectedWritable
            },
        )?;
        require(
            account.data_len() == RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
            ClutchError::WrongDataLength,
        )?;
        let state = RuntimeCompartmentV1::decode(&account.data.borrow())
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            state.kind.index() == index
                && state.identity.account_id.bytes() == account.key.to_bytes()
                && account.lamports()
                    >= state
                        .expected_account_balance_lamports()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            ClutchError::MismatchedState,
        )?;
        states[index] = state;
        index += 1;
    }
    let binding = DealerRuntimeLivenessBindingV1::from_canonical(&runtime_policy, &states)
        .map_err(dealer_fault)?;
    Ok((runtime_policy, states, binding))
}

#[inline(never)]
fn authenticate_general_epoch(
    program_id: &Pubkey,
    epoch_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    domain_account: &AccountInfo<'_>,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<DealerGeneralEpochEvidenceV3> {
    for (account, length) in [
        (epoch_account, GENERAL_EPOCH_ACCOUNT_BYTES),
        (window_account, WINDOW_ACCOUNT_BYTES),
        (domain_account, ECONOMIC_DOMAIN_ACCOUNT_BYTES),
    ] {
        require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
        require(
            !account.executable && !account.is_signer,
            ClutchError::MismatchedState,
        )?;
        require(account.data_len() == length, ClutchError::WrongDataLength)?;
    }
    let epoch = GeneralEpochV6AccountV1::decode(&epoch_account.data.borrow())?;
    let window = CandidateWindowV4AccountV1::decode(&window_account.data.borrow())?;
    let domain = EconomicDomainV2AccountV1::decode(&domain_account.data.borrow())?;
    expect_pda(
        epoch_account.key,
        seeds::general_v2_epoch_pda(program_id, &epoch.market_binding.bytes(), epoch.epoch_index),
        Some(epoch.stored_bump),
    )?;
    expect_pda(
        window_account.key,
        seeds::general_v2_window_pda(program_id, &epoch_account.key.to_bytes()),
        Some(window.stored_bump),
    )?;
    expect_pda(
        domain_account.key,
        seeds::general_v2_economic_domain_pda(program_id, &epoch_account.key.to_bytes()),
        Some(domain.stored_bump),
    )?;
    DealerGeneralEpochEvidenceV3::new(
        id(epoch_account.key),
        epoch,
        id(window_account.key),
        window,
        id(domain_account.key),
        domain,
        policy,
    )
    .map_err(dealer_fault)
}

fn require_aliases(accounts: &[AccountInfo<'_>], allowed: (usize, usize)) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            if (left, right) != allowed {
                require(
                    accounts[left].key != accounts[right].key,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

fn require_initialize_aliases(accounts: &[AccountInfo<'_>]) -> Outcome<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            let identity_only_alias = (left, right) == (0, 18);
            if !identity_only_alias {
                require(
                    accounts[left].key != accounts[right].key,
                    ClutchError::AccountAlias,
                )?;
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

#[inline(never)]
fn authenticate_sponsor_position(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    actor: Id,
    policy: &clutch_dealer_runtime_contract::DealerPolicyV1,
) -> Outcome<(
    PositionAccountV3,
    clutch_retirement::GeneralPositionProjectionV3,
)> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(
        !account.executable && !account.is_signer,
        ClutchError::MismatchedState,
    )?;
    require(
        account.data_len() == POSITION_V3_BYTES,
        ClutchError::WrongDataLength,
    )?;
    let position = PositionAccountV3::decode(&account.data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let pda = position.pda_seeds();
    expect_pda(
        account.key,
        seeds::position_v3_pda(
            program_id,
            &pda.market_instance_id().bytes(),
            &pda.owner().bytes(),
            pda.purpose(),
            &pda.purpose_binding_id().bytes(),
        ),
        Some(pda.stored_bump()),
    )?;
    require(
        position.purpose() == PositionPurposeV3::General
            && position.lifecycle() == PositionLifecycleV3::Open
            && position.controller().bytes() == actor.bytes()
            && position.market_instance_id().bytes() == policy.market_instance_v2_id.bytes()
            && position.realm_id().bytes() == policy.realm_id.bytes()
            && position.outcome_count() == policy.outcome_count,
        ClutchError::MismatchedState,
    )?;
    let projection = project_general_position_v3(
        position,
        AdapterPositionMarketBindingV3 {
            market_instance_id: position.market_instance_id(),
            outcome_count: position.outcome_count(),
            realm_id: position.realm_id(),
            collateral_policy_id: position.collateral_policy_id(),
            collateral_release_id: position.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: position.owner(),
            controller: position.controller(),
            purpose_binding_id: position.purpose_binding_id(),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok((position, projection))
}

fn write_dealer_body<T: FixedCodec>(
    account: &AccountInfo<'_>,
    tag: u8,
    version: u8,
    bump: u8,
    body: &T,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    encode_dealer_account_body_v1(&mut data, tag, version, bump, body).map_err(dealer_fault)
}

fn apply_liveness_transition(
    compartment: &AccountInfo<'_>,
    actor: &AccountInfo<'_>,
    payer: &AccountInfo<'_>,
    transition: &clutch_liveness::runtime_adapter_v1::RuntimeAtomicTransitionV1,
) -> Outcome<()> {
    require(compartment.is_writable, ClutchError::NotWritable)?;
    require(
        actor.is_writable && payer.is_writable,
        ClutchError::NotWritable,
    )?;
    require(
        transition.account_id.bytes() == compartment.key.to_bytes()
            && transition.write_account_data
            && !transition.close_account,
        ClutchError::MismatchedState,
    )?;
    let mut actor_credit = 0u64;
    let mut payer_credit = 0u64;
    for movement in transition.transfers() {
        match movement.role {
            RuntimeTransferRoleV1::KeeperPayment => {
                require(
                    movement.destination.bytes() == actor.key.to_bytes(),
                    ClutchError::MismatchedState,
                )?;
                actor_credit = actor_credit
                    .checked_add(movement.lamports)
                    .ok_or(ClutchError::Arithmetic)?;
            }
            RuntimeTransferRoleV1::PayerWorkRefund => {
                require(
                    movement.destination.bytes() == payer.key.to_bytes(),
                    ClutchError::MismatchedState,
                )?;
                payer_credit = payer_credit
                    .checked_add(movement.lamports)
                    .ok_or(ClutchError::Arithmetic)?;
            }
            _ => return Err(ClutchError::MismatchedState.into()),
        }
    }
    **compartment
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? =
        transition.account_balance_after;
    if actor.key == payer.key {
        let credit = actor_credit
            .checked_add(payer_credit)
            .ok_or(ClutchError::Arithmetic)?;
        let after = actor
            .lamports()
            .checked_add(credit)
            .ok_or(ClutchError::Arithmetic)?;
        **actor
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = after;
    } else {
        let actor_after = actor
            .lamports()
            .checked_add(actor_credit)
            .ok_or(ClutchError::Arithmetic)?;
        let payer_after = payer
            .lamports()
            .checked_add(payer_credit)
            .ok_or(ClutchError::Arithmetic)?;
        **actor
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = actor_after;
        **payer
            .try_borrow_mut_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))? = payer_after;
    }
    compartment
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(&transition.post_account_data);
    Ok(())
}

#[inline(never)]
fn initialize_facility(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, INITIALIZE_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::Initialize, payload_bytes)
        .map_err(dealer_fault)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    require(!accounts[1].is_writable, ClutchError::UnexpectedWritable)?;
    require(
        !accounts[1].is_signer && !accounts[1].executable,
        ClutchError::MismatchedState,
    )?;
    require_initialize_aliases(accounts)?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[2])?;
    let (sponsor_position, sponsor_projection) =
        authenticate_sponsor_position(program_id, &accounts[3], id(accounts[0].key), &policy)?;
    let sponsor = Id::from_bytes(sponsor_position.owner().bytes());
    let genesis = DealerFacilityGenesisV1 {
        policy_id: Id::from_bytes(policy_id),
        sponsor,
        sponsor_refund_recipient: id(accounts[1].key),
        facility_nonce: payload.facility_nonce,
    };
    let facility_id = genesis
        .facility_id_for_policy(&policy)
        .map_err(dealer_fault)?
        .untyped();
    let (state_address, state_bump) = seeds::dealer_state_v2_pda(program_id, &facility_id.bytes());
    expect_pda(accounts[4].key, (state_address, state_bump), None)?;
    let binding = FacilityPositionBindingV2 {
        facility_id,
        policy_id: Id::from_bytes(policy_id),
        market_instance_v2_id: policy.market_instance_v2_id,
        collateral_policy_id: Id::from_bytes(sponsor_position.collateral_policy_id().bytes()),
        collateral_release_id: Id::from_bytes(sponsor_position.collateral_release_id().bytes()),
        dealer_state_account_id: id(accounts[4].key),
        initial_position_generation: 1,
    };
    let binding_id = binding
        .binding_id_for(&genesis, &policy)
        .map_err(dealer_fault)?;
    let (facility_position_address, facility_position_bump) = seeds::position_v3_pda(
        program_id,
        &policy.market_instance_v2_id.bytes(),
        &facility_id.bytes(),
        PositionPurposeV3::DealerFacility,
        &binding_id.bytes(),
    );
    expect_pda(
        accounts[5].key,
        (facility_position_address, facility_position_bump),
        None,
    )?;
    let (replay_address, replay_bump) = seeds::purpose_replay_v3_pda(
        program_id,
        &accounts[5].key.to_bytes(),
        PositionPurposeV3::DealerFacility,
        &binding_id.bytes(),
    );
    expect_pda(accounts[6].key, (replay_address, replay_bump), None)?;
    let (dependency_address, dependency_bump) =
        seeds::dealer_funded_v2_pda(program_id, &facility_id.bytes());
    expect_pda(accounts[7].key, (dependency_address, dependency_bump), None)?;
    for account in [
        &accounts[4],
        &accounts[5],
        &accounts[6],
        &accounts[7],
        &accounts[17],
    ] {
        require_creatable(account)?;
    }

    let schedule = authenticate_schedule(program_id, &accounts[8])?;
    require(
        schedule.schedule_id().map_err(dealer_fault)?.untyped() == policy.liveness_policy_id,
        ClutchError::MismatchedState,
    )?;
    require(
        accounts[9].owner == program_id,
        ClutchError::WrongProgramOwner,
    )?;
    require(
        accounts[9].data_len() == RUNTIME_LIVENESS_POLICY_BYTES_V1,
        ClutchError::WrongDataLength,
    )?;
    require(
        !accounts[9].is_writable && !accounts[9].is_signer && !accounts[9].executable,
        ClutchError::MismatchedState,
    )?;
    require_signer(&accounts[18])?;
    require(accounts[18].is_writable, ClutchError::NotWritable)?;
    let rent = read_rent(&accounts[20])?;
    require_system_program(&accounts[21])?;
    let runtime_policy = RuntimeLivenessPolicyV1::decode(&accounts[9].data.borrow())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let runtime_policy_id = dealer_runtime_liveness_policy_id_v1(runtime_policy)
        .map_err(dealer_fault)?;
    expect_pda(
        accounts[9].key,
        seeds::dealer_runtime_liveness_policy_pda(program_id, &runtime_policy_id.bytes()),
        None,
    )?;
    require(
        accounts[9].lamports() >= rent.minimum_balance(RUNTIME_LIVENESS_POLICY_BYTES_V1)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let runtime_account_rent = rent.minimum_balance(RUNTIME_LIVENESS_ACCOUNT_BYTES_V1)?;
    let (first_runtime_state, first_runtime_bump) = prepare_runtime_compartment_admission(
        program_id,
        facility_id,
        id(accounts[4].key),
        &accounts[18],
        &accounts[10],
        runtime_policy,
        RUNTIME_COMPARTMENT_ORDER_V1[0],
        runtime_account_rent,
    )?;
    let mut runtime_states = [first_runtime_state; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut runtime_bumps = [first_runtime_bump; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut total_runtime_debit = runtime_policy
        .compartment(RUNTIME_COMPARTMENT_ORDER_V1[0])
        .total_payer_debit_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut runtime_index = 1usize;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let kind = RUNTIME_COMPARTMENT_ORDER_V1[runtime_index];
        let (state, bump) = prepare_runtime_compartment_admission(
            program_id,
            facility_id,
            id(accounts[4].key),
            &accounts[18],
            &accounts[10 + runtime_index],
            runtime_policy,
            kind,
            runtime_account_rent,
        )?;
        runtime_states[runtime_index] = state;
        runtime_bumps[runtime_index] = bump;
        total_runtime_debit = total_runtime_debit
            .checked_add(
                runtime_policy
                    .compartment(kind)
                    .total_payer_debit_lamports()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
            )
            .ok_or(ClutchError::Arithmetic)?;
        runtime_index += 1;
    }
    let runtime_bundle = RuntimeLivenessBundleV1 {
        policy_id: runtime_policy.policy_id,
        lifecycle_id: liveness_id(facility_id),
        compartments: runtime_states,
    };
    runtime_bundle
        .validate(runtime_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        total_runtime_debit
            == runtime_policy
                .total_payer_debit_lamports()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;
    let runtime_binding = DealerRuntimeLivenessBindingV1::from_canonical(
        &runtime_policy,
        &runtime_states,
    )
    .map_err(dealer_fault)?;
    require(
        runtime_binding.realm_id() == policy.realm_id
            && runtime_binding.lifecycle_id() == facility_id
            && runtime_binding.neutral_sink() == policy.neutral_sink,
        ClutchError::MismatchedState,
    )?;
    runtime_index = 1;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let compartment = match runtime_index {
            1 => DealerLivenessCompartmentV1::Candidate,
            2 => DealerLivenessCompartmentV1::Clearing,
            3 => DealerLivenessCompartmentV1::Settlement,
            4 => DealerLivenessCompartmentV1::Resolution,
            5 => DealerLivenessCompartmentV1::Retirement,
            6 => DealerLivenessCompartmentV1::Recovery,
            _ => return Err(ClutchError::MismatchedState.into()),
        };
        require(
            runtime_binding.owner(compartment) == id(accounts[4].key)
                && runtime_binding.receipt_program_id(compartment) == id(program_id),
            ClutchError::MismatchedState,
        )?;
        runtime_index += 1;
    }

    require(
        accounts[8].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    let state_principal = rent.minimum_balance(DEALER_STATE_V2_ACCOUNT_BYTES)?;
    let state_permanent = rent.minimum_balance(DEALER_ROOT_TOMBSTONE_V2_ACCOUNT_BYTES)?;
    let state_refundable = state_principal
        .checked_sub(state_permanent)
        .ok_or(ClutchError::Arithmetic)?;
    let position_principal = rent.minimum_balance(POSITION_V3_BYTES)?;
    let position_permanent = rent.minimum_balance(POSITION_TOMBSTONE_V3_BYTES)?;
    let position_refundable = position_principal
        .checked_sub(position_permanent)
        .ok_or(ClutchError::Arithmetic)?;
    let replay_principal =
        rent.minimum_balance(clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1)?;
    let dependency_principal = rent.minimum_balance(DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES)?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;

    let facility_position_pre = PositionAccountV3::new(PositionV3Fields {
        purpose: PositionPurposeV3::DealerFacility,
        lifecycle: PositionLifecycleV3::Open,
        outcome_count: policy.outcome_count,
        stored_bump: facility_position_bump,
        generation: 1,
        market_instance_id: retirement_id(policy.market_instance_v2_id)?,
        realm_id: retirement_id(policy.realm_id)?,
        collateral_policy_id: sponsor_position.collateral_policy_id(),
        collateral_release_id: sponsor_position.collateral_release_id(),
        owner: retirement_id(facility_id)?,
        controller: retirement_id(id(accounts[4].key))?,
        replay_account: retirement_id(id(accounts[6].key))?,
        purpose_binding_id: retirement_id(binding_id)?,
        cash_atoms: 0,
        reserved_cash_atoms: 0,
        native_eggs: [0; clutch_retirement::MAX_OUTCOMES],
        outstanding_reservations: 0,
        rent: RentSplitV2 {
            payer: retirement_id(id(accounts[0].key))?,
            refundable_live_principal: position_refundable,
            permanent_tombstone_principal: position_permanent,
            donation_floor: accounts[5].lamports(),
        },
    })
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facility_projection_pre = project_dealer_position_v3(
        facility_position_pre,
        AdapterPositionMarketBindingV3 {
            market_instance_id: facility_position_pre.market_instance_id(),
            outcome_count: facility_position_pre.outcome_count(),
            realm_id: facility_position_pre.realm_id(),
            collateral_policy_id: facility_position_pre.collateral_policy_id(),
            collateral_release_id: facility_position_pre.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: facility_position_pre.owner(),
            controller: facility_position_pre.controller(),
            purpose_binding_id: facility_position_pre.purpose_binding_id(),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market = DealerPositionMarketJoinV1 {
        market_instance_v2_id: policy.market_instance_v2_id,
        realm_id: policy.realm_id,
        collateral_policy_id: binding.collateral_policy_id,
        collateral_release_id: binding.collateral_release_id,
        outcome_count: policy.outcome_count,
    };
    let transfer = prepare_dealer_sponsor_funding_transfer_v1(
        market,
        sponsor,
        payload.sponsor_capital_atoms,
        DealerTransferPositionV3::General {
            account_id: id(accounts[3].key),
            position: sponsor_projection,
        },
        DealerTransferPositionV3::Facility {
            account_id: id(accounts[5].key),
            position: facility_projection_pre,
        },
    )
    .map_err(dealer_fault)?;
    let facility_position_post = transfer.destination_post();
    let facility_projection_post = project_dealer_position_v3(
        facility_position_post,
        AdapterPositionMarketBindingV3 {
            market_instance_id: facility_position_post.market_instance_id(),
            outcome_count: facility_position_post.outcome_count(),
            realm_id: facility_position_post.realm_id(),
            collateral_policy_id: facility_position_post.collateral_policy_id(),
            collateral_release_id: facility_position_post.collateral_release_id(),
        },
        AdapterPositionPurposeBindingV3 {
            owner: facility_position_post.owner(),
            controller: facility_position_post.controller(),
            purpose_binding_id: facility_position_post.purpose_binding_id(),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let facility_semantic_id = Id::from_bytes(
        facility_position_post
            .semantic_id(&RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let facility_observation = DealerPositionObservationV3 {
        account_id: id(accounts[5].key),
        semantic_id: facility_semantic_id,
        projection: facility_projection_post,
    };

    let clearing = runtime_states[DealerLivenessCompartmentV1::Clearing.index()];
    require(
        clearing.identity.payer.bytes() == accounts[18].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let receipt = DealerActionReceiptV1 {
        policy_id: Id::from_bytes(policy_id),
        facility_id,
        dealer_state_account_id: id(accounts[4].key),
        liveness_schedule_id: policy.liveness_policy_id,
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Clearing),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Clearing),
        quote_schedule_id: runtime_binding.quote_schedule_id(DealerLivenessCompartmentV1::Clearing),
        receipt_account_id: id(accounts[17].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[6].key),
        action: DealerRuntimeActionV1::Initialize,
        compartment: DealerLivenessCompartmentV1::Clearing,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Clearing),
        facility_generation: 1,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[DealerRuntimeActionV1::Initialize as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: 0,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[17].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[17].key, (receipt_address, receipt_bump), None)?;

    let dependency = DealerFundedDependenciesV2 {
        bindings: DealerFundedBudgetDependenciesV1 {
            policy_id: Id::from_bytes(policy_id),
            facility_id,
            liveness_schedule_id: policy.liveness_policy_id,
            runtime_liveness_policy_id: runtime_binding.runtime_policy_id(),
            runtime_liveness_program_id: id(program_id),
            runtime_liveness_policy_account_id: id(accounts[9].key),
            runtime_liveness_binding_digest: runtime_binding
                .binding_digest()
                .map_err(dealer_fault)?,
            fee_policy_id: policy.fee_policy_id,
            collateral_mint: policy.collateral_mint,
            token_program: policy.token_program,
            asset_vault_authority_account_id: id(accounts[4].key),
            neutral_sink: policy.neutral_sink,
            counted_generation: 0,
            dealer_liveness_work_principal_lamports: schedule
                .dealer_runtime_work_principal_lamports()
                .map_err(dealer_fault)?,
        },
        facility_position_binding_id: binding_id,
        initialize_receipt_account_id: id(accounts[17].key),
        initialize_receipt_semantic_id: receipt.semantic_receipt_id().map_err(dealer_fault)?,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: dependency_principal,
            donation_floor: accounts[7].lamports(),
        },
    };
    let state = DealerStateV2 {
        policy_id: Id::from_bytes(policy_id),
        facility_id,
        facility_position_binding_id: binding_id,
        facility_position_id: facility_semantic_id,
        facility_position_account_id: id(accounts[5].key),
        facility_replay_account_id: id(accounts[6].key),
        sponsor,
        sponsor_refund_recipient: id(accounts[1].key),
        lp_page_head_id: Id::ZERO,
        lp_page_set_root: Id::ZERO,
        last_lp_owner: Id::ZERO,
        active_epoch_id: Id::ZERO,
        active_epoch_binding_account_id: Id::ZERO,
        active_lease_id: Id::ZERO,
        funded_dependencies_id: dependency.dependency_id().map_err(dealer_fault)?,
        funded_dependencies_account_id: id(accounts[7].key),
        terminal_position_tombstone_id: Id::ZERO,
        terminal_replay_semantic_id: Id::ZERO,
        terminal_replay_intent_id: Id::ZERO,
        terminal_state_receipt_id: Id::ZERO,
        phase: DealerPhaseV2::Funding,
        sponsor_capital_disposition: SponsorCapitalDispositionV1::Refundable,
        outcome_count: policy.outcome_count,
        generation: 1,
        child_sequence: 0,
        total_shares: 0,
        queued_shares: 0,
        terminal_claimed_shares: 0,
        sponsor_capital_atoms: payload.sponsor_capital_atoms,
        net_sold: [0; clutch_dealer_runtime_contract::MAX_OUTCOMES],
        children: DealerChildCountsV2 {
            facility_positions: 1,
            facility_replays: 1,
            funded_dependencies: 1,
            ..DealerChildCountsV2::default()
        },
        rent: RootRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_live_principal: state_refundable,
            permanent_tombstone_principal: state_permanent,
            donation_floor: accounts[4].lamports(),
        },
    };
    state
        .validate_against_policy(&policy)
        .map_err(dealer_fault)?;
    let replay = DealerFacilityReplayV1::founding(
        id(accounts[5].key),
        id(accounts[6].key),
        binding_id,
        1,
        replay_bump,
        ReplayRentOwnerV1::from_persisted(
            retirement_id(id(accounts[0].key))?,
            replay_principal,
            accounts[6].lamports(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
    )
    .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &clearing)
        .map_err(dealer_fault)?;
    let liveness_intent = receipt.runtime_transition_intent().map_err(dealer_fault)?;
    let liveness_observation = receipt
        .runtime_receipt_observation()
        .map_err(dealer_fault)?;
    let clearing_before_balance = clearing
        .expected_account_balance_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let clearing_after_balance = clearing_before_balance
        .checked_sub(receipt.call_ceiling_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let mut clearing_pre_data = [0u8; RUNTIME_LIVENESS_ACCOUNT_BYTES_V1];
    clearing
        .encode(&mut clearing_pre_data)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let liveness_transition = plan_runtime_transition_v1(
        clutch_liveness::Id::from_bytes(program_id.to_bytes()),
        clutch_liveness::Id::from_bytes(accounts[9].key.to_bytes()),
        RuntimePersistedAccountViewV1 {
            account_id: clutch_liveness::Id::from_bytes(accounts[9].key.to_bytes()),
            owner_program_id: clutch_liveness::Id::from_bytes(program_id.to_bytes()),
            lamports: accounts[9].lamports(),
            data: &accounts[9].data.borrow(),
            writable: false,
        },
        RuntimePersistedAccountViewV1 {
            account_id: clutch_liveness::Id::from_bytes(accounts[12].key.to_bytes()),
            owner_program_id: clutch_liveness::Id::from_bytes(program_id.to_bytes()),
            lamports: clearing_before_balance,
            data: &clearing_pre_data,
            writable: true,
        },
        liveness_intent,
        Some(liveness_observation),
        clearing_after_balance,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        runtime_policy.policy_id.bytes() == dependency.bindings.runtime_liveness_policy_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    let replay_binding = DealerReplayAccountBindingV1 {
        replay_account_id: id(accounts[6].key),
        position_replay_account_id: Id::from_bytes(facility_position_post.replay_account().bytes()),
    };
    let prepared = prepare_facility_initialization_v3(
        &genesis,
        &binding,
        &policy,
        &schedule,
        &runtime_binding,
        id(accounts[4].key),
        id(accounts[7].key),
        &dependency,
        &authorization,
        &facility_observation,
        &state,
        transfer,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;
    let current_slot = read_clock_slot(&accounts[19])?;
    require(
        current_slot < policy.funding_deadline_slot,
        ClutchError::MismatchedState,
    )?;

    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[4],
        &accounts[21],
        &rent,
        DEALER_STATE_V2_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_STATE_V2,
            &facility_id.bytes(),
            &[state_bump],
        ],
    )?;
    let purpose_seed = [u8::from(PositionPurposeV3::DealerFacility)];
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[5],
        &accounts[21],
        &rent,
        POSITION_V3_BYTES,
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            &policy.market_instance_v2_id.bytes(),
            &facility_id.bytes(),
            &purpose_seed,
            &binding_id.bytes(),
            &[facility_position_bump],
        ],
    )?;
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[6],
        &accounts[21],
        &rent,
        clutch_dealer_runtime_contract::DEALER_FACILITY_REPLAY_BYTES_V1,
        &[
            clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
            &accounts[5].key.to_bytes(),
            &purpose_seed,
            &binding_id.bytes(),
            &[replay_bump],
        ],
    )?;
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[7],
        &accounts[21],
        &rent,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_FUNDED_V2,
            &facility_id.bytes(),
            &[dependency_bump],
        ],
    )?;
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[17],
        &accounts[21],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    runtime_index = 0;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let kind = RUNTIME_COMPARTMENT_ORDER_V1[runtime_index];
        let kind_seed = [runtime_kind_seed(kind)];
        let payer_debit = runtime_policy
            .compartment(kind)
            .total_payer_debit_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let donation = create_exact_payer_debit_pda(
            program_id,
            &accounts[18],
            &accounts[10 + runtime_index],
            &accounts[21],
            payer_debit,
            RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
            &[
                seeds::SEED_DEALER_RUNTIME_LIVENESS_ACCOUNT,
                &facility_id.bytes(),
                &kind_seed,
                &[runtime_bumps[runtime_index]],
            ],
        )?;
        require(
            donation == runtime_states[runtime_index].donation_received_lamports,
            ClutchError::MismatchedState,
        )?;
        let mut runtime_data = accounts[10 + runtime_index]
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        runtime_states[runtime_index]
            .encode(&mut runtime_data[..])
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        runtime_index += 1;
    }
    apply_liveness_transition(
        &accounts[12],
        &accounts[0],
        &accounts[18],
        &liveness_transition,
    )?;
    accounts[3]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &prepared
                .transfer
                .source_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    accounts[5]
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?
        .copy_from_slice(
            &prepared
                .transfer
                .destination_post()
                .encode()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        );
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[6].data.borrow_mut())
        .map_err(dealer_fault)?;
    write_dealer_body(
        &accounts[7],
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_TAG,
        DEALER_FUNDED_DEPENDENCIES_V2_ACCOUNT_VERSION,
        dependency_bump,
        &dependency,
    )?;
    write_dealer_body(
        &accounts[17],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    write_dealer_body(
        &accounts[4],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state,
    )
}

#[inline(never)]
fn bind_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    payload_bytes: &[u8],
) -> Outcome<()> {
    require_count(accounts, BIND_EPOCH_ACCOUNT_COUNT)?;
    let payload = DealerRuntimePayloadV1::decode(DealerFacilityAction::BindEpoch, payload_bytes)
        .map_err(dealer_fault)?;
    require(
        sequence == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    require_signer(&accounts[0])?;
    require(accounts[0].is_writable, ClutchError::NotWritable)?;
    // Actor and the immutable compartment payer may intentionally be the same
    // writable system account. Every semantic/state role remains disjoint.
    require_aliases(accounts, (0, 16))?;

    let (policy_id, policy) = authenticate_catalog_policy(program_id, &accounts[1])?;
    let state = authenticate_state(program_id, &accounts[2])?;
    require(
        state.policy_id.bytes() == policy_id && state.generation == payload.expected_generation,
        ClutchError::MismatchedState,
    )?;
    let (binding, _position, replay, replay_binding) = authenticate_position_and_replay(
        program_id,
        &accounts[2],
        &accounts[3],
        &accounts[4],
        &policy,
        &state,
    )?;
    require(
        replay.next_transition_ordinal() == payload.expected_replay_ordinal,
        ClutchError::Replay,
    )?;
    let dependency = authenticate_dependency(program_id, &accounts[5], state.facility_id)?;
    let schedule = authenticate_schedule(program_id, &accounts[6])?;
    let (runtime_policy, runtime_states, runtime_binding) = authenticate_runtime_bundle(
        program_id,
        &dependency,
        &accounts[7],
        &accounts[8..15],
        DealerLivenessCompartmentV1::Candidate.index(),
    )?;
    require(
        dependency.facility_position_binding_id == binding.binding_id().map_err(dealer_fault)?
            && dependency.bindings.runtime_liveness_binding_digest
                == runtime_binding.binding_digest().map_err(dealer_fault)?
            && dependency.bindings.policy_id == state.policy_id
            && dependency.bindings.facility_id == state.facility_id
            && dependency.bindings.liveness_schedule_id
                == schedule.schedule_id().map_err(dealer_fault)?.untyped()
            && dependency.bindings.liveness_schedule_id == policy.liveness_policy_id
            && dependency.bindings.runtime_liveness_policy_id
                == runtime_binding.runtime_policy_id()
            && runtime_binding.realm_id() == policy.realm_id
            && runtime_binding.lifecycle_id() == state.facility_id
            && runtime_binding.neutral_sink() == policy.neutral_sink
            && dependency.bindings.fee_policy_id == policy.fee_policy_id
            && dependency.bindings.collateral_mint == policy.collateral_mint
            && dependency.bindings.token_program == policy.token_program
            && dependency.bindings.asset_vault_authority_account_id == id(accounts[2].key)
            && dependency.bindings.neutral_sink == policy.neutral_sink
            && dependency.bindings.dealer_liveness_work_principal_lamports
                == schedule
                    .dealer_runtime_work_principal_lamports()
                    .map_err(dealer_fault)?,
        ClutchError::MismatchedState,
    )?;
    let mut runtime_index = 1usize;
    while runtime_index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let compartment = match runtime_index {
            1 => DealerLivenessCompartmentV1::Candidate,
            2 => DealerLivenessCompartmentV1::Clearing,
            3 => DealerLivenessCompartmentV1::Settlement,
            4 => DealerLivenessCompartmentV1::Resolution,
            5 => DealerLivenessCompartmentV1::Retirement,
            6 => DealerLivenessCompartmentV1::Recovery,
            _ => return Err(ClutchError::MismatchedState.into()),
        };
        require(
            runtime_binding.owner(compartment) == id(accounts[2].key)
                && runtime_binding.receipt_program_id(compartment) == id(program_id),
            ClutchError::MismatchedState,
        )?;
        runtime_index += 1;
    }
    let candidate = runtime_states[DealerLivenessCompartmentV1::Candidate.index()];
    require(
        candidate.identity.payer.bytes() == accounts[16].key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let rent = read_rent(&accounts[22])?;
    require(
        accounts[6].lamports() >= rent.minimum_balance(DEALER_LIVENESS_SCHEDULE_ACCOUNT_BYTES)?,
        ClutchError::DealerPolicyRentMismatch,
    )?;
    require_system_program(&accounts[23])?;
    require_creatable(&accounts[15])?;
    require_creatable(&accounts[17])?;
    let receipt_principal = rent.minimum_balance(DEALER_ACTION_RECEIPT_ACCOUNT_BYTES)?;
    let receipt = DealerActionReceiptV1 {
        policy_id: state.policy_id,
        facility_id: state.facility_id,
        dealer_state_account_id: id(accounts[2].key),
        liveness_schedule_id: schedule.schedule_id().map_err(dealer_fault)?.untyped(),
        runtime_policy_id: runtime_binding.runtime_policy_id(),
        runtime_account_id: runtime_binding.account_id(DealerLivenessCompartmentV1::Candidate),
        runtime_owner: runtime_binding.owner(DealerLivenessCompartmentV1::Candidate),
        quote_schedule_id: runtime_binding
            .quote_schedule_id(DealerLivenessCompartmentV1::Candidate),
        receipt_account_id: id(accounts[15].key),
        receipt_program_id: id(program_id),
        keeper: id(accounts[0].key),
        replay_account_id: id(accounts[4].key),
        action: DealerRuntimeActionV1::BindEpoch,
        compartment: DealerLivenessCompartmentV1::Candidate,
        runtime_generation: runtime_binding.generation(DealerLivenessCompartmentV1::Candidate),
        facility_generation: state.generation,
        call_ordinal: payload.liveness_call_ordinal,
        call_ceiling_lamports: schedule.reward_lamports[DealerRuntimeActionV1::BindEpoch as usize],
        keeper_payment_lamports: payload.keeper_payment_lamports,
        expected_replay_ordinal: payload.expected_replay_ordinal,
        rent: DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: receipt_principal,
            donation_floor: accounts[15].lamports(),
        },
    };
    let receipt_slot = receipt.receipt_slot_id().map_err(dealer_fault)?;
    let (receipt_address, receipt_bump) =
        seeds::dealer_action_receipt_pda(program_id, &receipt_slot.bytes());
    expect_pda(accounts[15].key, (receipt_address, receipt_bump), None)?;
    receipt
        .validate_against(&schedule, &runtime_binding)
        .map_err(dealer_fault)?;
    let authorization = receipt
        .authorization(&schedule, &runtime_binding, &candidate)
        .map_err(dealer_fault)?;
    let intent = receipt.runtime_transition_intent().map_err(dealer_fault)?;
    let observation = receipt
        .runtime_receipt_observation()
        .map_err(dealer_fault)?;
    let candidate_after_balance = accounts[9]
        .lamports()
        .checked_sub(receipt.call_ceiling_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    let liveness_transition = plan_runtime_transition_v1(
        clutch_liveness::Id::from_bytes(program_id.to_bytes()),
        clutch_liveness::Id::from_bytes(accounts[7].key.to_bytes()),
        RuntimePersistedAccountViewV1 {
            account_id: clutch_liveness::Id::from_bytes(accounts[7].key.to_bytes()),
            owner_program_id: clutch_liveness::Id::from_bytes(program_id.to_bytes()),
            lamports: accounts[7].lamports(),
            data: &accounts[7].data.borrow(),
            writable: false,
        },
        RuntimePersistedAccountViewV1 {
            account_id: clutch_liveness::Id::from_bytes(accounts[9].key.to_bytes()),
            owner_program_id: clutch_liveness::Id::from_bytes(program_id.to_bytes()),
            lamports: accounts[9].lamports(),
            data: &accounts[9].data.borrow(),
            writable: true,
        },
        intent,
        Some(observation),
        candidate_after_balance,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        runtime_policy.policy_id.bytes() == dependency.bindings.runtime_liveness_policy_id.bytes(),
        ClutchError::MismatchedState,
    )?;

    let general = authenticate_general_epoch(
        program_id,
        &accounts[18],
        &accounts[19],
        &accounts[20],
        &policy,
    )?;
    let current_slot = read_clock_slot(&accounts[21])?;
    let (epoch_address, epoch_bump) =
        seeds::dealer_epoch_v2_pda(program_id, &state.facility_id.bytes(), state.generation);
    expect_pda(accounts[17].key, (epoch_address, epoch_bump), None)?;
    let epoch_principal = rent.minimum_balance(DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES)?;
    let epoch = DealerEpochBindingV2::new_bound(
        &policy,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        &authorization,
        &general,
        id(accounts[17].key),
        current_slot,
        DeletableRentOwnerV1 {
            payer: id(accounts[0].key),
            neutral_sink: policy.neutral_sink,
            refundable_principal: epoch_principal,
            donation_floor: accounts[17].lamports(),
        },
    )
    .map_err(dealer_fault)?;
    let prepared = prepare_bind_epoch_v3(
        &policy,
        &state,
        id(accounts[2].key),
        &dependency,
        &schedule,
        &runtime_binding,
        &authorization,
        &epoch,
        &general,
        &replay,
        replay_binding,
    )
    .map_err(dealer_fault)?;

    let generation_bytes = state.generation.to_le_bytes();
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[15],
        &accounts[23],
        &rent,
        DEALER_ACTION_RECEIPT_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_ACTION_RECEIPT,
            &receipt_slot.bytes(),
            &[receipt_bump],
        ],
    )?;
    create_full_principal_pda(
        program_id,
        &accounts[0],
        &accounts[17],
        &accounts[23],
        &rent,
        DEALER_EPOCH_BINDING_V2_ACCOUNT_BYTES,
        &[
            seeds::SEED_DEALER_EPOCH_V2,
            &state.facility_id.bytes(),
            &generation_bytes,
            &[epoch_bump],
        ],
    )?;
    apply_liveness_transition(
        &accounts[9],
        &accounts[0],
        &accounts[16],
        &liveness_transition,
    )?;
    write_dealer_body(
        &accounts[15],
        DEALER_ACTION_RECEIPT_ACCOUNT_TAG,
        DEALER_ACTION_RECEIPT_ACCOUNT_VERSION,
        receipt_bump,
        &receipt,
    )?;
    write_dealer_body(
        &accounts[17],
        DEALER_EPOCH_BINDING_V2_ACCOUNT_TAG,
        DEALER_EPOCH_BINDING_V2_ACCOUNT_VERSION,
        epoch_bump,
        &epoch,
    )?;
    let state_bump = accounts[2].data.borrow()[2];
    write_dealer_body(
        &accounts[2],
        DEALER_STATE_V2_ACCOUNT_TAG,
        DEALER_STATE_V2_ACCOUNT_VERSION,
        state_bump,
        &prepared.state_after,
    )?;
    prepared
        .replay
        .replay_post()
        .encode_into(&mut accounts[4].data.borrow_mut())
        .map_err(dealer_fault)
}

/// Execute one facility action admitted by the non-production profile.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    sequence: u64,
    action: DealerFacilityAction,
    payload: &[u8],
) -> Outcome<()> {
    match action {
        DealerFacilityAction::Initialize => {
            initialize_facility(program_id, accounts, sequence, payload)
        }
        DealerFacilityAction::BindEpoch => bind_epoch(program_id, accounts, sequence, payload),
        _ => super::dealer_runtime::process_reserved_disabled(action),
    }
}
