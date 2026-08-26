import DClutchSemantics.SeriesOccurrenceV3
import Std.Tactic

/-!
# Series V3 mutable replay semantics

The immutable Template/Occurrence/Ticket records select economic facts.  This
module owns only the total mutable cursor machine evaluated by the Trading
interpreter.  In particular, `currentTicketPrepared` prevents two prepared
Tickets from targeting the same cursor occurrence: one such duplicate would
become un-expirable after the first Ticket advanced the cursor and would make
terminal root closure impossible.
-/

namespace DClutch.SeriesReplayV3

/-- Root lifecycle phase. -/
inductive Phase where
  | active
  | terminal
  deriving DecidableEq, Repr

/-- Exact mutable Series root tail interpreted from hostile bytes. -/
structure State where
  phase : Phase
  currentTicketPrepared : Bool
  nextOccurrence : Nat
  outstandingTickets : Nat
  revision : Nat
  closeRentRemaining : Nat
  deriving DecidableEq, Repr

/-- One successful Series transition candidate.  Generic Trading remains the
sole physical account writer. -/
structure Candidate where
  state : State
  deriving DecidableEq, Repr

def initial (closeRent : Nat) : State := {
  phase := .active
  currentTicketPrepared := false
  nextOccurrence := 0
  outstandingTickets := 0
  revision := 0
  closeRentRemaining := closeRent
}

/-- A root is canonical for a finite nonempty Template. -/
def State.valid (occurrenceCount : Nat) (state : State) : Bool :=
  0 < occurrenceCount && state.nextOccurrence ≤ occurrenceCount &&
  (!state.currentTicketPrepared || 0 < state.outstandingTickets) &&
  match state.phase with
  | .active => state.nextOccurrence < occurrenceCount
  | .terminal =>
      state.nextOccurrence = occurrenceCount && !state.currentTicketPrepared

/-- Prepare the unique Ticket for the current cursor occurrence. -/
def prepare (state : State) (expectedRevision : Nat) : Option Candidate :=
  if state.phase = .active && !state.currentTicketPrepared &&
      state.revision = expectedRevision then
    some { state := {
      state with
      currentTicketPrepared := true
      outstandingTickets := state.outstandingTickets + 1
      revision := state.revision + 1
    } }
  else none

/-- Consume or expire the prepared Ticket and advance exactly one occurrence. -/
def settle
    (occurrenceCount expectedRevision : Nat) (state : State) : Option Candidate :=
  let next := state.nextOccurrence + 1
  if state.phase = .active && state.currentTicketPrepared &&
      state.revision = expectedRevision && next ≤ occurrenceCount then
    some { state := {
      state with
      phase := if next = occurrenceCount then .terminal else .active
      currentTicketPrepared := false
      nextOccurrence := next
      revision := state.revision + 1
    } }
  else none

/-- Delete one already-terminal Ticket replay account. -/
def retire (state : State) (expectedRevision : Nat) : Option Candidate :=
  if state.revision = expectedRevision && 0 < state.outstandingTickets then
    some { state := {
      state with
      outstandingTickets := state.outstandingTickets - 1
      revision := state.revision + 1
    } }
  else none

/-- Admit root deletion only after schedule completion and every Ticket close. -/
def admitsClose (state : State) (expectedRevision : Nat) : Bool :=
  state.phase = .terminal && !state.currentTicketPrepared &&
  state.outstandingTickets = 0 && state.revision = expectedRevision

theorem prepared_current_refuses_duplicate
    (state : State) (expectedRevision : Nat)
    (prepared : state.currentTicketPrepared = true) :
    prepare state expectedRevision = none := by
  simp [prepare, prepared]

theorem successful_prepare_marks_unique_current
    (before after : State) (expectedRevision : Nat)
    (accepted : prepare before expectedRevision = some { state := after }) :
    after.currentTicketPrepared = true ∧
    after.outstandingTickets = before.outstandingTickets + 1 := by
  simp [prepare] at accepted
  rcases accepted with ⟨_, h⟩
  subst after
  simp

theorem successful_settlement_clears_current
    (before after : State) (occurrenceCount expectedRevision : Nat)
    (accepted : settle occurrenceCount expectedRevision before = some { state := after }) :
    after.currentTicketPrepared = false ∧
    after.nextOccurrence = before.nextOccurrence + 1 := by
  simp [settle] at accepted
  rcases accepted with ⟨_, h⟩
  subst after
  simp

theorem close_refuses_live_ticket
    (state : State) (expectedRevision : Nat)
    (live : 0 < state.outstandingTickets) :
    admitsClose state expectedRevision = false := by
  simp [admitsClose]
  omega

def hostileInitial : State := initial 7

def hostilePrepared : State := {
  phase := .active
  currentTicketPrepared := true
  nextOccurrence := 0
  outstandingTickets := 1
  revision := 1
  closeRentRemaining := 7
}

theorem hostile_duplicate_prepare_refuses :
    prepare hostilePrepared 1 = none := by native_decide

theorem hostile_close_with_terminal_ticket_refuses :
    admitsClose {
      phase := .terminal
      currentTicketPrepared := false
      nextOccurrence := 1
      outstandingTickets := 1
      revision := 2
      closeRentRemaining := 7
    } 2 = false := by native_decide

end DClutch.SeriesReplayV3
