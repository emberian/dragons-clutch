//! Unrouted Direct V3 lifecycle adapter.
//!
//! This module is compiled and host-tested, but [`crate::dispatch`] does not
//! route any Direct V3 tag to it. The dedicated request decoder and every V3
//! intent therefore remain fail-closed until the whole account family can be
//! switched on atomically. The first vertical slice below owns only creation
//! of the durable pre-freeze Epoch; page creation, Reservation V2 placement,
//! Freeze, staged verification, settlement, and lapse are still runtime STOPs.

use crate::accounts::{
    self, expect_pda, require, require_count, require_distinct, require_signer, Outcome, StateRole,
};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::{
    create_pda_account, read_rent, require_creatable, require_system_program,
};
use crate::seeds;
use clutch_batch_policy_identity::{batch_policy_digest, direct_window_v1::DIRECT_POLICY_V1};
use clutch_solana_layout::{
    account_len,
    direct_selection::DirectEpochV3Account,
    direct_selection_v3::{
        DirectBatchPolicyV3, DirectEpochV4Account, DirectTerminalReceiptV3, DirectV3Intent,
        DIRECT_BATCH_POLICY_V3_BYTES, DIRECT_EPOCH_V4_BYTES, DIRECT_LIFECYCLE_PHASE_PREFREEZE_OPEN,
    },
    Hash32,
};
use clutch_solana_reference::DirectV3Request;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use solana_sdk_ids::incinerator;

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

/// Entry point reserved for the future atomic Direct V3 dispatcher cut.
///
/// Only Init has an implemented body in this incremental checkpoint. All
/// other already-decoded actions refuse without reading accounts.
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
        _ => Err(ClutchError::NotYetImplemented.into()),
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
    require(
        policy.verifier_release_id == DIRECT_VERIFIER_RELEASE_ID_V3
            && policy.digest_for_epoch(epoch_id)? == intent.policy,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        accounts[IX_POLICY].key,
        seeds::direct_batch_policy_v3_pda(program_id, &epoch_id.bytes(), &intent.policy.bytes()),
        None,
    )?;

    let (epoch_address, epoch_bump) =
        seeds::epoch_pda(program_id, &intent.market.bytes(), intent.epoch_index);
    expect_pda(accounts[IX_EPOCH].key, (epoch_address, epoch_bump), None)?;
    let epoch = build_open_epoch(intent, terms.terms, grid.grid, grid.price_scale, epoch_bump)?;
    create_pda_account(
        program_id,
        &accounts[IX_PAYER],
        &accounts[IX_EPOCH],
        &accounts[IX_SYSTEM],
        &rent,
        DIRECT_EPOCH_V4_BYTES,
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

fn build_open_epoch(
    intent: InitEpochV4,
    terms: Hash32,
    grid: Hash32,
    price_scale: u64,
    epoch_bump: u8,
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
        reserved: [0; 4],
    };
    epoch.validate()?;
    Ok(epoch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(byte: u8) -> Hash32 {
        Hash32::from_bytes([byte; 32])
    }

    fn intent() -> InitEpochV4 {
        InitEpochV4 {
            market: h(1),
            epoch_index: 7,
            policy: h(2),
            submission_opens_slot: 10,
            submission_closes_slot: 20,
            selection_deadline_slot: 25,
            settlement_deadline_slot: 27,
            neutral_lamport_sink: Hash32::from_bytes(DIRECT_NEUTRAL_SINK_V3.to_bytes()),
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
        let value = build_open_epoch(intent(), h(3), h(4), 10_000, 9).unwrap();
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
        assert_eq!(value.validate(), Ok(()));
    }

    #[test]
    fn open_epoch_refuses_creator_selected_sink_and_bad_schedule() {
        let mut hostile = intent();
        hostile.neutral_lamport_sink = h(99);
        assert!(build_open_epoch(hostile, h(3), h(4), 10_000, 9).is_err());

        let mut hostile = intent();
        hostile.selection_deadline_slot = hostile.submission_closes_slot;
        assert!(build_open_epoch(hostile, h(3), h(4), 10_000, 9).is_err());
    }

    #[test]
    fn every_unimplemented_v3_action_refuses_before_accounts() {
        let request = DirectV3Request {
            sequence: 0,
            intent: DirectV3Intent::BeginVerification {
                market: h(1),
                epoch: h(2),
            },
        };
        assert_eq!(
            process(&Pubkey::new_from_array([7; 32]), &[], &request),
            Err(ClutchError::NotYetImplemented.into())
        );
        assert_ne!(
            Err::<(), _>(ClutchError::NotYetImplemented.into()),
            Err(Refusal::Reference(clutch_solana_reference::Error::WrongTag))
        );
    }
}
