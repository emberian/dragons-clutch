# Orphan-design triage — FRONTIER, 2026-08-30

Charter: take the aspiration ledger's expansion frontier and DIG's thirteen
one-owner-short designs, and turn each into a row a lane can pick up — what
exists, the one missing owner-step, the size, and where it goes. Land the ones
that are genuinely under an hour and unfenced. Measure before believing.

Inputs: `docs/evidence/ASPIRATION_ARCHAEOLOGY_2026_08_30.md` (§B.12, §B.13),
`docs/research/EXPANSION_FRONTIER_2026_08_25.md`, `docs/ASPIRATION_LEDGER.md`
(M-4, M-9, M-10, U-013), `docs/OMISSION_INDEX.md`, `WAVE.md` cycle-3 charter.

## The finding that reframes the rest

**Three of the four headline orphans are not orphans any more, and the one that
is turns out to be a harder problem than "unwired".** Measured at HEAD today:

- "Four proved kernels consumed by nothing" — **one**, not four (§4).
- "No layout by which a Market can select a spline basis at all" — the layout
  exists, carries every field U-013 demands, and is decoded on the live devnet
  path by ten modules (§2).
- "221 theorems wait on ONE field" — they wait on an **ABI unification
  ruling**. The kernel speaks a record family the tree affirmatively retired
  around it (§2).
- "The Dealer defect that stranded the first market's principal is live in four
  routes" — two routes, not four; the stranded market was a local validator,
  not devnet; and ADR 0008 §6 already owns it (§3.1).

The pattern is not that the dig was careless — it re-derived a great deal
correctly. The pattern is that **rows describing a gap decay faster than the
gap closes**, so a doc written to record an omission becomes, within days, a
record of an omission-shaped hole that has partly filled in. Every row below
carries the measurement that establishes its status, so the next lane can
re-run the measurement instead of re-running the archaeology.

## How to read a row

Each row states: **EXISTS** (what is built and provable today) — **MISSING
OWNER-STEP** (the single next act, named so it can be assigned) — **SIZE** —
**TARGET** (a cycle-3 charter item, cohort-7, a ruling, or compost).

Sizes are hours / days / weeks / ruling. "Ruling" means no amount of
engineering unblocks it; a person has to choose.

---

## 2. The spline field — the highest-leverage single edit named in the dig, measured and NOT taken

DIG B.12's top row and Tier-3 item 18 say: *"ONE Market layout field selecting
a basis; everything downstream is already proved+emitted"* and *"221 theorems
wait on one field."* I was chartered to land it if it was genuinely one field,
and to post the real size with evidence and not half-land if it was not.

**It is not one field. I did not land it.** The evidence, in the order that
changed my mind:

### 2.1 The selector already exists and already ships

`crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs:105`:

```rust
pub enum BasisKindV3 {
    /// Runtime-width one-hot basis with `Q = 1`.
    CategoricalQ1,
    /// Nonnegative rational graded curves plus one exact complement.
    GradedExactComplement,
}
```

`BasisShapeV3` (same file, `:131`) already carries `Constant`, `RampUp`,
`RampDown` and `Tent` over `u32` Product-owned knot indices, and `BasisInputV3`
carries `basis_width`, `payout_scale` (`Q`), `knot_denominator`, a runtime
`knots` vector, `terms` with per-claim amplitude, `evaluator_release_id`, and
`failure_payouts`.

So **shaped, non-categorical payoffs already ship on the live wire** —
degree-0 (`Constant`) and degree-1 (`RampUp` / `RampDown` / `Tent`), at runtime
width, with a certified categorical projection carrying a componentwise integer
error bound (`crates/dclutch-product-compiler/src/noncategorical_v3.rs`).

This matters beyond sizing. Ledger **M-4** records ember's *"'5 fixed bands' is
really not good enough"* as a dropped requirement. **It is not dropped — it is
partly delivered under a different name.** What does not ship is *curvature*:
degrees 2 and 3. That is a much smaller and much more honest gap to put in
front of him than "the B-spline requirement regressed silently."

### 2.2 The layout already carries every field U-013 demands

U-013 said: *"Still required: a Market and Claims layout carrying basis width,
payout scale, evaluator release, certificate schema, and capacity profile."*
All five are present:

| U-013 field | Where it lives today |
|---|---|
| basis width | Claims aggregate `claim_count` (`crates/dclutch-claims-svm/src/liability_basis_state_v2.rs`) |
| payout scale `Q` | Core Market `basis_scale: u64` (`crates/dclutch-market-core-codec/src/generated.rs:784`) |
| basis identity | Core Market `claim_basis` + `liability_basis` (`generated.rs:308-309`); Claims aggregate `BASIS` at offset 152; Position `BASIS` at offset 88 |
| evaluator release | `BasisInputV3.evaluator_release_id` |
| certificate schema | `CategoricalApproximationCertificateV3` (`product-payoff-v2-codec/src/registry_v3.rs`) |

And it is authenticated on chain:
`crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs:427` refuses when
`semantic_basis_id != runtime.liability_basis_id`.

**This layout is live production state, not a proposal.**
`LiabilityBasisMarketViewV2` is decoded by seven
`tools/local-validator/bootstrap/successor/` modules (`market`,
`flagship_resolution`, `wallet_terminal`, `direct_trade_producer`,
`direct_trade_token_setup`, `terminal_lifecycle`, `user_position_admission`),
by `tools/gauntlet/journey/{ledger,stages}.rs`, and by
`dclutch-market-retirement-v1-operator`. U-013 has been amended in place
(`docs/OMISSION_INDEX.md`) rather than left to be rediscovered a fourth time.

### 2.3 What is actually missing, and why it is not an edit

**The kernel speaks a retired ABI.** `dclutch-liability-basis-v2-kernel`
decodes record magics `DCLTLBV2` and `DCLTLNK2`.
`programs/dclutch-claims-sbf/src/liability_basis_v2.rs:11-27` records, in its
own words, that the `DCLTLNK2` route was **deleted as dead on both ends** —
*"nothing in the tree built a `DCLLBX02` instruction… and nothing on chain
finalized a `DCLTLNK2` record either"* — and that the deletion was deliberately
taken off the queue that said *"wait for whoever retires the V2
liability-basis kernel."* The kernel's 1,173-line `product_claims.rs` models a
record family the tree retired around it.

So there are **two independent evaluators**:

| | live path | kernel |
|---|---|---|
| authority | `ProductBasisV3`, `runtime_v3.rs`, ~1,700 lines, **handwritten**, no provenance header | `evaluate_spline_v2`, Lean-emitted, byte-guarded |
| record | `DCLTPAY3`, schema 3, 256-byte header | `DCLTLBV2` / `DCLTLNK2`, retired |
| arithmetic | its own exact signed-rational | integer de Boor |

Landing the spline means **proving these two agree, or retiring one**. That is
an authority decision about the protocol's wire, not a field.

Second blocker: **the price-gate certificate has no wire slot anywhere.** Magic
`DCLTPGT1` (320 bytes) occurs exactly once in the tree —
`formal/dclutch-semantics/DClutchSemantics/LiabilityBasisV2PriceGateAbi.lean:12`.
No instruction, no account, not even the Rust. And the gate is mandatory:
`EXPANSION_FRONTIER` §"Slice two" states that at degree ≥ 2 the simplex
condition stops being the no-arbitrage condition, so *"a Market that could
select such a basis without a price admission rule would carry an executable
arbitrage."* Degree ≤ 1 is exempt and provably so
(`PRICE_GATE_EXEMPT_DEGREE_V1 = 1`).

Third: **blast radius.** 61 files reference
`BasisKindV3` / `BasisShapeV3` / `ProductBasisV3`; there are 112
`BasisKindV3::` match sites. A third variant breaks every exhaustive match,
including six files in `programs/dclutch-claims-sbf` — where
`terminal_certificate_v3.rs` matches on the *pair* `(basis kind, certificate
kind)`, so the new variant needs a resolution-success and a resolution-failure
answer, not just a decode arm — and including TRADE's live devnet drivers,
mid-founding, today.

### 2.4 What is genuinely good news

- **The degree-≥2 price gate's Lean now EXISTS.** `EXPANSION_FRONTIER:153`
  says *"It has no Lean, which is the part worth adding here"* — stale.
  `LiabilityBasisV2PriceGate.lean` (753 lines) + `PriceGateAbi` +
  `PriceGateExamples` + `EmitLiabilityBasisV2PriceGateRust.lean` +
  `src/price_gate.rs` + `src/generated_price_gate.rs` +
  `check-generated-price-gate.sh` all landed. The safety precondition the
  frontier doc named as *the* blocker is met. Corrected in place.
- **The theorems are real**: 221 across seven files (101 + 50 + 17 + 37 + 13 +
  3), zero `sorry`, three emitters, and **all three generated files are
  byte-guarded** (`check-generated.sh`, `check-generated-spline.sh`,
  `check-generated-price-gate.sh`, each `lake build` → `lake env lean --run`
  → header pin → exact `wc -l` → array cardinality → `cmp`).
- **There is somewhere to land it.** `ProductBasisV3` has 50 zero-enforced
  reserved header bytes (offset 18 / 2 bytes, offset 208 / 48 bytes) plus a
  runtime knot and term tail. Contrast `CoreState`: Lean-emitted,
  `STATE_BYTES = 360`, last field ends at exactly 360, **zero slack**, manual
  fixed-offset packing, with `minimum_balance(STATE_BYTES)` and three
  `data.len() != STATE_BYTES` refusals on the live devnet path. A field there
  would be an account-size migration plus a version and magic bump plus a Lean
  edit. The right home is not the one the dig named.

### 2.5 And wiring would not close the gen-1 gap anyway

`docs/research/BSPLINE_ECLIPSE_SCORECARD_2026_08_27.md` records three deficits
against generation one that no consumer fixes: physical **width 10 vs 16**;
**edge policy** (this tree clamps only; gen-1 offered clamp-or-refuse); and
**rounding symmetry** (the cumulative floor is order-dependent; gen-1's
largest-remainder rule was reflection-symmetric). Its line 367: *"None of those
three deficits moved today."* Its answer to *"is the successor now at least as
capable as generation one on shaped dynamics?"* is still **no**.

### 2.6 CHARTER ROW — SPLINE-WIRE

> **EXISTS.** 221 sorry-free theorems in seven Lean modules, three emitters,
> three byte-identity guards, a 6,155-line no-std kernel with integer de Boor
> for degrees 1–3 and an integer hull-membership price gate. A live, on-chain,
> authenticated basis-identity layout with all five U-013 fields. Degree-0 and
> degree-1 shaped payoffs already shipping through
> `BasisKindV3::GradedExactComplement`.
> **MISSING OWNER-STEP.** *A ruling on which evaluator is the authority* —
> unify the kernel's retired-ABI evaluator with the live handwritten
> `ProductBasisV3` evaluator, or retire one. Everything else (a third
> `BasisKindV3` tag into the 50 reserved bytes, a wire slot for the 320-byte
> price-gate certificate, the 112 match sites, the four campaigns) is
> downstream of that one choice and cannot be sequenced before it.
> **SIZE.** The ruling: one conversation. The unification and wire-through:
> **weeks**, and it is a wire-format change to state TRADE is founding against
> right now. Not an afternoon and not safely concurrent with a live founding
> lane.
> **TARGET.** Cycle-3 charter as its own item, or cohort-7. Explicitly NOT
> this wave. Pair the ruling with M-4's correction: tell ember that ramps and
> tents ship today and the unreached capability is curvature, so he is ruling
> on the real question.
>
> **STATUS: THE DESIGN IS WRITTEN — `623101b2`,
> `docs/design/BASIS_ABI_UNIFICATION_V1.md` (FRONTIER-2, 2026-08-30).** M-4's
> correction landed separately at `1b49b0b9`. The ruling is still owed; what is
> no longer owed is the analysis it needs.
>
> **Five things in this section the design's own measurement contradicts.** They
> are listed here rather than silently fixed above, because this section's
> errors are the instructive part — every one came from a search whose *method*
> could not see the thing it was looking for:
>
> 1. **"the kernel: `evaluate_spline_v2`, Lean-emitted, byte-guarded" (§2.3
>    table) is false.** `spline.rs` is 628 **handwritten** lines and says so in
>    its own header; only `generated_spline.rs` is emitted, and it holds
>    constants and corpora, no algorithm. 70% of the kernel is handwritten. So
>    the unification is not "guarded artifact versus unguarded handwritten one"
>    — it is two handwritten evaluators, of which the **on-chain** one is the
>    one with no specification.
> 2. **"`DCLTPGT1` … not even the Rust" is false.** The magic is live at
>    `generated_price_gate.rs:29` as a hex byte array behind ~1,500 lines of
>    Rust. An ASCII `rg` cannot see a hex-encoded magic. *"No instruction, no
>    account"* is correct and is the real finding.
> 3. **"112 match sites across 61 files" is misleading by an order of
>    magnitude.** Of 13 `match` expressions over `BasisKindV3`, **10 are
>    exhaustive and the other 3 are fail-closed** — so a third variant cannot
>    silently mis-evaluate anywhere. `programs/dclutch-claims-sbf` holds
>    **one** exhaustive match, not six; the other five files mention the type
>    only in doc comments. The weeks are in the evaluator, not the enum.
> 4. **The U-013 field table's payout-scale and basis-identity citations point
>    at Lean kernel structs, not `CoreState`** — which carries no basis field at
>    all. Corrected in `docs/OMISSION_INDEX.md` under U-013.
> 5. **A surface this section did not count at all:** the basis-kind tag has
>    **four independent authors** — handwritten Rust, a Lean-emitted file that
>    *nothing re-runs*, and two byte-identical TypeScript copies with
>    `tag !== 1 && tag !== 2` hardcoded.
>
> **And the sizing conclusion inverts.** This section says the ruling is one
> conversation and the work is "weeks, and it is a wire-format change… not
> safely concurrent with a live founding lane." The first half stands. The
> second is half wrong: **five of the design's ten commits change no wire bytes
> and are landable under a live founding lane today**, and one of them — putting
> a byte guard on the unguarded second author of the kind tag — is free and is a
> precondition for doing the risky half safely.

---

## 3. The thirteen one-owner-short designs

### 3.1 Dealer's principal-stranding defect class — **TRIAGED, NOT URGENT**

> **EXISTS.** Decision 0008 §1: the Claims aggregate's `custody_context` is the
> sole persisted owner of a Market's Custody namespace; every consumer derives
> replay and Vault from it. `founding_v5.rs:485` implements the write side —
> `custody_context = SHA-256(PROJECTED_HOARD_CONTEXT_DOMAIN_V1 ||
> permit.ticket_context)`, unconditionally.
> **MISSING OWNER-STEP.** Two Dealer routes derive `HoardPrincipal` from
> `context.market` with no backstop: `v3_composer.rs:561` and
> `v3_multi_lp.rs:1009-1011`. (`v3_accelerator_accounts.rs:548` *does* read the
> field — `market.custody_context != request.child_root` → refuse — so that
> route fails closed; `dealer-sbf/src/lib.rs:2049` is the v1 program, which
> ships in no release set.) The step is: give the Dealer frame the aggregate,
> then derive.
> **SIZE.** Days. **Severity: pre-launch blocker for the Dealer family, not a
> live vulnerability.**
> **TARGET.** ADR 0008 §6 item 2 — it already has an owner (tranche-A Dealer);
> the lane was recorded CLOSED for the CoreState decode fix and this row went
> with it. Re-open the row rather than chartering a new lane.

**Why not urgent** (charter asked me to escalate if it threatened a current
devnet market — it does not, on two independent grounds):

1. **No devnet market has ever reached Open.** `DEPLOY_1.md:142`: *"STOPPED AT
   A NAMED WALL — the market is NOT founded on devnet"* (`TooManyAccountLocks`;
   the atomic founding locks > 64 unique accounts and devnet lacks
   `increase_tx_account_lock_limit`). `FoundingV5` — the sole producer of a
   Claims aggregate — is inside the transaction that never executed. No
   aggregate exists on that cluster to be mis-derived.
2. **No Dealer route is dispatchable on devnet.** The deployment is exactly
   seven roles; `dclutch-dealer-sbf` gets no ELF and no derived address, and
   `tools/gauntlet/blocked.json:6` says of the accelerator *"not part of the
   seven-role successor release set; no tier deploys it."* The dealer-family
   bytes are inside the shipped Trading ELF, but reaching them requires a
   founded Market, a Claims aggregate and admitted Dealer artifacts.

**And the stranded market was never devnet.**
`apps/dclutch-web/fixtures/live-open-market.json` says so in its own note
field: *"Finalized account bytes copied verbatim off a local
successor-campaign validator. Not devnet or mainnet evidence."* Same for
`docs/evidence/FIRST_BROWSER_EXECUTION_2026_08_27.md:9-11`.

**One sharpening in the dangerous direction:** because `founding_v5` writes a
digest unconditionally, `custody_context` **never** equals the market address
for any market founded from HEAD. The two unbackstopped sites are therefore
wrong-by-construction for *the very first* market a Dealer is ever selected
on — certain, not possible.

**Sibling row (ADR 0008 §6 item 3):** Dealer v1 partitions by Dealer state
address (`dealer-sbf/src/lib.rs:2042-2057`) where v2/v3/v4 partition by
`child_root` (`v3_composer.rs:559-560`, `v3_accelerator_accounts.rs:837`,
`v3_multi_lp.rs:986`, `dealer/mod.rs:365`). v1 is compiled and gauntlet-tiered
but deployed nowhere. **Size: the v1-supersession question is a ruling; the
fix is days if v1 survives, zero if it does not.**

### 3.2 The recovery ladder is welded shut for every family

> **EXISTS.** The funded-recovery machinery is written.
> `MAINNET_STATE_RELAY.md` §13 names the post-v1 lane (funded FailNext over
> RecoveryPolicyV2, relayed leg under a disjoint key set).
> **MISSING OWNER-STEP.** A ruling, then an owner. Verified today:
> `funded::process_funded_transition`'s only call site is inside
> `#[cfg(any())] fn removed_legacy_v1_direct_instruction`
> (`programs/dclutch-resolution-proof-sbf/src/lib.rs:240-247`) — a
> never-compiled block whose *name* says it was removed. Every devnet market
> therefore has exactly one source attempt and no fallback.
> **SIZE.** Weeks, under a ruling.
> **TARGET.** The ruling first: *does v1 ship one-attempt markets forever?*
> That is a Tier-3 ember question and it is not in the census Q-set. Then
> LIVENESS (cycle-3 item 3) — "every 'someone must act' point gets a funded,
> anyone-can-act path or a precisely named gap" is exactly this row's shape.

### 3.3 The fee program's trigger fired and the lane did not spawn

> **EXISTS.** `docs/design/FEE_GEOMETRY.md` — a complete fee-geometry study
> with the composite `kappa*G + kappa'*R` shape selected by delegation.
> **MISSING OWNER-STEP.** The lane. Verified today: `dispersion_bps`,
> `FeeBaseV1` and `accrual_monotone` have **zero** occurrences in any `.rs` or
> `.lean` file in the tree. General charges nothing. FEE-GEO's own row says
> "Trigger: cycle 3"; cycle 3 opened 08-29 with six items and FEE-GEO is not
> one of them.
> **SIZE.** Kernel + composite: weeks. **The rate itself: ember's alone**
> (M-26, the oldest open question in the project — day one).
> **TARGET.** Ask M-26 in the same conversation as §2.6 and §3.2's ruling —
> all three are "ember chooses a number or a posture." Then cycle-3 or
> cohort-7. Note N-15's precondition (formalize before freeze) is recorded in
> no gen-3 doc; it should ride the charter.

### 3.4 General is an order-book venue with no way to place an order

> **EXISTS.** GEN-SEVEN with its rungs laid, in the 08-27 handoff queue.
> Activation artifacts exist; all three generations have typed encoders pinned
> against Lean-emitted artifacts.
> **MISSING OWNER-STEP.** Authoring seven actions. All seven
> collection/candidate actions are still listed as `unauthored_actions!`
> (`crates/dclutch-general-adapter-contract/src/{specialization,artifacts_v3,account_rules_v3}.rs`).
> Inside it, three decision-0010 residues: work-escrow lamports never move
> (§6.3); `ExpireSettlement` has no counterpart, so a stalled settlement cursor
> is stuck (§6.4); nothing creates or closes the claim-escrow Position (§6.5).
> **SIZE.** Weeks, one coordinated unit.
> **TARGET.** A cycle-3 charter item in its own right, or explicitly
> post-launch. The finding is not GEN-SEVEN (known) — it is that the cycle-3
> charter dropped it and the three residues are recorded nowhere in it.

### 3.5 Decision 0005's promised omission rows were never recorded

> **EXISTS.** Decision 0005 promises two rows "recorded in the omission
> index." `docs/OMISSION_INDEX.md` has neither.
> **MISSING OWNER-STEP.** Two index rows. Verified today: no seal emitter
> exists, and `CloseSeal` has **zero** occurrences anywhere in the tree — so
> seal rent is permanently unreclaimable on a write-once account class that
> only grows.
> **SIZE.** The index rows: **minutes**. The capability-seal Lean ABI
> migration: days. The `CloseSeal` route: days, and it needs a rent-reclamation
> ruling (who receives it).
> **TARGET.** The rows should be added by whoever next touches
> `OMISSION_INDEX.md` with decision 0005 open in front of them — I did not add
> them because writing an omission row requires reading the decision's exact
> promise, and I could not do that and the spline measurement in one lane
> without doing one of them badly. **This is the cheapest unlanded row in this
> document; it is genuinely minutes for someone with 0005 already open.**
>
> **STATUS: LANDED — `e5005be6` (FRONTIER-2, 2026-08-30).** Both rows written
> as `P-006` (seal rent permanently unreclaimable) and `P-007` (seal byte
> layout hand-authored, not Lean-emitted), each against measurement at HEAD
> rather than against 0005's prose. Two findings the row's own estimate did not
> contain, both of which change the sizing of the follow-on work:
>
> 1. **The rent leak scales with the release cadence, not the Market count.**
>    `trading_semantic_release` is the fourth PDA seed, and `0005:280` says a
>    Trading upgrade *"does not invalidate a seal so much as stop addressing
>    it"* — so every Trading release permanently strands the rent of every seal
>    written under its predecessor, across all descriptors × actions. That is
>    why the deferral in `0005:303-305` was defensible when written and gets
>    less so with each release. Each seal is 968 bytes (152-byte header + six
>    136-byte rows), ≈ 0.00763 SOL.
> 2. **The hand-authored layout has three authors, not one.**
>    `programs/dclutch-trading-sbf` depends on the crate, `hot_v3/seal.rs:72`
>    writes it on chain, and `apps/dclutch-web/lib/directHotChain.ts` and
>    `packages/dclutch-sdk/lib/directHotChain.ts` both derive seal accounts
>    against the same offsets — a persisted on-chain layout with no
>    byte-identity gate, in a crate directory holding only `Cargo.toml` and
>    `src/` against 14 `check-generated.sh` guards elsewhere in `crates/`. So
>    `P-007` is not merely a provenance nicety: it is this project's signature
>    defect class (two authors for one fact) in miniature, and it is the same
>    class as §2's spline problem at a size someone can actually close.

### 3.6 The artifact bridge is frozen at day one

> **EXISTS.** Eight committed Kani harnesses
> (`tools/direct-translation-validator/src/kani_proofs.rs`, verified: 8
> `kani::proof` attributes). Four ELF-level theorems, all dated 2026-08-25,
> covering artifacts retired the same day.
> **MISSING OWNER-STEP.** Running Kani once. Verified today:
> `formal/qedsvm-direct-v12/` contains **zero** `.lean` files — traces,
> harness, `evidence.json` and a `verify_capture.sh`, nothing else.
> **SIZE.** Running the harnesses: hours. The bridge itself: a ruling first —
> *is this still the architecture?*
> **TARGET.** The Kani run is a Tier-1-sized unfenced win for any lane with a
> spare hour and a Kani toolchain; it is the only item in the thirteen where
> "run the thing that is already committed" is the whole step. The bridge is
> Tier 3 (ember).
>
> **CHARTER SHARPENED 2026-08-30 (FRONTIER-2): "a spare hour and a Kani
> toolchain" is doing real work in that sentence, and the toolchain is not
> present.** `which kani cargo-kani` → **not found** on this machine. So the
> row is not "run the harnesses"; it is "install a verifier, then run them",
> and the install is the part with the risk: `cargo install --locked
> kani-verifier` followed by `cargo kani setup` pulls a **pinned Rust toolchain
> and a CBMC build**, which is a multi-GB download and a toolchain that must
> agree with the crate's edition. **Check free disk before starting** — this
> machine's data volume was at **100% (689 MiB free of 7.3 TiB)** when this row
> was sharpened, which is on its own enough to fail the install halfway and
> leave a confusing mess.
> **The honest size is therefore hours-to-a-day, most of it environment**, and
> a lane that budgets one hour will abandon it partway. If the harnesses do run,
> the deliverable is the evidence file the eight `kani::proof` attributes have
> never produced.

### 3.7 Retirement has never run anywhere, on any substrate

> **EXISTS.** The retirement path is built; the journey gauntlet exercises it;
> `dclutch-market-retirement-v1-operator` exists.
> **MISSING OWNER-STEP.** One complete wind-down, anywhere. README's own
> voice: *"winding a market all the way down to retired has not run anywhere
> yet."* The gauntlet records the gap **moving** rather than closing: retire
> refuses while the Hoard holds one atom; emptying means redeeming; redemption
> is behind the Hot gate.
> **SIZE.** Days-to-weeks after the ruling.
> **TARGET.** **Partly unblocked today.** Ember's Q3 ruling (option (c),
> perpetual CLAIM not perpetual account: post-deadline compaction to a durable
> claim-check, market accounts close, the holder's right survives redeemable
> forever) and Q6 (CloseReplay gated on the terminal receipt, shaped by Q3(c))
> both landed at ~10:20 EDT. Q3C-DESIGN is live on the design. This row's
> ruling dependency is **satisfied**; it now needs the run. It is the last
> unexercised acceptance condition — the thing that distinguishes a market from
> a one-way trap.

### 3.8 Dealer's capital design has no path to execution

> **EXISTS.** A complete written design — consent-bound tranches, quiescent
> epochs, scenario solvency — plus `dclutch-dealer-scenario-kernel` (735 lines,
> proved: `minimumSplit_is_least`, hostile refusals) and 17 theorems.
> **MISSING OWNER-STEP.** A design pass that produces a descriptor. Verified
> today: `epoch` has **zero** occurrences across
> `crates/dclutch-dealer-codec/src` and
> `crates/dclutch-dealer-scenario-kernel/src`. O-011's closure condition is
> untouched; census Q4 rates Dealer least-live. The design doc's tranche fence
> ("must not simulate tranches with Dealer counters") holds only because
> nothing simulates anything.
> **SIZE.** Design pass + weeks, with a capital-structure ruling inside.
> **TARGET.** Its own charter, not a row in someone else's. Sequence it behind
> §3.1 — fixing the custody derivation is a precondition for any Dealer
> capital work being worth doing.

### 3.9 The plan carries a transport candidate the design already ruled out

> **EXISTS.** Both statements. `WAVE.md:463` names Wormhole Queries as
> *"Candidate permissionless upgrade to verify."* `MAINNET_STATE_RELAY.md` §3
> concluded it is *"not a candidate for v1 and not a near-term upgrade path"*,
> and §3.2 gives the reason (on devnet the guardian set is one test key);
> `MAINNET_STATE_RELAY.md:64` marks the row **"not available."**
> **MISSING OWNER-STEP.** Deleting or annotating one line in `WAVE.md`.
> **SIZE.** One paragraph. Minutes.
> **TARGET.** Whoever next edits `WAVE.md`. **I did not take it**: `WAVE.md` is
> the shared plan and five lanes are live in it right now; a concurrent edit to
> a file everyone appends to is how a wave loses a lane's work. It is listed
> here so the next `WAVE.md` editor can fold it into an edit they are already
> making. The cost of leaving it: a lane picks up Wormhole Queries and re-runs
> a closed investigation.

### 3.10 Relay-slice named lifts, each with a stated trigger and no owner

> **EXISTS.** Four named lifts with stated triggers: large-account chunking
> (an inline window > 448 B needs a persisted SHA-256 midstate); m-of-n with
> m > 1 (threshold expressible, no multi-signer campaign); the Realm-level
> shared observation cache (N markets pay N rents); and §10.1's time bomb —
> after DBC 0.2.0 a decoder pinned to `VirtualPool` silently stops seeing
> transfer-hook pools, with the fix already named (account 5 = `PoolConfig`,
> unbuilt).
> **MISSING OWNER-STEP.** For three of the four: a trigger that has not fired,
> which is correct and should stay parked. For the fourth: the graduation
> market's nine-record set is *"one `getAccountInfo` away and none has been
> read"* — the demo thesis is complete in a harness and has never touched
> mainnet bytes.
> **SIZE.** The nine reads: hours. The lifts: days-to-weeks each.
> **TARGET.** Park three with their triggers written down. **The DBC-0.2.0
> decoder row promotes to Tier 2 the moment a graduation market is real** —
> that is a silent-failure time bomb, not a feature gap, and it should be
> recorded as such rather than sitting in a list of enhancements.

### 3.11 Cross-host reproducibility is unestablished

> **EXISTS.** A complete offline checked-release evidence chain
> (`tools/release/checked-release-candidate.sh`).
> **MISSING OWNER-STEP.** Running it on a second host and comparing digests.
> The script says so in its own capitals (`:6`): *"WHAT THIS PRODUCES IS A
> LOCAL, REPRODUCIBLE RELEASE CANDIDATE."* Nothing shows a second host
> reproduces them. PROJECT_METHOD rung 6; U-011.
> **SIZE.** Hours on hbox (via `swarm-build`), assuming the toolchain pins
> transfer.
> **TARGET.** Pairs with CI (§3.13 note) — both are assurance rungs that matter
> *because* the deployment is already public. This is the cheapest credible
> "our releases are reproducible" claim available, and it is one hbox run.
>
> **CHARTER SHARPENED 2026-08-30 (FRONTIER-2): "one hbox run" is true and the
> precondition is not currently met.** Measured today, hbox is at **104 GiB used
> of 123 GiB** — codex owns the datacake HOL build there and is co-tenant. A
> checked-release reproduction started into 19 GiB of headroom is how a box gets
> power-cycled, and `AGENTS.md`'s box-safety rule exists because that has
> already happened once. **So the row's real first step is a scheduling
> handshake, not a command**: confirm hbox is quiet, then run under
> `swarm-build` (never bare `taskset` — it caps CPU affinity only, and memory is
> what kills the box), with `SWARM_MEM_MAX` set deliberately.
> **Two things worth pinning before the run, so the comparison means
> something:** the toolchain pins must transfer verbatim (the script's own
> capitals at `:6` say it produces a *local* candidate, so "reproducible" is
> exactly the claim under test), and the digests to compare must be recorded
> from a local run *first* — otherwise a mismatch is unattributable between the
> two hosts and the run has to happen twice.

### 3.12 Gen-1's ratified commit/reveal subdivision did not cross the generation boundary

> **EXISTS.** ADR 0006 (gen-1) ratified a commit/reveal subdivision as the
> answer to candidate withholding and proposer bonds.
> **MISSING OWNER-STEP.** A record of why it was dropped, or a design pass to
> restore it. Verified today: `commit/reveal` has **zero** occurrences in any
> `.rs` in gen-3 (only `ASPIRATION_LEDGER.md` and the archaeology doc mention
> it). General ships Consider→Freeze without the subdivision and no record says
> why. **A solver who withholds the best candidate faces no bond and no
> detection.**
> **SIZE.** A design pass; vision-level mechanism work.
> **TARGET.** Tier 4 — *recorded so it stops being re-invented* — but with one
> upgrade on the dig's framing: this is not merely a lost mechanism, it is an
> **unpriced adversary in a live venue design**. It belongs in the same
> conversation as §3.4 (General's collection half), because authoring the seven
> actions without deciding the withholding question bakes the omission in.
>
> **CHARTER SHARPENED — the premise is wrong, and the next lane should not
> spend an hour rediscovering that (FRONTIER-2, 2026-08-30).** This row says
> *"no record says why"* the commit/reveal subdivision was dropped. **A record
> exists, and it is a good one**, in the code rather than in a doc —
> `crates/dclutch-general-adapter-contract/src/candidate_v1.rs:274-295`:
>
> > *"Permissionless and UNBONDED, which is gen-2's answer and the family's:
> > every verb in its collection half was permissionless, gated on windows and
> > counters rather than on identity, and gen-2 carried no bond anywhere. **A
> > bond is a fee on being right as much as on being wrong** — a solver whose
> > valid candidate simply loses the comparison has done the protocol a service
> > — and slashing the honest case is what makes an open solver set close. […]
> > What replaces it is gen-2's real invention: a COMPARTMENTALIZED, FULLY
> > REFUNDABLE WORK ESCROW."*
>
> It goes on to record that the escrow closes a liveness gap gen-2 had: gen-2's
> consideration was permissionless and **unpaid**, so a valid candidate nobody
> cranked before the window closed never competed, and a censored submitter had
> no recourse.
>
> So the honest row is narrower and more interesting. The **proposer-bond** half
> of ADR 0006 was dropped deliberately, with a reason, and the reason is
> defensible. The **commit/reveal** half — withholding, which the work escrow
> does not address at all, because an escrow prices *cranking* rather than
> *disclosure* — is the part with no record. `rg -li 'commit.?reveal'` over
> every non-doc file in gen-3 is still **zero**.
>
> **What the next lane inherits, then, is one question rather than an
> archaeology task:** *does the work-escrow design leave a solver free to
> withhold a better candidate at no cost, and if so is that priced anywhere?*
> That is answerable against `candidate_v1.rs` and `collection_v1.rs` in an
> afternoon, and it is the only part of ADR 0006 that did not cross the
> generation boundary with its reasoning intact.

### 3.13 Three cheap honesty repairs

> **(a) README advertises a relay publication log that does not exist**
> (`portal.dregg.studio/relay/publication_log.jsonl`), while DEPLOY_1 §6.3 has
> the relayer disarmed and §4.11 says a relayer profile *"should not be
> released"* without publication. The site makes a liveness-checkability claim
> the deployment cannot support. **Size: hours** (delete the claim, or publish
> the log). **Target: whoever owns README** — I left it alone because README is
> a launch-surface file and ALIVE/DEVSITE are both live on presentation copy
> this hour.
>
> **(b) `ADOPTED_2026-08-20` cited eight times at two wrong paths.** **LANDED
> TODAY** — see §5.
>
> **(c) `COMPOST.md` promises the repo graft *"will have its own reviewed
> plan"*; none exists.** **Size: hours.** **Target:** fold into the next
> compost pass; it is a promise with no deadline and no reader, which is the
> cheapest kind of debt to discharge honestly (write the plan, or amend the
> promise).

**And a fourteenth, which the dig files under B.6 but belongs here:** ~~there is
no CI.~~ **CHARTER SHARPENED 2026-08-30 (FRONTIER-2): CI now exists, and the
useful statement is what it deliberately excludes.** `9f48f148` — *"ci: the
first checks this repository has ever had"* — added `.github/workflows/checks.yml`
and `pages.yml` **to `~/dev/dragons-clutch`, the publishing repo**, which
vendors this tree as a lagging subtree at `dclutch/`. The live tree still has no
`.github` of its own, which is why the row read as false rather than stale.
Three facts the next lane needs, all verified today:
>
> 1. **No Lean tier, and the file says why.** `checks.yml:20-28`: *"No Lean
>    build. The `check-generated.sh` byte-identity scripts and the four
>    `lean-emit.mjs` verifiers all shell out to `lake`. That is not
>    unaffordable … but it is minutes, and it belongs on a tier that has earned
>    it rather than in the first CI this repo has ever had."* That is a
>    documented tier decision, not an oversight — so the work is *earning the
>    tier*, not arguing for it.
> 2. **The one census step is a no-op today.** `checks.yml:116-123` runs
>    `emission_guard.py --verify` under `if [ -f … ]`, and
>    `dragons-clutch/dclutch/tools/emission-guard` **does not exist** — the
>    directory is not in the published subtree. It emits a warning and gates
>    nothing. **The cheapest real win in this row is refreshing the subtree so
>    that step starts running**, and it needs no new workflow.
> 3. And `--verify` is the *census* ratchet, not a byte check — `COVERAGE.md`'s
>    own words: *"A green census does not mean the bytes match. It means we know
>    which bytes nobody is checking."*
>
> So the row is no longer "write the first workflow." It is: **refresh the
> subtree so the census gate stops being inert, then earn the Lean tier.** The
> original text follows for the record.
>
> ~~No `.github` directory exists in the live tree at all;~~
`checked-release-candidate.sh` greps clean for `lake`, `check-generated` and
`emission_guard`. Every gate this project is proud of runs only when a person
runs it. This is cycle-3 item 4's (SEAM-CI) missing substrate: SEAM-CI built
the *audit*, and nothing executes it on push. **Size: a day for the first
honest workflow (fast gates only, hbox for heavy). Target: cycle-3 item 4.**
Related and sharper than the dig's number:
`tools/emission-guard/COVERAGE.md` now measures the byte-guard gap exactly —
**70 generated files from 68 emitters; 29 guarded, 41 unguarded** — and has a
running tool behind it (`tools/emission-guard/emission_guard.py --run --all`).

---

## 4. The four frontier kernels

Ledger **M-9** and archaeology **A.4** both say "four proved kernels consumed
by nothing." Measured today by dependency edge (`Cargo.toml` references
excluding each crate's own manifest):

| Kernel | LOC | Edges | Status |
|---|---|---|---|
| `dclutch-liability-basis-v2-kernel` | 4,491 | **1** — the root workspace member list, and nothing else | **CONSUMED BY NOTHING. Confirmed.** |
| `dclutch-structured-v2-kernel` | 1,648 | 4, incl. `programs/dclutch-claims-sbf` | **MOVED.** Has a real caller |
| `dclutch-dealer-scenario-kernel` | 735 | 1 — `dclutch-dealer-codec` | Wired one hop, dead at the end of it |
| `dclutch-representation-composition-v3-kernel` | 3,306 | **17**, incl. `programs/dclutch-claims-sbf` | **GENUINELY WIRED** |

**So it is one kernel, not four.** The headline should be retired.

**STATUS: LANDED — `711b8959` (FRONTIER-2, 2026-08-30).** The headline is
retired at all three sites that carried it: `ASPIRATION_LEDGER.md`'s summary
line, M-9's dated amendment (with the full re-measurement table), and the
archaeology's A.4 row that re-inherited it. Two things the re-measurement
found that this section's table does not say:

- **F-5's count is 17, not 8.** `dclutch-representation-composition-v3-kernel`
  has eighteen referring manifests. M-9's table says eight. It was true when
  written; the tree kept wiring and the row did not — which is this document's
  own meta-finding, landing on the document that generated it.
- **M-9 refuted its own headline in its own body and left the headline
  standing.** The Structured caller landed 08-27 and M-9 recorded it two
  paragraphs *below* the table that still said four. That is the failure mode
  worth naming: a row can be amended honestly and still propagate its original
  claim, because what travels is the headline, not the amendment. The fix that
  actually works is editing the sentence other documents quote.

### 4.1 `dclutch-liability-basis-v2-kernel` — the only true orphan

*Proves:* nonnegative integer partitions of unity at runtime width —
partition exactness, the sole capped-ramp apportionment boundary and its
rounding direction, `H >= Q*peak(T)` exact for both admitted families,
split/merge/transfer/redemption preservation, integer de Boor for degrees 1–3,
and an integer hull-membership price gate for degree ≥ 2.
*Consumer would be:* the live `ProductBasisV3` evaluator — if the two are
unified.
*Size:* weeks, behind a ruling. *Disposition:* **cycle-3 / cohort-7 charter**
(§2.6). It is the one row here that is genuinely load-bearing for the product
thesis, and the one where "just wire it" is provably the wrong instruction.

### 4.2 `dclutch-structured-v2-kernel` — retire the orphan label, keep the caveat

*Proves:* one Structured receipt atom backed by `c_i` exact claim-shard atoms
per coordinate; `K_i = S * c_i` derived rather than asserted.
*Consumer:* `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs`
derives its execution descriptor through
`derive_structured_representation_descriptor_v2` over real Structured terms.
The `Cargo.toml` edge is marked *"DEV only: none of these reach the cdylib, and
the ELF digest is the control"* — which is **correct by construction, not a
shortfall**: under decision 0011 §3b Structured has no program and every route
it can execute is a Claims route, so the lowering is host-side and no
`programs/` crate should depend on it.
*Still open:* `hot_v2.rs` has zero non-test callers (the operator's adversary,
deliberately); `RetireCoordinate` / `RetireReceipt` unexercised; the measured
`K = 2` packet cap on a cluster against the `K = 3` RequestProfile ceiling.
*Disposition:* **not compost, not a charter — amend the row.** M-9's headline
is stale in the ledger's own amendment already; A.4 re-inherited it.

### 4.3 `dclutch-dealer-scenario-kernel` — wired one hop into a dead end

*Proves:* runtime-width Dealer scenario solvency and complete-set netting over
borrowed projections, persisting no inventory; `minimumSplit_is_least` and
hostile refusals; 17 theorems.
*Consumer:* `dclutch-dealer-codec` — which is itself not reachable from any
deployed program. So the edge exists and carries nothing.
*Size:* weeks. *Disposition:* **park with §3.8** (Dealer capital design). It is
not an independent orphan; it is the proved half of a design whose executable
half has no path, and it should be triaged with that design or not at all.
O-010's generalization path says "connect or state why not" — stating why not
is the cheap correct move today.

### 4.4 `dclutch-representation-composition-v3-kernel` — not an orphan, delete the row

*Proves:* bounded acyclic representation composition over one native liability
basis; finalized descriptors own the sole Market / result-domain / release-set
/ native-basis identities; nonnegative exact rational composition with
decreasing rank.
*Consumer:* 17 crates including `programs/dclutch-claims-sbf`. Frontier 5 is
the one frontier the ledger already marked **CARRIED**.
*Disposition:* **remove from the orphan list.** Its *operator* (the 9,448-LOC
island at `WAVE.md:500`) is a separate and still-open question; the kernel is
not.

### 4.5 The other frontiers, for completeness

M-10 verdicts still hold except where noted: F-1 partly carried as U-014 (the
measurement never ran); **F-2 is §2 of this document**; F-3 missing; F-4
missing (§3.8); F-5 carried; F-6 missing (the "first lift" happened
incidentally as a hardcoded struct, not the versioned selectable profile the
frontier specified); F-7 **done** — the only frontier fully closed; F-8 width
erasure done, paging missing.

---

## 5. Landed today

Three stale rows corrected in place, chosen because each one is load-bearing —
a lane reading any of them today would size its work wrongly or, in the third
case, believe a safety precondition was unmet when it is met.

1. **`docs/OMISSION_INDEX.md` U-013** — amended with the measurement in §2.2.
   The row is not flipped (the consumer half is genuinely open); its "Still
   required" clause is replaced with what is *actually* still required, per the
   file's own maintenance rule that moving a row requires exact evidence links.
2. **`docs/research/EXPANSION_FRONTIER_2026_08_25.md`** — the degree-≥2 price
   gate paragraph corrected: the Lean exists. The original text is retained
   rather than rewritten, because its *rule* is unchanged and binding; what
   changed is that satisfying it is now a wiring job rather than a proof job.
   The "kernel still has no consumer" bullet is re-measured and confirmed, with
   the distinction between the *kernel crate* and the *capability* made
   explicit so the next reader does not conclude that shaped payoffs are
   missing from the wire.
3. **Two broken citation paths** (dig §B.13.13b).
   `ADOPTED_2026-08-20.md` lives only at
   `dragons-clutch/archive/gen1/docs/decisions/`. `ASPIRATION_LEDGER.md` cited
   it as `docs/decisions/…` (no such file) and `FEE_GEOMETRY.md` as
   `dragons-clutch/docs/decisions/…` (missing `archive/gen1/`). Both corrected;
   the fee-shape decision record is now reachable from the repo that cites it.

**Deliberately not landed**, with reasons, because a wave is entitled to know
what a lane declined and why:

- **The spline field** — not one field (§2). Half-landing it would have been a
  wire-format change to live devnet state under a founding lane's feet.
- **§3.9's `WAVE.md` line** — correct and minutes-sized, but `WAVE.md` is the
  shared plan with five lanes live in it this hour.
- **§3.13a's README claim** — correct and hours-sized, but README is a
  launch-surface file with two lanes live on presentation copy.
- **§3.5's two omission rows** — genuinely minutes, but writing an omission row
  requires reading decision 0005's exact promise, and I would rather hand a
  lane a precise unstarted row than a hastily-worded permanent one.

## 6. What one WAVE entry should say

The dig's own meta-finding is that rows survive only in the thing the plan
reads, and that this document is the next candidate orphan. So, in the form
`WAVE.md` can absorb:

**SCHEDULE:** §3.7 retirement run (ruling satisfied today, needs the run);
§3.11 cross-host reproducibility (one hbox run); §3.6 the Kani harnesses (one
hour, already committed); §3.13's fourteenth — first CI workflow (cycle-3 item
4's missing substrate).

**RULING FIRST, THEN CHARTER:** §2.6 spline evaluator unification (+ correct
M-4's story to ember); §3.2 one-attempt markets forever?; §3.3 FEE-GEO + M-26
the rate; §3.4 GEN-SEVEN in or explicitly post-launch. These four are one
conversation, not four.

**RE-OPEN AN EXISTING ROW:** §3.1 (ADR 0008 §6 item 2 — has an owner, lost its
lane); §3.8 (Dealer capital, wants its own charter).

**PARK WITH A TRIGGER, WRITTEN DOWN:** §3.10's three relay lifts — and promote
the DBC-0.2.0 decoder row to Tier 2 the moment a graduation market is real,
because it is a silent-failure time bomb rather than a feature gap.

**AMEND, DO NOT SCHEDULE:** §4.2 and §4.4 — two of the "four orphan kernels"
are not orphans; the headline should stop propagating.

**CHEAP AND UNCLAIMED:** §3.5's two omission rows (minutes); §3.9's `WAVE.md`
paragraph (minutes); §3.13a and §3.13c (hours each).

---

*Provenance: FRONTIER lane, 2026-08-30, ~10:45–11:45 EDT. Every status claim
in this document was measured at HEAD in `~/dev/dclutch` on the date above,
not inherited from the source docs — which is how §2, §3.1 and §4 came to
contradict them. Verification commands and exact file/line citations inline.
Two supporting measurement passes (spline wiring, Dealer reachability) were run
independently and their load-bearing lines re-verified in the live tree before
being cited here. Read-only on all program and driver code; the only files
changed are the three named in §5 and this one.*
