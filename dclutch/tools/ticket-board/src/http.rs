//! The wire: three routes, and the JSON each one answers with.
//!
//! ROUTING IS A `match`, NOT A FRAMEWORK. Three routes do not need a router,
//! and [`handle_v1`] is deliberately a plain synchronous function over
//! `(method, target, body)` rather than anything tied to a server type. That
//! is what lets every refusal in `board.rs` be tested by name without binding a
//! socket, which is the difference between testing this service and testing
//! whether tokio works.
//!
//! WHAT THE RESPONSES PROMISE. A listed offer is WELL-FORMED and CORRECTLY
//! SIGNED — nothing here reads chain state, so nothing here may be rendered as
//! "verified". No response field says `valid`, and none ever should: only the
//! chain verifies, by re-deriving the signing message at execution. The
//! client's own decoder re-reads every `text` this service emits, so the
//! board's opinion of a ticket reaches no further than its own admission.

use std::sync::Mutex;

use serde::Serialize;

use crate::board::{BoardRefusalV1, BoardStateV1, ListingQueryV1};

/// One answer, ready to write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponseV1 {
    /// HTTP status code.
    pub status: u16,
    /// The JSON body. Every response on every path is JSON.
    pub body: String,
}

/// One offer, on the wire.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OfferWireV1 {
    digest: String,
    /// The exact ticket text, verbatim, for the client's own decoder to read.
    text: String,
    /// Canonical decimal, because a u64 slot does not survive JavaScript's
    /// number type and a board that rounded a slot would be lying quietly.
    posted_at_slot: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListingWireV1 {
    offers: Vec<OfferWireV1>,
    slot_basis: Option<String>,
    dropped_expired: usize,
    /// The standing honesty line, carried by the protocol rather than left to
    /// each client to remember. A board that only documented its own limits
    /// would depend on every consumer choosing to repeat them.
    notice: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptedWireV1 {
    accepted: bool,
    digest: String,
    duplicate: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RefusedWireV1 {
    accepted: bool,
    /// The stable machine name of the refusal.
    refusal: &'static str,
    /// The sentence a human should read.
    reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthWireV1 {
    status: &'static str,
    offers: usize,
    capacity: usize,
    /// The Market this board serves, or `null` for every Market.
    served_market: Option<String>,
    /// Always `null`: this board reads no chain and holds no clock.
    ///
    /// The field exists so a client need not special-case a board that has one
    /// later, and it is `null` rather than absent so that "no clock" is an
    /// answer rather than an omission.
    observed_slot: Option<String>,
    notice: &'static str,
}

/// The standing line every board state shows.
///
/// It is one fixed string, in one place, because a board's honesty is a fixed
/// string or it is decoration.
pub const BOARD_NOTICE_V1: &str = "Offers are collected by a relay, not by the chain. \
     The chain checks every signature when the trade executes — a relay can hide an offer \
     from you, but it cannot change one.";

fn json_or_refusal(value: &impl Serialize, status: u16) -> HttpResponseV1 {
    match serde_json::to_string(value) {
        Ok(body) => HttpResponseV1 { status, body },
        // Encoding a struct of owned Strings cannot fail, but this crate denies
        // `unwrap_used` and an unreachable branch is cheaper than an exception
        // to the lint.
        Err(error) => HttpResponseV1 {
            status: 500,
            body: format!("{{\"accepted\":false,\"refusal\":\"ENCODE\",\"reason\":\"{error}\"}}"),
        },
    }
}

fn refused(refusal: &BoardRefusalV1) -> HttpResponseV1 {
    json_or_refusal(
        &RefusedWireV1 {
            accepted: false,
            refusal: refusal.name(),
            reason: refusal.sentence(),
        },
        refusal.status(),
    )
}

fn query_invalid(sentence: impl Into<String>) -> HttpResponseV1 {
    refused(&BoardRefusalV1::QueryInvalid {
        sentence: sentence.into(),
    })
}

/// Split a request target into its path and its raw query.
fn split_target_v1(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    }
}

/// Read one parameter out of a raw query string.
///
/// No percent-decoding: every parameter this board takes is base58 or decimal,
/// so a value carrying an escape is not one of them and is refused by the
/// canonical checks below rather than quietly decoded into something else.
fn parameter_v1<'a>(query: &'a str, wanted: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            (name == wanted).then_some(value)
        })
}

/// Parse one canonical unsigned decimal `u64` parameter.
fn canonical_u64_v1(value: &str, label: &str) -> Result<u64, HttpResponseV1> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| query_invalid(format!("`{label}` is not one unsigned 64-bit integer")))?;
    if parsed.to_string() != value {
        return Err(query_invalid(format!(
            "`{label}` is not canonical decimal text"
        )));
    }
    Ok(parsed)
}

/// Answer one request.
///
/// The board lock is taken for the shortest possible span and never held across
/// a snapshot write; the caller owns persistence, so a slow disk cannot stall
/// every reader.
pub fn handle_v1(
    method: &str,
    target: &str,
    body: &[u8],
    board: &Mutex<BoardStateV1>,
) -> HttpResponseV1 {
    let (path, query) = split_target_v1(target);
    match (method, path) {
        ("GET", "/health") => health_v1(board),
        ("GET", "/tickets") => list_v1(query, board),
        ("POST", "/tickets") => post_v1(query, body, board),
        ("GET" | "POST", _) => json_or_refusal(
            &RefusedWireV1 {
                accepted: false,
                refusal: "ROUTE_NOT_FOUND",
                reason: format!(
                    "this board serves GET /health, GET /tickets and POST /tickets; \
                     it has no route {path}"
                ),
            },
            404,
        ),
        _ => json_or_refusal(
            &RefusedWireV1 {
                accepted: false,
                refusal: "METHOD_NOT_ALLOWED",
                reason: format!("{method} is not a method this board answers"),
            },
            405,
        ),
    }
}

fn health_v1(board: &Mutex<BoardStateV1>) -> HttpResponseV1 {
    let Ok(state) = board.lock() else {
        return json_or_refusal(
            &RefusedWireV1 {
                accepted: false,
                refusal: "BOARD_UNAVAILABLE",
                reason: "the board's state lock was poisoned by an earlier panic".into(),
            },
            500,
        );
    };
    json_or_refusal(
        &HealthWireV1 {
            status: "ok",
            offers: state.len(),
            capacity: state.capacity(),
            served_market: state.served_market().map(str::to_owned),
            observed_slot: None,
            notice: BOARD_NOTICE_V1,
        },
        200,
    )
}

fn list_v1(query: &str, board: &Mutex<BoardStateV1>) -> HttpResponseV1 {
    let Some(market) = parameter_v1(query, "market") else {
        return query_invalid(
            "GET /tickets needs a `market` parameter: a board is read one Market at a time",
        );
    };
    // The one authority on what a canonical address is, shared with the reader
    // that admits tickets.
    if let Err(error) = dclutch_direct_ticket::canonical_ticket_pubkey_v1(market, "the `market`") {
        return query_invalid(error.to_string());
    }

    let outcome = match parameter_v1(query, "outcome") {
        None => None,
        Some(text) => match canonical_u64_v1(text, "outcome") {
            Err(response) => return response,
            Ok(value) => match u32::try_from(value) {
                Ok(outcome) => Some(outcome),
                Err(_) => {
                    return query_invalid(
                        "`outcome` is above the runtime's 32-bit coordinate width",
                    );
                }
            },
        },
    };

    let slot = match parameter_v1(query, "slot") {
        None => None,
        Some(text) => match canonical_u64_v1(text, "slot") {
            Err(response) => return response,
            Ok(value) => Some(value),
        },
    };

    let Ok(state) = board.lock() else {
        return query_invalid("the board's state lock was poisoned by an earlier panic");
    };
    let listing = state.list_v1(&ListingQueryV1 {
        market: market.to_owned(),
        outcome,
        slot,
    });
    json_or_refusal(
        &ListingWireV1 {
            offers: listing
                .offers
                .iter()
                .map(|entry| OfferWireV1 {
                    digest: entry.digest.clone(),
                    text: entry.text.clone(),
                    posted_at_slot: entry.posted_at_slot.map(|slot| slot.to_string()),
                })
                .collect(),
            slot_basis: listing.slot_basis.map(|slot| slot.to_string()),
            dropped_expired: listing.dropped_expired,
            notice: BOARD_NOTICE_V1,
        },
        200,
    )
}

fn post_v1(query: &str, body: &[u8], board: &Mutex<BoardStateV1>) -> HttpResponseV1 {
    let at_slot = match parameter_v1(query, "slot") {
        None => None,
        Some(text) => match canonical_u64_v1(text, "slot") {
            Err(response) => return response,
            Ok(value) => Some(value),
        },
    };
    let Ok(mut state) = board.lock() else {
        return query_invalid("the board's state lock was poisoned by an earlier panic");
    };
    match state.admit_v1(body, at_slot) {
        Err(refusal) => refused(&refusal),
        Ok(accepted) => json_or_refusal(
            &AcceptedWireV1 {
                accepted: true,
                digest: accepted.digest,
                duplicate: accepted.duplicate,
            },
            201,
        ),
    }
}
