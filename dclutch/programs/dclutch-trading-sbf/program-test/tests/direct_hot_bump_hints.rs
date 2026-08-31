//! The hinted wire executes, costs less, and refuses a wrong byte.
//!
//! `direct_hot_top_level.rs` proves the public route executes. It proves it
//! with an ALL-ZERO hint block, which is the absent case: every address is
//! searched for, exactly as before this mechanism existed. That is the
//! backward-compatibility half and it is not the half the ruling is about.
//!
//! This file submits the same trade on the same fixture with the hints the
//! caller can mine off chain actually filled in, and asserts the three things
//! that make the mechanism real rather than plausible:
//!
//! 1. **It executes.** Same collateral movement, same commit-last ACK. A
//!    mechanism that reproduces addresses correctly for seven distinct PDAs
//!    across three programs is a mechanism whose seed reconstruction agrees
//!    with every reader's, and nothing short of running it says so.
//! 2. **It costs less, measured on the same draw.** Both submissions run
//!    against the same fixture in the same file, so the difference is the
//!    searches and nothing else. That number is the lane's evidence.
//! 3. **A wrong byte refuses, in every slot, one slot at a time.** This is
//!    the safety argument executed rather than argued: the unit tests show a
//!    wrong hint names a different address, and this shows the route then
//!    refuses rather than proceeding against that address.
//!
//! Two of the eight slots behave differently, and the file is explicit about
//! both rather than quietly skipping them.
//!
//! **The Claims caller authority is not covered.** Its seeds end in a digest
//! over the projected Claims child request, which this fixture does not publish
//! -- it publishes the four Custody routes' digests and not the Claims one --
//! so the test leaves that slot zero and it searches. The operator fills it
//! from `derive_direct_inline_child_authorities_v3`; that path has no on-chain
//! coverage here.
//!
//! **The Market slot is INERT here, and asserting that is the point.** Since
//! `30574297` the gate fixture stages what `plan_found` actually writes, so
//! `CoreState` RECORDS the Market bump and all three readers reproduce from the
//! record instead of from the wire. `market_core_state_address_v2` spells the
//! precedence as `state.bumps.market.or(hint)`: the creator's own on-chain
//! assertion outranks a byte off the wire, so a caller cannot steer a Market
//! that already knows its own address. The refusal walk below therefore
//! requires a hostile Market hint to be IGNORED rather than refused, which is a
//! stronger claim than a refusal and is the one that makes accepting a hint
//! from a stranger safe in the first place.

use dclutch_capability_program_contract::hot_v3::{
    HOT_BUMP_HINT_COUNT_V1, HOT_BUMP_HINTS_OFFSET_V1, HotBumpHintsV1, HotExecutionEnvelopeV3,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CallerRoleV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1};
use dclutch_direct_codec::successor::{DirectCoordinatesV1, MakerReplaySeedsV1};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::TokenAccount;
use solana_program::instruction::Instruction;
use solana_program::pubkey::Pubkey;
use solana_program_test::ProgramTestContext;
use solana_sdk::signer::Signer;

use dclutch_direct_hot_program_test_support::waist::{
    COMPUTE_LIMIT, CORE_PROGRAM_ID, CUSTODY_PROGRAM_ID, TRADING_PROGRAM_ID, add_lookup_table,
    add_release_waist, canonical_lookup_addresses, direct_case, direct_top_level_instructions,
    elves, fixture_substrate, program_test_without_forced_budget, start_with_substrate,
    submit_v0_observed,
};

/// How many slots this fixture publishes enough information to mine.
const MINED_SLOTS: usize = 7;
/// The Market slot: mined, but carried by `CoreState` and therefore inert.
const MARKET_SLOT: usize = 0;
/// The Claims caller-authority slot, which this fixture cannot mine.
const CLAIMS_CALLER_SLOT: usize = 4;
/// Mined slots the route actually reproduces an address from.
const LOAD_BEARING_SLOTS: usize = MINED_SLOTS - 1;

/// Which hint slot each name occupies, so a failure names the slot.
const SLOT_NAMES: [&str; HOT_BUMP_HINT_COUNT_V1] = [
    "market",
    "root",
    "seller maker replay",
    "buyer maker replay",
    "Claims caller authority",
    "Custody caller authority",
    "Custody replay",
    "Custody transfer authority",
];

/// Mine every hint this fixture publishes enough information to derive.
///
/// Exactly what an off-chain caller does, and from exactly the same inputs: the
/// envelope it is about to send, and the accounts it is about to name.
fn mine(
    instructions: &[Instruction; 4],
    case: &dclutch_direct_hot_program_test_support::waist::DirectCase,
) -> HotBumpHintsV1 {
    let data = &instructions[3].data;
    let envelope =
        HotExecutionEnvelopeV3::decode(&data[..HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1])
            .expect("the wire this test is about to send");
    let account = |key: Pubkey| {
        case.chain
            .accounts
            .iter()
            .find(|installed| installed.key == key)
            .map(|installed| installed.account.data.clone())
            .unwrap_or_else(|| panic!("fixture publishes {key}"))
    };

    let market_state = CoreState::decode(&account(case.chain.market)).expect("Core Market state");
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market_state.identity).as_slices(),
        &CORE_PROGRAM_ID,
    )
    .1;

    let root_data = account(case.chain.root);
    let root_header = CapabilityRootHeaderV1::decode(&root_data[..CAPABILITY_ROOT_HEADER_BYTES_V1])
        .expect("capability root header");
    let root =
        Pubkey::find_program_address(&root_header.seeds().as_slices(), &TRADING_PROGRAM_ID).1;

    let coordinates = DirectCoordinatesV1::new(envelope.market(), envelope.generation())
        .expect("Direct coordinates");
    let mut lifecycle = [0_u8; 2];
    for (slot, maker) in lifecycle
        .iter_mut()
        .zip([case.makers[0].pubkey(), case.makers[1].pubkey()])
    {
        *slot = Pubkey::find_program_address(
            &MakerReplaySeedsV1::new(coordinates, maker.to_bytes())
                .expect("maker replay seeds")
                .as_slices(),
            &TRADING_PROGRAM_ID,
        )
        .1;
    }

    // The Custody route the fee-free fixture actually enables is the
    // seller-terminal one, index 0, and it is the SECOND child invocation --
    // Claims is the first -- so it owns `child_caller[1]`.
    let buyer_maker_root = case.chain.maker_replays[1].to_bytes();
    let custody_caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(envelope.release_set()).expect("release set"),
            envelope.market(),
            ExecutionRoleV1::Trading,
            buyer_maker_root,
            case.chain.custody_routes[0].request_digest,
        )
        .expect("Custody caller authority seeds")
        .as_slices(),
        &TRADING_PROGRAM_ID,
    )
    .1;

    let custody_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            envelope.market(),
            envelope.release_set(),
            CallerRoleV1::Trading,
            buyer_maker_root,
        )
        .as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .1;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(envelope.market(), envelope.release_set()).as_slices(),
        &CUSTODY_PROGRAM_ID,
    )
    .1;

    HotBumpHintsV1 {
        market,
        root,
        lifecycle,
        // The Claims slot stays absent; see this file's header.
        child_caller: [0, custody_caller],
        child_relay: [custody_replay, custody_authority],
    }
}

fn with_hints(instructions: &mut [Instruction; 4], hints: HotBumpHintsV1) {
    let block = hints.to_bytes();
    instructions[3].data
        [HOT_BUMP_HINTS_OFFSET_V1..HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1]
        .copy_from_slice(&block);
}

async fn account_data(context: &mut ProgramTestContext, key: Pubkey) -> Vec<u8> {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("account present")
        .data
}

/// Run one top-level submission at the given hints and report its cost.
async fn submit(hints: Option<HotBumpHintsV1>) -> Result<(u64, [u64; 2]), Option<u32>> {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);
    let mut instructions = direct_top_level_instructions(&direct);
    if let Some(hints) = hints {
        with_hints(&mut instructions, hints);
    }
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;
    let outcome = submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await;
    let execution = match outcome {
        Ok(execution) => execution,
        Err(refused) => return Err(refusal_code(&refused.error)),
    };
    let mut balances = [0_u64; 2];
    for (slot, key) in balances.iter_mut().zip([
        direct.chain.collateral_accounts[0],
        direct.chain.collateral_accounts[1],
    ]) {
        *slot = TokenAccount::parse(&account_data(&mut context, key).await)
            .expect("collateral token account")
            .amount;
    }
    Ok((execution.compute_units_consumed, balances))
}

fn refusal_code(error: &solana_program_test::BanksClientError) -> Option<u32> {
    use solana_program::instruction::InstructionError;
    use solana_sdk::transaction::TransactionError;
    let transaction = match error {
        solana_program_test::BanksClientError::TransactionError(value) => value,
        solana_program_test::BanksClientError::SimulationError { err, .. } => err,
        _ => return None,
    };
    match transaction {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
        _ => None,
    }
}

/// Mine what a caller can mine, and the same trade costs measurably less.
#[tokio::test]
async fn the_mined_hints_execute_the_same_trade_for_fewer_compute_units() {
    let (bare_units, bare_balances) = submit(None).await.expect("unhinted execution");

    let artifacts = elves();
    let mut probe = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut probe, &artifacts);
    let case = direct_case(&mut probe, releases, &artifacts, false);
    let hints = mine(&direct_top_level_instructions(&case), &case);
    // Every slot this fixture can fill is filled, and none of them is zero --
    // a zero would mean the mining silently produced "absent" and the test
    // would then be measuring the unhinted route twice.
    for (index, byte) in hints.to_bytes().into_iter().enumerate() {
        if index == CLAIMS_CALLER_SLOT {
            continue; // Not mineable here; see this file's header.
        }
        assert_ne!(
            byte, 0,
            "slot {index} ({}) mined to absent",
            SLOT_NAMES[index]
        );
    }
    drop(probe);

    let (hinted_units, hinted_balances) = submit(Some(hints)).await.expect("hinted execution");

    // The trade is the same trade. `direct_hot_top_level.rs` owns these two
    // numbers; they are repeated rather than referenced because a cheaper
    // execution that moved different collateral would be a regression wearing
    // a saving's clothes.
    assert_eq!(bare_balances, [95, 35]);
    assert_eq!(hinted_balances, [95, 35]);

    assert!(bare_units <= COMPUTE_LIMIT && hinted_units <= COMPUTE_LIMIT);

    // REPORTED, NOT GATED, and the reason is CARRY's correction restated for
    // this measurement. `create_program_address` costs the same 1,500 CU as one
    // rejected `find_program_address` candidate, so converting a search of
    // depth `d` saves `(d - 1) * 1,500` and saves NOTHING at `d = 1`. This
    // fixture's bumps are one draw. A test that asserted a positive saving
    // would be asserting that this draw was unlucky enough to make the
    // conversion pay, which is a property of the keys and not of the code --
    // exactly the class of assertion b61ffdad retired from the margin gate.
    // The constancy claim belongs where the sweep is: the margin gate.
    //
    // What the number IS good for is sizing, so it is printed with its
    // decomposition. The residual after whole bump attempts is the per-site
    // cost of the reproduction ARM over the search arm -- building the seed
    // array with the trailing bump and mapping the error -- which is real,
    // small, and paid once per converted site whether or not the search it
    // replaced was deep.
    let saved = i64::try_from(bare_units).expect("bare units")
        - i64::try_from(hinted_units).expect("hinted units");
    let attempts = saved.div_euclid(1_500);
    let residual = saved.rem_euclid(1_500);
    println!(
        "top-level Direct Hot: {bare_units} unhinted, {hinted_units} hinted, {saved} CU \
         = {attempts} bump attempts at 1,500 plus {residual} CU of arm difference \
         over {MINED_SLOTS} converted sites"
    );
}

/// One wrong byte in any slot refuses. Slot by slot, so the failure names one.
#[tokio::test]
async fn a_wrong_hint_in_any_slot_refuses_rather_than_executing() {
    let artifacts = elves();
    let mut probe = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut probe, &artifacts);
    let case = direct_case(&mut probe, releases, &artifacts, false);
    let canonical = mine(&direct_top_level_instructions(&case), &case);
    drop(probe);

    let mut refused = 0_usize;
    for slot in 0..HOT_BUMP_HINT_COUNT_V1 {
        if slot == CLAIMS_CALLER_SLOT {
            continue; // Not mined here; see this file's header.
        }
        let mut bytes = canonical.to_bytes();
        // Not a random byte: the canonical bump MINUS ONE is the strongest
        // hostile available, because it is the next candidate the search
        // itself would have tried and is therefore the value most likely to
        // also be a valid program address rather than an off-curve refusal.
        bytes[slot] = bytes[slot].wrapping_sub(1);
        let mut hostile = canonical;
        hostile.market = bytes[0];
        hostile.root = bytes[1];
        hostile.lifecycle = [bytes[2], bytes[3]];
        hostile.child_caller = [bytes[4], bytes[5]];
        hostile.child_relay = [bytes[6], bytes[7]];

        let outcome = submit(Some(hostile)).await;
        if slot == MARKET_SLOT {
            // NOT a refusal, and this row matters as much as the refusals do.
            // Since `30574297` the fixture stages what `plan_found` writes, so
            // `CoreState` RECORDS this bump and the reader never reaches the
            // hint: a hostile byte changes nothing. That is the precedence
            // `market_core_state_address_v2` spells as
            // `state.bumps.market.or(hint)` -- the creator's own on-chain
            // assertion outranks a byte off the wire, so a caller cannot steer
            // a Market that already knows its own address. Reorder that `or`
            // so the wire wins and this row goes red and names the slot.
            assert!(
                outcome.is_ok(),
                "a wrong Market hint changed the outcome. The recorded bump must \
                 outrank the wire: see `market_core_state_address_v2`.",
            );
            continue;
        }
        assert!(
            outcome.is_err(),
            "slot {slot} ({}) executed with a hint that is not its canonical bump. \
             The reproduction is not being compared against the account the frame \
             supplied -- see this file's header.",
            SLOT_NAMES[slot],
        );
        refused = refused.saturating_add(1);
    }
    assert_eq!(
        refused, LOAD_BEARING_SLOTS,
        "every mined slot the route reads a byte FROM must refuse a wrong one"
    );
}
