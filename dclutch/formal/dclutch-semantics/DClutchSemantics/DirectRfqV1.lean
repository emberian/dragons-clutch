import Std.Tactic
import DClutchSemantics.JointClearingV1
import DClutchSemantics.DirectIntentV3Codec

/-!
# The RFQ: a Direct trade is a batch of two

The batch spine (`docs/design/MECHANISM_BATCH_SPINE_2026_09_04.md` §1.5) keeps
Direct as the RFQ venue and says what that means exactly: *every transfer of
claims between two parties is a verified General candidate — including the
bilateral one*. This module is that sentence as a theorem. A Direct fill is
two signed tickets; this module maps them onto two `JointClearing.Order`s,
derives the one price the pair clears at, builds the `Clearing` of that
batch of two, and proves that a crossed pair's clearing passes the joint
clearing's own certificate, `Clearing.valid`, with no complete-set move.

Two amendments turn the inline fill into an RFQ rather than a bilateral
match, and both are stated here:

* **the price is derived, not the matcher's.** `derivedPrice` is the equal
  split of the seller's floor and the buyer's cap, rounded down. A pair that
  agreed a price (`floor = cap`) clears at it unchanged
  (`derivedPrice_of_agreed`); a pair whose limits cross clears strictly
  between them (`derivedPrice_between`). Decision 0032 §2b's tie-break —
  the lexicographically minimal price vector among certified candidates —
  is a SELECTION rule among competing candidates; an RFQ has exactly one
  candidate and no selection, so it needs a price rule of its own, and the
  equal split is the one that treats the two signers symmetrically.
* **the ticket may name its taker.** `Rfq.counterpartyAdmits` is the
  conjunct: a ticket whose `counterparty` is nonzero binds the other maker;
  a bearer ticket binds nobody.

What the RFQ gives up relative to General is stated at the end: it has no
window, so it has no solver competition, so it does not need a selection,
so `Batch.clear?` on the two-order batch is the whole lifecycle.

`ClearingPriceV1` — the persisted per-batch price vector the price series
reads — is the JOINT-CLEARING lane's; `Rfq.priceVector` is exactly the
vector that fact would record for a batch of two, and this module names the
seam rather than building a second author of it.
-/

namespace DClutch.DirectRfqV1

open DClutch.JointClearing

/-! ## The derived price -/

/-- The equal split of the seller's floor and the buyer's cap, rounded down.
Symmetric in its arguments, and the identity on an agreed price. -/
def derivedPrice (sellerFloor buyerCap : Nat) : Nat := (sellerFloor + buyerCap) / 2

theorem derivedPrice_symm (s b : Nat) : derivedPrice s b = derivedPrice b s := by
  simp [derivedPrice, Nat.add_comm]

theorem derivedPrice_of_agreed (p : Nat) : derivedPrice p p = p := by
  simp [derivedPrice]
  omega

/-- A crossed pair clears between its limits. -/
theorem derivedPrice_between (s b : Nat) (h : s ≤ b) :
    s ≤ derivedPrice s b ∧ derivedPrice s b ≤ b := by
  simp only [derivedPrice]
  omega

/-- The floor never exceeds the cap on a crossed pair, and the derived price
cannot be moved off the interval by rounding: the interval always contains it. -/
theorem derivedPrice_le_scale (s b scale : Nat) (hb : b ≤ scale) :
    derivedPrice s b ≤ scale ∨ scale < s := by
  simp only [derivedPrice]
  omega

/-! ## Vectors of one outcome -/

def zeros (outcomeCount : Nat) : List Nat := List.replicate outcomeCount 0

def unit (outcomeCount outcome : Nat) : List Nat :=
  (List.range outcomeCount).map fun i => if i = outcome then 1 else 0

theorem zeros_length (n : Nat) : (zeros n).length = n := by simp [zeros]

theorem unit_length (n o : Nat) : (unit n o).length = n := by simp [unit]

theorem valueAt_zeros (n i : Nat) : valueAt (zeros n) i = 0 := by
  simp only [valueAt, zeros, List.getElem?_replicate]
  split <;> rfl

theorem valueAt_unit (n o i : Nat) (hi : i < n) :
    valueAt (unit n o) i = if i = o then 1 else 0 := by
  simp [valueAt, unit, hi]

/-- `sumRange` of a function supported on one index is that index's value. -/
theorem sumRange_single (n j : Nat) (hj : j < n) (c : Nat → Int) :
    sumRange n (fun i => if i = j then c i else 0) = c j := by
  induction n generalizing j c with
  | zero => omega
  | succ n ih =>
    simp only [sumRange]
    cases j with
    | zero =>
      simp only [if_true]
      rw [sumRange_congr (g := fun _ => 0) (fun i _ => by simp)]
      rw [sumRange_zero]
      omega
    | succ k =>
      have hk : k < n := by omega
      simp only [show (0 = k + 1) = False from by simp, if_false, Int.zero_add]
      rw [sumRange_congr (g := fun i => if i = k then c (i + 1) else 0)
        (fun i _ => by by_cases h : i = k <;> simp [h])]
      exact ih k hk (fun i => c (i + 1))

/-! ## The RFQ -/

/-- Two tickets on one outcome, reduced to the facts the clearing needs.
`sellerFloor` and `buyerCap` are the two tickets' `limitPrice`s; `fill` is
the common maximum fill (fill-or-kill: the RFQ has no partial fill, because a
partial fill of a batch of two would leave one party rationed strictly inside
their limit, which the marginal conjunct refuses). -/
structure Rfq where
  outcomeCount : Nat
  scale : Nat
  outcome : Nat
  sellerId : Nat
  buyerId : Nat
  sellerFloor : Nat
  buyerCap : Nat
  fill : Nat
  deriving DecidableEq, Repr

/-- The RFQ's price on its traded outcome. -/
def Rfq.price (rfq : Rfq) : Nat := derivedPrice rfq.sellerFloor rfq.buyerCap

/-- Where the simplex remainder sits: the highest index other than the traded
outcome. This is decision 0032 §2b's lexicographically-minimal vector
restricted to the one-dimensional face an RFQ can certify — every outcome
before the remainder's index is priced zero. -/
def complementIndex (outcomeCount outcome : Nat) : Nat :=
  if outcome + 1 = outcomeCount then outcomeCount - 2 else outcomeCount - 1

/-- The RFQ's price vector: `price` on the traded outcome, the remainder on
the complement index, zero elsewhere. This is what `ClearingPriceV1` would
persist for the batch of two. -/
def Rfq.priceVector (rfq : Rfq) : List Nat :=
  (List.range rfq.outcomeCount).map fun i =>
    if i = rfq.outcome then rfq.price
    else if i = complementIndex rfq.outcomeCount rfq.outcome then rfq.scale - rfq.price
    else 0

/-- The seller's ticket as a joint-clearing order: delivers `e_outcome`, floor
as a negative limit (`JointClearingV1` §1.1). -/
def Rfq.sellerOrder (rfq : Rfq) : Order := {
  orderId := rfq.sellerId
  receivePerLot := zeros rfq.outcomeCount
  deliverPerLot := unit rfq.outcomeCount rfq.outcome
  quantity := rfq.fill
  limit := -(rfq.sellerFloor : Int)
}

/-- The buyer's ticket as a joint-clearing order: receives `e_outcome`, cap as
a positive limit. -/
def Rfq.buyerOrder (rfq : Rfq) : Order := {
  orderId := rfq.buyerId
  receivePerLot := unit rfq.outcomeCount rfq.outcome
  deliverPerLot := zeros rfq.outcomeCount
  quantity := rfq.fill
  limit := (rfq.buyerCap : Int)
}

@[simp] theorem sellerOrder_orderId (rfq : Rfq) : rfq.sellerOrder.orderId = rfq.sellerId := rfl
@[simp] theorem sellerOrder_receive (rfq : Rfq) :
    rfq.sellerOrder.receivePerLot = zeros rfq.outcomeCount := rfl
@[simp] theorem sellerOrder_deliver (rfq : Rfq) :
    rfq.sellerOrder.deliverPerLot = unit rfq.outcomeCount rfq.outcome := rfl
@[simp] theorem sellerOrder_quantity (rfq : Rfq) : rfq.sellerOrder.quantity = rfq.fill := rfl
@[simp] theorem sellerOrder_limit (rfq : Rfq) :
    rfq.sellerOrder.limit = -(rfq.sellerFloor : Int) := rfl
@[simp] theorem buyerOrder_orderId (rfq : Rfq) : rfq.buyerOrder.orderId = rfq.buyerId := rfl
@[simp] theorem buyerOrder_receive (rfq : Rfq) :
    rfq.buyerOrder.receivePerLot = unit rfq.outcomeCount rfq.outcome := rfl
@[simp] theorem buyerOrder_deliver (rfq : Rfq) :
    rfq.buyerOrder.deliverPerLot = zeros rfq.outcomeCount := rfl
@[simp] theorem buyerOrder_quantity (rfq : Rfq) : rfq.buyerOrder.quantity = rfq.fill := rfl
@[simp] theorem buyerOrder_limit (rfq : Rfq) : rfq.buyerOrder.limit = (rfq.buyerCap : Int) := rfl

/-- The batch of two, cleared: both orders filled in full at the derived
price, and no complete-set move — a transfer. -/
def Rfq.clearing (rfq : Rfq) : Clearing := {
  outcomeCount := rfq.outcomeCount
  scale := rfq.scale
  prices := rfq.priceVector
  fills := [⟨rfq.sellerOrder, rfq.fill⟩, ⟨rfq.buyerOrder, rfq.fill⟩]
  sets := 0
}

@[simp] theorem clearing_outcomeCount (rfq : Rfq) :
    rfq.clearing.outcomeCount = rfq.outcomeCount := rfl
@[simp] theorem clearing_scale (rfq : Rfq) : rfq.clearing.scale = rfq.scale := rfl
@[simp] theorem clearing_prices (rfq : Rfq) : rfq.clearing.prices = rfq.priceVector := rfl
@[simp] theorem clearing_fills (rfq : Rfq) :
    rfq.clearing.fills = [⟨rfq.sellerOrder, rfq.fill⟩, ⟨rfq.buyerOrder, rfq.fill⟩] := rfl
@[simp] theorem clearing_sets (rfq : Rfq) : rfq.clearing.sets = 0 := rfl

/-- The batch of two before clearing: closed on arrival (its collection
window is the transaction), two live orders. -/
def Rfq.batch (_ : Rfq) : Batch := { phase := .closed, liveOrders := 2, clearing := none }

/-- What the RFQ verifier checks before it builds the clearing. Every clause
is one refusal by name in the adapter. -/
def Rfq.crossed (rfq : Rfq) : Bool :=
  2 ≤ rfq.outcomeCount && rfq.outcome < rfq.outcomeCount &&
    0 < rfq.scale && rfq.buyerCap ≤ rfq.scale &&
    rfq.sellerFloor ≤ rfq.buyerCap && 0 < rfq.fill &&
    rfq.sellerId != 0 && rfq.buyerId != 0 && rfq.sellerId != rfq.buyerId

/-- The counterparty conjunct, stated on the two tickets. A ticket that names
a counterparty binds the other maker; a bearer ticket binds nobody. -/
def counterpartyAdmits (ticket : DirectIntentV3Codec.CompactIntentV3)
    (otherMaker : DirectControllerCodec.Bytes32) : Bool :=
  ticket.bearer ||
    DirectControllerCodec.encodeBytes32 ticket.counterparty ==
      DirectControllerCodec.encodeBytes32 otherMaker

/-- Two signed tickets to the facts the clearing reads. The ids are the
adapter's content identities of the two signed preimages; only their
distinctness and nonzeroness matter here. -/
def Rfq.ofTickets (outcomeCount scale sellerId buyerId : Nat)
    (seller buyer : DirectIntentV3Codec.CompactIntentV3) : Rfq := {
  outcomeCount := outcomeCount
  scale := scale
  outcome := seller.outcome
  sellerId := sellerId
  buyerId := buyerId
  sellerFloor := seller.limitPrice
  buyerCap := buyer.limitPrice
  fill := seller.maximumFill
}

/-! ## Facts about the price vector -/

theorem priceVector_length (rfq : Rfq) : rfq.priceVector.length = rfq.outcomeCount := by
  simp [Rfq.priceVector]

theorem complementIndex_lt (n o : Nat) (hn : 2 ≤ n) :
    complementIndex n o < n := by
  simp only [complementIndex]
  split <;> omega

theorem complementIndex_ne (n o : Nat) (hn : 2 ≤ n) :
    complementIndex n o ≠ o := by
  simp only [complementIndex]
  split <;> omega

theorem valueAt_priceVector (rfq : Rfq) (i : Nat) (hi : i < rfq.outcomeCount) :
    valueAt rfq.priceVector i =
      if i = rfq.outcome then rfq.price
      else if i = complementIndex rfq.outcomeCount rfq.outcome then rfq.scale - rfq.price
      else 0 := by
  simp [valueAt, Rfq.priceVector, hi]

theorem price_at_outcome (rfq : Rfq) (ho : rfq.outcome < rfq.outcomeCount) :
    rfq.clearing.price rfq.outcome = (rfq.price : Int) := by
  simp [Rfq.clearing, Clearing.price, valueAtZ, valueAt_priceVector rfq rfq.outcome ho]

/-- The price vector is on the simplex whenever the pair is crossed. -/
theorem priceVector_sum (rfq : Rfq) (h : rfq.crossed = true) :
    rfq.priceVector.sum = rfq.scale := by
  simp only [Rfq.crossed, Bool.and_eq_true, decide_eq_true_eq, bne_iff_ne, ne_eq] at h
  obtain ⟨⟨⟨⟨⟨⟨⟨⟨hn, ho⟩, hs⟩, hcap⟩, hcross⟩, hfill⟩, hsid⟩, hbid⟩, hne⟩ := h
  have hp : rfq.price ≤ rfq.scale := by
    have := derivedPrice_between rfq.sellerFloor rfq.buyerCap hcross
    simp only [Rfq.price]
    omega
  have hc := complementIndex_lt rfq.outcomeCount rfq.outcome hn
  have hcne := complementIndex_ne rfq.outcomeCount rfq.outcome hn
  have key : sumRange rfq.priceVector.length (valueAtZ rfq.priceVector) =
      (rfq.price : Int) + ((rfq.scale - rfq.price : Nat) : Int) := by
    rw [priceVector_length]
    rw [sumRange_congr (g := fun i =>
      (if i = rfq.outcome then (rfq.price : Int) else 0) +
      (if i = complementIndex rfq.outcomeCount rfq.outcome
        then ((rfq.scale - rfq.price : Nat) : Int) else 0))
      (fun i hi => by
        rw [valueAtZ, valueAt_priceVector rfq i hi]
        by_cases h1 : i = rfq.outcome
        · subst h1
          simp [hcne.symm]
        · by_cases h2 : i = complementIndex rfq.outcomeCount rfq.outcome
          · subst h2
            simp [h1]
          · simp [h1, h2])]
    rw [sumRange_add, sumRange_single _ _ ho, sumRange_single _ _ hc]
  have := sumRange_valueAt rfq.priceVector
  rw [key] at this
  have hz : (rfq.price : Int) + ((rfq.scale - rfq.price : Nat) : Int) = (rfq.scale : Int) := by
    omega
  rw [hz] at this
  exact Int.ofNat.inj this.symm

/-! ## The two orders, at the derived price -/

theorem buyer_perLotDebit (rfq : Rfq) (ho : rfq.outcome < rfq.outcomeCount) :
    rfq.buyerOrder.perLotDebit rfq.clearing = (rfq.price : Int) := by
  simp only [Order.perLotDebit, Rfq.clearing, Clearing.price, Order.flow, buyerOrder_receive,
    buyerOrder_deliver]
  rw [sumRange_congr (g := fun i => if i = rfq.outcome then (rfq.price : Int) else 0)
    (fun i hi => by
      simp only [valueAtZ, valueAt_unit _ _ _ hi, valueAt_zeros, valueAt_priceVector rfq i hi]
      by_cases h1 : i = rfq.outcome
      · subst h1; simp
      · simp [h1])]
  exact sumRange_single _ _ ho _

theorem seller_perLotDebit (rfq : Rfq) (ho : rfq.outcome < rfq.outcomeCount) :
    rfq.sellerOrder.perLotDebit rfq.clearing = -(rfq.price : Int) := by
  simp only [Order.perLotDebit, Rfq.clearing, Clearing.price, Order.flow, sellerOrder_receive,
    sellerOrder_deliver]
  rw [sumRange_congr (g := fun i => if i = rfq.outcome then -(rfq.price : Int) else 0)
    (fun i hi => by
      simp only [valueAtZ, valueAt_unit _ _ _ hi, valueAt_zeros, valueAt_priceVector rfq i hi]
      by_cases h1 : i = rfq.outcome
      · subst h1; simp
      · simp [h1])]
  exact sumRange_single _ _ ho _

/-- A transfer: the two fills cancel at every outcome. -/
theorem net_zero (rfq : Rfq) (i : Nat) (hi : i < rfq.outcomeCount) :
    rfq.clearing.net i = 0 := by
  simp only [Clearing.net, Rfq.clearing, netAt, List.map_cons, List.map_nil, List.sum_cons,
    List.sum_nil, Fill.contribution, Order.flow, sellerOrder_receive, sellerOrder_deliver,
    buyerOrder_receive, buyerOrder_deliver, valueAtZ, valueAt_unit _ _ _ hi, valueAt_zeros]
  by_cases h1 : i = rfq.outcome <;> simp [h1] <;> omega

/-! ## The theorem: a crossed RFQ is a valid clearing of a batch of two -/

theorem crossed_clears (rfq : Rfq) (h : rfq.crossed = true) :
    rfq.clearing.valid = true := by
  have hcopy := h
  simp only [Rfq.crossed, Bool.and_eq_true, decide_eq_true_eq, bne_iff_ne, ne_eq] at h
  obtain ⟨⟨⟨⟨⟨⟨⟨⟨hn, ho⟩, hs⟩, hcap⟩, hcross⟩, hfill⟩, hsid⟩, hbid⟩, hne⟩ := h
  have hbetween := derivedPrice_between rfq.sellerFloor rfq.buyerCap hcross
  have hbuy := buyer_perLotDebit rfq ho
  have hsell := seller_perLotDebit rfq ho
  -- the seller is paid at least the floor; the buyer pays at most the cap
  have hsellerLimit : rfq.sellerOrder.perLotDebit rfq.clearing ≤ -(rfq.sellerFloor : Int) := by
    rw [hsell]; simp only [Rfq.price]; omega
  have hbuyerLimit : rfq.buyerOrder.perLotDebit rfq.clearing ≤ (rfq.buyerCap : Int) := by
    rw [hbuy]; simp only [Rfq.price]; omega
  have hrows : rfq.clearing.fills.all (fillAdmissible rfq.clearing) = true := by
    simp [fillAdmissible, Order.validFor, zeros_length, unit_length, hsid, hbid, hfill,
      hsellerLimit, hbuyerLimit]
  have hdistinct : (rfq.clearing.fills.map fun f => f.order.orderId).Nodup := by
    simp [hne]
  have hcover : (List.range rfq.clearing.outcomeCount).all
      (fun i => decide (rfq.clearing.net i ≤ rfq.clearing.sets) &&
        (decide (rfq.clearing.price i = 0) || decide (rfq.clearing.net i = rfq.clearing.sets))) =
      true := by
    simp only [List.all_eq_true, List.mem_range, clearing_outcomeCount]
    intro i hi
    have hnet := net_zero rfq i hi
    simp [hnet]
  simp only [Clearing.valid, Bool.and_eq_true, decide_eq_true_eq]
  refine ⟨⟨⟨⟨⟨⟨?_, ?_⟩, ?_⟩, ?_⟩, hrows⟩, hdistinct⟩, hcover⟩
  · show 0 < rfq.clearing.outcomeCount
    simp only [clearing_outcomeCount]; omega
  · show 0 < rfq.clearing.scale
    simpa using hs
  · show rfq.clearing.prices.length = rfq.clearing.outcomeCount
    simp [priceVector_length]
  · show rfq.clearing.prices.sum = rfq.clearing.scale
    simpa using priceVector_sum rfq hcopy

/-- The RFQ's batch clears exactly once, into exactly this clearing. -/
theorem rfq_clears_once (rfq : Rfq) (h : rfq.crossed = true) :
    rfq.batch.clear? rfq.clearing = .ok
      { phase := .cleared, liveOrders := 2, clearing := some rfq.clearing } := by
  simp [Rfq.batch, Batch.clear?, crossed_clears rfq h]

/-- An RFQ is a transfer: no complete set is minted or merged. -/
theorem rfq_moves_no_sets (rfq : Rfq) : rfq.clearing.sets = 0 := rfl

/-- An RFQ is a batch of exactly two orders, and both are filled in full. -/
theorem rfq_is_two_full_fills (rfq : Rfq) :
    rfq.clearing.fills.length = 2 ∧
      rfq.clearing.fills.all (fun f => f.lots == f.order.quantity) = true := by
  simp [Rfq.clearing, Rfq.sellerOrder, Rfq.buyerOrder]

/-- The buyer pays and the seller is paid exactly the derived price per lot:
the uniform price of the batch of two, by arithmetic. -/
theorem rfq_uniform_price (rfq : Rfq) (ho : rfq.outcome < rfq.outcomeCount) :
    rfq.buyerOrder.perLotDebit rfq.clearing = -(rfq.sellerOrder.perLotDebit rfq.clearing) := by
  rw [buyer_perLotDebit rfq ho, seller_perLotDebit rfq ho]
  omega

/-! ## Hostiles, each against its conjunct -/

/-- An uncrossed pair — floor above cap — has no price both admit: the derived
price lies between them and so violates one side, and the clearing refuses. -/
theorem uncrossed_refuses (rfq : Rfq) (ho : rfq.outcome < rfq.outcomeCount)
    (hfill : 0 < rfq.fill) (h : rfq.buyerCap < rfq.sellerFloor) :
    rfq.clearing.valid = false := by
  have hbuy := buyer_perLotDebit rfq ho
  have hsell := seller_perLotDebit rfq ho
  cases hval : rfq.clearing.valid
  · rfl
  · exfalso
    have hs := filled_at_or_better rfq.clearing hval ⟨rfq.sellerOrder, rfq.fill⟩
      (by simp [Rfq.clearing]) hfill
    have hb := filled_at_or_better rfq.clearing hval ⟨rfq.buyerOrder, rfq.fill⟩
      (by simp [Rfq.clearing]) hfill
    simp only [hsell, hbuy, sellerOrder_limit, buyerOrder_limit] at hs hb
    omega

/-! ## Executable witnesses -/

namespace Examples

/-- Two outcomes, scale 100: a seller at 40 and a buyer at 60 clear at 50. -/
def crossed : Rfq := {
  outcomeCount := 2, scale := 100, outcome := 0,
  sellerId := 1, buyerId := 2, sellerFloor := 40, buyerCap := 60, fill := 10 }

theorem crossed_price : crossed.price = 50 := by native_decide
theorem crossed_vector : crossed.priceVector = [50, 50] := by native_decide
theorem crossed_valid : crossed.clearing.valid = true := by native_decide

/-- An agreed price: both at 50 clears at 50, unchanged from the inline fill. -/
def agreed : Rfq := { crossed with sellerFloor := 50, buyerCap := 50 }

theorem agreed_price : agreed.price = 50 := by native_decide
theorem agreed_valid : agreed.clearing.valid = true := by native_decide

/-- Five outcomes, trading outcome 2 at 40/60: the remainder sits on the last
outcome and every other coordinate is zero — the lexicographically minimal
vector on the face. -/
def wide : Rfq := { crossed with outcomeCount := 5, outcome := 2, sellerId := 7, buyerId := 9 }

theorem wide_vector : wide.priceVector = [0, 0, 50, 0, 50] := by native_decide
theorem wide_valid : wide.clearing.valid = true := by native_decide

/-- Trading the LAST outcome moves the remainder one index down. -/
def last : Rfq := { wide with outcome := 4 }

theorem last_vector : last.priceVector = [0, 0, 0, 50, 50] := by native_decide
theorem last_valid : last.clearing.valid = true := by native_decide

/-- HOSTILE: a matcher who names 55 on the 40/60 pair. The certificate at 55
still passes (every price between the limits certifies) — which is exactly
why the RFQ derives the price rather than checking it: the adapter refuses
`executionPrice ≠ derivedPrice` BEFORE the certificate, and this witness is
the reason that conjunct exists. -/
def matcherAt55 : Clearing := { crossed.clearing with prices := [55, 45] }

theorem matcher_price_certifies : matcherAt55.valid = true := by native_decide
theorem matcher_price_is_not_derived : matcherAt55.prices ≠ crossed.priceVector := by
  native_decide

/-- HOSTILE: an uncrossed pair, seller at 60 and buyer at 40. -/
def uncrossed : Rfq := { crossed with sellerFloor := 60, buyerCap := 40 }

theorem uncrossed_invalid : uncrossed.clearing.valid = false := by native_decide

/-- HOSTILE: the same identity on both sides. -/
def selfCross : Rfq := { crossed with buyerId := 1 }

theorem self_cross_invalid : selfCross.clearing.valid = false := by native_decide
theorem self_cross_not_crossed : selfCross.crossed = false := by native_decide

/-- HOSTILE: a cap above the scale. The pair is not crossed, and the vector
it would build is off the simplex. -/
def overScale : Rfq := { crossed with buyerCap := 140, sellerFloor := 120 }

theorem over_scale_not_crossed : overScale.crossed = false := by native_decide

/-- HOSTILE: an RFQ that clears twice. -/
theorem clears_twice_refuses :
    (crossed.batch.clear? crossed.clearing).bind (fun post => post.clear? crossed.clearing)
      = .error .alreadyCleared := by native_decide

/-- HOSTILE: a partial acceptance. The taker fills 6 of the maker's 10 and the
maker is rationed strictly inside their floor: the marginal conjunct refuses,
which is why the RFQ is fill-or-kill. -/
def partialFill : Clearing := {
  crossed.clearing with
    fills := [⟨crossed.sellerOrder, 6⟩, ⟨crossed.buyerOrder, 6⟩] }

theorem partial_fill_refuses : partialFill.valid = false := by native_decide

/-- The counterparty conjunct on the codec's two example tickets. -/
def taker : DirectControllerCodec.Bytes32 := DirectIntentV3Codec.byte32 0x77
def stranger : DirectControllerCodec.Bytes32 := DirectIntentV3Codec.byte32 0x78

theorem named_admits_its_taker :
    counterpartyAdmits DirectIntentV3Codec.Examples.named taker = true := by native_decide
theorem named_refuses_a_stranger :
    counterpartyAdmits DirectIntentV3Codec.Examples.named stranger = false := by native_decide
theorem bearer_admits_anyone :
    counterpartyAdmits DirectIntentV3Codec.Examples.bearer stranger = true := by native_decide

end Examples

end DClutch.DirectRfqV1
