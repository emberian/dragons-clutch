import DClutchSemantics.Direct
import Std.Tactic

/-!
# Direct successor configuration and replay ownership

This module owns the state that the data-defined Direct capability actually
persists.  The immutable execution config selects the one positive quote scale,
fee rate, and fee recipient.  The capability root owns global admission and the
number of live maker replay roots; each maker root separately owns its gap-free
nonce, registered-live count, cancel-through high-water mark, and historical
rent principal/refund owner.

The ordinary 41/4 transition program is deliberately not imported.  It is one
inline ordinary execution strategy, not the complete Direct lifecycle.
-/

namespace DClutch.DirectSuccessor

open DClutch.Direct

/-- Immutable content-selected Direct economics. -/
structure ExecutionConfig where
  priceScale : Nat
  feeBasisPoints : Nat
  feeRecipient : Nat
  deriving DecidableEq, Repr

/-- Exact representability and economic validity of the immutable config. -/
def ExecutionConfig.Valid (config : ExecutionConfig) : Prop :=
  0 < config.priceScale ∧
  config.priceScale < u64Limit ∧
  config.feeBasisPoints ≤ feeDenominator ∧
  config.feeRecipient ≠ 0

instance (config : ExecutionConfig) : Decidable config.Valid := by
  unfold ExecutionConfig.Valid
  infer_instance

/-- The capability root stops all new maker nonce consumption before closure. -/
inductive RootPhase where
  | open
  | retiring
  deriving DecidableEq, Inhabited, Repr

/-- Sole global Direct state inside the composite Trading root tail. -/
structure Root where
  phase : RootPhase
  openMakerRootCount : Nat
  deriving DecidableEq, Inhabited, Repr

def Root.Valid (root : Root) : Prop := root.openMakerRootCount < u64Limit

instance (root : Root) : Decidable root.Valid := by
  unfold Root.Valid
  infer_instance

/-- One replay high-water owner for an exact Market/generation/maker tuple.

`feeOwed` is the sole record of one unsettled Direct fee receivable (the
FEE-TX2 obligation): nonzero exactly between the fill that owed it and the
settlement that pays it.  It rides the maker root because no other account
remembers the debt -- which is why `closeMaker` below must refuse while it is
nonzero: closing the account would erase a receivable with no residue. -/
structure MakerRoot where
  market : Nat
  generation : Nat
  maker : Nat
  nextNonce : Nat
  liveCount : Nat
  minimumLiveNonce : Nat
  rentOwner : Nat
  rentPrincipal : Nat
  feeOwed : Nat
  deriving DecidableEq, Inhabited, Repr

def MakerRoot.Valid (root : MakerRoot) : Prop :=
  root.market ≠ 0 ∧
  root.maker ≠ 0 ∧
  root.rentOwner ≠ 0 ∧
  0 < root.rentPrincipal ∧
  root.nextNonce < u64Limit ∧
  root.liveCount ≤ root.nextNonce ∧
  root.minimumLiveNonce ≤ root.nextNonce

instance (root : MakerRoot) : Decidable root.Valid := by
  unfold MakerRoot.Valid
  infer_instance

/-- Signature-authenticated replay coordinates of one maker intent. -/
structure IntentCoordinate where
  market : Nat
  generation : Nat
  maker : Nat
  nonce : Nat
  deriving DecidableEq, Inhabited, Repr

/-- Authenticated first-use funding observation for a vacant canonical PDA. -/
structure FirstUse where
  rentOwner : Nat
  rentPrincipal : Nat
  observedLamports : Nat
  deriving DecidableEq, Inhabited, Repr

/-- Exact dust-tolerant account-creation output. -/
structure CreationPlan where
  topUpLamports : Nat
  postLamports : Nat
  deriving DecidableEq, Inhabited, Repr

def creationPlan (first : FirstUse) : CreationPlan := {
  topUpLamports := first.rentPrincipal - first.observedLamports
  postLamports := max first.rentPrincipal first.observedLamports
}

theorem creation_plan_exact (first : FirstUse) :
    (creationPlan first).postLamports =
      first.observedLamports + (creationPlan first).topUpLamports := by
  simp [creationPlan]
  omega

theorem creation_plan_rent_funded (first : FirstUse) :
    first.rentPrincipal ≤ (creationPlan first).postLamports := by
  exact Nat.le_max_left _ _

/-- Whether consuming one maker nonce creates a live registered intent. -/
inductive Consumption where
  | inline
  | register
  deriving DecidableEq, Inhabited, Repr

/-- Candidate result of one nonce consumption.  A creation plan exists exactly
when the canonical maker root was absent. -/
structure ConsumeResult where
  root : Root
  makerRoot : MakerRoot
  creation : Option CreationPlan
  deriving DecidableEq, Inhabited, Repr

private def sameCoordinate (root : MakerRoot) (intent : IntentCoordinate) : Prop :=
  root.market = intent.market ∧
  root.generation = intent.generation ∧
  root.maker = intent.maker

private instance (root : MakerRoot) (intent : IntentCoordinate) :
    Decidable (sameCoordinate root intent) := by
  unfold sameCoordinate
  infer_instance

private def advanceMaker
    (makerRoot : MakerRoot)
    (consumption : Consumption) : MakerRoot := {
  makerRoot with
  nextNonce := makerRoot.nextNonce + 1
  liveCount := match consumption with
    | .inline => makerRoot.liveCount
    | .register => makerRoot.liveCount + 1
}

/-- Total atomic nonce consumption over an existing or first-use maker root.

Physical vacancy/PDA/System-owner checks precede construction of `none`; this
semantic function owns only the resulting state and exact dust top-up. -/
def consumeNonce
    (root : Root) (makerRoot : Option MakerRoot)
    (intent : IntentCoordinate) (consumption : Consumption)
    (firstUse : Option FirstUse) : Option ConsumeResult := do
  if root.Valid ∧ root.phase = .open then
  match makerRoot, firstUse with
  | none, some first =>
      if intent.market ≠ 0 ∧ intent.maker ≠ 0 ∧ intent.nonce = 0 ∧
          first.rentOwner ≠ 0 ∧ first.rentPrincipal ≠ 0 ∧
          root.openMakerRootCount + 1 < u64Limit then
        let initial : MakerRoot := {
          market := intent.market
          generation := intent.generation
          maker := intent.maker
          nextNonce := 0
          liveCount := 0
          minimumLiveNonce := 0
          rentOwner := first.rentOwner
          rentPrincipal := first.rentPrincipal
          feeOwed := 0
        }
        some {
          root := { root with openMakerRootCount := root.openMakerRootCount + 1 }
          makerRoot := advanceMaker initial consumption
          creation := some (creationPlan first)
        }
      else none
  | some existing, none =>
      if existing.Valid ∧ sameCoordinate existing intent ∧
          intent.nonce = existing.nextNonce ∧
          existing.nextNonce + 1 < u64Limit ∧
          (consumption = .register → existing.liveCount + 1 < u64Limit) ∧
          existing.feeOwed = 0 then
        some {
          root
          makerRoot := advanceMaker existing consumption
          creation := none
        }
      else none
  | _, _ => none
  else none

theorem first_use_count_conserved
    (root : Root) (intent : IntentCoordinate) (consumption : Consumption)
    (first : FirstUse) (result : ConsumeResult)
    (success : consumeNonce root none intent consumption (some first) = some result) :
    result.root.openMakerRootCount = root.openMakerRootCount + 1 := by
  simp [consumeNonce] at success
  rcases success with ⟨_, _, _, _, _, _, _, rfl⟩
  rfl

theorem existing_count_conserved
    (root : Root) (makerRoot : MakerRoot) (intent : IntentCoordinate)
    (consumption : Consumption) (result : ConsumeResult)
    (success : consumeNonce root (some makerRoot) intent consumption none = some result) :
    result.root.openMakerRootCount = root.openMakerRootCount := by
  simp [consumeNonce] at success
  rcases success with ⟨_, _, _, _, _, _, rfl⟩
  rfl

theorem nonce_advances_once
    (root : Root) (makerRoot : Option MakerRoot) (intent : IntentCoordinate)
    (consumption : Consumption) (firstUse : Option FirstUse) (result : ConsumeResult)
    (success : consumeNonce root makerRoot intent consumption firstUse = some result) :
    result.makerRoot.nextNonce = intent.nonce + 1 := by
  cases makerRoot with
  | none =>
      cases firstUse with
      | none => simp [consumeNonce] at success
      | some first =>
          simp [consumeNonce, advanceMaker] at success
          rcases success with ⟨_, conditions, rfl⟩
          simpa using conditions.2.2.1
  | some existing =>
      cases firstUse with
      | none =>
          simp [consumeNonce, advanceMaker] at success
          rcases success with ⟨_, conditions, rfl⟩
          simpa using conditions.2.2.1.symm
      | some _ => simp [consumeNonce] at success

theorem exact_replay_refuses
    (root : Root) (makerRoot : MakerRoot) (intent : IntentCoordinate)
    (consumption : Consumption)
    (validRoot : root.Valid) (openRoot : root.phase = .open)
    (validMaker : makerRoot.Valid) (coordinate : sameCoordinate makerRoot intent)
    (stale : intent.nonce < makerRoot.nextNonce) :
    consumeNonce root (some makerRoot) intent consumption none = none := by
  have nonceMismatch : intent.nonce ≠ makerRoot.nextNonce := Nat.ne_of_lt stale
  simp [consumeNonce, validRoot, openRoot, validMaker, coordinate, nonceMismatch]

/-- Close one registered live intent after its record/custody resources close. -/
def closeLive (makerRoot : MakerRoot) : Option MakerRoot :=
  if ¬makerRoot.Valid ∨ makerRoot.liveCount = 0 then none
  else some { makerRoot with liveCount := makerRoot.liveCount - 1 }

/-- Record the obligation one fee-bearing fill leaves behind.

At most one unsettled fee exists per maker per market: recording onto a root
that already owes refuses rather than accumulates, exactly as the chain's
`record_fee_owed` does. -/
def recordFeeOwed (makerRoot : MakerRoot) (amount : Nat) : Option MakerRoot :=
  if ¬makerRoot.Valid ∨ amount = 0 ∨ makerRoot.feeOwed ≠ 0 then none
  else some { makerRoot with feeOwed := amount }

/-- Clear the obligation, for exactly the amount recorded -- never "whatever
was delegated", so a short settlement can never clear the flag. -/
def settleFeeOwed (makerRoot : MakerRoot) (amount : Nat) : Option MakerRoot :=
  if ¬makerRoot.Valid ∨ amount = 0 ∨ makerRoot.feeOwed ≠ amount then none
  else some { makerRoot with feeOwed := 0 }

/-- The E5 lockout: an outstanding fee refuses every further nonce consumption
by that maker until settled.  Without this, close-and-recreate in Open would
mint a fresh `feeOwed = 0` root and launder the debt. -/
theorem outstanding_fee_locks_consumption
    (root : Root) (makerRoot : MakerRoot) (intent : IntentCoordinate)
    (consumption : Consumption)
    (owing : makerRoot.feeOwed ≠ 0) :
    consumeNonce root (some makerRoot) intent consumption none = none := by
  simp [consumeNonce, owing]

/-- A settlement clears exactly the recorded amount and changes nothing else. -/
theorem settle_is_exact
    (makerRoot settled : MakerRoot) (amount : Nat)
    (success : settleFeeOwed makerRoot amount = some settled) :
    makerRoot.feeOwed = amount ∧ 0 < amount ∧
      settled = { makerRoot with feeOwed := 0 } := by
  simp [settleFeeOwed] at success
  rcases success with ⟨⟨_, nonzero, exact⟩, rfl⟩
  exact ⟨exact, Nat.pos_of_ne_zero nonzero, rfl⟩

/-- Sole persisted economic state for one registered signed intent.  The
physical record also carries exact signed bytes, PDA bump, and rent facts; this
projection owns only the transition-relevant integers. -/
structure RegisteredRecord where
  intent : Intent
  filled : Nat
  reservedClaims : Nat
  reservedCollateral : Nat
  cumulativeGross : Nat
  cumulativeFee : Nat
  deriving DecidableEq, Repr

/-- The one named fee floor. -/
def feeFloor (config : ExecutionConfig) (gross : Nat) : Nat :=
  gross * config.feeBasisPoints / feeDenominator

/-- Exact worst-case Buy reserve at the signed limit. -/
def maximumBuyReserve (config : ExecutionConfig) (intent : Intent) : Nat :=
  let gross := intent.maxFill * intent.limitPrice / config.priceScale
  gross + feeFloor config gross

/-- A live record has one reservation and one cumulative fee authority. -/
def RegisteredRecord.Valid
    (config : ExecutionConfig) (record : RegisteredRecord) : Prop :=
  config.Valid ∧
  record.intent.lifecycle = .goodTillCancelled ∧
  record.intent.maxFill ≠ 0 ∧
  record.intent.limitPrice ≤ config.priceScale ∧
  record.intent.feeBasisPoints = config.feeBasisPoints ∧
  record.filled < record.intent.maxFill ∧
  record.cumulativeGross ≤ record.filled ∧
  record.cumulativeFee = feeFloor config record.cumulativeGross ∧
  match record.intent.side with
    | .sell =>
        record.reservedClaims = record.intent.maxFill - record.filled ∧
        record.reservedCollateral = 0
    | .buy =>
        record.reservedClaims = 0 ∧
        record.reservedCollateral =
          maximumBuyReserve config record.intent -
            (record.cumulativeGross + record.cumulativeFee)

instance (config : ExecutionConfig) (record : RegisteredRecord) :
    Decidable (record.Valid config) := by
  unfold RegisteredRecord.Valid ExecutionConfig.Valid
  cases record.intent.side <;> infer_instance

/-- Checked participant effects.  For Sell, fee is withheld from gross; for
Buy it is added above gross.  Thus every maker's cumulative fee is independent
of how a matcher partitions fills. -/
structure RegisteredFillEffects where
  claimCustodyDebit : Nat
  claimPositionCredit : Nat
  grossDebit : Nat
  grossCredit : Nat
  feeDelta : Nat
  netCredit : Nat
  deriving DecidableEq, Repr

structure RegisteredFillCandidate where
  record : Option RegisteredRecord
  effects : RegisteredFillEffects
  claimRefund : Nat
  collateralRefund : Nat
  deriving DecidableEq, Repr

/-- Total registered partial/full-fill transition.  Exact quote divisibility,
slot/price/Market joins, and replay-root membership are additional outer
admission facts and never caller-authored effects. -/
def fillRegistered
    (config : ExecutionConfig) (record : RegisteredRecord)
    (fill gross : Nat) : Option RegisteredFillCandidate := do
  if ¬record.Valid config ∨ fill = 0 ∨ record.intent.maxFill < record.filled + fill ∨
      gross > fill then none else
  let nextGross := record.cumulativeGross + gross
  let nextFee := feeFloor config nextGross
  if nextFee < record.cumulativeFee then none else
  let feeDelta := nextFee - record.cumulativeFee
  match record.intent.side with
  | .sell =>
      if record.reservedClaims < fill ∨ gross < feeDelta then none else
      let claims := record.reservedClaims - fill
      let filled := record.filled + fill
      let next := {
        record with
        filled := filled
        reservedClaims := claims
        cumulativeGross := nextGross
        cumulativeFee := nextFee
      }
      some {
        record := if filled = record.intent.maxFill then none else some next
        effects := {
          claimCustodyDebit := fill
          claimPositionCredit := 0
          grossDebit := 0
          grossCredit := gross
          feeDelta := feeDelta
          netCredit := gross - feeDelta
        }
        claimRefund := if filled = record.intent.maxFill then claims else 0
        collateralRefund := 0
      }
  | .buy =>
      if record.reservedCollateral < gross + feeDelta then none else
      let collateral := record.reservedCollateral - (gross + feeDelta)
      let filled := record.filled + fill
      let next := {
        record with
        filled := filled
        reservedCollateral := collateral
        cumulativeGross := nextGross
        cumulativeFee := nextFee
      }
      some {
        record := if filled = record.intent.maxFill then none else some next
        effects := {
          claimCustodyDebit := 0
          claimPositionCredit := fill
          grossDebit := gross
          grossCredit := 0
          feeDelta := feeDelta
          netCredit := 0
        }
        claimRefund := 0
        collateralRefund := if filled = record.intent.maxFill then collateral else 0
      }

/-- Difference-of-rounded fees telescope to the single named floor. -/
theorem cumulative_fee_difference_exact
    (config : ExecutionConfig) (prior added : Nat)
    (monotone : feeFloor config prior ≤ feeFloor config (prior + added)) :
    feeFloor config prior +
        (feeFloor config (prior + added) - feeFloor config prior) =
      feeFloor config (prior + added) := by
  omega

/-- One runtime-width complement is canonical only when every outcome appears
once in order and the price/gross sums close exactly. -/
structure CanonicalComplement (config : ExecutionConfig) (fill : Nat) where
  outcomes : List Nat
  prices : List Nat
  gross : List Nat
  widthAtLeastTwo : 2 ≤ outcomes.length
  equalWidths : outcomes.length = prices.length ∧ prices.length = gross.length
  outcomesCanonical : outcomes = List.range outcomes.length
  priceSum : prices.sum = config.priceScale
  grossSum : gross.sum = fill

theorem complement_gross_is_conserved
    (config : ExecutionConfig) (fill : Nat)
    (complement : CanonicalComplement config fill) :
    complement.gross.sum = fill := complement.grossSum

/-- O(1) maker kill switch.  Older live records become permissionlessly
closable by their separate record lifecycle. -/
def cancelThrough (makerRoot : MakerRoot) (minimum : Nat) : Option MakerRoot :=
  if ¬makerRoot.Valid ∨ minimum < makerRoot.minimumLiveNonce ∨
      makerRoot.nextNonce < minimum then none
  else some { makerRoot with minimumLiveNonce := minimum }

/-- Irreversibly stop new Direct nonce consumption. -/
def beginRetiring (root : Root) : Option Root :=
  if ¬root.Valid ∨ root.phase ≠ .open then none
  else some { root with phase := .retiring }

/-- Exact account-rent and unclassified-donation refund on maker-root close. -/
structure MakerClosePlan where
  rentOwner : Nat
  rentPrincipal : Nat
  unclassifiedDonation : Nat
  totalCredit : Nat
  deriving DecidableEq, Inhabited, Repr

structure MakerCloseResult where
  root : Root
  plan : MakerClosePlan
  deriving DecidableEq, Inhabited, Repr

/-- Close a zero-live, zero-debt maker root only after global Direct
retirement begins.

The `feeOwed ≠ 0` refusal is the FEE-TX2 amendment (cohort-9 review item 1,
amendment 2): the maker root is the sole record of the receivable, so a close
that ignored it would erase a debt with no residue.  Settlement is phase-free,
so settle-then-close is always available in Retiring; nothing strands. -/
def closeMaker
    (root : Root) (makerRoot : MakerRoot) (observedLamports : Nat) :
    Option MakerCloseResult := do
  if ¬root.Valid ∨ root.phase ≠ .retiring ∨ root.openMakerRootCount = 0 ∨
      ¬makerRoot.Valid ∨ makerRoot.liveCount ≠ 0 ∨ makerRoot.feeOwed ≠ 0 ∨
      observedLamports < makerRoot.rentPrincipal then none
  else
    some {
      root := { root with openMakerRootCount := root.openMakerRootCount - 1 }
      plan := {
        rentOwner := makerRoot.rentOwner
        rentPrincipal := makerRoot.rentPrincipal
        unclassifiedDonation := observedLamports - makerRoot.rentPrincipal
        totalCredit := observedLamports
      }
    }

theorem maker_close_count_conserved
    (root : Root) (makerRoot : MakerRoot) (lamports : Nat)
    (result : MakerCloseResult)
    (success : closeMaker root makerRoot lamports = some result) :
    result.root.openMakerRootCount + 1 = root.openMakerRootCount := by
  simp [closeMaker] at success
  rcases success with ⟨_, _, count, _, _, _, _, rfl⟩
  change root.openMakerRootCount - 1 + 1 = root.openMakerRootCount
  omega

theorem maker_close_refund_conserved
    (root : Root) (makerRoot : MakerRoot) (lamports : Nat)
    (result : MakerCloseResult)
    (success : closeMaker root makerRoot lamports = some result) :
    result.plan.rentPrincipal + result.plan.unclassifiedDonation =
      result.plan.totalCredit := by
  simp [closeMaker] at success
  rcases success with ⟨_, _, _, _, _, _, funded, rfl⟩
  change makerRoot.rentPrincipal + (lamports - makerRoot.rentPrincipal) = lamports
  omega

/-- Fee conservation at close: a close is never the event that ends a nonzero
obligation.  The only transition that zeroes `feeOwed` is `settleFeeOwed`,
which demands the exact recorded amount -- so the receivable either stands on
the replay or was paid in full, never erased. -/
theorem close_conserves_fee_receivable
    (root : Root) (makerRoot : MakerRoot) (lamports : Nat)
    (result : MakerCloseResult)
    (success : closeMaker root makerRoot lamports = some result) :
    makerRoot.feeOwed = 0 := by
  simp [closeMaker] at success
  exact success.1.2.2.2.2.2.1

/-- The debtor's close refuses by name: `feeOwed ≠ 0` alone forces refusal. -/
theorem debtor_close_refuses
    (root : Root) (makerRoot : MakerRoot) (lamports : Nat)
    (owing : makerRoot.feeOwed ≠ 0) :
    closeMaker root makerRoot lamports = none := by
  simp [closeMaker, owing]

/-- The reachability amendment (cohort-9 review item 1, amendment 1): global
retirement begins regardless of how many maker roots stand open.  Makers wind
down INSIDE Retiring -- `consumeNonce` already refuses every non-Open phase,
so Retiring stops new consumption while `closeMaker` drains the count. -/
theorem begin_retiring_admits_open_maker_roots
    (root : Root) (valid : root.Valid) (opened : root.phase = .open) :
    beginRetiring root =
      some { root with phase := .retiring } := by
  simp [beginRetiring, valid, opened]

/-- A composite Direct root may physically close only after retirement and the
last maker root has returned its rent. -/
def rootClosable (root : Root) : Bool :=
  decide (root.Valid ∧ root.phase = .retiring ∧ root.openMakerRootCount = 0)

theorem open_root_not_closable (count : Nat) :
    rootClosable { phase := .open, openMakerRootCount := count } = false := by
  simp [rootClosable]

/-- The invariant the moved gates still protect: the step to Retired -- the
physical root close -- is legal only inside Retiring with zero open maker
roots.  Amendment 1 moved the count gate off `beginRetiring`; the invariant
itself never moved. -/
theorem retired_requires_zero_open_makers
    (root : Root) (closable : rootClosable root = true) :
    root.phase = .retiring ∧ root.openMakerRootCount = 0 := by
  simp [rootClosable] at closable
  exact ⟨closable.2.1, closable.2.2⟩

namespace Examples

def openRoot : Root := { phase := .open, openMakerRootCount := 0 }
def intent : IntentCoordinate := { market := 1, generation := 7, maker := 2, nonce := 0 }
def funding : FirstUse := { rentOwner := 9, rentPrincipal := 100, observedLamports := 3 }

theorem inline_first_use :
    consumeNonce openRoot none intent .inline (some funding) = some {
      root := { phase := .open, openMakerRootCount := 1 }
      makerRoot := {
        market := 1, generation := 7, maker := 2, nextNonce := 1,
        liveCount := 0, minimumLiveNonce := 0, rentOwner := 9,
        rentPrincipal := 100, feeOwed := 0
      }
      creation := some { topUpLamports := 97, postLamports := 100 }
    } := by native_decide

theorem hostile_replay_refuses :
    let created := (consumeNonce openRoot none intent .inline (some funding)).get!
    consumeNonce created.root (some created.makerRoot) intent .inline none = none := by
  native_decide

theorem closure_path :
    let created := (consumeNonce openRoot none intent .inline (some funding)).get!
    let retiring := (beginRetiring created.root).get!
    let closed := (closeMaker retiring created.makerRoot 111).get!
    closed.plan.rentPrincipal = 100 ∧
      closed.plan.unclassifiedDonation = 11 ∧
      closed.root.openMakerRootCount = 0 ∧
      rootClosable closed.root = true := by
  native_decide

/-- The debtor's whole story, decided: an outstanding fee refuses the close
and every further consumption; a short settlement refuses; the exact
settlement unlocks the close. -/
theorem debtor_settles_before_closing :
    let created := (consumeNonce openRoot none intent .inline (some funding)).get!
    let owing := (recordFeeOwed created.makerRoot 4).get!
    let retiring := (beginRetiring created.root).get!
    closeMaker retiring owing 111 = none ∧
      consumeNonce created.root (some owing)
        { intent with nonce := 1 } .inline none = none ∧
      settleFeeOwed owing 3 = none ∧
      (closeMaker retiring ((settleFeeOwed owing 4).get!) 111).isSome = true := by
  native_decide

/-- Retirement begins over a standing maker root -- the count is drained
INSIDE Retiring, which is what makes the close reachable for a filled market. -/
theorem retiring_starts_with_open_makers :
    let created := (consumeNonce openRoot none intent .inline (some funding)).get!
    let retiring := (beginRetiring created.root).get!
    created.root.openMakerRootCount = 1 ∧
      retiring.phase = RootPhase.retiring ∧
      rootClosable retiring = false := by
  native_decide

end Examples

end DClutch.DirectSuccessor
