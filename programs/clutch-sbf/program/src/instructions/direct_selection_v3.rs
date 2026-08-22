//! Complete Direct V3 lifecycle adapter.
//!
//! Every V3 lifecycle intent has exactly one handler here: Init and the
//! private `freeze_abort`, `staged`, and `terminal` submodules cover tags 36
//! through 46 with no unimplemented arm, so [`crate::dispatch`] can route the
//! family all-or-nothing. Page creation and funded Reservation V2 placement
//! remain branches of the existing `InitOrderPage`/`PlaceOrder` wires,
//! selected only by the 672-byte V4 Epoch account that exists only through
//! the routed `InitDirectEpochV4`.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_creatable, require_system_program,
    transfer_data, RentParameters, MAX_PERMITTED_DATA_INCREASE, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_batch_policy_identity::{batch_policy_digest, direct_window_v1::DIRECT_POLICY_V1};
use clutch_solana_layout::{
    account_len,
    direct_selection::DirectEpochV3Account,
    direct_selection_v3::{
        DirectBatchPolicyV3, DirectEpochV4Account, DirectFundingLedgerV3, DirectTerminalReceiptV3,
        DirectV3Intent, DIRECT_BATCH_POLICY_V3_BYTES, DIRECT_EPOCH_V4_BYTES,
        DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN,
    },
    Hash32,
};
use clutch_solana_reference::DirectV3Request;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::incinerator;

mod common;
mod freeze_abort;
mod staged;
mod terminal;

/// Domain label whose SHA-256 bytes define this semantic release identifier.
///
/// The identifier is a version label owned by this exact verifier adapter. It
/// is deliberately **not** described as an ELF, ProgramData, source, or code
/// hash. Upgradeable deployment trust remains a separate release boundary.
pub const DIRECT_VERIFIER_RELEASE_LABEL_V3: &str = "dragons-clutch/direct-verifier-release/v3/1";

/// Compile-time Direct V3 verifier release identifier.
pub const DIRECT_VERIFIER_RELEASE_ID_V3: Hash32 = Hash32::from_bytes([
    0x03, 0x89, 0x14, 0xd7, 0x91, 0x30, 0x57, 0x58, 0x9f, 0xa6, 0xbf, 0x30, 0x3f, 0x02, 0xa6, 0xb9,
    0xb5, 0xe1, 0x2a, 0x1e, 0xe7, 0x18, 0x56, 0x10, 0x79, 0xab, 0x57, 0xd3, 0x75, 0x94, 0x77, 0x04,
]);

/// The only neutral-lamport sink admitted by the first Direct V3 runtime.
///
/// The current Realm layout has no neutral-sink field. Specializing the
/// already-general model to Solana's canonical incinerator avoids accepting a
/// creator-selected donation beneficiary and introduces no second authority.
pub const DIRECT_NEUTRAL_SINK_V3: Pubkey = incinerator::ID;

const INIT_ACCOUNT_COUNT: usize = 9;
const IX_PAYER: usize = 0;
const IX_EPOCH: usize = 1;
const IX_MARKET: usize = 2;
const IX_TERMS: usize = 3;
const IX_GRID: usize = 4;
const IX_POLICY: usize = 5;
const IX_SYSTEM: usize = 6;
const IX_RENT: usize = 7;
const IX_CLOCK: usize = 8;

const INIT_STATE_ROLES: [StateRole; 4] = [
    StateRole::read_only(IX_MARKET, account_len::MARKET),
    StateRole::read_only(IX_TERMS, account_len::TERMS),
    StateRole::read_only(IX_GRID, account_len::PRICE_GRID),
    StateRole::read_only(IX_POLICY, DIRECT_BATCH_POLICY_V3_BYTES),
];

macro_rules! borrow_mut {
    ($account:expr) => {
        $account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))
    };
}

/// Route one already-decoded Direct V3 lifecycle request.
///
/// The match is exhaustive with no fallback arm: a new lifecycle intent
/// cannot compile without a handler, which is what lets [`crate::dispatch`]
/// admit tags 36 through 46 as one all-or-nothing family.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    request: &DirectV3Request,
) -> Outcome<()> {
    match request.intent {
        DirectV3Intent::InitEpoch {
            market,
            epoch_index,
            policy,
            submission_opens_slot,
            submission_closes_slot,
            selection_deadline_slot,
            settlement_deadline_slot,
            neutral_lamport_sink,
        } => init_epoch(
            program_id,
            accounts,
            request.sequence,
            InitEpochV4 {
                market,
                epoch_index,
                policy,
                submission_opens_slot,
                submission_closes_slot,
                selection_deadline_slot,
                settlement_deadline_slot,
                neutral_lamport_sink,
            },
        ),
        DirectV3Intent::FreezeEpoch {
            market,
            epoch,
            reward_deposit,
            rewards,
        } => freeze_abort::freeze(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            reward_deposit,
            rewards,
        ),
        DirectV3Intent::AbortUnfrozen { market, epoch } => {
            freeze_abort::abort(program_id, accounts, request.sequence, market, epoch)
        }
        DirectV3Intent::SubmitCandidate {
            market,
            epoch,
            outcome_price,
        } => staged::submit(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            outcome_price,
        ),
        DirectV3Intent::BeginVerification { market, epoch } => {
            staged::begin(program_id, accounts, request.sequence, market, epoch)
        }
        DirectV3Intent::VerifyCandidate {
            market,
            epoch,
            retained_index,
        } => staged::verify(
            program_id,
            accounts,
            request.sequence,
            market,
            epoch,
            retained_index,
        ),
        DirectV3Intent::FinalizeSelection { market, epoch } => {
            terminal::finalize(program_id, accounts, request.sequence, market, epoch)
        }
        DirectV3Intent::Settle { market, epoch } => {
            terminal::settle(program_id, accounts, request.sequence, market, epoch)
        }
        DirectV3Intent::LapseEmpty { market, epoch } => {
            terminal::lapse_empty(program_id, accounts, request.sequence, market, epoch)
        }
        DirectV3Intent::LapseUnselected { market, epoch } => {
            terminal::lapse_unselected(program_id, accounts, request.sequence, market, epoch)
        }
        DirectV3Intent::LapseSelected { market, epoch } => {
            terminal::lapse_selected(program_id, accounts, request.sequence, market, epoch)
        }
    }
}

#[derive(Clone, Copy)]
struct InitEpochV4 {
    market: Hash32,
    epoch_index: u64,
    policy: Hash32,
    submission_opens_slot: u64,
    submission_closes_slot: u64,
    selection_deadline_slot: u64,
    settlement_deadline_slot: u64,
    neutral_lamport_sink: Hash32,
}

#[inline(never)]
fn init_epoch(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    sequence: u64,
    intent: InitEpochV4,
) -> Outcome<()> {
    require_count(accounts, INIT_ACCOUNT_COUNT)?;
    require(sequence == 0, ClutchError::Replay)?;
    require_signer(&accounts[IX_PAYER])?;
    require(accounts[IX_PAYER].is_writable, ClutchError::NotWritable)?;
    require_distinct(accounts)?;
    accounts::validate_state_roles(program_id, accounts, &INIT_STATE_ROLES)?;
    require_creatable(&accounts[IX_EPOCH])?;
    require_system_program(&accounts[IX_SYSTEM])?;
    let rent = read_rent(&accounts[IX_RENT])?;
    let now = read_clock_slot(&accounts[IX_CLOCK])?;
    require(intent.submission_opens_slot > now, ClutchError::NotActive)?;

    let market = accounts::read_market(&accounts[IX_MARKET].data.borrow())?;
    let terms = accounts::read_terms(&accounts[IX_TERMS].data.borrow())?;
    let grid = accounts::read_price_grid(&accounts[IX_GRID].data.borrow())?;
    require(
        market.market == intent.market
            && market.lifecycle == 0
            && market.outcome_count == 2
            && terms.terms == market.terms
            && terms.realm == market.realm
            && terms.profile == market.profile
            && terms.outcome_count == 2
            && terms.price_grid == grid.grid
            && grid.realm == market.realm,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_MARKET].key,
        seeds::market_pda(program_id, &market.realm.bytes(), &market.market.bytes()),
        Some(market.stored_bump),
    )?;
    expect_pda(
        accounts[IX_TERMS].key,
        seeds::terms_pda(program_id, &terms.realm.bytes(), &terms.terms.bytes()),
        Some(terms.stored_bump),
    )?;
    expect_pda(
        accounts[IX_GRID].key,
        seeds::grid_pda(program_id, &grid.realm.bytes(), &grid.grid.bytes()),
        Some(grid.stored_bump),
    )?;

    let epoch_id = clutch_solana_layout::canonical_epoch_id(intent.market, intent.epoch_index);
    let policy = DirectBatchPolicyV3::decode(&accounts[IX_POLICY].data.borrow())?;
    require_exact_direct_policy(epoch_id, intent.policy, policy)?;
    expect_pda(
        accounts[IX_POLICY].key,
        seeds::direct_batch_policy_v3_pda(program_id, &epoch_id.bytes(), &intent.policy.bytes()),
        None,
    )?;

    let (epoch_address, epoch_bump) =
        seeds::epoch_pda(program_id, &intent.market.bytes(), intent.epoch_index);
    expect_pda(accounts[IX_EPOCH].key, (epoch_address, epoch_bump), None)?;
    let funding = direct_creation_funding(
        &accounts[IX_PAYER],
        &accounts[IX_EPOCH],
        &rent,
        DIRECT_EPOCH_V4_BYTES,
        DIRECT_NEUTRAL_SINK_V3,
    )?;
    let epoch = build_open_epoch(
        intent,
        terms.terms,
        terms.basis_degree,
        grid.grid,
        grid.price_scale,
        epoch_bump,
        funding,
    )?;
    create_pda_account_full_principal(
        program_id,
        &accounts[IX_PAYER],
        &accounts[IX_EPOCH],
        &accounts[IX_SYSTEM],
        &rent,
        DIRECT_EPOCH_V4_BYTES,
        funding,
        0,
        &[
            seeds::SEED_EPOCH,
            &intent.market.bytes(),
            &intent.epoch_index.to_le_bytes(),
            &[epoch_bump],
        ],
    )?;
    epoch.encode(&mut borrow_mut!(accounts[IX_EPOCH])?)?;
    Ok(())
}

fn require_exact_direct_policy(
    epoch_id: Hash32,
    intent_policy: Hash32,
    policy: DirectBatchPolicyV3,
) -> Outcome<()> {
    let exact_policy = DirectBatchPolicyV3::direct(DIRECT_VERIFIER_RELEASE_ID_V3)?;
    require(
        policy == exact_policy && policy.digest_for_epoch(epoch_id)? == intent_policy,
        ClutchError::MismatchedState,
    )
}

fn build_open_epoch(
    intent: InitEpochV4,
    terms: Hash32,
    basis_degree: u8,
    grid: Hash32,
    price_scale: u64,
    epoch_bump: u8,
    funding: DirectFundingLedgerV3,
) -> Outcome<DirectEpochV4Account> {
    require(
        intent.neutral_lamport_sink == Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
        ClutchError::MismatchedState,
    )?;
    let relation_policy = batch_policy_digest(&DIRECT_POLICY_V1)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let direct = DirectEpochV3Account::open(
        intent.market,
        terms,
        grid,
        Hash32::from_bytes(relation_policy.0),
        intent.epoch_index,
        price_scale,
        basis_degree,
        intent.submission_opens_slot,
        intent.submission_closes_slot,
        epoch_bump,
    )?;
    let epoch = DirectEpochV4Account {
        direct,
        selection_deadline_slot: intent.selection_deadline_slot,
        settlement_deadline_slot: intent.settlement_deadline_slot,
        lifecycle_phase: DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN,
        terminal: DirectTerminalReceiptV3::EMPTY,
        neutral_lamport_sink: intent.neutral_lamport_sink,
        verifier_release_id: DIRECT_VERIFIER_RELEASE_ID_V3,
        direct_policy_v3_id: intent.policy,
        epoch_funding: funding,
        page_funding: DirectFundingLedgerV3::ZERO,
        reserved: [0; 4],
    };
    epoch.validate_for_release(DIRECT_VERIFIER_RELEASE_ID_V3)?;
    Ok(epoch)
}

/// Create one durable Epoch while keeping prefund donation disjoint from the
/// payer's exact rent principal.
///
/// Unlike the generic genesis constructor, a hostile predictable-PDA prefund
/// never discounts this payer's deposit. The target must move from `B` to
/// exactly `B + P` before allocation, and the persisted ledger records those
/// two independently owned compartments.
#[allow(clippy::too_many_arguments)] // one argument per runtime account/value
#[inline(never)]
pub(crate) fn direct_creation_funding(
    payer: &AccountInfo,
    target: &AccountInfo,
    rent: &RentParameters,
    space: usize,
    neutral_sink: Pubkey,
) -> Outcome<DirectFundingLedgerV3> {
    let funding = DirectFundingLedgerV3 {
        payer: Hash32::from_bytes(payer.key.to_bytes()),
        payer_principal_lamports: rent.minimum_balance(space)?,
        prior_donation_lamports: target.lamports(),
    };
    funding.validate_for_sink(Hash32::from_bytes(neutral_sink.to_bytes()))?;
    Ok(funding)
}

/// Observe one surviving Direct V3 account without letting later donations
/// replace any part of its authenticated payer principal.
pub(crate) fn observe_direct_funding(
    funding: DirectFundingLedgerV3,
    live_lamports: u64,
    neutral_sink: Pubkey,
) -> Outcome<DirectFundingLedgerV3> {
    funding.validate_for_sink(Hash32::from_bytes(neutral_sink.to_bytes()))?;
    let accounted = funding
        .payer_principal_lamports
        .checked_add(funding.prior_donation_lamports)
        .ok_or(ClutchError::Arithmetic)?;
    require(live_lamports >= accounted, ClutchError::MismatchedState)?;
    let observed = DirectFundingLedgerV3 {
        prior_donation_lamports: live_lamports
            .checked_sub(funding.payer_principal_lamports)
            .ok_or(ClutchError::Arithmetic)?,
        ..funding
    };
    observed.validate_for_sink(Hash32::from_bytes(neutral_sink.to_bytes()))?;
    Ok(observed)
}

/// `extra_deposit` is a second, independently accounted payer compartment
/// transferred in the same exact create delta — the WorkBudget reward pool.
/// It is not principal and never becomes donation: the persisted account
/// bytes record it separately and every later observation protects it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_pda_account_full_principal<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    space: usize,
    funding: DirectFundingLedgerV3,
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

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn intent() -> InitEpochV4 {
        let market = h(1);
        let epoch_index = 7;
        let epoch_id = clutch_solana_layout::canonical_epoch_id(market, epoch_index);
        InitEpochV4 {
            market,
            epoch_index,
            policy: DirectBatchPolicyV3::direct(DIRECT_VERIFIER_RELEASE_ID_V3)
                .unwrap()
                .digest_for_epoch(epoch_id)
                .unwrap(),
            submission_opens_slot: 10,
            submission_closes_slot: 20,
            selection_deadline_slot: 25,
            settlement_deadline_slot: 27,
            neutral_lamport_sink: Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
        }
    }

    fn funding() -> DirectFundingLedgerV3 {
        DirectFundingLedgerV3 {
            payer: h(24),
            payer_principal_lamports: 1_000,
            prior_donation_lamports: 7,
        }
    }

    #[test]
    fn release_label_and_id_are_exact_and_not_a_deployment_claim() {
        assert_eq!(
            DIRECT_VERIFIER_RELEASE_LABEL_V3,
            "dragons-clutch/direct-verifier-release/v3/1"
        );
        assert_eq!(
            DIRECT_VERIFIER_RELEASE_ID_V3.bytes(),
            [
                0x03, 0x89, 0x14, 0xd7, 0x91, 0x30, 0x57, 0x58, 0x9f, 0xa6, 0xbf, 0x30, 0x3f, 0x02,
                0xa6, 0xb9, 0xb5, 0xe1, 0x2a, 0x1e, 0xe7, 0x18, 0x56, 0x10, 0x79, 0xab, 0x57, 0xd3,
                0x75, 0x94, 0x77, 0x04,
            ]
        );
        assert_eq!(
            solana_sha256_hasher::hashv(&[DIRECT_VERIFIER_RELEASE_LABEL_V3.as_bytes()]).to_bytes(),
            DIRECT_VERIFIER_RELEASE_ID_V3.bytes()
        );
    }

    #[test]
    fn open_epoch_uses_relation_policy_and_canonical_neutral_sink() {
        let request = intent();
        let value = build_open_epoch(request, h(3), 1, h(4), 10_000, 9, funding()).unwrap();
        assert_eq!(
            value.direct.common.policy.bytes(),
            batch_policy_digest(&DIRECT_POLICY_V1).unwrap().0
        );
        assert_eq!(
            value.neutral_lamport_sink,
            Hash32::from_bytes(incinerator::ID.to_bytes())
        );
        assert_eq!(value.lifecycle_phase, DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN);
        assert_eq!(value.terminal, DirectTerminalReceiptV3::EMPTY);
        assert_eq!(value.verifier_release_id, DIRECT_VERIFIER_RELEASE_ID_V3);
        assert_eq!(value.direct_policy_v3_id, request.policy);
        assert_eq!(value.epoch_funding, funding());
        assert_eq!(value.page_funding, DirectFundingLedgerV3::ZERO);
        assert_eq!(value.validate(), Ok(()));
    }

    #[test]
    fn live_funding_observation_preserves_principal_and_only_grows_donation() {
        let initial = funding();
        let sink = DIRECT_NEUTRAL_SINK_V3;
        assert_eq!(observe_direct_funding(initial, 1_007, sink), Ok(initial));
        let observed = observe_direct_funding(initial, 1_019, sink).unwrap();
        assert_eq!(observed.payer, initial.payer);
        assert_eq!(
            observed.payer_principal_lamports,
            initial.payer_principal_lamports
        );
        assert_eq!(observed.prior_donation_lamports, 19);
        assert_eq!(
            observe_direct_funding(initial, 1_006, sink),
            Err(ClutchError::MismatchedState.into())
        );
    }

    #[test]
    fn open_epoch_refuses_creator_selected_sink_and_bad_schedule() {
        let mut hostile = intent();
        hostile.neutral_lamport_sink = h(99);
        assert!(build_open_epoch(hostile, h(3), 1, h(4), 10_000, 9, funding()).is_err());

        let mut hostile = intent();
        hostile.selection_deadline_slot = hostile.submission_closes_slot;
        assert!(build_open_epoch(hostile, h(3), 1, h(4), 10_000, 9, funding()).is_err());
    }

    #[test]
    fn canonical_non_direct_policy_and_release_substitution_refuse_precreation() {
        let request = intent();
        let epoch_id =
            clutch_solana_layout::canonical_epoch_id(request.market, request.epoch_index);
        let exact = DirectBatchPolicyV3::direct(DIRECT_VERIFIER_RELEASE_ID_V3).unwrap();
        assert_eq!(
            require_exact_direct_policy(epoch_id, request.policy, exact),
            Ok(())
        );

        let mut nondirect = exact;
        nondirect.policy_bytes[20] = 0;
        assert_eq!(nondirect.validate(), Ok(()));
        assert_eq!(
            require_exact_direct_policy(epoch_id, request.policy, nondirect),
            Err(ClutchError::MismatchedState.into())
        );

        let wrong_release = DirectBatchPolicyV3::direct(h(81)).unwrap();
        let coherent_wrong_release_id = wrong_release.digest_for_epoch(epoch_id).unwrap();
        assert_eq!(wrong_release.validate(), Ok(()));
        assert_eq!(
            require_exact_direct_policy(epoch_id, coherent_wrong_release_id, wrong_release),
            Err(ClutchError::MismatchedState.into())
        );
        assert_eq!(
            require_exact_direct_policy(epoch_id, h(82), exact),
            Err(ClutchError::MismatchedState.into())
        );
    }

    /// Every routed lifecycle action now has one real handler, and each
    /// fail-closes on account shape or replay sequence before touching state.
    /// Nothing anywhere in the family returns `NotYetImplemented`.
    #[test]
    fn every_routed_v3_action_fail_closes_before_reading_state() {
        let market = h(1);
        let epoch = h(2);
        let rewards = clutch_solana_layout::direct_selection_v3::DirectKeeperRewardsV3 {
            begin_verification: 1,
            verify_candidate: 1,
            finalize_selection: 1,
            settle: 1,
            lapse: 1,
        };
        let intents = [
            DirectV3Intent::FreezeEpoch {
                market,
                epoch,
                reward_deposit: rewards.worst_case().unwrap(),
                rewards,
            },
            DirectV3Intent::AbortUnfrozen { market, epoch },
            DirectV3Intent::SubmitCandidate {
                market,
                epoch,
                outcome_price: 2_500,
            },
            DirectV3Intent::BeginVerification { market, epoch },
            DirectV3Intent::VerifyCandidate {
                market,
                epoch,
                retained_index: 0,
            },
            DirectV3Intent::FinalizeSelection { market, epoch },
            DirectV3Intent::Settle { market, epoch },
            DirectV3Intent::LapseEmpty { market, epoch },
            DirectV3Intent::LapseUnselected { market, epoch },
            DirectV3Intent::LapseSelected { market, epoch },
        ];
        let not_yet: Refusal = ClutchError::NotYetImplemented.into();
        for intent in intents {
            let request = DirectV3Request {
                sequence: 0,
                intent,
            };
            let refusal = process(&Pubkey::new_from_array([7; 32]), &[], &request).unwrap_err();
            assert_eq!(
                refusal,
                Refusal::Adapter(ClutchError::AccountCount),
                "{intent:?}"
            );
            assert_ne!(refusal, not_yet, "{intent:?}");
            let replay = DirectV3Request {
                sequence: 1,
                intent,
            };
            assert_eq!(
                process(&Pubkey::new_from_array([7; 32]), &[], &replay),
                Err(ClutchError::Replay.into()),
                "{intent:?}"
            );
        }
    }
}
