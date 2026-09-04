# The Dealer as a bounded-loss scoring-rule participant

Design lane SCORING-DEALER, 2026-09-04. Tree `/Users/ember/dev/dclutch`, read at
`554a29119`; no program changed. The Lean owner is
`formal/dclutch-semantics/DClutchSemantics/ScoringRuleV1.lean` (this commit).
The compute figures in §6 are **program-test evidence on a real SBF ELF** built
from a scratch probe in this lane's private scratch directory (the probe is the
arithmetic of §1 verbatim, Q62/u128, under `sol_log_compute_units`), not
devnet and not mainnet evidence. The companion notes: BATCH-SPINE
(`MECHANISM_BATCH_SPINE_2026_09_04.md`, landed) names the Dealer *"a
participant that places one demand schedule per batch; it is why every batch
clears"* and owes this note the schedule's shape and the exact price-impact
slope; JOINT-CLEARING's Lean (`JointClearingV1.lean`, in the tree, its note not
yet landed) states the clearing as a KKT certificate over signed limit orders.
§2 composes with both by name.

**In one paragraph.** A sponsor — the founder, or anyone — deposits
`Ŝ = ⌈b·log₂K⌉` claim units of collateral into a Dealer whose one sealed rule is
Hanson's logarithmic market scoring rule, stated in base two. The Dealer holds
that cash and a Claims inventory `inv` (one coordinate per ordinary outcome,
starting empty). Its marginal price for outcome `i` is
`p_i = 2^(−inv_i/b) / Σ_j 2^(−inv_j/b)`: always defined, strictly inside
`(0, 1)`, summing to one. Each batch, a solver may fill the Dealer to any
inventory `inv′` provided the batch's uniform price is the Dealer's marginal
price at `inv′` and the cash the Dealer pays is covered by the increase of one
integer-valued potential `Ŵ(inv)` that an SBF program computes exactly. Then
`Φ = cash + Ŵ(inv)` never falls, the Dealer's wealth in every scenario is at
least `Φ`, and the sponsor's loss over the market's whole life is at most `Ŝ`.
That sentence is `bounded_loss` in the Lean, proven, with no real number in it.
What the sponsor buys for `Ŝ` is a price series that exists in every batch —
the forecast — and whatever the flow pays above it; there is no fee, no
inventory beyond the rule's state, no quote the rule did not derive, and the
rule is sealed at founding under the capability seal like every Dealer rule
before it.

---

## 1. The cost function, exactly

### 1.1 Base two, and the inventory reading

LMSR: `C(q) = b_e · ln Σ_i exp(q_i / b_e)`, `q_i` the maker's net units sold of
outcome `i`, marginal prices `∂C/∂q_i`, worst-case loss `b_e · ln K`. The same
family in base two with `b = b_e · ln 2`:

```text
C(q) = b · log₂ Σ_i 2^(q_i / b),      p_i = 2^(q_i/b) / Σ_j 2^(q_j/b),      loss ≤ b · log₂ K
```

Base two is chosen for the arithmetic: the integer part of `q_i / b` becomes a
shift, and the sponsor's bound is `b` exactly for two outcomes, `2b` for four.

In a fully collateralized complete-set market the maker never holds a signed
position. Selling `q_i` of outcome `i` is minting `N` complete sets and
delivering; what the Dealer holds is `inv_i = N − q_i ≥ 0`, its Claims Position
(`crates/dclutch-dealer-scenario-kernel/src/lib.rs:63-78`, the projection the
scenario kernel already borrows and refuses to mirror). Since
`C(q + t·1) = C(q) + t`, read `C` through `q = N·1 − inv`:

```text
W(inv) := −b · log₂ Σ_i 2^(−inv_i / b)          (= C(−inv);  C(q) = N + W(inv))
p_i(inv) = 2^(−inv_i/b) / Σ_j 2^(−inv_j/b)      (= ∂W/∂inv_i)
```

`W` is what the rule computes. Two facts about it carry every property below:

- **dominance** `W(inv) ≤ inv_i` for every `i` (the sum contains the term for
  `i`, so `log₂ Σ ≥ −inv_i / b`); and
- **par** `W(inv + t·1) = W(inv) + t` (a complete set is worth exactly one).

The classical bound is one line from dominance. Over any sequence of fills
priced by `W` — the Dealer's cash moves by `W(inv′) − W(inv)` at each — the
telescoped cash is `S + W(inv) − W(0)`; in scenario `i` the inventory pays
`inv_i ≥ W(inv)`, so wealth `≥ S − W(0)... ` with `W(0) = −b · log₂ K`, i.e.
loss `≤ b · log₂ K`. §2 replaces "priced by `W`" with "the batch's uniform
price covered by `W`", and §3(a) shows the bound survives that.

### 1.2 The fixed point: `Ŵ`, `Ê`, `L̂`

Everything is Q62 in `u128`. `one = 2^62`. `m = min_i inv_i`.

```text
Ê(d)   =  max(1, ⌊ 2^62 · 2^(−d/b) ⌋)                 d = inv_i − m ≥ 0
S      =  Σ_i Ê(inv_i − m)                             ∈ [2^62, K·2^62]
L̂(S)   ≈  2^62 · log₂(S / 2^62), rounded UP
Ŵ(inv) =  m − ⌈ b · L̂(S) / 2^62 ⌉                    (claim units, an Int)
Ŝ      =  −Ŵ(0) = ⌈ b · L̂(K·2^62) / 2^62 ⌉           the subsidy, computed once at founding
```

**`Ê`** (`exp2Neg` in the Lean; `exp2_neg` in the probe). Write `d = n·b + r`.
If `n ≥ 62` the result is `1` (the price floor, §1.3). Else the fraction
`fq = ⌈ r · 2^62 / b ⌉` — one `u128` division, a shift when `b` is a power of
two — is consumed bit by bit: `F := 2^62; for k in 1..=62: if bit (62−k) of fq:
F := ⌊F · T[k] / 2^62⌋`, then `Ê = max(1, F >> n)`. The table is

```text
T[0] = 2^61,   T[k] = isqrt(T[k−1] · 2^62)        so T[k] = ⌊2^62 · 2^(−2^(−k))⌋ within 2 units
```

— an integer chain with no real number in its definition. The Lean pins the 63
literals to the chain by `table_is_the_root_chain` (`native_decide`), and
`table_power_bound` proves `T[k]^(2^k) ≤ 2^(62·2^k − 1)` from it by induction,
which is the whole reason `Ê` is a one-sided bound. **Every rounding in `Ê` is downward**:
the fraction is rounded up (so the power is rounded down), each product floors,
and the floor at `1` only raises a value the real function has already taken
below one unit. Measured against the exact rational model over 3,600 random
states: the relative shortfall of `Ê` is under `2^(−22)` everywhere, largest
where `Ê` is small and one unit is a large fraction of it, and its effect on
`Ŵ` is under one claim unit; the shortfall of the table itself is under two
units at every level.

**`L̂`** (`log2Ceil`). `n = ⌊log₂ S⌋ − 62` from the bit length; mantissa
`x = S >> n ∈ [2^62, 2^63)`; 62 floored squarings `x := ⌊x²/2^62⌋`, emitting a
fraction bit and halving whenever `x ≥ 2^63`; result `n·2^62 + frac + 128`.
The 62 truncations lose under `1.5` units each in `log₂`, so `128` makes it an
**upper bound**; measured overshoot over 2,000 random arguments: `< 128` units
of `2^(−62)`. `L̂(2^62) = 128` (not zero) and it does not matter: it is added to
a cost that `⌈·⌉` then divides by `2^62`, and `b · 128 / 2^62 < 1` for every
admitted `b`.

**Ranges.** `2 ≤ K ≤ 16` (the Dealer ABI's provisional `maxOutcomes`,
`DealerLiquidityAbi.lean:19`; the arithmetic itself has no K ceiling below
`2^66/2^62`), `1 ≤ b ≤ 2^40` claim units, price `scale` with `K ≤ scale ≤ 2^62`
(`parametersAdmissible`). Under them: `S < 2^66`, `x² < 2^126`, `F · T[k] <
2^124`, `b · L̂ < 2^40 · 2^66 = 2^106`, `Ê · (scale − K) < 2^124` — every
intermediate fits `u128`, and the SBF build carries `overflow-checks` so a
violation is a fault, not a wrap. `b` is in **claim units**; where claim units
are not collateral atoms the conversion is the authenticated
`ProductBasisV3::payout_scale` the scenario kernel already names as its one
scale (`dclutch-dealer-scenario-kernel/src/lib.rs:15-20`); as built, one
ordinary claim redeems for exactly one atom (`b0deb2902`), and the design keeps
the boundary named rather than assuming it away.

**Error, and where it goes.** `|Ŵ(inv) − W(inv)| < 1` claim unit over every
state the model sampled (`K ∈ {2,3,5,16}`, `b ∈ {2^20, 2^30, 2^40}`); the
recorded subsidy exceeds `b·log₂K` by at most one unit. This error changes
*which fills the rule admits* — a spread against the real LMSR of about one
claim unit per fill — and does not enter the sponsor's bound at all, because
the bound is the integer `Ŝ` the founding records (§3(a)). The Lean states the
one-sided facts as exact natural-number power inequalities
(`exp2Neg_below_the_real_value`, `log2Ceil_above_the_real_value`) and the
two-sided slack (`…_near_the_real_value`), all four `sorry` for the reason the
module gives: core Lean without Mathlib has no real exponential, the statements
are the composition of the chain lemma with one floor per step, and the
bounded inductions are owed. They are the only `sorry` in the module.

### 1.3 Prices

```text
p̂_i = 1 + ⌊ Ê_i · (scale − K) / S ⌋        for every i
p̂_i* += scale − Σ_i p̂_i                  i* = the lowest index attaining m
```

Floors sum to at most `scale − K`, so the residual is nonnegative; every
coordinate is at least one unit; for `K ≥ 2` no coordinate exceeds
`scale − (K−1)`. Proven: `pricesOf_sum`, `pricesOf_pos`, `pricesOf_lt`. The
rule's default `scale` is `2^62` — the batch's `PriceVector.scale` is
per-candidate data (`GeneralClearing.lean:128-135`; `Clearing.scale` in
`JointClearingV1.lean`), so a Dealer's candidates simply use the rule's. The
floor at one unit is the only place the rule departs from the real function:
past a skew of `62b` on one outcome the real price is below `2^(−62)` and the
rule quotes one unit — a market at that probability is a corner, not a hostile.

**The exact slope, owed to BATCH-SPINE §2(d)(ii).** With one outcome moving
and the others fixed, `log₂(p_i / p_j) = (inv_j − inv_i) / b` exactly: a fill
of `q` claims on outcome `i` moves its log₂-odds against every other outcome
by exactly `q / b`. In probability units the slope is
`∂p_i/∂inv_i = −(ln 2 / b) · p_i (1 − p_i) ≤ (ln 2 / 4) · (1/b) ≈ 0.173 / b`,
so one order of size `q` moves a price by at most `0.173 · q / b` — tighter
than the `q / b` the spine assumed, and exact rather than worst-region.

---

## 2. Participation in a batch

The Dealer is **one participant with one schedule order per batch**; it never
takes a taker, never quotes outside a batch, and never carries state the
projection of its Claims Position and its sealed record do not already
determine. Composition with the spine is at `PlaceOrder`: the Dealer's
schedule order is *"a plain order verified by General"* in the spine's phrase,
except that its rows are computed rather than signed.

**The order.** `ScheduleOrder(rule, inv, escrow)`: the sealed rule record
(§7), the Dealer's inventory at admission (a projection, refused stale by
revision exactly as `positionAdmissible` does today,
`DealerScenarioCollateral.lean:47-52`), and the escrow the spine's admission
rule requires: *the exact worst case* (0010 §2). For a scoring Dealer the
worst case has a closed form — the most it can ever be asked to pay in one
batch is `−Ŵ(inv)` (§3(d)) — so the escrow is `−Ŵ(inv)` claim units of its
present cash, never the whole capital, and never the subsidy `Ŝ` once the
inventory has moved.

**The fill.** A candidate names, for the Dealer's row, a nonnegative `receive`
vector `y`, a nonnegative `deliver` vector `z`, and takes the price vector
`p̂` and the debit from the batch. The accelerator (or the kernel linked into
whichever program verifies the row, §7) admits the row iff

| | conjunct | refusal |
|---|---|---|
| R0 | widths equal `K`; `z_i ≤ inv_i`; `y_i · z_i = 0` (canonical form) | width / deliverable / non-canonical |
| R1 | `inv′ = inv + y − z` has `min_i inv′_i = 0` | `NotNormalized` — the Dealer never holds a complete set as idle cash |
| R2 | `\|p̂_i − p̂_i(inv′)\| ≤ τ` for every `i`, at the rule's `scale` | `OffSchedule` — the batch price is not the Dealer's marginal price at its post-fill state |
| R3 | `debit ≤ Ŵ(inv′) − Ŵ(inv)` where `debit` is the batch's one canonical net quote for `(receive y, deliver z)` at `p̂` (`roundedQuoteFor`: receipt rounded up, delivery rounded down, once per order) | `Uncovered` — the cash asked exceeds what the potential allows |

`τ` is sealed with the rule (default: one price unit). It exists because `Ŵ`
is an integer and `p̂` a rounded vector: a fill of a few claim units at the
exact marginal price can fail R3 by the unit of rounding, and `τ` lets the
solver shade the price by one unit toward the Dealer so that it passes. Fills
of more than about `√(2b)` claim units pass at `τ = 0` because the LMSR's own
curvature `‖y‖²/(2b)` exceeds the rounding. `Fill.admissible` in the Lean is
R0–R1 and R3; R2 is the schedule's *willingness* and is stated on the price
vector, not on the potential.

**What the solver does.** Off chain it inverts the rule — `inv′_i = b ·
log₂(p_max / p_i)` for a target price, then rounds to the lattice and takes
`p̂ := p̂(inv′)` — so the Dealer's fill absorbs whatever imbalance the other
orders leave, and the batch's complete-set move is whatever `Clearing.valid`
requires of the total. **No logarithm runs on chain.** The verifier evaluates
`Ê` and `L̂` only (§6); the log is the solver's cost. A clearing always exists:
`y = z = 0` with `p̂ = p̂(inv)` is admissible for every state, so a batch with
only the Dealer in it clears at the Dealer's current price — the spine's "it
is why every batch clears" is R2 at the null fill.

**Composition with `JointClearingV1`.** Its `Order` carries a signed `limit`
and its certificate `Clearing.valid` is KKT: every filled row at or inside its
limit, no row rationed strictly inside it, minted sets exactly funded, claims
left over only on outcomes priced at zero. The Dealer's row is not a limit
order; it is a demand *function*, and its KKT stationarity is R2: the price is
(within `τ`) the gradient of `Ŵ` at the chosen point, which is the first-order
condition of `max_inv′ [Ŵ(inv′) − p̂·(inv′ − inv)]`. So the composed certificate
is `Clearing.valid` over the signed orders **plus** R0–R3 over the Dealer's
row, with the Dealer's `y − z` entering `netAt` like any fill's contribution.
`certificate_is_optimal` (JOINT-CLEARING's theorem) is then optimality over
the signed book *given* the Dealer's fill as data; a joint optimality statement
that includes the Dealer's `Ŵ`-utility holds up to the rounding `Ŵ` carries
against a concave function and is **owed** as one theorem across the two
modules, not claimed here. Nothing in `GeneralClearing.lean` is edited; the
spine's `ScheduleOrder` is this row.

**Merges, and the escrow that forecloses them.** R1 is not a convenience.
`b0deb2902` established that under decision 0025 the failure coordinate sits
in an escrow, so *no* Position but the escrow's can merge a complete set, and
the Dealer would have no way to turn an idle complete set back into cash. R1
makes the question moot: the Dealer's inventory has a zero coordinate at
founding (`founded_normalized`) and after every admitted fill, so it never
holds a complete set and never needs the merge. Buying a complete set from the
Dealer is expressed as the Dealer delivering all outcomes it holds and the
batch minting; selling one to it is the Dealer receiving the complement. The
`par` law (`lmsrValue_shift`, proven) is what makes those two readings the
same trade.

---

## 3. Properties

### (a) Bounded loss — **proven**

`bounded_loss`: for any `Potential` (a `List Nat → Int` with dominance), any
deposit, any `K`, any admissible path of fills, and any ordinary outcome `i`,
`deposit − wealth_i ≤ subsidy = −value(0)`. For the concrete rule,
`lmsrPotential b` is a `Potential` (`lmsrValue_dominated`) and
`subsidy_is_the_founding_cost` equates its subsidy to `subsidyOf b K`, the
integer the founding records. The proof is `potential_step` (R3 is literally
"`Φ` does not fall"), `potential_life` (induction on the path),
`wealth_ge_potential` (dominance), and `omega`.

Why it survives uniform pricing rather than cost-function pricing: R3 compares
the batch's actual debit against `Ŵ`'s increment, so whatever the uniform
price is, the Dealer's cash change is at least the potential's. For the real
`W` this is implied by concavity — `W(inv) ≤ W(inv′) + ∇W(inv′)·(inv − inv′)`
gives `−p·(inv′ − inv) ≥ W(inv′) − W(inv)` at the marginal price — which is
why a solver can always find an admissible fill near `p̂(inv′)`; but the
theorem does not use it. The bound also holds in the **failure scenario**:
under 0025 the escrow refunds ordinary claims pro rata, `1/K` per claim as
founded, so the Dealer's wealth is `cash + (Σ_i inv_i)/K ≥ cash + min_i inv_i
≥ Φ`; the same dominance, averaged — `failure_wealth_ge_potential`, proven.

### (b) Prices in `(0, 1)` summing to one — **proven**

`pricesOf_sum`, `pricesOf_pos`, `pricesOf_lt`, for every state, every
`2 ≤ K ≤ scale`. By construction, §1.3.

### (c) Myopic incentive compatibility — **proof sketch, real-valued**

For a price-taking trader with belief `π ∈ Δ^K` facing the schedule: the
trader's problem in the batch is `max_x (π − p̂)·x` over its limit orders, so it
buys `i` iff `π_i > p̂_i` and sells iff `π_i < p̂_i`, and it stops exactly where
the batch price equals its belief. Because the Dealer's schedule is strictly
monotone (log-odds move linearly in inventory, §1.3) and always present, the
marginal trader in a batch with only itself and the Dealer sets `p̂ = π`
component-wise up to the lattice and `τ`; that is the classical result
(`argmax_q′ [π·(q′−q) − (C(q′) − C(q))]` has `∇C(q′) = π`, concavity of the
objective). Two honest limits. First, a **strategic** trader in a
uniform-price batch shades — it pays the post-fill price for the whole
quantity, so its optimum stops short of `π` by the classical demand-reduction
of uniform-price auctions; the spine names this as the residual manipulation
and the slope in §1.3 bounds it. Second, "myopic": a trader who expects to
trade again has the usual incentive to under-reveal; LMSR is myopically, not
dynamically, incentive compatible, and this note claims no more.

### (d) Full backing — **proven where it is arithmetic; named where it is a law**

- `solvent`: with `Φ ≥ 0`, every admissible fill's debit is at most present
  cash. So the Dealer never needs credit, and `Φ ≥ 0` — the withdrawal floor
  of §3(e) — is the invariant the LP routes maintain.
- The sponsor's deposit is **TradingPrincipal of the Dealer** (the compartment
  the LP Open route already funds), never Hoard principal; it enters the Hoard
  only as the batch's `Materialize` mints complete sets at par, the same
  movement every participant's quote makes, and leaves the Hoard only through
  a merge the batch performs or a redemption at resolution. `HoardPrincipal →
  TradingPrincipal` as a direct pair is not a movement this design makes, and
  the census holds it: **L8** (`tools/gauntlet/journey/src/ledger.rs:1004-1012`,
  per-class conservation derived from the vault's own seeds), **L4**
  (`hoard ≥ max_i supply(i) · unit`, `:31-36`), **L3** (Positions sum to the
  aggregate, `:25-29`), and **L2** (Hoard movement declared, `:16-23`). Not one
  lamport leaves the class: the Dealer's Position is complete-set claims; its
  cash is its own compartment; the batch's escrow is `Settlement(order_id)`
  under 0010 §2. AGENTS.md's law — *Hoard principal is never fees, rent,
  bounty, insurance, work funding, reserve, or treasury capital* — is not
  touched because `b` is none of those: it is a participant's own trading
  capital, at risk to the participant alone.

### (e) What the sponsor gets, and the refusal that keeps this a participant

**No fee.** Decision 0024 rules no protocol take and no beneficiary; this
design adds no fee of its own. The sponsor's return is the flow's payment
above the schedule — a trader who moves the price to `π` and is wrong pays
the Dealer; one who is right is paid — bounded below by `−Ŝ`, and above by
nothing. What the sponsor buys for `Ŝ` is the **information**: a price series
that exists in every batch, is a chain fact, and moves by exactly `q / b` in
log-odds per `q` claims. A founder who wants a forecast funds `b`; a founder
who wants a fee does not have that product here.

**The refusal.** The Dealer is a participant and not a venue because: there
is **no discretionary quoting** — every price is `p̂(inv′)` of the rule and
R2 refuses any other; there is **no inventory beyond the rule's state** — the
inventory is the Claims Position the batch settled and R1 forbids the one
thing (a complete set) the rule does not price; there is **no order book and
no taker** — the Dealer trades only inside a batch at the batch's uniform
price, which is the spine's line against an AMM as a product (INTENT §4: *"the
rule is against dClutch being an AMM, not against ever admitting one as a
venue"* — this admits exactly *"a formally admitted convex cost-function
maker"*, the fourth venue of the twelve-item ceiling); and **the rule is
sealed at founding** — `b`, `scale`, `τ`, `Ŝ` are one content-addressed record
under `authenticate_capability_seal_v3` (decision 0005;
`programs/dclutch-trading-sbf/src/hot_v3/seal.rs:789`), so a request naming
another has no seal.

**Withdrawal.** `withdraw_floor`: the sponsor may take out exactly `Φ` and
not one unit more — accumulated profit, or unneeded subsidy once inventory
has moved and `−Ŵ(inv) < Ŝ`, never the committed part. The LP Remove route
gains this conjunct; it is a strengthening of the scenario kernel's locked
floor (`CandidateBelowLockedFloor`, which bounds per-scenario equity) because
`Φ` bounds every scenario at once.

---

## 4. LS-LMSR, and why not

Othman–Sandholm's liquidity-sensitive rule replaces `b` with `α · Σ_i q_i`, so
liquidity grows with volume, prices sum to slightly more than one, and the
maker can profit. Three reasons it is refused here, in order of weight.

1. **It gives up (b).** Its prices sum to `1 + α·(…) > 1`; the batch's
   `PriceVector` is an exact simplex (`validFor`, `coordinates.sum = scale`)
   and the complete-set move is what makes the clearing arbitrage-free across
   outcomes. A participant whose schedule is off the simplex either forces a
   second price scale into the clearing or is a taker of a spread the batch
   then has to fund. The spine's whole point is one simplex per batch.
2. **The sponsor already is the liquidity sensitivity.** LS-LMSR exists
   because a subsidy-free maker needs volume to deepen; here `b` is paid by a
   named party at founding and is governable (0024's amendment): a market that
   wants more depth is re-founded with a larger `b`, or a second sponsor adds
   capital through the LP Add route into the same rule. Liquidity is a
   decision with an owner, not a side effect of flow.
3. **Its bound is worse, not better.** LS-LMSR's worst-case loss is not fixed
   at founding — it depends on the volume path — and the tree's rule is that a
   bound is labelled mathematical, chain-derived, measured or provisional.
   `Ŝ` is mathematical. `α · Σ q` is a promise about the future.

What is kept from the LS-LMSR literature is the observation that a fixed `b`
over-prices early and under-prices late. The remedy consistent with the
simplex is not a rule change but the LP Add/Remove routes: `b` is a record
field with a delay and an event under the parameter surface, and a sponsor who
watches the market thin adds capital — `b` scales linearly with the deposit
(`Ŝ = b log₂ K`), so a top-up is a re-computation of `Ŝ` and `Ŵ` at the same
inventory, and `Φ` is unchanged by it (both sides move by the added cash).

---

## 5. Hostiles

Each is a test to be written red first and named by discriminant, in the
band the rule record's program takes.

| hostile | what happens | where it is refused |
|---|---|---|
| a state whose prices do not sum to one | `pricesOf` cannot produce one (`pricesOf_sum`); a hand-written candidate carrying one is refused by `PriceVector.validFor` / `Clearing.valid` before the Dealer's row is read | the batch verifier, existing |
| a valid simplex that is not the Dealer's price | `\|p̂ − p̂(inv′)\|_∞ > τ` | R2 `OffSchedule` |
| a trade exceeding the bound | there is no such trade: a fill whose debit exceeds `Ŵ(inv′) − Ŵ(inv)` is refused (R3 `Uncovered`), and `bounded_loss` says no admissible path reaches a loss above `Ŝ`. The test is adversarial: the most damaging admissible path a solver can construct, with the ledger's L-laws and the Dealer's cash re-read at every boundary, never below `S − Ŝ` | R3, and the theorem |
| a fill that leaves a complete set in the Dealer | `min inv′ > 0` | R1 `NotNormalized` |
| a rule changed after sealing | a request naming another `b`, `scale`, `τ` or `Ŝ` has no seal account; a record whose `subsidy` is not `subsidyOf b K` fails `subsidyRecorded` at founding | the seal (0005), and founding |
| a sponsor withdrawing `b` mid-life | `w > Φ` | LP Remove, `withdraw_floor` |
| a stale inventory | the row's `inv` revision differs from the Claims Position's | `positionAdmissible`, existing |
| a price pump by round trips | each leg is an admissible fill; `Φ` does not fall on any of them (`potential_step`), so the pumper pays the rounding spread each way and the sponsor gains it — the pump is the sponsor's income | the theorem |
| `Ê = 0` (a zero price) | unrepresentable: `Ê ≥ 1` (`one_le_exp2Neg`), `p̂ ≥ 1` (`pricesOf_pos`) | by construction |
| arithmetic overflow | every intermediate bounded under `parametersAdmissible` (§1.2); the ELF carries overflow checks | the parameter check at founding |

---

## 6. The compute price

Measured on a real SBF ELF in `solana-program-test` (`platform-tools v1.53`,
`solana-program 3.0.0`, `solana-program-test 4.3.0-beta.2`), the arithmetic of
§1.2 as written — Q62, `u128`, the 62-step table product and the 62-squaring
log — between two `sol_log_compute_units` calls, so each figure includes one
syscall (`≈ 100`). Inputs: `b = 2^30`, `scale = 2^62`, inventories spread over
`[0, 2.4b]`, a receive vector of `7·10^6` claims on alternate outcomes.

| region | CU |
|---|---:|
| baseline, two adjacent syscalls | 102 |
| `Ê`, one fraction bit set (`d = b/2`) | 2,557 |
| `Ê`, 20 fraction bits set (`b = 2^20`, `d = b−1`) | 6,236 |
| `Ê`, 40 fraction bits set (`b = 2^40`, `d = b−1`) | 10,483 |
| `L̂` (any `K`) | 8,663 – 9,662 |
| prices `p̂`, K=2 / 3 / 5 / 16 | 5,569 / 8,245 / 25,767 / 52,453 |
| potential `Ŵ`, K=2 / 3 / 5 / 16 | 14,657 / 16,593 / 32,937 / 52,555 |
| **full participation check** (`Ŵ(inv)`, `Ŵ(inv′)`, `p̂(inv′)`, R1), K=2 / 3 / 5 / 16 | **38,949 / 46,468 / 92,543 / 182,952** |

Per set fraction bit `Ê` costs about 194 CU (one `u128` multiply, shift and
table load) over a floor of about 2,900 (the 62-iteration bit walk), so the
**worst case** of `Ê` is `≈ 14,800` with all 62 bits set, and the worst full
check is about `3K · 14,800 + 2 · 9,700 + K · 500`: **108k / 153k / 242k** at
K = 2 / 3 / 5, 730k at K = 16. Two reductions, both structural:

- **share `Ê(inv′)`** between `Ŵ(inv′)` and `p̂(inv′)` — the probe recomputes
  it; the rule needs `2K` exponentials, not `3K`: worst **79k / 109k / 168k**.
- **carry `Ŵ(inv)`** — it is the previous batch's `Ŵ(inv′)`, a fact the
  program already computed, and the compute note's law (*"the expensive thing
  was never the check; it was a fact re-derived by someone who already had
  it"*) says give it a carrier: a `potential` field on the row's state, written
  at settlement and checked at the next admission by revision. Then a batch
  costs `K` exponentials and one log: worst **39k / 54k / 84k**, typical
  **15k / 17k / 33k** (the `Ŵ` rows above).

**Against the ceiling.** The transaction ceiling is 1,400,000 (1,399,700
granted). The Dealer's selector-9 scenario trade — the thing the spine deletes
and this row replaces — cost 131,790 of evaluation inside a 420,514 accelerator
leg (`DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md`, the accelerator table); the
scoring row's evaluation is cheaper than that at every `K ≤ 5` measured and
even in the worst case at `K ≤ 3`. The batch's verify row,
`VerifyCandidateRow`, costs 579,699 at width 258
(`programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs:3632-3633`)
and fits the 200,000 default at width 1; its cost at `K = 5` is **not
measured here and is owed**, but it is bounded above by the width-258 figure,
which leaves at least 820k for the Dealer's row — room for the measured check
at every `K`, for the worst case with shared `Ê` at every `K`, and for the
naive worst case at `K ≤ 5`. If the evaluation stays in the Dealer accelerator
behind a CPI, its prelude (still `≈ 200k` after `742d7b7be`'s move, §"the
209,000") is the dominant cost, not the arithmetic; §7 prefers linking the
kernel into the verifier for that reason.

**The `u64` alternative, priced.** Q32 in `u64` — one native multiply per
step — would cut each `Ê` to roughly 300–600 CU and the full check at K = 5 to
under 10k, at the cost of `|Ŵ − W|` rising to about `b · 2^(−24)` claim units
(64 units at `b = 2^30`), a spread that small fills would feel. It is a
sealed-parameter choice (`fractionBits`), not a design fork; the Lean is
written at 62 and the theorems do not depend on the width.

---

## 7. Build list

**Reused, unchanged.**
- The Dealer's sealed-rule accelerator and its campaign: the LP lifecycle —
  Open, equity Add (`DEALER_EQUITY_CONTRIBUTE_P0..P2_SELECTOR_V3 = 1..3`),
  Remove (`REDEEM_P0..P2 = 4..6`), Close (`v3_equity_operator.rs:209-219`;
  `DCLMLP03` / `DCLMEQ03`) — is the sponsor's fund and withdraw, 31/31 on real
  ELFs. Add re-computes `Ŝ` and `Ŵ` at the same inventory; Remove gains the
  `withdraw_floor` conjunct.
- The Claims Position projection and its staleness refusal
  (`positionAdmissible`); `ProductBasisV3::payout_scale` as the one unit
  boundary.
- The General batch: `PlaceOrder`, the escrow at admission, `Clearing.valid`
  (JOINT-CLEARING) over the signed book, `Materialize`, `Distribute`, the
  cadence triple.
- The capability seal (0005) over the rule record; the census laws.

**New.**
- `ScoringRuleV1` record, **112 bytes**, Lean-emitted
  (`rule_record_is_112_bytes`; `ruleSchema`): magic `DCLSCR01`, version,
  `outcomeCount`, reserved, `marketId`, `dealerId`, `liquidity` (`b`),
  `scale`, `tolerance` (`τ`), `subsidy` (`Ŝ`). `subsidyRecorded` and
  `parametersAdmissible` are the founding conjuncts. The 63-entry table is
  Lean-owned data the emitter writes into the kernel crate, as the reservation
  state's coordinates are today (`EmitDealerScenarioReservationStateV1Rust.lean`).
- A `no_std`, `no_alloc` kernel crate `dclutch-scoring-rule-kernel`: `Ê`,
  `L̂`, `Ŵ`, `p̂`, and `admit(rule, inv, y, z, p̂, debit) → Result<(), Refusal>`
  with the refusals of §5 as named discriminants in the program's band. The
  probe is its first draft (about 150 lines).
- The **schedule row** in the batch verifier: `ScheduleOrder` as the spine
  names it — one row, computed not signed, escrow `−Ŵ(inv)`, verified by the
  kernel. Whether the kernel links into the General accelerator or the Dealer
  accelerator is invoked by CPI from the verify step is the one open
  placement question; §6's prelude figure argues for the link.
- The `potential` carrier on the row's state (§6), written at settlement.
- Hostile tests per §5, each red first.
- Deleted with the spine: selector 9 and the seven-step checkpoint chain
  (`DealerScenarioTradeV4Abi.lean`, the reservation route), as BATCH-SPINE's
  table already lists. `DealerScenarioSolvency.lean`'s `floorAdmissible`
  survives in spirit as `withdraw_floor`.

**Cohort.** No program moves under cohort-16 (the mechanism agenda's rule).
The scoring row is a General verifier change and a Dealer LP change, so it
rides the cohort that carries the spine's batch-keyed selection and
JOINT-CLEARING's signed limits — cohort-17 at the earliest — with the
`ScoringRuleV1` record founded into the Dealer capability of one market and a
solver that inverts the rule in the load simulator.

**`b` against the failure escrow and the recovery ladder.** The sponsor's
capital is the Dealer's cash plus its ordinary claims. Under 0027 a market
whose primary source goes silent walks the funded ladder; batches keep
clearing or are frozen by the Market's lifecycle policy — a product choice,
not this note's — and the Dealer's `Φ` is unchanged either way. When the
ladder exhausts into the failure selector, 0025's escrow refunds ordinary
claims pro rata, so the Dealer receives `(Σ inv_i)/K ≥ min inv_i` for its
claims and keeps its cash: **an unresolved market returns at least `S − Ŝ`
to the sponsor**, by the same theorem, and returns `S` exactly if nobody
traded. The LP Close route then pays the sponsor. What it does *not* do is
return `Ŝ` when the flow has taken it — the subsidy was the price of the
forecast, and an outage does not refund a forecast that was made. The founder
**bond** (the sibling design) is a different compartment with a different
law: it is forfeited on exhaustion and paid to holders; `b` is never
forfeited, only lost to the flow. They must not share a vault, and L8 is the
instrument that keeps them apart.

---

## 8. Owed

1. The four `sorry` bounds in §1.2, as bounded inductions over the chain.
2. The joint optimality statement across `JointClearingV1` and
   `ScoringRuleV1` (§2), once JOINT-CLEARING's note lands.
3. `VerifyCandidateRow` at `K = 5`, measured, so §6's headroom is a number
   and not a bound.
4. The trade-off `τ` versus `fractionBits`, measured on the load simulator:
   how often a solver needs the shade.
