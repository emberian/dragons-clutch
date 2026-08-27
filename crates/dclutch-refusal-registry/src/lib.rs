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

/// Width of one refusal band, in codes.
///
/// A power of two so that `code >> BAND_SHIFT` recovers the band, and a
/// hexadecimally round one so that a band reads directly off a log line.
pub const BAND_SPAN: u32 = 0x1000;

/// `code >> BAND_SHIFT` is the band index. See [`BAND_SPAN`].
pub const BAND_SHIFT: u32 = 12;

/// First band index available to on-chain protocol programs.
pub const FIRST_PROGRAM_BAND: u32 = 0x001;

/// First band index reserved for test-only caller programs.
///
/// A caller program below this line would be indistinguishable from a
/// deployed one in a validator log, which is the confusion this whole
/// registry exists to end.
pub const FIRST_TEST_BAND: u32 = 0x100;

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

// --------------------------------------------------------------- band bases
//
// One named constant per owner. Programs import their own; tooling walks
// `BANDS`. The two must not drift, which `BASES_MATCH_BANDS` below proves at
// compile time.

/// Band 1 — `dclutch-registry-sbf`.
pub const REGISTRY_REFUSAL_BASE: u32 = 0x0000_1000;
/// Band 2 — `dclutch-rent-sbf`.
pub const RENT_REFUSAL_BASE: u32 = 0x0000_2000;
/// Band 3 — `dclutch-core-sbf`.
pub const CORE_REFUSAL_BASE: u32 = 0x0000_3000;
/// Band 4 — `dclutch-trading-sbf`.
pub const TRADING_REFUSAL_BASE: u32 = 0x0000_4000;
/// Band 5 — `dclutch-claims-sbf`.
pub const CLAIMS_REFUSAL_BASE: u32 = 0x0000_5000;
/// Band 6 — `dclutch-custody-sbf`.
pub const CUSTODY_REFUSAL_BASE: u32 = 0x0000_6000;
/// Band 7 — `dclutch-dealer-sbf`.
pub const DEALER_REFUSAL_BASE: u32 = 0x0000_7000;
/// Band 8 — `dclutch-resolution-proof-sbf`.
pub const RESOLUTION_REFUSAL_BASE: u32 = 0x0000_8000;
/// Band 9 — `dclutch-product-runtime-v2-sbf`.
pub const PRODUCT_RUNTIME_V2_REFUSAL_BASE: u32 = 0x0000_9000;
/// Band 10 — `dclutch-direct-aot-sbf`.
pub const DIRECT_AOT_REFUSAL_BASE: u32 = 0x0000_A000;
/// Band 11 — `dclutch-series-shadow-sbf`.
pub const SERIES_SHADOW_REFUSAL_BASE: u32 = 0x0000_B000;
/// Band 12 — `dclutch-general-accelerator-sbf`.
pub const GENERAL_ACCELERATOR_REFUSAL_BASE: u32 = 0x0000_C000;
/// Band 13 — `dclutch-dealer-accelerator-sbf`.
pub const DEALER_ACCELERATOR_REFUSAL_BASE: u32 = 0x0000_D000;
/// Band 14 — `dclutch-controller-proof-sbf`.
pub const CONTROLLER_PROOF_REFUSAL_BASE: u32 = 0x0000_E000;
/// Band 15 — `dclutch-custody-proof-sbf`.
pub const CUSTODY_PROOF_REFUSAL_BASE: u32 = 0x0000_F000;
/// Band 16 — `dclutch-claims-proof-sbf`.
///
/// Allocated but unpopulated: the program is a generated-profile evaluator and
/// raises no custom code today. The band is held so that when it grows one it
/// does not reach for zero.
pub const CLAIMS_PROOF_REFUSAL_BASE: u32 = 0x0001_0000;

/// Band 0x100 — `dclutch-claims-sbf` test caller `affine-batch-caller`.
pub const TEST_CLAIMS_AFFINE_BATCH_CALLER_BASE: u32 = 0x0010_0000;
/// Band 0x101 — `dclutch-claims-sbf` test caller `fractional-signed-delta-caller`.
pub const TEST_CLAIMS_FRACTIONAL_SIGNED_DELTA_CALLER_BASE: u32 = 0x0010_1000;
/// Band 0x102 — `dclutch-claims-sbf` test caller `liability-basis-caller`.
pub const TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE: u32 = 0x0010_2000;
/// Band 0x103 — `dclutch-claims-sbf` test caller `rational-lifecycle-caller`.
pub const TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE: u32 = 0x0010_3000;
/// Band 0x104 — `dclutch-claims-sbf` test caller `rational-v2-caller`.
pub const TEST_CLAIMS_RATIONAL_V2_CALLER_BASE: u32 = 0x0010_4000;
/// Band 0x105 — `dclutch-claims-sbf` test caller `sparse-chain-caller`.
pub const TEST_CLAIMS_SPARSE_CHAIN_CALLER_BASE: u32 = 0x0010_5000;
/// Band 0x106 — `dclutch-claims-sbf` test caller `terminal-settlement-caller`.
pub const TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE: u32 = 0x0010_6000;
/// Band 0x107 — `dclutch-custody-sbf` test caller `caller`.
pub const TEST_CUSTODY_CALLER_BASE: u32 = 0x0010_7000;
/// Band 0x108 — `dclutch-dealer-accelerator-sbf` test caller `dealer-caller`.
pub const TEST_DEALER_ACCELERATOR_CALLER_BASE: u32 = 0x0010_8000;
/// Band 0x109 — `dclutch-general-accelerator-sbf` test caller `general-caller`.
pub const TEST_GENERAL_ACCELERATOR_CALLER_BASE: u32 = 0x0010_9000;
/// Band 0x10A — `dclutch-svm-harness` test caller `resolution-receipt-caller`.
pub const TEST_RESOLUTION_RECEIPT_CALLER_BASE: u32 = 0x0010_A000;

// --------------------------------------------------------------- band table

/// Every allocated band, ascending by base. This table is the authority; the
/// named constants above and the ADR are both views of it.
pub const BANDS: &[RefusalBand] = &[
    RefusalBand {
        label: "registry",
        package: "dclutch-registry-sbf",
        base: REGISTRY_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "rent",
        package: "dclutch-rent-sbf",
        base: RENT_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "core",
        package: "dclutch-core-sbf",
        base: CORE_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "trading",
        package: "dclutch-trading-sbf",
        base: TRADING_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "claims",
        package: "dclutch-claims-sbf",
        base: CLAIMS_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "custody",
        package: "dclutch-custody-sbf",
        base: CUSTODY_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "dealer",
        package: "dclutch-dealer-sbf",
        base: DEALER_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "resolution",
        package: "dclutch-resolution-proof-sbf",
        base: RESOLUTION_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "product-runtime-v2",
        package: "dclutch-product-runtime-v2-sbf",
        base: PRODUCT_RUNTIME_V2_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "direct-aot",
        package: "dclutch-direct-aot-sbf",
        base: DIRECT_AOT_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "series-shadow",
        package: "dclutch-series-shadow-sbf",
        base: SERIES_SHADOW_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "general-accelerator",
        package: "dclutch-general-accelerator-sbf",
        base: GENERAL_ACCELERATOR_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "dealer-accelerator",
        package: "dclutch-dealer-accelerator-sbf",
        base: DEALER_ACCELERATOR_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "controller-proof",
        package: "dclutch-controller-proof-sbf",
        base: CONTROLLER_PROOF_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "custody-proof",
        package: "dclutch-custody-proof-sbf",
        base: CUSTODY_PROOF_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "claims-proof",
        package: "dclutch-claims-proof-sbf",
        base: CLAIMS_PROOF_REFUSAL_BASE,
        span: BAND_SPAN,
        tier: BandTier::Program,
    },
    RefusalBand {
        label: "test/claims-affine-batch-caller",
        package: "dclutch-claims-affine-batch-test-caller",
        base: TEST_CLAIMS_AFFINE_BATCH_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/claims-fractional-signed-delta-caller",
        package: "dclutch-fractional-signed-delta-test-caller",
        base: TEST_CLAIMS_FRACTIONAL_SIGNED_DELTA_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/claims-liability-basis-caller",
        package: "dclutch-liability-basis-test-caller",
        base: TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/claims-rational-lifecycle-caller",
        package: "dclutch-rational-lifecycle-test-caller",
        base: TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/claims-rational-v2-caller",
        package: "dclutch-rational-v2-test-caller",
        base: TEST_CLAIMS_RATIONAL_V2_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/claims-sparse-chain-caller",
        package: "dclutch-sparse-chain-test-caller",
        base: TEST_CLAIMS_SPARSE_CHAIN_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/claims-terminal-settlement-caller",
        package: "dclutch-terminal-settlement-test-caller",
        base: TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/custody-caller",
        package: "dclutch-custody-test-caller",
        base: TEST_CUSTODY_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/dealer-accelerator-caller",
        package: "dclutch-dealer-accelerator-test-caller",
        base: TEST_DEALER_ACCELERATOR_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/general-accelerator-caller",
        package: "dclutch-general-accelerator-test-caller",
        base: TEST_GENERAL_ACCELERATOR_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
    RefusalBand {
        label: "test/resolution-receipt-caller",
        package: "dclutch-resolution-receipt-test-caller",
        base: TEST_RESOLUTION_RECEIPT_CALLER_BASE,
        span: BAND_SPAN,
        tier: BandTier::TestCaller,
    },
];

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
        for base in [
            REGISTRY_REFUSAL_BASE,
            RENT_REFUSAL_BASE,
            CORE_REFUSAL_BASE,
            TRADING_REFUSAL_BASE,
            CLAIMS_REFUSAL_BASE,
            CUSTODY_REFUSAL_BASE,
            DEALER_REFUSAL_BASE,
            RESOLUTION_REFUSAL_BASE,
            PRODUCT_RUNTIME_V2_REFUSAL_BASE,
            DIRECT_AOT_REFUSAL_BASE,
            SERIES_SHADOW_REFUSAL_BASE,
            GENERAL_ACCELERATOR_REFUSAL_BASE,
            DEALER_ACCELERATOR_REFUSAL_BASE,
            CONTROLLER_PROOF_REFUSAL_BASE,
            CUSTODY_PROOF_REFUSAL_BASE,
            CLAIMS_PROOF_REFUSAL_BASE,
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
        ] {
            assert!(
                BANDS.iter().any(|band| band.base == base),
                "named base {base:#x} is not in BANDS"
            );
        }
        assert_eq!(BANDS.len(), 27, "BANDS gained or lost an entry");
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
