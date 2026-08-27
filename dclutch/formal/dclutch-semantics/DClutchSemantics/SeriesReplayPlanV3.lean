import DClutchSemantics.SeriesReplayV3
import Std.Tactic

/-!
# Series V3 joint replay planning

This is the stateless joint root/Ticket evaluator exposed to the generic
Trading interpreter.  Its result is candidate data only: it grants no account
write, deletion, value movement, or child-program authority.
-/

namespace DClutch.SeriesReplayPlanV3

open DClutch.SeriesReplayV3

/-- Mutable phase of one occurrence Ticket. -/
inductive TicketPhase where
  | prepared
  | consumed
  | expired
  deriving DecidableEq, Repr

/-- The Ticket record owns immutable facts; mutable replay retains only its
identity, phase, and revision. -/
structure TicketState where
  phase : TicketPhase
  revision : Nat
  ticketRecord : Nat
  deriving DecidableEq, Repr

/-- Exact semantic action after immutable content admission. -/
inductive Action where
  | prepare (ticketRecord : Nat)
  | consume (ticketRecord expectedTicketRevision : Nat)
  | expire (ticketRecord expectedTicketRevision : Nat)
  | retire (ticketRecord expectedTicketRevision : Nat)
  | close
  deriving DecidableEq, Repr

/-- Joint candidate state. `none` means deletion for an account selected by the
action, and absence for an account not selected by the action. The action makes
that distinction canonical. -/
structure Witness where
  root : Option State
  ticket : Option TicketState
  deriving DecidableEq, Repr

def TicketState.terminal (ticket : TicketState) : Bool :=
  ticket.phase != .prepared

def TicketState.settle
    (ticket : TicketState) (record expectedRevision : Nat)
    (terminal : TicketPhase) : Option TicketState :=
  if ticket.phase = .prepared && ticket.ticketRecord = record &&
      ticket.revision = expectedRevision && terminal != .prepared then
    some { ticket with phase := terminal, revision := ticket.revision + 1 }
  else none

/-- Evaluate both replay accounts before returning either candidate. -/
def evaluate
    (occurrenceCount expectedSeriesRevision : Nat)
    (action : Action) (root : State) (ticket : Option TicketState) : Option Witness :=
  match action with
  | .prepare record =>
      match ticket, SeriesReplayV3.prepare root expectedSeriesRevision with
      | none, some candidate => some {
          root := some candidate.state
          ticket := some { phase := .prepared, revision := 0, ticketRecord := record }
        }
      | _, _ => none
  | .consume record expectedTicketRevision =>
      match ticket,
          SeriesReplayV3.settle occurrenceCount expectedSeriesRevision root with
      | some before, some rootCandidate =>
          match before.settle record expectedTicketRevision .consumed with
          | some ticketCandidate => some {
              root := some rootCandidate.state
              ticket := some ticketCandidate
            }
          | none => none
      | _, _ => none
  | .expire record expectedTicketRevision =>
      match ticket,
          SeriesReplayV3.settle occurrenceCount expectedSeriesRevision root with
      | some before, some rootCandidate =>
          match before.settle record expectedTicketRevision .expired with
          | some ticketCandidate => some {
              root := some rootCandidate.state
              ticket := some ticketCandidate
            }
          | none => none
      | _, _ => none
  | .retire record expectedTicketRevision =>
      match ticket, SeriesReplayV3.retire root expectedSeriesRevision with
      | some before, some rootCandidate =>
          if before.ticketRecord = record && before.revision = expectedTicketRevision &&
              before.terminal then
            some { root := some rootCandidate.state, ticket := none }
          else none
      | _, _ => none
  | .close =>
      match ticket with
      | some _ => none
      | none =>
          if SeriesReplayV3.admitsClose root expectedSeriesRevision then
            some { root := none, ticket := none }
          else none

theorem close_refuses_extraneous_ticket
    (occurrenceCount expectedSeriesRevision : Nat)
    (root : State) (ticket : TicketState) :
    evaluate occurrenceCount expectedSeriesRevision .close root (some ticket) = none := by
  simp [evaluate]

private def preparedTicket (record revision : Nat) : TicketState := {
  phase := .prepared
  revision := revision
  ticketRecord := record
}

theorem retire_refuses_prepared_ticket
    (occurrenceCount expectedSeriesRevision record expectedTicketRevision : Nat)
    (root : State) :
    evaluate occurrenceCount expectedSeriesRevision
      (.retire record expectedTicketRevision) root
      (some (preparedTicket record expectedTicketRevision)) = none := by
  cases result : SeriesReplayV3.retire root expectedSeriesRevision <;>
    simp [evaluate, result, preparedTicket, TicketState.terminal]

theorem successful_ticket_settle_is_exact
    (before after : TicketState) (record expectedRevision : Nat)
    (terminal : TicketPhase)
    (accepted : before.settle record expectedRevision terminal = some after) :
    after.phase = terminal ∧ after.ticketRecord = record := by
  simp [TicketState.settle] at accepted
  rcases accepted with ⟨_, _, _, _, equality⟩
  simp_all

theorem successful_consume_is_joint
    (occurrenceCount expectedSeriesRevision record expectedTicketRevision : Nat)
    (beforeRoot afterRoot : State) (beforeTicket afterTicket : TicketState)
    (accepted : evaluate occurrenceCount expectedSeriesRevision
      (.consume record expectedTicketRevision) beforeRoot (some beforeTicket) =
      some { root := some afterRoot, ticket := some afterTicket }) :
    afterRoot.currentTicketPrepared = false ∧
    afterTicket.phase = .consumed ∧
    afterTicket.ticketRecord = record := by
  cases rootResult : SeriesReplayV3.settle occurrenceCount expectedSeriesRevision beforeRoot with
  | none => simp [evaluate, rootResult] at accepted
  | some rootCandidate =>
      cases ticketResult : beforeTicket.settle record expectedTicketRevision .consumed with
      | none => simp [evaluate, rootResult, ticketResult] at accepted
      | some ticketCandidate =>
          have rootResult' :
              SeriesReplayV3.settle occurrenceCount expectedSeriesRevision beforeRoot =
                some { state := rootCandidate.state } := by
            simpa using rootResult
          have rootProperties := SeriesReplayV3.successful_settlement_clears_current
            beforeRoot rootCandidate.state occurrenceCount expectedSeriesRevision rootResult'
          have ticketProperties := successful_ticket_settle_is_exact
            beforeTicket ticketCandidate record expectedTicketRevision .consumed ticketResult
          simp [evaluate, rootResult, ticketResult] at accepted
          rcases accepted with ⟨rootEquality, ticketEquality⟩
          subst afterRoot
          subst afterTicket
          exact ⟨rootProperties.1, ticketProperties.1, ticketProperties.2⟩

def hostileRoot : State := {
  phase := .active
  currentTicketPrepared := true
  nextOccurrence := 0
  outstandingTickets := 1
  revision := 1
  closeRentRemaining := 7
}

def hostilePreparedTicket : TicketState := {
  phase := .prepared
  revision := 0
  ticketRecord := 9
}

theorem hostile_substituted_ticket_refuses :
    evaluate 1 1 (.consume 10 0) hostileRoot (some hostilePreparedTicket) = none := by
  native_decide

theorem hostile_partial_close_refuses :
    evaluate 1 1 .close hostileRoot (some hostilePreparedTicket) = none := by
  native_decide

end DClutch.SeriesReplayPlanV3
