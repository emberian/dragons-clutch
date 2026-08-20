//! The clearing plane: the streaming checkpoint and the candidate feed.
//!
//! `docs/implementation/STREAMING_RELATION_DESIGN.md` §10 names two accounts
//! the streaming verifier needs and assigns both to this crate.  They are here,
//! and **nothing consumes them yet**: no instruction in `clutch-sbf` reads or
//! writes either account, and this module makes no claim that the streaming
//! verifier has been integrated.  What it delivers is the byte ownership the
//! integration will need, frozen and adversarially tested first, so that the
//! settlement lane inherits a codec rather than an offset arithmetic exercise.
//!
//! | account | tag | bytes | what it holds |
//! | --- | ---: | ---: | --- |
//! | [`ClearWorkAccount`] | 17 | 50,054 | the resumable checkpoint: a layout-owned header, the layout-owned owner-interning region, and a `ClearWorkV1` codec body |
//! | [`CandidateFeedAccount`] | 18 | 6,266 | the solver-written feed: candidate header, fill vector, optional pairing witness |
//! | [`EpochWindowAccount`] | 24 | 84 | the general epoch's deadline-window companion |
//!
//! # Neither account is ever a value
//!
//! The order page taught this lane the lesson at 4 KiB
//! ([`crate::stream`]); the checkpoint is **twelve times** the page.  So there
//! is deliberately no `ClearWorkAccount::decode` and no
//! `CandidateFeedAccount::decode` returning a whole account: every entry point
//! here either returns a small header by value, walks one element at a time, or
//! writes into a caller-owned account slice.  The largest value any function in
//! this module holds is one [`CandidateFeedHeader`] (346 bytes) — a diagnostic
//! from `cargo-build-sbf` for anything in this module would be a defect, not a
//! documented host-only path.
//!
//! # The body is the codec's, and that is a statement, not a shrug
//!
//! [`crate::CLEAR_WORK_BODY_BYTES`] is the pinned `ENCODED_BYTES` of the
//! checkpoint codec in `clutch_batch::relation_v1_stream` — an explicit
//! little-endian serializer (`encode_into`/`decode_into`, Tier 2 join 5) —
//! so the body is a wire format with exactly one owner, and it is not this
//! crate.  This crate still gives the region no interpretation: it owns the
//! length, the framing around it, the identity binding, and the two window
//! accessors ([`clear_work_body`], [`clear_work_body_mut`]) that hand the
//! region to the codec.  Casting these bytes to a `&mut ClearWorkV1` remains
//! **unsanctioned**: the struct is `repr(Rust)` and only the codec's field
//! walk relates it to bytes.
//!
//! # Index vocabulary: order indices are live ranks
//!
//! Every order index in this module — the [`CandidateFeedAccount`] fill
//! vector's index and the payload of [`LegRef::Order`] — is the order's
//! **zero-based live rank** in the canonical page-set walk: its position among
//! the records the projection actually feeds ([`crate::projection`]), which
//! skips retirements.  It is *not* the record's global slot index, and the two
//! vocabularies coincide exactly when the frozen set has `tombstone_count == 0`
//! on every page — the narrow settlement slice pins that case explicitly, which
//! is why it could leave the general reading unstated.  Zero-based, where the
//! relation's `canonical_order_id` is the same live rank one-based: the id
//! keeps zero reserved for "no order", an array index does not.
//!
//! # The consumed-fold binding
//!
//! §10 assigns the cryptographic anchoring of P-BATCH-03 to this crate: SHA-256
//! page digests authenticate the *bytes*, and the in-crate `mix` fold
//! authenticates the *walk*.  [`bind_order_set`] is the layout half — it stamps
//! `(order_set, consumed_fold)` once, at pass-1 finalize — and
//! [`require_continuation`] is the refusal a later pass runs into when its epoch
//! shows a different `order_set`.  Neither function verifies a fold; the fold is
//! `clutch-batch`'s, and saying so is the point.

use super::projection::OwnerInterner;
use super::{
    account_len, account_version, canonical_candidate_digest, canonical_epoch_id, check_count,
    check_hash, check_header, check_padded_amounts, digest, put_header, CodecError, EpochAccount,
    EpochId, Hash32, MarketId, Reader, Result, Writer, CANDIDATE_FEED_TAG, CLEAR_WORK_BODY_BYTES,
    CLEAR_WORK_TAG, EPOCH_PHASE_OPEN, HASH_BYTES, MAX_EPOCH_ORDERS, MAX_ORDERS_PER_PAGE,
    MAX_ORDER_PAGES, MAX_OUTCOMES, MAX_SLICES, RELATION_VERSION,
};

/* ------------------------------------------------------------------------ */
/* The streaming checkpoint                                                  */
/* ------------------------------------------------------------------------ */

/// Bytes of a [`ClearWorkAccount`] before its owner-interning region.
///
/// The only part of a checkpoint this crate ever materializes by value.
pub const CLEAR_WORK_HEADER_BYTES: usize =
    account_len::CLEAR_WORK - CLEAR_WORK_INTERNER_BYTES - CLEAR_WORK_BODY_BYTES;

/// Bytes of the layout-owned owner-interning region between the header and
/// the opaque body: the interned count, then all 64 owner slots.
///
/// The region persists the projection's [`OwnerInterner`] across walk
/// transactions — the `owner: u16` coordinate every projected order carries
/// is the index of its 32-byte owner's first appearance in the canonical
/// walk, and a resumed pass must reproduce exactly the tags pass 1 minted.
/// The relation checkpoint cannot carry it (its owner table is the *u16 tag*
/// side), so the mapping is layout state, framed here and written only
/// through [`write_owner_interner`].
pub const CLEAR_WORK_INTERNER_BYTES: usize = 2 + (MAX_EPOCH_ORDERS * HASH_BYTES);

/// Byte offset of the owner-interning region inside a checkpoint account.
const CLEAR_WORK_INTERNER_AT: usize = CLEAR_WORK_HEADER_BYTES;
/// Byte offset of the opaque codec body inside a checkpoint account.
const CLEAR_WORK_BODY_AT: usize = CLEAR_WORK_HEADER_BYTES + CLEAR_WORK_INTERNER_BYTES;

const _: () = assert!(CLEAR_WORK_HEADER_BYTES == 158);
const _: () = assert!(CLEAR_WORK_INTERNER_BYTES == 2_050);
const _: () = assert!(
    CLEAR_WORK_HEADER_BYTES + CLEAR_WORK_INTERNER_BYTES + CLEAR_WORK_BODY_BYTES
        == account_len::CLEAR_WORK
);

/// [`ClearWorkHeader::status`]: pass one is walking; nothing is bound yet.
pub const CLEAR_WORK_STATUS_OPEN: u8 = 0;
/// [`ClearWorkHeader::status`]: the order set and consumed fold are stamped.
pub const CLEAR_WORK_STATUS_BOUND: u8 = 1;
/// [`ClearWorkHeader::status`]: the feed reached a verdict; no pass may resume.
pub const CLEAR_WORK_STATUS_COMPLETE: u8 = 2;

/// The layout-owned header of one resumable clearing checkpoint.
///
/// Every field here is a fact the *layout* plane decides.  What is deliberately
/// **not** here is anything the checkpoint body already decides: the feed
/// phase, the pass number, the push cursor, the interned owner count and the
/// running fold are `ClearWorkV1`'s, and restating them would be a second
/// truth that could disagree with the first.  The walk position below is not
/// one of those: `ClearWorkV1`'s cursor counts *pushes*, this one names a
/// *page and a slot*, and only this crate knows what those are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearWorkHeader {
    /// Market identity.
    pub market: Hash32,
    /// Epoch identity whose frozen page set this checkpoint walks.
    pub epoch: Hash32,
    /// Candidate identity being verified; equals a
    /// [`CandidateFeedHeader::candidate`].
    pub candidate: Hash32,
    /// The frozen order-set digest this checkpoint is bound to.
    ///
    /// Zero exactly while [`ClearWorkHeader::status`] is
    /// [`CLEAR_WORK_STATUS_OPEN`]; stamped once by [`bind_order_set`] and
    /// immutable afterwards.  This is the anchor of
    /// `STREAMING_RELATION_DESIGN.md` §10: a later pass fed from a *different*
    /// frozen set is refused rather than silently folded in.
    pub order_set: Hash32,
    /// The sealed pass-1 continuation digest, as reported by `consumed_fold`.
    ///
    /// Not a duplicate of the body's running fold: that value moves on every
    /// push, this one is the single sealed value the later passes are compared
    /// against.  This crate stores it and never computes it — the fold is
    /// `clutch-batch`'s deterministic identity, explicitly not a commitment.
    pub consumed_fold: u128,
    /// Length of the opaque body; must equal [`CLEAR_WORK_BODY_BYTES`].
    ///
    /// Stored rather than implied so that an account written by a build with a
    /// different checkpoint size is a decode refusal here instead of a silent
    /// misread later.
    pub body_len: u32,
    /// Zero-based index of the page the walk is positioned at.
    pub page_cursor: u16,
    /// Live orders visited so far — the relation's order index, which counts
    /// records and not retirements.
    pub live_rank: u16,
    /// Zero-based slot within [`ClearWorkHeader::page_cursor`].
    pub slot_cursor: u8,
    /// Lifecycle; one of the `CLEAR_WORK_STATUS_*` constants.
    pub status: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl ClearWorkHeader {
    /// Validate identities, the body length, the walk position, and the
    /// status/binding agreement.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.epoch)?;
        check_hash(self.candidate)?;
        if self.status > CLEAR_WORK_STATUS_COMPLETE || self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        if self.body_len as usize != CLEAR_WORK_BODY_BYTES {
            return Err(CodecError::InvalidCount);
        }
        /* Binding and status are the same fact stated twice, so they must
         * agree exactly: an open checkpoint has no order set, and a bound one
         * has one.  A checkpoint carrying a fold with no set is the shape a
         * tampered resume would like to have. */
        let bound = self.status != CLEAR_WORK_STATUS_OPEN;
        if bound == (self.order_set == Hash32::ZERO) {
            return Err(if bound {
                CodecError::ZeroIdentity
            } else {
                CodecError::NonCanonicalPadding
            });
        }
        if !bound && self.consumed_fold != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        if self.page_cursor as usize > MAX_ORDER_PAGES {
            return Err(CodecError::InvalidCount);
        }
        if self.slot_cursor as usize > MAX_ORDERS_PER_PAGE {
            return Err(CodecError::InvalidCount);
        }
        if self.live_rank as usize > MAX_EPOCH_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        /* One past the last page is the only position where a slot cursor may
         * be nonzero-free: the walk is finished, and there is no page to be
         * partway through. */
        if self.page_cursor as usize == MAX_ORDER_PAGES && self.slot_cursor != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        Ok(())
    }

    /// Encode exactly [`CLEAR_WORK_HEADER_BYTES`] bytes.
    ///
    /// The header only: this never touches the body, which is why it can be
    /// called on a caller's small buffer as well as on an account.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < CLEAR_WORK_HEADER_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, CLEAR_WORK_TAG, account_version::CLEAR_WORK)?;
        w.hash(self.market)?;
        w.hash(self.epoch)?;
        w.hash(self.candidate)?;
        w.hash(self.order_set)?;
        w.u128(self.consumed_fold)?;
        w.u32(self.body_len)?;
        w.u16(self.page_cursor)?;
        w.u16(self.live_rank)?;
        w.u8(self.slot_cursor)?;
        w.u8(self.status)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        if w.at != CLEAR_WORK_HEADER_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(w.at)
    }

    /// Parse the header of a whole [`account_len::CLEAR_WORK`] account.
    ///
    /// The input must be the *whole* account: a checkpoint that is short by one
    /// body byte is not a checkpoint, and finding that out here rather than at
    /// the first body write is the point of the exact-length rule every codec
    /// in this crate keeps.
    pub fn decode(input: &[u8]) -> Result<Self> {
        check_header(
            input,
            CLEAR_WORK_TAG,
            account_version::CLEAR_WORK,
            account_len::CLEAR_WORK,
        )?;
        let mut r = Reader::at(input, 2);
        let value = Self {
            market: r.hash()?,
            epoch: r.hash()?,
            candidate: r.hash()?,
            order_set: r.hash()?,
            consumed_fold: r.u128()?,
            body_len: r.u32()?,
            page_cursor: r.u16()?,
            live_rank: r.u16()?,
            slot_cursor: r.u8()?,
            status: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// The whole checkpoint account, named for the doc table.
///
/// It is a *type-level* name only: there is deliberately no value of this type,
/// because a value of it would be 50,054 bytes on a call frame.  A caller reads
/// [`verify_clear_work`] and walks the body through [`clear_work_body`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClearWorkAccount {}

/// Verify a checkpoint account's framing and return its header.
///
/// Everything this crate can decide about a checkpoint: the tag, the version,
/// the exact account length, and every header rule of
/// [`ClearWorkHeader::validate`].  It decides **nothing** about the body, and
/// a body of any 47,846 bytes at all passes here — the body's own validity is
/// the checkpoint codec's (`ClearWorkV1::decode_into`), which is why this
/// function's name is `verify_clear_work` and not `verify_checkpoint`.
pub fn verify_clear_work(input: &[u8]) -> Result<ClearWorkHeader> {
    ClearWorkHeader::decode(input)
}

/// The opaque body region of a checkpoint account, borrowed in place.
pub fn clear_work_body(input: &[u8]) -> Result<&[u8]> {
    ClearWorkHeader::decode(input)?;
    Ok(&input[CLEAR_WORK_BODY_AT..])
}

/// The opaque body region of a checkpoint account, borrowed mutably in place.
///
/// This is the whole write side of the body: the caller hands this slice to
/// whoever owns the checkpoint's semantics and this crate never looks at what
/// comes back.  Nothing is copied, so no frame ever holds the body.
pub fn clear_work_body_mut(input: &mut [u8]) -> Result<&mut [u8]> {
    ClearWorkHeader::decode(input)?;
    Ok(&mut input[CLEAR_WORK_BODY_AT..])
}

/// Read the persisted owner-interning table of a checkpoint account.
///
/// The framing refusals are the header decoder's; the region's own rules are
/// [`OwnerInterner::restore`]'s — a bounded count, no zero owner below it,
/// canonical zero padding at and beyond it.  Distinctness below the count is
/// **by construction**, not re-verified here: the table is written only
/// through [`write_owner_interner`] from a table [`OwnerInterner::intern`]
/// built, the account is program-owned, and an all-pairs comparison on every
/// resume would price the walk out of its compute budget for a fact no
/// program path can break.
pub fn read_owner_interner(input: &[u8]) -> Result<OwnerInterner> {
    let mut owners = OwnerInterner::NEW;
    read_owner_interner_into(input, &mut owners)?;
    Ok(owners)
}

/// Read the persisted owner-interning table into a caller-owned table.
///
/// The in-place form of [`read_owner_interner`], for callers whose frames
/// must never hold a second 2,050-byte table — the on-chain walk reads into a
/// heap-boxed table through this.  Same rules, same refusals; on any refusal
/// the output table is reset to empty rather than left half-written.
pub fn read_owner_interner_into(input: &[u8], out: &mut OwnerInterner) -> Result<()> {
    ClearWorkHeader::decode(input)?;
    let mut r = Reader::at(input, CLEAR_WORK_INTERNER_AT);
    let count = r.u16()?;
    if count as usize > MAX_EPOCH_ORDERS {
        return Err(CodecError::InvalidCount);
    }
    let (owners, stored_count) = out.raw_parts_mut();
    *stored_count = 0;
    let mut i = 0usize;
    while i < MAX_EPOCH_ORDERS {
        let owner = match r.hash() {
            Ok(owner) => owner,
            Err(error) => {
                owners[..i].fill(Hash32::ZERO);
                return Err(error);
            }
        };
        let fault = if i < count as usize {
            check_hash(owner).err()
        } else if owner != Hash32::ZERO {
            Some(CodecError::NonCanonicalPadding)
        } else {
            None
        };
        if let Some(error) = fault {
            owners[..i].fill(Hash32::ZERO);
            return Err(error);
        }
        owners[i] = owner;
        i += 1;
    }
    *stored_count = count;
    Ok(())
}

/// Persist one owner-interning table into a checkpoint account.
///
/// All 64 slots are written — the live prefix verbatim, canonical zeros
/// beyond it — so the region can never drift from the exact image
/// [`read_owner_interner`] reproduces.
pub fn write_owner_interner(account: &mut [u8], owners: &OwnerInterner) -> Result<()> {
    ClearWorkHeader::decode(account)?;
    let end = CLEAR_WORK_INTERNER_AT + CLEAR_WORK_INTERNER_BYTES;
    let mut w = Writer::new(&mut account[CLEAR_WORK_INTERNER_AT..end]);
    w.u16(owners.count())?;
    let live = owners.owners();
    let mut i = 0usize;
    while i < MAX_EPOCH_ORDERS {
        w.hash(if i < live.len() { live[i] } else { Hash32::ZERO })?;
        i += 1;
    }
    if w.at != CLEAR_WORK_INTERNER_BYTES {
        return Err(CodecError::OutputTooSmall);
    }
    Ok(())
}

/// Rewind the page-set walk position to the top of the set, for the next pass.
///
/// The one sanctioned exception to [`advance_walk`]'s monotone rule: the feed
/// protocol re-walks the same frozen set once per pass, so at a pass boundary
/// — and only there — the cursor legitimately returns to `(0, 0)`.  Requiring
/// [`CLEAR_WORK_STATUS_BOUND`] is what pins "at a pass boundary": binding
/// happens exactly at pass-1 completion, every later pass starts from a bound
/// checkpoint, and a completed one refuses.  Double-counting within a pass
/// stays impossible: a re-fed prefix changes the pass's running fold, and the
/// checkpoint codec refuses the pass at its end (`ResumeFoldMismatch`) rather
/// than folding an order twice into a verdict.
pub fn rewind_walk(account: &mut [u8]) -> Result<ClearWorkHeader> {
    let mut header = ClearWorkHeader::decode(account)?;
    if header.status != CLEAR_WORK_STATUS_BOUND {
        return Err(CodecError::MismatchedBinding);
    }
    header.page_cursor = 0;
    header.slot_cursor = 0;
    header.live_rank = 0;
    header.encode(account)?;
    Ok(header)
}

/// Write an open checkpoint over a fresh account, and return its header.
///
/// A zeroed account is not an open checkpoint: it carries no tag, no version,
/// and no identity.  This is the writer that makes one, and it is the only one
/// here that does not require the buffer to already decode.  The body is zeroed
/// in place — `ClearWorkV1::NEW` is not all-zero in every field, so the caller
/// must still write the idle checkpoint through its owning crate; this
/// establishes the *account*, not the checkpoint's initial value, and saying so
/// is the difference between a framing and a lie.  The zero fill also covers
/// the owner-interning region, whose all-zero image is exactly the empty
/// table ([`OwnerInterner::NEW`]).
pub fn init_clear_work(
    account: &mut [u8],
    market: Hash32,
    epoch: Hash32,
    candidate: Hash32,
    stored_bump: u8,
) -> Result<ClearWorkHeader> {
    if account.len() != account_len::CLEAR_WORK {
        return Err(CodecError::OutputTooSmall);
    }
    let header = ClearWorkHeader {
        market,
        epoch,
        candidate,
        order_set: Hash32::ZERO,
        consumed_fold: 0,
        body_len: CLEAR_WORK_BODY_BYTES as u32,
        page_cursor: 0,
        live_rank: 0,
        slot_cursor: 0,
        status: CLEAR_WORK_STATUS_OPEN,
        stored_bump,
        flags: 0,
    };
    header.encode(account)?;
    account[CLEAR_WORK_HEADER_BYTES..].fill(0);
    Ok(header)
}

/// Stamp the frozen order set and the sealed pass-1 fold, once.
///
/// The layout half of `STREAMING_RELATION_DESIGN.md` §10's anchoring.  It
/// refuses a second stamp ([`CodecError::MismatchedBinding`]) and a zero order
/// set ([`CodecError::ZeroIdentity`]), which is what makes the binding a
/// one-way transition rather than a mutable field.  It does not check the fold
/// against anything: this crate cannot recompute a `clutch-batch` fold and does
/// not pretend to.
pub fn bind_order_set(
    account: &mut [u8],
    order_set: Hash32,
    consumed_fold: u128,
) -> Result<ClearWorkHeader> {
    let mut header = ClearWorkHeader::decode(account)?;
    if header.status != CLEAR_WORK_STATUS_OPEN {
        return Err(CodecError::MismatchedBinding);
    }
    check_hash(order_set)?;
    header.order_set = order_set;
    header.consumed_fold = consumed_fold;
    header.status = CLEAR_WORK_STATUS_BOUND;
    header.encode(account)?;
    Ok(header)
}

/// Refuse a resumed pass whose epoch shows a different frozen order set.
///
/// The other half of the anchoring, and the only refusal that makes "the same
/// feed" checkable rather than assumed.  An open checkpoint has nothing to
/// compare against, so it refuses too: a pass that believes it is resuming a
/// bound walk and finds an unbound one is exactly the tamper case.
pub fn require_continuation(header: &ClearWorkHeader, epoch_order_set: Hash32) -> Result<()> {
    header.validate()?;
    if header.status == CLEAR_WORK_STATUS_OPEN || header.order_set != epoch_order_set {
        return Err(CodecError::MismatchedBinding);
    }
    Ok(())
}

/// Move the page-set walk position, and return the header that resulted.
///
/// Monotone by construction: a walk position may only move forward in
/// `(page_cursor, slot_cursor)` lexicographic order, and `live_rank` may only
/// rise.  A backward move is [`CodecError::MismatchedBinding`] — the same
/// refusal class a replayed page write gets — because a cursor that can go
/// backwards is a cursor that can double-count an order into every fold the
/// checkpoint holds.
pub fn advance_walk(
    account: &mut [u8],
    page_cursor: u16,
    slot_cursor: u8,
    live_rank: u16,
) -> Result<ClearWorkHeader> {
    let mut header = ClearWorkHeader::decode(account)?;
    if header.status == CLEAR_WORK_STATUS_COMPLETE {
        return Err(CodecError::MismatchedBinding);
    }
    let before = (header.page_cursor, header.slot_cursor);
    let after = (page_cursor, slot_cursor);
    if after <= before || live_rank < header.live_rank {
        return Err(CodecError::MismatchedBinding);
    }
    header.page_cursor = page_cursor;
    header.slot_cursor = slot_cursor;
    header.live_rank = live_rank;
    header.encode(account)?;
    Ok(header)
}

/// Close a bound checkpoint: no later pass may resume it.
///
/// Only a bound checkpoint can complete, because completing an open one would
/// mean a verdict was reached without the walk ever having been anchored to a
/// frozen order set.
pub fn complete_clear_work(account: &mut [u8]) -> Result<ClearWorkHeader> {
    let mut header = ClearWorkHeader::decode(account)?;
    if header.status != CLEAR_WORK_STATUS_BOUND {
        return Err(CodecError::MismatchedBinding);
    }
    header.status = CLEAR_WORK_STATUS_COMPLETE;
    header.encode(account)?;
    Ok(header)
}

/* ------------------------------------------------------------------------ */
/* The staged-creation grow prefix                                           */
/* ------------------------------------------------------------------------ */

/// The staged-creation grow step, in bytes.
///
/// This is the runtime's per-instruction data-growth ceiling
/// (`solana_program_entrypoint::MAX_PERMITTED_DATA_INCREASE`), restated here
/// because the checkpoint's creation protocol is *defined* by it: the one
/// account above the ceiling (the test
/// `only_the_checkpoint_exceeds_the_cpi_creation_ceiling` pins which) is
/// created at this length and grown by at most this much per instruction,
/// five instructions in all.  The cap is per instruction, not per
/// transaction, so the five may share one transaction — and nothing here
/// requires that.
pub const CLEAR_WORK_GROW_STEP: usize = 10 * 1024;

/// Marker byte at offset 2 of a *growing* checkpoint account.
///
/// In a finished checkpoint the same offset holds `market[0]`, which this
/// byte cannot be relied upon to differ from; the load-bearing separation is
/// the **length rule**: a growing account's length is a positive multiple of
/// [`CLEAR_WORK_GROW_STEP`] strictly below [`account_len::CLEAR_WORK`], a
/// finished checkpoint's length is exactly [`account_len::CLEAR_WORK`], and
/// the two sets are disjoint by the `const` assertion below.  The marker is
/// what makes the prefix un-repurposable — no other account family writes
/// this shape — and human-legible in a hex dump.
pub const CLEAR_WORK_GROWING_MARKER: u8 = b'G';

/// Exact bytes of the grow-stage prefix: tag, version, marker, `target_len`,
/// the identity triple, and the stored bump.
pub const CLEAR_WORK_GROW_PREFIX_BYTES: usize = 2 + 1 + 4 + (3 * HASH_BYTES) + 1;

const _: () = assert!(CLEAR_WORK_GROW_PREFIX_BYTES == 104);
const _: () = assert!(CLEAR_WORK_GROW_PREFIX_BYTES <= CLEAR_WORK_GROW_STEP);
// Five stages: create at one step, grow four times, the last one short.
const _: () = assert!(account_len::CLEAR_WORK > 4 * CLEAR_WORK_GROW_STEP);
const _: () = assert!(account_len::CLEAR_WORK < 5 * CLEAR_WORK_GROW_STEP);
// The length-rule disjointness the marker doc relies on: no growing length
// can equal the finished length.
const _: () = assert!(!account_len::CLEAR_WORK.is_multiple_of(CLEAR_WORK_GROW_STEP));

/// The resumable prefix of a checkpoint account that is still being grown.
///
/// Written once by the creating instruction and preserved verbatim by every
/// grow (a realloc keeps existing bytes), it makes a half-grown account
/// *resumable* — the grower re-derives the PDA from the stored triple — and
/// *inert*: [`ClearWorkHeader::decode`]'s exact-length rule refuses the
/// account outright, so no checkpoint consumer can read one, and the final
/// grow overwrites this prefix with the real header in the same instruction
/// that reaches the target length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClearWorkGrowStage {
    /// Market identity, as the eventual [`ClearWorkHeader::market`].
    pub market: Hash32,
    /// Epoch identity, as the eventual [`ClearWorkHeader::epoch`].
    pub epoch: Hash32,
    /// Candidate identity, as the eventual [`ClearWorkHeader::candidate`].
    pub candidate: Hash32,
    /// The finished account length; must equal [`account_len::CLEAR_WORK`].
    ///
    /// Stored rather than implied for the same reason
    /// [`ClearWorkHeader::body_len`] is: a stage written by a build with a
    /// different checkpoint size must refuse here, not misread later.
    pub target_len: u32,
    /// Stored PDA bump, as the eventual [`ClearWorkHeader::stored_bump`].
    pub stored_bump: u8,
}

impl ClearWorkGrowStage {
    /// Validate the identity triple and the target length.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        check_hash(self.epoch)?;
        check_hash(self.candidate)?;
        if self.target_len as usize != account_len::CLEAR_WORK {
            return Err(CodecError::InvalidCount);
        }
        Ok(())
    }

    /// Encode exactly [`CLEAR_WORK_GROW_PREFIX_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < CLEAR_WORK_GROW_PREFIX_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, CLEAR_WORK_TAG, account_version::CLEAR_WORK)?;
        w.u8(CLEAR_WORK_GROWING_MARKER)?;
        w.u32(self.target_len)?;
        w.hash(self.market)?;
        w.hash(self.epoch)?;
        w.hash(self.candidate)?;
        w.u8(self.stored_bump)?;
        if w.at != CLEAR_WORK_GROW_PREFIX_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(w.at)
    }

    /// Parse the prefix of a whole *growing* checkpoint account.
    ///
    /// The input must be the whole account, and its length must satisfy the
    /// staged-length rule ([`clear_work_grow_stage_len`]) — which a finished
    /// checkpoint's exact length never does, so the two decoders can never
    /// accept the same bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        clear_work_grow_stage_len(input.len())?;
        if input[0] != CLEAR_WORK_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != account_version::CLEAR_WORK {
            return Err(CodecError::WrongVersion);
        }
        if input[2] != CLEAR_WORK_GROWING_MARKER {
            return Err(CodecError::InvalidEnum);
        }
        let mut r = Reader::at(input, 3);
        let value = Self {
            target_len: r.u32()?,
            market: r.hash()?,
            epoch: r.hash()?,
            candidate: r.hash()?,
            stored_bump: r.u8()?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// The staged-length rule: a growing account is a positive multiple of
/// [`CLEAR_WORK_GROW_STEP`] strictly below [`account_len::CLEAR_WORK`].
pub fn clear_work_grow_stage_len(len: usize) -> Result<()> {
    if (CLEAR_WORK_GROW_STEP..account_len::CLEAR_WORK).contains(&len)
        && len.is_multiple_of(CLEAR_WORK_GROW_STEP)
    {
        Ok(())
    } else {
        Err(CodecError::InvalidCount)
    }
}

/// The length one grow reaches from `len`: one step, capped at the target.
pub const fn clear_work_grown_len(len: usize) -> usize {
    let grown = len + CLEAR_WORK_GROW_STEP;
    if grown > account_len::CLEAR_WORK {
        account_len::CLEAR_WORK
    } else {
        grown
    }
}

/// Write the grow-stage prefix over a freshly allocated first-stage account.
///
/// The account must be exactly one [`CLEAR_WORK_GROW_STEP`] long — the length
/// the creating instruction allocates.  Everything after the prefix is
/// zeroed; those bytes are scratch the final grow overwrites entirely.
pub fn init_clear_work_grow_stage(
    account: &mut [u8],
    market: Hash32,
    epoch: Hash32,
    candidate: Hash32,
    stored_bump: u8,
) -> Result<ClearWorkGrowStage> {
    if account.len() != CLEAR_WORK_GROW_STEP {
        return Err(CodecError::OutputTooSmall);
    }
    let stage = ClearWorkGrowStage {
        market,
        epoch,
        candidate,
        target_len: account_len::CLEAR_WORK as u32,
        stored_bump,
    };
    stage.encode(account)?;
    account[CLEAR_WORK_GROW_PREFIX_BYTES..].fill(0);
    Ok(stage)
}

/* ------------------------------------------------------------------------ */
/* The general epoch lifecycle                                               */
/* ------------------------------------------------------------------------ */

/// Account tag of the general epoch's deadline-window companion.
pub const EPOCH_WINDOW_TAG: u8 = 24;
/// Account version of the general epoch's deadline-window companion.
pub const EPOCH_WINDOW_VERSION: u8 = 1;
/// Exact byte length of one [`EpochWindowAccount`].
pub const EPOCH_WINDOW_ACCOUNT_BYTES: usize = 2 + 32 + 32 + 8 + 8 + 1 + 1;

const _: () = assert!(EPOCH_WINDOW_ACCOUNT_BYTES == 84);

/// Derive the one book identity admitted by the general clearing plane.
///
/// The general sibling of `canonical_direct_book_id`: one epoch clears exactly
/// one book, so the book identity is a total function of the epoch identity,
/// under its own domain separator so a general book and a direct book of the
/// same epoch can never collide.
pub fn canonical_general_book_id(epoch: Hash32) -> Hash32 {
    digest(b"dragons-clutch:general-book:v1", &[&epoch.0])
}

/// Derive the deterministic relation remainder seed for the general plane.
///
/// The same construction as the direct profile's: the epoch identity's first
/// eight bytes, little-endian.  The seed's only consumer is the relation's
/// frozen largest-remainder permutation; determinism from the epoch identity
/// is what matters, unpredictability is not a goal it could meet anyway.
pub const fn canonical_general_remainder_seed(epoch: Hash32) -> u64 {
    u64::from_le_bytes([
        epoch.0[0], epoch.0[1], epoch.0[2], epoch.0[3], epoch.0[4], epoch.0[5], epoch.0[6],
        epoch.0[7],
    ])
}

/// Construct the canonical open general [`EpochAccount`].
///
/// The general sibling of `DirectEpochV3Account::open`, minus the `== 2`
/// gates: outcome width and price scale arrive from the terms and grid the
/// caller has already bound, the book identity and the remainder seed are
/// derived from the epoch identity, and every frozen-set field starts at its
/// canonical open zero.  `owner_count` opens at [`MAX_EPOCH_ORDERS`] — the
/// widest owner space a 64-slot book can carry — and the freeze rewrites it
/// with the exact distinct-owner count interned over the frozen set, which is
/// the value the pass-1 walk is later compared against.
#[allow(clippy::too_many_arguments)] // one argument per bound account fact
pub fn open_general_epoch(
    market: MarketId,
    terms: Hash32,
    price_grid: Hash32,
    policy: Hash32,
    epoch_index: u64,
    price_scale: u64,
    outcome_count: u8,
    stored_bump: u8,
) -> Result<EpochAccount> {
    let epoch = canonical_epoch_id(market, epoch_index);
    let value = EpochAccount {
        epoch,
        market,
        book: canonical_general_book_id(epoch),
        terms,
        price_grid,
        policy,
        order_set: Hash32::ZERO,
        first_order_id: Hash32::ZERO,
        last_order_id: Hash32::ZERO,
        epoch_index,
        relation_version: RELATION_VERSION,
        price_scale,
        remainder_seed: canonical_general_remainder_seed(epoch),
        owner_count: MAX_EPOCH_ORDERS as u16,
        page_count: 0,
        order_count: 0,
        outcome_count,
        phase: EPOCH_PHASE_OPEN,
        stored_bump,
        flags: 0,
    };
    value.validate()?;
    Ok(value)
}

/// The deadline-slot companion of one general epoch.
///
/// The V3 window precedent applied to the general plane: deadline slots ride a
/// small dedicated account instead of an [`EpochAccount`] format bump, so the
/// epoch codec every existing consumer decodes stays byte-stable.  V1 carries
/// exactly one deadline — the slot at or after which the permissionless
/// freeze is admitted; placements and cancellations gate on the epoch phase,
/// not on this slot.  The candidate-window deadlines of the selection
/// lifecycle (T2-7) are a later format revision of this account, not a second
/// account family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EpochWindowAccount {
    /// Epoch identity; must equal `canonical_epoch_id(market, epoch_index)`.
    pub epoch: EpochId,
    /// Market identity.
    pub market: MarketId,
    /// Epoch index within the market.
    pub epoch_index: u64,
    /// First slot at which [`FreezeEpoch`](crate::Intent::FreezeEpoch) is
    /// admitted; zero is refused — a window with no deadline is not a window.
    pub freeze_deadline_slot: u64,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; currently zero.
    pub flags: u8,
}

impl EpochWindowAccount {
    /// Validate identities, the canonical epoch derivation, and the deadline.
    pub fn validate(&self) -> Result<()> {
        check_hash(self.market)?;
        if self.epoch != canonical_epoch_id(self.market, self.epoch_index) {
            return Err(CodecError::NonCanonicalIdentity);
        }
        if self.freeze_deadline_slot == 0 {
            return Err(CodecError::ZeroValue);
        }
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    /// Encode exactly [`EPOCH_WINDOW_ACCOUNT_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < EPOCH_WINDOW_ACCOUNT_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, EPOCH_WINDOW_TAG, EPOCH_WINDOW_VERSION)?;
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.u64(self.epoch_index)?;
        w.u64(self.freeze_deadline_slot)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        if w.at != EPOCH_WINDOW_ACCOUNT_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(w.at)
    }

    /// Parse exactly [`EPOCH_WINDOW_ACCOUNT_BYTES`] bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        check_header(
            input,
            EPOCH_WINDOW_TAG,
            EPOCH_WINDOW_VERSION,
            EPOCH_WINDOW_ACCOUNT_BYTES,
        )?;
        let mut r = Reader::at(input, 2);
        let value = Self {
            epoch: r.hash()?,
            market: r.hash()?,
            epoch_index: r.u64()?,
            freeze_deadline_slot: r.u64()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        value.validate()?;
        Ok(value)
    }
}

/* ------------------------------------------------------------------------ */
/* The candidate feed                                                        */
/* ------------------------------------------------------------------------ */

/// Exact encoded length of one pairing slice.
///
/// Two leg references at two bytes each, the bound outcome, and the quantity.
/// A leg reference is a kind byte and an order index; the index of a virtual
/// leg is canonical zero padding, which is why the pair is two bytes rather
/// than a tagged union of two widths.
pub const PAIRING_SLICE_BYTES: usize = 2 + 2 + 1 + 8;

/// Bytes of a [`CandidateFeedAccount`] before its fill vector.
pub const CANDIDATE_FEED_HEADER_BYTES: usize =
    account_len::CANDIDATE_FEED - (MAX_EPOCH_ORDERS * 8) - (MAX_SLICES * PAIRING_SLICE_BYTES);

const _: () = assert!(PAIRING_SLICE_BYTES == 13);
const _: () = assert!(CANDIDATE_FEED_HEADER_BYTES == 346);

/// Byte offset of the fill vector inside a candidate feed account.
const FILLS_AT: usize = CANDIDATE_FEED_HEADER_BYTES;
/// Byte offset of the slice vector inside a candidate feed account.
const SLICES_AT: usize = FILLS_AT + (MAX_EPOCH_ORDERS * 8);

/// [`CandidateFeedHeader::flags`] bit: an explicit pairing witness is declared.
///
/// This bit is what makes `declared_slices: Option<u16>` representable: the
/// count alone cannot distinguish "no witness" from "a witness of zero
/// slices", and those are different feeds — the second one asserts that the
/// canonical decomposition is empty and the first asserts nothing at all.
pub const CANDIDATE_FEED_FLAG_SLICES_DECLARED: u8 = 1;

/// Leg-reference kind: the filled leg of the order at this index.
pub const LEG_KIND_ORDER: u8 = 0;
/// Leg-reference kind: the single global virtual split.
pub const LEG_KIND_SPLIT: u8 = 1;
/// Leg-reference kind: the single global virtual merge.
pub const LEG_KIND_MERGE: u8 = 2;

/// One end of a pairing slice.
///
/// Mirrors `clutch_batch::relation_v1::LegRefV1` with the host model's
/// `Order(u8)` payload kept at its own width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegRef {
    /// The filled leg of the order at this index, on the slice's outcome.
    ///
    /// The index is the order's **zero-based live rank** in the canonical
    /// page-set walk — the same vocabulary as the fill vector, see the module
    /// docs — never a global slot index.  [`PairingSlice::validate`] bounds it
    /// by the feed's `order_len`, which is the frozen set's *live* count.
    Order(u8),
    /// The global virtual split, which serves buy legs on every outcome.
    Split,
    /// The global virtual merge, which absorbs sell legs on every outcome.
    Merge,
}

impl LegRef {
    /// The kind discriminator this reference encodes as.
    pub const fn kind(&self) -> u8 {
        match self {
            Self::Order(_) => LEG_KIND_ORDER,
            Self::Split => LEG_KIND_SPLIT,
            Self::Merge => LEG_KIND_MERGE,
        }
    }
    /// The index byte this reference encodes as; zero for a virtual leg.
    pub const fn index(&self) -> u8 {
        match self {
            Self::Order(index) => *index,
            Self::Split | Self::Merge => 0,
        }
    }
}

/// One slice of an explicit pairing witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingSlice {
    /// A buy leg, or [`LegRef::Merge`].
    pub buy_ref: LegRef,
    /// A sell leg, or [`LegRef::Split`].
    pub sell_ref: LegRef,
    /// The bound outcome of both ends.
    pub outcome: u8,
    /// Egg atoms moved by this slice.
    pub quantity: u64,
}

impl PairingSlice {
    /// The all-zero slice, which is what canonical padding is.
    pub const PADDING: Self = Self {
        buy_ref: LegRef::Order(0),
        sell_ref: LegRef::Order(0),
        outcome: 0,
        quantity: 0,
    };

    /// Validate one live slice against the feed's order count and outcomes.
    ///
    /// What this decides is exactly what a *representation* can decide: which
    /// side a virtual leg may appear on, that an order index names an order
    /// the feed actually carries, that the outcome is active, and that a live
    /// slice moves something.  Whether the decomposition is *executable* — the
    /// covered-versus-legs comparison of the design's §5 — is
    /// `clutch-batch`'s, and nothing here approximates it.
    pub fn validate(&self, order_len: u8, outcome_count: u8) -> Result<()> {
        /* `buy_ref` is a buy leg or the merge; `sell_ref` is a sell leg or the
         * split.  A split in the buy slot is not a slice with a bad value in
         * it, it is not a slice. */
        if matches!(self.buy_ref, LegRef::Split) || matches!(self.sell_ref, LegRef::Merge) {
            return Err(CodecError::InvalidEnum);
        }
        for leg in [self.buy_ref, self.sell_ref] {
            if let LegRef::Order(index) = leg {
                if index >= order_len {
                    return Err(CodecError::InvalidCount);
                }
            }
        }
        if self.outcome >= outcome_count {
            return Err(CodecError::InvalidCount);
        }
        if self.quantity == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }
}

/// The header of one solver-written candidate feed.
///
/// Field for field this is `clutch_batch::relation_v1_stream::StreamCandidateV1`
/// with two additions this crate needs and that crate does not: the
/// `(candidate, epoch, market)` identity triple, and the `order_set` the fills
/// were computed against.  The claimed score is carried as its five components
/// rather than as an opaque blob, so a mismatch between a feed and the
/// [`crate::CandidateRecord`] of the same candidate is a field comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateFeedHeader {
    /// Candidate identity; the canonical digest of the free coordinates.
    ///
    /// The **same** digest [`crate::CandidateRecord::candidate`] carries, over
    /// the same preimage, so a feed and a record for one candidate agree by
    /// construction or one of them does not decode.
    pub candidate: Hash32,
    /// Epoch identity.
    pub epoch: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// The frozen order-set digest the fills below were computed against.
    pub order_set: Hash32,
    /// Exact scaled prices on the simplex; inactive outcomes are zero.
    pub prices: [u64; MAX_OUTCOMES],
    /// `sigma`: complete sets created by the single global virtual split.
    pub virtual_split: u64,
    /// `mu`: complete sets destroyed by the single global virtual merge.
    pub virtual_merge: u64,
    /// Honored minimum-fill subset, one bit per order.
    pub honored_aon_mask: u64,
    /// Claimed score component 1, net of the self-overlap term.
    pub weighted_direct_volume: i128,
    /// Claimed score component 3, in exact price units.
    pub limit_surplus_price_units: u128,
    /// Claimed score component 6: the canonical candidate digest, as the
    /// relation's own 128-bit permutation rather than as a SHA-256 identity.
    pub claimed_digest: u128,
    /// Claimed score component 5: `sigma + mu`.
    pub churn: u64,
    /// Declared explicit-witness length; meaningful only when
    /// [`CANDIDATE_FEED_FLAG_SLICES_DECLARED`] is set.
    pub declared_slices: u16,
    /// Claimed score component 4: distinct participating owners.
    pub distinct_owners: u16,
    /// Orders this candidate binds; must equal the frozen book length.
    pub order_len: u8,
    /// Active outcome count, in `2..=MAX_OUTCOMES`.
    pub outcome_count: u8,
    /// Stored PDA bump.
    pub stored_bump: u8,
    /// Flags; bit 0 is [`CANDIDATE_FEED_FLAG_SLICES_DECLARED`].
    pub flags: u8,
}

impl CandidateFeedHeader {
    /// Whether an explicit pairing witness is declared, and how long it is.
    ///
    /// The exact `Option<u16>` of `StreamCandidateV1::declared_slices`.
    pub const fn declared_slices(&self) -> Option<u16> {
        if self.flags & CANDIDATE_FEED_FLAG_SLICES_DECLARED != 0 {
            Some(self.declared_slices)
        } else {
            None
        }
    }

    /// Recompute the candidate identity from its free coordinates and domain.
    ///
    /// Deliberately the same preimage
    /// [`crate::CandidateRecord::recomputed_candidate_digest`] uses: one
    /// candidate has one identity whichever account states it.
    pub fn recomputed_candidate_digest(&self) -> Result<Hash32> {
        let mut body = [0; CANDIDATE_FEED_DIGEST_BODY_BYTES];
        let mut w = Writer::new(&mut body);
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.u8(self.order_len)?;
        w.u8(self.outcome_count)?;
        w.amounts(&self.prices)?;
        w.u64(self.virtual_split)?;
        w.u64(self.virtual_merge)?;
        w.u64(self.honored_aon_mask)?;
        if w.at != CANDIDATE_FEED_DIGEST_BODY_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(canonical_candidate_digest(&body))
    }

    /// Validate coordinates, mask width, canonical churn, and the identity.
    ///
    /// Every rule [`crate::CandidateRecord::validate`] applies to the shared
    /// coordinates, restated over these fields rather than shared through a
    /// conversion, plus the two facts only this account carries: a nonzero
    /// order set, and a declared witness length inside [`MAX_SLICES`].
    pub fn validate(&self) -> Result<()> {
        check_hash(self.epoch)?;
        check_hash(self.market)?;
        check_hash(self.order_set)?;
        check_count(self.outcome_count)?;
        if self.order_len == 0 || self.order_len as usize > MAX_EPOCH_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        if self.flags & !CANDIDATE_FEED_FLAG_SLICES_DECLARED != 0 {
            return Err(CodecError::InvalidEnum);
        }
        check_padded_amounts(&self.prices, self.outcome_count as usize)?;
        // A mask bit above the book length is a claim about an order that does
        // not exist; it is a leak, not padding to be ignored.
        if self.order_len < 64 && self.honored_aon_mask >> self.order_len != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        // Canonical churn: a candidate never splits and merges at once.
        if self.virtual_split != 0 && self.virtual_merge != 0 {
            return Err(CodecError::InvalidEnum);
        }
        let churn = self
            .virtual_split
            .checked_add(self.virtual_merge)
            .ok_or(CodecError::ArithmeticOverflow)?;
        if self.churn != churn {
            return Err(CodecError::MismatchedBinding);
        }
        if self.distinct_owners as usize > MAX_EPOCH_ORDERS {
            return Err(CodecError::InvalidCount);
        }
        match self.declared_slices() {
            Some(len) if len as usize > MAX_SLICES => return Err(CodecError::InvalidCount),
            // An undeclared witness has no length, so the field is padding.
            None if self.declared_slices != 0 => return Err(CodecError::NonCanonicalPadding),
            _ => {}
        }
        if self.candidate != self.recomputed_candidate_digest()? {
            return Err(CodecError::NonCanonicalIdentity);
        }
        Ok(())
    }

    /// Encode exactly [`CANDIDATE_FEED_HEADER_BYTES`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize> {
        self.validate()?;
        if out.len() < CANDIDATE_FEED_HEADER_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        let mut w = Writer::new(out);
        put_header(&mut w, CANDIDATE_FEED_TAG, account_version::CANDIDATE_FEED)?;
        w.hash(self.candidate)?;
        w.hash(self.epoch)?;
        w.hash(self.market)?;
        w.hash(self.order_set)?;
        w.amounts(&self.prices)?;
        w.u64(self.virtual_split)?;
        w.u64(self.virtual_merge)?;
        w.u64(self.honored_aon_mask)?;
        w.i128(self.weighted_direct_volume)?;
        w.u128(self.limit_surplus_price_units)?;
        w.u128(self.claimed_digest)?;
        w.u64(self.churn)?;
        w.u16(self.declared_slices)?;
        w.u16(self.distinct_owners)?;
        w.u8(self.order_len)?;
        w.u8(self.outcome_count)?;
        w.u8(self.stored_bump)?;
        w.u8(self.flags)?;
        if w.at != CANDIDATE_FEED_HEADER_BYTES {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(w.at)
    }

    /// Parse the header of a whole [`account_len::CANDIDATE_FEED`] account.
    pub fn decode(input: &[u8]) -> Result<Self> {
        check_header(
            input,
            CANDIDATE_FEED_TAG,
            account_version::CANDIDATE_FEED,
            account_len::CANDIDATE_FEED,
        )?;
        let mut r = Reader::at(input, 2);
        let value = Self {
            candidate: r.hash()?,
            epoch: r.hash()?,
            market: r.hash()?,
            order_set: r.hash()?,
            prices: r.amounts()?,
            virtual_split: r.u64()?,
            virtual_merge: r.u64()?,
            honored_aon_mask: r.u64()?,
            weighted_direct_volume: r.i128()?,
            limit_surplus_price_units: r.u128()?,
            claimed_digest: r.u128()?,
            churn: r.u64()?,
            declared_slices: r.u16()?,
            distinct_owners: r.u16()?,
            order_len: r.u8()?,
            outcome_count: r.u8()?,
            stored_bump: r.u8()?,
            flags: r.u8()?,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Bytes of the candidate-identity preimage.
///
/// Exactly `CandidateRecord`'s: epoch, market, order length, outcome count,
/// prices, sigma, mu, honored mask.  Restated here as a length so the two
/// encoders cannot drift apart silently; a codec test compares the digests
/// themselves rather than trusting this constant.
const CANDIDATE_FEED_DIGEST_BODY_BYTES: usize =
    (2 * HASH_BYTES) + 1 + 1 + (MAX_OUTCOMES * 8) + 8 + 8 + 8;

/// The whole candidate feed account, named for the doc table.
///
/// Like [`ClearWorkAccount`], a type-level name with no values: 6,266 bytes is
/// well past what a frame should hold, so a caller reads
/// [`verify_candidate_feed`] and walks with [`fill_at`] / [`slice_at`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateFeedAccount {}

/// Read one fill without materializing the fill vector.
///
/// `index` is the relation's order index — the order's **zero-based live
/// rank** in the canonical page-set walk (see the module docs), not its global
/// slot index; on a set with any tombstone the two disagree from the first
/// retirement on.  Fills at or beyond `order_len` are canonical zero padding and are refused
/// here rather than returned as zero, because a caller asking for one is
/// asking about an order the feed does not carry.
pub fn fill_at(input: &[u8], header: &CandidateFeedHeader, index: u8) -> Result<u64> {
    if input.len() != account_len::CANDIDATE_FEED {
        return Err(CodecError::Truncated);
    }
    if index >= header.order_len {
        return Err(CodecError::InvalidCount);
    }
    let mut r = Reader::at(input, FILLS_AT + (index as usize * 8));
    r.u64()
}

/// Read one slice without materializing the witness.
///
/// Refuses an index at or beyond the declared length for the same reason
/// [`fill_at`] does, and refuses at all when no witness is declared.
pub fn slice_at(input: &[u8], header: &CandidateFeedHeader, index: u16) -> Result<PairingSlice> {
    if input.len() != account_len::CANDIDATE_FEED {
        return Err(CodecError::Truncated);
    }
    let declared = header.declared_slices().ok_or(CodecError::InvalidEnum)?;
    if index >= declared {
        return Err(CodecError::InvalidCount);
    }
    decode_slice(input, index as usize)
}

/// Decode one slice at a raw index, without the declared-length rule.
fn decode_slice(input: &[u8], index: usize) -> Result<PairingSlice> {
    let mut r = Reader::at(input, SLICES_AT + (index * PAIRING_SLICE_BYTES));
    let buy_kind = r.u8()?;
    let buy_index = r.u8()?;
    let sell_kind = r.u8()?;
    let sell_index = r.u8()?;
    let outcome = r.u8()?;
    let quantity = r.u64()?;
    Ok(PairingSlice {
        buy_ref: decode_leg(buy_kind, buy_index)?,
        sell_ref: decode_leg(sell_kind, sell_index)?,
        outcome,
        quantity,
    })
}

/// Decode one leg reference; a virtual leg's index byte must be zero.
fn decode_leg(kind: u8, index: u8) -> Result<LegRef> {
    match kind {
        LEG_KIND_ORDER => Ok(LegRef::Order(index)),
        LEG_KIND_SPLIT | LEG_KIND_MERGE => {
            if index != 0 {
                return Err(CodecError::NonCanonicalPadding);
            }
            Ok(if kind == LEG_KIND_SPLIT {
                LegRef::Split
            } else {
                LegRef::Merge
            })
        }
        _ => Err(CodecError::WrongTag),
    }
}

/// Encode one slice into an already-framed account.
fn write_slice(account: &mut [u8], index: usize, slice: &PairingSlice) -> Result<()> {
    let at = SLICES_AT + (index * PAIRING_SLICE_BYTES);
    let end = at + PAIRING_SLICE_BYTES;
    if end > account.len() {
        return Err(CodecError::OutputTooSmall);
    }
    let mut w = Writer::new(&mut account[at..end]);
    w.u8(slice.buy_ref.kind())?;
    w.u8(slice.buy_ref.index())?;
    w.u8(slice.sell_ref.kind())?;
    w.u8(slice.sell_ref.index())?;
    w.u8(slice.outcome)?;
    w.u64(slice.quantity)
}

/// Verify a whole candidate feed account and return its header.
///
/// One pass over the fills and one over the slices, one element at a time.
/// Beyond [`CandidateFeedHeader::validate`] it decides the two array rules:
///
/// * every fill at or beyond `order_len` is zero, and
/// * every slice below the declared length validates, while every slice at or
///   beyond it is all-zero padding — including the whole vector when no
///   witness is declared at all.
///
/// It decides nothing about whether the fills are *correct*: a fill vector the
/// frozen book cannot justify passes here and is refused by the relation, which
/// is the only thing that could refuse it.
pub fn verify_candidate_feed(input: &[u8]) -> Result<CandidateFeedHeader> {
    let header = CandidateFeedHeader::decode(input)?;
    let mut index = 0;
    while index < MAX_EPOCH_ORDERS {
        let mut r = Reader::at(input, FILLS_AT + (index * 8));
        let fill = r.u64()?;
        if index >= header.order_len as usize && fill != 0 {
            return Err(CodecError::NonCanonicalPadding);
        }
        index += 1;
    }
    let declared = header.declared_slices().unwrap_or(0) as usize;
    let mut index = 0;
    while index < MAX_SLICES {
        let slice = decode_slice(input, index)?;
        if index < declared {
            slice.validate(header.order_len, header.outcome_count)?;
        } else if slice != PairingSlice::PADDING {
            return Err(CodecError::NonCanonicalPadding);
        }
        index += 1;
    }
    Ok(header)
}

/// Write a fresh candidate feed: header, zero fills, zero slices.
///
/// The feed's arrays are written afterwards by [`write_fill`] and
/// [`write_slice_at`], one element per call, so no caller ever holds either
/// vector.  A feed straight out of this call verifies — an all-zero fill vector
/// is a legal candidate (it claims nothing filled) — which is what makes
/// "written but not yet populated" a state rather than a corrupt account.
pub fn init_candidate_feed(
    account: &mut [u8],
    header: &CandidateFeedHeader,
) -> Result<CandidateFeedHeader> {
    if account.len() != account_len::CANDIDATE_FEED {
        return Err(CodecError::OutputTooSmall);
    }
    header.encode(account)?;
    account[FILLS_AT..].fill(0);
    Ok(*header)
}

/// Write one fill into an already-framed feed.
///
/// Refuses an index the feed does not carry, exactly as [`fill_at`] refuses to
/// read one: the padding rule is enforced on the write side too, so a feed can
/// never be walked into a state [`verify_candidate_feed`] would refuse.
pub fn write_fill(account: &mut [u8], index: u8, fill: u64) -> Result<()> {
    let header = CandidateFeedHeader::decode(account)?;
    if index >= header.order_len {
        return Err(CodecError::InvalidCount);
    }
    let at = FILLS_AT + (index as usize * 8);
    let mut w = Writer::new(&mut account[at..at + 8]);
    w.u64(fill)
}

/// Write one slice into an already-framed feed.
///
/// Refuses an index at or beyond the declared witness length, refuses when no
/// witness is declared, and refuses a slice the feed's own coordinates cannot
/// admit ([`PairingSlice::validate`]).
pub fn write_slice_at(account: &mut [u8], index: u16, slice: &PairingSlice) -> Result<()> {
    let header = CandidateFeedHeader::decode(account)?;
    let declared = header.declared_slices().ok_or(CodecError::InvalidEnum)?;
    if index >= declared {
        return Err(CodecError::InvalidCount);
    }
    slice.validate(header.order_len, header.outcome_count)?;
    write_slice(account, index as usize, slice)
}

/// A forward cursor over one feed's live fills.
///
/// The fill-side counterpart of [`crate::stream::OrderSlotCursor`]: one `u64`
/// per step, no vector anywhere.
#[derive(Clone, Copy, Debug)]
pub struct FillCursor<'a> {
    input: &'a [u8],
    order_len: u8,
    at: u8,
}

impl<'a> FillCursor<'a> {
    /// Open a cursor over an already-verified feed.
    pub fn new(input: &'a [u8], header: &CandidateFeedHeader) -> Self {
        Self {
            input,
            order_len: header.order_len,
            at: 0,
        }
    }
}

impl Iterator for FillCursor<'_> {
    type Item = Result<u64>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.order_len {
            return None;
        }
        let mut r = Reader::at(self.input, FILLS_AT + (self.at as usize * 8));
        self.at += 1;
        Some(r.u64())
    }
}

/// A forward cursor over one feed's declared pairing slices.
#[derive(Clone, Copy, Debug)]
pub struct SliceCursor<'a> {
    input: &'a [u8],
    declared: u16,
    at: u16,
}

impl<'a> SliceCursor<'a> {
    /// Open a cursor over an already-verified feed; empty when none declared.
    pub fn new(input: &'a [u8], header: &CandidateFeedHeader) -> Self {
        Self {
            input,
            declared: header.declared_slices().unwrap_or(0),
            at: 0,
        }
    }
}

impl Iterator for SliceCursor<'_> {
    type Item = Result<PairingSlice>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.declared {
            return None;
        }
        let index = self.at as usize;
        self.at += 1;
        Some(decode_slice(self.input, index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CandidateRecord, CANDIDATE_STATUS_SUBMITTED, MAX_PRICE_SCALE};

    /// One named mutation of a valid feed header and the refusal it earns.
    type HostileCoordinate = (&'static str, fn(&mut CandidateFeedHeader), CodecError);

    fn h(byte: u8) -> Hash32 {
        Hash32([byte; HASH_BYTES])
    }

    fn prices(outcomes: usize) -> [u64; MAX_OUTCOMES] {
        let mut out = [0; MAX_OUTCOMES];
        let mut i = 0;
        while i < outcomes {
            out[i] = 1_000;
            i += 1;
        }
        out
    }

    fn feed_header() -> CandidateFeedHeader {
        let mut header = CandidateFeedHeader {
            candidate: Hash32::ZERO,
            epoch: h(2),
            market: h(3),
            order_set: h(4),
            prices: prices(4),
            virtual_split: 7,
            virtual_merge: 0,
            honored_aon_mask: 0b101,
            weighted_direct_volume: -9,
            limit_surplus_price_units: 11,
            claimed_digest: 13,
            churn: 7,
            declared_slices: 0,
            distinct_owners: 3,
            order_len: 5,
            outcome_count: 4,
            stored_bump: 254,
            flags: 0,
        };
        header.candidate = header.recomputed_candidate_digest().unwrap();
        header
    }

    fn feed(header: &CandidateFeedHeader) -> [u8; account_len::CANDIDATE_FEED] {
        let mut account = [0; account_len::CANDIDATE_FEED];
        init_candidate_feed(&mut account, header).unwrap();
        account
    }

    fn work() -> [u8; account_len::CLEAR_WORK] {
        let mut account = [0; account_len::CLEAR_WORK];
        init_clear_work(&mut account, h(1), h(2), h(3), 253).unwrap();
        account
    }

    /* ------------------------------------------------------------------ */
    /* Sizes                                                               */
    /* ------------------------------------------------------------------ */

    #[test]
    fn clearing_account_golden_lengths() {
        // 47,846 is `ClearWorkV1::ENCODED_BYTES`, pinned on the other side by
        // clutch-batch's `clear_work_encoded_bytes_are_pinned` — and asserted
        // against the symbol itself, not only restated, so the two crates
        // cannot drift silently.
        assert_eq!(CLEAR_WORK_BODY_BYTES, 47_846);
        assert_eq!(
            CLEAR_WORK_BODY_BYTES,
            clutch_batch::relation_v1_stream::ClearWorkV1::ENCODED_BYTES
        );
        assert_eq!(CLEAR_WORK_HEADER_BYTES, 158);
        // The owner-interning region: count plus 64 owners of 32 bytes.
        assert_eq!(CLEAR_WORK_INTERNER_BYTES, 2 + 64 * 32);
        assert_eq!(account_len::CLEAR_WORK, 50_054);
        assert_eq!(CANDIDATE_FEED_HEADER_BYTES, 346);
        assert_eq!(PAIRING_SLICE_BYTES, 13);
        assert_eq!(MAX_SLICES, 416);
        assert_eq!(account_len::CANDIDATE_FEED, 6_266);
        // The feed is exactly its header, 64 fills, and 416 slices.
        assert_eq!(
            account_len::CANDIDATE_FEED,
            CANDIDATE_FEED_HEADER_BYTES
                + (MAX_EPOCH_ORDERS * 8)
                + (MAX_SLICES * PAIRING_SLICE_BYTES)
        );
        // The checkpoint is exactly its header, the interning region, and
        // the opaque body.
        assert_eq!(
            account_len::CLEAR_WORK,
            CLEAR_WORK_HEADER_BYTES + CLEAR_WORK_INTERNER_BYTES + CLEAR_WORK_BODY_BYTES
        );
        assert_eq!(EPOCH_WINDOW_ACCOUNT_BYTES, 84);
    }

    /// The one account in the inventory that no single system-program creation
    /// can allocate through a CPI.  Pinned as arithmetic rather than prose so
    /// that a future body growth trips a test rather than a mainnet failure.
    #[test]
    fn only_the_checkpoint_exceeds_the_cpi_creation_ceiling() {
        /// `solana_program_entrypoint::MAX_PERMITTED_DATA_INCREASE`: the most
        /// an account's data may grow inside one instruction, which is also
        /// the most a program can allocate for an account through a
        /// system-program CPI.
        const CPI_CEILING: usize = 10 * 1024;
        let lengths = [
            ("clear work", account_len::CLEAR_WORK, false),
            ("candidate feed", account_len::CANDIDATE_FEED, true),
            ("order page", account_len::ORDER_PAGE, true),
            ("terms", account_len::TERMS, true),
            ("price grid", account_len::PRICE_GRID, true),
        ];
        for (label, len, fits) in lengths {
            assert_eq!(len <= CPI_CEILING, fits, "{label}");
        }
        // Create at the ceiling, then grow by the ceiling: five instructions.
        assert_eq!(account_len::CLEAR_WORK.div_ceil(CPI_CEILING), 5);
    }

    /* ------------------------------------------------------------------ */
    /* Checkpoint                                                          */
    /* ------------------------------------------------------------------ */

    #[test]
    fn a_fresh_checkpoint_is_open_bound_once_and_completes_once() {
        let mut account = work();
        let header = verify_clear_work(&account).unwrap();
        assert_eq!(header.status, CLEAR_WORK_STATUS_OPEN);
        assert_eq!(header.order_set, Hash32::ZERO);
        assert_eq!(header.consumed_fold, 0);
        assert_eq!(header.body_len as usize, CLEAR_WORK_BODY_BYTES);
        assert_eq!(
            clear_work_body(&account).unwrap().len(),
            CLEAR_WORK_BODY_BYTES
        );
        assert!(clear_work_body(&account).unwrap().iter().all(|b| *b == 0));

        // An open checkpoint has nothing to continue.
        assert_eq!(
            require_continuation(&header, h(9)),
            Err(CodecError::MismatchedBinding)
        );
        // Completing before binding is refused: no verdict without an anchor.
        assert_eq!(
            complete_clear_work(&mut account),
            Err(CodecError::MismatchedBinding)
        );

        let bound = bind_order_set(&mut account, h(9), 0x1234_5678_9abc_def0).unwrap();
        assert_eq!(bound.status, CLEAR_WORK_STATUS_BOUND);
        assert_eq!(bound.order_set, h(9));
        assert_eq!(bound.consumed_fold, 0x1234_5678_9abc_def0);
        assert_eq!(require_continuation(&bound, h(9)), Ok(()));
        assert_eq!(
            require_continuation(&bound, h(10)),
            Err(CodecError::MismatchedBinding)
        );
        // Re-binding is a one-way transition, not a mutable field.
        assert_eq!(
            bind_order_set(&mut account, h(10), 1),
            Err(CodecError::MismatchedBinding)
        );
        assert!(bind_order_set(&mut account, Hash32::ZERO, 1).is_err());

        let done = complete_clear_work(&mut account).unwrap();
        assert_eq!(done.status, CLEAR_WORK_STATUS_COMPLETE);
        assert_eq!(
            complete_clear_work(&mut account),
            Err(CodecError::MismatchedBinding)
        );
        // A completed checkpoint accepts no further walk.
        assert_eq!(
            advance_walk(&mut account, 1, 0, 1),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn the_walk_cursor_only_moves_forward() {
        let mut account = work();
        assert_eq!(advance_walk(&mut account, 0, 1, 1).unwrap().slot_cursor, 1);
        assert_eq!(advance_walk(&mut account, 0, 5, 4).unwrap().live_rank, 4);
        assert_eq!(advance_walk(&mut account, 1, 0, 4).unwrap().page_cursor, 1);
        // Same position, earlier position, and a live rank that fell.
        assert_eq!(
            advance_walk(&mut account, 1, 0, 4),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            advance_walk(&mut account, 0, 9, 4),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            advance_walk(&mut account, 1, 1, 3),
            Err(CodecError::MismatchedBinding)
        );
        // Past the last page, and past the last slot of a page.
        assert_eq!(
            advance_walk(&mut account, MAX_ORDER_PAGES as u16 + 1, 0, 4),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            advance_walk(&mut account, 2, MAX_ORDERS_PER_PAGE as u8 + 1, 4),
            Err(CodecError::InvalidCount)
        );
        // The terminal position is one past the last page, at slot zero only.
        assert_eq!(
            advance_walk(&mut account, MAX_ORDER_PAGES as u16, 1, 4),
            Err(CodecError::NonCanonicalPadding)
        );
        assert!(advance_walk(&mut account, MAX_ORDER_PAGES as u16, 0, 4).is_ok());
    }

    #[test]
    fn the_checkpoint_refuses_every_hostile_frame() {
        let good = work();
        assert!(verify_clear_work(&good).is_ok());

        let mut short = [0; account_len::CLEAR_WORK - 1];
        short.copy_from_slice(&good[..account_len::CLEAR_WORK - 1]);
        assert_eq!(verify_clear_work(&short), Err(CodecError::Truncated));

        let mut long = [0; account_len::CLEAR_WORK + 1];
        long[..account_len::CLEAR_WORK].copy_from_slice(&good);
        assert_eq!(verify_clear_work(&long), Err(CodecError::TrailingBytes));

        let mut wrong_tag = good;
        wrong_tag[0] = CANDIDATE_FEED_TAG;
        assert_eq!(verify_clear_work(&wrong_tag), Err(CodecError::WrongTag));

        let mut wrong_version = good;
        wrong_version[1] = account_version::CLEAR_WORK + 1;
        assert_eq!(
            verify_clear_work(&wrong_version),
            Err(CodecError::WrongVersion)
        );

        // A zero identity anywhere in the triple.
        for at in [2, 34, 66] {
            let mut zeroed = good;
            zeroed[at..at + HASH_BYTES].fill(0);
            assert_eq!(verify_clear_work(&zeroed), Err(CodecError::ZeroIdentity));
        }

        // A fold with no order set behind it.
        let mut floating_fold = good;
        floating_fold[130] = 1;
        assert_eq!(
            verify_clear_work(&floating_fold),
            Err(CodecError::NonCanonicalPadding)
        );

        // A body length that is not this build's.
        let mut wrong_body = good;
        wrong_body[146..150].copy_from_slice(&(CLEAR_WORK_BODY_BYTES as u32 - 1).to_le_bytes());
        assert_eq!(
            verify_clear_work(&wrong_body),
            Err(CodecError::InvalidCount)
        );

        // Reserved bits and an unknown status.
        let mut flagged = good;
        flagged[157] = 1;
        assert_eq!(verify_clear_work(&flagged), Err(CodecError::InvalidEnum));
        let mut statused = good;
        statused[155] = CLEAR_WORK_STATUS_COMPLETE + 1;
        assert_eq!(verify_clear_work(&statused), Err(CodecError::InvalidEnum));
    }

    /// The body is opaque, and that is a property with a test: every possible
    /// body byte pattern decodes to the same header.
    #[test]
    fn every_body_pattern_leaves_the_header_verdict_unchanged() {
        let mut account = work();
        let header = verify_clear_work(&account).unwrap();
        for pattern in [0x00_u8, 0x01, 0x7f, 0xff, 0xa5] {
            clear_work_body_mut(&mut account).unwrap().fill(pattern);
            assert_eq!(verify_clear_work(&account), Ok(header));
            assert!(clear_work_body(&account)
                .unwrap()
                .iter()
                .all(|b| *b == pattern));
        }
    }

    /* ------------------------------------------------------------------ */
    /* Grow stage                                                          */
    /* ------------------------------------------------------------------ */

    fn grow_stage_account() -> [u8; CLEAR_WORK_GROW_STEP] {
        let mut account = [0; CLEAR_WORK_GROW_STEP];
        init_clear_work_grow_stage(&mut account, h(1), h(2), h(3), 253).unwrap();
        account
    }

    #[test]
    fn the_grow_stage_walks_exactly_five_lengths_to_the_target() {
        let mut len = CLEAR_WORK_GROW_STEP;
        let mut stages = 1;
        while len < account_len::CLEAR_WORK {
            assert_eq!(clear_work_grow_stage_len(len), Ok(()));
            len = clear_work_grown_len(len);
            stages += 1;
        }
        assert_eq!(len, account_len::CLEAR_WORK);
        assert_eq!(stages, 5);
        // The finished length is not a stage, and one more grow is a no-op cap.
        assert_eq!(
            clear_work_grow_stage_len(len),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(clear_work_grown_len(len), account_len::CLEAR_WORK);
    }

    #[test]
    fn a_grow_stage_round_trips_at_every_length_and_every_checkpoint_reader_refuses_it() {
        let first = grow_stage_account();
        let stage = ClearWorkGrowStage::decode(&first).unwrap();
        assert_eq!(stage.market, h(1));
        assert_eq!(stage.epoch, h(2));
        assert_eq!(stage.candidate, h(3));
        assert_eq!(stage.target_len as usize, account_len::CLEAR_WORK);
        assert_eq!(stage.stored_bump, 253);
        assert!(first[CLEAR_WORK_GROW_PREFIX_BYTES..]
            .iter()
            .all(|b| *b == 0));

        // Simulate every realloc: a longer account carrying the same prefix.
        let mut grown = [0u8; 4 * CLEAR_WORK_GROW_STEP];
        grown[..CLEAR_WORK_GROW_STEP].copy_from_slice(&first);
        let mut len = CLEAR_WORK_GROW_STEP;
        loop {
            // The stage decodes, and the half-grown account is inert: every
            // checkpoint reader and writer refuses it on the exact-length
            // rule before interpreting one byte.
            assert_eq!(ClearWorkGrowStage::decode(&grown[..len]), Ok(stage));
            assert_eq!(
                verify_clear_work(&grown[..len]),
                Err(CodecError::Truncated)
            );
            assert_eq!(clear_work_body(&grown[..len]), Err(CodecError::Truncated));
            assert_eq!(
                clear_work_body_mut(&mut grown[..len]),
                Err(CodecError::Truncated)
            );
            assert_eq!(
                bind_order_set(&mut grown[..len], h(9), 1),
                Err(CodecError::Truncated)
            );
            assert_eq!(
                advance_walk(&mut grown[..len], 0, 1, 1),
                Err(CodecError::Truncated)
            );
            assert_eq!(
                complete_clear_work(&mut grown[..len]),
                Err(CodecError::Truncated)
            );
            if len == 4 * CLEAR_WORK_GROW_STEP {
                break;
            }
            len = clear_work_grown_len(len);
        }
    }

    #[test]
    fn the_grow_stage_refuses_every_hostile_frame() {
        // A finished checkpoint never decodes as a stage: its exact length
        // fails the staged-length rule before any byte is read.
        let finished = work();
        assert_eq!(
            ClearWorkGrowStage::decode(&finished),
            Err(CodecError::InvalidCount)
        );
        // Off-multiple and out-of-range lengths refuse.
        let good = grow_stage_account();
        assert_eq!(
            ClearWorkGrowStage::decode(&good[..CLEAR_WORK_GROW_STEP - 1]),
            Err(CodecError::InvalidCount)
        );
        let mut long = [0u8; CLEAR_WORK_GROW_STEP + 1];
        long[..CLEAR_WORK_GROW_STEP].copy_from_slice(&good);
        assert_eq!(
            ClearWorkGrowStage::decode(&long),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(
            ClearWorkGrowStage::decode(&[0u8; 0]),
            Err(CodecError::InvalidCount)
        );

        let mut wrong_tag = good;
        wrong_tag[0] = CANDIDATE_FEED_TAG;
        assert_eq!(
            ClearWorkGrowStage::decode(&wrong_tag),
            Err(CodecError::WrongTag)
        );
        let mut wrong_version = good;
        wrong_version[1] = account_version::CLEAR_WORK + 1;
        assert_eq!(
            ClearWorkGrowStage::decode(&wrong_version),
            Err(CodecError::WrongVersion)
        );
        let mut wrong_marker = good;
        wrong_marker[2] = 0;
        assert_eq!(
            ClearWorkGrowStage::decode(&wrong_marker),
            Err(CodecError::InvalidEnum)
        );
        let mut wrong_target = good;
        wrong_target[3..7].copy_from_slice(&(account_len::CLEAR_WORK as u32 - 1).to_le_bytes());
        assert_eq!(
            ClearWorkGrowStage::decode(&wrong_target),
            Err(CodecError::InvalidCount)
        );
        // A zero identity anywhere in the triple.
        for at in [3 + 4, 3 + 4 + HASH_BYTES, 3 + 4 + (2 * HASH_BYTES)] {
            let mut zeroed = good;
            zeroed[at..at + HASH_BYTES].fill(0);
            assert_eq!(
                ClearWorkGrowStage::decode(&zeroed),
                Err(CodecError::ZeroIdentity)
            );
        }

        // The initializer only writes the exact first stage.
        let mut short = [0u8; CLEAR_WORK_GROW_STEP - 1];
        assert_eq!(
            init_clear_work_grow_stage(&mut short, h(1), h(2), h(3), 253),
            Err(CodecError::OutputTooSmall)
        );
        let mut wide = [0u8; CLEAR_WORK_GROW_STEP + 1];
        assert_eq!(
            init_clear_work_grow_stage(&mut wide, h(1), h(2), h(3), 253),
            Err(CodecError::OutputTooSmall)
        );
        let mut zero_identity = [0u8; CLEAR_WORK_GROW_STEP];
        assert_eq!(
            init_clear_work_grow_stage(&mut zero_identity, Hash32::ZERO, h(2), h(3), 253),
            Err(CodecError::ZeroIdentity)
        );
    }

    /* ------------------------------------------------------------------ */
    /* Candidate feed                                                      */
    /* ------------------------------------------------------------------ */

    #[test]
    fn a_feed_round_trips_and_shares_the_records_identity() {
        let header = feed_header();
        let account = feed(&header);
        assert_eq!(
            &account[..2],
            [CANDIDATE_FEED_TAG, account_version::CANDIDATE_FEED]
        );
        assert_eq!(verify_candidate_feed(&account), Ok(header));
        assert_eq!(CandidateFeedHeader::decode(&account), Ok(header));

        /* One candidate, one identity: the record and the feed derive the same
         * digest from the same coordinates, over the same preimage. */
        let record = CandidateRecord {
            candidate: header.candidate,
            epoch: header.epoch,
            market: header.market,
            prices: header.prices,
            virtual_split: header.virtual_split,
            virtual_merge: header.virtual_merge,
            honored_aon_mask: header.honored_aon_mask,
            weighted_direct_volume: header.weighted_direct_volume,
            limit_surplus_price_units: header.limit_surplus_price_units,
            churn: header.churn,
            submitted_slot: 42,
            distinct_owners: header.distinct_owners,
            order_len: header.order_len,
            outcome_count: header.outcome_count,
            status: CANDIDATE_STATUS_SUBMITTED,
            stored_bump: 251,
            flags: 0,
        };
        assert_eq!(
            record.recomputed_candidate_digest().unwrap(),
            header.recomputed_candidate_digest().unwrap()
        );
        let mut record_bytes = [0; account_len::CANDIDATE];
        record.encode(&mut record_bytes).unwrap();
        assert_eq!(
            CandidateRecord::decode(&record_bytes).unwrap().candidate,
            header.candidate
        );
    }

    #[test]
    fn fills_are_walkable_and_padding_is_enforced_on_both_sides() {
        let header = feed_header();
        let mut account = feed(&header);
        for index in 0..header.order_len {
            write_fill(&mut account, index, 10 + index as u64).unwrap();
        }
        assert_eq!(
            write_fill(&mut account, header.order_len, 1),
            Err(CodecError::InvalidCount)
        );
        assert_eq!(verify_candidate_feed(&account), Ok(header));

        let mut walked = [0_u64; MAX_EPOCH_ORDERS];
        let mut count = 0;
        for fill in FillCursor::new(&account, &header) {
            walked[count] = fill.unwrap();
            count += 1;
        }
        assert_eq!(count, header.order_len as usize);
        assert_eq!(&walked[..count], &[10, 11, 12, 13, 14]);
        for index in 0..header.order_len {
            assert_eq!(fill_at(&account, &header, index), Ok(10 + index as u64));
        }
        assert_eq!(
            fill_at(&account, &header, header.order_len),
            Err(CodecError::InvalidCount)
        );

        // A fill smuggled past the book length is not padding to be ignored.
        let at = FILLS_AT + (header.order_len as usize * 8);
        account[at] = 1;
        assert_eq!(
            verify_candidate_feed(&account),
            Err(CodecError::NonCanonicalPadding)
        );
    }

    #[test]
    fn a_declared_witness_walks_and_an_undeclared_one_refuses_every_slice() {
        let mut header = feed_header();
        header.flags = CANDIDATE_FEED_FLAG_SLICES_DECLARED;
        header.declared_slices = 2;
        let mut account = feed(&header);

        let first = PairingSlice {
            buy_ref: LegRef::Order(1),
            sell_ref: LegRef::Order(2),
            outcome: 3,
            quantity: 5,
        };
        let second = PairingSlice {
            buy_ref: LegRef::Merge,
            sell_ref: LegRef::Split,
            outcome: 0,
            quantity: 9,
        };
        write_slice_at(&mut account, 0, &first).unwrap();
        write_slice_at(&mut account, 1, &second).unwrap();
        assert_eq!(verify_candidate_feed(&account), Ok(header));
        assert_eq!(slice_at(&account, &header, 0), Ok(first));
        assert_eq!(slice_at(&account, &header, 1), Ok(second));
        assert_eq!(
            slice_at(&account, &header, 2),
            Err(CodecError::InvalidCount)
        );
        let mut walked = [PairingSlice::PADDING; 4];
        let mut count = 0;
        for slice in SliceCursor::new(&account, &header) {
            walked[count] = slice.unwrap();
            count += 1;
        }
        assert_eq!(count, 2);
        assert_eq!(&walked[..2], &[first, second]);

        // A slice written past the declared length is refused on the way in.
        assert_eq!(
            write_slice_at(&mut account, 2, &first),
            Err(CodecError::InvalidCount)
        );
        // And smuggled in by hand it is caught by the padding rule.
        let mut smuggled = account;
        write_slice(&mut smuggled, 2, &first).unwrap();
        assert_eq!(
            verify_candidate_feed(&smuggled),
            Err(CodecError::NonCanonicalPadding)
        );

        // An undeclared witness has no slices at all, and says so.
        let plain = feed_header();
        let plain_account = feed(&plain);
        assert_eq!(
            slice_at(&plain_account, &plain, 0),
            Err(CodecError::InvalidEnum)
        );
        assert_eq!(SliceCursor::new(&plain_account, &plain).count(), 0);
        let mut writable = plain_account;
        assert_eq!(
            write_slice_at(&mut writable, 0, &first),
            Err(CodecError::InvalidEnum)
        );

        /* A declared witness of length zero is a different feed from an
         * undeclared one: it asserts an empty decomposition. */
        let mut empty = feed_header();
        empty.flags = CANDIDATE_FEED_FLAG_SLICES_DECLARED;
        empty.declared_slices = 0;
        let empty_account = feed(&empty);
        assert_eq!(empty.declared_slices(), Some(0));
        assert_eq!(plain.declared_slices(), None);
        assert_eq!(verify_candidate_feed(&empty_account), Ok(empty));
        assert_ne!(empty_account[345], plain_account[345]);
    }

    #[test]
    fn the_feed_refuses_every_slice_the_relation_shape_forbids() {
        let mut header = feed_header();
        header.flags = CANDIDATE_FEED_FLAG_SLICES_DECLARED;
        header.declared_slices = 1;
        let mut account = feed(&header);
        let refused: [(&str, PairingSlice, CodecError); 5] = [
            (
                "the split is not a buy leg",
                PairingSlice {
                    buy_ref: LegRef::Split,
                    sell_ref: LegRef::Order(0),
                    outcome: 0,
                    quantity: 1,
                },
                CodecError::InvalidEnum,
            ),
            (
                "the merge is not a sell leg",
                PairingSlice {
                    buy_ref: LegRef::Order(0),
                    sell_ref: LegRef::Merge,
                    outcome: 0,
                    quantity: 1,
                },
                CodecError::InvalidEnum,
            ),
            (
                "an order index above the book length",
                PairingSlice {
                    buy_ref: LegRef::Order(header.order_len),
                    sell_ref: LegRef::Order(0),
                    outcome: 0,
                    quantity: 1,
                },
                CodecError::InvalidCount,
            ),
            (
                "an inactive outcome",
                PairingSlice {
                    buy_ref: LegRef::Order(0),
                    sell_ref: LegRef::Order(1),
                    outcome: header.outcome_count,
                    quantity: 1,
                },
                CodecError::InvalidCount,
            ),
            (
                "a live slice that moves nothing",
                PairingSlice {
                    buy_ref: LegRef::Order(0),
                    sell_ref: LegRef::Order(1),
                    outcome: 0,
                    quantity: 0,
                },
                CodecError::ZeroValue,
            ),
        ];
        for (label, slice, error) in refused {
            assert_eq!(
                write_slice_at(&mut account, 0, &slice),
                Err(error),
                "{label}"
            );
            // Hand-written into the bytes, the whole-account verify agrees.
            let mut smuggled = account;
            write_slice(&mut smuggled, 0, &slice).unwrap();
            assert_eq!(verify_candidate_feed(&smuggled), Err(error), "{label}");
        }

        // An unknown leg kind, and a virtual leg carrying an index.
        let mut unknown = account;
        unknown[SLICES_AT] = 3;
        assert_eq!(verify_candidate_feed(&unknown), Err(CodecError::WrongTag));
        let mut indexed = account;
        indexed[SLICES_AT] = LEG_KIND_MERGE;
        indexed[SLICES_AT + 1] = 1;
        assert_eq!(
            verify_candidate_feed(&indexed),
            Err(CodecError::NonCanonicalPadding)
        );
    }

    #[test]
    fn the_feed_header_refuses_every_hostile_coordinate() {
        let good = feed_header();
        let account = feed(&good);
        assert!(verify_candidate_feed(&account).is_ok());

        let cases: [HostileCoordinate; 9] = [
            (
                "a zero order set",
                |h| h.order_set = Hash32::ZERO,
                CodecError::ZeroIdentity,
            ),
            (
                "an empty book",
                |h| h.order_len = 0,
                CodecError::InvalidCount,
            ),
            (
                "a book above the relation bound",
                |h| h.order_len = MAX_EPOCH_ORDERS as u8 + 1,
                CodecError::InvalidCount,
            ),
            (
                "one outcome",
                |h| h.outcome_count = 1,
                CodecError::InvalidCount,
            ),
            (
                "a price on an inactive outcome",
                |h| h.prices[MAX_OUTCOMES - 1] = 1,
                CodecError::NonCanonicalPadding,
            ),
            (
                "a mask bit above the book length",
                |h| h.honored_aon_mask = 1 << 63,
                CodecError::NonCanonicalPadding,
            ),
            (
                "splitting and merging at once",
                |h| h.virtual_merge = 1,
                CodecError::InvalidEnum,
            ),
            (
                "churn that is not sigma plus mu",
                |h| h.churn += 1,
                CodecError::MismatchedBinding,
            ),
            (
                "a reserved flag bit",
                |h| h.flags = 0x80,
                CodecError::InvalidEnum,
            ),
        ];
        for (label, mutate, error) in cases {
            let mut header = good;
            mutate(&mut header);
            let mut out = [0; CANDIDATE_FEED_HEADER_BYTES];
            assert_eq!(header.encode(&mut out), Err(error), "{label} (encode)");
        }

        /* The identity is self-certifying: moving a coordinate without moving
         * the digest is refused, and moving both is a *different* candidate
         * rather than a forged one. */
        let mut moved = good;
        moved.virtual_split += 1;
        moved.churn += 1;
        let mut out = [0; CANDIDATE_FEED_HEADER_BYTES];
        assert_eq!(
            moved.encode(&mut out),
            Err(CodecError::NonCanonicalIdentity)
        );
        moved.candidate = moved.recomputed_candidate_digest().unwrap();
        assert!(moved.encode(&mut out).is_ok());
        assert_ne!(moved.candidate, good.candidate);

        // An undeclared witness may not carry a length.
        let mut lengthy = good;
        lengthy.declared_slices = 1;
        assert_eq!(
            lengthy.encode(&mut out),
            Err(CodecError::NonCanonicalPadding)
        );
        // A declared witness may not exceed the relation's slice bound.
        let mut wide = good;
        wide.flags = CANDIDATE_FEED_FLAG_SLICES_DECLARED;
        wide.declared_slices = MAX_SLICES as u16 + 1;
        assert_eq!(wide.encode(&mut out), Err(CodecError::InvalidCount));
    }

    #[test]
    fn the_feed_refuses_every_hostile_frame() {
        let header = feed_header();
        let good = feed(&header);

        let mut short = [0; account_len::CANDIDATE_FEED - 1];
        short.copy_from_slice(&good[..account_len::CANDIDATE_FEED - 1]);
        assert_eq!(verify_candidate_feed(&short), Err(CodecError::Truncated));

        let mut long = [0; account_len::CANDIDATE_FEED + 1];
        long[..account_len::CANDIDATE_FEED].copy_from_slice(&good);
        assert_eq!(verify_candidate_feed(&long), Err(CodecError::TrailingBytes));

        let mut wrong_tag = good;
        wrong_tag[0] = CLEAR_WORK_TAG;
        assert_eq!(verify_candidate_feed(&wrong_tag), Err(CodecError::WrongTag));

        let mut wrong_version = good;
        wrong_version[1] = account_version::CANDIDATE_FEED + 1;
        assert_eq!(
            verify_candidate_feed(&wrong_version),
            Err(CodecError::WrongVersion)
        );

        // A price above the widest scale a simplex sum can hold.
        let mut huge = header;
        huge.prices[0] = MAX_PRICE_SCALE;
        huge.candidate = huge.recomputed_candidate_digest().unwrap();
        let mut out = [0; CANDIDATE_FEED_HEADER_BYTES];
        assert!(huge.encode(&mut out).is_ok());
        /* Stated rather than checked: the simplex is a *domain* fact and this
         * account does not carry the epoch's price scale, so the codec cannot
         * decide it.  `CandidateRecord::binds_epoch` is where that check
         * lives, and this account inherits it through the shared identity. */
    }

    #[test]
    fn an_init_over_the_wrong_buffer_size_refuses() {
        let header = feed_header();
        let mut small = [0; account_len::CANDIDATE_FEED - 1];
        assert_eq!(
            init_candidate_feed(&mut small, &header),
            Err(CodecError::OutputTooSmall)
        );
        let mut small_work = [0; account_len::CLEAR_WORK - 1];
        assert_eq!(
            init_clear_work(&mut small_work, h(1), h(2), h(3), 0),
            Err(CodecError::OutputTooSmall)
        );
    }

    /* ------------------------------------------------------------------ */
    /* The owner-interning region and the pass-boundary rewind             */
    /* ------------------------------------------------------------------ */

    #[test]
    fn the_interner_region_round_trips_and_starts_empty() {
        let mut account = work();
        // A fresh checkpoint's zeroed region is exactly the empty table.
        let empty = read_owner_interner(&account).unwrap();
        assert_eq!(empty.count(), 0);
        assert_eq!(empty.owners(), &[]);

        let mut owners = OwnerInterner::new();
        assert_eq!(owners.intern(h(0xA1)).unwrap(), 0);
        assert_eq!(owners.intern(h(0xA2)).unwrap(), 1);
        assert_eq!(owners.intern(h(0xA1)).unwrap(), 0);
        write_owner_interner(&mut account, &owners).unwrap();
        let restored = read_owner_interner(&account).unwrap();
        assert_eq!(restored, owners);
        // A resumed pass reproduces pass-1's exact tags from the region.
        let mut resumed = restored;
        assert_eq!(resumed.intern(h(0xA2)).unwrap(), 1);
        assert_eq!(resumed.intern(h(0xA3)).unwrap(), 2);
        // The region write moved neither the header nor the body.
        let header = verify_clear_work(&account).unwrap();
        assert_eq!(header.market, h(1));
        assert!(clear_work_body(&account).unwrap().iter().all(|b| *b == 0));
    }

    #[test]
    fn the_interner_region_refuses_hostile_images() {
        let mut account = work();
        let mut owners = OwnerInterner::new();
        owners.intern(h(0xB1)).unwrap();
        write_owner_interner(&mut account, &owners).unwrap();

        // A count past the table refuses.
        let at = CLEAR_WORK_INTERNER_AT;
        let mut over = account;
        over[at..at + 2].copy_from_slice(&(MAX_EPOCH_ORDERS as u16 + 1).to_le_bytes());
        assert_eq!(read_owner_interner(&over), Err(CodecError::InvalidCount));

        // A zero owner below the count is no identity.
        let mut hollow = account;
        hollow[at + 2..at + 2 + HASH_BYTES].fill(0);
        assert_eq!(read_owner_interner(&hollow), Err(CodecError::ZeroIdentity));

        // A nonzero owner at or beyond the count is a leak, not padding.
        let mut leaked = account;
        leaked[at + 2 + HASH_BYTES] = 0xEE;
        assert_eq!(
            read_owner_interner(&leaked),
            Err(CodecError::NonCanonicalPadding)
        );

        // A region on a half-grown account has no framing to live in.
        let mut short = [0u8; CLEAR_WORK_GROW_STEP];
        assert_eq!(
            read_owner_interner(&short[..]),
            Err(CodecError::Truncated)
        );
        assert_eq!(
            write_owner_interner(&mut short[..], &owners),
            Err(CodecError::Truncated)
        );
    }

    #[test]
    fn the_rewind_is_admitted_exactly_at_a_bound_pass_boundary() {
        let mut account = work();
        advance_walk(&mut account, 1, 3, 7).unwrap();
        // An open checkpoint has no pass boundary to rewind at.
        assert_eq!(rewind_walk(&mut account), Err(CodecError::MismatchedBinding));

        bind_order_set(&mut account, h(9), 0xF01D).unwrap();
        let rewound = rewind_walk(&mut account).unwrap();
        assert_eq!(rewound.page_cursor, 0);
        assert_eq!(rewound.slot_cursor, 0);
        assert_eq!(rewound.live_rank, 0);
        assert_eq!(rewound.status, CLEAR_WORK_STATUS_BOUND);
        assert_eq!(rewound.order_set, h(9));
        assert_eq!(rewound.consumed_fold, 0xF01D);
        // The next pass walks forward again from the top.
        assert_eq!(advance_walk(&mut account, 0, 1, 1).unwrap().slot_cursor, 1);

        // A completed checkpoint rewinds nowhere.
        complete_clear_work(&mut account).unwrap();
        assert_eq!(rewind_walk(&mut account), Err(CodecError::MismatchedBinding));
    }

    /* ------------------------------------------------------------------ */
    /* General epoch lifecycle                                             */
    /* ------------------------------------------------------------------ */

    fn window() -> EpochWindowAccount {
        let market = h(0x51);
        EpochWindowAccount {
            epoch: canonical_epoch_id(market, 7),
            market,
            epoch_index: 7,
            freeze_deadline_slot: 900,
            stored_bump: 254,
            flags: 0,
        }
    }

    #[test]
    fn the_epoch_window_round_trips_and_refuses_hostile_frames() {
        let value = window();
        let mut bytes = [0; EPOCH_WINDOW_ACCOUNT_BYTES];
        assert_eq!(value.encode(&mut bytes).unwrap(), EPOCH_WINDOW_ACCOUNT_BYTES);
        assert_eq!(bytes[0], EPOCH_WINDOW_TAG);
        assert_eq!(bytes[1], EPOCH_WINDOW_VERSION);
        assert_eq!(EpochWindowAccount::decode(&bytes), Ok(value));

        assert_eq!(
            EpochWindowAccount::decode(&bytes[..EPOCH_WINDOW_ACCOUNT_BYTES - 1]),
            Err(CodecError::Truncated)
        );
        let mut wrong_tag = bytes;
        wrong_tag[0] = CLEAR_WORK_TAG;
        assert_eq!(
            EpochWindowAccount::decode(&wrong_tag),
            Err(CodecError::WrongTag)
        );
        let mut wrong_version = bytes;
        wrong_version[1] = EPOCH_WINDOW_VERSION + 1;
        assert_eq!(
            EpochWindowAccount::decode(&wrong_version),
            Err(CodecError::WrongVersion)
        );

        // A window whose epoch identity is not the canonical derivation of
        // its own (market, index) names an epoch that cannot exist.
        let mut wrong_epoch = value;
        wrong_epoch.epoch = h(0x52);
        assert_eq!(
            wrong_epoch.validate(),
            Err(CodecError::NonCanonicalIdentity)
        );
        let mut wrong_index = value;
        wrong_index.epoch_index = 8;
        assert_eq!(
            wrong_index.validate(),
            Err(CodecError::NonCanonicalIdentity)
        );
        // No deadline is not a window, and reserved flags stay zero.
        let mut no_deadline = value;
        no_deadline.freeze_deadline_slot = 0;
        assert_eq!(no_deadline.validate(), Err(CodecError::ZeroValue));
        let mut flagged = value;
        flagged.flags = 1;
        assert_eq!(flagged.validate(), Err(CodecError::InvalidEnum));
    }

    #[test]
    fn the_open_general_epoch_is_canonical_and_wide() {
        let market = h(0x53);
        let epoch = open_general_epoch(market, h(0x54), h(0x55), h(0x56), 7, 10_000, 16, 253)
            .unwrap();
        assert_eq!(epoch.epoch, canonical_epoch_id(market, 7));
        assert_eq!(epoch.book, canonical_general_book_id(epoch.epoch));
        assert_eq!(
            epoch.remainder_seed,
            canonical_general_remainder_seed(epoch.epoch)
        );
        assert_eq!(epoch.phase, EPOCH_PHASE_OPEN);
        assert_eq!(epoch.owner_count, MAX_EPOCH_ORDERS as u16);
        assert_eq!(epoch.page_count, 0);
        assert_eq!(epoch.order_count, 0);
        assert_eq!(epoch.order_set, Hash32::ZERO);
        assert_eq!(epoch.outcome_count, 16);
        // The general book identity never collides with the direct one.
        assert_ne!(
            canonical_general_book_id(epoch.epoch),
            crate::direct_selection::canonical_direct_book_id(epoch.epoch)
        );
        // The full 16-outcome width the direct plane's `== 2` gates refuse is
        // exactly what this constructor admits; outside the codec bound stays
        // refused.
        assert!(open_general_epoch(market, h(0x54), h(0x55), h(0x56), 7, 10_000, 17, 253).is_err());
        assert!(open_general_epoch(market, h(0x54), h(0x55), h(0x56), 7, 0, 4, 253).is_err());
    }
}
