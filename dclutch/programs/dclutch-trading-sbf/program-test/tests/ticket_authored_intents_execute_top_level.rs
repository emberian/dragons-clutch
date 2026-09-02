//! The ticket a maker signs off-chain is the thing `hot_v3` executes.
//!
//! The portable Direct ticket (`crates/dclutch-direct-ticket`, and
//! `encodeDirectIntentTicketV1` in the browser SDK) is how a maker's intent
//! actually travels: `dclutch ticket author` writes one JSON file, a board or
//! a counterparty carries it, and a producer settles a PAIR of them. Every
//! link in that chain had a test except the last one. The ticket crate proved
//! its bytes against a cross-language vector, the CLI proved it invokes that
//! crate, and the Direct program-tests proved `hot_v3` executes -- over
//! signatures produced two lines above the transaction, from intents the
//! fixture had just built in memory.
//!
//! So the preimage a ticket commits to and the preimage the program
//! authenticates were only ever the same by construction, in two separate
//! constructions, and a divergence between them is precisely the failure the
//! ticket crate's own header says it exists to prevent: "a second
//! implementation of a signing preimage is a signature that verifies nowhere,
//! discovered at the refused trade."
//!
//! This file closes it with a round trip through the filesystem. The fixture's
//! two signed preimages are decoded back into `CompactIntentV2` by the codec
//! that owns them, authored into two real ticket files by the shared author,
//! read off disk by `parse_portable_direct_ticket_v1` -- the same reader
//! `dclutch ticket verify` and the trade producer run -- and the detached
//! signatures those files carry are what the Ed25519 program verifies. The
//! trade then executes on the real Trading ELF, top-level, and the assertion
//! that matters is that it executes at all: an intent whose ticket round trip
//! changed a single byte cannot produce a signature that verifies.

use std::fs;

use dclutch_direct_codec::intent_v2::CompactIntentV2;
use dclutch_direct_hot_program_test_support::waist::{
    COMPUTE_LIMIT, TRADING_PROGRAM_ID, add_lookup_table, add_release_waist,
    canonical_lookup_addresses, direct_case, direct_top_level_instructions_with_signatures, elves,
    fixture_substrate, program_test_without_forced_budget, start_with_substrate,
    submit_v0_observed,
};
use dclutch_direct_ticket::{
    encode_portable_direct_ticket_v1, parse_portable_direct_ticket_v1, sign_direct_intent_v1,
};
use dclutch_token_svm::TokenAccount;
use solana_program::pubkey::Pubkey;
use solana_program_test::ProgramTestContext;
use solana_sdk::signature::Signer;

async fn account(context: &mut ProgramTestContext, key: Pubkey) -> solana_account::Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("live account")
}

#[tokio::test]
async fn ticket_authored_intents_execute_top_level() {
    let artifacts = elves();
    let mut test = program_test_without_forced_budget(&artifacts);
    let releases = add_release_waist(&mut test, &artifacts);
    let direct = direct_case(&mut test, releases, &artifacts, false);

    // A directory this test owns, named after the test, so a failed run leaves
    // the exact two files a reader can open with `dclutch ticket verify`.
    let directory = std::env::temp_dir().join(format!("dclutch-ticket-hot-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("ticket directory");

    let mut signatures = [[0_u8; 64]; 2];
    for (index, side) in ["seller", "buyer"].into_iter().enumerate() {
        // The intent the FIXTURE signed, recovered by the codec that owns the
        // preimage rather than rebuilt from the fixture's constants. If this
        // decode ever disagreed with `signed_preimage`, the round trip below
        // would produce a different message and the Ed25519 program would
        // refuse -- which is the property under test, not an assumption of it.
        let intent = CompactIntentV2::decode_signed_preimage(&direct.chain.signed_messages[index])
            .expect("the fixture's signed preimage decodes as a CompactIntentV2");

        let signed = sign_direct_intent_v1(&direct.makers[index], intent)
            .expect("author one Direct intent ticket");
        let path = directory.join(format!("{side}.json"));
        fs::write(
            &path,
            encode_portable_direct_ticket_v1(&signed).expect("portable ticket bytes"),
        )
        .expect("write the ticket file");

        // Everything above this line is authoring. Everything below reads the
        // file back the way a counterparty would, with no access to the
        // keypair, the fixture, or the intent that produced it.
        let bytes = fs::read(&path).expect("read the ticket file back");
        let parsed = parse_portable_direct_ticket_v1(&bytes, &path.display().to_string())
            .expect("the authored ticket parses and its signature verifies");
        assert_eq!(
            parsed.maker,
            direct.makers[index].pubkey(),
            "the ticket names a different maker than the one who signed it",
        );
        assert_eq!(
            parsed
                .intent
                .signed_preimage()
                .expect("preimage of the parsed intent")
                .as_slice(),
            direct.chain.signed_messages[index].as_slice(),
            "a ticket round trip changed the {side} preimage the program authenticates",
        );
        signatures[index] = parsed.signature;
    }

    let instructions = direct_top_level_instructions_with_signatures(&direct, signatures);
    assert_eq!(
        instructions[3].program_id, TRADING_PROGRAM_ID,
        "this test must submit to Trading directly, not through an outer",
    );
    let addresses = canonical_lookup_addresses(&instructions, Pubkey::default());
    add_lookup_table(&mut test, &addresses);
    let mut context = start_with_substrate(test, fixture_substrate()).await;

    let execution = submit_v0_observed(
        &mut context,
        &instructions,
        addresses,
        Some(&direct.payer),
        &[],
    )
    .await
    .expect("a Direct trade whose two intents came off disk as authored tickets");

    let units = execution.compute_units_consumed;
    assert!(
        units > 0 && units <= COMPUTE_LIMIT,
        "ticket-authored Direct Hot consumed {units} units against a {COMPUTE_LIMIT} limit",
    );
    println!("ticket-authored top-level Direct Hot compute units consumed: {units}");

    // The collateral moved, which is the same trade the fixture-signed test
    // asserts. Stated here too because "it executed" and "it executed the
    // trade the tickets described" are different claims, and only the second
    // one is worth the round trip.
    assert_eq!(
        TokenAccount::parse(
            &account(&mut context, direct.chain.collateral_accounts[0])
                .await
                .data
        )
        .expect("source token")
        .amount,
        95,
    );
    assert_eq!(
        TokenAccount::parse(
            &account(&mut context, direct.chain.collateral_accounts[1])
                .await
                .data
        )
        .expect("destination token")
        .amount,
        35,
    );

    fs::remove_dir_all(&directory).ok();
}
