//! The board itself: what it holds, what it refuses, and why each refusal has a
//! name.
//!
//! WHAT THIS IS. A ticket board is rung (b) of the transport ladder in
//! `docs/design/FLOWFUL_IA_V1.md` §4.4: a small relay that accepts Direct
//! intent tickets, holds them, and lists them back. It exists because step ③ of
//! the trade flow currently asks a reader to obtain a 4096-byte signed blob out
//! of band, which is homework almost nobody can do.
//!
//! WHY A RELAY IS PERMITTED AT ALL. A Direct ticket is bearer-signed
//! self-authenticating data. Every field the transaction depends on is covered
//! by the maker's detached Ed25519 signature, and the chain re-derives the
//! signing message and verifies it natively. So: **a relay can withhold, and a
//! relay can never forge.** Its worst case is censorship and staleness — never
//! a wrong trade, never a stolen one. That is why this service can hold no
//! keys, take no custody, and carry no authority, and losing it entirely loses
//! availability and nothing else.
//!
//! WHAT IT DOES NOT VALIDATE, stated up front because the gap is the honest
//! part. This board checks the ticket's shape and its SIGNATURE. It does not
//! read chain state, so it cannot check that the maker's Position covers the
//! offer, that the generation is current, that the fee rate matches the
//! immutable Direct config, or that the outcome is inside the Product width.
//! Those are decided against finalized state by the code that builds the
//! transaction, and by the chain. An offer listed here is WELL-FORMED and
//! CORRECTLY SIGNED. It is not "valid", and this service must never say it is.
//!
//! SINGLE AUTHORSHIP. Every byte of a ticket interpreted here is interpreted by
//! [`dclutch_direct_ticket::parse_portable_direct_ticket_v1`] — the same reader
//! `dclutch ticket verify` runs and the same one the producer runs before it
//! opens a socket. This module parses no wire format of its own and re-derives
//! no signing message. That is not tidiness: a second implementation of a
//! signing preimage is a signature that verifies nowhere, discovered at the
//! refused trade.
//!
//! THE BOARD CANNOT SIGN. `dclutch-direct-ticket` is taken here with
//! `default-features = false`, so the `author` feature is off and no signer
//! crate is linked into this binary at all. The inability to mint a ticket is a
//! property of the build, not a promise in a comment.

use std::collections::BTreeMap;

use dclutch_direct_ticket::{
    MAXIMUM_TICKET_BYTES_V1, SignedDirectIntentV3, parse_portable_direct_ticket_v1, sha256_hex_v1,
};
use solana_program::pubkey::Pubkey;

/// The most offers one board holds before it refuses new ones.
///
/// A bound is mandatory: without one, an unauthenticated `POST` is unbounded
/// memory growth for anyone who can reach the port. It is deliberately a
/// REFUSAL rather than an eviction — a board that evicts to make room for the
/// newest offer lets a flood push every honest offer out, which is exactly the
/// censorship a relay is otherwise incapable of. Refusing keeps the failure
/// visible and keeps existing offers safe.
pub const MAXIMUM_OFFERS_V1: usize = 4_096;

/// The largest request body this board reads, in bytes.
///
/// A ticket is bounded at 4096 bytes by its own codec, so anything larger is
/// not a ticket and is refused before it is parsed, allocated, or hashed.
pub const MAXIMUM_BODY_BYTES_V1: usize = MAXIMUM_TICKET_BYTES_V1;

/// Every way this board refuses, named.
///
/// The name is the point. A caller that is told only that something "failed"
/// cannot act; a maker told `EXPIRED` re-authors with a later window, and a
/// maker told `MARKET_NOT_SERVED` posts to a different board. The wire carries
/// both this stable name and the sentence, and the tests assert on the variant
/// rather than on the presence of an error — an `is_err()` assertion is a test
/// of nothing, since it passes on whatever the service refuses first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BoardRefusalV1 {
    /// The request body was larger than a ticket can be.
    BodyTooLarge {
        /// What arrived, in bytes.
        received: usize,
    },
    /// The shared reader refused these bytes. Carries its sentence verbatim.
    ///
    /// This is the variant a TAMPERED ticket lands in: the reader re-derives
    /// the signing preimage from the fields as they arrived and checks the
    /// detached signature against it, so changing any signed field here is
    /// refused at admission rather than at the Ed25519 program much later.
    TicketMalformed {
        /// The reader's own words, unaltered.
        sentence: String,
    },
    /// This board serves one Market and the ticket names a different one.
    MarketNotServed {
        /// The Market this board serves.
        served: String,
        /// The Market the ticket names.
        offered: String,
    },
    /// The ticket's validity window had already closed at the slot the poster
    /// themselves supplied.
    Expired {
        /// The last slot the ticket is valid in.
        valid_through: u64,
        /// The slot the poster asserted as current.
        at_slot: u64,
    },
    /// The board is at capacity.
    BoardFull {
        /// The bound that was reached.
        capacity: usize,
    },
    /// A query parameter was missing or not canonical.
    QueryInvalid {
        /// What was wrong, in one sentence.
        sentence: String,
    },
}

impl BoardRefusalV1 {
    /// The stable machine name, for a caller that branches on the refusal.
    ///
    /// These strings are wire surface: a client may match on them, so they
    /// change only with a version.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::BodyTooLarge { .. } => "BODY_TOO_LARGE",
            Self::TicketMalformed { .. } => "TICKET_MALFORMED",
            Self::MarketNotServed { .. } => "MARKET_NOT_SERVED",
            Self::Expired { .. } => "EXPIRED",
            Self::BoardFull { .. } => "BOARD_FULL",
            Self::QueryInvalid { .. } => "QUERY_INVALID",
        }
    }

    /// The sentence a human should read.
    #[must_use]
    pub fn sentence(&self) -> String {
        match self {
            Self::BodyTooLarge { received } => format!(
                "the request body is {received} bytes; a Direct ticket is bounded at \
                 {MAXIMUM_BODY_BYTES_V1} bytes by its own codec"
            ),
            Self::TicketMalformed { sentence } => sentence.clone(),
            Self::MarketNotServed { served, offered } => {
                format!("this board serves Market {served}; the ticket offers Market {offered}")
            }
            Self::Expired {
                valid_through,
                at_slot,
            } => format!(
                "the ticket's last valid slot is {valid_through} and the posted current slot is \
                 {at_slot}; it is already expired and would be refused at execution"
            ),
            Self::BoardFull { capacity } => format!(
                "this board already holds its bound of {capacity} offers and refuses new ones \
                 rather than evicting existing offers to make room"
            ),
            Self::QueryInvalid { sentence } => sentence.clone(),
        }
    }

    /// The HTTP status this refusal answers with.
    #[must_use]
    pub fn status(&self) -> u16 {
        match self {
            // Not the client's request shape: the board has no room. 503 says
            // "come back", where 400 would say "your ticket was wrong".
            Self::BoardFull { .. } => 503,
            Self::BodyTooLarge { .. } => 413,
            _ => 400,
        }
    }
}

/// One offer, as the board holds it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardEntryV1 {
    /// SHA-256 of `text`, lowercase hex. The board's identifier for the offer.
    pub digest: String,
    /// The exact bytes the maker authored, stored verbatim.
    ///
    /// Verbatim matters: a ticket's encoding is canonical, and re-serializing
    /// it here would make this service a SECOND writer of a shape that has
    /// exactly one. The board transports bytes it does not author.
    pub text: String,
    /// Arrival order. Monotone, and the only ordering this board asserts.
    pub sequence: u64,
    /// The slot the poster asserted when posting, if they asserted one.
    ///
    /// Caller-supplied and unverifiable. It is reported so a reader can see
    /// staleness, and it decides nothing.
    pub posted_at_slot: Option<u64>,
    /// The signed intent, decoded at admission. Held so listing needs no
    /// re-parse to filter by market and outcome.
    pub signed: SignedDirectIntentV3,
}

impl BoardEntryV1 {
    /// The Market this offer is for, as canonical base58.
    #[must_use]
    pub fn market(&self) -> String {
        Pubkey::new_from_array(self.signed.intent.market).to_string()
    }

    /// The outcome coordinate this offer is for.
    #[must_use]
    pub fn outcome(&self) -> u32 {
        self.signed.intent.outcome
    }

    /// The last slot this offer is valid in, inclusive.
    #[must_use]
    pub fn valid_through(&self) -> u64 {
        self.signed.intent.valid_through
    }
}

/// What one accepted post amounted to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedOfferV1 {
    /// The board's identifier for the offer.
    pub digest: String,
    /// True when the board already held this exact ticket.
    ///
    /// A duplicate is accepted rather than refused: a maker re-posting after a
    /// timeout has done nothing wrong, and the ticket is content-addressed, so
    /// the second copy is the first one.
    pub duplicate: bool,
}

/// What one listing amounted to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingV1 {
    /// Matching offers, newest first.
    pub offers: Vec<BoardEntryV1>,
    /// The slot expiry was judged against, or `None` when the caller named no
    /// slot and the board therefore judged no expiry.
    pub slot_basis: Option<u64>,
    /// Matching offers dropped as expired at `slot_basis`.
    pub dropped_expired: usize,
}

/// The one filter a listing takes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListingQueryV1 {
    /// Canonical base58 Market address.
    pub market: String,
    /// Restrict to one outcome coordinate, or every outcome.
    pub outcome: Option<u32>,
    /// The slot to judge expiry against.
    ///
    /// Supplied by the caller because the board HAS NO CLOCK. It reads no chain
    /// and holds no trusted slot, and inventing one from whatever callers
    /// assert would hand any caller a lever to expire everyone else's offers —
    /// which is precisely the censorship a relay must not be given for free.
    /// So a slot decides one response and never touches stored state.
    pub slot: Option<u64>,
}

/// The board's whole state.
#[derive(Debug, Default)]
pub struct BoardStateV1 {
    entries: BTreeMap<String, BoardEntryV1>,
    next_sequence: u64,
    /// The Market this board serves, or every Market.
    served_market: Option<String>,
    capacity: usize,
}

impl BoardStateV1 {
    /// An empty board holding up to [`MAXIMUM_OFFERS_V1`] offers.
    #[must_use]
    pub fn new(served_market: Option<String>) -> Self {
        Self::with_capacity_v1(served_market, MAXIMUM_OFFERS_V1)
    }

    /// An empty board with an explicit capacity.
    ///
    /// The bound is a knob rather than a constant because the right one depends
    /// on how much memory an operator has given the process, and because a
    /// smaller board is how the full-board refusal gets exercised by a test
    /// that does not have to author four thousand signatures to reach it.
    #[must_use]
    pub fn with_capacity_v1(served_market: Option<String>, capacity: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            next_sequence: 0,
            served_market,
            capacity,
        }
    }

    /// The most offers this board will hold.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many offers the board holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the board holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The Market this board serves, if it serves only one.
    #[must_use]
    pub fn served_market(&self) -> Option<&str> {
        self.served_market.as_deref()
    }

    /// Every offer held, in arrival order. For snapshots.
    pub fn entries_in_arrival_order(&self) -> Vec<&BoardEntryV1> {
        let mut all: Vec<&BoardEntryV1> = self.entries.values().collect();
        all.sort_by_key(|entry| entry.sequence);
        all
    }

    /// Admit one ticket, or refuse it by name.
    ///
    /// `at_slot` is the poster's own assertion of the current slot. It is used
    /// for exactly one thing — refusing a ticket that is ALREADY expired — and
    /// a poster who lies about it can only harm their own post. It is never
    /// allowed to remove anyone else's offer.
    pub fn admit_v1(
        &mut self,
        body: &[u8],
        at_slot: Option<u64>,
    ) -> Result<AcceptedOfferV1, BoardRefusalV1> {
        if body.len() > MAXIMUM_BODY_BYTES_V1 {
            return Err(BoardRefusalV1::BodyTooLarge {
                received: body.len(),
            });
        }

        // THE LIFT. Shape, canonical form, codec roundtrip, and the detached
        // Ed25519 signature against the preimage this reader rebuilds. This
        // board adds no check of its own to any of it.
        let signed = parse_portable_direct_ticket_v1(body, "posted").map_err(|error| {
            BoardRefusalV1::TicketMalformed {
                sentence: error.to_string(),
            }
        })?;

        let offered = Pubkey::new_from_array(signed.intent.market).to_string();
        if let Some(served) = self.served_market.as_ref()
            && served != &offered
        {
            return Err(BoardRefusalV1::MarketNotServed {
                served: served.clone(),
                offered,
            });
        }

        if let Some(slot) = at_slot
            && signed.intent.valid_through < slot
        {
            return Err(BoardRefusalV1::Expired {
                valid_through: signed.intent.valid_through,
                at_slot: slot,
            });
        }

        let digest = sha256_hex_v1(body);
        if self.entries.contains_key(&digest) {
            return Ok(AcceptedOfferV1 {
                digest,
                duplicate: true,
            });
        }
        if self.entries.len() >= self.capacity {
            return Err(BoardRefusalV1::BoardFull {
                capacity: self.capacity,
            });
        }

        // The reader accepted these bytes, so they are UTF-8 JSON; the ticket
        // is stored exactly as it arrived rather than re-encoded.
        let text = String::from_utf8_lossy(body).into_owned();
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.entries.insert(
            digest.clone(),
            BoardEntryV1 {
                digest: digest.clone(),
                text,
                sequence,
                posted_at_slot: at_slot,
                signed,
            },
        );
        Ok(AcceptedOfferV1 {
            digest,
            duplicate: false,
        })
    }

    /// List the offers matching one query, newest first.
    ///
    /// Ordering is ARRIVAL order reversed, not price order. Price ordering
    /// needs the route's price scale and the reader's side, neither of which a
    /// transport has; the flow sorts for presentation. A board that claimed to
    /// rank offers "best first" would be asserting something it cannot compute.
    #[must_use]
    pub fn list_v1(&self, query: &ListingQueryV1) -> ListingV1 {
        let mut matched: Vec<&BoardEntryV1> = self
            .entries
            .values()
            .filter(|entry| entry.market() == query.market)
            .filter(|entry| query.outcome.is_none_or(|wanted| entry.outcome() == wanted))
            .collect();

        let mut dropped_expired = 0usize;
        if let Some(slot) = query.slot {
            let before = matched.len();
            matched.retain(|entry| entry.valid_through() >= slot);
            dropped_expired = before.saturating_sub(matched.len());
        }

        matched.sort_by_key(|entry| core::cmp::Reverse(entry.sequence));
        ListingV1 {
            offers: matched.into_iter().cloned().collect(),
            slot_basis: query.slot,
            dropped_expired,
        }
    }

    /// Restore one entry read back from a snapshot.
    ///
    /// Snapshot rows go through [`Self::admit_v1`] exactly like a live post, so
    /// a hand-edited snapshot cannot inject an offer the reader would refuse.
    /// The board does not trust its own file either.
    pub fn restore_v1(
        &mut self,
        body: &[u8],
        posted_at_slot: Option<u64>,
    ) -> Result<AcceptedOfferV1, BoardRefusalV1> {
        // Restoration passes no `at_slot`: the snapshot's own recorded slot is
        // history, and re-applying it as an expiry test would silently discard
        // offers that were fine when they were written.
        let accepted = self.admit_v1(body, None)?;
        if !accepted.duplicate
            && let Some(entry) = self.entries.get_mut(&accepted.digest)
        {
            entry.posted_at_slot = posted_at_slot;
        }
        Ok(accepted)
    }
}
