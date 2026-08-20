# Fee economics — findings and required corrections

Status: **ASSESSMENT / CORRECTION RECORD.** Maps `docs/FEE_GEOMETRY.md` and
`docs/ECONOMICS.md` against the code and experiments that exist. It selects
no fee base and freezes no parameter. Its purpose is that the canonical
economics documents currently assert properties the project's own research
refutes, and no decision should be made on top of that.

## 1. The headline: a proven fee-evasion channel is missing from the fee's own threat list

`docs/research/RISK_SUMMED_POSITIONS.md:522-536` proves (Proposition 9):

```text
ker Gamma_p = { a : a constant on supp(p) } = span(1) ⊕ R^{Z(p)}
```

At boundary prices the dispersion fee's kernel is **strictly larger** than
the risk quotient, so **risk transfer supported entirely on zero-priced
outcomes is literally feeless, however large its model-free range.**

`FEE_GEOMETRY.md` §3 presents kernel invariance as the fee's central virtue
(complete sets move free) and §5's laundering list does not mention that the
same invariance degenerates into an evasion channel at extreme prices. The
document names no zero-price falsifier, and `FeeBasis` in the lab still has
two members with no zero-price test.

This is a falsifier of the fee's own anti-laundering requirement, proved
in-repo, unrecorded in the canonical doc. It must be added to §5 and to the
lab before any base is selected.

## 2. Two corrections the canonical docs have not absorbed

Both from `RISK_SUMMED_POSITIONS.md`, both proved there, neither cited by
`FEE_GEOMETRY.md` or `ECONOMICS.md` — which cite that document nowhere.

- **Dispersion is not the quotient norm (Proposition 10, refuted).**
  `Gamma_p(a) <= R(a)/4`, with equality only at the supremum over prices.
  The single-Egg case shows the whole gap: the ratio
  `2 p(1-p)` tends to zero at extreme prices. The docs' framing of `G` as
  "the exact generalization" overstates it.
- **But `G` is characterized, not merely constructed (Propositions 11-12).**
  It is the *unique* positively 1-homogeneous functional reducing to
  `q(1-q)` on digitals and additive over layer-cake decompositions, and
  within the pairwise family relabeling symmetry plus homogeneity force
  `phi(t) = c|t|`. That is a stronger result than the docs claim.

The correction and the upgrade both need to land in `FEE_GEOMETRY.md`.

## 3. The promotion gate is unstartable, not pending

`FEE_GEOMETRY.md:16-17` conditions canonicity on beating flat-notional **and
per-Egg** controls. Of the five demanded control arms, arm 3 — per-Egg
`q*p_i*(1-p_i)` charged leg by leg, the specific baseline the design was
built to beat — **does not exist in any language**. `FeeBasis` has exactly
two members. So the fee has never been compared against its own benchmark,
and §7 reads as if the comparison were merely un-run.

A sixth arm is also owed: `RISK_SUMMED_POSITIONS.md:650-652` demands the
quotient-norm base `kappa'·R(a)` as a control. Demanded arms are now six, of
which three exist.

Adding arms 3 and 6 to the Python lab is about a day. The blocker is
`FEE_GEOMETRY.md:154-165`'s measurement axes: **depth, participation, fill
rate, and route leakage require a market-quality simulator that exists
nowhere in the tree** — no order-flow generator, no elasticity model, no
counterparty model. Without it, four of the eight axes are unmeasurable and
the promotion gate cannot close as written.

## 4. Ten further assertions without support

1. `ECONOMICS.md:135-138` asserts wash volume is **negative**; the lab found
   it is **non-negative** — exactly zero in zero-fee cells with dropped
   carry. Strict negativity holds only under terminal-ceil.
2. `ECONOMICS.md:128-129`'s "20 basis points at `p = 0.5`" is exact as a
   rational and false at small size: terminal-ceil charges one atom per
   intent regardless, and the repo's own fee vector records a 1-atom fee on
   1 atom of consideration — 10,000 bp, not 20.
3. `ECONOMICS.md:165`'s break-even inequality returns `unbounded` at every
   currently-true configuration; the "$2,000 of volume per dollar of cost"
   figure is arithmetic on an assumed 5-bp net take, not a measurement.
4. `ECONOMICS.md:88-89` asserts an O(1) cumulative reimbursement index; the
   model implements O(k) recomputation and says so.
5. `FEE_GEOMETRY.md:166-169` requires formal verification of
   partition-refinement invariance; there is bounded exhaustive Python
   (`n <= 5`, `S <= 12`) and three Rust unit tests in a file with no
   consumers. The Verus/Rocq closure the promotion gate requires does not
   exist — Rocq has zero theorems.
6. `FEE_GEOMETRY.md:99-102` requires five bounds frozen **before**
   implementation; `dispersion_fee_step` is implemented and none is frozen.
   Checked `u128` arithmetic makes it safe but changes the claim: its domain
   is "whatever does not overflow" rather than an audited envelope.
7. `RevenuePolicy` is named as an architectural boundary in four documents
   and is zero lines of code.
8. The 60 / ≤15 / ≥25 maker-executor-treasury split has derived basis-point
   figures in the docs and no Rust implementation; the only Rust fee
   allocator in the tree allocates by LP capital-time weight, a different
   mechanism, with no consumers.
9. `OPEN_QUESTIONS.md` still lists as open two rows that
   `FAILURE_PAYOUT_DECISION_V1.md` decided (dust/lots, failure payout), and
   still lists the fee-base fork itself at `:70` — correctly, but the
   decided rows were never retired.
10. `benchmarks/constants.json` is pinned 3,109 layout-lines stale and
    soft-notes the drift it exists to refuse.

## 5. What is actually built, stated precisely

- **`dispersion_fee_step`** (`portfolio_settlement.rs:388`) implements
  §4 exactly in checked `u128` — and is orphaned *inside its own module*:
  `prepare_full_pair` never calls it, and its only callers are that file's
  own tests. `lots` is not a parameter; the caller must fold it into the
  payoff. Only floor is implemented; there is no terminal-ceil close.
- **`IntentFeeCarry`** (`clutch-liveness:1128-1245`) already implements the
  exact "signed-intent domain, terminal-ceil close" design both policy
  documents recommend — with authentication refusing wrong owner, wrong
  intent, reopen, and non-canonical carry, and tests proving fragmentation
  invariance. **Zero consumers.**
- **`FeeBaseV1::FlatNotional`** (`clutch-batch/relation_v1.rs`) is a complete
  working flat-notional arm with carry, conservation, and an exactness
  check. Unreachable from the program.
- **Correction to the earlier scorecard:** `clutch-liveness` *is* in the SBF
  program's dependency graph and *is* reached at runtime — transitively via
  `batch-policy-identity`, where `DonationLedger::admit_prefunded` is called
  on the live V3 path. Exactly one of its ~14 public types is wired; the
  entire admission surface is not.
- **The layout is already fee-capable.** `max_fee_atoms` rides
  `Intent::PlaceOrder`, is plumbed correctly through `ReservationPlan`
  (buyers reserve consideration plus fee; sellers withhold from proceeds),
  and round-trips in the codec. It is forced to zero at **five** gates.
  Relaxing them is trivial; there is nothing to pay the fee *to*.

## 6. The real shape of the decision

Charging any fee requires, in dependency order: an authenticated
fee-destination account; an authenticated per-intent carry account; a
`FrozenPolicyV1` other than `DIRECT_POLICY_V1` admitted by the domain
validator — and the compact candidate account has **no fee field at all**,
so this is an ABI change, not a flag; and only then removing the five
zero-gates. None of the first three exists.

Sequencing that follows from the above, offered as a recommendation and not
a decision:

1. **Correct the documents first** (§1-§4). A fee base selected against a
   threat list missing its own proved evasion channel is not a decision, it
   is an accident.
2. **Add arms 3 and 6 to the Python lab plus a zero-price laundering
   falsifier.** Cheap, and it makes the existing recommendation
   (atomic dispersion, terminal-ceil, intent-scoped carry) either survive a
   real comparison or fail it.
3. **Decide the destination before the base.** `RevenuePolicy` is the
   genuinely missing object, and the closest live seam is ResolutionWork's
   five charge fields — all hardcoded to zero with the reason stated in the
   source: *"Every protocol charge is zero because V1 has no authenticated
   fee sink."* That sentence is the whole blocker, and it is a rent/SOL
   denomination, distinct from the collateral-atom fee the geometry
   describes.
4. **Only then** wire `IntentFeeCarry`, which is already written and tested.

The market-quality axes (depth, fill rate, route leakage) should be declared
out of scope for V1 selection rather than left as an unmeetable gate; a fee
base can be chosen on arithmetic invariants and laundering resistance alone,
provided the document says that is what happened.
