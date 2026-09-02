import Std.Tactic

/-!
# Refusal band allocation

Decision 0007's band scheme, and the register both the browser and the SDK
consume. A Solana `ProgramError::Custom` is a bare `u32` that the runtime
reports without saying which program produced it; this allocation makes the
code identify its program, by giving every package a disjoint 4096-code band
and never allocating band zero.

The scheme had no author. `crates/dclutch-refusal-registry` held the table by
hand and proved ascent and disjointness at compile time -- a real gate, and a
gate over an unauthored table -- while the browser and SDK obtained the SAME
table by running a REGULAR EXPRESSION over that Rust source. Two readings of
one fact, the second one a text scrape of the first, and nothing making either
answerable to a statement of what a band is.

This module is that statement. A band is an INDEX, not a base: the base is
`index * span`, so `code >>> shift` recovering the index is a theorem rather
than a convention, and a base that is not band-aligned cannot be written down.

What this module does not own: the 305 individual refusal codes. Those are
discriminants of per-program enums, generated into `docs/reference/refusals.md`
by `tools/genref`, and their author is each program. This module owns the
allocation those codes live inside, which is exactly what decision 0007 is
about and exactly what the two hand-copies disagreed about the shape of.
-/

namespace DClutch.RefusalBands

/-- Whether a band's codes can ever appear on a real cluster. -/
inductive BandTier where
  /-- An on-chain protocol program. -/
  | program
  /-- A test-only caller program, never deployed to a real cluster. -/
  | testCaller
  deriving DecidableEq, Repr

/-- One package's exclusive allocation, identified by its band INDEX. -/
structure Band where
  label : String
  package : String
  rustName : String
  index : Nat
  tier : BandTier
  deriving Repr

/-- `code >>> bandShift` is the band index. -/
def bandShift : Nat := 12

/-- Width of one band, in codes. A power of two so the shift recovers the
index, and hexadecimally round so a band reads off a validator log line. -/
def bandSpan : Nat := 4096

/-- First index available to on-chain protocol programs. Band zero is never
allocated, which is what makes a custom code below `0x1000` provably not ours. -/
def firstProgramBand : Nat := 1

/-- First index reserved for test-only callers. -/
def firstTestBand : Nat := 256

def Band.base (band : Band) : Nat := band.index * bandSpan
def Band.last (band : Band) : Nat := band.base + bandSpan - 1

def bands : List Band := [
  { label := "registry", package := "dclutch-registry-sbf",
    rustName := "REGISTRY_REFUSAL_BASE", index := 1,
    tier := .program },
  { label := "rent", package := "dclutch-rent-sbf",
    rustName := "RENT_REFUSAL_BASE", index := 2,
    tier := .program },
  { label := "core", package := "dclutch-core-sbf",
    rustName := "CORE_REFUSAL_BASE", index := 3,
    tier := .program },
  { label := "trading", package := "dclutch-trading-sbf",
    rustName := "TRADING_REFUSAL_BASE", index := 4,
    tier := .program },
  { label := "claims", package := "dclutch-claims-sbf",
    rustName := "CLAIMS_REFUSAL_BASE", index := 5,
    tier := .program },
  { label := "custody", package := "dclutch-custody-sbf",
    rustName := "CUSTODY_REFUSAL_BASE", index := 6,
    tier := .program },
  { label := "resolution", package := "dclutch-resolution-proof-sbf",
    rustName := "RESOLUTION_REFUSAL_BASE", index := 8,
    tier := .program },
  { label := "product-runtime-v2", package := "dclutch-product-runtime-v2-sbf",
    rustName := "PRODUCT_RUNTIME_V2_REFUSAL_BASE", index := 9,
    tier := .program },
  { label := "direct-aot", package := "dclutch-direct-aot-sbf",
    rustName := "DIRECT_AOT_REFUSAL_BASE", index := 10,
    tier := .program },
  { label := "series-shadow", package := "dclutch-series-shadow-sbf",
    rustName := "SERIES_SHADOW_REFUSAL_BASE", index := 11,
    tier := .program },
  { label := "general-accelerator", package := "dclutch-general-accelerator-sbf",
    rustName := "GENERAL_ACCELERATOR_REFUSAL_BASE", index := 12,
    tier := .program },
  { label := "dealer-accelerator", package := "dclutch-dealer-accelerator-sbf",
    rustName := "DEALER_ACCELERATOR_REFUSAL_BASE", index := 13,
    tier := .program },
  { label := "test/claims-affine-batch-caller", package := "dclutch-claims-affine-batch-test-caller-sbf",
    rustName := "TEST_CLAIMS_AFFINE_BATCH_CALLER_BASE", index := 256,
    tier := .testCaller },
  { label := "test/claims-fractional-signed-delta-caller", package := "dclutch-fractional-signed-delta-test-caller-sbf",
    rustName := "TEST_CLAIMS_FRACTIONAL_SIGNED_DELTA_CALLER_BASE", index := 257,
    tier := .testCaller },
  { label := "test/claims-liability-basis-caller", package := "dclutch-claims-liability-basis-test-caller-sbf",
    rustName := "TEST_CLAIMS_LIABILITY_BASIS_CALLER_BASE", index := 258,
    tier := .testCaller },
  { label := "test/claims-rational-lifecycle-caller", package := "dclutch-rational-lifecycle-test-caller-sbf",
    rustName := "TEST_CLAIMS_RATIONAL_LIFECYCLE_CALLER_BASE", index := 259,
    tier := .testCaller },
  { label := "test/claims-rational-v2-caller", package := "dclutch-rational-v2-test-caller-sbf",
    rustName := "TEST_CLAIMS_RATIONAL_V2_CALLER_BASE", index := 260,
    tier := .testCaller },
  { label := "test/claims-sparse-chain-caller", package := "dclutch-claims-sparse-chain-test-caller-sbf",
    rustName := "TEST_CLAIMS_SPARSE_CHAIN_CALLER_BASE", index := 261,
    tier := .testCaller },
  { label := "test/claims-terminal-settlement-caller", package := "dclutch-terminal-settlement-test-caller-sbf",
    rustName := "TEST_CLAIMS_TERMINAL_SETTLEMENT_CALLER_BASE", index := 262,
    tier := .testCaller },
  { label := "test/custody-caller", package := "dclutch-custody-test-caller-sbf",
    rustName := "TEST_CUSTODY_CALLER_BASE", index := 263,
    tier := .testCaller },
  { label := "test/dealer-accelerator-caller", package := "dclutch-dealer-accelerator-test-caller-sbf",
    rustName := "TEST_DEALER_ACCELERATOR_CALLER_BASE", index := 264,
    tier := .testCaller },
  { label := "test/general-accelerator-caller", package := "dclutch-general-accelerator-test-caller-sbf",
    rustName := "TEST_GENERAL_ACCELERATOR_CALLER_BASE", index := 265,
    tier := .testCaller },
  { label := "test/resolution-receipt-caller", package := "dclutch-resolution-receipt-test-caller-sbf",
    rustName := "TEST_RESOLUTION_RECEIPT_CALLER_BASE", index := 266,
    tier := .testCaller },
  { label := "test/claims-fractional-atomic-caller", package := "dclutch-fractional-atomic-test-caller-sbf",
    rustName := "TEST_CLAIMS_FRACTIONAL_ATOMIC_CALLER_BASE", index := 267,
    tier := .testCaller },
  { label := "test/claims-claim-check-escrow-signer", package := "dclutch-claim-check-escrow-signer-test-sbf",
    rustName := "TEST_CLAIMS_CLAIM_CHECK_ESCROW_SIGNER_BASE", index := 268,
    tier := .testCaller },
  { label := "test/claims-fractional-compaction-caller", package := "dclutch-fractional-compaction-test-caller-sbf",
    rustName := "TEST_CLAIMS_FRACTIONAL_COMPACTION_CALLER_BASE", index := 269,
    tier := .testCaller }
]

/-! ## What the allocation is -/

theorem band_span_is_two_to_the_shift : bandSpan = 2 ^ bandShift := by native_decide

/-- Twenty-six allocations: twelve on-chain programs and fourteen test-only
callers. -/
theorem band_population_is_exact :
    bands.length = 26 ∧
      (bands.filter (fun band => band.tier == .program)).length = 12 ∧
      (bands.filter (fun band => band.tier == .testCaller)).length = 14 := by
  native_decide

/-- Band zero is never allocated. This is the property that makes a custom
code below `0x1000` provably foreign -- SPL Token, the Loader and every other
program number from zero, and now nothing of ours does. -/
theorem band_zero_is_never_allocated :
    bands.all (fun band => decide (firstProgramBand ≤ band.index)) = true := by
  native_decide

/-- Indices strictly ascend, so with a uniform span the bands are pairwise
disjoint and the table is in the order the ADR reads it. -/
def strictlyAscending : List Nat → Bool
  | [] => true
  | [_] => true
  | left :: right :: rest => decide (left < right) && strictlyAscending (right :: rest)

theorem bands_ascend_strictly :
    strictlyAscending (bands.map Band.index) = true := by native_decide

/-- The defining property of the scheme: shifting any code in a band by
`bandShift` recovers that band's index, at both ends of the band. A base that
was not band-aligned would fail here, and it cannot even be written down
because a base is derived from an index. -/
theorem shifting_a_code_recovers_its_band :
    bands.all (fun band =>
      (band.base >>> bandShift == band.index) &&
        (band.last >>> bandShift == band.index) &&
        (band.last == band.base + bandSpan - 1)) = true := by
  native_decide

/-- The tiers are separated by `firstTestBand`, so a test caller's refusal can
never be mistaken for a deployed program's in a validator log. -/
theorem tiers_are_separated_by_the_first_test_band :
    bands.all (fun band =>
      match band.tier with
      | .program => decide (band.index < firstTestBand)
      | .testCaller => decide (firstTestBand ≤ band.index)) = true := by
  native_decide

/-- Labels and packages are unique, which is what `band_for_label` and
`band_for_package` assume when they return the first match. -/
theorem labels_and_packages_are_unique :
    (bands.map Band.label).eraseDups.length = bands.length ∧
      (bands.map Band.package).eraseDups.length = bands.length ∧
      (bands.map Band.rustName).eraseDups.length = bands.length := by
  native_decide

/-- The gaps are real and they are checkable. Band 7 is RETIRED --
`dclutch-dealer-sbf`, deleted 2026-09-02 -- and bands 14, 15 and 16 were
drafted for the three banished DCLTCAT1 proof programs and never allocated.
Bands are append-only: a spent band is a gap, never a reuse. This was prose in
a Rust comment; ascent alone does not say it, because ascent permits filling a
gap later. -/
theorem retired_and_unallocated_indices_stay_absent :
    ([7, 14, 15, 16].all (fun index =>
      !(bands.any (fun band => band.index == index)))) = true := by
  native_decide

end DClutch.RefusalBands
