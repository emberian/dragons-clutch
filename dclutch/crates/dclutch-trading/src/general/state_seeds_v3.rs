//! The one supported derivation for every General action-state PDA.
//!
//! # Why this module exists
//!
//! A General release names a `StateLifecyclePolicyV5` whose recipes tell the
//! generic Trading lifecycle adapter how to derive each action-state address.
//! Get one literal or one seed position wrong and the release still
//! AUTHENTICATES -- the `CapabilityProgramSetV2` table is well formed, every
//! descriptor joins its finalized artifact bundle, every digest agrees with
//! itself -- and then derives addresses that are not the ones the family
//! executes at. That failure is strictly worse than publishing nothing, because
//! it is invisible until an account that should exist is vacant on chain.
//!
//! The defect class is the one this tree keeps paying for: **two authors for
//! one fact**. Before this module the four seed literals were module-private
//! constants in `state_artifacts_v3.rs`, and
//! `general-accelerator-sbf/program-test/tests/hot_instruction_v3.rs` restated
//! all four, plus the seed ORDER, plus the action-to-phase mapping -- three
//! facts, six authors. Its own doc comment admitted the restatement and argued
//! it was self-checking. Eighteen lines further down, the same file imports
//! `RAW_RECORD_PDA_SEED_V1` from `dclutch-registry::record` rather than retyping
//! it, which is the pattern that ended the same defect for records.
//!
//! # How a wrong seed is made inexpressible rather than reviewed-against
//!
//! There is exactly ONE table per recipe, and both sides are PROJECTIONS of it:
//!
//! - the **policy** side ([`GeneralStateRecipeV3::lifecycle_seeds`]) hands the
//!   table straight to `encode_lifecycle_policy_v{4,5}_atomic`, and takes its
//!   `seed_count` and `bump_offset` from the table's own shape rather than from
//!   hand-written numbers that could disagree with it;
//! - the **address** side ([`GeneralStateAddressSeedsV3::as_slices`]) does not
//!   contain a seed order at all. It WALKS the same table and substitutes the
//!   caller's concrete bytes for each entry.
//!
//! So there is no second place to state an order, and a caller cannot supply
//! one. Adding, reordering, or respelling a seed is a single edit here that
//! moves the policy and every derived address together, by construction.
//!
//! This module deliberately does not derive an address. `find_program_address`
//! needs the owning program id and a curve check, which belong to the small SVM
//! adapter boundary -- the same line `dclutch-claims::structured::seeds`
//! draws.

use crate::general_codec::Action;
use dclutch_vm::account_profile::lifecycle_v3::{
    MAX_SEED_BYTES, MAX_SEEDS, encode::LifecycleSeedInputV3,
};

use crate::general::{
    MAX_PDA_SEED_BYTES,
    candidate_v1::{GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_VERIFIER_PDA_DOMAIN_V1},
    hot_candidate_v3::{identity, scalar},
};

// The crate publishes its own 32-byte seed bound and the lifecycle interpreter
// enforces one at runtime. Two constants for one physical limit is the defect
// this module exists to end, so they are joined here instead of both being
// trusted.
const _: () = assert!(
    MAX_PDA_SEED_BYTES == MAX_SEED_BYTES as usize,
    "the crate seed bound and the lifecycle interpreter's bound are one limit"
);

/// PDA seed domain shared by every General action state.
pub const GENERAL_STATE_SEED_DOMAIN_V3: &[u8] = b"dclutch-general-state-v3";
/// Selection-phase state discriminator.
pub const GENERAL_SELECTION_STATE_SEED_V3: &[u8] = b"selection";
/// Settlement-phase state discriminator.
pub const GENERAL_SETTLEMENT_STATE_SEED_V3: &[u8] = b"settlement";
/// Close-only terminal-record discriminator.
pub const GENERAL_TERMINAL_STATE_SEED_V3: &[u8] = b"terminal";
/// Batch-window state discriminator.
pub const GENERAL_BATCH_STATE_SEED_V3: &[u8] = b"batch";
/// Order-record state discriminator.
pub const GENERAL_ORDER_STATE_SEED_V3: &[u8] = b"order";
/// Raw terminal verified-candidate result domain.
///
/// The result is deliberately not a `GeneralLocalStateV3` envelope, so its PDA
/// needs a domain that names the immutable certificate itself rather than
/// borrowing the candidate or verifier domains.
pub const GENERAL_VERIFIED_CANDIDATE_PDA_DOMAIN_V1: &[u8] = b"dclutch-general-verified-v1";

// A seed longer than 32 bytes makes `find_program_address` refuse every bump,
// so the state it names would have no derivable address at all -- the module
// whose stated job is to be the sole authority on the seed order must not be
// able to publish an order no adapter could execute. An EMPTY literal is
// refused by the lifecycle encoder itself, so it would fail at emit time rather
// than at derive time; both bounds are checked here, at compile time, instead.
const _: () = assert!(
    !GENERAL_STATE_SEED_DOMAIN_V3.is_empty()
        && GENERAL_STATE_SEED_DOMAIN_V3.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed must be nonempty and at most 32 bytes to derive an address"
);
const _: () = assert!(
    !GENERAL_SELECTION_STATE_SEED_V3.is_empty()
        && GENERAL_SELECTION_STATE_SEED_V3.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed must be nonempty and at most 32 bytes to derive an address"
);
const _: () = assert!(
    !GENERAL_SETTLEMENT_STATE_SEED_V3.is_empty()
        && GENERAL_SETTLEMENT_STATE_SEED_V3.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed must be nonempty and at most 32 bytes to derive an address"
);
const _: () = assert!(
    !GENERAL_TERMINAL_STATE_SEED_V3.is_empty()
        && GENERAL_TERMINAL_STATE_SEED_V3.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed must be nonempty and at most 32 bytes to derive an address"
);
const _: () = assert!(
    !GENERAL_BATCH_STATE_SEED_V3.is_empty()
        && GENERAL_BATCH_STATE_SEED_V3.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed must be nonempty and at most 32 bytes to derive an address"
);
const _: () = assert!(
    !GENERAL_ORDER_STATE_SEED_V3.is_empty()
        && GENERAL_ORDER_STATE_SEED_V3.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed must be nonempty and at most 32 bytes to derive an address"
);
const _: () = assert!(
    !GENERAL_VERIFIED_CANDIDATE_PDA_DOMAIN_V1.is_empty()
        && GENERAL_VERIFIED_CANDIDATE_PDA_DOMAIN_V1.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed must be nonempty and at most 32 bytes to derive an address"
);

/// Exact width of a 32-byte identity seed coordinate.
pub const GENERAL_IDENTITY_SEED_BYTES_V3: usize = 32;
/// Exact encoded width of the terminal-coordinate scalar seed.
pub const GENERAL_TERMINAL_COORDINATE_SEED_BYTES_V3: u8 = 8;

/// Narrow a register index to the width one encoded seed entry carries.
///
/// The register bank declares indices as `u32` and the seed entry encodes a
/// `u16`. A truncating cast would name a DIFFERENT register and derive a
/// different address in perfect silence, so the bound is asserted rather than
/// assumed; every call below is a compile-time constant, so a violation is a
/// build failure and never a runtime one.
#[allow(clippy::cast_possible_truncation)]
const fn narrow_register(value: u32) -> u16 {
    assert!(
        value <= u16::MAX as u32,
        "a General seed register index must survive narrowing to the encoded width"
    );
    value as u16
}

/// Narrow a seed ordinal to the width the lifecycle recipe declares.
///
/// `seed_count` and `bump_offset` are single bytes on the wire. A recipe longer
/// than 255 seeds could not be declared to the encoder at all, so it is refused
/// at compile time rather than wrapping to a shorter, wrong seed program.
#[allow(clippy::cast_possible_truncation)]
const fn narrow_seed_ordinal(value: usize) -> u8 {
    assert!(
        value <= u8::MAX as usize,
        "a General recipe declares at most 255 seeds"
    );
    value as u8
}

/// Common identity register naming the General root.
pub const GENERAL_ROOT_IDENTITY_REGISTER_V3: u16 = narrow_register(identity::GENERAL_ROOT);
/// Common identity register naming the selected verified candidate.
pub const GENERAL_CANDIDATE_IDENTITY_REGISTER_V3: u16 = narrow_register(identity::CANDIDATE);
/// Common scalar register naming the settlement cursor's terminal coordinate.
pub const GENERAL_TERMINAL_COORDINATE_SCALAR_REGISTER_V3: u16 =
    narrow_register(scalar::CURSOR_TERMINAL_COORDINATE);
/// Common identity register naming one batch's content identity.
///
/// The batch pair's RequestProfile projects the request subject here, so the
/// address the lifecycle adapter derives is keyed by the identity the caller
/// named and authenticated content joins against -- exactly the replay shape
/// ADR-0009 SS4 chose: a second admission of the same content hits an occupied
/// address.
pub const GENERAL_BATCH_IDENTITY_REGISTER_V3: u16 = narrow_register(identity::SELECTION_BATCH);
/// Common identity register naming one order's content identity.
///
/// The order actions' RequestProfile projects the request subject here, so the
/// address the lifecycle adapter derives is keyed by the identity the maker
/// signed -- a second admission of the same signed order hits an occupied
/// address, and the tombstone a cancellation or release leaves keeps it
/// occupied forever.
pub const GENERAL_ORDER_IDENTITY_REGISTER_V3: u16 = narrow_register(identity::ORDER);

// Two identity coordinates that narrowed to the same register would make the
// address walk below resolve both to whichever arm it matched first, so the
// distinctness the seed order depends on is checked rather than eyeballed.
const _: () = assert!(
    GENERAL_ROOT_IDENTITY_REGISTER_V3 != GENERAL_CANDIDATE_IDENTITY_REGISTER_V3,
    "the General root and candidate must be distinct seed registers"
);
const _: () = assert!(
    GENERAL_BATCH_IDENTITY_REGISTER_V3 != GENERAL_ROOT_IDENTITY_REGISTER_V3
        && GENERAL_BATCH_IDENTITY_REGISTER_V3 != GENERAL_CANDIDATE_IDENTITY_REGISTER_V3,
    "the General batch must be a distinct seed register"
);
const _: () = assert!(
    GENERAL_ORDER_IDENTITY_REGISTER_V3 != GENERAL_ROOT_IDENTITY_REGISTER_V3
        && GENERAL_ORDER_IDENTITY_REGISTER_V3 != GENERAL_CANDIDATE_IDENTITY_REGISTER_V3
        && GENERAL_ORDER_IDENTITY_REGISTER_V3 != GENERAL_BATCH_IDENTITY_REGISTER_V3,
    "the General order must be a distinct seed register"
);

/// Greatest number of non-bump seeds any General recipe declares.
pub const GENERAL_MAX_STATE_SEEDS_V3: usize = 5;

/// Sole seed order for a selection-phase General state.
///
/// ONE PER BATCH, and it was one per General ROOT until 2026-09-04. The
/// candidate is still absent from the key -- the cursor exists before any
/// candidate is chosen, so naming one here would name a fact that does not yet
/// hold -- but the BATCH is not: a selection is opened by considering a
/// candidate that already names its batch, and it is frozen around one.
///
/// The old key made General one call auction per Market. Nothing in the fifteen
/// writes a selection back to `Open` (`runtime_selection.rs:343-349` writes it
/// only at creation), so after the first `Freeze` a root's selection was Frozen
/// for the life of the capability, and `consider_verified_candidate_v2` refused
/// a second batch's candidate by `batch_id` (`:363`). `db9c6c75c` measured the
/// asymmetry exactly: a market can open, fill and close a second batch -- the
/// batch recipe is keyed by (root, batch id) and the root carries a monotonic
/// sequence -- and could SELECT in only the first.
///
/// Keyed by the batch identity register, this is the same construction as the
/// settlement recipe below, and it is what makes "one clearing per batch" a
/// property of an address rather than of a conjunct.
pub const GENERAL_SELECTION_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 5] = [
    LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_BATCH_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::Literal(GENERAL_SELECTION_STATE_SEED_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Sole seed order for a settlement-phase General state.
///
/// Keyed by the candidate as well as the root, so two candidates under one root
/// can never settle into the same account.
pub const GENERAL_SETTLEMENT_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 5] = [
    LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_CANDIDATE_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::Literal(GENERAL_SETTLEMENT_STATE_SEED_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Sole seed order for the close-only terminal General record.
///
/// The terminal coordinate is carried as the low eight little-endian bytes of a
/// common scalar, so one settlement can close into exactly one terminal record
/// per coordinate rather than a single record per candidate.
pub const GENERAL_TERMINAL_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 6] = [
    LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_CANDIDATE_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonScalar {
        index: GENERAL_TERMINAL_COORDINATE_SCALAR_REGISTER_V3,
        width: GENERAL_TERMINAL_COORDINATE_SEED_BYTES_V3,
    },
    LifecycleSeedInputV3::Literal(GENERAL_TERMINAL_STATE_SEED_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Sole seed order for one batch-window General state.
///
/// Keyed by the batch's own content identity under the root, so one signed
/// opening derives one address and a replayed open finds it occupied.
pub const GENERAL_BATCH_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 5] = [
    LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_BATCH_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::Literal(GENERAL_BATCH_STATE_SEED_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Sole seed order for one order-record General state.
///
/// Keyed by the order's own content identity under the root -- the masked
/// split digest of exactly the bytes the maker signed -- so one signed order
/// derives one address, exactly the replay shape ADR-0009 SS4 chose.
pub const GENERAL_ORDER_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 5] = [
    LifecycleSeedInputV3::Literal(GENERAL_STATE_SEED_DOMAIN_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ORDER_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::Literal(GENERAL_ORDER_STATE_SEED_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Sole seed order for one candidate submission record.
///
/// The existing candidate domain is the protocol's semantic owner; using it
/// here makes lifecycle admission and every candidate consumer derive the same
/// address without introducing a second General-local spelling.
pub const GENERAL_CANDIDATE_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 4] = [
    LifecycleSeedInputV3::Literal(GENERAL_CANDIDATE_PDA_DOMAIN_V1),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_CANDIDATE_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Sole seed order for one streamed candidate-verifier cursor.
///
/// The domain is owned by `candidate_v1`; consuming it here joins the lifecycle
/// policy to the exact address that the verifier runtime names.
pub const GENERAL_VERIFIER_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 4] = [
    LifecycleSeedInputV3::Literal(GENERAL_VERIFIER_PDA_DOMAIN_V1),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_CANDIDATE_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Sole seed order for the raw terminal `VerifiedCandidateV2` certificate.
pub const GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3: [LifecycleSeedInputV3<'static>; 4] = [
    LifecycleSeedInputV3::Literal(GENERAL_VERIFIED_CANDIDATE_PDA_DOMAIN_V1),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CommonIdentity(GENERAL_CANDIDATE_IDENTITY_REGISTER_V3),
    LifecycleSeedInputV3::CanonicalBump,
];

/// Locate the sole canonical bump in one recipe, refusing every other shape.
///
/// The lifecycle adapter derives the bump itself and appends it last. A recipe
/// with no bump, two bumps, or a bump anywhere but last describes a derivation
/// the adapter cannot perform, so it is refused at COMPILE time by the
/// assertions below rather than discovered against a vacant account on chain.
const fn canonical_bump_offset(seeds: &[LifecycleSeedInputV3<'static>]) -> usize {
    assert!(
        !seeds.is_empty(),
        "a General recipe declares at least one seed"
    );
    let mut index = 0;
    let mut found = usize::MAX;
    while index < seeds.len() {
        if matches!(seeds[index], LifecycleSeedInputV3::CanonicalBump) {
            assert!(
                found == usize::MAX,
                "a General recipe declares exactly one canonical bump"
            );
            found = index;
        }
        index += 1;
    }
    assert!(
        found == seeds.len() - 1,
        "the canonical bump is the last seed of a General recipe"
    );
    found
}

const _: () = {
    canonical_bump_offset(&GENERAL_SELECTION_STATE_RECIPE_V3);
    canonical_bump_offset(&GENERAL_SETTLEMENT_STATE_RECIPE_V3);
    canonical_bump_offset(&GENERAL_TERMINAL_STATE_RECIPE_V3);
    canonical_bump_offset(&GENERAL_BATCH_STATE_RECIPE_V3);
    canonical_bump_offset(&GENERAL_ORDER_STATE_RECIPE_V3);
    canonical_bump_offset(&GENERAL_CANDIDATE_STATE_RECIPE_V3);
    canonical_bump_offset(&GENERAL_VERIFIER_STATE_RECIPE_V3);
    canonical_bump_offset(&GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3);
};

// Every recipe's non-bump seed count must fit the buffer the address projection
// walks into, or `as_slices` could not express the order the policy declares.
const _: () = assert!(
    GENERAL_SELECTION_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3
        && GENERAL_SETTLEMENT_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3
        && GENERAL_TERMINAL_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3
        && GENERAL_BATCH_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3
        && GENERAL_ORDER_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3
        && GENERAL_CANDIDATE_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3
        && GENERAL_VERIFIER_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3
        && GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3.len() - 1 <= GENERAL_MAX_STATE_SEEDS_V3,
    "a General recipe declares more seeds than the address projection can hold"
);

// Solana bounds a PDA at 16 seeds, canonical bump included. A recipe over that
// names a state no adapter could derive, exactly like an over-long seed does.
const _: () = assert!(
    GENERAL_SELECTION_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize
        && GENERAL_SETTLEMENT_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize
        && GENERAL_TERMINAL_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize
        && GENERAL_BATCH_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize
        && GENERAL_ORDER_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize
        && GENERAL_CANDIDATE_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize
        && GENERAL_VERIFIER_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize
        && GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3.len() <= MAX_SEEDS as usize,
    "a General recipe declares more seeds than a program-derived address admits"
);

/// Total seeds in the two-recipe table Close declares.
pub const GENERAL_CLOSE_SEED_COUNT_V3: usize =
    GENERAL_SETTLEMENT_STATE_RECIPE_V3.len() + GENERAL_TERMINAL_STATE_RECIPE_V3.len();

/// Index in the Close seed table where the terminal recipe's window begins.
pub const GENERAL_CLOSE_TERMINAL_SEED_START_V3: u16 =
    narrow_seed_start(GENERAL_SETTLEMENT_STATE_RECIPE_V3.len());

/// Narrow a seed-table offset to the width one lifecycle recipe declares.
#[allow(clippy::cast_possible_truncation)]
const fn narrow_seed_start(value: usize) -> u16 {
    assert!(
        value <= u16::MAX as usize,
        "a General seed table starts within the encoded offset width"
    );
    value as u16
}

/// The Close seed table: the settlement order, then the terminal order.
///
/// Close closes one settlement state and creates one terminal record, so its
/// policy carries two recipes over one shared seed table. The table is BUILT
/// from the same two recipes every other action uses rather than written out a
/// second time -- an eleven-entry literal here would be precisely the second
/// author this module exists to remove, and the longest one in the family.
const fn close_seed_table() -> [LifecycleSeedInputV3<'static>; GENERAL_CLOSE_SEED_COUNT_V3] {
    let mut table = [LifecycleSeedInputV3::CanonicalBump; GENERAL_CLOSE_SEED_COUNT_V3];
    let mut index = 0;
    while index < GENERAL_SETTLEMENT_STATE_RECIPE_V3.len() {
        table[index] = GENERAL_SETTLEMENT_STATE_RECIPE_V3[index];
        index += 1;
    }
    let mut offset = 0;
    while offset < GENERAL_TERMINAL_STATE_RECIPE_V3.len() {
        table[GENERAL_SETTLEMENT_STATE_RECIPE_V3.len() + offset] =
            GENERAL_TERMINAL_STATE_RECIPE_V3[offset];
        offset += 1;
    }
    table
}

/// Sole seed table for the Close action, settlement window then terminal window.
pub const GENERAL_CLOSE_STATE_SEED_TABLE_V3: [LifecycleSeedInputV3<'static>;
    GENERAL_CLOSE_SEED_COUNT_V3] = close_seed_table();

/// Exact combined seed count for the CancelOrder policy.
pub const GENERAL_CANCEL_SEED_COUNT_V3: usize =
    GENERAL_BATCH_STATE_RECIPE_V3.len() + GENERAL_ORDER_STATE_RECIPE_V3.len();

/// Seed-table offset where the CancelOrder policy's order window begins.
pub const GENERAL_CANCEL_ORDER_SEED_START_V3: u16 =
    narrow_seed_start(GENERAL_BATCH_STATE_RECIPE_V3.len());

const fn cancel_seed_table() -> [LifecycleSeedInputV3<'static>; GENERAL_CANCEL_SEED_COUNT_V3] {
    let mut table = [LifecycleSeedInputV3::CanonicalBump; GENERAL_CANCEL_SEED_COUNT_V3];
    let mut index = 0;
    while index < GENERAL_BATCH_STATE_RECIPE_V3.len() {
        table[index] = GENERAL_BATCH_STATE_RECIPE_V3[index];
        index += 1;
    }
    let mut offset = 0;
    while offset < GENERAL_ORDER_STATE_RECIPE_V3.len() {
        table[GENERAL_BATCH_STATE_RECIPE_V3.len() + offset] = GENERAL_ORDER_STATE_RECIPE_V3[offset];
        offset += 1;
    }
    table
}

/// Sole seed table for the CancelOrder action, batch window then order window.
pub const GENERAL_CANCEL_STATE_SEED_TABLE_V3: [LifecycleSeedInputV3<'static>;
    GENERAL_CANCEL_SEED_COUNT_V3] = cancel_seed_table();

/// Exact combined seed count for Candidate, Verifier, and terminal Result.
pub const GENERAL_VERIFY_SEED_COUNT_V3: usize = GENERAL_CANDIDATE_STATE_RECIPE_V3.len()
    + GENERAL_VERIFIER_STATE_RECIPE_V3.len()
    + GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3.len();

/// Seed-table offset where the Verify verifier-cursor recipe begins.
pub const GENERAL_VERIFY_VERIFIER_SEED_START_V3: u16 =
    narrow_seed_start(GENERAL_CANDIDATE_STATE_RECIPE_V3.len());

/// Seed-table offset where the Verify raw result recipe begins.
pub const GENERAL_VERIFY_RESULT_SEED_START_V3: u16 = narrow_seed_start(
    GENERAL_CANDIDATE_STATE_RECIPE_V3.len() + GENERAL_VERIFIER_STATE_RECIPE_V3.len(),
);

const fn verify_seed_table() -> [LifecycleSeedInputV3<'static>; GENERAL_VERIFY_SEED_COUNT_V3] {
    let mut table = [LifecycleSeedInputV3::CanonicalBump; GENERAL_VERIFY_SEED_COUNT_V3];
    let mut index = 0;
    while index < GENERAL_CANDIDATE_STATE_RECIPE_V3.len() {
        table[index] = GENERAL_CANDIDATE_STATE_RECIPE_V3[index];
        index += 1;
    }
    let mut offset = 0;
    while offset < GENERAL_VERIFIER_STATE_RECIPE_V3.len() {
        table[GENERAL_CANDIDATE_STATE_RECIPE_V3.len() + offset] =
            GENERAL_VERIFIER_STATE_RECIPE_V3[offset];
        offset += 1;
    }
    offset = 0;
    while offset < GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3.len() {
        table[GENERAL_CANDIDATE_STATE_RECIPE_V3.len()
            + GENERAL_VERIFIER_STATE_RECIPE_V3.len()
            + offset] = GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3[offset];
        offset += 1;
    }
    table
}

/// Sole Verify seed table: Candidate, Verifier, then raw terminal Result.
pub const GENERAL_VERIFY_STATE_SEED_TABLE_V3: [LifecycleSeedInputV3<'static>;
    GENERAL_VERIFY_SEED_COUNT_V3] = verify_seed_table();

/// One of the eight General state derivations, and there are only eight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralStateRecipeV3 {
    /// Selection cursor, one per General root.
    Selection,
    /// Settlement state, one per (root, candidate).
    Settlement,
    /// Close-only terminal record, one per (root, candidate, coordinate).
    Terminal,
    /// Batch-window state, one per (root, batch identity).
    Batch,
    /// Order-record state, one per (root, order identity).
    Order,
    /// Candidate submission, one per (root, candidate identity).
    Candidate,
    /// Streamed verifier cursor, one per (root, candidate identity).
    Verifier,
    /// Raw immutable verified-candidate result, one per (root, candidate identity).
    VerifiedCandidate,
}

impl GeneralStateRecipeV3 {
    /// Borrow the sole seed order this recipe publishes, canonical bump included.
    #[must_use]
    pub const fn lifecycle_seeds(self) -> &'static [LifecycleSeedInputV3<'static>] {
        match self {
            Self::Selection => &GENERAL_SELECTION_STATE_RECIPE_V3,
            Self::Settlement => &GENERAL_SETTLEMENT_STATE_RECIPE_V3,
            Self::Terminal => &GENERAL_TERMINAL_STATE_RECIPE_V3,
            Self::Batch => &GENERAL_BATCH_STATE_RECIPE_V3,
            Self::Order => &GENERAL_ORDER_STATE_RECIPE_V3,
            Self::Candidate => &GENERAL_CANDIDATE_STATE_RECIPE_V3,
            Self::Verifier => &GENERAL_VERIFIER_STATE_RECIPE_V3,
            Self::VerifiedCandidate => &GENERAL_VERIFIED_CANDIDATE_STATE_RECIPE_V3,
        }
    }

    /// Total seeds, canonical bump included, as the lifecycle recipe declares it.
    ///
    /// Read off the table rather than written down beside it, so a policy cannot
    /// declare a count the seed order does not have.
    #[must_use]
    pub const fn seed_count(self) -> u8 {
        narrow_seed_ordinal(self.lifecycle_seeds().len())
    }

    /// Position of the canonical bump, likewise read off the table.
    #[must_use]
    pub const fn bump_offset(self) -> u8 {
        narrow_seed_ordinal(canonical_bump_offset(self.lifecycle_seeds()))
    }

    /// Seeds a caller must supply concretely; the adapter appends the bump.
    #[must_use]
    pub const fn supplied_seed_count(self) -> usize {
        self.lifecycle_seeds().len() - 1
    }

    /// The recipe the action's PRIMARY state uses.
    ///
    /// Consider and Freeze run before a candidate is frozen, so their state is
    /// the selection cursor; every later action settles. `Close` also creates a
    /// terminal record, which is [`Self::Terminal`] and is not returned here.
    #[must_use]
    pub const fn primary_for_action(action: Action) -> Self {
        match action {
            Action::SubmitCandidate | Action::VerifyCandidateRow | Action::CloseCandidate => {
                Self::Candidate
            }
            Action::Consider | Action::Freeze => Self::Selection,
            // The batch four share the Batch envelope: `PlaceOrder` and
            // `CancelOrder` authenticate the window as their primary state and
            // touch their order as a secondary one.
            Action::OpenBatch | Action::PlaceOrder | Action::CancelOrder | Action::CloseBatch => {
                Self::Batch
            }
            // `ReleaseOrder` is batch-free: its window gate is a constant of
            // the record the maker signed, so the order IS the primary state.
            Action::ReleaseOrder => Self::Order,
            _ => Self::Settlement,
        }
    }
}

/// Stable refusal from a General state-seed derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralStateSeedErrorV3 {
    /// A seed coordinate was the zero identity.
    ZeroIdentity,
    /// Two independent identity coordinates were the same value.
    AccountAlias,
    /// The recipe names a coordinate this seed set does not carry.
    MissingCoordinate,
    /// The recipe names a seed source no General derivation supports.
    UnsupportedSeedInput,
    /// A derived address disagreed with the observed or persisted address.
    AddressMismatch,
}

/// Result alias for General state-seed derivations.
pub type Result<T> = core::result::Result<T, GeneralStateSeedErrorV3>;

const fn is_zero(value: &[u8; 32]) -> bool {
    let mut index = 0;
    while index < 32 {
        if value[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}

fn require_nonzero(value: [u8; 32]) -> Result<[u8; 32]> {
    if is_zero(&value) {
        return Err(GeneralStateSeedErrorV3::ZeroIdentity);
    }
    Ok(value)
}

/// Borrowed seed order ready for `find_program_address`, bump excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralStateSeedSlicesV3<'a> {
    slices: [&'a [u8]; GENERAL_MAX_STATE_SEEDS_V3],
    count: usize,
}

impl<'a> GeneralStateSeedSlicesV3<'a> {
    /// Borrow exactly the seeds the recipe declares, in the recipe's own order.
    #[must_use]
    pub fn as_slice(&self) -> &[&'a [u8]] {
        &self.slices[..self.count]
    }
}

/// Concrete coordinates for one General state address.
///
/// Carries no seed order of its own. [`Self::as_slices`] obtains the order by
/// walking [`GeneralStateRecipeV3::lifecycle_seeds`], which is the same table
/// the published policy is encoded from -- so an address derived through this
/// type and an address derived by the on-chain adapter from the policy cannot
/// disagree about the order, only about the bytes, and the bytes are the
/// caller's own authenticated registers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralStateAddressSeedsV3 {
    recipe: GeneralStateRecipeV3,
    general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    candidate: Option<[u8; GENERAL_IDENTITY_SEED_BYTES_V3]>,
    terminal_coordinate: Option<[u8; GENERAL_TERMINAL_COORDINATE_SEED_BYTES_V3 as usize]>,
}

impl GeneralStateAddressSeedsV3 {
    /// Coordinates for the selection cursor of one (root, batch identity).
    ///
    /// It took the root alone until 2026-09-04, which made General one call
    /// auction per Market. See `GENERAL_SELECTION_STATE_RECIPE_V3`. The batch
    /// rides the `candidate` field because that field IS the recipe's second
    /// identity slot, whatever the recipe calls it -- `batch` uses it for the
    /// batch identity already.
    pub fn selection(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        batch: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let batch = require_nonzero(batch)?;
        if general_root == batch {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::Selection,
            general_root,
            candidate: Some(batch),
            terminal_coordinate: None,
        })
    }

    /// Coordinates for the batch-window state of one (root, batch identity).
    pub fn batch(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        batch: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let batch = require_nonzero(batch)?;
        if general_root == batch {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::Batch,
            general_root,
            candidate: Some(batch),
            terminal_coordinate: None,
        })
    }

    /// Coordinates for the order-record state of one (root, order identity).
    pub fn order(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        order: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let order = require_nonzero(order)?;
        if general_root == order {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::Order,
            general_root,
            candidate: Some(order),
            terminal_coordinate: None,
        })
    }

    /// Coordinates for the candidate submission state of one (root, candidate).
    pub fn candidate(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        candidate: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let candidate = require_nonzero(candidate)?;
        if general_root == candidate {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::Candidate,
            general_root,
            candidate: Some(candidate),
            terminal_coordinate: None,
        })
    }

    /// Coordinates for the streamed verifier cursor of one (root, candidate).
    pub fn verifier(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        candidate: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let candidate = require_nonzero(candidate)?;
        if general_root == candidate {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::Verifier,
            general_root,
            candidate: Some(candidate),
            terminal_coordinate: None,
        })
    }

    /// Coordinates for the raw terminal certificate of one (root, candidate).
    pub fn verified_candidate(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        candidate: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let candidate = require_nonzero(candidate)?;
        if general_root == candidate {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::VerifiedCandidate,
            general_root,
            candidate: Some(candidate),
            terminal_coordinate: None,
        })
    }

    /// Coordinates for the settlement state of one (root, candidate).
    pub fn settlement(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        candidate: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let candidate = require_nonzero(candidate)?;
        if general_root == candidate {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::Settlement,
            general_root,
            candidate: Some(candidate),
            terminal_coordinate: None,
        })
    }

    /// Coordinates for the close-only terminal record at one cursor coordinate.
    pub fn terminal(
        general_root: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        candidate: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        terminal_coordinate: u64,
    ) -> Result<Self> {
        let general_root = require_nonzero(general_root)?;
        let candidate = require_nonzero(candidate)?;
        if general_root == candidate {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(Self {
            recipe: GeneralStateRecipeV3::Terminal,
            general_root,
            candidate: Some(candidate),
            terminal_coordinate: Some(terminal_coordinate.to_le_bytes()),
        })
    }

    /// The recipe these coordinates were built for.
    #[must_use]
    pub const fn recipe(self) -> GeneralStateRecipeV3 {
        self.recipe
    }

    /// Project the recipe's seed order onto these coordinates.
    ///
    /// This walk is the whole point of the module: it is the only place a
    /// General state address is spelled out, and it reads the order from the
    /// same table the policy is emitted from instead of restating it. A seed
    /// source the walk does not recognise refuses rather than being skipped,
    /// because a skipped seed derives a different address than the policy does.
    pub fn as_slices(&self) -> Result<GeneralStateSeedSlicesV3<'_>> {
        let mut slices: [&[u8]; GENERAL_MAX_STATE_SEEDS_V3] = [&[]; GENERAL_MAX_STATE_SEEDS_V3];
        let mut count = 0_usize;
        for seed in self.recipe.lifecycle_seeds() {
            let bytes: &[u8] = match *seed {
                LifecycleSeedInputV3::Literal(literal) => literal,
                LifecycleSeedInputV3::CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3) => {
                    &self.general_root
                }
                LifecycleSeedInputV3::CommonIdentity(GENERAL_CANDIDATE_IDENTITY_REGISTER_V3)
                | LifecycleSeedInputV3::CommonIdentity(GENERAL_BATCH_IDENTITY_REGISTER_V3)
                | LifecycleSeedInputV3::CommonIdentity(GENERAL_ORDER_IDENTITY_REGISTER_V3) => self
                    .candidate
                    .as_ref()
                    .ok_or(GeneralStateSeedErrorV3::MissingCoordinate)?,
                LifecycleSeedInputV3::CommonScalar {
                    index: GENERAL_TERMINAL_COORDINATE_SCALAR_REGISTER_V3,
                    width: GENERAL_TERMINAL_COORDINATE_SEED_BYTES_V3,
                } => self
                    .terminal_coordinate
                    .as_ref()
                    .ok_or(GeneralStateSeedErrorV3::MissingCoordinate)?,
                // The bump is the adapter's, and it is last, so the walk ends
                // here rather than continuing over seeds the caller cannot fill.
                LifecycleSeedInputV3::CanonicalBump => break,
                _ => return Err(GeneralStateSeedErrorV3::UnsupportedSeedInput),
            };
            *slices
                .get_mut(count)
                .ok_or(GeneralStateSeedErrorV3::UnsupportedSeedInput)? = bytes;
            count = count
                .checked_add(1)
                .ok_or(GeneralStateSeedErrorV3::UnsupportedSeedInput)?;
        }
        if count != self.recipe.supplied_seed_count() {
            return Err(GeneralStateSeedErrorV3::MissingCoordinate);
        }
        Ok(GeneralStateSeedSlicesV3 { slices, count })
    }

    /// Join an adapter-derived PDA to the address actually observed or persisted.
    ///
    /// The derivation itself belongs to the SVM adapter; this closes the loop so
    /// an adapter that derived with the wrong seeds fails its own join.
    pub fn authenticate_address(
        self,
        derived_address: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
        observed_address: [u8; GENERAL_IDENTITY_SEED_BYTES_V3],
    ) -> Result<[u8; GENERAL_IDENTITY_SEED_BYTES_V3]> {
        let derived = require_nonzero(derived_address)?;
        let observed = require_nonzero(observed_address)?;
        if derived != observed {
            return Err(GeneralStateSeedErrorV3::AddressMismatch);
        }
        if derived == self.general_root || Some(derived) == self.candidate {
            return Err(GeneralStateSeedErrorV3::AccountAlias);
        }
        Ok(derived)
    }
}

#[cfg(test)]
mod tests;
