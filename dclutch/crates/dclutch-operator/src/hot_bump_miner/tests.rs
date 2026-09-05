//! Both arms of every slot, and the equality the hint is a memo about.
//!
//! The degradation arms are the ones a producer gets wrong -- 8a691ee57 and
//! 82465e00b were both a producer emitting zeros where a derivation was
//! possible -- so each is asserted against a POSITIVE control built from the
//! same canonical encoders, never against "nothing happened".
//!
//! `activated_custody_program_v1` has only its `None` arm here, because
//! encoding a five-role activation cache is a fixture this crate has no other
//! use for. Its positive arm is exercised where it matters: with it returning
//! `None` the Rational outer builders emit a zero `child_relay[1]` and
//! `current_common_hot_executes_issue_and_selected_denominate_through_real_elves`
//! reports the offset.

use dclutch_core_contract::ContentId;
use dclutch_custody::CustodyAuthoritySeedsV1;
use dclutch_market::capability_program::{
    CapabilityRootHeaderV1, SelectedRecordBumpsV1, hot_v3::HotBumpHintsV1,
};
use dclutch_market::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness, StateBumpsV1,
};
use dclutch_registry::release_set::CapabilityExecutionSelectionV1;
use solana_program::pubkey::Pubkey;

use super::{
    HOT_BUMP_HINT_SLOT_NAMES_V1, HotBumpCorpusV1, activated_custody_program_v1,
    capability_root_bump_v1, custody_transfer_authority_bump_v1, hot_bump_hint_slot_name_v1,
    market_state_bump_v1, mine_hot_bump_hints_v1,
};

const CORE_PROGRAM: [u8; 32] = [0x21; 32];
const TRADING_PROGRAM: [u8; 32] = [0x22; 32];
const CUSTODY_PROGRAM: [u8; 32] = [0x23; 32];
const MARKET: [u8; 32] = [0x24; 32];
const RELEASE_SET: [u8; 32] = [0x25; 32];
const REALM: [u8; 32] = [0x26; 32];
const PRODUCT_RECORD: [u8; 32] = [0x27; 32];
const PRODUCT_ID: [u8; 32] = [0x28; 32];
const RESOLUTION_POLICY: [u8; 32] = [0x29; 32];
const CAPABILITY_MANIFEST: [u8; 32] = [0x2a; 32];
const REGISTRY_PROGRAM: [u8; 32] = [0x2b; 32];
const RENT_BENEFICIARY: [u8; 32] = [0x2c; 32];
const SELECTION_KIND: [u8; 32] = [0x2d; 32];
const SELECTION_RELEASE: [u8; 32] = [0x2e; 32];
const SELECTION_CONFIG: [u8; 32] = [0x2f; 32];
const GENERATION: u64 = 23;

fn identity(value: [u8; 32]) -> Identity {
    Identity::new(value).expect("distinct nonzero identity fill")
}

fn market_identity() -> MarketIdentity {
    MarketIdentity {
        market_id: identity(MARKET),
        realm_id: identity(REALM),
        product_record: identity(PRODUCT_RECORD),
        product_id: identity(PRODUCT_ID),
        resolution_policy: identity(RESOLUTION_POLICY),
        capability_manifest: identity(CAPABILITY_MANIFEST),
        selected_release_set: identity(RELEASE_SET),
        registry_program: identity(REGISTRY_PROGRAM),
        generation: GENERATION,
    }
}

fn market_state() -> Vec<u8> {
    CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: market_identity(),
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity(RENT_BENEFICIARY),
        terminal_receipt: None,
        // UNRECORDED on purpose: a Market that records its own bump makes the
        // `market` hint inert, so a fixture that records one would be pinning a
        // byte no reader consumes.
        bumps: StateBumpsV1::UNRECORDED,
    }
    .encode()
    .expect("canonical open Core Market state")
    .to_vec()
}

fn root_header() -> Vec<u8> {
    CapabilityRootHeaderV1::new(
        ContentId::new(RELEASE_SET).expect("release set"),
        MARKET,
        GENERATION,
        CapabilityExecutionSelectionV1::new(
            3,
            ContentId::new(CAPABILITY_MANIFEST).expect("manifest"),
            ContentId::new(SELECTION_KIND).expect("kind"),
            ContentId::new(SELECTION_RELEASE).expect("capability release"),
            ContentId::new(SELECTION_CONFIG).expect("config"),
        )
        .expect("execution selection")
        .with_capability_release_record_bumps(0xfd, 0xfc),
        SelectedRecordBumpsV1::new(0xff, 0xfe, 0xfb, 0xfa),
    )
    .expect("capability root header")
    .to_bytes()
    .to_vec()
}

fn corpus<'a>(market_data: &'a [u8], root_data: &'a [u8]) -> HotBumpCorpusV1<'a> {
    HotBumpCorpusV1 {
        market_key: Pubkey::new_from_array(MARKET),
        market_data,
        root_data,
        core_program: Pubkey::new_from_array(CORE_PROGRAM),
        trading_program: Pubkey::new_from_array(TRADING_PROGRAM),
        custody_program: Some(Pubkey::new_from_array(CUSTODY_PROGRAM)),
        release_set: RELEASE_SET,
    }
}

/// Each mined byte reproduces the address the reader will compare against.
///
/// This is the whole claim a hint makes. `create_program_address` with the
/// mined bump names the same account `find_program_address` searched down to,
/// so the reader's equality holds without the search.
#[test]
fn every_mined_bump_reproduces_the_address_its_reader_derives() {
    let market_data = market_state();
    let root_data = root_header();
    let corpus = corpus(&market_data, &root_data);
    let hints = mine_hot_bump_hints_v1(&corpus);

    let market_seeds = MarketCoreStateSeedsV2::new(market_identity());
    let mut seeds = market_seeds.as_slices().to_vec();
    let bump = [hints.market];
    seeds.push(&bump);
    assert_eq!(
        Pubkey::create_program_address(&seeds, &corpus.core_program).expect("market state address"),
        Pubkey::find_program_address(&market_seeds.as_slices(), &corpus.core_program).0,
    );

    let header = CapabilityRootHeaderV1::decode(&root_data).expect("root header");
    let root_seeds = header.seeds();
    let mut seeds = root_seeds.as_slices().to_vec();
    let bump = [hints.root];
    seeds.push(&bump);
    assert_eq!(
        Pubkey::create_program_address(&seeds, &corpus.trading_program)
            .expect("capability root address"),
        Pubkey::find_program_address(&root_seeds.as_slices(), &corpus.trading_program).0,
    );

    let authority_seeds = CustodyAuthoritySeedsV1::new(MARKET, RELEASE_SET);
    let mut seeds = authority_seeds.as_slices().to_vec();
    let bump = [hints.child_relay[1]];
    seeds.push(&bump);
    let custody = corpus.custody_program.expect("Custody deployment");
    assert_eq!(
        Pubkey::create_program_address(&seeds, &custody).expect("transfer authority address"),
        Pubkey::find_program_address(&authority_seeds.as_slices(), &custody).0,
    );
}

/// The five slots this corpus cannot reach stay zero, and say so by staying
/// zero rather than by refusing.
#[test]
fn the_slots_this_corpus_cannot_reach_stay_absent() {
    let market_data = market_state();
    let root_data = root_header();
    let hints = mine_hot_bump_hints_v1(&corpus(&market_data, &root_data));
    assert_eq!(hints.lifecycle, [0, 0]);
    assert_eq!(hints.child_caller, [0, 0]);
    assert_eq!(hints.child_relay[0], 0);
}

/// A corpus that decodes nowhere yields the absent block, never a refusal.
///
/// The positive control is the test above: the same three slots, filled, from
/// the same three constructors.
#[test]
fn an_undecodable_corpus_degrades_to_absent_rather_than_refusing() {
    let hints = mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
        market_key: Pubkey::new_from_array(MARKET),
        market_data: &[],
        root_data: &[],
        core_program: Pubkey::new_from_array(CORE_PROGRAM),
        trading_program: Pubkey::new_from_array(TRADING_PROGRAM),
        custody_program: None,
        release_set: RELEASE_SET,
    });
    assert_eq!(hints, HotBumpHintsV1::ABSENT);
    assert!(hints.is_absent());
}

/// Each slot degrades on its own, so one unreadable account does not cost the
/// other two their hints.
#[test]
fn one_unreadable_account_costs_only_its_own_slot() {
    let market_data = market_state();
    let root_data = root_header();

    let mut corpus = corpus(&market_data, &[]);
    assert_eq!(capability_root_bump_v1(&corpus), None);
    assert!(market_state_bump_v1(&corpus).is_some());
    assert!(custody_transfer_authority_bump_v1(&corpus).is_some());

    corpus = self::corpus(&[], &root_data);
    assert_eq!(market_state_bump_v1(&corpus), None);
    assert!(capability_root_bump_v1(&corpus).is_some());
    assert!(custody_transfer_authority_bump_v1(&corpus).is_some());

    corpus = self::corpus(&market_data, &root_data);
    corpus.custody_program = None;
    assert_eq!(custody_transfer_authority_bump_v1(&corpus), None);
    assert!(market_state_bump_v1(&corpus).is_some());
    assert!(capability_root_bump_v1(&corpus).is_some());
}

/// A root body truncated inside its own header is absent, not a panic.
#[test]
fn a_truncated_root_header_is_absent() {
    let market_data = market_state();
    let root_data = root_header();
    let truncated = root_data.get(..8).expect("truncated root prefix");
    assert_eq!(
        capability_root_bump_v1(&corpus(&market_data, truncated)),
        None
    );
}

/// An activation cache that does not decode names no Custody deployment.
#[test]
fn an_undecodable_activation_cache_names_no_custody_deployment() {
    assert_eq!(activated_custody_program_v1(&[]), None);
    assert_eq!(activated_custody_program_v1(&[0; 512]), None);
}

/// The slot names index the block they name, and nothing outside it.
#[test]
fn the_slot_names_cover_the_block_and_stop_at_its_edges() {
    use dclutch_market::capability_program::hot_v3::{
        HOT_BUMP_HINT_COUNT_V1, HOT_BUMP_HINTS_OFFSET_V1, HOT_EXECUTION_ENVELOPE_BYTES_V3,
    };
    assert_eq!(HOT_BUMP_HINT_SLOT_NAMES_V1.len(), HOT_BUMP_HINT_COUNT_V1);
    assert_eq!(
        hot_bump_hint_slot_name_v1(HOT_BUMP_HINTS_OFFSET_V1),
        Some("market")
    );
    assert_eq!(
        hot_bump_hint_slot_name_v1(HOT_EXECUTION_ENVELOPE_BYTES_V3 - 1),
        Some("child_relay[1]")
    );
    assert_eq!(
        hot_bump_hint_slot_name_v1(HOT_BUMP_HINTS_OFFSET_V1 - 1),
        None
    );
    assert_eq!(
        hot_bump_hint_slot_name_v1(HOT_EXECUTION_ENVELOPE_BYTES_V3),
        None
    );
}
