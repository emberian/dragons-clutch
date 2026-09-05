#![no_std]
#![deny(missing_docs)]

//! Protocol-wide allocation of Solana `ProgramError::Custom` codes.
//!
//! # Why this exists
//!
//! A Solana `custom program error` is a bare `u32`. The runtime reports it
//! without saying which program produced it, and a CPI chain can carry several
//! programs' refusals through one transaction. Before this registry every
//! first-party dClutch program numbered its refusals from zero, so
//! `Custom(0)`, `Custom(1)` and `Custom(2)` were each claimed by sixteen
//! different programs at once. Anything reading a code back — a test, the
//! gauntlet census, a human reading `custom program error: 0x1` in a validator
//! log — was reading an ambiguous number and resolving it by assuming which
//! program it came from. That assumption is a mirror: it agrees with the
//! harness rather than with the chain, and it agrees loudest exactly when a
//! CPI child refused for an unrelated reason that happened to share a number.
//!
//! This module removes the ambiguity at the source. Every program owns a
//! disjoint band of the `u32` space, so a code identifies its program.
//!
//! # The scheme
//!
//! ```text
//! band = code >> 12          (each band is 0x1000 = 4096 codes wide)
//! ```
//!
//! - **Band 0 (`0x0000..=0x0FFF`) is never allocated.** A custom code below
//!   `0x1000` is, by construction, not a first-party dClutch refusal. That is
//!   the single most useful property here: SPL Token, the Loader, and every
//!   other foreign program number from zero, and now so does nothing of ours.
//! - **Bands `0x001..=0x0FF`** belong to on-chain protocol programs.
//! - **Bands `0x100` and up** belong to test-only caller programs, which exist
//!   to drive hostile CPI cases in `program-test` and are never deployed to a
//!   real cluster. They are registered anyway: the whole point is that a
//!   deliberate late failure inside a test caller can never be mistaken for a
//!   protocol refusal.
//!
//! Read a band off a log line by dropping the last three hex digits:
//! `custom program error: 0x5100` is band 5 (Claims), offset `0x100`.
//!
//! # Sub-bands inside a program
//!
//! Claims established the convention this registry generalises: a program with
//! several independently versioned request families gives each family its own
//! round hexadecimal offset inside its band, rather than interleaving them.
//! Claims' historical decimal offsets (100, 140, 160, 180, 200, 210, 260, 500)
//! survive verbatim as *hexadecimal* offsets inside band 5, so the family
//! structure reads straight off the code: `0x5180` is Claims / founding-V5 /
//! `Instruction`. Sub-bands are a program's own business and are documented in
//! `docs/decisions/0007-namespaced-refusal-codes.md`, not enforced here.
//!
//! # How a program binds itself to its band
//!
//! Discriminants stay written as plain hexadecimal literals — they are what a
//! reader greps for after seeing a code in a log, and an arithmetic expression
//! would hide that. The binding to this registry is a compile-time assertion
//! next to the enum:
//!
//! ```ignore
//! const _: () = assert!(
//!     CoreSbfError::Instruction as u32 == dclutch_refusal_registry::CORE_REFUSAL_BASE,
//!     "Core's refusal enum must start at its registered band base"
//! );
//! ```
//!
//! The gauntlet census closes the remaining gap: `inventory --check-unique`
//! fails on any refusal code that is duplicated, or that falls outside the band
//! registered to the program that declares it.
//!
//! # Changing an allocation
//!
//! Bands are append-only. A new program takes the next free base; a deleted
//! program's band is retired, never reused. Renumbering an existing band is a
//! wire-compatibility break and needs its own decision record. (Bands were
//! allocated at a point where no wire carried a compatibility entitlement,
//! which is precisely why they were allocated then.)

/// Whether a band is deployed to a real cluster or exists only under
/// `program-test`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BandTier {
    /// An on-chain protocol program.
    Program,
    /// A test-only caller program, never deployed to a real cluster.
    TestCaller,
}

/// One package's exclusive allocation of custom program error codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefusalBand {
    /// The gauntlet census label for the owning program, or a `test/`-prefixed
    /// label for a test-only caller. Matches `TARGETS` in the census.
    pub label: &'static str,
    /// The Cargo package that owns the band.
    pub package: &'static str,
    /// Lowest code in the band.
    pub base: u32,
    /// Number of codes in the band.
    pub span: u32,
    /// Whether the band's codes can ever appear on a real cluster.
    pub tier: BandTier,
}

impl RefusalBand {
    /// Highest code in the band.
    #[must_use]
    pub const fn last(&self) -> u32 {
        self.base + self.span - 1
    }

    /// Whether `code` falls inside this band.
    #[must_use]
    pub const fn contains(&self, code: u32) -> bool {
        code >= self.base && code <= self.last()
    }
}

// -------------------------------------------------- the generated allocation
//
// `DClutchSemantics.RefusalBandsV1` is the authority for the scheme and for
// every allocation in it. A band there is an INDEX, not a base -- the base is
// `index * span` -- so a base that is not band-aligned cannot be written down,
// and `code >> BAND_SHIFT` recovering the band is a theorem rather than a
// convention. The compile-time proofs below re-establish the same invariants
// over the generated bytes, which is a second CHECK of one authored table
// rather than a second author of it.
//
// The gaps are load-bearing and Lean pins them in
// `retired_and_unallocated_indices_stay_absent`. Band 7 is RETIRED: it
// belonged to `dclutch-dealer-sbf`, deleted 2026-09-02, a standalone
// measurement prototype its own header disclaimed as "not a second accepted
// Trading release identity", marked `false` in the release tool's
// SHIPPED_LINKS, and with no consumer but its own program-test. Bands 9 and
// 10 are RETIRED the same way: `dclutch-product-runtime-v2-sbf` (an admission
// receipt no program read) and `dclutch-direct-aot-sbf` (an accelerator for
// the superseded Direct V2 descriptor), both `false` in SHIPPED_LINKS, in no
// cohort, deleted 2026-09-04. Bands 11 and 13 (`dclutch-series-shadow-sbf`,
// `dclutch-dealer-accelerator-sbf`) were folded into `dclutch-accelerator-sbf`
// on band 12 the same day; their refusals live on as sub-bands 0xC200 and
// 0xC100 of it, so the two indices are retired, not moved. Bands 14, 15
// and 16 were drafted for `dclutch-controller-proof-sbf`,
// `dclutch-custody-proof-sbf` and `dclutch-claims-proof-sbf` and are NOT
// allocated: `11ca28b` banished all three DCLTCAT1 proof programs while this
// registry was being written. They are absent rather than retired-in-place
// because no wire ever carried them -- a band entry for a program that does
// not exist reads exactly like a live one. Ascent alone does not say any of
// this, because ascent permits filling a gap later. See docs/decisions/0007.

#[path = "generated_bands.rs"]
mod generated_bands;

pub use generated_bands::{
    ACCELERATOR_REFUSAL_BASE, BAND_COUNT, BAND_SHIFT, BAND_SPAN, BANDS, CLAIMS_REFUSAL_BASE,
    CORE_REFUSAL_BASE, CUSTODY_REFUSAL_BASE, FIRST_PROGRAM_BAND, FIRST_TEST_BAND,
    PROGRAM_BAND_COUNT, REGISTRY_REFUSAL_BASE, RENT_REFUSAL_BASE, RESOLUTION_REFUSAL_BASE,
    TEST_CALLER_BAND_COUNT, TEST_CLAIMS_AFFINE_BATCH_CALLER_BASE,
    TEST_CLAIMS_CLAIM_CHECK_ESCROW_SIGNER_BASE, TEST_CLAIMS_FRACTIONAL_ATOMIC_CALLER_BASE,
    TEST_CLAIMS_FRACTIONAL_COMPACTION_CALLER_BASE, TEST_CLAIMS_FRACTIONAL_SIGNED_DELTA_CALLER_BASE,
    TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE, TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE,
    TEST_CLAIMS_RATIONAL_V2_CALLER_BASE, TEST_CLAIMS_SPARSE_CHAIN_CALLER_BASE,
    TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE, TEST_CUSTODY_CALLER_BASE,
    TEST_DEALER_ACCELERATOR_CALLER_BASE, TEST_GENERAL_ACCELERATOR_CALLER_BASE,
    TEST_RESOLUTION_RECEIPT_CALLER_BASE, TRADING_REFUSAL_BASE,
};

// -------------------------------------------------------------- deliberate aliases
//
// One enum may deliberately reuse another's codes when it is a published
// boundary of the owning program rather than a program of its own.

/// Enums that deliberately raise another package's codes, with the band label
/// whose codes they raise.
///
/// A published boundary crate is not a program: its refusals must be
/// *indistinguishable* from those of the program that publishes it, so its
/// codes are aliases rather than a band of their own. Any uniqueness check has
/// to know that on purpose, or the alias reads as the exact collision this
/// registry forbids.
pub const ALIASES: &[(&str, &str)] = &[
    // Trading's published read-only Shadow accelerator callback boundary. The
    // binding assertions live at
    // `programs/dclutch-trading-sbf/src/execution_strategy_v2.rs`.
    ("ShadowAcceleratorAuthErrorV4", "trading"),
];

// ------------------------------------------------------------------ lookups

/// The band index a code belongs to. Band 0 is never allocated.
#[must_use]
pub const fn band_index(code: u32) -> u32 {
    code >> BAND_SHIFT
}

/// The band that owns `code`, or `None` when no allocation covers it.
///
/// `None` for any code below [`BAND_SPAN`] is the load-bearing case: it is how
/// a reader learns that a refusal came from outside the protocol.
#[must_use]
pub fn owner(code: u32) -> Option<&'static RefusalBand> {
    BANDS.iter().find(|band| band.contains(code))
}

/// The band allocated to a census label, or `None` when the label is unknown.
#[must_use]
pub fn band_for_label(label: &str) -> Option<&'static RefusalBand> {
    BANDS.iter().find(|band| band.label == label)
}

/// The band allocated to a Cargo package, or `None` when it owns no band.
#[must_use]
pub fn band_for_package(package: &str) -> Option<&'static RefusalBand> {
    BANDS.iter().find(|band| band.package == package)
}

// ------------------------------------------ pinning an enum to its band

/// The base of the band that contains `code`: `code` with its offset cleared.
#[must_use]
pub const fn band_base_of(code: u32) -> u32 {
    (code >> BAND_SHIFT) << BAND_SHIFT
}

/// Count identifiers at compile time. Support for [`pin_refusal_band!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __count_idents {
    () => { 0_usize };
    ($head:ident $($tail:ident)*) => { 1_usize + $crate::__count_idents!($($tail)*) };
}

/// Pin a `#[repr(u32)]` refusal enum to its registered band, or to a sub-band
/// inside it.
///
/// ```ignore
/// dclutch_refusal_registry::pin_refusal_band!(
///     CoreSbfError,
///     dclutch_refusal_registry::CORE_REFUSAL_BASE,
///     [Instruction, AccountFrame, /* … every variant, in discriminant order */]
/// );
/// ```
///
/// The enum keeps its literal hexadecimal discriminants -- they are what a
/// reader greps for after seeing a code in a validator log, and what the
/// route census reads (`tools/gauntlet/census` walks the enum item itself, so
/// the enum must stay a plain item and never move inside this macro). The
/// macro supplies everything else the binding needs, once:
///
/// - `ALL`, every variant in discriminant order;
/// - `From<Enum> for ProgramError` as `Custom(code)`;
/// - the compile-time proof that the listed discriminants are the contiguous
///   run `base, base + 1, …` and never leave the band `base` sits in.
///
/// **Why a list and not two endpoints.** A ceiling assertion that names one
/// variant by hand as "the last one" says nothing about the variants after
/// it and goes stale silently every time the enum grows: Claims' bound went
/// on naming `ReleaseSuperseded` after a later variant landed, and for as
/// long as it stood the newest refusal in the program was checked by nothing.
/// So the band is checked over the whole list, element by element, and the
/// list is welded to the enum by an exhaustive match: a variant that is not
/// listed is a non-exhaustive match and does not compile, so a new refusal
/// cannot join quietly -- its author has to say where in the run it sits.
#[macro_export]
macro_rules! pin_refusal_band {
    ($refusal:ident, $base:expr, [$($variant:ident),+ $(,)?]) => {
        impl $refusal {
            /// Every refusal this enum can raise, in discriminant order.
            pub const ALL: [Self; $crate::__count_idents!($($variant)+)] = [$(Self::$variant),+];
        }

        const _: () = {
            // Exhaustive on purpose: a variant absent from the list is a
            // compile error here, not a discriminant nothing checks.
            const fn listed(refusal: $refusal) {
                match refusal {
                    $($refusal::$variant => {}),+
                }
            }
            const BASE: u32 = $base;
            const CEILING: u32 = $crate::band_base_of(BASE) + $crate::BAND_SPAN;
            let mut index: u32 = 0;
            while (index as usize) < $refusal::ALL.len() {
                let variant = $refusal::ALL[index as usize];
                listed(variant);
                assert!(
                    variant as u32 == BASE + index,
                    "refusal discriminants are not the contiguous run from the band base that ALL lists"
                );
                assert!(
                    (variant as u32) < CEILING,
                    "refusal enum runs past its registered band"
                );
                index += 1;
            }
        };

        impl From<$refusal> for ::solana_program::program_error::ProgramError {
            fn from(value: $refusal) -> Self {
                Self::Custom(value as u32)
            }
        }
    };
}

// ------------------------------------------------- compile-time band proofs
//
// The table is the authority, so the table's own invariants are proved here
// rather than asserted in a test that a stale build could skip.

#[allow(
    clippy::indexing_slicing,
    reason = "const fn cannot call slice::get; the loop bound is the slice length"
)]
const fn bands_are_ascending_and_disjoint() -> bool {
    let mut index = 1;
    while index < BANDS.len() {
        let previous = BANDS[index - 1];
        let current = BANDS[index];
        if current.base <= previous.last() {
            return false;
        }
        index += 1;
    }
    true
}

#[allow(
    clippy::indexing_slicing,
    reason = "const fn cannot call slice::get; the loop bound is the slice length"
)]
const fn every_band_is_tiered_correctly() -> bool {
    let mut index = 0;
    while index < BANDS.len() {
        let band = BANDS[index];
        if band.span == 0 {
            return false;
        }
        let first = band_index(band.base);
        let last = band_index(band.last());
        // A band must not straddle two band indices, or `code >> 12` stops
        // being a program identity.
        if first != last {
            return false;
        }
        let tiered = match band.tier {
            BandTier::Program => first >= FIRST_PROGRAM_BAND && first < FIRST_TEST_BAND,
            BandTier::TestCaller => first >= FIRST_TEST_BAND,
        };
        if !tiered {
            return false;
        }
        index += 1;
    }
    true
}

const _: () = assert!(
    bands_are_ascending_and_disjoint(),
    "refusal bands must be listed in ascending order and must not overlap"
);
const _: () = assert!(
    every_band_is_tiered_correctly(),
    "every refusal band must occupy exactly one band index inside its tier"
);
const _: () = assert!(
    band_index(0) == 0 && band_index(BAND_SPAN - 1) == 0,
    "band 0 must cover the whole sub-0x1000 region that foreign programs use"
);
const _: () = assert!(
    BAND_SPAN == 1 << BAND_SHIFT,
    "the band span and the band shift describe the same partition"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_below_one_band_is_ours() {
        assert!(owner(0).is_none());
        assert!(owner(1).is_none());
        assert!(owner(BAND_SPAN - 1).is_none());
        assert!(owner(BAND_SPAN).is_some());
    }

    #[test]
    fn every_named_base_is_in_the_table() {
        // The array width is `BAND_COUNT`, which Lean authors, so a band added
        // to the allocation and not to this list -- or removed from one and not
        // the other -- fails to COMPILE rather than failing a number written
        // down beside the table. The number written down beside the table was
        // 27 while the table held 26, and had been since band 7 was retired.
        const NAMED_BASES: [u32; BAND_COUNT] = [
            REGISTRY_REFUSAL_BASE,
            RENT_REFUSAL_BASE,
            CORE_REFUSAL_BASE,
            TRADING_REFUSAL_BASE,
            CLAIMS_REFUSAL_BASE,
            CUSTODY_REFUSAL_BASE,
            RESOLUTION_REFUSAL_BASE,
            ACCELERATOR_REFUSAL_BASE,
            TEST_CLAIMS_AFFINE_BATCH_CALLER_BASE,
            TEST_CLAIMS_FRACTIONAL_SIGNED_DELTA_CALLER_BASE,
            TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE,
            TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE,
            TEST_CLAIMS_RATIONAL_V2_CALLER_BASE,
            TEST_CLAIMS_SPARSE_CHAIN_CALLER_BASE,
            TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE,
            TEST_CUSTODY_CALLER_BASE,
            TEST_DEALER_ACCELERATOR_CALLER_BASE,
            TEST_GENERAL_ACCELERATOR_CALLER_BASE,
            TEST_RESOLUTION_RECEIPT_CALLER_BASE,
            TEST_CLAIMS_FRACTIONAL_ATOMIC_CALLER_BASE,
            TEST_CLAIMS_CLAIM_CHECK_ESCROW_SIGNER_BASE,
            TEST_CLAIMS_FRACTIONAL_COMPACTION_CALLER_BASE,
        ];
        for base in NAMED_BASES {
            assert!(
                BANDS.iter().any(|band| band.base == base),
                "named base {base:#x} is not in BANDS"
            );
        }
        assert_eq!(
            BANDS
                .iter()
                .filter(|band| band.tier == BandTier::Program)
                .count(),
            PROGRAM_BAND_COUNT,
            "on-chain band population"
        );
        assert_eq!(
            BANDS
                .iter()
                .filter(|band| band.tier == BandTier::TestCaller)
                .count(),
            TEST_CALLER_BAND_COUNT,
            "test-caller band population"
        );
    }

    #[test]
    fn labels_and_packages_are_unique() {
        for (index, band) in BANDS.iter().enumerate() {
            for other in BANDS.iter().skip(index + 1) {
                assert_ne!(band.label, other.label, "duplicate band label");
                assert_ne!(band.package, other.package, "duplicate band package");
            }
        }
    }

    #[test]
    fn a_code_resolves_to_exactly_one_owner() {
        let band = owner(CLAIMS_REFUSAL_BASE + 0x100).expect("claims band");
        assert_eq!(band.label, "claims");
        assert_eq!(band_index(CLAIMS_REFUSAL_BASE + 0x100), 5);
    }

    #[test]
    fn test_callers_never_share_a_tier_with_programs() {
        for band in BANDS {
            match band.tier {
                BandTier::Program => assert!(band_index(band.base) < FIRST_TEST_BAND),
                BandTier::TestCaller => assert!(band_index(band.base) >= FIRST_TEST_BAND),
            }
        }
    }
}
