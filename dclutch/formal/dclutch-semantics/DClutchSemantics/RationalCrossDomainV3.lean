import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Rational representation / Product result-domain separation

The Rational representation descriptor and LBV2 Claims state use one native
basis width `K`.  Product terminal resolution uses an independently
authenticated result width `N`.  A representation support coordinate is
therefore bounded by `K`, while a Product terminal selector is bounded by `N`.
Neither admission implies `K = N`.

The fixed layout below is a translation-test corpus, not a persisted protocol
record.  Lean emits canonical unequal-width witnesses and hostile substitutions
so Rust tests can check the executable adapter boundary without introducing a
second request or payout authority.
-/

namespace DClutch.RationalCrossDomainV3

open DClutch.AbiSchema

/-! ## Typed semantic domains -/

structure AuthenticatedWidths where
  basisWidth : Nat
  terminalWidth : Nat
  deriving DecidableEq, Repr

inductive Coordinate where
  | claim (index : Nat)
  | terminal (selector : Nat)
  deriving DecidableEq, Repr

def AuthenticatedWidths.valid (widths : AuthenticatedWidths) : Prop :=
  2 ≤ widths.basisWidth ∧ 2 ≤ widths.terminalWidth

def AuthenticatedWidths.admits
    (widths : AuthenticatedWidths) : Coordinate → Prop
  | .claim index => index < widths.basisWidth
  | .terminal selector => selector < widths.terminalWidth

theorem claim_coordinate_is_bounded_only_by_basis
    (widths : AuthenticatedWidths) (index : Nat) :
    widths.admits (.claim index) ↔ index < widths.basisWidth := by
  rfl

theorem terminal_selector_is_bounded_only_by_product
    (widths : AuthenticatedWidths) (selector : Nat) :
    widths.admits (.terminal selector) ↔ selector < widths.terminalWidth := by
  rfl

theorem claim_and_terminal_coordinates_are_disjoint
    (index selector : Nat) :
    Coordinate.claim index ≠ Coordinate.terminal selector := by
  simp

structure JoinedObservation where
  widths : AuthenticatedWidths
  claimCoordinate : Nat
  terminalSelector : Nat
  deriving DecidableEq, Repr

def JoinedObservation.valid (observation : JoinedObservation) : Prop :=
  observation.widths.valid ∧
  observation.widths.admits (.claim observation.claimCoordinate) ∧
  observation.widths.admits (.terminal observation.terminalSelector)

theorem independent_widths_admit
    (basisWidth terminalWidth claimCoordinate terminalSelector : Nat)
    (basisPositive : 2 ≤ basisWidth)
    (terminalPositive : 2 ≤ terminalWidth)
    (claimBound : claimCoordinate < basisWidth)
    (terminalBound : terminalSelector < terminalWidth) :
    (JoinedObservation.mk ⟨basisWidth, terminalWidth⟩
      claimCoordinate terminalSelector).valid := by
  exact ⟨⟨basisPositive, terminalPositive⟩, claimBound, terminalBound⟩

theorem rejected_claim_cannot_be_admitted_as_support
    (widths : AuthenticatedWidths) (index : Nat)
    (outside : widths.basisWidth ≤ index) :
    ¬ widths.admits (.claim index) := by
  simp only [AuthenticatedWidths.admits]
  omega

theorem rejected_terminal_cannot_be_admitted_as_result
    (widths : AuthenticatedWidths) (selector : Nat)
    (outside : widths.terminalWidth ≤ selector) :
    ¬ widths.admits (.terminal selector) := by
  simp only [AuthenticatedWidths.admits]
  omega

/-- A Product selector that lies beyond the native basis remains a valid
Product coordinate when it is below `N`; changing only its domain tag cannot
turn it into a Claims coordinate. -/
theorem terminal_only_selector_cannot_be_reinterpreted_as_claim
    (widths : AuthenticatedWidths) (selector : Nat)
    (outsideBasis : widths.basisWidth ≤ selector)
    (insideTerminal : selector < widths.terminalWidth) :
    widths.admits (.terminal selector) ∧
      ¬ widths.admits (.claim selector) := by
  constructor
  · exact insideTerminal
  · exact rejected_claim_cannot_be_admitted_as_support widths selector outsideBasis

/-! ## Lean-owned differential corpus layout -/

def corpusMagic : List UInt8 := "DCRKNV3!".toUTF8.toList
def corpusVersion : Nat := 3

inductive CorpusField where
  | magic | version | reserved
  | basisWidth | terminalWidth | claimCoordinate | terminalSelector
  deriving DecidableEq, Repr

def corpusSchema : List (FieldSpec CorpusField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reserved, .reserved 2⟩,
  ⟨.basisWidth, .u32⟩,
  ⟨.terminalWidth, .u32⟩,
  ⟨.claimCoordinate, .u32⟩,
  ⟨.terminalSelector, .u32⟩
]

def corpusLayout : List (PlacedField CorpusField) := specialize corpusSchema
def corpusBytes : Nat := schemaWidth corpusSchema

namespace CorpusField

def rustName : CorpusField → String
  | .magic => "RATIONAL_CROSS_DOMAIN_MAGIC_OFFSET_V3"
  | .version => "RATIONAL_CROSS_DOMAIN_VERSION_OFFSET_V3"
  | .reserved => "RATIONAL_CROSS_DOMAIN_RESERVED_OFFSET_V3"
  | .basisWidth => "RATIONAL_CROSS_DOMAIN_BASIS_WIDTH_OFFSET_V3"
  | .terminalWidth => "RATIONAL_CROSS_DOMAIN_TERMINAL_WIDTH_OFFSET_V3"
  | .claimCoordinate => "RATIONAL_CROSS_DOMAIN_CLAIM_COORDINATE_OFFSET_V3"
  | .terminalSelector => "RATIONAL_CROSS_DOMAIN_TERMINAL_SELECTOR_OFFSET_V3"

end CorpusField

def zeros (count : Nat) : List UInt8 := List.replicate count 0

def encodeCorpus (observation : JoinedObservation) : List UInt8 :=
  corpusMagic ++ DClutch.Codec.encodeLE 2 corpusVersion ++ zeros 2 ++
  DClutch.Codec.encodeLE 4 observation.widths.basisWidth ++
  DClutch.Codec.encodeLE 4 observation.widths.terminalWidth ++
  DClutch.Codec.encodeLE 4 observation.claimCoordinate ++
  DClutch.Codec.encodeLE 4 observation.terminalSelector

def witnessK3N9 : JoinedObservation := ⟨⟨3, 9⟩, 2, 8⟩
def witnessK3N258 : JoinedObservation := ⟨⟨3, 258⟩, 2, 257⟩

def patch (bytes : List UInt8) (offset : Nat) (replacement : List UInt8) : List UInt8 :=
  bytes.take offset ++ replacement ++ bytes.drop (offset + replacement.length)

def corpusOffset (field : CorpusField) : Nat :=
  ((coordinate? field corpusLayout).getD (0, 0)).1

def hostileClaimAtK : List UInt8 :=
  patch (encodeCorpus witnessK3N9) (corpusOffset .claimCoordinate)
    (DClutch.Codec.encodeLE 4 3)

def hostileTerminalAtN9 : List UInt8 :=
  patch (encodeCorpus witnessK3N9) (corpusOffset .terminalSelector)
    (DClutch.Codec.encodeLE 4 9)

/-- Substituting an otherwise valid Product selector into the Claims field. -/
def hostileTerminalAsClaimN258 : List UInt8 :=
  patch (encodeCorpus witnessK3N258) (corpusOffset .claimCoordinate)
    (DClutch.Codec.encodeLE 4 257)

def hostileTerminalAtN258 : List UInt8 :=
  patch (encodeCorpus witnessK3N258) (corpusOffset .terminalSelector)
    (DClutch.Codec.encodeLE 4 258)

def hostileZeroBasisWidth : List UInt8 :=
  patch (encodeCorpus witnessK3N9) (corpusOffset .basisWidth)
    (DClutch.Codec.encodeLE 4 0)

def hostileZeroTerminalWidth : List UInt8 :=
  patch (encodeCorpus witnessK3N9) (corpusOffset .terminalWidth)
    (DClutch.Codec.encodeLE 4 0)

def hostileNonzeroReserved : List UInt8 :=
  List.set (encodeCorpus witnessK3N9) (corpusOffset .reserved) 1

theorem corpus_layout_width_is_exact : corpusBytes = 28 := by native_decide

theorem corpus_layout_is_byte_disjoint : corpusLayout.Pairwise Before :=
  specializeFrom_pairwise 0 corpusSchema

theorem witness_k3_n9_is_valid : witnessK3N9.valid := by
  exact independent_widths_admit 3 9 2 8 (by omega) (by omega) (by omega) (by omega)

theorem witness_k3_n258_is_valid : witnessK3N258.valid := by
  exact independent_widths_admit 3 258 2 257
    (by omega) (by omega) (by omega) (by omega)

theorem unequal_width_witnesses_require_no_equality :
    witnessK3N9.widths.basisWidth ≠ witnessK3N9.widths.terminalWidth ∧
      witnessK3N258.widths.basisWidth ≠ witnessK3N258.widths.terminalWidth := by
  decide

theorem k3_n9_terminal_selector_cannot_become_claim :
    witnessK3N9.widths.admits (.terminal witnessK3N9.terminalSelector) ∧
      ¬ witnessK3N9.widths.admits (.claim witnessK3N9.terminalSelector) := by
  change 8 < 9 ∧ ¬ 8 < 3
  omega

theorem k3_n258_terminal_selector_cannot_become_claim :
    witnessK3N258.widths.admits (.terminal witnessK3N258.terminalSelector) ∧
      ¬ witnessK3N258.widths.admits (.claim witnessK3N258.terminalSelector) := by
  change 257 < 258 ∧ ¬ 257 < 3
  omega

theorem canonical_witnesses_have_exact_width :
    (encodeCorpus witnessK3N9).length = corpusBytes ∧
      (encodeCorpus witnessK3N258).length = corpusBytes := by native_decide

theorem hostile_corpus_preserves_exact_width :
    hostileClaimAtK.length = corpusBytes ∧
      hostileTerminalAtN9.length = corpusBytes ∧
      hostileTerminalAsClaimN258.length = corpusBytes ∧
      hostileTerminalAtN258.length = corpusBytes ∧
      hostileZeroBasisWidth.length = corpusBytes ∧
      hostileZeroTerminalWidth.length = corpusBytes ∧
      hostileNonzeroReserved.length = corpusBytes := by native_decide

end DClutch.RationalCrossDomainV3
