import Std.Tactic

/-!
# Joint clearing of one Market's outcomes in one batch

The semantic owner of the JOINT-CLEARING mechanism
(`docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md`): every outcome of one
categorical Market clears in one batch at one uniform price per outcome, and
the batch mints or merges complete sets inside the clearing so that demand on
one outcome can be met by the collateral of buyers of the other outcomes.

This module states the clearing as a CERTIFICATE, not as an algorithm. A
`Clearing` is data — a price vector on the exact simplex, one fill per live
order, and one signed complete-set quantity — and `Clearing.valid` is the
verifier: the Karush–Kuhn–Tucker conditions of the batch's linear program,
which are linear checks a bounded-compute verifier can make. The theorems
below are what the verifier is entitled to conclude from a passing candidate:

* `prices_sum_to_scale`, `price_le_scale` — the price vector is on the simplex;
* `filled_at_or_better` — every positive fill is at or inside its limit;
* `partial_fill_is_marginal` — no order is rationed strictly inside its limit;
* `collectedQuote_funds_sets` — the quote the batch collects is exactly the
  collateral of the sets it mints (or exactly what a merge releases);
* `residual_worth_nothing` — claims the batch is left holding sit only on
  outcomes priced at zero;
* `complement_prices_to_scale` — any bundle and its complement price to one
  complete set, so no round trip through the batch nets a quote;
* `valid_perm` — validity and every quote are independent of the order in
  which fills were submitted;
* `certificate_is_optimal` — a valid clearing maximises limit-price surplus
  net of set cost over every feasible allocation of the same book. This is the
  checked optimality certificate `AGENTS.md` requires before the phrase
  "optimal clearing" may be used.

Everything physical — that a mint moves collateral into the Hoard, that a
fill's order record is the escrowed one, that the beneficiary row receives the
residual — is an adapter obligation exactly as in `GeneralClearing`'s
`AdapterBoundary`; nothing here is a theorem about accounts, signatures or
SBF. The module imports nothing from the existing General model on purpose:
it is a new semantic owner and `GeneralClearing.lean` is not edited.
-/

namespace DClutch.JointClearing

/-! ## Finite sums over an outcome range -/

/-- `sumRange n f = f 0 + … + f (n-1)`, head-first so a list walks in step. -/
def sumRange : Nat → (Nat → Int) → Int
  | 0, _ => 0
  | n + 1, f => f 0 + sumRange n (fun i => f (i + 1))

theorem sumRange_congr {n : Nat} {f g : Nat → Int}
    (h : ∀ i, i < n → f i = g i) : sumRange n f = sumRange n g := by
  induction n generalizing f g with
  | zero => rfl
  | succ n ih =>
    simp only [sumRange]
    rw [h 0 (by omega), ih (fun i hi => h (i + 1) (by omega))]

theorem sumRange_add (n : Nat) (f g : Nat → Int) :
    sumRange n (fun i => f i + g i) = sumRange n f + sumRange n g := by
  induction n generalizing f g with
  | zero => rfl
  | succ n ih =>
    simp only [sumRange]
    rw [ih]
    omega

theorem sumRange_mul_left (n : Nat) (c : Int) (f : Nat → Int) :
    sumRange n (fun i => c * f i) = c * sumRange n f := by
  induction n generalizing f with
  | zero => simp [sumRange]
  | succ n ih =>
    simp only [sumRange]
    rw [ih, Int.mul_add]

theorem sumRange_mul_right (n : Nat) (f : Nat → Int) (c : Int) :
    sumRange n (fun i => f i * c) = sumRange n f * c := by
  rw [Int.mul_comm, ← sumRange_mul_left]
  exact sumRange_congr (fun i _ => Int.mul_comm _ _)

theorem sumRange_zero (n : Nat) : sumRange n (fun _ => 0) = 0 := by
  induction n with
  | zero => rfl
  | succ n ih => simp [sumRange, ih]

theorem sumRange_split (n : Nat) (p : Nat → Int) (mask : Nat → Bool) :
    sumRange n (fun i => if mask i then p i else 0) +
      sumRange n (fun i => if mask i then 0 else p i) = sumRange n p := by
  rw [← sumRange_add]
  exact sumRange_congr (fun i _ => by cases mask i <;> simp)

theorem sumRange_le {n : Nat} {f g : Nat → Int}
    (h : ∀ i, i < n → f i ≤ g i) : sumRange n f ≤ sumRange n g := by
  induction n generalizing f g with
  | zero => exact Int.le_refl 0
  | succ n ih =>
    simp only [sumRange]
    have h0 := h 0 (by omega)
    have hrest := ih (fun i hi => h (i + 1) (by omega))
    omega

/-! ## Exact positional access -/

def valueAt (values : List Nat) (index : Nat) : Nat :=
  values[index]?.getD 0

def valueAtZ (values : List Nat) (index : Nat) : Int :=
  (valueAt values index : Int)

theorem sumRange_valueAt (values : List Nat) :
    sumRange values.length (valueAtZ values) = (values.sum : Int) := by
  induction values with
  | nil => rfl
  | cons h t ih =>
    simp only [sumRange, List.sum_cons]
    rw [show valueAtZ (h :: t) 0 = (h : Int) from by simp [valueAtZ, valueAt]]
    rw [sumRange_congr (f := fun i => valueAtZ (h :: t) (i + 1)) (g := valueAtZ t)
      (fun i _ => by simp [valueAtZ, valueAt])]
    rw [ih]
    omega

theorem valueAt_le_sum
    (values : List Nat) (index : Nat) (inBounds : index < values.length) :
    valueAt values index ≤ values.sum := by
  induction values generalizing index with
  | nil => simp at inBounds
  | cons head tail induction =>
      cases index with
      | zero => simp [valueAt]
      | succ index =>
          have tailBounds : index < tail.length := by simpa using inBounds
          calc
            valueAt (head :: tail) (index + 1) = valueAt tail index := by
              simp [valueAt]
            _ ≤ tail.sum := induction index tailBounds
            _ ≤ (head :: tail).sum := by simp

/-! ## Orders, fills, and the clearing record -/

/-- One live order of the batch. `receivePerLot` and `deliverPerLot` are
nonnegative claim vectors, one coordinate per outcome; a buy of outcome `i`
receives `e_i`, a sell of outcome `i` delivers `e_i`, and a bundle order
receives several. `limit` is the maximum net quote debit per lot in units of
the price scale, and it is SIGNED: a seller's floor is a negative limit — the
half of the limit the shipping General order record cannot express. -/
structure Order where
  orderId : Nat
  receivePerLot : List Nat
  deliverPerLot : List Nat
  quantity : Nat
  limit : Int
  deriving DecidableEq, Repr

/-- One order with the lots the clearing fills of it. A zero fill is a row
too: an order the clearing leaves unfilled must still be accounted for, so
that no order can be omitted from the certificate. -/
structure Fill where
  order : Order
  lots : Nat
  deriving DecidableEq, Repr

/-- The clearing of one batch. `sets` is the signed number of complete sets
the batch materialises inside the clearing: positive mints, negative merges. -/
structure Clearing where
  outcomeCount : Nat
  scale : Nat
  prices : List Nat
  fills : List Fill
  sets : Int
  deriving DecidableEq, Repr

def Clearing.price (c : Clearing) : Nat → Int :=
  valueAtZ c.prices

/-- Signed per-lot claim flow of one order at outcome `i`. -/
def Order.flow (o : Order) (i : Nat) : Int :=
  valueAtZ o.receivePerLot i - valueAtZ o.deliverPerLot i

/-- Net quote debit per lot at the batch prices, in scale units. Uniform: it
is a function of the price vector and the order alone, never of who else
filled, how much, or when. -/
def Order.perLotDebit (c : Clearing) (o : Order) : Int :=
  sumRange c.outcomeCount (fun i => c.price i * o.flow i)

/-- What one fill does to the batch's claim inventory at outcome `i`. -/
def Fill.contribution (f : Fill) (i : Nat) : Int :=
  f.order.flow i * (f.lots : Int)

/-- Net claims the fills take out of the batch at outcome `i`. -/
def netAt (fills : List Fill) (i : Nat) : Int :=
  (fills.map fun f => f.contribution i).sum

def Clearing.net (c : Clearing) (i : Nat) : Int :=
  netAt c.fills i

/-- Claims the batch is left holding at outcome `i` after minting `sets`. -/
def Clearing.residual (c : Clearing) (i : Nat) : Int :=
  c.sets - c.net i

def Order.validFor (o : Order) (outcomeCount : Nat) : Bool :=
  o.orderId != 0 && o.receivePerLot.length = outcomeCount &&
    o.deliverPerLot.length = outcomeCount && 0 < o.quantity

/-- The per-row conjuncts of the certificate: bounded by the signed quantity,
at or inside the limit when filled at all, and never rationed strictly inside
the limit. The last is the dual-feasibility half of the KKT conditions, and it
is what makes the clearing a competitive one rather than any allocation the
submitter liked. -/
def fillAdmissible (c : Clearing) (f : Fill) : Bool :=
  f.order.validFor c.outcomeCount && f.lots ≤ f.order.quantity &&
    (f.lots = 0 || f.order.perLotDebit c ≤ f.order.limit) &&
    (f.lots = f.order.quantity || f.order.limit ≤ f.order.perLotDebit c)

/-- The whole certificate. The last conjunct is complementary slackness: the
batch may be left holding claims only on an outcome priced at zero. It is
the exact form of "the minted sets are funded", see `collectedQuote_funds_sets`. -/
def Clearing.valid (c : Clearing) : Bool :=
  0 < c.outcomeCount && 0 < c.scale &&
    c.prices.length = c.outcomeCount && c.prices.sum = c.scale &&
    c.fills.all (fillAdmissible c) &&
    decide (c.fills.map fun f => f.order.orderId).Nodup &&
    (List.range c.outcomeCount).all fun i =>
      c.net i ≤ c.sets && (c.price i = 0 || c.net i = c.sets)

/-! ## Unpacking a passing certificate -/

structure Certified (c : Clearing) : Prop where
  outcomes : 0 < c.outcomeCount
  scale : 0 < c.scale
  width : c.prices.length = c.outcomeCount
  simplex : c.prices.sum = c.scale
  rows : ∀ f ∈ c.fills, fillAdmissible c f = true
  distinct : (c.fills.map fun f => f.order.orderId).Nodup
  covered : ∀ i, i < c.outcomeCount → c.net i ≤ c.sets
  slack : ∀ i, i < c.outcomeCount → c.price i = 0 ∨ c.net i = c.sets

theorem certified_of_valid (c : Clearing) (h : c.valid = true) : Certified c := by
  simp only [Clearing.valid, Bool.and_eq_true, decide_eq_true_eq] at h
  obtain ⟨⟨⟨⟨⟨⟨hn, hs⟩, hw⟩, hsum⟩, hrows⟩, hids⟩, hrange⟩ := h
  refine ⟨hn, hs, hw, hsum, List.all_eq_true.mp hrows, hids, ?_, ?_⟩
  · intro i hi
    have := List.all_eq_true.mp hrange i (List.mem_range.mpr hi)
    simp only [Bool.and_eq_true, decide_eq_true_eq] at this
    exact this.1
  · intro i hi
    have := List.all_eq_true.mp hrange i (List.mem_range.mpr hi)
    simp only [Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at this
    exact this.2

/-! ## (a) The price vector is on the exact simplex -/

theorem prices_sum_to_scale (c : Clearing) (h : c.valid = true) :
    c.prices.sum = c.scale :=
  (certified_of_valid c h).simplex

theorem prices_total (c : Clearing) (h : c.valid = true) :
    sumRange c.outcomeCount c.price = (c.scale : Int) := by
  have cert := certified_of_valid c h
  unfold Clearing.price
  rw [← cert.width, sumRange_valueAt, cert.simplex]

theorem price_le_scale
    (c : Clearing) (h : c.valid = true) (i : Nat) (inBounds : i < c.outcomeCount) :
    c.price i ≤ (c.scale : Int) := by
  have cert := certified_of_valid c h
  have listBounds : i < c.prices.length := by rw [cert.width]; exact inBounds
  have := valueAt_le_sum c.prices i listBounds
  rw [cert.simplex] at this
  unfold Clearing.price valueAtZ
  omega

theorem price_nonneg (c : Clearing) (i : Nat) : 0 ≤ c.price i := by
  unfold Clearing.price valueAtZ
  omega

/-! ## (b) Every filled order is at or inside its limit -/

theorem filled_at_or_better
    (c : Clearing) (h : c.valid = true) (f : Fill) (mem : f ∈ c.fills)
    (positive : 0 < f.lots) : f.order.perLotDebit c ≤ f.order.limit := by
  have hf := (certified_of_valid c h).rows f mem
  simp only [fillAdmissible, Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at hf
  rcases hf.1.2 with hz | hle
  · omega
  · exact hle

theorem partial_fill_is_marginal
    (c : Clearing) (h : c.valid = true) (f : Fill) (mem : f ∈ c.fills)
    (partialFill : f.lots < f.order.quantity) : f.order.limit ≤ f.order.perLotDebit c := by
  have hf := (certified_of_valid c h).rows f mem
  simp only [fillAdmissible, Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at hf
  rcases hf.2 with hq | hle
  · omega
  · exact hle

theorem fill_bounded
    (c : Clearing) (h : c.valid = true) (f : Fill) (mem : f ∈ c.fills) :
    f.lots ≤ f.order.quantity := by
  have hf := (certified_of_valid c h).rows f mem
  simp only [fillAdmissible, Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq] at hf
  exact hf.1.1.2

theorem fill_quantity_positive
    (c : Clearing) (h : c.valid = true) (f : Fill) (mem : f ∈ c.fills) :
    0 < f.order.quantity := by
  have hf := (certified_of_valid c h).rows f mem
  simp only [fillAdmissible, Order.validFor, Bool.and_eq_true, Bool.or_eq_true,
    decide_eq_true_eq] at hf
  exact hf.1.1.1.2

/-! ## (c) Full backing: the collected quote is exactly the sets' collateral -/

/-- The net quote the batch collects, in scale units: buyers' debits less
sellers' credits, every one of them at the uniform price. -/
def collectedQuote (c : Clearing) (fills : List Fill) : Int :=
  (fills.map fun f => f.order.perLotDebit c * (f.lots : Int)).sum

theorem netAt_nil (i : Nat) : netAt [] i = 0 := by
  simp [netAt]

theorem netAt_cons (f : Fill) (rest : List Fill) (i : Nat) :
    netAt (f :: rest) i = f.contribution i + netAt rest i := by
  simp [netAt]

/-- One fill's quote is the price-weighted sum of its own claim flow. -/
theorem fill_quote_expands (c : Clearing) (f : Fill) :
    f.order.perLotDebit c * (f.lots : Int) =
      sumRange c.outcomeCount (fun i => c.price i * f.contribution i) := by
  unfold Order.perLotDebit Fill.contribution
  rw [← sumRange_mul_right]
  exact sumRange_congr (fun i _ => (Int.mul_assoc _ _ _))

/-- Exchange of summation: summing quotes over fills equals summing
price × net flow over outcomes. -/
theorem collectedQuote_exchange (c : Clearing) (fills : List Fill) :
    collectedQuote c fills = sumRange c.outcomeCount (fun i => c.price i * netAt fills i) := by
  induction fills with
  | nil =>
    simp only [collectedQuote, List.map_nil, List.sum_nil, netAt_nil, Int.mul_zero]
    exact (sumRange_zero c.outcomeCount).symm
  | cons f rest ih =>
    simp only [collectedQuote, List.map_cons, List.sum_cons, netAt_cons] at ih ⊢
    rw [ih]
    rw [sumRange_congr (f := fun i => c.price i * (f.contribution i + netAt rest i))
      (g := fun i => c.price i * f.contribution i + c.price i * netAt rest i)
      (fun i _ => Int.mul_add _ _ _)]
    rw [sumRange_add, fill_quote_expands]

theorem sets_are_funded (c : Clearing) (h : c.valid = true) :
    sumRange c.outcomeCount (fun i => c.price i * c.net i) = (c.scale : Int) * c.sets := by
  have cert := certified_of_valid c h
  have pointwise : ∀ i, i < c.outcomeCount → c.price i * c.net i = c.sets * c.price i := by
    intro i hi
    rcases cert.slack i hi with hp | hn
    · rw [hp]; simp
    · rw [hn, Int.mul_comm]
  rw [sumRange_congr pointwise, sumRange_mul_left, prices_total c h, Int.mul_comm]

/-- **Full backing.** The quote the batch collects at the uniform prices is
exactly `scale × sets`: a mint of `sets` complete sets is paid for to the atom
by the buyers it serves, and a merge releases exactly what the sellers are
paid. This is census law L1 restricted to the batch's Settlement compartment
and law L8's declared delta for the Hoard class: the Hoard moves by exactly
`sets` units and nothing else in the batch touches it. -/
theorem collectedQuote_funds_sets (c : Clearing) (h : c.valid = true) :
    collectedQuote c c.fills = (c.scale : Int) * c.sets := by
  rw [collectedQuote_exchange]
  exact sets_are_funded c h

theorem residual_nonneg
    (c : Clearing) (h : c.valid = true) (i : Nat) (inBounds : i < c.outcomeCount) :
    0 ≤ c.residual i := by
  have := (certified_of_valid c h).covered i inBounds
  unfold Clearing.residual
  omega

/-- Whatever the batch is left holding is priced at zero by the batch itself:
its value at the clearing prices is nil, coordinate by coordinate. -/
theorem residual_worth_nothing
    (c : Clearing) (h : c.valid = true) (i : Nat) (inBounds : i < c.outcomeCount) :
    c.price i * c.residual i = 0 := by
  rcases (certified_of_valid c h).slack i inBounds with hp | hn
  · rw [hp]; simp
  · unfold Clearing.residual; rw [hn]; simp

/-! ## (d) No arbitrage across outcomes inside one batch -/

/-- What one lot of a bundle costs at the batch prices. -/
def bundlePrice (c : Clearing) (inBundle : Nat → Bool) : Int :=
  sumRange c.outcomeCount (fun i => if inBundle i then c.price i else 0)

/-- What one lot of everything outside the bundle costs. -/
def complementPrice (c : Clearing) (inBundle : Nat → Bool) : Int :=
  sumRange c.outcomeCount (fun i => if inBundle i then 0 else c.price i)

/-- A bundle and its complement together price to exactly one complete set:
assembling a set through the batch and merging it, or splitting one and selling
every piece, nets zero quote. The batch's implied price of a complete set is
its own mint cost, whichever way the set is cut. -/
theorem complement_prices_to_scale
    (c : Clearing) (h : c.valid = true) (inBundle : Nat → Bool) :
    bundlePrice c inBundle + complementPrice c inBundle = (c.scale : Int) := by
  unfold bundlePrice complementPrice
  rw [sumRange_split]
  exact prices_total c h

/-- Uniform price: two orders with the same claim flow pay the same per lot,
whoever placed them and whatever else is in the batch. -/
theorem perLotDebit_uniform
    (c : Clearing) (left right : Order) (sameFlow : ∀ i, left.flow i = right.flow i) :
    left.perLotDebit c = right.perLotDebit c := by
  unfold Order.perLotDebit
  exact sumRange_congr (fun i _ => by rw [sameFlow i])

/-! ## (e) The clearing does not depend on submission order -/

theorem netAt_perm {left right : List Fill} (h : left.Perm right) (i : Nat) :
    netAt left i = netAt right i := by
  induction h with
  | nil => rfl
  | cons x _ ih => simp only [netAt_cons, ih]
  | swap x y l => simp only [netAt_cons]; omega
  | trans _ _ ih₁ ih₂ => exact ih₁.trans ih₂

theorem all_perm {α : Type} {left right : List α} (h : left.Perm right) (p : α → Bool) :
    left.all p = right.all p := by
  induction h with
  | nil => rfl
  | cons x _ ih => simp only [List.all_cons, ih]
  | swap x y l => simp only [List.all_cons]; cases p x <;> cases p y <;> simp
  | trans _ _ ih₁ ih₂ => exact ih₁.trans ih₂

theorem all_congr_mem {α : Type} {l : List α} {p q : α → Bool}
    (h : ∀ x ∈ l, p x = q x) : l.all p = l.all q := by
  induction l with
  | nil => rfl
  | cons x rest ih =>
    simp only [List.all_cons]
    rw [h x List.mem_cons_self, ih (fun y hy => h y (List.mem_cons_of_mem x hy))]

theorem collectedQuote_perm
    (c : Clearing) {left right : List Fill} (h : left.Perm right) :
    collectedQuote c left = collectedQuote c right := by
  rw [collectedQuote_exchange, collectedQuote_exchange]
  exact sumRange_congr (fun i _ => by rw [netAt_perm h i])

/-- **Submission time is not a coordinate of the clearing.** Reordering the
fills — which is all that "who came first inside the batch" can mean once
every fill is at the uniform price — changes neither the verdict nor any
quote. Rationing among orders that are exactly marginal is a tie-break the
design note fixes as pro-rata by content identity, and it is the one place a
priority can re-enter; the theorem says the certificate itself has none. -/
theorem valid_perm (c : Clearing) {fills : List Fill} (h : c.fills.Perm fills) :
    ({ c with fills := fills } : Clearing).valid = c.valid := by
  have hids : decide (fills.map fun f => f.order.orderId).Nodup =
      decide (c.fills.map fun f => f.order.orderId).Nodup :=
    decide_eq_decide.mpr (List.Perm.nodup_iff (h.map fun f => f.order.orderId)).symm
  simp only [Clearing.valid, Clearing.net, Clearing.price]
  rw [← all_perm h, hids]
  congr 1
  apply all_congr_mem
  intro i _
  apply Bool.eq_iff_iff.mpr
  simp only [Bool.and_eq_true, Bool.or_eq_true, decide_eq_true_eq, netAt_perm h i]

/-! ## The optimality certificate -/

/-- Limit-price surplus of an allocation, less the collateral of the sets it
requires. Maximising this over the book is the batch's linear program. -/
def objective (fills : List Fill) (scale : Nat) (sets : Int) : Int :=
  (fills.map fun f => f.order.limit * (f.lots : Int)).sum - (scale : Int) * sets

/-- Dual slack of one order at the prices: how far inside the price its limit
is, or zero. -/
def Order.slack (c : Clearing) (o : Order) : Int :=
  if o.perLotDebit c ≤ o.limit then o.limit - o.perLotDebit c else 0

theorem slack_nonneg (c : Clearing) (o : Order) : 0 ≤ o.slack c := by
  unfold Order.slack
  split <;> omega

theorem limit_le_debit_add_slack (c : Clearing) (o : Order) :
    o.limit ≤ o.perLotDebit c + o.slack c := by
  unfold Order.slack
  split <;> omega

/-- The dual objective: every order's slack at its full signed quantity. It
depends on the book and the prices only, not on the fills. -/
def dualValue (c : Clearing) (fills : List Fill) : Int :=
  (fills.map fun f => f.order.slack c * (f.order.quantity : Int)).sum

theorem sum_map_le {α : Type} (l : List α) (a b : α → Int)
    (h : ∀ x ∈ l, a x ≤ b x) : (l.map a).sum ≤ (l.map b).sum := by
  induction l with
  | nil => exact Int.le_refl 0
  | cons x rest ih =>
    simp only [List.map_cons, List.sum_cons]
    have hx := h x List.mem_cons_self
    have hrest := ih (fun y hy => h y (List.mem_cons_of_mem x hy))
    omega

theorem sum_map_add {α : Type} (l : List α) (a b : α → Int) :
    (l.map fun x => a x + b x).sum = (l.map a).sum + (l.map b).sum := by
  induction l with
  | nil => rfl
  | cons x rest ih => simp only [List.map_cons, List.sum_cons, ih]; omega

/-- **Weak duality.** Any allocation of the book bounded by the signed
quantities and covered by `sets'` sets is worth at most the dual value at any
simplex price vector. -/
theorem weak_duality (c : Clearing) (fills : List Fill) (sets' : Int)
    (bounded : ∀ f ∈ fills, f.lots ≤ f.order.quantity)
    (covered : ∀ i, i < c.outcomeCount → netAt fills i ≤ sets')
    (hsum : sumRange c.outcomeCount c.price = (c.scale : Int)) :
    objective fills c.scale sets' ≤ dualValue c fills := by
  have step1 : (fills.map fun f => f.order.limit * (f.lots : Int)).sum ≤
      (fills.map fun f =>
        f.order.perLotDebit c * (f.lots : Int) + f.order.slack c * (f.lots : Int)).sum := by
    apply sum_map_le
    intro f _
    rw [← Int.add_mul]
    exact Int.mul_le_mul_of_nonneg_right (limit_le_debit_add_slack c f.order) (by omega)
  have step2 : (fills.map fun f => f.order.slack c * (f.lots : Int)).sum ≤ dualValue c fills := by
    apply sum_map_le
    intro f mem
    exact Int.mul_le_mul_of_nonneg_left (by have := bounded f mem; omega) (slack_nonneg c f.order)
  have step3 : collectedQuote c fills ≤ (c.scale : Int) * sets' := by
    rw [collectedQuote_exchange]
    calc sumRange c.outcomeCount (fun i => c.price i * netAt fills i)
        ≤ sumRange c.outcomeCount (fun i => c.price i * sets') :=
          sumRange_le (fun i hi => Int.mul_le_mul_of_nonneg_left (covered i hi) (price_nonneg c i))
      _ = sumRange c.outcomeCount c.price * sets' := sumRange_mul_right _ _ _
      _ = (c.scale : Int) * sets' := by rw [hsum]
  rw [sum_map_add] at step1
  unfold objective collectedQuote at *
  omega

theorem map_order_factor (c : Clearing) (l : List Fill) :
    (l.map fun f => f.order.slack c * (f.order.quantity : Int)) =
      (l.map fun f => f.order).map fun o => o.slack c * (o.quantity : Int) := by
  rw [List.map_map]
  rfl

theorem dualValue_of_book (c : Clearing) {left right : List Fill}
    (sameBook : (left.map fun f => f.order) = right.map fun f => f.order) :
    dualValue c left = dualValue c right := by
  unfold dualValue
  rw [map_order_factor, map_order_factor, sameBook]

/-- A passing certificate attains the dual value: complementary slackness in
both directions plus full backing. -/
theorem certificate_value (c : Clearing) (h : c.valid = true) :
    objective c.fills c.scale c.sets = dualValue c c.fills := by
  have rowwise : ∀ f ∈ c.fills,
      f.order.limit * (f.lots : Int) =
        f.order.perLotDebit c * (f.lots : Int) + f.order.slack c * (f.order.quantity : Int) := by
    intro f mem
    have bounded := fill_bounded c h f mem
    by_cases hz : f.lots = 0
    · -- an unfilled row is either exactly marginal or outside with zero slack
      have hq := fill_quantity_positive c h f mem
      have marginal := partial_fill_is_marginal c h f mem (by omega)
      have hl : (f.lots : Int) = 0 := by omega
      unfold Order.slack
      rw [hl]
      split
      · have heq : f.order.limit = f.order.perLotDebit c := by omega
        rw [heq]; simp
      · simp
    · have better := filled_at_or_better c h f mem (by omega)
      unfold Order.slack
      rw [if_pos better]
      by_cases hfull : f.lots = f.order.quantity
      · rw [hfull, Int.sub_mul]; omega
      · have marginal := partial_fill_is_marginal c h f mem (by omega)
        have heq : f.order.limit = f.order.perLotDebit c := by omega
        rw [heq]; simp
  unfold objective dualValue
  rw [List.map_congr_left rowwise, sum_map_add]
  have funded := collectedQuote_funds_sets c h
  unfold collectedQuote at funded
  omega

/-- **The checked optimality certificate.** A valid clearing is worth at least
as much as every allocation of the same book — any fills bounded by the
signed quantities, covered by any number of sets. This is the theorem that
lets a certified candidate be called an optimal clearing under the stated
objective; a candidate without it remains "best valid submitted". -/
theorem certificate_is_optimal (c : Clearing) (h : c.valid = true)
    (fills : List Fill) (sets' : Int)
    (sameBook : (fills.map fun f => f.order) = c.fills.map fun f => f.order)
    (bounded : ∀ f ∈ fills, f.lots ≤ f.order.quantity)
    (covered : ∀ i, i < c.outcomeCount → netAt fills i ≤ sets') :
    objective fills c.scale sets' ≤ objective c.fills c.scale c.sets := by
  rw [certificate_value c h, ← dualValue_of_book c sameBook]
  exact weak_duality c fills sets' bounded covered (prices_total c h)

/-! ## A batch clears once -/

inductive BatchPhase where
  | collecting
  | closed
  | cleared
  deriving DecidableEq, Repr

structure Batch where
  phase : BatchPhase
  liveOrders : Nat
  clearing : Option Clearing
  deriving DecidableEq, Repr

inductive BatchRefusal where
  | stillCollecting
  | alreadyCleared
  | invalidClearing
  | orderOmitted
  deriving DecidableEq, Repr

instance : DecidableEq (Except BatchRefusal Batch) := fun left right =>
  match left, right with
  | .ok a, .ok b =>
      if h : a = b then isTrue (by rw [h]) else isFalse (by intro e; cases e; exact h rfl)
  | .error a, .error b =>
      if h : a = b then isTrue (by rw [h]) else isFalse (by intro e; cases e; exact h rfl)
  | .ok _, .error _ => isFalse (by intro e; cases e)
  | .error _, .ok _ => isFalse (by intro e; cases e)

/-- Clearing a batch: only a closed batch, only a passing certificate, and only
one that accounts for every live order — a certificate with fewer rows than
the batch has live orders has omitted an order, and omission is how a solver
would evade `partial_fill_is_marginal`. Membership of each row's order in this
batch is the adapter's obligation (the escrowed order record it reads). -/
def Batch.clear? (b : Batch) (c : Clearing) : Except BatchRefusal Batch :=
  match b.phase with
  | .collecting => .error .stillCollecting
  | .cleared => .error .alreadyCleared
  | .closed =>
      if c.valid then
        if c.fills.length = b.liveOrders then
          .ok { b with phase := .cleared, clearing := some c }
        else .error .orderOmitted
      else .error .invalidClearing

theorem clears_once (b post : Batch) (c later : Clearing) (h : b.clear? c = .ok post) :
    post.clear? later = .error .alreadyCleared := by
  unfold Batch.clear? at h
  split at h
  · exact absurd h (by simp)
  · exact absurd h (by simp)
  · split at h
    · split at h
      · simp only [Except.ok.injEq] at h
        rw [← h]
        rfl
      · exact absurd h (by simp)
    · exact absurd h (by simp)

theorem clearing_is_recorded (b post : Batch) (c : Clearing) (h : b.clear? c = .ok post) :
    post.clearing = some c ∧ c.valid = true ∧ c.fills.length = b.liveOrders := by
  unfold Batch.clear? at h
  split at h
  · exact absurd h (by simp)
  · exact absurd h (by simp)
  · split at h
    next hvalid =>
      split at h
      next hcount =>
        simp only [Except.ok.injEq] at h
        exact ⟨by rw [← h], hvalid, hcount⟩
      · exact absurd h (by simp)
    · exact absurd h (by simp)

/-! ## Executable witnesses and hostiles

Two-outcome examples use scale 100. Each `native_decide` is a proof about this
model, not a claim about the SBF adapter. -/

namespace Examples

def buy (id outcome quantity : Nat) (limit : Int) (width : Nat) : Order := {
  orderId := id
  receivePerLot := (List.range width).map fun i => if i = outcome then 1 else 0
  deliverPerLot := List.replicate width 0
  quantity
  limit
}

def sell (id outcome quantity : Nat) (floor : Int) (width : Nat) : Order := {
  orderId := id
  receivePerLot := List.replicate width 0
  deliverPerLot := (List.range width).map fun i => if i = outcome then 1 else 0
  quantity
  limit := -floor
}

/-- Two buyers of opposite outcomes jointly fund one complete set. -/
def jointMint : Clearing := {
  outcomeCount := 2
  scale := 100
  prices := [50, 50]
  fills := [
    { order := buy 1 0 1 60 2, lots := 1 },
    { order := buy 2 1 1 60 2, lots := 1 }
  ]
  sets := 1
}

example : jointMint.valid = true := by native_decide
example : collectedQuote jointMint jointMint.fills = 100 := by native_decide
example : jointMint.net 0 = 1 ∧ jointMint.net 1 = 1 := by native_decide

/-- The same book at a different point of the degenerate optimal face: both
buyers full, so any simplex price inside both limits certifies. The tie-break
among such vectors is the design note's, not the certificate's. -/
def jointMintSkewed : Clearing := { jointMint with prices := [60, 40] }

example : jointMintSkewed.valid = true := by native_decide
example : objective jointMint.fills 100 1 = objective jointMintSkewed.fills 100 1 := by
  native_decide

/-- HOSTILE: an unbacked mint. One buyer, one set claimed, the other outcome
priced at 50 with nobody funding it. Complementary slackness refuses it, and
the arithmetic says why: the batch collected 50 for a set that costs 100. -/
def unbackedMint : Clearing := {
  outcomeCount := 2
  scale := 100
  prices := [50, 50]
  fills := [{ order := buy 1 0 1 60 2, lots := 1 }]
  sets := 1
}

example : unbackedMint.valid = false := by native_decide
example : collectedQuote unbackedMint unbackedMint.fills = 50 := by native_decide
example : unbackedMint.residual 1 = 1 ∧ unbackedMint.price 1 = 50 := by native_decide

/-- HOSTILE: a price vector off the simplex. -/
def offSimplex : Clearing := { jointMint with prices := [50, 40] }

example : offSimplex.valid = false := by native_decide

/-- HOSTILE: a fill worse than its limit. -/
def worseThanLimit : Clearing := {
  jointMint with fills := [
    { order := buy 1 0 1 40 2, lots := 1 },
    { order := buy 2 1 1 60 2, lots := 1 }
  ]
}

example : worseThanLimit.valid = false := by native_decide

/-- A transfer: a seller with a floor of 30 and a buyer with a limit of 50
cross at 40, no set is minted, the other outcome's price is idle. -/
def transfer : Clearing := {
  outcomeCount := 2
  scale := 100
  prices := [40, 60]
  fills := [
    { order := sell 1 0 1 30 2, lots := 1 },
    { order := buy 2 0 1 50 2, lots := 1 }
  ]
  sets := 0
}

example : transfer.valid = true := by native_decide
example : collectedQuote transfer transfer.fills = 0 := by native_decide

/-- HOSTILE: the seller's floor. At 20 the buyer is happy and the seller is
not; the shipping General verifier has no conjunct that refuses this, because
its order record carries only the debit-side limit. -/
def belowFloor : Clearing := { transfer with prices := [20, 80] }

example : belowFloor.valid = false := by native_decide

/-- A merge: sellers of both outcomes deliver a complete set and are paid its
collateral between them, exactly. -/
def jointMerge : Clearing := {
  outcomeCount := 2
  scale := 100
  prices := [45, 55]
  fills := [
    { order := sell 1 0 1 40 2, lots := 1 },
    { order := sell 2 1 1 50 2, lots := 1 }
  ]
  sets := -1
}

example : jointMerge.valid = true := by native_decide
example : collectedQuote jointMerge jointMerge.fills = -100 := by native_decide

/-- Three outcomes where the market prices one at zero. Buyers of 0 and 1
together value a set at 120, so ten sets mint and eight claims of outcome 2
are left with the batch at price zero. The objective is 320. -/
def zeroPriced : Clearing := {
  outcomeCount := 3
  scale := 100
  prices := [60, 40, 0]
  fills := [
    { order := buy 1 0 10 60 3, lots := 10 },
    { order := buy 2 1 10 60 3, lots := 10 },
    { order := buy 3 2 2 60 3, lots := 2 }
  ]
  sets := 10
}

example : zeroPriced.valid = true := by native_decide
example : zeroPriced.residual 2 = 8 ∧ zeroPriced.price 2 = 0 := by native_decide
example : collectedQuote zeroPriced zeroPriced.fills = 1000 := by native_decide
example : objective zeroPriced.fills 100 10 = 320 := by native_decide

/-- The residual-free alternative on the same book — mint two sets and ration
the deep side — is worth 160, and `certificate_is_optimal` says it cannot beat
320. It is also refused outright: buyers 1 and 2 are rationed strictly inside
their limits at every simplex price. -/
def thinRationed : Clearing := {
  zeroPriced with
  prices := [50, 50, 0]
  fills := [
    { order := buy 1 0 10 60 3, lots := 2 },
    { order := buy 2 1 10 60 3, lots := 2 },
    { order := buy 3 2 2 60 3, lots := 2 }
  ]
  sets := 2
}

example : objective thinRationed.fills 100 2 = 160 := by native_decide
example : thinRationed.valid = false := by native_decide

/-- A lone buyer who pays for the whole set: the batch is left holding the
other outcome's claim, priced at zero. Economically certified on its own. -/
def soloBuy : Clearing := {
  outcomeCount := 2
  scale := 100
  prices := [100, 0]
  fills := [{ order := buy 1 0 1 100 2, lots := 1 }]
  sets := 1
}

example : soloBuy.valid = true := by native_decide
example : soloBuy.residual 1 = 1 ∧ collectedQuote soloBuy soloBuy.fills = 100 := by native_decide

/-- HOSTILE: omitting an order. A certified one-row clearing against a batch
with two live orders is refused by count: the missing order might have been
strictly inside the price, and `partial_fill_is_marginal` can only speak about
rows it sees. -/
def closedBatch : Batch := { phase := .closed, liveOrders := 2, clearing := none }

example : closedBatch.clear? unbackedMint = .error .invalidClearing := by native_decide
example : closedBatch.clear? soloBuy = .error .orderOmitted := by native_decide

/-- HOSTILE: a batch that clears twice. -/
example :
    (closedBatch.clear? jointMint).bind (fun post => post.clear? jointMintSkewed) =
      .error .alreadyCleared := by native_decide

example : ({ closedBatch with phase := .collecting } : Batch).clear? jointMint =
    .error .stillCollecting := by native_decide

end Examples

end DClutch.JointClearing
