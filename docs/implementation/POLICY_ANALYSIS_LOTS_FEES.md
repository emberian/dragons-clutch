# Policy analysis: fractional payout claims, fee policy, economics-lab alignment

Status: **PROPOSED analysis only.** Nothing in this document canonizes a policy
(per the standing rule that policy is not canonized by an analysis lane). Every
statement below is either MODEL (a property of a named host-only model or of the
landed offline code, checkable by the cited experiment) or PROPOSED (a candidate
policy or design lean awaiting the user's decision). Leans are labeled as leans;
they are not decisions. Wave-2 lanes should implement the experiment matrix in
§5 tonight so the morning decision is made from executed data.

Scope: (1) fractional payout claims, (2) fee policy, (3) economics-lab/kernel
alignment. Out of scope (owned by the sibling batch lane): the batch relation
itself, residual-pair settlement, and AON/min-fill policy. Where a payout or fee
policy imposes a requirement *on* the batch relation, this document names the
requirement as an interface obligation and stops there.

Ground truth read for this analysis:

- `crates/clutch-kernel/src/lib.rs` — `PayoutVector`/`PayoutSet` (common
  denominator `D`, weights sum exactly to `D`), `split` (any positive quantity),
  `redeem_internal`/`redeem_external` (refuse with `RemainderRequired` when
  `quantity * weight mod D != 0`), `required_collateral` (ceiling rounding),
  `merge` (Active phase only).
- `docs/implementation/ADVERSARIAL_REVIEW_V0.md` §P1-A (exit-dead fractional
  claims) and §P1-E (lab/kernel market-set mismatch; vertical fee has no payer,
  wrong base, no allocation, no carry).
- `docs/ECONOMICS.md` §5, `docs/FEE_GEOMETRY.md` — the documented fee thesis.
- `docs/PROTOCOL.md` §Redeem — "a frozen redemption lot `D / gcd(weight_i, D)`;
  or explicit persistent remainder credits. Silent floor rounding or routing
  dust to treasury is prohibited."
- `research/economics/model.py`, `research/economics/test_lab.py` (28 tests),
  `docs/implementation/ECONOMICS_LAB.md`.
- `research/vertical-model/src/lib.rs` — `clear_batch_inner` fee accounting.
- `apps/static-client/terms.json` — the `rounding` terms string.
- `docs/EVIDENCE_MATRIX.md` — `P-SOLV-01`, `P-FEE-01`.

---

## 1. Fractional payout claims

### 1.1 The exact trap (MODEL, already executed as kernel test)

`PayoutVector::validate` admits any weights with `sum(w_i) = D`, `w_i <= D`,
common `D` across the set. `split` mints any positive `q` of every outcome.
`redeem` pays `q * w_i / D` only when the division is exact, else refuses with
`RemainderRequired`.

For `weights=[1,1], D=2`: split 1 atom, resolve. Each one-unit claim has
`1*1 mod 2 = 1` — both redemptions refuse forever. Collateral 1 satisfies the
(ceiling-rounded) solvency invariant but can never leave. **Solvent but
exit-dead.** Issuance admits states that redemption cannot unwind.

Three structural facts frame every candidate:

1. **Losing claims always exit.** `w_i = 0` gives numerator 0, remainder 0, for
   any quantity. Exit-liveness problems exist only for `0 < w_i < D`.
2. **A complete set never remainders — jointly.** `sum_i q * w_i = q * D ≡ 0
   (mod D)`. The P1-A trap is *exactly one complete set*; only the per-outcome
   redemption API makes it dead. This motivates the orthogonal primitive in
   §1.5, which is safe under every candidate.
3. **The lot for outcome `i` is fully determined by number theory.**
   `q * w ≡ 0 (mod D)` iff `(D / gcd(w, D)) | q`. Under a payout *set* `P`
   (resolution unknown at issuance time), the binding lot is the lcm over
   vectors, which collapses (for `x, y | D`, `lcm(D/x, D/y) = D/gcd(x,y)`) to:

   ```text
   g_i = gcd(D, {v_i : v in P, v_i != 0})        (g_i = D if all v_i in {0, D})
   L_i = D / g_i                                  per-outcome redemption lot
   L_split = lcm_i L_i = D / gcd_i g_i            complete-set (split/merge) lot
   ```

   So lots are **per-outcome, computed across the whole payout set** — a pure
   function of `PayoutSet`, requiring **zero stored state**. For one-hot-only
   sets every `L_i = 1` and `L_split = 1`; lots are the degenerate no-op there.

### 1.2 Candidate (a): one-hot-only admission in V1 (PROPOSED)

Admission rule: every vector's weights lie in `{0, D}` (with `sum = D` this
forces exactly one weight equal to `D`). Fractional vectors are reserved for a
later, versioned ambiguity-policy schema.

- **Enforcement locus.** Two sub-options:
  - (a1) tighten `PayoutVector::validate` in the kernel (new or reused error,
    e.g. `InvalidPayoutWeights`). The kernel is the semantic authority; no
    adapter can forget the gate; `RemainderRequired` becomes provably
    unreachable code (keep it as defense in depth).
  - (a2) keep the kernel general and gate at adapter/Market admission. The trap
    stays reachable through the kernel API forever; every adapter and model
    must re-implement the gate. Lean within (a): **(a1)**, with the fractional
    grammar returning under an explicit payout-set schema version.
- **Solvency:** unchanged (already holds; ceiling rounding becomes exact since
  one-hot liabilities are integral).
- **Exit-liveness:** total. Every redemption pays `q` or `0` exactly.
- **State cost:** zero. **Kernel API impact:** one validation tightening.
- **Token-2022 composability:** perfect. External Eggs transfer and redeem in
  arbitrary quantities; no lot to enforce at a boundary that cannot enforce it.
- **Batch-relation interface obligation:** none. Fills stay unquantized.
- **Adversarial:** no new surface. Dust griefing reduces to ordinary token dust.
- **Cost (must be said plainly):** V1 loses fractional fallbacks, including the
  `compatible_payout` equal-weight-over-compatible-set family that
  `docs/ECONOMICS.md` §7 prefers for data failure. Under (a), the V1 failure
  policy must be expressed with one-hot vectors only (e.g. a designated failure
  outcome — which §7 correctly criticizes as making the failure incentive
  tradeable) or the fractional fallback waits for the V2 schema. Choosing (a)
  is implicitly a statement that the ambiguity policy — "the most important
  open cryptoeconomic design gate" — is not being frozen in V1 either.
- **Terms-fixture disposition:** the kernel stands; the fixture moves. The
  string `exact-scaled-integer-floor-at-final-payout-boundary` is wrong twice
  (the kernel refuses rather than floors; one-hot never rounds at all). Under
  (a) it should read e.g. `one-hot-exact-integer-payout`.

### 1.3 Candidate (b): redemption lots `L_i = D / gcd(D, v_i over P)` (PROPOSED)

- **Where lot enforcement must live** (this is the load-bearing question):
  - `split` / `merge`: kernel, in multiples of `L_split` (split mints and merge
    burns *every* outcome symmetrically, so the complete-set lot binds). P1-A's
    fixture (`L_split = 2`) is refused at split — the trap becomes unreachable.
  - `materialize` / `dematerialize`: kernel, in multiples of `L_i`. This is the
    last boundary the protocol controls before bearer Token-2022 transfers.
  - internal transfer / order escrow reservation: adapter, multiples of `L_i`.
  - `redeem`: already enforced by the exact-divisibility refusal; with upstream
    gates, `RemainderRequired` becomes an invariant-violation signal rather
    than a user-facing dead end (for internal balances).
  - Lots are **derived, not stored**: expose a kernel helper
    `redemption_lot(i)` computed from the immutable `PayoutSet`. Zero state.
- **The external-transfer hole.** Token-2022 transfers cannot be lot-gated: the
  frozen collateral thesis forbids transfer hooks and transfer fees
  (`docs/ECONOMICS.md` §5), and no freeze authority exists (§8). Two
  sub-options:
  - (b1) *gate at materialize/redeem only.* External wallets can fragment a lot
    across wallets; each sub-lot fragment is unredeemable until some wallet
    recombines a full lot. Aggregate `T_i` stays lot-aligned (transfers do not
    change totals), so market-level accounting stays exact; the stranding is
    strictly per-wallet and strictly self-inflicted or dust-griefed.
  - (b2) *lot-scaled external mints.* One external atom of outcome-mint `i`
    represents `L_i` internal atoms (scaling applied at the
    materialize/dematerialize boundary). Every external quantity is then
    automatically lot-aligned and (b)'s external liveness becomes total. Cost:
    per-outcome economic scale differs across mints of one market (UI and
    price-display hazard; terms must freeze and display every `L_i`), and the
    adapter owns an exact scaling conversion.
- **Solvency:** unchanged, and strictly *sharper*: if every `T_i` is a multiple
  of `L_i` then each liability term `T_i * v_i` is divisible by `D`
  (since `(D/g_i) * g_i = D` and `g_i | v_i`), so `required_collateral` never
  ceilings — reserved collateral equals exact liability. (EXP-LOT-B5.)
- **Exit-liveness:** total for internal balances; total external under (b2);
  external-up-to-recombinable-dust under (b1).
- **State cost:** zero stored; pervasive checks. **API impact:** lot checks in
  four kernel transitions + helper + one error variant.
- **Batch-relation interface obligation (flag to sibling lane, not designed
  here):** fills transfer per-outcome claim quantities, so under (b) every fill
  of outcome `i` must be quantized to `L_i` or positions leave lot alignment.
  This constrains the pro-rata/remainder rule of the batch relation itself.
  This is (b)'s largest hidden cost: the divisibility policy leaks out of
  redemption into the venue.
- **Lot-magnitude blowup (quantify tonight, EXP-LOT-B4).** The kernel requires
  one common `D` across the payout set. A rich compatibility fallback family
  forces `D = lcm` of the constituent denominators; lots scale with `D / g_i`.
  With `MAX_PAYOUTS = 8` the set is small, but e.g. equal-weight fallbacks over
  subsets of sizes {2, .., 8} already force `D = 840`; six-decimal collateral
  puts `L_split` at 840 atoms = 0.00084 tokens (tolerable) but adversarially
  chosen subset sizes grow it multiplicatively. The experiment should produce
  the actual table for the payout families anyone intends to admit.
- **Adversarial:**
  - *Dust griefing:* an attacker can send sub-lot external fragments to victims
    (ordinary token-dust spam; victim loses nothing they paid for) — only
    real under (b1).
  - *Lot-fragmentation attack on others:* impossible; all internal transitions
    are gated, and external fragmentation never affects other wallets' balances
    or the aggregate.
  - *Retirement griefing:* under (b1) a single permanently-stranded sub-lot
    external fragment makes total liability nonzero forever, so the §8
    ("retire when every liability is zero") cleanup path never opens and rent/
    liveness obligations zombie on. Under one-hot, an unredeemed atom also
    blocks retirement but its holder *can* always exit; under (b1) the sub-lot
    holder *cannot*, so the block is irreparable without a terminal residual
    rule — which is candidate (c) machinery sneaking back in. (EXP-LOT-X2.)
- **Terms-fixture disposition:** the kernel refusal stands; the fixture moves
  to e.g. `exact-divisibility-with-frozen-redemption-lots` and must carry the
  frozen `L_i` values. Silent floor remains prohibited (`docs/PROTOCOL.md`).

### 1.4 Candidate (c): persistent remainder credit with terminal residual rule (PROPOSED)

- **Mechanism.** Redemption of `(outcome i, quantity q)` under resolved `v`
  pays `floor(q * v_i / D)` and records `credit_num += (q * v_i) mod D` — an
  exact sub-atom claim in units of `1/D` collateral atoms. A `claim_credit`
  transition pays `floor(credit_num / D)` and keeps the sub-atom residue.
  Per-redemption conservation is exact and integer:
  `q * v_i = D * paid + credit_delta`.
- **Who owns the credit.** It must be a Position-scoped balance: one `u64`
  numerator per (Position, Market) — credits from different outcomes share the
  `1/D` unit so one counter suffices; per-outcome credit is unnecessary state.
  For `redeem_external` (bearer Eggs) the presenting wallet must have or create
  a Position to receive the credit. That is the candidate's structural
  friction: **fractional external redemption stops being positionless.**
- **What state it needs.** Position: `+8` bytes (`credit_num < D`, claim
  eagerly or on demand). MarketState: `+8` bytes `credit_num_total`, because
  solvency must include owed credit:

  ```text
  collateral * D >= sum_i T_i * v_i + credit_num_total     (resolved phase)
  ```

  and the Active-phase max over `P` analogously. This is a kernel invariant
  change, a Position layout change (the landed 220-byte Position grows — P1-F
  cost goldens shift again), and a new public transition. It is the largest
  kernel-API footprint of the three candidates.
- **Solvency:** preserved; the ceiling in `required_collateral` already covers
  the fractional value that credits make explicit. The invariant above makes it
  exact instead of conservative.
- **Exit-liveness:** total to within one atom per Position, immediately — no
  lot, no waiting for recombination. Sub-atom residue is claimable whenever it
  accretes to a whole atom.
- **Terminal residual rule.** At retirement, each surviving Position residue is
  `< 1` atom. Frozen rule options: (i) roll to the predeclared neutral sink
  (the same no-interested-party sink §4 uses for failure residue) — an
  explicit, disclosed rule, arguably compatible with the "no *silent* dust
  routing" prohibition; (ii) leave in Hoard forever — blocks retirement,
  reproducing the (b1) zombie problem; (iii) round the *final* claimant up —
  requires collateral over-reservation of up to one atom per Position, i.e. a
  new liability term. Lean within (c): (i), named in immutable terms.
- **Token-2022 composability:** transfers are perfect (any quantity moves);
  redemption needs a Position (friction, rent).
- **Batch-relation interface obligation:** none — fills stay unquantized;
  credits exist only at the redemption boundary. This is (c)'s strongest
  property relative to (b).
- **Adversarial:**
  - *Fragmented-redemption arbitrage:* impossible; floor is subadditive and the
    credit stores the exact difference, so any fragmentation across time within
    one Position pays identically, and fragmentation across `k` Positions
    strands `< k` atoms in credits the attacker owns (their loss, gain zero).
    (EXP-LOT-C1/C2.)
  - *Credit-account bloat:* one credit per Position; Positions cost rent; the
    attack budget is rent-bounded and self-funded.
  - *Credit transferability:* keep credits claim-only, non-transferable, to
    avoid inventing a second fungible instrument with its own dust problem.
- **Terms-fixture disposition:** this is the only candidate under which the
  fixture's `floor-at-final-payout-boundary` language is nearly right — but it
  must move anyway, to e.g.
  `exact-floor-with-persistent-remainder-credit(terminal=neutral-sink)`; the
  kernel moves too (credit state + transition). The current kernel
  refuse-not-floor behavior is replaced, not merely relaxed.

### 1.5 Orthogonal primitive: terminal complete-set redemption (PROPOSED, all candidates)

Add a Resolved-phase transition that redeems a *complete set* jointly: burn `q`
of every outcome, pay exactly `q` (since `sum_i q * w_i = q * D`). It is exact
for every payout vector, never remainders, decreases liability by exactly its
payout, and is the Resolved-phase twin of `merge` (which currently refuses
after resolution — `require_active`). It single-handedly exits the P1-A fixture
(whose trap is exactly one complete set), rescues any balanced position under
every candidate, and is zero-state. It does not replace a candidate — an
*unbalanced* fractional position still needs (a), (b), or (c) — but every
candidate composes with it and wave 2 should test it once, not three times.
(EXP-LOT-X1.)

### 1.6 Comparison and lean

| Axis | (a) one-hot V1 | (b) lots | (c) remainder credit |
|---|---|---|---|
| Solvency | holds, exact | holds, provably exact under alignment | holds, exact with credit term |
| Exit-liveness | total | total internal; external total only under (b2) | total to <1 atom, immediate |
| Stored state | 0 | 0 (derived) | +8B/Position, +8B/Market, new transition |
| Kernel API delta | 1 validation | 4 transition gates + helper | invariant, layout, transition |
| Token-2022 external | unconstrained | unenforceable at transfer; (b1) strands dust, (b2) rescales mints | transfer free; redeem needs Position |
| Batch interface leak | none | **fills quantized to `L_i`** (sibling impact) | none |
| Ambiguity-policy support | deferred to V2 schema | supported, lot table may blow up with `lcm` D | supported, no lot blowup |
| New adversarial surface | none | dust strand + irreparable retirement block (b1) | credit bloat (rent-bounded) |
| Terms fixture | rewrite (`one-hot-exact`) | rewrite (lots) | rewrite (floor+credit) |

**Lean (not a decision):** (a1) for V1 — one-hot enforced in the kernel — plus
the §1.5 terminal complete-set redemption; with (c) as the *reserved* design
for the ambiguity-policy schema and (b2) noted as the alternative if
positionless external fractional redemption is judged non-negotiable. Reasons:
zero state and zero new attack surface tonight; the only demand for fractional
vectors is the failure/ambiguity policy, which is itself explicitly undecided,
so paying (b)'s batch-fill quantization leak or (c)'s Position layout change
now would be buying state for an unfrozen policy; and (c) dominates (b) on the
two axes that will matter when fractional vectors do arrive (no lot blowup
under `lcm`-inflated `D`, no leak into the batch relation), at a state cost that
should be paid when, and only when, that schema is frozen. The strongest
argument *against* this lean is that (a) quietly narrows the V1 failure-policy
space to one-hot constructions; if the morning decision wants the
compatible-outcome fallback live in V1, the lean flips to (c).

In every candidate the **kernel's refuse-don't-floor stance survives or is
replaced by exact-conservation machinery; the static-client terms string is the
side that must move.** No candidate validates
`exact-scaled-integer-floor-at-final-payout-boundary` as written; silent floor
is prohibited by `docs/PROTOCOL.md` and should stay prohibited.

---

## 2. Fee policy

### 2.1 What is broken today (MODEL, from §P1-E and the landed code)

`research/vertical-model/src/lib.rs` `clear_batch_inner`:

```rust
let fee = candidate.matched.checked_mul(self.fee_bps)... / FEE_BPS_DENOMINATOR;
self.accounting.add_fee(fee)?;   // fee_revenue += fee — no payer debited
```

Three defects: (1) `fee_revenue` increments from thin air — cash conservation
with fees is not even expressible; (2) the base is matched *claim quantity*,
not the documented dispersion of the transferred vector at clearing prices;
(3) `floor` with no carry and no allocation — the documented
maker/executor/treasury split, upward taker rounding, and persistent carry are
all absent. `fee_liveness_and_principal_boundaries_remain_disjoint` is a
namespace test, not conservation evidence.

### 2.2 (i) Carry domain: Position vs signed intent vs Epoch

A reframing first, because it changes the whole comparison. The lab
(`fee_fragmentation_result`) contrasts *persistent floor carry* (exactly
fragmentation-invariant inside a domain, but resettable by leaving the domain)
with *stateless ceil* (reset-proof, overcharges dust). There is a third policy
that dominates both (PROPOSED — call it **terminal-ceil carry**):

```text
per fill:   paid = floor((carry + fee_num) / den);  carry' = (carry + fee_num) mod den
at domain close (intent fully filled/cancelled, Position closed, Epoch ended):
            if carry' > 0: charge 1 more atom; carry' = 0
```

Domain-lifetime total is then exactly `ceil(sum fee_num / den)`. Consequences:

- **Fragmentation-invariance** *within* a domain instance: exact, by identity.
- **Cross-domain splitting** is never cheaper: `ceil(a) + ceil(b) >= ceil(a+b)`.
  The carry-reset attack (lab: `test_resetting_carry_can_erase_dust_fee`,
  1,001 dust fills paying zero) exists only for *dropped* carry; under
  terminal-ceil, each abandoned domain instance costs the attacker up to one
  extra atom instead of saving one. The attack inverts sign.
- Every nonzero-dispersion intent pays **at least 1 atom** — a structural
  anti-dust floor with no minimum-fee parameter.

With terminal-ceil, all three domains become fragmentation-*safe*; the choice
reduces to state cost, attribution, and residual ownership:

| Axis | Position | Signed intent | Epoch |
|---|---|---|---|
| Invariant across | all of one owner's activity | fills of one intent | fills within one epoch |
| Reset cost (with terminal-ceil) | pays ≥ ceil per Position lifetime; new Position also costs rent | pays ≥ ceil per intent; ≥1 atom min per intent | pays ≥ ceil per (payer, epoch) |
| Reset cost (naive floor, dropped carry) | < 1 atom gain per extra Position; rent >> 1 collateral atom → uneconomic | free-ish per extra intent → **dust fees vanish** | new epoch every batch → **worst; cross-epoch fragmentation is free** |
| State | +8B per (Position, Market), lives as long as Position | +8B inside the order record, dies with the intent — **no new object** | per-(payer, epoch) map — worst growth, new object class |
| Residual carry owner at domain end | Position close: terminal-ceil (payer pays) | intent close: terminal-ceil (payer pays) | epoch end: terminal-ceil per payer, or a pooled residue with broken attribution |
| Attribution | mixes economically unrelated intents of one owner | exactly the vector one signature committed to | mixes payers unless keyed per payer (= Position domain with more objects) |

Cross-epoch fragmentation: only the Epoch domain resets at epoch cadence;
Position and intent domains span epochs natively (an intent that rests across
epochs keeps its carry). Under naive floor-carry the Epoch domain is the
uniquely bad choice; under terminal-ceil it is merely the most stateful with
the least meaningful attribution. Either way it loses.

**Lean (not a decision):** carry domain = **signed intent, with terminal-ceil
at intent close**. Reasons: it is the domain `docs/FEE_GEOMETRY.md` §5 already
names as the simplest V1 rule (fee per filled signed intent on its committed
canonical vector); the carry lives inside an object that already exists and
already dies at the right time (no new state class, no residual-ownership
question — the close event is the terminal-ceil event); reset economics are
sign-inverted; and attribution is exactly the signed commitment, which is what
a payer-debit receipt has to reference anyway. Position-domain carry is the
runner-up if intent records turn out not to persist a u64; Epoch loses on all
axes.

### 2.3 (ii) Payer debit

PROPOSED specification (replacing the thin-air increment):

- **Base.** Fee is computed on the *dispersion of the vector transferred by the
  filled intent at the frozen clearing prices* (§2.4), never on claim quantity.
  For a scalar single-Egg fill of quantity `q` at clearing price `p` on scale
  `S`: `fee_num = kappa_num * q * p * (S - p)`, `den = kappa_den * S^2`.
- **Sides.** `docs/FEE_GEOMETRY.md` §4: "Fee is paid in Realm collateral on top
  of buy consideration or withheld from sell proceeds." Read literally this is
  the **per-intent rule: each filled intent pays the fee on its own committed
  vector** — a matched pair collects two fees, each side bearing 20 bps of its
  cash consideration at the single-Egg midpoint under `kappa = 0.004`. The
  alternative reading (risk transfers once; charge once, split 50/50 across the
  pair) halves venue take and halves the wash-treasury floor. This is a real
  policy fork the docs do not pin; both arms go into the experiment matrix
  (EXP-FEE-P2) and the fixture schema carries a `fee_side_arm` field. Lean:
  per-intent both-sides, because it needs no pair-attribution rule and is the
  reading under which the documented 60/15/25 wash-loss argument was computed.
- **Legs, exactly.** For one settled fill at clearing price `p_c` with
  consideration `C = quantity * p_c` (exact integer by construction of the
  grid), buyer intent fee `f_b` and seller intent fee `f_s` from each side's
  carry-domain accumulator:

  ```text
  buyer cash debit    = C + f_b        (fee on top; rounds via floor+carry,
  seller cash credit  = C - f_s         terminal-ceil at intent close)
  fee pot delta       = f_b + f_s
  conservation:  sum(buyer debits) - sum(seller credits) = fee pot delta
  ```

  Order-escrow reservation must cover the worst case at reservation time:
  `limit consideration + ceil(max fee at the intent's own committed vector over
  admissible clearing prices)`. Reserving without the fee head-room makes the
  documented "fee never comes from the Hoard" rule unenforceable at settlement.
- **When.** The candidate/clear step *computes and commits* the per-intent fee
  schedule (so lazy settlement cannot drift), but cash moves only with the
  settlement receipt, which gains explicit `fee_buy`/`fee_sell` legs beside
  `consideration`. How receipts pair and how residual pairs settle is the
  sibling lane's relation; the obligation from this side is only: **every
  receipt's cash legs must satisfy the conservation identity above, and fee
  legs debit the named payer's escrowed cash, never the Hoard, never thin air.**
- **Allocation.** Per batch, on the pot actually collected:

  ```text
  maker    = floor(pot * 60 / 100)
  executor = min(floor(pot * 15 / 100), batch executor cap)
  treasury = pot - maker - executor          (all integer, conserves atoms,
                                              treasury >= 25% by construction)
  ```

  Rebates round down, treasury takes the remainder (matches
  `research/economics/model.py::allocate_fee`, which already conserves and
  floors — it needs only the executor cap added). Rebates are computed on
  collected atoms per batch; no cross-payer carry leaks into the pot.

### 2.4 (iii) Simplex-dispersion fee: exact-integer formulation

PROPOSED exact formulation (this is the `docs/FEE_GEOMETRY.md` object with the
widths and rounding pinned):

```text
Inputs:  prices p_i >= 0 integers, sum_i p_i = S (frozen scale, e.g. 10_000)
         intent's committed net payoff vector a_i, integers, |a_i| <= A_max
         lots q (or fold q into a), kappa_num / kappa_den frozen per Market

G_num(a, p) = sum_{i<j} p_i * p_j * |a_i - a_j|          (<= 120 pairs at n=16)
fee_num     = kappa_num * q * G_num(a, p)
den         = kappa_den * S^2
paid, carry per §2.2 (floor + persistent carry, terminal-ceil at domain close)
```

All products in checked `u128`; freeze `S`, `A_max`, max lots, `kappa` so that
`kappa_num * q_max * A_max * S^2 / 2` fits `u128` with margin (at `S = 10^4`,
`den = kappa_den * 10^8`; with `kappa = 4/1000`, `den = 4 * 10^11 / 4` — the
bound is comfortable; EXP-FEE-G2 computes the exact maxima). No pairwise
truncation, one final division, no floats.

**Reduction:** `G_num(q * e_k, p) = q * p_k * (S - p_k)` — exactly
`q * p * (1-p)` in normalized units, so the single-Egg curve is the special
case, and the vertical model's `matched * bps / 10^4` is *not* on this curve
under any parameter choice (it is price-independent).

**Conservation proof obligations** (the `P-FEE-01` closure set, to be stated as
theorems and checked tonight as exhaustive-domain tests):

1. *Atom conservation per fill:* `fee_num_contribution = den * paid_delta +
   carry_delta` exactly; carry `in [0, den)` always.
2. *Domain-lifetime identity:* total paid over a closed domain instance
   `= ceil(sum fee_num / den)`; no atom created or destroyed by any
   fragmentation of fills within the domain.
3. *Cross-domain superadditivity:* splitting one economic intent across `k`
   domain instances pays `>=` the unsplit fee (ceil superadditivity).
4. *Allocation conservation:* `maker + executor + treasury = pot`, executor cap
   respected, treasury `>= floor(25%)`.
5. *Payer conservation:* `sum(buyer debits) - sum(seller credits) = pot delta`;
   joint cash+claims conservation of the whole vertical including fees.
6. *Seminorm identities* (already spot-checked in the lab; to be exhaustive and
   then Verus/Rocq targets per `docs/FEE_GEOMETRY.md` §7): single-Egg
   reduction, complete-set invariance `G(a + c*1, p) = G(a, p)`, relabeling
   symmetry, homogeneity `G(q*a, p) = q*G(a, p)`, subadditivity
   `G(a+b, p) <= G(a, p) + G(b, p)`, and partition-refinement invariance
   (split a state into equal-payoff subcells with exactly-summing prices).

### 2.5 (iv) Self-wash economics under each carry domain (MODEL + PROPOSED)

With rebates paid only from the collected pot and allocation conserving atoms,
a Sybil controlling taker, maker, and executor recovers at most
`maker + executor <= 75%` of its own fee; net wash `= -(treasury) - network
<= -25% of fee - network` (the lab's `wash_cycle_loss` already computes this).
Carry-domain interaction:

- *Intent domain, terminal-ceil:* wash-by-dust is doubly punished — every dust
  intent pays `>= 1` atom (terminal ceil) and forfeits `>= 25%` of it. Strictly
  negative at every size. Best case for the venue.
- *Position domain:* same sign; the washer's own carry accrues across its wash
  trades, so fragmentation cannot push fee below `ceil` per Position; extra
  Positions add rent.
- *Epoch domain with dropped carry:* the only configuration in which wash dust
  can pay ~0 fee — but then rebates are ~0 too, so wash *profit* still cannot
  go positive; the damage is fee evasion on real flow, not rebate farming.
- Standing caveat from the lab: all of this holds only while there is **no
  external emission, points program, or creator-volume rebate** stacked on top.

**Fee-policy leans (not decisions), summarized:** carry domain = signed intent
with terminal-ceil (§2.2); payer debit = per-intent, buyer on-top / seller
withheld, fee legs on the settlement receipt, escrow reserves fee head-room
(§2.3); fee base = the exact-integer dispersion of §2.4 as the experimental arm
against flat-notional and per-leg controls exactly as `docs/FEE_GEOMETRY.md` §6
prescribes — nothing here promotes `kappa = 0.004` or 60/15/25.

---

## 3. Economics-lab / kernel alignment

Target: the lab and the kernel admit the same market set, cover the same payout
semantics, and compute the same fees — witnessed by language-neutral
differential fixtures, per `docs/EVIDENCE_MATRIX.md` `P-SOLV-01` / `P-FEE-01`.

### 3.1 Same admitted market set (weight-sum equality)

- `research/economics/model.py::maximum_liability` refuses only
  `sum(vector) > 1`; the kernel requires `sum(w_i) == D` exactly. Change the
  lab check to `sum(vector) != 1 -> ModelError` (MODEL: no current lab test
  feeds a sub-one vector — `one_hot_vectors` and `compatible_payout` both sum
  to 1 — so the change is test-compatible; add an explicit refusal test for
  sub-one and super-one).
- Add the admission gate for whichever §1 candidate is chosen: under (a) the
  lab refuses non-one-hot sets exactly where the kernel does; under (b)/(c) the
  lab mirrors the lot/credit grammar instead.
- Mirror the kernel's *shape* rules the lab currently ignores: common
  denominator across the set, `MAX_OUTCOMES`/`MAX_PAYOUTS`/`MIN_OUTCOMES`
  bounds, zero-vector padding — as fixture-driven refusal cases, so the two
  sides classify identical inputs identically.

### 3.2 Same payout semantics (fractional coverage per candidate)

`CategoricalMarket` models one-hot resolution only (`winner: int`). Add a
`WeightedMarket` mirror of the kernel's integer semantics — integer weights and
denominator, **not** `Fraction` — with `redeem` refusing on remainder exactly
like the kernel, plus one policy arm per candidate: (a) admission refusal,
(b) lot-gated `split`/`merge`/`materialize` with derived `L_i`, (c) credit
accrual with the §1.4 invariant, and the §1.5 terminal complete-set redemption
in all arms. Extend `enumerate_solvency_traces` to walk these arms and assert,
per candidate, the exit-liveness claim of §5 (not just solvency).

### 3.3 Same fee model

Port §2.3/§2.4 into `model.py`: a payer-debit accounting mirror
(`buyer_cash`, `seller_cash`, `fee_pot`) with the conservation identity, the
terminal-ceil carry policy as a third arm beside the existing
persistent/reset/ceil trio in `fee_fragmentation_result`, the executor cap in
`allocate_fee`, and `dispersion_numerator` as the fee base of record wired into
the debit path (it currently exists but nothing charges by it). The vertical
model then changes to match the same fixtures: fee base = committed-vector
dispersion at clearing price, debit legs on receipts, allocation into the fee
pot — closing all three §P1-E fee defects with one shared vector set.

### 3.4 Differential-fixture plan (language-neutral vectors both sides must pass)

PROPOSED: a JSON fixture family under `fixtures/economics/` (new), consumed by
a Rust test in `clutch-kernel`/`vertical-model` and a Python test in
`research/economics`. Shared error-class vocabulary (strings, mapped each side:
`remainder_required`, `invalid_payout_weights`, `lot_violation`,
`zero_quantity`, `insufficient_balance`, ...). Three schemas:

1. **Admission vectors** (`P-SOLV-01` support): payout set (outcomes, count,
   integer weights, denominator) → `admit | refuse(error_class)`. Includes
   sum<1, sum>1, mixed denominators, padding violations, one-hot/fractional
   per candidate arm.
2. **Trace vectors** (`P-SOLV-01`): market terms + operation list
   (`split/merge/materialize/dematerialize/resolve/redeem_internal/
   redeem_external/claim_credit/redeem_complete_set` with quantities) →
   per-step `ok(payout)/refuse(error_class)` + final
   `collateral/total_supply/credit` state. The P1-A fixture is trace vector #1
   with per-candidate expected outcomes.
3. **Fee vectors** (`P-FEE-01`): `(a, p, S, kappa_num/den, fee_side_arm,
   carry_domain_arm, fill fragmentation schedule, domain-close events)` →
   per-fill `paid/carry`, terminal charge, allocation triple, payer-debit
   deltas, and the conservation checks of §2.4 as required-true flags.

Rules: exact integers only in fixtures; every fixture names its policy arm;
a fixture failing on either side is a finding, not a fixture to edit; minimized
failures become permanent named vectors (matches the cross-runtime differential
gate in `docs/EVIDENCE_MATRIX.md` §7).

---

## 4. Which side moves, per surface (summary table)

| Mismatch | Side that moves | Under |
|---|---|---|
| terms `rounding` string vs kernel refusal | terms fixture (and any schema consuming it) | all candidates (§1.2–1.4 name the replacement string per candidate) |
| lab `sum <= 1` vs kernel `sum == D` | lab | always |
| lab one-hot-only walk vs kernel fractional grammar | lab grows arms; kernel grows gates per chosen candidate | §3.2 |
| vertical fee (base/payer/allocation/carry) vs documented policy | vertical model (and lab gains the debit mirror) | §2.3, §3.3 |
| kernel `merge` refusing post-resolution | kernel (add terminal complete-set redemption) | §1.5, all candidates |

---

## 5. Falsifier / experiment matrix

Vocabulary: every row is MODEL work over host-only code; none of it promotes a
policy. "Exhaustive" means bounded exhaustive enumeration, exact integers;
sampling only where a bound is stated. Adversarial cases are named
individually. Suggested owner lanes: LOT = payout-liveness lane, FEE = fee
lane, ALIGN = differential-fixture lane.

| ID | Candidate / claim under test | Experiment (wave 2, tonight) | Falsifies the candidate if |
|---|---|---|---|
| EXP-LOT-A1 | (a) one-hot admission | Exhaustive payout sets: outcomes ≤ 4, D ≤ 6, all weight tuples; assert admitted iff every weight ∈ {0, D}; then exhaustive traces depth ≤ 6 over admitted sets asserting `RemainderRequired` unreachable | any admitted set reaches a refusal, or gate refuses a one-hot set |
| EXP-LOT-B1 | (b) lot formula | Exhaustive D ≤ 12, all vectors/sets: verify `L_i = D / gcd(D, {v_i≠0})` is exactly the minimal `q`-modulus: every multiple redeems exactly under every admitted resolution AND every non-multiple fails under some admitted resolution | either direction fails (formula too weak or too strong) |
| EXP-LOT-B2 | (b) internal closure | Exhaustive traces with lot-gated split/merge/materialize (D ≤ 6, depth ≤ 6): no reachable resolved state holds a sub-lot internal balance; P1-A fixture refused at split | any reachable exit-dead internal state |
| EXP-LOT-B3 | (b1) external fragmentation adversary | 3-wallet model, arbitrary external transfer quantities, lot-gated dematerialize/redeem_external, exhaustive small quantities: trapped collateral ≤ sub-lot dust; full-lot recombination always recovers | trapping exceeds dust bound, or recombination cannot recover |
| EXP-LOT-B4 | (b) lot magnitude | Compute `D = lcm`, `L_i`, `L_split` tables for the candidate ambiguity families (equal-weight compatible subsets, n ≤ 16, ≤ 8 vectors); report against 6-decimal collateral | intended payout families produce unusable lots (data for morning call, not pass/fail) |
| EXP-LOT-B5 | (b) exact reservation | Exhaustive lot-aligned states: `required_collateral` never ceilings (liability integral) | any aligned state rounds up |
| EXP-LOT-C1 | (c) credit conservation | Exhaustive redemptions/fragmentations (D ≤ 6, q ≤ 12, across ≤ 3 positions): `q*v_i = D*paid + credit_delta` per step; any fragmentation total equals whole exactly; solvency invariant with `credit_num_total` term across bounded traces | one atom created/destroyed, or invariant violated |
| EXP-LOT-C2 | (c) fragmented-redemption arbitrage | Adversary splits redemption across k ≤ 5 positions, exhaustive schedules: total paid + claimable never exceeds unsplit; stranded residue < k atoms all attacker-owned | any positive arbitrage |
| EXP-LOT-X1 | §1.5 terminal complete-set redemption | Exhaustive over admitted sets: joint redemption of complete set pays exactly q, never remainders, preserves solvency exactly; P1-A trap exits | any remainder or solvency drift |
| EXP-LOT-X2 | retirement liveness per candidate | Enumerate bounded terminal states per candidate arm: classify markets permanently unretireable (liability > 0 with no exit path) | (data): candidate admits irreparable zombies — expected for (b1); quantifies §1.3 finding |
| EXP-TERMS-01 | terms/kernel rounding language | Fixture asserting the terms `rounding` string maps to an executable semantic; run current string against kernel semantics (expected FAIL under all candidates); emit per-candidate replacement string vectors | (regression): passing would mean the mismatch was misdiagnosed |
| EXP-FEE-D1 | §2.2 terminal-ceil intent carry | Implement terminal-ceil arm in `fee_fragmentation_result`; exhaustive fill compositions of q ≤ 24 across 1..4 domain instances: within-domain total = `ceil(exact)`; cross-domain ≥ unsplit; reset attack gain ≤ 0 | any fragmentation pays less than `ceil(exact)` |
| EXP-FEE-D2 | Epoch-domain refusal evidence | Exhaustive dust-per-epoch schedules under dropped epoch carry: fee → 0 while volume > 0 | (regression for the refusal): if fees do NOT vanish the Epoch-domain criticism weakens |
| EXP-FEE-P1 | §2.3 payer conservation | Given fixed fill vectors (not the batch relation — sibling's), exhaustive tiny fills: `Σ buyer debits − Σ seller credits = fee pot Δ`; joint cash+claim conservation of vertical incl. fees; Hoard untouched by any fee path | any conservation miss or Hoard leak |
| EXP-FEE-P2 | both-sides vs split-once fee | Both arms over exhaustive (p on grid, q ≤ 20): effective bps per side and total, wash loss, incidence at fixed clearing price | (data for morning call): arms differ where docs assumed they did not |
| EXP-FEE-G1 | §2.4 seminorm identities | Exhaustive n ≤ 5, price compositions of S ≤ 12, payoffs ≤ 4: single-Egg reduction, complete-set invariance, relabeling, homogeneity, subadditivity, partition refinement — exact | any identity fails on any point |
| EXP-FEE-G2 | §2.4 widths | Maximize `fee_num` symbolically over proposed frozen bounds (S, A_max, lots, kappa); assert u128 margin; emit the frozen-constant proposal table | overflow reachable within proposed bounds |
| EXP-FEE-W1 | §2.5 self-wash | For each (carry domain × allocation arm × fee-side arm), exhaustive dust wash schedules ≤ 24 fills: max Sybil recovery ≤ maker+executor; net wash < 0 including 1-atom terminal-ceil floors; no positive cell in the whole matrix | any configuration with non-negative wash |
| EXP-FEE-A1 | allocation exactness with executor cap | Exhaustive pot ≤ 10^4 × cap ∈ {0, 1, pot/10, ∞}: maker+executor+treasury = pot; executor ≤ cap; treasury ≥ 25% of pot | any atom lost or floor broken |
| EXP-ALIGN-01 | §3.1 same admitted set | Admission fixture family (incl. sum<1, sum>1, mixed D, padding): Rust and Python classify identically | any divergent classification |
| EXP-ALIGN-02 | §3.4 trace differential (`P-SOLV-01`) | Trace fixtures (P1-A first) through kernel tests and lab per candidate arm: identical payouts and error classes | any divergence |
| EXP-ALIGN-03 | §3.4 fee differential (`P-FEE-01`) | Fee fixtures through lab and vertical model: identical paid/carry/allocation/debit vectors | any divergence |

Execution notes for wave 2: exhaustive enumerations above are all ≤ ~10^6
states at the stated bounds; keep bounds in the fixture header so both sides
enumerate identically; every falsifier that fires becomes a permanent named
fixture, not a deleted test.

---

## 6. Leans (restated, clearly leans and not decisions)

1. Payout claims: **(a1) one-hot enforced in the kernel for V1**, plus the
   **terminal complete-set redemption** (§1.5) regardless of candidate; **(c)**
   reserved as the fractional design for the ambiguity-policy schema; (b) only
   if positionless external fractional redemption is required, and then in its
   (b2) lot-scaled-mint form. Flip condition: if the morning decision wants the
   compatible-outcome failure fallback live in V1, choose (c) now.
2. Fee carry domain: **signed intent with terminal-ceil at close** (§2.2).
3. Fee payer: **per-intent, buyer on-top / seller withheld, fee legs on the
   settlement receipt, escrow reserves fee head-room** (§2.3); both-sides vs
   split-once left to EXP-FEE-P2 data.
4. Fee base: the **exact-integer dispersion** of §2.4 as the experimental arm,
   promoted only through the `docs/FEE_GEOMETRY.md` §7 gates; `kappa = 0.004`
   and 60/15/25 remain unpromoted experimental parameters.
5. Alignment: lab moves to weight-sum equality and gains the payer-debit and
   terminal-ceil arms; the vertical model's fee path is replaced; the terms
   fixture's rounding string is rewritten under every candidate; all of it
   witnessed by the §3.4 fixture family before any `P-SOLV-01`/`P-FEE-01`
   evidence is claimed.
