// Integration-test crate: clippy's `allow-*-in-tests` settings only reach
// `#[cfg(test)]` modules, so the same test-only ergonomics are allowed here
// explicitly. Non-test code in `src/` is held to the full bar.
#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::panic
)]

//! One board, exercised through the surface a caller actually reaches.
//!
//! THE CENTRAL CLAIM UNDER TEST is that this service adds no validation of its
//! own and lifts the real one. That claim is only worth anything if a HOSTILE
//! ticket dies on the lifted code, so
//! [`a_tampered_signed_field_dies_on_the_lifted_signature_check`] and its
//! sibling are the tests the rest of the file exists to support: they mutate a
//! signed field and a signature byte after authoring, and require the refusal
//! to name the signature specifically. A board that had merely re-checked JSON
//! shape would pass every other test here and fail those two.
//!
//! Every refusal is asserted BY NAME, on the variant. An `is_err()` assertion
//! would pass on whatever the service happened to refuse first and would prove
//! nothing about which wall was hit.

use std::sync::Mutex;

use dclutch_direct_codec::intent_v2::CompactIntentV2;
use dclutch_direct_ticket::{
    encode_portable_direct_ticket_v1, parse_portable_direct_ticket_v1, sign_direct_intent_v1,
};
use dclutch_ticket_board::{
    board::{BoardRefusalV1, BoardStateV1, ListingQueryV1},
    http::{BOARD_NOTICE_V1, handle_v1},
    snapshot::{SNAPSHOT_SCHEMA_V1, load_snapshot_v1, write_snapshot_v1},
};
use solana_keypair::Keypair;
use solana_program::pubkey::Pubkey;

const MARKET_V1: &str = "5F8wMRFMdYGMkjWQUye6WfbgRVWEo9yyKo9aFPk2TLaD";
const OTHER_MARKET_V1: &str = "8bcRzB3v6PxbbtkVCiX9ceW2whwakA6gX7qvSYbeMHLq";
const COLLATERAL_V1: &str = "7xwJ3uceuBV7KyCsdJsBs9Ljfh1bL3WB7NbGpwUNeJ2o";

fn maker_v1(seed: u8) -> Keypair {
    Keypair::new_from_array([seed; 32])
}

/// Author one real ticket: a real signature over a real preimage.
fn ticket_v1(
    keypair: &Keypair,
    market: &str,
    outcome: u32,
    nonce: u64,
    valid_through: u64,
) -> String {
    let intent = CompactIntentV2 {
        side: 0,
        lifecycle: 1,
        outcome,
        market: market.parse::<Pubkey>().unwrap().to_bytes(),
        generation: 7,
        nonce,
        valid_from: 11,
        valid_through,
        maximum_fill: 100_000_000,
        limit_price: 500_000,
        fee_basis_points: 50,
        collateral_account: COLLATERAL_V1.parse::<Pubkey>().unwrap().to_bytes(),
    };
    let signed = sign_direct_intent_v1(keypair, intent).unwrap();
    encode_portable_direct_ticket_v1(&signed).unwrap()
}

fn board_v1() -> Mutex<BoardStateV1> {
    Mutex::new(BoardStateV1::new(None))
}

fn field_v1(body: &str, name: &str) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(body).unwrap()[name].clone()
}

// ---------------------------------------------------------------------------
// The round trip: author -> post -> list -> the shape step ③ contracts for.
// ---------------------------------------------------------------------------

#[test]
fn an_authored_ticket_posts_lists_and_decodes_back_to_the_intent_that_was_signed() {
    let board = board_v1();
    let keypair = maker_v1(1);
    let text = ticket_v1(&keypair, MARKET_V1, 3, 9, 4_294_967_295);

    let posted = handle_v1("POST", "/tickets", text.as_bytes(), &board);
    assert_eq!(posted.status, 201, "{}", posted.body);
    assert_eq!(field_v1(&posted.body, "accepted"), serde_json::json!(true));
    assert_eq!(
        field_v1(&posted.body, "duplicate"),
        serde_json::json!(false)
    );

    let listed = handle_v1("GET", &format!("/tickets?market={MARKET_V1}"), b"", &board);
    assert_eq!(listed.status, 200, "{}", listed.body);
    let offers = field_v1(&listed.body, "offers");
    let offers = offers.as_array().unwrap();
    assert_eq!(offers.len(), 1);

    // The board stored the maker's bytes VERBATIM: a relay that re-encoded a
    // ticket would be a second writer of a canonical shape.
    let served = offers[0]["text"].as_str().unwrap();
    assert_eq!(
        served, text,
        "the board did not serve the authored bytes back"
    );

    // And step ③'s contract: one `SignedDirectIntentV3`, by any means. What
    // came off the board decodes to exactly what was signed.
    let from_board = parse_portable_direct_ticket_v1(served.as_bytes(), "listed").unwrap();
    let from_author = parse_portable_direct_ticket_v1(text.as_bytes(), "authored").unwrap();
    assert_eq!(from_board, from_author);
    assert_eq!(from_board.maker, keypair_pubkey_v1(&keypair));
    assert_eq!(from_board.intent.outcome, 3);

    // The digest names the offer, and it is the SHA-256 of those exact bytes.
    assert_eq!(
        offers[0]["digest"].as_str().unwrap(),
        dclutch_direct_ticket::sha256_hex_v1(text.as_bytes())
    );
}

fn keypair_pubkey_v1(keypair: &Keypair) -> Pubkey {
    use solana_signer::Signer as _;
    keypair.pubkey()
}

#[test]
fn the_same_ticket_posted_twice_is_held_once_and_named_a_duplicate() {
    let board = board_v1();
    let text = ticket_v1(&maker_v1(2), MARKET_V1, 0, 1, 4_294_967_295);

    let first = handle_v1("POST", "/tickets", text.as_bytes(), &board);
    let second = handle_v1("POST", "/tickets", text.as_bytes(), &board);
    assert_eq!(first.status, 201);
    assert_eq!(second.status, 201, "a re-post is not an error");
    assert_eq!(field_v1(&second.body, "duplicate"), serde_json::json!(true));
    assert_eq!(
        field_v1(&first.body, "digest"),
        field_v1(&second.body, "digest")
    );
    assert_eq!(board.lock().unwrap().len(), 1);
}

#[test]
fn offers_are_listed_newest_first() {
    let board = board_v1();
    let mut digests = Vec::new();
    for nonce in 0..3u64 {
        let text = ticket_v1(&maker_v1(3), MARKET_V1, 1, nonce, 4_294_967_295);
        digests.push(dclutch_direct_ticket::sha256_hex_v1(text.as_bytes()));
        assert_eq!(
            handle_v1("POST", "/tickets", text.as_bytes(), &board).status,
            201
        );
    }
    let listed = handle_v1("GET", &format!("/tickets?market={MARKET_V1}"), b"", &board);
    let offers = field_v1(&listed.body, "offers");
    let served: Vec<&str> = offers
        .as_array()
        .unwrap()
        .iter()
        .map(|offer| offer["digest"].as_str().unwrap())
        .collect();
    digests.reverse();
    assert_eq!(served, digests, "the board must answer newest first");
}

#[test]
fn the_outcome_filter_selects_one_claim() {
    let board = board_v1();
    for outcome in 0..3u32 {
        let text = ticket_v1(
            &maker_v1(4),
            MARKET_V1,
            outcome,
            u64::from(outcome),
            4_294_967_295,
        );
        handle_v1("POST", "/tickets", text.as_bytes(), &board);
    }
    let listed = handle_v1(
        "GET",
        &format!("/tickets?market={MARKET_V1}&outcome=2"),
        b"",
        &board,
    );
    let offers = field_v1(&listed.body, "offers");
    assert_eq!(offers.as_array().unwrap().len(), 1);

    // A market with nothing on it is an empty list, not a refusal.
    let empty = handle_v1(
        "GET",
        &format!("/tickets?market={OTHER_MARKET_V1}"),
        b"",
        &board,
    );
    assert_eq!(empty.status, 200);
    assert_eq!(field_v1(&empty.body, "offers"), serde_json::json!([]));
}

// ---------------------------------------------------------------------------
// The hostile pair. These are the tests that prove the lift is real.
// ---------------------------------------------------------------------------

#[test]
fn a_tampered_signed_field_dies_on_the_lifted_signature_check() {
    let board = board_v1();
    let honest = ticket_v1(&maker_v1(5), MARKET_V1, 3, 9, 4_294_967_295);

    // Move the price the maker signed. Same length, still canonical decimal,
    // still perfectly well-formed JSON — so ONLY a real signature check can
    // catch it. This is the attack the whole invariant rests on being refused.
    let tampered = honest.replace("\"limitPrice\": \"500000\"", "\"limitPrice\": \"400000\"");
    assert_ne!(
        tampered, honest,
        "the fixture must actually have been edited"
    );
    serde_json::from_str::<serde_json::Value>(&tampered).expect("still valid JSON");

    let refusal = board
        .lock()
        .unwrap()
        .admit_v1(tampered.as_bytes(), None)
        .expect_err("a tampered signed field must never be admitted");

    match &refusal {
        BoardRefusalV1::TicketMalformed { sentence } => assert!(
            sentence.contains("detached signature did not verify"),
            "the refusal must name the SIGNATURE, not merely the shape: {sentence}"
        ),
        other => panic!("expected TICKET_MALFORMED, got {other:?}"),
    }
    assert_eq!(refusal.name(), "TICKET_MALFORMED");
    assert_eq!(refusal.status(), 400);
    assert!(board.lock().unwrap().is_empty());

    // The honest original still posts, so the refusal above was about the
    // tampering and not about the fixture being broken all along.
    assert_eq!(
        handle_v1("POST", "/tickets", honest.as_bytes(), &board).status,
        201
    );
}

#[test]
fn a_tampered_signature_is_refused_by_name() {
    let board = board_v1();
    let honest = ticket_v1(&maker_v1(6), MARKET_V1, 1, 4, 4_294_967_295);
    let signed = parse_portable_direct_ticket_v1(honest.as_bytes(), "honest").unwrap();

    // Flip one hex digit of the 128-character detached signature.
    let hex = dclutch_direct_ticket::hex_lower_v1(&signed.signature);
    let flipped = format!(
        "{}{}",
        if hex.starts_with('0') { '1' } else { '0' },
        &hex[1..]
    );
    let tampered = honest.replace(&hex, &flipped);
    assert_ne!(tampered, honest);

    let refusal = board
        .lock()
        .unwrap()
        .admit_v1(tampered.as_bytes(), None)
        .expect_err("a tampered signature must never be admitted");
    match &refusal {
        BoardRefusalV1::TicketMalformed { sentence } => assert!(
            sentence.contains("detached signature did not verify"),
            "{sentence}"
        ),
        other => panic!("expected TICKET_MALFORMED, got {other:?}"),
    }
}

#[test]
fn a_ticket_signed_by_a_different_key_than_it_names_is_refused() {
    let board = board_v1();
    let mine = ticket_v1(&maker_v1(7), MARKET_V1, 1, 4, 4_294_967_295);
    let theirs = ticket_v1(&maker_v1(8), MARKET_V1, 1, 4, 4_294_967_295);

    let mine_signed = parse_portable_direct_ticket_v1(mine.as_bytes(), "mine").unwrap();
    let theirs_signed = parse_portable_direct_ticket_v1(theirs.as_bytes(), "theirs").unwrap();

    // Keep my signature, claim their identity: the intent bytes are identical,
    // so this is a pure impersonation attempt.
    let forged = mine.replace(
        &mine_signed.maker.to_string(),
        &theirs_signed.maker.to_string(),
    );
    let refusal = board
        .lock()
        .unwrap()
        .admit_v1(forged.as_bytes(), None)
        .expect_err("a signature must not verify under a stranger's key");
    assert_eq!(refusal.name(), "TICKET_MALFORMED");
}

// ---------------------------------------------------------------------------
// Expiry.
// ---------------------------------------------------------------------------

#[test]
fn an_offer_past_its_window_is_dropped_from_the_listing_and_counted() {
    let board = board_v1();
    let short = ticket_v1(&maker_v1(9), MARKET_V1, 1, 1, 500);
    let long = ticket_v1(&maker_v1(9), MARKET_V1, 1, 2, 5_000);
    for text in [&short, &long] {
        assert_eq!(
            handle_v1("POST", "/tickets", text.as_bytes(), &board).status,
            201
        );
    }

    // With no slot the board judges no expiry, and says so rather than
    // pretending to a clock it does not have.
    let unjudged = handle_v1("GET", &format!("/tickets?market={MARKET_V1}"), b"", &board);
    assert_eq!(
        field_v1(&unjudged.body, "offers").as_array().unwrap().len(),
        2
    );
    assert_eq!(
        field_v1(&unjudged.body, "slotBasis"),
        serde_json::Value::Null
    );
    assert_eq!(
        field_v1(&unjudged.body, "droppedExpired"),
        serde_json::json!(0)
    );

    // At slot 1000 the short offer is gone, and the drop is COUNTABLE so the
    // UI can say "1 offer hidden — why?" rather than silently shrinking.
    let judged = handle_v1(
        "GET",
        &format!("/tickets?market={MARKET_V1}&slot=1000"),
        b"",
        &board,
    );
    let offers = field_v1(&judged.body, "offers");
    let offers = offers.as_array().unwrap();
    assert_eq!(offers.len(), 1);
    assert_eq!(offers[0]["text"].as_str().unwrap(), long);
    assert_eq!(
        field_v1(&judged.body, "slotBasis"),
        serde_json::json!("1000")
    );
    assert_eq!(
        field_v1(&judged.body, "droppedExpired"),
        serde_json::json!(1)
    );

    // Filtering is a READ, never a mutation: the expired offer is still held,
    // so one caller's slot can never expire another caller's view. That is the
    // censorship lever this board declines to build.
    assert_eq!(board.lock().unwrap().len(), 2);
    let inclusive = handle_v1(
        "GET",
        &format!("/tickets?market={MARKET_V1}&slot=500"),
        b"",
        &board,
    );
    assert_eq!(
        field_v1(&inclusive.body, "offers")
            .as_array()
            .unwrap()
            .len(),
        2,
        "validThrough is the last valid slot, inclusive"
    );
}

#[test]
fn posting_an_already_expired_ticket_is_refused_by_name() {
    let board = board_v1();
    let text = ticket_v1(&maker_v1(10), MARKET_V1, 1, 1, 500);
    let response = handle_v1("POST", "/tickets?slot=900", text.as_bytes(), &board);
    assert_eq!(response.status, 400);
    assert_eq!(
        field_v1(&response.body, "refusal"),
        serde_json::json!("EXPIRED")
    );

    let refusal = board
        .lock()
        .unwrap()
        .admit_v1(text.as_bytes(), Some(900))
        .expect_err("already expired");
    match refusal {
        BoardRefusalV1::Expired {
            valid_through,
            at_slot,
        } => {
            assert_eq!(valid_through, 500);
            assert_eq!(at_slot, 900);
        }
        other => panic!("expected EXPIRED, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The rest of the refusals, each by name.
// ---------------------------------------------------------------------------

#[test]
fn a_board_pinned_to_one_market_refuses_every_other_by_name() {
    let board = Mutex::new(BoardStateV1::new(Some(MARKET_V1.to_owned())));
    let elsewhere = ticket_v1(&maker_v1(11), OTHER_MARKET_V1, 1, 1, 4_294_967_295);

    let refusal = board
        .lock()
        .unwrap()
        .admit_v1(elsewhere.as_bytes(), None)
        .expect_err("this board serves one Market");
    match &refusal {
        BoardRefusalV1::MarketNotServed { served, offered } => {
            assert_eq!(served, MARKET_V1);
            assert_eq!(offered, OTHER_MARKET_V1);
        }
        other => panic!("expected MARKET_NOT_SERVED, got {other:?}"),
    }
    assert_eq!(refusal.name(), "MARKET_NOT_SERVED");

    let here = ticket_v1(&maker_v1(11), MARKET_V1, 1, 1, 4_294_967_295);
    assert_eq!(
        handle_v1("POST", "/tickets", here.as_bytes(), &board).status,
        201
    );
}

#[test]
fn a_body_above_the_ticket_bound_is_refused_before_it_is_parsed() {
    let board = board_v1();
    let oversized = vec![b'{'; 5_000];
    let refusal = board
        .lock()
        .unwrap()
        .admit_v1(&oversized, None)
        .expect_err("above the codec's own bound");
    match refusal {
        BoardRefusalV1::BodyTooLarge { received } => assert_eq!(received, 5_000),
        other => panic!("expected BODY_TOO_LARGE, got {other:?}"),
    }
    assert_eq!(
        handle_v1("POST", "/tickets", &oversized, &board).status,
        413
    );
}

#[test]
fn a_full_board_refuses_the_new_offer_rather_than_evicting_an_old_one() {
    let board = Mutex::new(BoardStateV1::with_capacity_v1(None, 2));
    let held: Vec<String> = (0..2u64)
        .map(|nonce| ticket_v1(&maker_v1(12), MARKET_V1, 1, nonce, 4_294_967_295))
        .collect();
    for text in &held {
        assert_eq!(
            handle_v1("POST", "/tickets", text.as_bytes(), &board).status,
            201
        );
    }

    let overflow = ticket_v1(&maker_v1(12), MARKET_V1, 1, 99, 4_294_967_295);
    let response = handle_v1("POST", "/tickets", overflow.as_bytes(), &board);
    assert_eq!(
        response.status, 503,
        "a full board is unavailable, not wrong"
    );
    assert_eq!(
        field_v1(&response.body, "refusal"),
        serde_json::json!("BOARD_FULL")
    );

    // The point of refusing instead of evicting: a flood cannot push honest
    // offers off the board, which would be exactly the censorship a relay is
    // otherwise structurally incapable of.
    let listed = handle_v1("GET", &format!("/tickets?market={MARKET_V1}"), b"", &board);
    let offers = field_v1(&listed.body, "offers");
    let offers = offers.as_array().unwrap();
    assert_eq!(offers.len(), 2);
    for text in &held {
        assert!(
            offers
                .iter()
                .any(|offer| offer["text"].as_str().unwrap() == text),
            "an existing offer was evicted to make room"
        );
    }
}

#[test]
fn every_malformed_query_is_refused_by_name_and_never_guessed() {
    let board = board_v1();
    for target in [
        "/tickets",
        "/tickets?outcome=1",
        "/tickets?market=not-base58",
        &format!("/tickets?market={MARKET_V1}&outcome=-1"),
        &format!("/tickets?market={MARKET_V1}&outcome=007"),
        &format!("/tickets?market={MARKET_V1}&slot=abc"),
        &format!("/tickets?market={MARKET_V1}&slot=99999999999999999999999"),
        &format!("/tickets?market={MARKET_V1}&outcome=4294967296"),
    ] {
        let response = handle_v1("GET", target, b"", &board);
        assert_eq!(response.status, 400, "{target} was not refused");
        assert_eq!(
            field_v1(&response.body, "refusal"),
            serde_json::json!("QUERY_INVALID"),
            "{target}"
        );
    }
}

#[test]
fn unknown_routes_and_methods_are_refused_by_name() {
    let board = board_v1();
    let missing = handle_v1("GET", "/offers", b"", &board);
    assert_eq!(missing.status, 404);
    assert_eq!(
        field_v1(&missing.body, "refusal"),
        serde_json::json!("ROUTE_NOT_FOUND")
    );

    // No DELETE, and this matters: a relay with a delete route is a relay that
    // can be made to censor by whoever reaches it.
    let deleted = handle_v1("DELETE", "/tickets", b"", &board);
    assert_eq!(deleted.status, 405);
    assert_eq!(
        field_v1(&deleted.body, "refusal"),
        serde_json::json!("METHOD_NOT_ALLOWED")
    );
}

// ---------------------------------------------------------------------------
// The board never claims authority it lacks.
// ---------------------------------------------------------------------------

#[test]
fn no_response_ever_calls_an_offer_verified_or_valid() {
    let board = board_v1();
    let text = ticket_v1(&maker_v1(13), MARKET_V1, 1, 1, 4_294_967_295);
    handle_v1("POST", "/tickets", text.as_bytes(), &board);

    for response in [
        handle_v1("GET", "/health", b"", &board),
        handle_v1("GET", &format!("/tickets?market={MARKET_V1}"), b"", &board),
    ] {
        // The ticket TEXT legitimately contains none of these words, so this
        // guard is not vacuous — it reads the whole body on purpose.
        for forbidden in ["verified", "\"valid\"", "confirmed", "guaranteed"] {
            assert!(
                !response.body.contains(forbidden),
                "a board must not claim `{forbidden}`: {}",
                response.body
            );
        }
        assert!(
            response.body.contains("a relay can hide an offer"),
            "every board state carries the standing honesty line"
        );
    }
    assert!(BOARD_NOTICE_V1.contains("not by the chain"));
}

#[test]
fn health_reports_the_holdings_and_names_its_missing_clock() {
    let board = Mutex::new(BoardStateV1::new(Some(MARKET_V1.to_owned())));
    let text = ticket_v1(&maker_v1(14), MARKET_V1, 1, 1, 4_294_967_295);
    handle_v1("POST", "/tickets", text.as_bytes(), &board);

    let health = handle_v1("GET", "/health", b"", &board);
    assert_eq!(health.status, 200);
    assert_eq!(field_v1(&health.body, "status"), serde_json::json!("ok"));
    assert_eq!(field_v1(&health.body, "offers"), serde_json::json!(1));
    assert_eq!(
        field_v1(&health.body, "servedMarket"),
        serde_json::json!(MARKET_V1)
    );
    // Null and present, not absent: "this board has no clock" is an answer.
    assert_eq!(
        field_v1(&health.body, "observedSlot"),
        serde_json::Value::Null
    );
}

// ---------------------------------------------------------------------------
// The snapshot.
// ---------------------------------------------------------------------------

#[test]
fn a_snapshot_round_trips_and_every_row_is_revalidated_on_load() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("board.json");

    let board = board_v1();
    let texts: Vec<String> = (0..3u64)
        .map(|nonce| ticket_v1(&maker_v1(15), MARKET_V1, 1, nonce, 4_294_967_295))
        .collect();
    for text in &texts {
        handle_v1("POST", "/tickets?slot=42", text.as_bytes(), &board);
    }
    write_snapshot_v1(&path, &board.lock().unwrap()).unwrap();

    let mut restored = BoardStateV1::new(None);
    let load = load_snapshot_v1(&path, &mut restored).unwrap();
    assert_eq!(load.restored, 3);
    assert!(load.refused.is_empty());
    let listing = restored.list_v1(&ListingQueryV1 {
        market: MARKET_V1.to_owned(),
        outcome: None,
        slot: None,
    });
    assert_eq!(listing.offers.len(), 3);
    // Arrival order survives the round trip, so "newest first" still means it.
    assert_eq!(listing.offers[0].text, texts[2]);
    assert_eq!(listing.offers[0].posted_at_slot, Some(42));

    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["schema"], serde_json::json!(SNAPSHOT_SCHEMA_V1));
}

#[test]
fn a_hand_edited_snapshot_cannot_inject_an_offer_the_reader_would_refuse() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("board.json");

    let board = board_v1();
    let honest = ticket_v1(&maker_v1(16), MARKET_V1, 1, 1, 4_294_967_295);
    handle_v1("POST", "/tickets", honest.as_bytes(), &board);
    write_snapshot_v1(&path, &board.lock().unwrap()).unwrap();

    // Edit the file the way an attacker with disk access would: move a signed
    // field and leave the signature alone.
    let doctored = std::fs::read_to_string(&path).unwrap().replace(
        "\\\"limitPrice\\\": \\\"500000\\\"",
        "\\\"limitPrice\\\": \\\"1\\\"",
    );
    assert!(
        !doctored.contains("\\\"limitPrice\\\": \\\"500000\\\""),
        "the snapshot edit must actually have landed"
    );
    std::fs::write(&path, &doctored).unwrap();

    let mut restored = BoardStateV1::new(None);
    let load = load_snapshot_v1(&path, &mut restored).unwrap();
    assert_eq!(load.restored, 0, "a forged row must not be restored");
    assert_eq!(load.refused.len(), 1);
    assert!(
        load.refused[0].starts_with("TICKET_MALFORMED"),
        "{:?}",
        load.refused
    );
    assert!(restored.is_empty());
}

#[test]
fn a_missing_snapshot_is_the_first_run_and_not_an_error() {
    let directory = tempfile::tempdir().unwrap();
    let mut board = BoardStateV1::new(None);
    let load = load_snapshot_v1(&directory.path().join("absent.json"), &mut board).unwrap();
    assert_eq!(load.restored, 0);
    assert!(load.refused.is_empty());
}

#[test]
fn a_failed_snapshot_write_leaves_the_last_accepted_one_intact() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("board.json");
    let board = board_v1();
    let text = ticket_v1(&maker_v1(17), MARKET_V1, 1, 1, 4_294_967_295);
    handle_v1("POST", "/tickets", text.as_bytes(), &board);
    write_snapshot_v1(&path, &board.lock().unwrap()).unwrap();
    let accepted = std::fs::read(&path).unwrap();

    // A directory where the temporary file wants to be: the write fails and
    // must not touch the canonical path.
    std::fs::create_dir(directory.path().join("board.json.writing")).unwrap();
    write_snapshot_v1(&path, &board.lock().unwrap())
        .expect_err("the staged write cannot succeed here");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        accepted,
        "a failed write must leave the last accepted snapshot byte-for-byte intact"
    );
}
