//! The two-sided caller-mined bump-hint vector.
//!
//! `apps/dclutch-web/lib/directHotBumpHintsV1.ts` and its byte-identical SDK
//! twin mine the eight bumps the V3 hot envelope reserves, so a browser-built
//! trade stops paying `find_program_address` on chain at 1,500 CU per rejected
//! candidate. Mining is only useful if the browser reconstructs the SAME seeds
//! the program does -- a hint that names a different address is refused, so a
//! drifted TypeScript miner turns a saving into a wall.
//!
//! This test is the joint author. One vector file, two independent producers:
//! the Rust seed constructors here, and the TypeScript miner in
//! `directHotBumpHintsV1.test.ts`. If a seed order moves, THIS test goes red
//! first -- the authority stays in the crates, and the browser is the side that
//! has to catch up.
//!
//! WHICH READER EACH SEED CONSTRUCTOR SERVES, so nothing below is a private
//! restatement of an order some other file also spells:
//!
//! - `MarketCoreStateSeedsV2` -- Core's own state address, and the one
//!   `market_core_state_address_v2` reproduces on chain.
//! - `CapabilityRootHeaderV1::seeds` -- the root's address, derived from the
//!   header the root account itself carries.
//! - `MakerReplaySeedsV1` -- the two accounts the InlineOrdinary lifecycle
//!   materializes, seller then buyer, which IS the slot order.
//! - `CustodyReplaySeedsV1` / `CustodyAuthoritySeedsV1` -- the two addresses
//!   Custody derives for itself and can carry from nowhere.
//! - `CallerAuthoritySeedsV1` -- one child caller authority, whose last seed is
//!   a digest over the child's projected request.
//!
//! The same five are what `direct_inline_hot_bump_hints_v1` mines through and
//! what `direct_hot_bump_hints.rs` proves execute against a real Trading ELF.
//!
//! WHAT THE INPUTS ARE, so no field here is a number chosen to make something
//! pass: every identity is a distinct constant fill, supplied to the encoders
//! as an INPUT. The assertion is that both languages take the same inputs
//! through the same seed order to the same eight bytes. The Core state and root
//! header are emitted as their real canonical encodings rather than as loose
//! seed lists, because the browser reads those exact account bodies at exactly
//! those offsets and a vector of pre-extracted seeds would not test that.
//!
//! Regenerate with `DCLUTCH_WRITE_WIRE_VECTOR=1 cargo test -p dclutch-operator
//! --test browser_bump_hint_vector`, and only when a seed order deliberately
//! moved. THAT RUN REFUSES on purpose: it writes both copies and then fails,
//! because a test cannot review its own regeneration. Finish the move with
//! `python3 tools/gate emission --pins --update` and commit the fixtures and
//! their pins together. Note that the write branch writes the SDK copy too,
//! which nothing here reads back -- both are pinned for that reason.

use std::{env, fs, path::PathBuf};

use dclutch_market::capability_program::{
    CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    hot_v3::{HOT_BUMP_HINT_COUNT_V1, HotBumpHintsV1},
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{CallerRoleV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1};
use dclutch_trading::successor::{DirectCoordinatesV1, MakerReplaySeedsV1};
use dclutch_market::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness, StateBumpsV1,
};
use dclutch_registry::release_set::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CallerAuthoritySeedsV1,
    CapabilityExecutionSelectionV1, ExecutionRoleV1,
};
use solana_program::pubkey::Pubkey;

const NOTE: &str = "Two-sided vector for the caller-mined bump hints the V3 hot envelope carries at HOT_BUMP_HINTS_OFFSET_V1. Produced by crates/dclutch-operator/tests/browser_bump_hint_vector.rs through the same exported seed constructors direct_inline_hot_bump_hints_v1 mines through, and reproduced independently by the byte-identical TypeScript miner in packages/dclutch-sdk/lib/directHotBumpHintsV1.ts and apps/dclutch-web/lib/directHotBumpHintsV1.ts. The Rust crates are the authority: if a seed order moves, the Rust test fails first. Every identity is a distinct constant fill supplied as an encoder INPUT, never as an expected answer. The Market state and capability root are emitted as their real canonical account encodings because the browser reads those exact bodies at those exact offsets. childCaller carries the two Trading caller-authority bumps derived from the two pinned child request digests; the miner takes them as a parameter for the same reason build_direct_inline_hot_v4 does -- their seeds end in a digest over a PROJECTED child request, which no exterior caller rebuilds.";

const CORE_PROGRAM: [u8; 32] = [0x41; 32];
const TRADING_PROGRAM: [u8; 32] = [0x42; 32];
const CUSTODY_PROGRAM: [u8; 32] = [0x43; 32];
const MARKET: [u8; 32] = [0x44; 32];
const SELLER_MAKER: [u8; 32] = [0x45; 32];
const BUYER_MAKER: [u8; 32] = [0x46; 32];
const RELEASE_SET: [u8; 32] = [0x47; 32];
const REALM: [u8; 32] = [0x48; 32];
const PRODUCT_RECORD: [u8; 32] = [0x49; 32];
const PRODUCT_ID: [u8; 32] = [0x4a; 32];
const RESOLUTION_POLICY: [u8; 32] = [0x4b; 32];
const CAPABILITY_MANIFEST: [u8; 32] = [0x4c; 32];
const REGISTRY_PROGRAM: [u8; 32] = [0x4d; 32];
const RENT_BENEFICIARY: [u8; 32] = [0x4e; 32];
const SELECTION_KIND: [u8; 32] = [0x4f; 32];
const SELECTION_RELEASE: [u8; 32] = [0x50; 32];
const SELECTION_CONFIG: [u8; 32] = [0x51; 32];
const PARENT_REQUEST_DIGEST: [u8; 32] = [0x52; 32];
const CLAIMS_REQUEST_DIGEST: [u8; 32] = [0x53; 32];
const CUSTODY_REQUEST_DIGEST: [u8; 32] = [0x54; 32];
const GENERATION: u64 = 19;
const SELECTION_ENTRY_INDEX: u16 = 3;

fn vector_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/dclutch-web/fixtures/direct-hot-bump-hints.json")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn core_identity(value: [u8; 32]) -> Identity {
    Identity::new(value).expect("distinct nonzero identity fill")
}

fn canonical_market_state() -> [u8; 368] {
    CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: MarketIdentity {
            market_id: core_identity(MARKET),
            realm_id: core_identity(REALM),
            product_record: core_identity(PRODUCT_RECORD),
            product_id: core_identity(PRODUCT_ID),
            resolution_policy: core_identity(RESOLUTION_POLICY),
            capability_manifest: core_identity(CAPABILITY_MANIFEST),
            selected_release_set: core_identity(RELEASE_SET),
            registry_program: core_identity(REGISTRY_PROGRAM),
            generation: GENERATION,
        },
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: core_identity(RENT_BENEFICIARY),
        terminal_receipt: None,
        // UNRECORDED on purpose. A Market that records its own bump makes the
        // Market hint INERT -- `state.bumps.market.or(hint)` reaches the record
        // and never the wire -- so a vector staging a recorded bump would be
        // pinning a byte no reader consumes. This one is the pre-tail shape,
        // which is exactly the case the Market slot still buys something on.
        bumps: StateBumpsV1::UNRECORDED,
    }
    .encode()
    .expect("canonical open Core Market state")
}

fn canonical_root_header() -> [u8; 232] {
    CapabilityRootHeaderV1::new(
        ContentId::new(RELEASE_SET).expect("release set"),
        MARKET,
        GENERATION,
        CapabilityExecutionSelectionV1::new(
            SELECTION_ENTRY_INDEX,
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
}

fn maker_replay(maker: [u8; 32]) -> (Pubkey, u8) {
    let coordinates = DirectCoordinatesV1::new(MARKET, GENERATION).expect("Direct coordinates");
    Pubkey::find_program_address(
        &MakerReplaySeedsV1::new(coordinates, maker)
            .expect("maker replay seeds")
            .as_slices(),
        &Pubkey::new_from_array(TRADING_PROGRAM),
    )
}

fn caller_authority_bump(context: [u8; 32], request_digest: [u8; 32]) -> u8 {
    Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(RELEASE_SET).expect("release set"),
            MARKET,
            ExecutionRoleV1::Trading,
            context,
            request_digest,
        )
        .expect("caller authority seeds")
        .as_slices(),
        &Pubkey::new_from_array(TRADING_PROGRAM),
    )
    .1
}

fn mined_hints() -> HotBumpHintsV1 {
    let state = CoreState::decode(&canonical_market_state()).expect("Core Market state decodes");
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        &Pubkey::new_from_array(CORE_PROGRAM),
    )
    .1;
    let header = CapabilityRootHeaderV1::decode(&canonical_root_header())
        .expect("capability root header decodes");
    let root = Pubkey::find_program_address(
        &header.seeds().as_slices(),
        &Pubkey::new_from_array(TRADING_PROGRAM),
    )
    .1;
    let seller = maker_replay(SELLER_MAKER);
    let buyer = maker_replay(BUYER_MAKER);
    let custody = Pubkey::new_from_array(CUSTODY_PROGRAM);
    HotBumpHintsV1 {
        market,
        root,
        lifecycle: [seller.1, buyer.1],
        child_caller: [
            caller_authority_bump(PARENT_REQUEST_DIGEST, CLAIMS_REQUEST_DIGEST),
            caller_authority_bump(buyer.0.to_bytes(), CUSTODY_REQUEST_DIGEST),
        ],
        child_relay: [
            Pubkey::find_program_address(
                &CustodyReplaySeedsV1::new(
                    MARKET,
                    RELEASE_SET,
                    CallerRoleV1::Trading,
                    buyer.0.to_bytes(),
                )
                .as_slices(),
                &custody,
            )
            .1,
            Pubkey::find_program_address(
                &CustodyAuthoritySeedsV1::new(MARKET, RELEASE_SET).as_slices(),
                &custody,
            )
            .1,
        ],
    }
}

fn rendered_vector() -> String {
    let hints = mined_hints();
    let buyer_root = maker_replay(BUYER_MAKER).0;
    format!(
        concat!(
            "{{\n",
            "  \"format\": \"dclutch/direct-hot-bump-hints/v1\",\n",
            "  \"note\": \"{note}\",\n",
            "  \"coreProgram\": \"{core}\",\n",
            "  \"tradingProgram\": \"{trading}\",\n",
            "  \"custodyProgram\": \"{custody}\",\n",
            "  \"market\": \"{market}\",\n",
            "  \"generation\": \"{generation}\",\n",
            "  \"releaseSetHex\": \"{release_set}\",\n",
            "  \"sellerMaker\": \"{seller}\",\n",
            "  \"buyerMaker\": \"{buyer}\",\n",
            "  \"marketCoreStateHex\": \"{state}\",\n",
            "  \"capabilityRootHeaderHex\": \"{header}\",\n",
            "  \"buyerMakerReplay\": \"{buyer_root}\",\n",
            "  \"childCaller\": {{\n",
            "    \"claimsContextHex\": \"{parent_digest}\",\n",
            "    \"claimsRequestDigestHex\": \"{claims_digest}\",\n",
            "    \"custodyContextIsBuyerMakerReplay\": true,\n",
            "    \"custodyRequestDigestHex\": \"{custody_digest}\"\n",
            "  }},\n",
            "  \"hintBlockHex\": \"{block}\"\n",
            "}}\n"
        ),
        note = NOTE,
        core = Pubkey::new_from_array(CORE_PROGRAM),
        trading = Pubkey::new_from_array(TRADING_PROGRAM),
        custody = Pubkey::new_from_array(CUSTODY_PROGRAM),
        market = Pubkey::new_from_array(MARKET),
        generation = GENERATION,
        release_set = hex(&RELEASE_SET),
        seller = Pubkey::new_from_array(SELLER_MAKER),
        buyer = Pubkey::new_from_array(BUYER_MAKER),
        state = hex(&canonical_market_state()),
        header = hex(&canonical_root_header()),
        buyer_root = buyer_root,
        parent_digest = hex(&PARENT_REQUEST_DIGEST),
        claims_digest = hex(&CLAIMS_REQUEST_DIGEST),
        custody_digest = hex(&CUSTODY_REQUEST_DIGEST),
        block = hex(&hints.to_bytes()),
    )
}

#[test]
fn browser_bump_hint_vector_matches_the_live_seed_constructors() {
    let rendered = rendered_vector();
    let path = vector_path();
    if env::var_os("DCLUTCH_WRITE_WIRE_VECTOR").is_some() {
        fs::write(&path, &rendered).expect("write bump hint vector");
        fs::write(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/dclutch-sdk/fixtures/direct-hot-bump-hints.json"),
            &rendered,
        )
        .expect("write SDK bump hint vector");
        // WRITING IS NOT PASSING. This branch exists so a deliberate move can
        // land, and it used to `return` -- which made one environment variable
        // enough to turn "the wire moved and nobody noticed" into a green run
        // on BOTH sides at once: regenerate, and the encoder, the fixture and
        // the browser mirror agree again about bytes nobody read. The test
        // cannot verify its own regeneration, so it refuses instead, and the
        // pin in tools/gates/wire-vector-pins.tsv -- which no test can write -- is
        // what a human moves in the same commit.
        panic!(
            "DCLUTCH_WRITE_WIRE_VECTOR=1 wrote the regenerated vector. This is a \
             REFUSAL, not a failure of the encoder.\n\
             The checked-in bytes have changed and nothing has reviewed them yet. \
             Run\n\
             \n    python3 tools/gate emission --pins --update\n\n\
             and commit the regenerated fixture AND the moved pin together, with \
             the digests\n\
             in the message. Separately, each half reads as an accident to \
             whoever finds it next.\n\
             Then re-run this test WITHOUT DCLUTCH_WRITE_WIRE_VECTOR to confirm \
             the encoder and\n\
             the fixture agree, and expect the browser mirror to stay red until \
             it catches up."
        );
    }
    let recorded = fs::read_to_string(&path).expect("bump hint vector is present");
    assert_eq!(
        recorded, rendered,
        "a caller-mined bump seed order moved; regenerate with DCLUTCH_WRITE_WIRE_VECTOR=1 and update the browser miner in the same commit"
    );
}

#[test]
fn every_mined_slot_is_nonzero_and_the_block_is_the_envelope_tail() {
    // Zero is ABSENT, not a value, so a vector whose slot mined to zero would
    // be pinning "this caller searched" rather than "these two languages agree".
    let block = mined_hints().to_bytes();
    assert_eq!(block.len(), HOT_BUMP_HINT_COUNT_V1);
    for (slot, byte) in block.into_iter().enumerate() {
        assert_ne!(byte, 0, "slot {slot} mined to absent");
    }
    assert_eq!(
        CAPABILITY_EXECUTION_SELECTION_BYTES_V1, 144,
        "the browser reads the root selection at this width"
    );
}
