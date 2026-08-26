import Std.Tactic

/-!
# Series V3 pre-founding collateral semantics

Series emits a stateless effect sequence. Custody alone owns physical token
accounts, replay persistence, authority, and receipts.
-/

namespace DClutch.SeriesEscrowV3

inductive Effect where
  | initializeReplay
  | lock
  | consumeIntoHoard
  | refundExpired
  deriving DecidableEq, Repr

def expectedRevision : Effect → Nat
  | .initializeReplay => 0
  | .lock => 1
  | .consumeIntoHoard | .refundExpired => 2

def resultingRevision (effect : Effect) : Nat := expectedRevision effect + 1

def movesCollateral : Effect → Bool
  | .initializeReplay => false
  | .lock | .consumeIntoHoard | .refundExpired => true

def terminal : Effect → Bool
  | .consumeIntoHoard | .refundExpired => true
  | .initializeReplay | .lock => false

/-- A Custody replay accepts only the exact next Series semantic edge. -/
def accepts (currentRevision : Nat) (alreadyTerminal : Bool) (effect : Effect) : Bool :=
  !alreadyTerminal && currentRevision = expectedRevision effect

theorem prepare_revision_chain :
    resultingRevision .initializeReplay = expectedRevision .lock := by
  decide

theorem consume_and_refund_share_exact_prestate :
    expectedRevision .consumeIntoHoard = expectedRevision .refundExpired := by
  decide

theorem successful_terminal_refuses_second_terminal (effect next : Effect)
    (terminalEffect : terminal effect = true)
    (nextTerminal : terminal next = true) :
    accepts (resultingRevision effect) false next = false := by
  cases effect <;> cases next <;>
    simp_all [terminal, accepts, resultingRevision, expectedRevision]

theorem hoard_and_refund_are_distinct_effects :
    Effect.consumeIntoHoard ≠ Effect.refundExpired := by
  decide

theorem hostile_skip_lock_refuses : accepts 1 false .consumeIntoHoard = false := by
  native_decide

theorem hostile_replay_after_terminal_refuses : accepts 2 true .refundExpired = false := by
  native_decide

end DClutch.SeriesEscrowV3
