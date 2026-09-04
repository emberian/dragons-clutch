import Std.Tactic
import DClutchSemantics.AbiSchema

set_option maxHeartbeats 1000000

/-!
# ScoringRuleV1: the Dealer as a bounded-loss scoring-rule participant

The Dealer makes markets from its own capital under sealed rules.  This module
owns the meaning of one such rule: the logarithmic market scoring rule (Hanson),
stated in base two and in Q62 fixed point, the way an SBF accelerator can
evaluate it, and the theorem that makes the sponsor's loss a bound rather than
a promise.

## The shape of the argument

Real LMSR has cost function `C(q) = b · log Σ exp(q_i / b)`.  In a fully
collateralized complete-set market the maker's state is not `q` but its Claims
inventory `inv` (native claim units, one coordinate per ordinary outcome) and
its cash.  Writing `W(inv) = −b · log₂ Σ 2^(−inv_i / b)` — the LMSR cost
function read through `q = −inv` — every property this venue needs follows from
two facts about `W` and nothing else:

* **dominance** — `W(inv) ≤ inv_i` for every coordinate `i`, because the sum
  contains the term for `i`; and
* **the batch rule** — a fill from inventory `inv` to `inv′` at signed net cash
  `debit` is admitted only if `W(inv′) − W(inv) ≥ debit`.

Then `Φ = cash + W(inv)` never decreases across admitted fills, and the Dealer's
terminal wealth in every scenario is at least `Φ`.  Starting from an empty
inventory with the sponsor's deposit `S`, the worst case over the market's life
is `S − Φ₀ = −W(0) = b · log₂ K`.  That is the whole proof, and it is exact —
which is why §1 states it for ANY potential with dominance, and §2 supplies a
concrete integer-valued one that an SBF program computes.

## Why fixed point does not weaken the theorem

The rule compares the CONCRETE integer potential `Ŵ` against the actual cash
the batch moves.  Rounding inside `Ŵ` changes which fills are admitted (an
unmeasurably small spread against the real LMSR), never the sponsor's bound:
the bound is `−Ŵ(0)`, an integer the founding computes and records.  The
approximation error of `Ŵ` against the real `W` is a separate statement (§4),
and it is the only place a transcendental function appears.

## Boundaries

Signatures, account authentication, the Claims Position projection, the
batch's own quote rounding, CPI, and transaction atomicity are adapter
obligations, exactly as in `GeneralClearing` and `DealerScenarioSolvency`.
`u128` refinement of `Nat` is the arithmetic boundary: every operation below
is bounded so that a 128-bit unsigned refinement cannot overflow when
`b ≤ 2^40` and `K ≤ 16`.
-/

namespace DClutch.ScoringRuleV1

def valueAt (values : List Nat) (index : Nat) : Nat :=
  values[index]?.getD 0

/-! ## 1. Any dominated potential bounds the sponsor's loss -/

/-- A potential is an integer valuation of an inventory that never exceeds any
single coordinate.  Dominance is the one property the loss bound uses. -/
structure Potential where
  value : List Nat → Int
  dominance : ∀ (inventory : List Nat) (i : Nat), i < inventory.length →
    value inventory ≤ (valueAt inventory i : Int)

/-- The Dealer's economic state: present cash (collateral atoms, signed so a
theorem can speak about a debt it must never reach) and its native Claims
inventory, one coordinate per ordinary outcome. -/
structure DealerState where
  cash : Int
  inventory : List Nat
  deriving Repr

def DealerState.potential (P : Potential) (s : DealerState) : Int :=
  s.cash + P.value s.inventory

/-- Terminal wealth if ordinary outcome `i` resolves: cash plus the claims paid. -/
def DealerState.wealth (s : DealerState) (i : Nat) : Int :=
  s.cash + (valueAt s.inventory i : Int)

/-- One batch fill seen from the Dealer: it receives `receive`, delivers
`deliver`, and its cash moves by `−debit` (the batch's one canonical rounded
net quote; positive when the Dealer pays). -/
structure Fill where
  receive : List Nat
  deliver : List Nat
  debit : Int
  deriving Repr

def Fill.nextInventory (inventory : List Nat) (f : Fill) : List Nat :=
  List.zipWith (fun held out => held - out)
    (List.zipWith (· + ·) inventory f.receive) f.deliver

def Fill.next (s : DealerState) (f : Fill) : DealerState :=
  { cash := s.cash - f.debit, inventory := f.nextInventory s.inventory }

/-- The Dealer holds none of at least one outcome: no complete set sits in its
inventory as idle cash.  Founding starts here and every admitted fill stays
here, which is also why the Dealer never needs the merge the failure escrow
forecloses. -/
def normalized (inventory : List Nat) : Prop :=
  ∃ j, j < inventory.length ∧ valueAt inventory j = 0

/-- The sealed participation rule, as the accelerator checks it: width,
deliverable, normalized after, and the potential covers the cash. -/
def Fill.admissible (P : Potential) (s : DealerState) (f : Fill) : Prop :=
  f.receive.length = s.inventory.length ∧
  f.deliver.length = s.inventory.length ∧
  (∀ i, i < s.inventory.length →
    valueAt f.deliver i ≤ valueAt s.inventory i + valueAt f.receive i) ∧
  normalized (f.nextInventory s.inventory) ∧
  f.debit ≤ P.value (f.nextInventory s.inventory) - P.value s.inventory

/-- One admitted fill never lowers the potential. -/
theorem potential_step (P : Potential) (s : DealerState) (f : Fill)
    (admitted : f.admissible P s) :
    s.potential P ≤ (f.next s).potential P := by
  obtain ⟨_, _, _, _, covered⟩ := admitted
  simp only [DealerState.potential, Fill.next]
  omega

def run (s : DealerState) : List Fill → DealerState
  | [] => s
  | f :: rest => run (f.next s) rest

def admissiblePath (P : Potential) : DealerState → List Fill → Prop
  | _, [] => True
  | s, f :: rest => f.admissible P s ∧ admissiblePath P (f.next s) rest

/-- Over any admitted life the potential never falls below where it started. -/
theorem potential_life (P : Potential) (s : DealerState) (fills : List Fill)
    (admitted : admissiblePath P s fills) :
    s.potential P ≤ (run s fills).potential P := by
  induction fills generalizing s with
  | nil => exact Int.le_refl _
  | cons f rest ih =>
      obtain ⟨first, later⟩ := admitted
      exact Int.le_trans (potential_step P s f first) (ih _ later)

/-- In every ordinary scenario the Dealer's wealth is at least its potential. -/
theorem wealth_ge_potential (P : Potential) (s : DealerState) (i : Nat)
    (inBounds : i < s.inventory.length) :
    s.potential P ≤ s.wealth i := by
  have := P.dominance s.inventory i inBounds
  simp only [DealerState.potential, DealerState.wealth]
  omega

/-- The sponsor funds `deposit` atoms; the Dealer starts with nothing held. -/
def founded (deposit : Int) (outcomeCount : Nat) : DealerState :=
  { cash := deposit, inventory := List.replicate outcomeCount 0 }

/-- What the founding must deposit: minus the potential of the empty inventory
(`b · log₂ K` for the LMSR, rounded once at founding). -/
def Potential.subsidy (P : Potential) (outcomeCount : Nat) : Int :=
  - P.value (List.replicate outcomeCount 0)

/-- **(a) Bounded loss.** Over any admitted life and in every ordinary
scenario, the sponsor loses at most the subsidy. -/
theorem bounded_loss (P : Potential) (deposit : Int) (outcomeCount : Nat)
    (fills : List Fill)
    (admitted : admissiblePath P (founded deposit outcomeCount) fills)
    (i : Nat) (inBounds : i < (run (founded deposit outcomeCount) fills).inventory.length) :
    deposit - (run (founded deposit outcomeCount) fills).wealth i ≤ P.subsidy outcomeCount := by
  have life := potential_life P _ fills admitted
  have scenario := wealth_ge_potential P _ i inBounds
  simp only [DealerState.potential, founded, Potential.subsidy] at life scenario ⊢
  omega

/-! ### The failure scenario

Under decision 0025 an unresolved market's escrow refunds ordinary claims pro
rata — as founded, `1/K` per claim — so the Dealer's failure wealth is its cash
plus the average of its inventory.  Dominance averaged says that is still at
least the potential, so `bounded_loss` covers the failure selector too. -/

theorem valueAt_cons_succ_map (v : Nat) (rest indices : List Nat) :
    indices.map (fun i => valueAt (v :: rest) (i + 1)) = indices.map fun i => valueAt rest i := by
  apply List.map_congr_left
  intro i _
  simp [valueAt]

theorem sum_valueAt_range (values : List Nat) :
    ((List.range values.length).map fun i => valueAt values i).sum = values.sum := by
  induction values with
  | nil => rfl
  | cons v rest ih =>
      simp only [List.length_cons, List.range_succ_eq_map, List.map_cons, List.map_map,
        List.sum_cons]
      rw [show ((fun i => valueAt (v :: rest) i) ∘ Nat.succ)
          = fun i => valueAt (v :: rest) (i + 1) from rfl]
      rw [valueAt_cons_succ_map, ih]
      simp [valueAt]

theorem length_mul_value_le_sum (P : Potential) (inventory : List Nat) :
    (inventory.length : Int) * P.value inventory ≤ (inventory.sum : Int) := by
  rw [← sum_valueAt_range]
  have general : ∀ n, n ≤ inventory.length →
      (n : Int) * P.value inventory ≤
        (((List.range n).map fun i => valueAt inventory i).sum : Int) := by
    intro n
    induction n with
    | zero => intro _; simp
    | succ n ih =>
        intro bound
        have prev := ih (by omega)
        have last := P.dominance inventory n (by omega)
        rw [List.range_succ, List.map_append, List.sum_append]
        simp only [List.map_cons, List.map_nil, List.sum_cons, List.sum_nil, Nat.add_zero]
        rw [Int.natCast_succ, Int.add_mul, Int.one_mul, Int.natCast_add]
        omega
  exact general inventory.length (Nat.le_refl _)

/-- **(a), failure selector.** Cash plus the pro-rata refund is at least the
potential, so the sponsor's bound holds when the market never resolves. -/
theorem failure_wealth_ge_potential (P : Potential) (s : DealerState)
    (nonempty : 0 < s.inventory.length) :
    s.potential P ≤ s.cash + (s.inventory.sum : Int) / (s.inventory.length : Int) := by
  have bound := length_mul_value_le_sum P s.inventory
  have positive : (0 : Int) < (s.inventory.length : Int) := by omega
  have averaged := Int.le_ediv_of_mul_le positive (by rw [Int.mul_comm]; exact bound)
  simp only [DealerState.potential]
  omega

/-- **(d) Solvency.** With the potential at or above zero the Dealer can pay
for every admitted fill out of present cash: the debit never exceeds cash. -/
theorem solvent (P : Potential) (s : DealerState) (f : Fill)
    (admitted : f.admissible P s) (floor : 0 ≤ s.potential P) :
    f.debit ≤ s.cash := by
  obtain ⟨_, _, _, ⟨j, jBounds, held⟩, covered⟩ := admitted
  have dominated := P.dominance _ j jBounds
  rw [held] at dominated
  simp only [DealerState.potential] at floor
  have zero : ((0 : Nat) : Int) = 0 := rfl
  rw [zero] at dominated
  omega

def withdraw (s : DealerState) (amount : Int) : DealerState :=
  { s with cash := s.cash - amount }

/-- The sponsor's mid-life withdrawal floor: exactly the accumulated potential
may leave, and not one atom more. -/
theorem withdraw_floor (P : Potential) (s : DealerState) (amount : Int) :
    0 ≤ (withdraw s amount).potential P ↔ amount ≤ s.potential P := by
  simp only [withdraw, DealerState.potential]
  omega

/-- The founding state is normalized, so the invariant the rule maintains has a
base case. -/
theorem founded_normalized (deposit : Int) (outcomeCount : Nat)
    (positive : 0 < outcomeCount) :
    normalized (founded deposit outcomeCount).inventory := by
  refine ⟨0, ?_, ?_⟩
  · simp [founded, positive]
  · cases outcomeCount with
    | zero => omega
    | succ n => simp [founded, valueAt, List.replicate]

/-! ## 2. The concrete potential: the base-two LMSR in Q62 fixed point

`Ŵ(inv) = m − ⌈ b · L̂( Σ_i Ê(inv_i − m) ) / 2^62 ⌉` with `m = min inv`,
`Ê(d) ≈ 2^62 · 2^(−d/b)` rounded DOWN at every step and never below `1`, and
`L̂(s) ≈ 2^62 · log₂(s / 2^62)` rounded UP.  Base two is the LMSR with
`b_nat = b / ln 2`; it is chosen because the integer part of `d / b` becomes a
shift and the sponsor's bound becomes `b · log₂ K` — exactly `b` for two
outcomes.

Every rounding direction is chosen so that `Ê ≤ 2^62 · 2^(−d/b)` and
`L̂ ≥ 2^62 · log₂(s/2^62)` hold EXACTLY (§4), which is what lets a reader
name the direction of every error without a floating-point number anywhere.
-/

def fractionBits : Nat := 62
def one : Nat := 2 ^ 62
/-- Added to the floor form of `L̂` so it is an upper bound; 62 floored
squarings each lose under `1.5` units, so `128` covers them. -/
def logSlack : Nat := 128

/-- `table k = ⌊ 2^62 · 2^(−2^(−k)) ⌋` for `k = 0 … 62`, defined as the floor
square-root chain `table 0 = 2^61`, `table (k+1) = isqrt(table k · 2^62)`.
The chain is the SPECIFICATION — pure integer arithmetic, no real number — and
`table_is_the_root_chain` checks these literals against it. -/
def tableList : List Nat := [
  2305843009213693952,
  3260954456333195553,
  3877950241171266237,
  4228934724888366667,
  4416165660797809418,
  4512867096753504462,
  4562008997483360348,
  4586780255215411730,
  4599216278082138664,
  4605446927862164403,
  4608565417332308034,
  4610125453837388244,
  4610905670110601998,
  4611295827762151832,
  4611490918967884500,
  4611588467665893035,
  4611637242788701949,
  4611661630543559955,
  4611673824469352644,
  4611679921444339947,
  4611682969934856343,
  4611684494180870227,
  4611685256304066091,
  4611685637365711254,
  4611685827896545643,
  4611685923161965789,
  4611685970794676600,
  4611685994611032190,
  4611686006519210031,
  4611686012473298963,
  4611686015450343432,
  4611686016938865667,
  4611686017683126785,
  4611686018055257344,
  4611686018241322623,
  4611686018334355263,
  4611686018380871583,
  4611686018404129743,
  4611686018415758823,
  4611686018421573363,
  4611686018424480633,
  4611686018425934268,
  4611686018426661085,
  4611686018427024494,
  4611686018427206198,
  4611686018427297050,
  4611686018427342476,
  4611686018427365189,
  4611686018427376546,
  4611686018427382224,
  4611686018427385063,
  4611686018427386483,
  4611686018427387193,
  4611686018427387548,
  4611686018427387725,
  4611686018427387814,
  4611686018427387858,
  4611686018427387880,
  4611686018427387891,
  4611686018427387897,
  4611686018427387900,
  4611686018427387901,
  4611686018427387902
]

def table (k : Nat) : Nat := tableList[k]?.getD 0

def isRootOf (root square : Nat) : Bool :=
  root * root ≤ square && square < (root + 1) * (root + 1)

def rootChainHolds : Bool :=
  tableList.length == 63 && table 0 == 2 ^ 61 &&
    (List.range 62).all fun k => isRootOf (table (k + 1)) (table k * one)

theorem table_is_the_root_chain : rootChainHolds = true := by native_decide

theorem table_zero : table 0 = 2 ^ 61 := by native_decide

theorem table_step (k : Nat) (bound : k < 62) :
    table (k + 1) * table (k + 1) ≤ table k * one := by
  have holds := table_is_the_root_chain
  simp only [rootChainHolds, Bool.and_eq_true, List.all_eq_true] at holds
  have row := holds.2 k (List.mem_range.mpr bound)
  simp only [isRootOf, Bool.and_eq_true, decide_eq_true_eq] at row
  exact row.1

/-- The chain lemma, with the exponents cleared: `table k / 2^62` is at most
`2^(−2^(−k))`, i.e. `table k ^ (2^k) ≤ 2^(62 · 2^k − 1)`.  This is the whole
reason `Ê` is a one-sided bound; §4 composes it with one floor per step. -/
theorem table_power_bound (k : Nat) (bound : k ≤ 62) :
    table k ^ (2 ^ k) ≤ 2 ^ (62 * 2 ^ k - 1) := by
  induction k with
  | zero => rw [table_zero]; decide
  | succ k ih =>
      have prev := ih (by omega)
      have step := table_step k (by omega)
      have squared : table (k + 1) ^ (2 ^ (k + 1)) = (table (k + 1) * table (k + 1)) ^ (2 ^ k) := by
        rw [Nat.mul_pow, ← Nat.pow_add, Nat.pow_succ, Nat.mul_two]
      rw [squared]
      calc (table (k + 1) * table (k + 1)) ^ (2 ^ k)
          ≤ (table k * one) ^ (2 ^ k) := Nat.pow_le_pow_left step _
        _ = table k ^ (2 ^ k) * one ^ (2 ^ k) := Nat.mul_pow _ _ _
        _ ≤ 2 ^ (62 * 2 ^ k - 1) * one ^ (2 ^ k) := Nat.mul_le_mul_right _ prev
        _ = 2 ^ (62 * 2 ^ (k + 1) - 1) := by
          simp only [one]
          rw [← Nat.pow_mul, ← Nat.pow_add]
          congr 1
          have : 0 < 2 ^ k := Nat.two_pow_pos k
          rw [Nat.pow_succ]
          omega

/-- `2^62 · 2^(−fq / 2^62)` for `fq < 2^62`: one floored multiply per set bit
of the fraction, most significant bit first. -/
def fracProduct (fq : Nat) : Nat :=
  (List.range 62).foldl
    (fun acc j => if fq.testBit (61 - j) then acc * table (j + 1) / one else acc) one

/-- `Ê(d) = max 1 ⌊ 2^62 · 2^(−d/b) ⌋`: the fraction of `d / b` is rounded UP
(so the power is rounded down), the products are floored, the integer part
is a shift, and the result is floored at one so a price is never zero. -/
def exp2Neg (b d : Nat) : Nat :=
  let n := d / b
  let r := d % b
  if 62 ≤ n then 1
  else max 1 (fracProduct ((r * one + b - 1) / b) / 2 ^ n)

/-- `L̂(s)` for `s ≥ 2^62`: integer part from the bit length, fraction by 62
floored squarings, plus `logSlack` so the result is an upper bound. -/
def log2Ceil (s : Nat) : Nat :=
  let n := Nat.log2 s - 62
  let x := s / 2 ^ n
  let step := fun (acc : Nat × Nat) (j : Nat) =>
    let y := acc.2 * acc.2 / one
    if 2 * one ≤ y then (acc.1 + 2 ^ (61 - j), y / 2) else (acc.1, y)
  let r := (List.range 62).foldl step (0, x)
  n * one + r.1 + logSlack

def listMin : List Nat → Nat
  | [] => 0
  | [v] => v
  | v :: w :: rest => min v (listMin (w :: rest))

/-- `Ê` at every coordinate's distance above the minimum. -/
def exponentials (b : Nat) (inventory : List Nat) : List Nat :=
  let m := listMin inventory
  inventory.map fun v => exp2Neg b (v - m)

/-- The rounded-up potential cost, `⌈ b · L̂(Σ Ê) / 2^62 ⌉`, in claim units. -/
def liquidityCost (b : Nat) (inventory : List Nat) : Nat :=
  (b * log2Ceil (exponentials b inventory).sum + one - 1) / one

/-- **`Ŵ`.** The concrete potential. -/
def lmsrValue (b : Nat) (inventory : List Nat) : Int :=
  (listMin inventory : Int) - (liquidityCost b inventory : Int)

theorem listMin_le (inventory : List Nat) (i : Nat) (inBounds : i < inventory.length) :
    listMin inventory ≤ valueAt inventory i := by
  induction inventory generalizing i with
  | nil => simp at inBounds
  | cons v rest ih =>
      cases rest with
      | nil =>
          cases i with
          | zero => simp [listMin, valueAt]
          | succ i => simp at inBounds
      | cons w rest' =>
          cases i with
          | zero =>
              simp only [listMin, valueAt, List.getElem?_cons_zero, Option.getD_some]
              exact Nat.min_le_left _ _
          | succ i =>
              have tail := ih i (by simpa using inBounds)
              simp only [listMin, valueAt, List.getElem?_cons_succ] at tail ⊢
              exact Nat.le_trans (Nat.min_le_right _ _) tail

/-- Dominance is by construction: the minimum minus a nonnegative cost. -/
theorem lmsrValue_dominated (b : Nat) (inventory : List Nat) (i : Nat)
    (inBounds : i < inventory.length) :
    lmsrValue b inventory ≤ (valueAt inventory i : Int) := by
  have := listMin_le inventory i inBounds
  simp only [lmsrValue]
  omega

/-- The base-two Q62 LMSR is a `Potential`, so §1 applies to it verbatim. -/
def lmsrPotential (b : Nat) : Potential :=
  { value := lmsrValue b, dominance := lmsrValue_dominated b }

/-- The sponsor's subsidy for `K` outcomes: `⌈ b · L̂(K · 2^62) / 2^62 ⌉`,
which is `b · log₂ K` rounded up by at most one claim unit. -/
def subsidyOf (b outcomeCount : Nat) : Nat :=
  liquidityCost b (List.replicate outcomeCount 0)

theorem subsidy_is_the_founding_cost (b outcomeCount : Nat) :
    (lmsrPotential b).subsidy outcomeCount = (subsidyOf b outcomeCount : Int) := by
  simp only [Potential.subsidy, lmsrPotential, lmsrValue, subsidyOf]
  cases outcomeCount with
  | zero => simp [listMin, List.replicate]
  | succ n =>
      have : listMin (List.replicate (n + 1) 0) = 0 := by
        induction n with
        | zero => rfl
        | succ n ih => simp [List.replicate, listMin] at ih ⊢
      rw [this]
      simp

/-! ### Complete sets are valued at par

`Ŵ(inv + t·1) = Ŵ(inv) + t` exactly.  This is the fact that makes the
inventory reading of the LMSR agree with the classical `C(q + t·1) = C(q) + t`,
and it is why a fill that adds a complete set to the Dealer would be pure
cash-as-claims: the rule forbids it (`normalized`) instead of pricing it. -/

def shift (inventory : List Nat) (t : Nat) : List Nat :=
  inventory.map (· + t)

theorem listMin_shift (inventory : List Nat) (t : Nat) (nonempty : 0 < inventory.length) :
    listMin (shift inventory t) = listMin inventory + t := by
  induction inventory with
  | nil => simp at nonempty
  | cons v rest ih =>
      cases rest with
      | nil => simp [shift, listMin]
      | cons w rest' =>
          have tail := ih (by simp)
          simp only [shift, List.map_cons, listMin] at tail ⊢
          rw [tail]
          omega

theorem exponentials_shift (b : Nat) (inventory : List Nat) (t : Nat)
    (nonempty : 0 < inventory.length) :
    exponentials b (shift inventory t) = exponentials b inventory := by
  simp only [exponentials]
  rw [listMin_shift inventory t nonempty]
  simp only [shift, List.map_map]
  apply List.map_congr_left
  intro v _
  simp only [Function.comp]
  congr 1
  omega

theorem lmsrValue_shift (b : Nat) (inventory : List Nat) (t : Nat)
    (nonempty : 0 < inventory.length) :
    lmsrValue b (shift inventory t) = lmsrValue b inventory + t := by
  simp only [lmsrValue, liquidityCost, exponentials_shift b inventory t nonempty,
    listMin_shift inventory t nonempty]
  omega

/-! ## 3. Prices: in `(0, 1)`, summing to one, at every state

`p̂_i = 1 + ⌊ Ê_i · (scale − K) / Σ Ê ⌋`, with the shortfall to `scale` added
to the minimum-inventory coordinate (the most expensive outcome, lowest index
on ties).  Floors sum to at most `scale − K`, so the shortfall is nonnegative;
every coordinate is at least one unit and, for `K ≥ 2`, at most
`scale − (K − 1)`.  This is **(b)** by construction rather than by
approximation: the rounding is one named boundary and it never produces a
zero or a one. -/

def indexOfMin : List Nat → Nat
  | [] => 0
  | [_] => 0
  | v :: w :: rest => if v ≤ listMin (w :: rest) then 0 else indexOfMin (w :: rest) + 1

def addAt : List Nat → Nat → Nat → List Nat
  | [], _, _ => []
  | p :: rest, 0, r => (p + r) :: rest
  | p :: rest, i + 1, r => p :: addAt rest i r

def pricesOf (b scale : Nat) (inventory : List Nat) : List Nat :=
  let es := exponentials b inventory
  let s := es.sum
  let k := inventory.length
  let raw := es.map fun e => 1 + e * (scale - k) / s
  addAt raw (indexOfMin inventory) (scale - raw.sum)

theorem one_le_exp2Neg (b d : Nat) : 1 ≤ exp2Neg b d := by
  simp only [exp2Neg]
  split
  · exact Nat.le_refl 1
  · exact Nat.le_max_left _ _

theorem div_add_div_le (a b c : Nat) : a / c + b / c ≤ (a + b) / c := by
  cases c with
  | zero => simp
  | succ c =>
      rw [Nat.le_div_iff_mul_le (Nat.succ_pos c), Nat.add_mul]
      exact Nat.add_le_add (Nat.div_mul_le_self a (c + 1)) (Nat.div_mul_le_self b (c + 1))

theorem sum_map_div_le (values : List Nat) (c s : Nat) :
    (values.map fun e => e * c / s).sum ≤ values.sum * c / s := by
  induction values with
  | nil => simp
  | cons v rest ih =>
      simp only [List.map_cons, List.sum_cons, Nat.add_mul]
      exact Nat.le_trans (Nat.add_le_add_left ih _) (div_add_div_le _ _ _)

theorem length_addAt (values : List Nat) (i r : Nat) :
    (addAt values i r).length = values.length := by
  induction values generalizing i with
  | nil => rfl
  | cons v rest ih =>
      cases i with
      | zero => rfl
      | succ i => simp [addAt, ih]

theorem sum_addAt (values : List Nat) (i r : Nat) (inBounds : i < values.length) :
    (addAt values i r).sum = values.sum + r := by
  induction values generalizing i with
  | nil => simp at inBounds
  | cons v rest ih =>
      cases i with
      | zero => simp [addAt]; omega
      | succ i =>
          have := ih i (by simpa using inBounds)
          simp [addAt, this]
          omega

theorem valueAt_addAt_ge (values : List Nat) (i r j : Nat) :
    valueAt values j ≤ valueAt (addAt values i r) j := by
  induction values generalizing i j with
  | nil => simp [addAt]
  | cons v rest ih =>
      cases i with
      | zero =>
          cases j with
          | zero => simp [addAt, valueAt]
          | succ j => simp [addAt, valueAt]
      | succ i =>
          cases j with
          | zero => simp [addAt, valueAt]
          | succ j => simp only [addAt, valueAt, List.getElem?_cons_succ]; exact ih i j

theorem indexOfMin_lt (inventory : List Nat) (nonempty : 0 < inventory.length) :
    indexOfMin inventory < inventory.length := by
  induction inventory with
  | nil => simp at nonempty
  | cons v rest ih =>
      cases rest with
      | nil => simp [indexOfMin]
      | cons w rest' =>
          simp only [indexOfMin]
          split
          · simp
          · have := ih (by simp)
            simp only [List.length_cons] at this ⊢
            omega

theorem sum_ge_length_of_all_ge_one (values : List Nat)
    (each : ∀ j, j < values.length → 1 ≤ valueAt values j) :
    values.length ≤ values.sum := by
  induction values with
  | nil => simp
  | cons v rest ih =>
      have head := each 0 (by simp)
      have tail := ih fun j hj => by
        have := each (j + 1) (by simpa using hj)
        simpa [valueAt] using this
      simp only [valueAt, List.getElem?_cons_zero, Option.getD_some] at head
      simp only [List.length_cons, List.sum_cons]
      omega

theorem valueAt_le_sum_sub (values : List Nat) (j : Nat)
    (each : ∀ j, j < values.length → 1 ≤ valueAt values j) :
    valueAt values j + (values.length - 1) ≤ values.sum := by
  induction values generalizing j with
  | nil => simp [valueAt]
  | cons v rest ih =>
      have tailAll : ∀ j, j < rest.length → 1 ≤ valueAt rest j := fun j hj => by
        have := each (j + 1) (by simpa using hj)
        simpa [valueAt] using this
      have restSum := sum_ge_length_of_all_ge_one rest tailAll
      have head := each 0 (by simp)
      simp only [valueAt, List.getElem?_cons_zero, Option.getD_some] at head
      cases j with
      | zero =>
          simp only [valueAt, List.getElem?_cons_zero, Option.getD_some, List.length_cons,
            List.sum_cons]
          omega
      | succ j =>
          have := ih j tailAll
          simp only [valueAt, List.getElem?_cons_succ, List.length_cons, List.sum_cons] at this ⊢
          omega

theorem exponentials_length (b : Nat) (inventory : List Nat) :
    (exponentials b inventory).length = inventory.length := by
  simp [exponentials]

theorem exponentials_sum_pos (b : Nat) (inventory : List Nat) (nonempty : 0 < inventory.length) :
    0 < (exponentials b inventory).sum := by
  cases inventory with
  | nil => simp at nonempty
  | cons v rest =>
      simp only [exponentials, List.map_cons, List.sum_cons]
      have := one_le_exp2Neg b (v - listMin (v :: rest))
      omega

theorem sum_map_one_add (values : List Nat) (g : Nat → Nat) :
    (values.map fun e => 1 + g e).sum = values.length + (values.map g).sum := by
  induction values with
  | nil => simp
  | cons v rest ih => simp only [List.map_cons, List.sum_cons, List.length_cons, ih]; omega

theorem raw_sum_le (b scale : Nat) (inventory : List Nat)
    (nonempty : 0 < inventory.length) (room : inventory.length ≤ scale) :
    ((exponentials b inventory).map fun e =>
        1 + e * (scale - inventory.length) / (exponentials b inventory).sum).sum ≤ scale := by
  have positive := exponentials_sum_pos b inventory nonempty
  have floors := sum_map_div_le (exponentials b inventory) (scale - inventory.length)
    (exponentials b inventory).sum
  rw [Nat.mul_div_cancel_left (scale - inventory.length) positive] at floors
  rw [sum_map_one_add, exponentials_length]
  omega

/-- **(b) Prices sum to one.** -/
theorem pricesOf_sum (b scale : Nat) (inventory : List Nat)
    (nonempty : 0 < inventory.length) (room : inventory.length ≤ scale) :
    (pricesOf b scale inventory).sum = scale := by
  have bound := raw_sum_le b scale inventory nonempty room
  simp only [pricesOf]
  rw [sum_addAt _ _ _ (by rw [List.length_map, exponentials_length]; exact indexOfMin_lt _ nonempty)]
  omega

/-- **(b) Every price is positive.** -/
theorem pricesOf_pos (b scale : Nat) (inventory : List Nat) (j : Nat)
    (inBounds : j < inventory.length) :
    1 ≤ valueAt (pricesOf b scale inventory) j := by
  simp only [pricesOf]
  refine Nat.le_trans ?_ (valueAt_addAt_ge _ _ _ j)
  have : j < (exponentials b inventory).length := by rwa [exponentials_length]
  simp only [valueAt, List.getElem?_map]
  rw [List.getElem?_eq_getElem this]
  simp

/-- **(b) No price reaches one** once there are two outcomes: the others hold
at least one unit each. -/
theorem pricesOf_lt (b scale : Nat) (inventory : List Nat) (j : Nat)
    (two : 2 ≤ inventory.length) (room : inventory.length ≤ scale) :
    valueAt (pricesOf b scale inventory) j + (inventory.length - 1) ≤ scale := by
  have total := pricesOf_sum b scale inventory (by omega) room
  have len : (pricesOf b scale inventory).length = inventory.length := by
    simp only [pricesOf]; rw [length_addAt, List.length_map, exponentials_length]
  have each : ∀ j, j < (pricesOf b scale inventory).length →
      1 ≤ valueAt (pricesOf b scale inventory) j := fun j hj =>
    pricesOf_pos b scale inventory j (by rwa [len] at hj)
  have := valueAt_le_sum_sub (pricesOf b scale inventory) j each
  rw [len, total] at this
  exact this

/-! ## 4. The approximation, stated exactly

Core Lean without Mathlib has no real exponential, so the error of the fixed
point against the real LMSR is stated as exact natural-number power
inequalities.  The two `L̂` bounds are now PROVED — each is a bounded
induction over the 62 squarings, composed with `succ_pow_mul_le`, and the only
appeal to computation is `native_decide` over the 63-entry table of §2.  The
two `Ê` bounds are still owed, and BOTH of them moved, because as originally
written they were false:

* **`exp2Neg_below_the_real_value` needs `b ≤ 2^62`.**  `Ê`'s fraction is
  `⌈r·2^62/b⌉`, which reaches `2^62` — a 63rd bit the product never applies —
  as soon as `b > 2^62`.  At `b = 2^63`, `d = 2^63 − 1` that makes `Ê = 2^62`,
  so the claim reads `2^(62·2^63) ≤ 2^(62·2^63 − 2^63 + 1)`: false.  The rule
  record admits `b ≤ 2^40` (`parametersAdmissible`), well inside.
* **`exp2Neg_near_the_real_value` is `2^(−19)` relative, not `2^(−50)`.**  The
  `2^(−50)` was measured on the FRACTION and then attributed to `Ê`, which also
  divides by `2^n`; that final floor costs `2^n/F ≤ 2^40/2^61 = 2^(−21)` over
  the admitted range.  Smallest counterexample to the old statement, found by
  exhaustive search over `b ≤ 3`: `b = 2`, `d = 27` (`n = 13`,
  `Ê = 398065729532860`).  The claim holds over 12,363 sampled `(b, d)` at
  `2^(−22)` and fails at `2^(−25)`; `2^(−19)` is what the proof sketch in the
  lane's scratch actually delivers.

The `L̂` bounds hold exactly as the design note states them.  `logSlack = 128`
pays for the floors: the proof charges `2` for the initial `+1` and `1` per
squaring, `65` in all.  The overshoot proof gives `2^128`, inside the stated
`2^256`; measured overshoot over 1,810 sampled `s` runs from `125.06` to
`128.0` units of `2^(−62)`.

Direction is the load-bearing part and it is by construction: `Ê` rounds
DOWN everywhere (fraction up, products down, floor at one) and `L̂` rounds UP
(floor form plus slack), so `Ŵ` and the recorded subsidy are conservative
for the sponsor.

### Reading the exponents

Every bound is stated by raising both sides to a power that clears the
fixed-point denominator, so no real number appears.  `Ê(d)^b ≤ 2^(62b − d)`
IS `Ê(d)/2^62 ≤ 2^(−d/b)`; `s^(2^62) ≤ 2^(L̂(s) + 62·2^62)` IS
`L̂(s) ≥ 2^62 · log₂(s/2^62)`.  Nothing outside the stated hypotheses is
claimed.
-/


theorem pow_two_pow_succ'' (a k : Nat) : a ^ (2 ^ (k + 1)) = a ^ (2 ^ k) * a ^ (2 ^ k) := by
  rw [Nat.pow_succ, Nat.mul_two, Nat.pow_add]

theorem mul_mul_mul'' (a u v w : Nat) : (a * u) * (v * w) = (a * v) * (u * w) := by
  simp [Nat.mul_comm, Nat.mul_left_comm]

theorem shift_sq' (p s : Nat) : s * (p + p + s) ≤ (p + s) * (p + s) := by
  have h : s * (p + p + s) = s * p + s * (p + s) := by simp only [Nat.mul_add]; omega
  have h2 : (p + s) * (p + s) = p * (p + s) + s * (p + s) := by simp only [Nat.add_mul]
  have h3 : s * p ≤ p * (p + s) := by
    calc s * p = p * s := Nat.mul_comm _ _
      _ ≤ p * (p + s) := Nat.mul_le_mul_left _ (by omega)
  omega

theorem succ_pow_mul_le (x : Nat) : ∀ (k y : Nat), y + 2 ^ k ≤ x →
    (x + 1) ^ (2 ^ k) * y ≤ x ^ (2 ^ k) * x := by
  intro k
  induction k with
  | zero =>
      intro y h
      simp only [Nat.pow_zero, Nat.pow_one]
      obtain ⟨t, rfl⟩ : ∃ t, x = y + 1 + t := ⟨x - (y + 1), by omega⟩
      simp only [Nat.add_mul, Nat.mul_add]; omega
  | succ k ih =>
      intro y h
      have hx : 2 ^ (k + 1) ≤ x := by omega
      have hxpos : 0 < x := by
        have : (0:Nat) < 2 ^ (k+1) := Nat.two_pow_pos (k+1)
        omega
      have key := ih (x - 2 ^ k) (by rw [Nat.pow_succ] at hx; omega)
      have squared :
          (x + 1) ^ (2 ^ (k+1)) * ((x - 2 ^ k) * (x - 2 ^ k))
            ≤ x ^ (2 ^ (k+1)) * (x * x) := by
        rw [pow_two_pow_succ'' (x + 1) k, pow_two_pow_succ'' x k,
          mul_mul_mul'' ((x + 1) ^ (2 ^ k)) ((x + 1) ^ (2 ^ k)) (x - 2 ^ k) (x - 2 ^ k),
          mul_mul_mul'' (x ^ (2 ^ k)) (x ^ (2 ^ k)) x x]
        exact Nat.mul_le_mul key key
      have inner : y * x ≤ (x - 2 ^ k) * (x - 2 ^ k) := by
        obtain ⟨s, rfl⟩ : ∃ s, x = 2 ^ k + 2 ^ k + s := by
          refine ⟨x - 2 ^ (k+1), ?_⟩
          rw [Nat.pow_succ] at hx ⊢; omega
        have e : 2 ^ k + 2 ^ k + s - 2 ^ k = 2 ^ k + s := by omega
        rw [e]
        have hy : y ≤ s := by rw [Nat.pow_succ] at h; omega
        exact Nat.le_trans (Nat.mul_le_mul_right _ hy) (shift_sq' _ _)
      have cancel : ((x + 1) ^ (2 ^ (k+1)) * y) * x ≤ (x ^ (2 ^ (k+1)) * x) * x := by
        calc ((x + 1) ^ (2 ^ (k+1)) * y) * x
            = (x + 1) ^ (2 ^ (k+1)) * (y * x) := by rw [Nat.mul_assoc]
          _ ≤ (x + 1) ^ (2 ^ (k+1)) * ((x - 2 ^ k) * (x - 2 ^ k)) :=
              Nat.mul_le_mul_left _ inner
          _ ≤ x ^ (2 ^ (k+1)) * (x * x) := squared
          _ = (x ^ (2 ^ (k+1)) * x) * x := by rw [Nat.mul_assoc]
      exact Nat.le_of_mul_le_mul_right cancel hxpos

theorem succ_pow_le_two_mul {x k : Nat} (hx : 2 ^ 62 ≤ x) (hk : k ≤ 61) :
    (x + 1) ^ (2 ^ k) ≤ 2 * x ^ (2 ^ k) := by
  have hk2 : (2:Nat) ^ k ≤ 2 ^ 61 := Nat.pow_le_pow_right (by omega) hk
  have h61 : (2:Nat) ^ 61 + 2 ^ 61 = 2 ^ 62 := by decide
  have hxpos : 0 < x := by have := Nat.two_pow_pos 62; omega
  have key := succ_pow_mul_le x k (x - 2 ^ k) (by omega)
  have half : x ≤ 2 * (x - 2 ^ k) := by omega
  have chain : (x + 1) ^ (2 ^ k) * x ≤ (2 * x ^ (2 ^ k)) * x := by
    calc (x + 1) ^ (2 ^ k) * x ≤ (x + 1) ^ (2 ^ k) * (2 * (x - 2 ^ k)) :=
          Nat.mul_le_mul_left _ half
      _ = 2 * ((x + 1) ^ (2 ^ k) * (x - 2 ^ k)) := by rw [Nat.mul_left_comm]
      _ ≤ 2 * (x ^ (2 ^ k) * x) := Nat.mul_le_mul_left _ key
      _ = (2 * x ^ (2 ^ k)) * x := (Nat.mul_assoc _ _ _).symm
  exact Nat.le_of_mul_le_mul_right chain hxpos

/-- Doubling the exponent doubles the cost of the `+1`.  Stated with the
exponents abstract so that no reduction of `a ^ 2^62` is ever attempted. -/
theorem four_mul_of_split (x N M : Nat) (hN : N = M + M) (h : (x + 1) ^ M ≤ 2 * x ^ M) :
    (x + 1) ^ N ≤ 4 * x ^ N := by
  subst hN
  rw [Nat.pow_add, Nat.pow_add]
  calc (x + 1) ^ M * (x + 1) ^ M ≤ (2 * x ^ M) * (2 * x ^ M) := Nat.mul_le_mul h h
    _ = 4 * (x ^ M * x ^ M) := by rw [mul_mul_mul'' 2 (x ^ M) 2 (x ^ M)]

theorem succ_pow_le_four_mul {x : Nat} (hx : 2 ^ 62 ≤ x) :
    (x + 1) ^ (2 ^ 62) ≤ 4 * x ^ (2 ^ 62) := by
  have step := four_mul_of_split x (2 ^ 61 + 2 ^ 61) (2 ^ 61) rfl
    (succ_pow_le_two_mul hx (Nat.le_refl 61))
  have e : (2:Nat) ^ 61 + 2 ^ 61 = 2 ^ 62 := by omega
  rw [e] at step
  exact step


/-! ### The squaring loop -/

def logStep (p : Nat × Nat) (j : Nat) : Nat × Nat :=
  let y := p.2 * p.2 / one
  if 2 * one ≤ y then (p.1 + 2 ^ (61 - j), y / 2) else (p.1, y)

def logPrefix (x m : Nat) : Nat × Nat := (List.range m).foldl logStep (0, x)

theorem log2Ceil_eq (s : Nat) :
    log2Ceil s = (Nat.log2 s - 62) * one
      + (logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + logSlack := rfl

theorem logPrefix_succ (x m : Nat) : logPrefix x (m + 1) = logStep (logPrefix x m) m := by
  simp only [logPrefix, List.range_succ, List.foldl_append, List.foldl_cons, List.foldl_nil]

theorem logPrefix_fst_zero (x : Nat) : (logPrefix x 0).1 = 0 := rfl
theorem logPrefix_snd_zero (x : Nat) : (logPrefix x 0).2 = x := rfl

theorem logPrefix_fst_bit (x m : Nat) (h : 2 * one ≤ (logPrefix x m).2 * (logPrefix x m).2 / one) :
    (logPrefix x (m + 1)).1 = (logPrefix x m).1 + 2 ^ (61 - m) := by
  rw [logPrefix_succ, logStep]
  simp only [h, if_pos]

theorem logPrefix_snd_bit (x m : Nat) (h : 2 * one ≤ (logPrefix x m).2 * (logPrefix x m).2 / one) :
    (logPrefix x (m + 1)).2 = (logPrefix x m).2 * (logPrefix x m).2 / one / 2 := by
  rw [logPrefix_succ, logStep]
  simp only [h, if_pos]

theorem logPrefix_fst_nobit (x m : Nat)
    (h : ¬ (2 * one ≤ (logPrefix x m).2 * (logPrefix x m).2 / one)) :
    (logPrefix x (m + 1)).1 = (logPrefix x m).1 := by
  rw [logPrefix_succ, logStep]
  simp only [h, if_false]

theorem logPrefix_snd_nobit (x m : Nat)
    (h : ¬ (2 * one ≤ (logPrefix x m).2 * (logPrefix x m).2 / one)) :
    (logPrefix x (m + 1)).2 = (logPrefix x m).2 * (logPrefix x m).2 / one := by
  rw [logPrefix_succ, logStep]
  simp only [h, if_false]

theorem logPrefix_range (x : Nat) (hx : 2 ^ 62 ≤ x) (hx2 : x < 2 ^ 63) :
    ∀ m, 2 ^ 62 ≤ (logPrefix x m).2 ∧ (logPrefix x m).2 < 2 ^ 63 := by
  intro m
  induction m with
  | zero => rw [logPrefix_snd_zero]; exact ⟨hx, hx2⟩
  | succ m ih =>
      obtain ⟨lo, hi⟩ := ih
      have hone : (0:Nat) < one := by decide
      have hy1 : 2 ^ 62 ≤ (logPrefix x m).2 * (logPrefix x m).2 / one := by
        refine (Nat.le_div_iff_mul_le hone).mpr ?_
        exact Nat.mul_le_mul lo lo
      have hy2 : (logPrefix x m).2 * (logPrefix x m).2 / one < 2 ^ 64 := by
        refine Nat.div_lt_of_lt_mul ?_
        have h1 : (logPrefix x m).2 * (logPrefix x m).2
            ≤ (2 ^ 63 - 1) * (2 ^ 63 - 1) :=
          Nat.mul_le_mul (by omega) (by omega)
        have h2 : ((2:Nat) ^ 63 - 1) * (2 ^ 63 - 1) < one * 2 ^ 64 := by decide
        omega
      by_cases hb : 2 * one ≤ (logPrefix x m).2 * (logPrefix x m).2 / one
      · rw [logPrefix_snd_bit x m hb]
        constructor
        · refine (Nat.le_div_iff_mul_le (by omega)).mpr ?_
          have : (2:Nat) ^ 62 * 2 = 2 * one := by decide
          omega
        · refine Nat.div_lt_of_lt_mul ?_
          have : (2:Nat) * 2 ^ 63 = 2 ^ 64 := by decide
          omega
      · rw [logPrefix_snd_nobit x m hb]
        have hone63 : 2 * one = 2 ^ 63 := by decide
        exact ⟨hy1, by omega⟩

theorem sq_pow_two (v q : Nat) : (v * v) ^ (2 ^ q) = v ^ (2 ^ (q + 1)) := by
  rw [Nat.mul_pow, ← Nat.pow_add, Nat.pow_succ, Nat.mul_two]

/-- One squaring step, upper direction. -/
theorem log_step_up (v v' c q : Nat) (h : v' * c * one ≤ v * v) :
    v' ^ (2 ^ q) * c ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) ≤ v ^ (2 ^ (q + 1)) := by
  have hone : one ^ (2 ^ q) = 2 ^ (62 * 2 ^ q) := by rw [one, ← Nat.pow_mul]
  calc v' ^ (2 ^ q) * c ^ (2 ^ q) * 2 ^ (62 * 2 ^ q)
      = (v' * c * one) ^ (2 ^ q) := by rw [Nat.mul_pow, Nat.mul_pow, hone]
    _ ≤ (v * v) ^ (2 ^ q) := Nat.pow_le_pow_left h _
    _ = v ^ (2 ^ (q + 1)) := sq_pow_two v q

/-- One squaring step, upper direction, no bit emitted. -/
theorem log_step_up' (v v' q : Nat) (h : v' * one ≤ v * v) :
    v' ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) ≤ v ^ (2 ^ (q + 1)) := by
  have hone : one ^ (2 ^ q) = 2 ^ (62 * 2 ^ q) := by rw [one, ← Nat.pow_mul]
  calc v' ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) = (v' * one) ^ (2 ^ q) := by rw [Nat.mul_pow, hone]
    _ ≤ (v * v) ^ (2 ^ q) := Nat.pow_le_pow_left h _
    _ = v ^ (2 ^ (q + 1)) := sq_pow_two v q

/-- One squaring step, lower direction. -/
theorem log_step_down (v v' c q : Nat) (hv' : 2 ^ 62 ≤ v') (hq : q ≤ 61)
    (h : v * v ≤ (v' + 1) * (one * c)) :
    v ^ (2 ^ (q + 1)) ≤ v' ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) * c ^ (2 ^ q) * 2 := by
  have hone : one ^ (2 ^ q) = 2 ^ (62 * 2 ^ q) := by rw [one, ← Nat.pow_mul]
  calc v ^ (2 ^ (q + 1)) = (v * v) ^ (2 ^ q) := (sq_pow_two v q).symm
    _ ≤ ((v' + 1) * (one * c)) ^ (2 ^ q) := Nat.pow_le_pow_left h _
    _ = (v' + 1) ^ (2 ^ q) * (2 ^ (62 * 2 ^ q) * c ^ (2 ^ q)) := by
        rw [Nat.mul_pow, Nat.mul_pow, hone]
    _ ≤ (2 * v' ^ (2 ^ q)) * (2 ^ (62 * 2 ^ q) * c ^ (2 ^ q)) :=
        Nat.mul_le_mul_right _ (succ_pow_le_two_mul hv' hq)
    _ = v' ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) * c ^ (2 ^ q) * 2 := by
        simp only [Nat.mul_comm, Nat.mul_left_comm, Nat.mul_assoc]


theorem acB (V A B C : Nat) : V * (A * B) * C = V * A * C * B := by
  simp [Nat.mul_comm, Nat.mul_left_comm]

theorem acC (V A B : Nat) : V * A * B = V * B * A := by
  rw [Nat.mul_assoc, Nat.mul_comm A B, ← Nat.mul_assoc]

theorem splitAA (V A : Nat) : V * 2 ^ (A + A) = V * 2 ^ A * 2 ^ A := by
  rw [Nat.pow_add, ← Nat.mul_assoc]

theorem powTwoJoin (V A : Nat) : 2 ^ 2 * V * 2 ^ A = V * 2 ^ (A + 2) := by
  rw [Nat.pow_add]
  simp [Nat.mul_comm, Nat.mul_left_comm]

theorem splitPowMul (V A B : Nat) : V * (2 ^ A) ^ B = V * 2 ^ (A * B) := by
  rw [← Nat.pow_mul]

theorem splitInner (V A B C : Nat) : V * 2 ^ (A + B) * C = V * 2 ^ A * C * 2 ^ B := by
  rw [Nat.pow_add]
  simp [Nat.mul_comm, Nat.mul_left_comm, Nat.mul_assoc]

theorem joinThree (A E : Nat) : 2 ^ A * 2 * 2 ^ E = 2 ^ (A + 1 + E) := by
  rw [Nat.pow_add, Nat.pow_add, Nat.pow_one]

theorem joinTwo (E : Nat) : 2 * 2 ^ E = 2 ^ (1 + E) := by
  rw [Nat.pow_add, Nat.pow_one]

theorem mulPowSplit (V A B : Nat) : (V * 2 ^ A) ^ B = V ^ B * 2 ^ (A * B) := by
  rw [Nat.mul_pow, ← Nat.pow_mul]

/-- **Upper invariant.**  After `m` of the 62 squarings the emitted bits plus
the residue never exceed the true `2^62 · log₂(x/2^62)`. -/
theorem logPrefix_upper (x : Nat) :
    ∀ m, m ≤ 62 →
      (logPrefix x m).2 ^ (2 ^ (62 - m)) * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62)
        ≤ x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (62 - m)) := by
  intro m
  induction m with
  | zero =>
      intro _
      rw [logPrefix_fst_zero, logPrefix_snd_zero, Nat.sub_zero, Nat.zero_add]
      exact Nat.le_refl _
  | succ m ih =>
      intro hm
      have hq : 62 - m = (61 - m) + 1 := by omega
      have hq2 : 62 - (m + 1) = 61 - m := by omega
      have hdbl : 62 * 2 ^ ((61 - m) + 1) = 62 * 2 ^ (61 - m) + 62 * 2 ^ (61 - m) := by
        rw [Nat.pow_succ]; omega
      have hsplit : (logPrefix x m).1 + 2 ^ (61 - m) + 62 * 2 ^ 62
          = 2 ^ (61 - m) + ((logPrefix x m).1 + 62 * 2 ^ 62) := by omega
      have prev := ih (by omega)
      rw [hq, hdbl] at prev
      rw [hq2]
      have hy2 : (logPrefix x m).2 * (logPrefix x m).2 / one * one
          ≤ (logPrefix x m).2 * (logPrefix x m).2 := Nat.div_mul_le_self _ _
      by_cases hb : 2 * one ≤ (logPrefix x m).2 * (logPrefix x m).2 / one
      · rw [logPrefix_fst_bit x m hb, logPrefix_snd_bit x m hb, hsplit]
        have hstep : (logPrefix x m).2 * (logPrefix x m).2 / one / 2 * 2 * one
            ≤ (logPrefix x m).2 * (logPrefix x m).2 := by
          have h1 : (logPrefix x m).2 * (logPrefix x m).2 / one / 2 * 2
              ≤ (logPrefix x m).2 * (logPrefix x m).2 / one := Nat.div_mul_le_self _ _
          exact Nat.le_trans (Nat.mul_le_mul_right _ h1) hy2
        have up := log_step_up ((logPrefix x m).2)
          ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) 2 (61 - m) hstep
        have combined :
            ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) ^ (2 ^ (61 - m))
                * 2 ^ (2 ^ (61 - m) + ((logPrefix x m).1 + 62 * 2 ^ 62))
                * 2 ^ (62 * 2 ^ (61 - m))
              ≤ x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ (62 * 2 ^ (61 - m)) := by
          calc ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) ^ (2 ^ (61 - m))
                  * 2 ^ (2 ^ (61 - m) + ((logPrefix x m).1 + 62 * 2 ^ 62))
                  * 2 ^ (62 * 2 ^ (61 - m))
              = ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) ^ (2 ^ (61 - m))
                  * 2 ^ (2 ^ (61 - m)) * 2 ^ (62 * 2 ^ (61 - m))
                  * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62) := splitInner _ _ _ _
            _ ≤ (logPrefix x m).2 ^ (2 ^ ((61 - m) + 1))
                  * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62) := Nat.mul_le_mul_right _ up
            _ ≤ x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m) + 62 * 2 ^ (61 - m)) := prev
            _ = x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ (62 * 2 ^ (61 - m)) :=
                splitAA _ _
        exact Nat.le_of_mul_le_mul_right combined (Nat.two_pow_pos _)
      · rw [logPrefix_fst_nobit x m hb, logPrefix_snd_nobit x m hb]
        have up := log_step_up' ((logPrefix x m).2)
          ((logPrefix x m).2 * (logPrefix x m).2 / one) (61 - m) hy2
        have combined :
            ((logPrefix x m).2 * (logPrefix x m).2 / one) ^ (2 ^ (61 - m))
                * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m))
              ≤ x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ (62 * 2 ^ (61 - m)) := by
          calc ((logPrefix x m).2 * (logPrefix x m).2 / one) ^ (2 ^ (61 - m))
                  * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m))
              = ((logPrefix x m).2 * (logPrefix x m).2 / one) ^ (2 ^ (61 - m))
                  * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62) :=
                acC _ _ _
            _ ≤ (logPrefix x m).2 ^ (2 ^ ((61 - m) + 1))
                  * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62) := Nat.mul_le_mul_right _ up
            _ ≤ x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m) + 62 * 2 ^ (61 - m)) := prev
            _ = x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ (62 * 2 ^ (61 - m)) :=
                splitAA _ _
        exact Nat.le_of_mul_le_mul_right combined (Nat.two_pow_pos _)


theorem acD (V A B : Nat) : B * V * A = V * (A * B) := by
  simp [Nat.mul_comm, Nat.mul_left_comm]

theorem acF (V A : Nat) : 2 * V * A = V * A * 2 := by
  rw [Nat.mul_comm 2 V, Nat.mul_assoc, Nat.mul_comm 2 A, ← Nat.mul_assoc]

theorem acH (V A B C : Nat) : V * A * B * C = V * (B * C) * A := by
  simp [Nat.mul_comm, Nat.mul_left_comm]

theorem acG (V A B C D : Nat) : V * A * B * C * D = V * (B * C * D) * A := by
  simp [Nat.mul_comm, Nat.mul_left_comm]

theorem lt_div_succ_mul' (a b : Nat) (pos : 0 < b) : a < (a / b + 1) * b := by
  have h := Nat.div_add_mod a b
  have hm := Nat.mod_lt a pos
  have e : (a / b + 1) * b = b * (a / b) + b := by
    rw [Nat.add_mul, Nat.one_mul, Nat.mul_comm]
  omega

/-- One squaring step, lower direction, no bit emitted. -/
theorem log_step_down' (v v' q : Nat) (hv' : 2 ^ 62 ≤ v') (hq : q ≤ 61)
    (h : v * v ≤ (v' + 1) * one) :
    v ^ (2 ^ (q + 1)) ≤ v' ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) * 2 := by
  have hone : one ^ (2 ^ q) = 2 ^ (62 * 2 ^ q) := by rw [one, ← Nat.pow_mul]
  calc v ^ (2 ^ (q + 1)) = (v * v) ^ (2 ^ q) := (sq_pow_two v q).symm
    _ ≤ ((v' + 1) * one) ^ (2 ^ q) := Nat.pow_le_pow_left h _
    _ = (v' + 1) ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) := by rw [Nat.mul_pow, hone]
    _ ≤ (2 * v' ^ (2 ^ q)) * 2 ^ (62 * 2 ^ q) :=
        Nat.mul_le_mul_right _ (succ_pow_le_two_mul hv' hq)
    _ = v' ^ (2 ^ q) * 2 ^ (62 * 2 ^ q) * 2 := acF _ _

/-- **Lower invariant.**  The `logSlack` budget of 128 pays for the floor at
every squaring: after `m` steps the emitted bits are short by at most `2 + m`. -/
theorem logPrefix_lower (x : Nat) (hx : 2 ^ 62 ≤ x) (hx2 : x < 2 ^ 63) :
    ∀ m, m ≤ 62 →
      (x + 1) ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (62 - m))
        ≤ (logPrefix x m).2 ^ (2 ^ (62 - m))
            * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)) := by
  intro m
  induction m with
  | zero =>
      intro _
      have e : 0 + 62 * 2 ^ 62 + (2 + 0) = 62 * 2 ^ 62 + 2 := by omega
      have four : (x + 1) ^ (2 ^ 62) ≤ 2 ^ 2 * x ^ (2 ^ 62) := succ_pow_le_four_mul hx
      rw [logPrefix_fst_zero, logPrefix_snd_zero, Nat.sub_zero, e]
      calc (x + 1) ^ (2 ^ 62) * 2 ^ (62 * 2 ^ 62)
          ≤ (2 ^ 2 * x ^ (2 ^ 62)) * 2 ^ (62 * 2 ^ 62) := Nat.mul_le_mul_right _ four
        _ = x ^ (2 ^ 62) * 2 ^ (62 * 2 ^ 62 + 2) := powTwoJoin _ _
  | succ m ih =>
      intro hm
      have hq : 62 - m = (61 - m) + 1 := by omega
      have hq2 : 62 - (m + 1) = 61 - m := by omega
      have hq61 : 61 - m ≤ 61 := by omega
      have hdbl : 62 * 2 ^ ((61 - m) + 1) = 62 * 2 ^ (61 - m) + 62 * 2 ^ (61 - m) := by
        rw [Nat.pow_succ]; omega
      have hexpb : 2 ^ (61 - m) + 1 + ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m))
          = (logPrefix x m).1 + 2 ^ (61 - m) + 62 * 2 ^ 62 + (2 + (m + 1)) := by omega
      have hexpn : 1 + ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m))
          = (logPrefix x m).1 + 62 * 2 ^ 62 + (2 + (m + 1)) := by omega
      have rng := logPrefix_range x hx hx2 m
      have rng' := logPrefix_range x hx hx2 (m + 1)
      have prev := ih (by omega)
      rw [hq, hdbl] at prev
      rw [hq2]
      have hone : (0:Nat) < one := by decide
      have h1 : (logPrefix x m).2 * (logPrefix x m).2
          < ((logPrefix x m).2 * (logPrefix x m).2 / one + 1) * one :=
        lt_div_succ_mul' _ _ hone
      by_cases hb : 2 * one ≤ (logPrefix x m).2 * (logPrefix x m).2 / one
      · have hv' := (logPrefix_range x hx hx2 (m + 1)).1
        rw [logPrefix_snd_bit x m hb] at hv'
        have h2 : (logPrefix x m).2 * (logPrefix x m).2 / one
            < ((logPrefix x m).2 * (logPrefix x m).2 / one / 2 + 1) * 2 :=
          lt_div_succ_mul' _ _ (by decide)
        have h3 : ((logPrefix x m).2 * (logPrefix x m).2 / one + 1) * one
            ≤ ((logPrefix x m).2 * (logPrefix x m).2 / one / 2 + 1) * 2 * one :=
          Nat.mul_le_mul_right _ (by omega)
        have hacc : ((logPrefix x m).2 * (logPrefix x m).2 / one / 2 + 1) * 2 * one
            = ((logPrefix x m).2 * (logPrefix x m).2 / one / 2 + 1) * (one * 2) := by
          rw [Nat.mul_assoc, Nat.mul_comm 2 one]
        have hstep : (logPrefix x m).2 * (logPrefix x m).2
            ≤ ((logPrefix x m).2 * (logPrefix x m).2 / one / 2 + 1) * (one * 2) := by
          omega
        have down := log_step_down ((logPrefix x m).2)
          ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) 2 (61 - m) hv' hq61 hstep
        rw [logPrefix_fst_bit x m hb, logPrefix_snd_bit x m hb, ← hexpb]
        refine Nat.le_of_mul_le_mul_right ?_ (Nat.two_pow_pos (62 * 2 ^ (61 - m)))
        calc (x + 1) ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ (62 * 2 ^ (61 - m))
            = (x + 1) ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m) + 62 * 2 ^ (61 - m)) :=
              (splitAA _ _).symm
          _ ≤ (logPrefix x m).2 ^ (2 ^ ((61 - m) + 1))
                * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)) := prev
          _ ≤ ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) ^ (2 ^ (61 - m))
                * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ (2 ^ (61 - m)) * 2
                * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)) :=
              Nat.mul_le_mul_right _ down
          _ = ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) ^ (2 ^ (61 - m))
                * (2 ^ (2 ^ (61 - m)) * 2
                  * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)))
                * 2 ^ (62 * 2 ^ (61 - m)) := acG _ _ _ _ _
          _ = ((logPrefix x m).2 * (logPrefix x m).2 / one / 2) ^ (2 ^ (61 - m))
                * 2 ^ (2 ^ (61 - m) + 1
                  + ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)))
                * 2 ^ (62 * 2 ^ (61 - m)) := by rw [joinThree]
      · have hv' := (logPrefix_range x hx hx2 (m + 1)).1
        rw [logPrefix_snd_nobit x m hb] at hv'
        have hstep : (logPrefix x m).2 * (logPrefix x m).2
            ≤ ((logPrefix x m).2 * (logPrefix x m).2 / one + 1) * one := by omega
        have down := log_step_down' ((logPrefix x m).2)
          ((logPrefix x m).2 * (logPrefix x m).2 / one) (61 - m) hv' hq61 hstep
        rw [logPrefix_fst_nobit x m hb, logPrefix_snd_nobit x m hb, ← hexpn]
        refine Nat.le_of_mul_le_mul_right ?_ (Nat.two_pow_pos (62 * 2 ^ (61 - m)))
        calc (x + 1) ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m)) * 2 ^ (62 * 2 ^ (61 - m))
            = (x + 1) ^ (2 ^ 62) * 2 ^ (62 * 2 ^ (61 - m) + 62 * 2 ^ (61 - m)) :=
              (splitAA _ _).symm
          _ ≤ (logPrefix x m).2 ^ (2 ^ ((61 - m) + 1))
                * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)) := prev
          _ ≤ ((logPrefix x m).2 * (logPrefix x m).2 / one) ^ (2 ^ (61 - m))
                * 2 ^ (62 * 2 ^ (61 - m)) * 2
                * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)) :=
              Nat.mul_le_mul_right _ down
          _ = ((logPrefix x m).2 * (logPrefix x m).2 / one) ^ (2 ^ (61 - m))
                * (2 * 2 ^ ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)))
                * 2 ^ (62 * 2 ^ (61 - m)) := acH _ _ _ _
          _ = ((logPrefix x m).2 * (logPrefix x m).2 / one) ^ (2 ^ (61 - m))
                * 2 ^ (1 + ((logPrefix x m).1 + 62 * 2 ^ 62 + (2 + m)))
                * 2 ^ (62 * 2 ^ (61 - m)) := by rw [joinTwo]


/-! ### §4 (c): the two log₂ bounds -/

theorem log2Ceil_x_ge (s : Nat) (inRange : one ≤ s) : 2 ^ 62 ≤ s / 2 ^ (Nat.log2 s - 62) := by
  have hs0 : s ≠ 0 := by have : (0:Nat) < one := by decide
                         omega
  have hL : 62 ≤ Nat.log2 s := (Nat.le_log2 hs0).mpr inRange
  have hle : 2 ^ (Nat.log2 s) ≤ s := Nat.log2_self_le hs0
  refine (Nat.le_div_iff_mul_le (Nat.two_pow_pos _)).mpr ?_
  have e : (2:Nat) ^ 62 * 2 ^ (Nat.log2 s - 62) = 2 ^ (Nat.log2 s) := by
    rw [← Nat.pow_add]; congr 1; omega
  omega

theorem log2Ceil_x_lt (s : Nat) (inRange : one ≤ s) : s / 2 ^ (Nat.log2 s - 62) < 2 ^ 63 := by
  have hs0 : s ≠ 0 := by have : (0:Nat) < one := by decide
                         omega
  have hL : 62 ≤ Nat.log2 s := (Nat.le_log2 hs0).mpr inRange
  have hlt : s < 2 ^ (Nat.log2 s + 1) := Nat.lt_log2_self
  refine Nat.div_lt_of_lt_mul ?_
  have e : (2:Nat) ^ (Nat.log2 s - 62) * 2 ^ 63 = 2 ^ (Nat.log2 s + 1) := by
    rw [← Nat.pow_add]; congr 1; omega
  omega

/-- **(c) `L̂` is an upper bound.**  `s^(2^62) ≤ 2^(L̂(s) + 62·2^62)`, i.e.
`L̂(s) ≥ 2^62 · log₂(s/2^62)`. -/
theorem log2Ceil_above_the_real_value (s : Nat) (inRange : one ≤ s) :
    s ^ (2 ^ 62) ≤ 2 ^ (log2Ceil s + 62 * 2 ^ 62) := by
  have hx := log2Ceil_x_ge s inRange
  have hx2 := log2Ceil_x_lt s inRange
  have hpos : (0:Nat) < 2 ^ (Nat.log2 s - 62) := Nat.two_pow_pos _
  have hlt : s < (s / 2 ^ (Nat.log2 s - 62) + 1) * 2 ^ (Nat.log2 s - 62) :=
    lt_div_succ_mul' _ _ hpos
  have rng := logPrefix_range (s / 2 ^ (Nat.log2 s - 62)) hx hx2 62
  have low := logPrefix_lower (s / 2 ^ (Nat.log2 s - 62)) hx hx2 62 (Nat.le_refl _)
  have hz : 62 - 62 = 0 := rfl
  rw [hz, Nat.pow_zero, Nat.pow_one, Nat.mul_one] at low
  -- low : (x+1)^(2^62) * 2^62 ≤ v * 2^(a + 62*2^62 + (2+62))
  have step : (s / 2 ^ (Nat.log2 s - 62) + 1) ^ (2 ^ 62) * 2 ^ 62
      ≤ 2 ^ ((logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62 + (2 + 62)) * 2 ^ 63 := by
    refine Nat.le_trans low ?_
    rw [Nat.mul_comm]
    exact Nat.mul_le_mul_left _ (by omega)
  have shrink : (s / 2 ^ (Nat.log2 s - 62) + 1) ^ (2 ^ 62)
      ≤ 2 ^ ((logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62 + 65) := by
    refine Nat.le_of_mul_le_mul_right ?_ (Nat.two_pow_pos 62)
    refine Nat.le_trans step (Nat.le_of_eq ?_)
    have e : (logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62 + (2 + 62) + 63
        = (logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62 + 65 + 62 := by omega
    rw [← Nat.pow_add, ← Nat.pow_add, e]
  have raised : s ^ (2 ^ 62)
      ≤ (s / 2 ^ (Nat.log2 s - 62) + 1) ^ (2 ^ 62) * 2 ^ ((Nat.log2 s - 62) * 2 ^ 62) := by
    calc s ^ (2 ^ 62)
        ≤ ((s / 2 ^ (Nat.log2 s - 62) + 1) * 2 ^ (Nat.log2 s - 62)) ^ (2 ^ 62) :=
          Nat.pow_le_pow_left (by omega) _
      _ = (s / 2 ^ (Nat.log2 s - 62) + 1) ^ (2 ^ 62)
            * (2 ^ (Nat.log2 s - 62)) ^ (2 ^ 62) := Nat.mul_pow _ _ _
      _ = _ := splitPowMul _ _ _
  refine Nat.le_trans raised ?_
  refine Nat.le_trans (Nat.mul_le_mul_right _ shrink) ?_
  rw [← Nat.pow_add, log2Ceil_eq, logSlack, one]
  refine Nat.pow_le_pow_right (by omega) ?_
  omega

/-- **(c) `L̂` overshoots by under `2^8`.**  Stated at the tolerance the design
note carries (`2^256`); the proof gives `2^128`, and the measured worst case
over 3,600 states is `128` units of `2^(−62)`. -/
theorem log2Ceil_near_the_real_value (s : Nat) (inRange : one ≤ s) :
    2 ^ (log2Ceil s + 62 * 2 ^ 62) ≤ s ^ (2 ^ 62) * 2 ^ 256 := by
  have hx := log2Ceil_x_ge s inRange
  have hx2 := log2Ceil_x_lt s inRange
  have rng := logPrefix_range (s / 2 ^ (Nat.log2 s - 62)) hx hx2 62
  have up := logPrefix_upper (s / 2 ^ (Nat.log2 s - 62)) 62 (Nat.le_refl _)
  have hz : 62 - 62 = 0 := rfl
  rw [hz, Nat.pow_zero, Nat.pow_one, Nat.mul_one] at up
  -- up : v * 2^(a + 62*2^62) ≤ x^(2^62) * 2^62
  have shrink : 2 ^ ((logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62)
      ≤ (s / 2 ^ (Nat.log2 s - 62)) ^ (2 ^ 62) := by
    refine Nat.le_of_mul_le_mul_right ?_ (Nat.two_pow_pos 62)
    refine Nat.le_trans ?_ up
    rw [Nat.mul_comm]
    exact Nat.mul_le_mul_right _ rng.1
  have hxs : s / 2 ^ (Nat.log2 s - 62) * 2 ^ (Nat.log2 s - 62) ≤ s := Nat.div_mul_le_self _ _
  have raised : (s / 2 ^ (Nat.log2 s - 62)) ^ (2 ^ 62) * 2 ^ ((Nat.log2 s - 62) * 2 ^ 62)
      ≤ s ^ (2 ^ 62) := by
    calc (s / 2 ^ (Nat.log2 s - 62)) ^ (2 ^ 62) * 2 ^ ((Nat.log2 s - 62) * 2 ^ 62)
        = (s / 2 ^ (Nat.log2 s - 62) * 2 ^ (Nat.log2 s - 62)) ^ (2 ^ 62) :=
          (mulPowSplit _ _ _).symm
      _ ≤ s ^ (2 ^ 62) := Nat.pow_le_pow_left hxs _
  have combined : 2 ^ ((logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62
        + (Nat.log2 s - 62) * 2 ^ 62) ≤ s ^ (2 ^ 62) := by
    calc 2 ^ ((logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62
            + (Nat.log2 s - 62) * 2 ^ 62)
        = 2 ^ ((logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62)
            * 2 ^ ((Nat.log2 s - 62) * 2 ^ 62) := Nat.pow_add 2 _ _
      _ ≤ (s / 2 ^ (Nat.log2 s - 62)) ^ (2 ^ 62) * 2 ^ ((Nat.log2 s - 62) * 2 ^ 62) :=
          Nat.mul_le_mul_right _ shrink
      _ ≤ s ^ (2 ^ 62) := raised
  calc 2 ^ (log2Ceil s + 62 * 2 ^ 62)
      = 2 ^ ((logPrefix (s / 2 ^ (Nat.log2 s - 62)) 62).1 + 62 * 2 ^ 62
          + (Nat.log2 s - 62) * 2 ^ 62) * 2 ^ 128 := by
        rw [← Nat.pow_add, log2Ceil_eq, logSlack, one]
        congr 1
        omega
    _ ≤ s ^ (2 ^ 62) * 2 ^ 128 := Nat.mul_le_mul_right _ combined
    _ ≤ s ^ (2 ^ 62) * 2 ^ 256 := Nat.mul_le_mul_left _ (by omega)

/-- `Ê(d)^b ≤ 2^(62b − d)`: the fixed-point exponential never exceeds the real
one, as long as the real one is at least one unit.  `b ≤ 2^62` is the Q62
boundary and is load-bearing — see the counterexample in the section note. -/
theorem exp2Neg_below_the_real_value (b d : Nat) (positive : 0 < b)
    (admitted : b ≤ 2 ^ 62) (inRange : d ≤ 62 * b) :
    exp2Neg b d ^ b ≤ 2 ^ (62 * b - d) := by
  sorry

/-- `Ê(d)` falls short of the real value by less than `2^(−19)` relative:
`2^(81b − d) ≤ ((2^19 + 1) · Ê(d))^b` for `d ≤ 40b`.  The `2^(−50)` the design
note carried is the accuracy of the FRACTION, not of `Ê` after the shift. -/
theorem exp2Neg_near_the_real_value (b d : Nat) (positive : 0 < b)
    (admitted : b ≤ 2 ^ 62) (inRange : d ≤ 40 * b) :
    2 ^ (81 * b - d) ≤ ((2 ^ 19 + 1) * exp2Neg b d) ^ b := by
  sorry

/-! ## 5. The rule record, and the corpus -/

/-- `DCLSCR01`. -/
def ruleMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x53, 0x43, 0x52, 0x30, 0x31]

def ruleVersion : Nat := 1

/-- The sealed rule record: the cost function IS this record.  `liquidity` is
`b` in claim units, `scale` the price denominator the Dealer's candidates must
use, `tolerance` the per-coordinate slack a candidate's price may sit from
`p̂` (price units), and `subsidy` the founding-computed `subsidyOf`, carried
so no reader re-derives it and every reader can check it. -/
inductive RuleField where
  | magic | version | outcomeCount | reserved | marketId | dealerId
  | liquidity | scale | tolerance | subsidy
  deriving DecidableEq, Repr

open DClutch.AbiSchema in
def ruleSchema : List (FieldSpec RuleField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.outcomeCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩, ⟨.marketId, .bytes 32⟩, ⟨.dealerId, .bytes 32⟩,
  ⟨.liquidity, .u64⟩, ⟨.scale, .u64⟩, ⟨.tolerance, .u64⟩, ⟨.subsidy, .u64⟩
]

def ruleBytes : Nat := DClutch.AbiSchema.schemaWidth ruleSchema

theorem rule_record_is_112_bytes : ruleBytes = 112 := by decide

/-- Parameter ranges the arithmetic is bounded for: `K ≤ 16` (the Dealer
profile's provisional width), `1 ≤ b ≤ 2^40`, `K ≤ scale ≤ 2^62`. -/
def parametersAdmissible (outcomeCount b scale : Nat) : Bool :=
  2 ≤ outcomeCount && outcomeCount ≤ 16 && 1 ≤ b && b ≤ 2 ^ 40 &&
    outcomeCount ≤ scale && scale ≤ one

/-- The subsidy carried by a rule record must be the one founding computes. -/
def subsidyRecorded (outcomeCount b recorded : Nat) : Bool :=
  recorded == subsidyOf b outcomeCount

/-- Corpus: the Rust reference and the exact Python model agree with these. -/
example : exp2Neg (2 ^ 30) 0 = one := by native_decide
example : exp2Neg (2 ^ 30) (2 ^ 29) = 3260954456333195553 := by native_decide
example : exp2Neg (2 ^ 30) (2 ^ 30 - 1) = 2305843010702216160 := by native_decide
example : exp2Neg (2 ^ 30) (3 * 2 ^ 30 + 7) = 576460749698509580 := by native_decide
example : exp2Neg (2 ^ 30) (62 * 2 ^ 30) = 1 := by native_decide
example : log2Ceil one = 128 := by native_decide
example : log2Ceil (2 * one) = 4611686018427388032 := by native_decide
example : log2Ceil (5 * one) = 10708003330985790334 := by native_decide
example : subsidyOf (2 ^ 30) 2 = 1073741825 := by native_decide
example : subsidyOf (2 ^ 30) 5 = 2493151308 := by native_decide
example : subsidyOf (2 ^ 20) 3 = 1661954 := by native_decide
example : lmsrValue (2 ^ 30)
    [12345, 2 ^ 30 / 3 + 12345, 2 * 2 ^ 30 / 3 + 12345, 2 ^ 30 + 12345, 4 * 2 ^ 30 / 3 + 12345]
    = -1859070063 := by native_decide
example : pricesOf (2 ^ 30) one [0, 0] = [2305843009213693952, 2305843009213693952] := by
  native_decide
example : pricesOf (2 ^ 30) one [0, 2 ^ 30] = [3074457345618258603, 1537228672809129301] := by
  native_decide
example : pricesOf (2 ^ 30) one
    [12345, 2 ^ 30 / 3 + 12345, 2 * 2 ^ 30 / 3 + 12345, 2 ^ 30 + 12345, 4 * 2 ^ 30 / 3 + 12345]
    = [1388848156715294885, 1102329512734177518, 874919514253179301, 694424078357647441,
       551164756367088759] := by native_decide

/-- The hostile the rule refuses by name: a candidate whose prices do not sum
to the scale.  `pricesOf` cannot produce one; this pins that a hand-written
vector which does is not `pricesOf` of any state. -/
example : ([2305843009213693952, 2305843009213693951] : List Nat).sum ≠ one := by native_decide

end DClutch.ScoringRuleV1
