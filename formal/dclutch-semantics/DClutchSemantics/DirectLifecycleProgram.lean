import DClutchSemantics.DirectLifecycle
import DClutchSemantics.TransitionVM

/-!
# Registered Direct residual program

This compact program derives the only post-fill registration facts: remaining
quantity, next registration-local replay sequence, and phase.  It uses generic
checked subtraction and conditional-selection opcodes rather than an
action-specific Rust state transition.
-/

namespace DClutch.DirectLifecycleProgram

open DClutch.Direct DClutch.DirectLifecycle DClutch.TransitionVM

/-- Typed register schema for residual consumption. -/
inductive ScalarSlot where
  | lifecycle
  | remaining
  | fill
  | sequence
  | zero
  | one
  | goodTillCancelled
  | remainingOutput
  | sequenceOutput
  | phaseOutput
  deriving DecidableEq, Repr

namespace ScalarSlot

def all : List ScalarSlot := [
  .lifecycle, .remaining, .fill, .sequence, .zero, .one,
  .goodTillCancelled, .remainingOutput, .sequenceOutput, .phaseOutput
]

def inputs : List ScalarSlot := [.lifecycle, .remaining, .fill, .sequence]

def runtimeRegisters : List ScalarSlot := inputs ++ [
  .remainingOutput, .sequenceOutput, .phaseOutput
]

@[simp] def index : ScalarSlot → Nat
  | .lifecycle => 0
  | .remaining => 1
  | .fill => 2
  | .sequence => 3
  | .zero => 4
  | .one => 5
  | .goodTillCancelled => 6
  | .remainingOutput => 7
  | .sequenceOutput => 8
  | .phaseOutput => 9

def rustName : ScalarSlot → String
  | .lifecycle => "REGISTERED_LIFECYCLE"
  | .remaining => "REGISTERED_REMAINING"
  | .fill => "REGISTERED_FILL"
  | .sequence => "REGISTERED_SEQUENCE"
  | .zero => "REGISTERED_ZERO"
  | .one => "REGISTERED_ONE"
  | .goodTillCancelled => "REGISTERED_GOOD_TILL_CANCELLED"
  | .remainingOutput => "REGISTERED_REMAINING_OUTPUT"
  | .sequenceOutput => "REGISTERED_SEQUENCE_OUTPUT"
  | .phaseOutput => "REGISTERED_PHASE_OUTPUT"

def rustFieldName : ScalarSlot → String
  | .lifecycle => "lifecycle"
  | .remaining => "remaining"
  | .fill => "fill"
  | .sequence => "sequence"
  | .zero => "zero"
  | .one => "one"
  | .goodTillCancelled => "good_till_cancelled"
  | .remainingOutput => "remaining_output"
  | .sequenceOutput => "sequence_output"
  | .phaseOutput => "phase_output"

theorem indices_are_canonical :
    all.map index = List.range all.length := by
  native_decide

theorem rust_names_are_unique : (all.map rustName).Nodup := by
  native_decide

theorem inputs_are_canonical_prefix : all.take inputs.length = inputs := by
  native_decide

theorem runtime_registers_are_unique : runtimeRegisters.Nodup := by
  native_decide

end ScalarSlot

namespace Scalar

def lifecycle := ScalarSlot.index .lifecycle
def remaining := ScalarSlot.index .remaining
def fill := ScalarSlot.index .fill
def sequence := ScalarSlot.index .sequence
def zero := ScalarSlot.index .zero
def one := ScalarSlot.index .one
def goodTillCancelled := ScalarSlot.index .goodTillCancelled
def remainingOutput := ScalarSlot.index .remainingOutput
def sequenceOutput := ScalarSlot.index .sequenceOutput
def phaseOutput := ScalarSlot.index .phaseOutput
def count := ScalarSlot.all.length

end Scalar

def phaseTag : DirectLifecycle.Phase → Nat
  | .open => 0
  | .filled => 1
  | .cancelled => 2
  | .expired => 3

def lifecycleTag : Lifecycle → Nat
  | .fillOrKill => 0
  | .immediateOrCancel => 1
  | .goodTillCancelled => 2

private def registerState
    (state : DirectLifecycle.State) (fill zeroValue one goodTillCancelled
      remainingOutput sequenceOutput phaseOutput : Nat) : TransitionVM.State := {
  scalars := #[
    lifecycleTag state.terms.lifecycle,
    state.remaining,
    fill,
    state.sequence,
    zeroValue,
    one,
    goodTillCancelled,
    remainingOutput,
    sequenceOutput,
    phaseOutput
  ]
  identities := #[]
}

def state (registration : DirectLifecycle.State) (fill : Nat) : TransitionVM.State :=
  registerState registration fill 0 0 0 0 0 0

/-- Width-independent registered-residual state transition. -/
def program : List Op := [
  .loadConst Scalar.zero 0,
  .loadConst Scalar.one 1,
  .loadConst Scalar.goodTillCancelled 2,
  .loadConst Scalar.phaseOutput 2,
  .nonzero Scalar.fill,
  .lifecycleAccepts Scalar.lifecycle Scalar.remaining Scalar.fill,
  .incrementInto Scalar.sequence Scalar.sequenceOutput,
  .subInto Scalar.remaining Scalar.fill Scalar.remainingOutput,
  .selectEq Scalar.lifecycle Scalar.goodTillCancelled Scalar.zero Scalar.phaseOutput,
  .selectZero Scalar.remainingOutput Scalar.one Scalar.phaseOutput
]

def outputs (result : TransitionVM.State) : Option (Nat × Nat × Nat) := do
  some (← scalar result Scalar.remainingOutput,
    ← scalar result Scalar.sequenceOutput,
    ← scalar result Scalar.phaseOutput)

theorem program_length : program.length = 10 := by
  rfl

theorem encoded_program_length :
    (TransitionVM.Codec.encodeProgram program).length = 168 := by
  native_decide

/-- Every semantically admitted residual transition is computed exactly by the
generic VM program. -/
theorem admitted_program_refines
    (registration : DirectLifecycle.State) (fill : Nat)
    (positive : 0 < fill)
    (accepted : registration.terms.lifecycle.accepts registration.remaining fill)
    (sequenceFits : registration.sequence + 1 < u64Limit) :
    (run program (state registration fill)).bind outputs =
      some (registration.remaining - fill,
        registration.sequence + 1,
        phaseTag (phaseAfterFill registration fill)) := by
  have nonzero : fill ≠ 0 := Nat.ne_of_gt positive
  cases policy : registration.terms.lifecycle with
  | fillOrKill =>
      simp [Lifecycle.accepts, policy] at accepted
      have remainingNonzero : registration.remaining ≠ 0 := by omega
      simp [program, state, registerState, run, step, scalar, setScalar, require,
        outputs, Scalar.zero, Scalar.one, Scalar.goodTillCancelled,
        Scalar.fill, Scalar.lifecycle, Scalar.remaining, Scalar.sequence,
        Scalar.remainingOutput, Scalar.sequenceOutput, Scalar.phaseOutput,
        lifecycleTag, policy, accepted, remainingNonzero, sequenceFits,
        phaseAfterFill, phaseTag]
  | immediateOrCancel =>
      simp [Lifecycle.accepts, policy] at accepted
      by_cases complete : registration.remaining = fill
      · simp [program, state, registerState, run, step, scalar, setScalar, require,
          outputs, Scalar.zero, Scalar.one, Scalar.goodTillCancelled,
          Scalar.fill, Scalar.lifecycle, Scalar.remaining, Scalar.sequence,
          Scalar.remainingOutput, Scalar.sequenceOutput, Scalar.phaseOutput,
          lifecycleTag, policy, nonzero, sequenceFits,
          phaseAfterFill, phaseTag, complete]
      · have residualNonzero : registration.remaining - fill ≠ 0 := by omega
        simp [program, state, registerState, run, step, scalar, setScalar, require,
          outputs, Scalar.zero, Scalar.one, Scalar.goodTillCancelled,
          Scalar.fill, Scalar.lifecycle, Scalar.remaining, Scalar.sequence,
          Scalar.remainingOutput, Scalar.sequenceOutput, Scalar.phaseOutput,
          lifecycleTag, policy, accepted, nonzero, sequenceFits,
          phaseAfterFill, phaseTag, complete, residualNonzero]
  | goodTillCancelled =>
      simp [Lifecycle.accepts, policy] at accepted
      by_cases complete : registration.remaining = fill
      · simp [program, state, registerState, run, step, scalar, setScalar, require,
          outputs, Scalar.zero, Scalar.one, Scalar.goodTillCancelled,
          Scalar.fill, Scalar.lifecycle, Scalar.remaining, Scalar.sequence,
          Scalar.remainingOutput, Scalar.sequenceOutput, Scalar.phaseOutput,
          lifecycleTag, policy, nonzero, sequenceFits,
          phaseAfterFill, phaseTag, complete]
      · have residualNonzero : registration.remaining - fill ≠ 0 := by omega
        simp [program, state, registerState, run, step, scalar, setScalar, require,
          outputs, Scalar.zero, Scalar.one, Scalar.goodTillCancelled,
          Scalar.fill, Scalar.lifecycle, Scalar.remaining, Scalar.sequence,
          Scalar.remainingOutput, Scalar.sequenceOutput, Scalar.phaseOutput,
          lifecycleTag, policy, accepted, nonzero, sequenceFits,
          phaseAfterFill, phaseTag, complete, residualNonzero]

private def exampleIntent : Intent := {
  market := 1
  generation := 0
  maker := 2
  nonce := 0
  validFromSlot := 10
  validThroughSlot := 20
  side := .sell
  lifecycle := .goodTillCancelled
  outcome := 1
  maxFill := 100
  limitPrice := 5000
  feeBasisPoints := 20
}

private def exampleState : DirectLifecycle.State := {
  terms := exampleIntent
  phase := .open
  remaining := 100
  sequence := 4
}

theorem example_partial_remains_open :
    (run program (state exampleState 35)).bind outputs = some (65, 5, 0) := by
  native_decide

theorem example_final_fill_closes :
    (run program (state exampleState 100)).bind outputs = some (0, 5, 1) := by
  native_decide

theorem example_overfill_refuses :
    run program (state exampleState 101) = none := by
  native_decide

end DClutch.DirectLifecycleProgram
