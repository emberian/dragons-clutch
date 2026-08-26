import Std.Tactic

/-!
# Series V3 pre-founding collateral semantics

Series emits a stateless effect sequence. Custody alone owns physical token
accounts, replay persistence, authority, and receipts.
-/

namespace DClutch.SeriesEscrowV3

inductive Effect where
  | initializeReplay
  | openEscrowVault
  | lock
  | consumeIntoHoard
  | refundExpired
  | closeEscrowVault
  | closeReplay
  deriving DecidableEq, Repr

def expectedRevision : Effect → Nat
  | .initializeReplay => 0
  | .openEscrowVault => 1
  | .lock => 2
  | .consumeIntoHoard | .refundExpired => 3
  | .closeEscrowVault => 4
  | .closeReplay => 5

def resultingRevision (effect : Effect) : Nat := expectedRevision effect + 1

def movesCollateral : Effect → Bool
  | .initializeReplay | .openEscrowVault | .closeEscrowVault | .closeReplay => false
  | .lock | .consumeIntoHoard | .refundExpired => true

def terminalTransfer : Effect → Bool
  | .consumeIntoHoard | .refundExpired => true
  | .initializeReplay | .openEscrowVault | .lock | .closeEscrowVault | .closeReplay => false

/-- A Custody replay accepts only the exact next Series semantic edge. -/
def accepts (currentRevision : Nat) (effect : Effect) : Bool :=
  currentRevision = expectedRevision effect

theorem prepare_revision_chain :
    resultingRevision .initializeReplay = expectedRevision .openEscrowVault ∧
    resultingRevision .openEscrowVault = expectedRevision .lock := by
  decide

theorem lock_enables_terminal_edges :
    resultingRevision .lock = expectedRevision .consumeIntoHoard ∧
    resultingRevision .lock = expectedRevision .refundExpired := by
  decide

theorem terminal_cleanup_chain (effect : Effect)
    (terminalEffect : terminalTransfer effect = true) :
    resultingRevision effect = expectedRevision .closeEscrowVault ∧
    resultingRevision .closeEscrowVault = expectedRevision .closeReplay := by
  cases effect <;> simp_all [terminalTransfer, resultingRevision, expectedRevision]

theorem consume_and_refund_share_exact_prestate :
    expectedRevision .consumeIntoHoard = expectedRevision .refundExpired := by
  decide

theorem successful_terminal_refuses_second_terminal (effect next : Effect)
    (terminalEffect : terminalTransfer effect = true)
    (nextTerminal : terminalTransfer next = true) :
    accepts (resultingRevision effect) next = false := by
  cases effect <;> cases next <;>
    simp_all [terminalTransfer, accepts, resultingRevision, expectedRevision]

theorem hoard_and_refund_are_distinct_effects :
    Effect.consumeIntoHoard ≠ Effect.refundExpired := by
  decide

theorem hostile_skip_open_refuses : accepts 1 .lock = false := by
  native_decide

theorem hostile_skip_lock_refuses : accepts 2 .consumeIntoHoard = false := by
  native_decide

theorem hostile_second_terminal_refuses :
    accepts (resultingRevision .consumeIntoHoard) .refundExpired = false := by
  native_decide

theorem hostile_cleanup_before_terminal_refuses : accepts 3 .closeEscrowVault = false := by
  native_decide

theorem completed_cleanup_has_revision_six : resultingRevision .closeReplay = 6 := by
  decide

end DClutch.SeriesEscrowV3
