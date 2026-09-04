import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import DClutchSemantics.SourceRecoveryPolicyV2Abi
import Std.Tactic

/-!
# Runtime-width Source resolution state V2

The mutable state binds one Market generation to the content digest of the
compact `SourceMaterialV2`.  It stores a native `u32` Product selector; Product
outcome width remains foreign authenticated context and is never copied into
the state.  `decisionValid` is the terminal read join used by Core.
-/

namespace DClutch.SourceResolutionStateV2Abi

open DClutch
open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x53, 0x52, 0x53, 0x32]
def schemaVersion : Nat := 2
def pdaDomain : List UInt8 := "dclutch/source-state/v2".toUTF8.toList

inductive Field where
  | magic | version | phase | activeAttempt | terminalRoute | pdaBump
  | reservedHeader | selector | reservedSelector
  | market | generation | materialDigest | rentBeneficiary
  | reopenLink | resolutionEvidence | terminalSequence
  | resolvedAt | retiredAt | reservedTail
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.phase, .u8⟩,
  ⟨.activeAttempt, .u8⟩,
  ⟨.terminalRoute, .u8⟩,
  ⟨.pdaBump, .u8⟩,
  ⟨.reservedHeader, .reserved 2⟩,
  ⟨.selector, .u32⟩,
  ⟨.reservedSelector, .reserved 4⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.materialDigest, .bytes 32⟩,
  ⟨.rentBeneficiary, .bytes 32⟩,
  ⟨.reopenLink, .bytes 32⟩,
  ⟨.resolutionEvidence, .bytes 32⟩,
  ⟨.terminalSequence, .u64⟩,
  ⟨.resolvedAt, .u64⟩,
  ⟨.retiredAt, .u64⟩,
  ⟨.reservedTail, .reserved 8⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

/-- The greatest number of funded recovery attempts a market can buy, and
therefore the exclusive bound on the record's `activeAttempt` byte.

It had three authors: a bare `4` inside `State.valid` below, a bare
`RECOVERY_POLICY_MAX_ATTEMPTS_V2` emitted from the policy's own module, and a
hand-written `MAX_RECOVERY_ATTEMPTS_V2` in
`crates/dclutch-source-contract/src/source_resolution_v2.rs`.  The record's
bound is not independently chosen -- it is the policy's capacity, because an
`activeAttempt` the policy cannot fund is an attempt nothing paid for -- so it
is defined as that number rather than as a copy of it. -/
def maxRecoveryAttempts : Nat := SourceRecoveryPolicyV2Abi.maxAttempts

namespace Field

def rustName : Field → String
  | .magic => "SOURCE_RESOLUTION_STATE_V2_MAGIC_OFFSET"
  | .version => "SOURCE_RESOLUTION_STATE_V2_VERSION_OFFSET"
  | .phase => "SOURCE_RESOLUTION_STATE_V2_PHASE_OFFSET"
  | .activeAttempt => "SOURCE_RESOLUTION_STATE_V2_ACTIVE_ATTEMPT_OFFSET"
  | .terminalRoute => "SOURCE_RESOLUTION_STATE_V2_TERMINAL_ROUTE_OFFSET"
  | .pdaBump => "SOURCE_RESOLUTION_STATE_V2_PDA_BUMP_OFFSET"
  | .reservedHeader => "SOURCE_RESOLUTION_STATE_V2_RESERVED_HEADER_OFFSET"
  | .selector => "SOURCE_RESOLUTION_STATE_V2_SELECTOR_OFFSET"
  | .reservedSelector => "SOURCE_RESOLUTION_STATE_V2_RESERVED_SELECTOR_OFFSET"
  | .market => "SOURCE_RESOLUTION_STATE_V2_MARKET_OFFSET"
  | .generation => "SOURCE_RESOLUTION_STATE_V2_GENERATION_OFFSET"
  | .materialDigest => "SOURCE_RESOLUTION_STATE_V2_MATERIAL_DIGEST_OFFSET"
  | .rentBeneficiary => "SOURCE_RESOLUTION_STATE_V2_RENT_BENEFICIARY_OFFSET"
  | .reopenLink => "SOURCE_RESOLUTION_STATE_V2_REOPEN_LINK_OFFSET"
  | .resolutionEvidence => "SOURCE_RESOLUTION_STATE_V2_RESOLUTION_EVIDENCE_OFFSET"
  | .terminalSequence => "SOURCE_RESOLUTION_STATE_V2_TERMINAL_SEQUENCE_OFFSET"
  | .resolvedAt => "SOURCE_RESOLUTION_STATE_V2_RESOLVED_AT_OFFSET"
  | .retiredAt => "SOURCE_RESOLUTION_STATE_V2_RETIRED_AT_OFFSET"
  | .reservedTail => "SOURCE_RESOLUTION_STATE_V2_RESERVED_TAIL_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0

end Field

theorem exact_width : bytes = 224 := by native_decide

theorem schema_well_formed : WellFormed schema := by
  simp [WellFormed, schema, FieldKind.byteWidth]

theorem layout_is_byte_disjoint : layout.Pairwise Before :=
  specializeFrom_pairwise 0 schema

structure State where
  phase : Nat
  activeAttempt : Nat
  terminalRoute : Nat
  pdaBump : Nat
  selector : Nat
  market : Nat
  generation : Nat
  materialDigest : Nat
  rentBeneficiary : Nat
  reopenLink : Nat
  resolutionEvidence : Nat
  terminalSequence : Nat
  resolvedAt : Nat
  retiredAt : Nat
  deriving DecidableEq, Repr

def fits (width value : Nat) : Bool := value < 256 ^ width

/-- The six phases one Market generation's Source resolution moves through.

This module has stated the machine's whole canonicity rule since it was
written -- `State.valid` is a six-armed match on the phase byte, and it is the
strongest statement of the machine anywhere in the tree.  What it did not do
was NAME the six tags: they were bare numerals inside the arms, and
`crates/dclutch-source-contract/src/lib.rs` independently wrote the same six as
`#[repr(u8)]` discriminants and again as decoder arms.  So one machine had
three authors, and the Lean one -- the only one that says what each phase
MEANS for the record's other fields -- exported none of them. -/
inductive Phase where
  | primary | recovery | resolved | exhausted | failureCommitted | retired
  deriving DecidableEq, Repr

namespace Phase

def all : List Phase :=
  [.primary, .recovery, .resolved, .exhausted, .failureCommitted, .retired]

/-- The wire tag persisted in the phase byte. -/
def tag : Phase → Nat
  | .primary => 0
  | .recovery => 1
  | .resolved => 2
  | .exhausted => 3
  | .failureCommitted => 4
  | .retired => 5

def rustName : Phase → String
  | .primary => "SOURCE_RESOLUTION_PHASE_PRIMARY_V1"
  | .recovery => "SOURCE_RESOLUTION_PHASE_RECOVERY_V1"
  | .resolved => "SOURCE_RESOLUTION_PHASE_RESOLVED_V1"
  | .exhausted => "SOURCE_RESOLUTION_PHASE_EXHAUSTED_V1"
  | .failureCommitted => "SOURCE_RESOLUTION_PHASE_FAILURE_COMMITTED_V1"
  | .retired => "SOURCE_RESOLUTION_PHASE_RETIRED_V1"

def doc : Phase → String
  | .primary => "Primary source may still be accepted."
  | .recovery => "Exactly one ordered recovery attempt may be accepted."
  | .resolved => "A primary or recovery result has been committed."
  | .exhausted => "Every admitted attempt is exhausted; no result is selected yet."
  | .failureCommitted => "Product-owned failure semantics have been committed."
  | .retired => "Terminal state was retired after settlement."

/-- Whether Core may join on this state as a decided one.  `Exhausted` is the
phase this predicate is most easily got wrong about: every attempt is spent and
NO result is selected, so it is an end of the attempt sequence and not a
terminal read. -/
def terminal : Phase → Bool
  | .resolved | .failureCommitted | .retired => true
  | .primary | .recovery | .exhausted => false

/-- The phase one persisted byte names, or `none` for a byte outside the
machine. -/
def ofTag? (value : Nat) : Option Phase := all.find? fun phase => phase.tag == value

end Phase

/-- One past the greatest phase tag. -/
def phaseLimit : Nat := 6

/-- The terminal read join Core performs, resolved through the machine rather
than through a second list of numerals. -/
def terminalPhase (phase : Nat) : Bool :=
  match Phase.ofTag? phase with
  | some value => value.terminal
  | none => false

def State.valid (value : State) : Bool :=
  (Phase.ofTag? value.phase).isSome && value.activeAttempt < maxRecoveryAttempts &&
  value.terminalRoute ≤ 3 &&
  fits 1 value.pdaBump && fits 4 value.selector &&
  value.market != 0 && fits 32 value.market &&
  value.generation != 0 && fits 8 value.generation &&
  value.materialDigest != 0 && fits 32 value.materialDigest &&
  value.rentBeneficiary != 0 && fits 32 value.rentBeneficiary &&
  fits 32 value.reopenLink && fits 32 value.resolutionEvidence &&
  fits 8 value.terminalSequence && fits 8 value.resolvedAt && fits 8 value.retiredAt &&
  match value.phase with
  | 0 | 3 => value.activeAttempt = 0 && value.terminalRoute = 0 &&
      value.selector = 0 && value.resolutionEvidence = 0 &&
      value.terminalSequence = 0 && value.resolvedAt = 0 && value.retiredAt = 0
  | 1 => value.terminalRoute = 0 && value.selector = 0 &&
      value.resolutionEvidence = 0 && value.terminalSequence = 0 &&
      value.resolvedAt = 0 && value.retiredAt = 0
  | 2 => value.activeAttempt = 0 && (value.terminalRoute = 1 || value.terminalRoute = 2) &&
      value.resolutionEvidence != 0 && value.terminalSequence != 0 &&
      value.resolvedAt != 0 && value.retiredAt = 0
  | 4 => value.activeAttempt = 0 && value.terminalRoute = 3 &&
      value.resolutionEvidence != 0 && value.terminalSequence != 0 &&
      value.resolvedAt != 0 && value.retiredAt = 0
  | 5 => value.activeAttempt = 0 && value.terminalRoute != 0 &&
      value.resolutionEvidence != 0 && value.terminalSequence != 0 &&
      value.resolvedAt != 0 && value.retiredAt ≥ value.resolvedAt
  | _ => false

def encode (value : State) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++
  [UInt8.ofNat value.phase, UInt8.ofNat value.activeAttempt,
    UInt8.ofNat value.terminalRoute, UInt8.ofNat value.pdaBump] ++
  List.replicate 2 0 ++ Codec.encodeLE 4 value.selector ++ List.replicate 4 0 ++
  Codec.encodeLE 32 value.market ++ Codec.encodeLE 8 value.generation ++
  Codec.encodeLE 32 value.materialDigest ++ Codec.encodeLE 32 value.rentBeneficiary ++
  Codec.encodeLE 32 value.reopenLink ++ Codec.encodeLE 32 value.resolutionEvidence ++
  Codec.encodeLE 8 value.terminalSequence ++ Codec.encodeLE 8 value.resolvedAt ++
  Codec.encodeLE 8 value.retiredAt ++ List.replicate 8 0

theorem encoding_length (value : State) : (encode value).length = bytes := by
  simp [encode, bytes, schema, schemaWidth, magic, Codec.encodeLE_length,
    FieldKind.byteWidth]

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def decodedState (input : List UInt8) : State := {
  phase := sliceNat input Field.phase.offset 1
  activeAttempt := sliceNat input Field.activeAttempt.offset 1
  terminalRoute := sliceNat input Field.terminalRoute.offset 1
  pdaBump := sliceNat input Field.pdaBump.offset 1
  selector := sliceNat input Field.selector.offset 4
  market := sliceNat input Field.market.offset 32
  generation := sliceNat input Field.generation.offset 8
  materialDigest := sliceNat input Field.materialDigest.offset 32
  rentBeneficiary := sliceNat input Field.rentBeneficiary.offset 32
  reopenLink := sliceNat input Field.reopenLink.offset 32
  resolutionEvidence := sliceNat input Field.resolutionEvidence.offset 32
  terminalSequence := sliceNat input Field.terminalSequence.offset 8
  resolvedAt := sliceNat input Field.resolvedAt.offset 8
  retiredAt := sliceNat input Field.retiredAt.offset 8
}

def validBytes (input : List UInt8) : Bool :=
  input.length = bytes && input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reservedHeader.offset).take 2 = List.replicate 2 0 &&
  (input.drop Field.reservedSelector.offset).take 4 = List.replicate 4 0 &&
  (input.drop Field.reservedTail.offset).take 8 = List.replicate 8 0 &&
  (decodedState input).valid

def freshExample : State := {
  phase := 0, activeAttempt := 0, terminalRoute := 0, pdaBump := 7
  selector := 0, market := 1, generation := 9, materialDigest := 2
  rentBeneficiary := 3, reopenLink := 0, resolutionEvidence := 0
  terminalSequence := 0, resolvedAt := 0, retiredAt := 0
}

def wideTerminalExample : State := {
  freshExample with
    phase := 2
    terminalRoute := 1
    selector := 257
    resolutionEvidence := 4
    terminalSequence := 1
    resolvedAt := 100
}

theorem fresh_example_valid : freshExample.valid = true := by native_decide
theorem wide_terminal_example_valid : wideTerminalExample.valid = true := by native_decide

def decisionValid (state : State) (authenticatedOutcomeCount : Nat) : Bool :=
  state.valid && terminalPhase state.phase && authenticatedOutcomeCount ≥ 2 &&
  authenticatedOutcomeCount < 256 ^ 4 && state.selector < authenticatedOutcomeCount

theorem selector_257_is_not_truncated :
    decisionValid wideTerminalExample 258 = true := by native_decide

theorem selector_equal_to_count_refuses :
    decisionValid wideTerminalExample 257 = false := by native_decide

def refusalCorpus : List (List UInt8) := [
  (encode freshExample).set 0 0,
  (encode freshExample).set Field.version.offset 3,
  (encode freshExample).set Field.phase.offset 6,
  (encode freshExample).set Field.activeAttempt.offset 1,
  (encode freshExample).set Field.terminalRoute.offset 1,
  (encode freshExample).set Field.reservedHeader.offset 1,
  (encode freshExample).set Field.selector.offset 1,
  (encode freshExample).set Field.reservedSelector.offset 1,
  (encode freshExample).set Field.market.offset 0,
  (encode freshExample).set Field.generation.offset 0,
  (encode freshExample).set Field.materialDigest.offset 0,
  (encode freshExample).set Field.rentBeneficiary.offset 0,
  (encode freshExample).set Field.resolutionEvidence.offset 1,
  (encode freshExample).set Field.terminalSequence.offset 1,
  (encode freshExample).set Field.resolvedAt.offset 1,
  (encode freshExample).set Field.retiredAt.offset 1,
  (encode freshExample).set Field.reservedTail.offset 1
]

theorem encoded_examples_accepted :
    validBytes (encode freshExample) = true ∧
    validBytes (encode wideTerminalExample) = true := by native_decide

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

/-! ## The machine the record's own validity rule is written over -/

/-- **The six tags three files each authored a copy of.**  They are distinct,
they run from zero with no gap, and `phaseLimit` bounds them. -/
theorem the_machine_numbers_from_zero_without_a_gap :
    (Phase.all.map Phase.tag) = [0, 1, 2, 3, 4, 5] ∧
      (Phase.all.map Phase.tag).Nodup ∧
      Phase.all.all (fun phase => Phase.tag phase < phaseLimit) = true := by
  native_decide

/-- The `_ => false` arm of `State.valid` is exactly the complement of the
machine: every one of the six tags reaches a real arm and every other byte in
`0..255` falls through to refusal.  This is what `value.phase ≤ 5` used to say
by arithmetic coincidence, and what the Rust decoder's `_ => Err` says on its
own side. -/
theorem exactly_the_machine_s_tags_reach_a_validity_arm :
    (List.range 256).filter (fun tag => (Phase.ofTag? tag).isSome) =
      Phase.all.map Phase.tag := by
  native_decide

/-- **`Exhausted` is not terminal, and that is the whole point of naming
these.**  Three of six phases are a terminal read Core may join on; the fourth
end-of-sequence phase, `Exhausted`, has spent every attempt and selected no
result, so a reader that treated "no attempts remain" as "decided" would settle
a Market on nothing.  `terminalPhase` is now this predicate rather than a second
list of numerals beside it. -/
theorem the_terminal_read_is_three_of_six_and_exhausted_is_not_one :
    (Phase.all.filter Phase.terminal).map Phase.tag = [2, 4, 5] ∧
      Phase.terminal .exhausted = false ∧
      Phase.all.all (fun phase => terminalPhase (Phase.tag phase) = phase.terminal) = true := by
  native_decide

/-- A zeroed account claims `Primary`, so the phase byte alone cannot separate
an unwritten account from a live pre-resolution one and
`SOURCE_RESOLUTION_STATE_V2_MAGIC` is the whole partition -- the same shape the
occurrence-ticket state and the Direct root record, and the opposite of the
projected-custody ladder, which numbers from one on purpose. -/
theorem the_zero_tag_is_primary_so_the_magic_is_the_partition :
    Phase.ofTag? 0 = some .primary ∧ Phase.tag .primary = 0 := by
  native_decide


/-! ## The funded ordered-recovery ladder

`State.valid` says what a `Recovery` record may look like.  Until this section
nothing said how one is ENTERED.  The machine had three transitions -- resolve
the primary, exhaust the primary, commit the failure -- and not one of them moved
`activeAttempt`, so `Phase.recovery` was a phase the record could describe and no
route could reach.  The consequence was not cosmetic: a market founded with a
recovery policy had no terminal at all, because the primary exhaustion refuses a
recovery-bearing material on purpose (skipping paid-for legs would take an
outcome away from the holders who paid for them) and the failure commit only
fires from `Exhausted`.

The ladder is ONE transition, not a family.  A market's `RecoveryPolicyV2` names
an ordered list of funded attempts, each with its own absolute deadline; a
permissionless crank advances the active attempt when the current window closes
with nothing observed, and enters `Exhausted` when the LAST funded window closes.
`Exhausted` is where the existing failure commit already begins, so the ladder
adds a way in and changes no way out.

Each transition is split into a decidable GUARD and a total successor.  That is
not a stylistic choice: the guard is the thing the on-chain crank evaluates and
the thing every hostile below is aimed at, so it has to be a value a theorem can
name rather than a branch buried inside a partial function.
-/

/-- One market's whole ladder: the primary window's own deadline and the
finalized ordered policy that names what comes after it. -/
structure Ladder where
  /-- The primary window's closed deadline: `end_unix_seconds + max_age_seconds`. -/
  primaryDeadline : Nat
  /-- The market's finalized `RecoveryPolicyV2`, read rather than re-modelled. -/
  policy : SourceRecoveryPolicyV2Abi.Policy
  deriving Repr

/-- The two record bytes a crank moves.  Every other field of `State` is
untouched by the ladder, which is why the transitions are stated over the pair
and the record's canonicity rule stays exactly where it already is. -/
structure Rung where
  phase : Nat
  attempt : Nat
  deriving DecidableEq, Repr

namespace Ladder

/-- How many funded attempts the market bought. -/
def attemptCount (value : Ladder) : Nat := value.policy.attempts.length

/-- The absolute deadline of one funded attempt, or `none` past the end. -/
def deadline? (value : Ladder) (index : Nat) : Option Nat :=
  (value.policy.attempts[index]?).map (fun attempt => attempt.deadline)

/-- An attempt index is enterable exactly when the policy FUNDS it.  There is no
second notion of admissibility here: unfunded and unreachable are one word. -/
def funded (value : Ladder) (index : Nat) : Bool := decide (index < value.attemptCount)

/-- The current window has closed: strictly past the primary window's own
deadline while still on `Primary`, and strictly past the active attempt's own
deadline while on `Recovery`.  A rung with no funded attempt under it has no
window and therefore never closes. -/
def windowClosed (value : Ladder) (rung : Rung) (now : Nat) : Bool :=
  if rung.phase = Phase.tag .primary then decide (value.primaryDeadline < now)
  else if rung.phase = Phase.tag .recovery then
    match value.deadline? rung.attempt with
    | some due => decide (due < now)
    | none => false
  else false

/-- The attempt index a crank from this rung would enter. -/
def nextAttempt (rung : Rung) : Nat :=
  if rung.phase = Phase.tag .primary then 0 else rung.attempt + 1

/-- A ladder is well-formed when its policy is canonical and its first funded
attempt's deadline is strictly after the primary window's own, so no attempt is
already expired at the moment the primary one closes. -/
def valid (value : Ladder) : Bool :=
  value.policy.valid &&
    (match value.policy.attempts.head? with
     | some first => decide (value.primaryDeadline < first.deadline)
     | none => false)

/-- **The advance guard.**  The current window must have closed AND the attempt
being entered must be funded.  Both conjuncts are refusals a hostile can aim at:
the first is "advancing before the window's max_age", the second is "advancing
past an unfunded attempt". -/
def canAdvance (value : Ladder) (rung : Rung) (now : Nat) : Bool :=
  value.windowClosed rung now && value.funded (nextAttempt rung)

/-- **The exhaustion guard.**  The current window has closed and there is NO
funded attempt after it.  Exactly the complement of `canAdvance`'s second
conjunct, so from a closed window precisely one of the two fires. -/
def canExhaust (value : Ladder) (rung : Rung) (now : Nat) : Bool :=
  rung.phase = Phase.tag .recovery && value.windowClosed rung now &&
    !value.funded (nextAttempt rung)

/-- Advance to the next funded attempt: from `Primary` that is attempt `0`, from
`Recovery n` it is attempt `n+1`. -/
def advance? (value : Ladder) (rung : Rung) (now : Nat) : Option Rung :=
  if value.canAdvance rung now then some ⟨Phase.tag .recovery, nextAttempt rung⟩ else none

/-- Enter `Exhausted`: only from the last funded attempt, and only strictly after
that attempt's own deadline. -/
def exhaust? (value : Ladder) (rung : Rung) (now : Nat) : Option Rung :=
  if value.canExhaust rung now then some ⟨Phase.tag .exhausted, 0⟩ else none

/-- The honest capture: the ACTIVE attempt's own source was observed.  A capture
against any other attempt has no rung to land on, which is the whole content of
"the relay accepts the current attempt's source and no other". -/
def resolve? (value : Ladder) (rung : Rung) : Option Rung :=
  if rung.phase = Phase.tag .primary ||
      (rung.phase = Phase.tag .recovery && value.funded rung.attempt) then
    some ⟨Phase.tag .resolved, 0⟩
  else none

/-- Commit the Product-owned failure selector.  Only from `Exhausted`, and
unconditionally from it. -/
def commitFailure? (rung : Rung) : Option Rung :=
  if rung.phase = Phase.tag .exhausted then some ⟨Phase.tag .failureCommitted, 0⟩ else none

/-- The termination measure: how many advances the ladder has left. -/
def remaining (value : Ladder) (rung : Rung) : Nat :=
  if rung.phase = Phase.tag .primary then value.attemptCount + 1
  else if rung.phase = Phase.tag .recovery then value.attemptCount - rung.attempt
  else 0

theorem advance_eq_some (value : Ladder) (rung next : Rung) (now : Nat) :
    value.advance? rung now = some next ↔
      value.canAdvance rung now = true ∧ next = ⟨Phase.tag .recovery, nextAttempt rung⟩ := by
  unfold advance?
  by_cases guard : value.canAdvance rung now = true <;> simp [guard, eq_comm]

theorem exhaust_eq_some (value : Ladder) (rung next : Rung) (now : Nat) :
    value.exhaust? rung now = some next ↔
      value.canExhaust rung now = true ∧ next = ⟨Phase.tag .exhausted, 0⟩ := by
  unfold exhaust?
  by_cases guard : value.canExhaust rung now = true <;> simp [guard, eq_comm]

/-- A funded index is a real index into the policy, so its deadline exists. -/
theorem funded_iff_deadline (value : Ladder) (index : Nat) :
    value.funded index = true ↔ (value.deadline? index).isSome = true := by
  unfold funded deadline? attemptCount
  simp

/-- **Every attempt is funded before it is enterable.**  An advance lands on
`Recovery`, and on an index the policy actually funds -- so no crank can put a
market on a leg nothing paid for. -/
theorem every_entered_attempt_is_funded (value : Ladder) (rung next : Rung) (now : Nat)
    (step : value.advance? rung now = some next) :
    next.phase = Phase.tag .recovery ∧ next.attempt < value.attemptCount := by
  rw [advance_eq_some] at step
  obtain ⟨guard, shape⟩ := step
  subst shape
  refine ⟨rfl, ?_⟩
  have := (Bool.and_eq_true _ _).mp guard
  simpa [funded] using this.2

/-- A crank only ever leaves `Primary` or `Recovery`: from any other phase the
window has not closed and never will, so the guard is false whatever the clock
says.  Every case analysis below is therefore two cases and not six. -/
theorem advance_only_from_primary_or_recovery (value : Ladder) (rung : Rung) (now : Nat)
    (guard : value.canAdvance rung now = true) :
    rung.phase = Phase.tag .primary ∨ rung.phase = Phase.tag .recovery := by
  by_cases primary : rung.phase = Phase.tag .primary
  · exact Or.inl primary
  · by_cases recovery : rung.phase = Phase.tag .recovery
    · exact Or.inr recovery
    · simp [canAdvance, windowClosed, primary, recovery] at guard

/-- **The ladder is finite.**  Every advance strictly decreases a measure that
starts at `attemptCount + 1` and never rises, so no market can be cranked forever
and no crank revisits a rung it already left. -/
theorem the_ladder_is_finite (value : Ladder) (rung next : Rung) (now : Nat)
    (step : value.advance? rung now = some next) :
    value.remaining next < value.remaining rung ∧
      value.remaining rung ≤ value.attemptCount + 1 := by
  have entered := (every_entered_attempt_is_funded value rung next now step).2
  rw [advance_eq_some] at step
  obtain ⟨guard, shape⟩ := step
  have entry : value.remaining ⟨Phase.tag .recovery, nextAttempt rung⟩
      = value.attemptCount - nextAttempt rung := by
    simp [remaining, Phase.tag]
  subst shape
  rcases advance_only_from_primary_or_recovery value rung now guard with primary | recovery
  · have here : value.remaining rung = value.attemptCount + 1 := by simp [remaining, primary]
    have step0 : nextAttempt rung = 0 := by simp [nextAttempt, primary]
    rw [entry, here, step0]
    omega
  · have notPrimary : rung.phase ≠ Phase.tag .primary := by simp [recovery, Phase.tag]
    have here : value.remaining rung = value.attemptCount - rung.attempt := by
      simp [remaining, recovery, Phase.tag]
    have stepN : nextAttempt rung = rung.attempt + 1 := by simp [nextAttempt, notPrimary]
    rw [stepN] at entry
    simp only [stepN] at entered
    rw [stepN, entry, here]
    omega

/-- **`Exhausted` is reached only after the LAST funded window's own deadline.**
The rung the exhaustion leaves is a funded one, its own deadline has strictly
passed, and there is no attempt after it -- so the failure selector cannot be
committed while any paid-for leg still has time on it. -/
theorem exhausted_only_after_the_last_funded_deadline
    (value : Ladder) (rung next : Rung) (now : Nat)
    (step : value.exhaust? rung now = some next) :
    next = ⟨Phase.tag .exhausted, 0⟩ ∧ rung.phase = Phase.tag .recovery ∧
      ∃ due, value.deadline? rung.attempt = some due ∧ due < now ∧
        rung.attempt + 1 = value.attemptCount := by
  rw [exhaust_eq_some] at step
  obtain ⟨guard, shape⟩ := step
  unfold canExhaust at guard
  simp only [Bool.and_eq_true, Bool.not_eq_true', decide_eq_true_eq] at guard
  obtain ⟨⟨recovery, closed⟩, unfunded⟩ := guard
  have notPrimary : rung.phase ≠ Phase.tag .primary := by simp [recovery, Phase.tag]
  refine ⟨shape, recovery, ?_⟩
  unfold windowClosed at closed
  rw [if_neg notPrimary, if_pos recovery] at closed
  have absent : ¬ (rung.attempt + 1 < value.attemptCount) := by
    have := unfunded
    simp only [funded, nextAttempt, if_neg notPrimary, decide_eq_false_iff_not] at this
    exact this
  cases found : value.deadline? rung.attempt with
  | none => rw [found] at closed; simp at closed
  | some due =>
      rw [found] at closed
      have past : due < now := by simpa using closed
      have present : rung.attempt < value.attemptCount := by
        have := (funded_iff_deadline value rung.attempt).mpr (by rw [found]; rfl)
        simpa [funded] using this
      exact ⟨due, by simp, past, by omega⟩

/-- **Advancing before the window closes refuses**, and so does exhausting.  The
comparison is strict on both legs, so the last admissible second of an honest
observation and the first admissible second of a crank are different seconds. -/
theorem advancing_before_the_window_closes_refuses (value : Ladder) (rung : Rung) (now : Nat) :
    (rung.phase = Phase.tag .primary → now ≤ value.primaryDeadline →
      value.advance? rung now = none) ∧
    (∀ due, rung.phase = Phase.tag .recovery → value.deadline? rung.attempt = some due →
      now ≤ due → value.advance? rung now = none ∧ value.exhaust? rung now = none) := by
  have shut : ∀ rung', value.windowClosed rung' now = false →
      value.advance? rung' now = none ∧ value.exhaust? rung' now = none := by
    intro rung' closed
    constructor
    · simp [advance?, canAdvance, closed]
    · simp [exhaust?, canExhaust, closed]
  refine ⟨fun primary early => (shut rung ?_).1, fun due recovery found early => shut rung ?_⟩
  · simp [windowClosed, primary, Nat.not_lt.mpr early]
  · simp [windowClosed, recovery, Phase.tag, found, Nat.not_lt.mpr early]

/-- **Advancing past an unfunded attempt refuses.**  With the next index
unfunded the advance guard is false whatever the clock says, which is the
refusal that stops a crank from walking off the end of a paid-for ladder. -/
theorem advancing_past_an_unfunded_attempt_refuses (value : Ladder) (rung : Rung) (now : Nat)
    (unfunded : value.funded (nextAttempt rung) = false) :
    value.advance? rung now = none := by
  simp [advance?, canAdvance, unfunded]

/-- **A market that bought no attempt cannot enter the ladder.**  This is the
no-recovery market's guarantee stated positively: its terminal is the primary
exhaustion, and the crank has nothing to move. -/
theorem a_ladder_with_no_funded_attempt_cannot_be_entered
    (value : Ladder) (rung : Rung) (now : Nat) (empty : value.attemptCount = 0) :
    value.advance? rung now = none := by
  refine advancing_past_an_unfunded_attempt_refuses value rung now ?_
  simp [funded, empty]

/-- **Every rung the ladder enters fits the record.**  The advance's index is
funded, the policy's capacity is `maxRecoveryAttempts`, and that is the very
bound `State.valid` writes on the `activeAttempt` byte -- so the transition
system and the persisted layout cannot disagree about how wide a ladder is. -/
theorem every_entered_rung_fits_the_record (value : Ladder) (rung next : Rung) (now : Nat)
    (canon : value.valid = true) (step : value.advance? rung now = some next) :
    next.attempt < maxRecoveryAttempts := by
  have entered := (every_entered_attempt_is_funded value rung next now step).2
  have policy : value.policy.valid = true := by
    unfold valid at canon
    simpa using ((Bool.and_eq_true _ _).mp canon).1
  have capacity : value.policy.attempts.length ≤ SourceRecoveryPolicyV2Abi.maxAttempts := by
    unfold SourceRecoveryPolicyV2Abi.Policy.valid at policy
    simp only [Bool.and_eq_true, decide_eq_true_eq] at policy
    exact policy.1.1.2
  simpa [maxRecoveryAttempts, attemptCount] using Nat.lt_of_lt_of_le entered capacity

/-- **From a closed window, exactly one of advance and exhaust fires.**  There is
no rung at which a crank has two moves and none at which it has none, so the
ladder is a walk and not a choice, and no market can sit in `Recovery` with a
closed window and nothing to do. -/
theorem a_closed_recovery_window_has_exactly_one_move
    (value : Ladder) (rung : Rung) (now : Nat)
    (recovery : rung.phase = Phase.tag .recovery)
    (closed : value.windowClosed rung now = true) :
    (value.canAdvance rung now = true) ≠ (value.canExhaust rung now = true) := by
  unfold canAdvance canExhaust
  simp only [closed, recovery, Bool.true_and, Bool.and_true, decide_true,
    Bool.not_eq_true', ne_eq, eq_iff_iff]
  cases value.funded (nextAttempt rung) <;> simp

end Ladder

/-- **The ladder has exactly two ends, and `Exhausted` is neither.**  `Resolved`
and `FailureCommitted` are distinct phases and both are a terminal read, so a
market reaches at most one; `Exhausted` is not a terminal read at all, and the
failure commit is unconditionally enabled from it, so a run that reaches it has
exactly one move left and cannot stop there. -/
theorem the_ladder_has_exactly_two_ends_and_exhausted_is_neither :
    Phase.terminal .resolved = true ∧ Phase.terminal .failureCommitted = true ∧
      Phase.tag .resolved ≠ Phase.tag .failureCommitted ∧
      Phase.terminal .exhausted = false ∧
      Ladder.commitFailure? ⟨Phase.tag .exhausted, 0⟩ =
        some ⟨Phase.tag .failureCommitted, 0⟩ := by
  native_decide

/-- No crank of the ladder itself lands on a terminal read.  Only the capture and
the failure commit do, which is what makes the two-ends theorem exhaustive rather
than a claim about two phases picked out of six. -/
theorem a_crank_never_lands_on_a_terminal_read
    (value : Ladder) (rung next : Rung) (now : Nat) :
    (value.advance? rung now = some next → terminalPhase next.phase = false) ∧
    (value.exhaust? rung now = some next → terminalPhase next.phase = false) := by
  constructor
  · intro step
    have shape := (Ladder.every_entered_attempt_is_funded value rung next now step).1
    simp [terminalPhase, shape, Phase.tag, Phase.ofTag?, Phase.all, Phase.terminal]
  · intro step
    have shape := (Ladder.exhausted_only_after_the_last_funded_deadline value rung next now step).1
    subst shape
    simp [terminalPhase, Phase.tag, Phase.ofTag?, Phase.all, Phase.terminal]

/-- The two-source market the recovery campaign founds: a primary window closing
at `100` and exactly one funded alternative closing at `200`.  Every step of the
on-chain walk is decided here first -- the crank refuses at the primary deadline
and fires one second later, refuses to exhaust while the alternative still has
time, exhausts one second past it, and the failure commit follows; and the honest
branch, the alternative observed inside its own window, resolves instead. -/
def campaignLadder : Ladder where
  primaryDeadline := 100
  policy := {
    capacityProfile := 1
    attempts := [{ sourceSpec := 2, providerRelease := 3, deadline := 200, fundingAllocation := 4 }]
  }

theorem the_two_source_campaign_walks_the_whole_ladder :
    campaignLadder.valid = true ∧
      campaignLadder.advance? ⟨Phase.tag .primary, 0⟩ 100 = none ∧
      campaignLadder.advance? ⟨Phase.tag .primary, 0⟩ 101 =
        some ⟨Phase.tag .recovery, 0⟩ ∧
      campaignLadder.advance? ⟨Phase.tag .recovery, 0⟩ 201 = none ∧
      campaignLadder.exhaust? ⟨Phase.tag .recovery, 0⟩ 200 = none ∧
      campaignLadder.exhaust? ⟨Phase.tag .recovery, 0⟩ 201 =
        some ⟨Phase.tag .exhausted, 0⟩ ∧
      Ladder.commitFailure? ⟨Phase.tag .exhausted, 0⟩ =
        some ⟨Phase.tag .failureCommitted, 0⟩ ∧
      campaignLadder.resolve? ⟨Phase.tag .recovery, 0⟩ =
        some ⟨Phase.tag .resolved, 0⟩ := by
  native_decide

/-- Every rung of that walk is a byte image the record itself admits.  Without
this the transition system could be internally consistent and still emit a state
the decoder refuses. -/
theorem every_rung_of_the_campaign_is_a_valid_record :
    ([(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)] : List (Nat × Nat)).all
      (fun rung =>
        { freshExample with
            phase := rung.1
            activeAttempt := rung.2
            terminalRoute := if rung.1 = 2 then 2 else if rung.1 = 4 then 3 else 0
            resolutionEvidence := if rung.1 = 2 || rung.1 = 4 then 4 else 0
            terminalSequence := if rung.1 = 2 || rung.1 = 4 then 1 else 0
            resolvedAt := if rung.1 = 2 || rung.1 = 4 then 300 else 0 : State }.valid) := by
  native_decide

end DClutch.SourceResolutionStateV2Abi
