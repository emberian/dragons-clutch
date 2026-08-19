# Claude → Codex handoff

Session: 2026-08-18 into 2026-08-19. Baseline `fa4efb4`, 112 commits, HEAD
`7ce4c09`. Sibling repo `degg-research` at `18be77e` (36 commits; its most
recent, Draft 8 review editions, is Codex's own).

This is the return leg of [`CODEX_HANDOFF.md`](CODEX_HANDOFF.md). That
document's claim vocabulary (IMPLEMENTED / MODEL / PROPOSED / BLOCKER) is used
here unchanged, and its §10 authority boundary still governs — with one
correction recorded in §9 below.

Both repos have private GitHub remotes (`emberian/dragons-clutch`,
`emberian/degg-research`) and everything described here is pushed. Commits are
unsigned: 1Password declined to sign during autonomous work, which is the
agreed signal for "produced while ember was away."

---

## 1. Read this first: one gate is RED

`programs/clutch-sbf/scripts/run_bringup.sh` **exits 1**. Every step refuses
`AccountCount` (`0x1`).

This is not a mystery and not a defect in the program. Late in the session the
token plane became mandatory (`CreateMarket` creates the outcome mints and the
Hoard token account; the collateral leg is wired), which moved every account
plane: Split/Merge 10 → **16**, Materialize/Dematerialize 10 → **13**,
RedeemInternal 12 → **19**, CreateMarket 12 → **19 + outcome_count**. The
harness still emits the pre-token shapes.

Consequences, already handled or in flight:

- `docs/implementation/SBF_BRINGUP.md` and `LIFECYCLE_WALK.md` carry
  **STALE AS OF 2026-08-19** blocks; their CU tables and differential results
  are historical until regenerated.
- A harness-regeneration lane was running when this handoff was written. If
  its work is present and the gate is green, delete the stale blocks and this
  warning. If not, the lane's brief is reproducible from the breakage list in
  the `50c6e35` commit message, and `programs/clutch-sbf/svm-tests/src/lib.rs`
  is the working reference for the Profile-identity change that cascades
  through every PDA.

**The lesson worth keeping** (§8 expands it): host tests stayed green through
all of this. 328 Rust tests pass at HEAD. Only the SVM gate can see account
planes, and the manifest does not declare it — so a recorded PASS silently
became false. That is *evidence* drift, not documentation drift, and it is the
sharper failure mode.

---

## 2. What is green

At HEAD, all offline:

| Surface | Tests |
|---|---|
| `crates/clutch-kernel` | 23 |
| `crates/clutch-accumulator` | 24 + 2 doc |
| `crates/clutch-batch` | 61 |
| `programs/solana-layout` | 113 + 2 doc |
| `programs/solana-reference` | 41 |
| `programs/clutch-sbf` | 140 + 11 harness |
| `research/vertical-model` | 19 |
| `tools/vector-check` | 11 (+ 25 spine vectors) |
| `lean/` | **86 theorems, 0 `sorry`, 0 project axioms** |
| Python labs | 83 economics + 28 collateral + 41 benchmarks |
| `programs/clutch-sbf/svm-tests` | 15 bank scenarios (separate 1.93.1 pin) |

`MANIFEST.baseline.json` emits clean at **33/33 gates, 37 digests** — but see
§8 on what it does *not* cover.

---

## 3. What landed, by pillar

### 3.1 The Solana program (new since your handoff)

`programs/clutch-sbf` did not exist when you wrote `CODEX_HANDOFF.md`. It is
now a deployable SBF program with a routed instruction set:

- **IMPLEMENTED:** Split, Merge, Materialize, Dematerialize, CreateMarket,
  FeedAdvance, evidence-gated Resolve, RedeemInternal, PlaceOrder,
  CancelOrder, and the genesis family (InitRealm / InitProfile / InitPriceGrid
  / InitTerms / InitOrderPage / Endow).
- **REFUSES with a recorded finding:** SettlePage. On-chain candidate
  verification needs 39–45 KB frames against SBF's 4,096 (measured). The
  streaming verifier (§3.2) is the answer; integration is unbuilt.
- **Real Token-2022 CPI:** mint on materialize, burn on dematerialize,
  collateral in on Split and out on Merge/Redeem via `invoke_signed`, exact
  pre/post balance deltas, extension refusal at market init *and* at every
  token instruction, and a checked mirror between `HoardAccount` and the Hoard
  token account. Atomic revert after a post-kernel CPI failure is
  demonstrated, not assumed.
- Architecture: one module per instruction family over a shared account plane,
  with append-only rules on the shared files. `docs/implementation/SBF_BRINGUP.md`
  carries the ownership map.

`Merge` deserves a note: **the offline reference adapter never implemented
it**, so PROJECT.md's central recombination promise had no executable
semantics anywhere until this session. It now does, at the semantic owner
first, with two decisions written where they live (no collateral-cap check on
the way down; the cash credit follows the kernel step).

### 3.2 Semantic crates

- **`clutch-batch` gained the coupled `BatchRelationV1`.** P1-B is closed
  *structurally*: one global virtual split/merge pair forces a constant net
  imbalance across outcomes, so a cross-outcome "match" is arithmetically
  infeasible rather than refused by a check. Pairing completeness is an O(n)
  per-owner inequality (Hall/max-flow) with a deterministic slice constructor
  and an explicit-witness fallback. Exhaustive oracles: 3,255 + 1,072 flow
  tables and 2,592 books × 9 coordinates, accept-set coinciding exactly.
- **A streaming verifier** (`relation_v1_stream.rs`) whose working set is one
  order: measured **1,280-byte** frames against the monolith's 39,104, with a
  `ClearWorkV1` checkpoint (48,592 B) sealed by a consumed-order fold digest
  that refuses resumption on tamper. Equivalence gate: 19,520 verdict
  comparisons, zero divergences. P-BATCH-03 is stated as its central
  obligation and tested across 210 resume points.
- **`clutch-kernel`** gained `transfer_internal`, `redeem_complete_set` (the
  unconditional P1-A exit), `BasisMode` + `resolve_with_vector`, and
  structural check-before-mutate on all transitions.

### 3.3 Layout and persisted state

P1-C is closed. Eight new accounts (SupplyLedger, Terms, Epoch, PriceGrid,
CandidateRecord, FinalPot, SettlementReceipt, Resolution) plus the clearing
plane (ClearWork, CandidateFeed). **OrderPage v4**: derived positional order
ids (killing the page-burning griefing vector by construction), tombstones for
cancellation, per-order expiry, and a **streaming reader and writer** — the
program's hand-rolled offset chain deleted, net −115 lines while gaining two
capabilities. **TermsAccount v3** unified three converged threads in one
revision: the collateral cap (zero refuses at decode, so a capless market
cannot exist), obligation 18's threshold tables, and the distributional basis.

### 3.4 Evidence infrastructure

- **The vector spine** (`fixtures/vectors/`, `tools/vector-check/`): a
  152-code taxonomy over twelve error enums, 25 vectors, 439 asserted facts,
  and a first executor. Rule: an implementation may never edit a vector to go
  green. Lean already reproduces kernel vectors at build time via `#guard`.
- **`scripts/baseline_manifest.py`**: 33 declared gates with expected
  dispositions, 37 recomputed digests, strict dirty-tree refusal. It caught a
  concurrent lane regenerating goldens between its own emit and check.
- **Proof tools pinned and installed** — Verus `0.2026.08.15.7d4628a`, Rocq
  9.2.0 — which changed them from "unavailable" to *reporting the truth*: the
  E0 probe **fails** under Verus (it needs a `vstd` import that would change
  the pinned source digest — a real strain on the single-source premise), and
  `rocq/check.sh` PASSes as **definition typechecking only** (zero theorems;
  the resolve obligation's same-state conjunct is machine-checked vacuous).
- **The cost lab's `abi-audit` was dead** — erroring on an unpinned identifier
  rather than reporting drift — for several commits before a lane tripped over
  it. Repaired and hardened: unknown tokens are now named drift lines, widths
  evaluate over codec-derived values (killing a lockstep-masking hazard), and
  the parser reads wrapped declarations.

### 3.5 Lean

`lean/` is a dependency-free Lake package (Lean 4.33.0, no Mathlib) modelling
the **semantic plane only** — kernel state, all ten transitions, and the basis
as a partition-of-unity hypothesis on a total weight map. Accounts, bytes,
CPI, PDAs are deliberately absent; they stay Rust, bound by vectors.

The claim shape, which should not drift: *theorem X is proven of the model;
the Rust agrees with the model on N exhaustively-enumerated vectors.* Never
"the program is verified."

It found **four corrections to `DISTRIBUTIONAL_CLAIMS_DESIGN.md` §3.2**, one
of which is a latent hazard nobody had noticed: the `u128` headroom in
`required_for_vector` is a *partition-of-unity corollary*. Naively the
liability sum needs 2¹³²; had that `checked_add` fired, every subsequent
transition on the market would refuse — a bricked market, guarded by an
unstated dependency.

### 3.6 `degg-research`

All seven P1 packets landed: independent FBA oracle (300,436,169-case
differential, zero divergences), the frozen refusal order, relation-IR (the
relation as *data*, 2.1M-case Clear lowering), inclusion/availability (MMR
log, receipts, equivocation verdicts, 131 tests), the Shielded baseline
(composition of all three, with executor freedom **measured** — 377 of 1,125
alternative publications admissible, which is *why* the proof rung is needed),
and the settlement relation. Plus two regulatory evidence corpora
(bundling-invariance, manipulation-cost) and `docs/VERDICTS.md` reconciled.

---

## 4. The theory results

This is the least obvious and probably most valuable content. Full documents
in `docs/research/`.

**The dual is the measure — proved at the degrees we ship, refuted above.**
Under allocation A with the `2a / P-a / N-a-b` tuple, every accepted candidate
is a **zero-duality-gap optimum** of the limit-surplus LP, with the price
vector as the dual. `StrictUnderfill` *is* complementary slackness enforced as
a refusal; V8's cash closure *is* the weak-duality step; V2's trichotomy *is*
the dual-constraint case analysis. **The relation is a disassembled Cert-F
checker** — we built one without knowing it. And at degrees 0–1 every valid
price vector is the basis-moment vector of an explicit atomic measure, so
Breeden–Litzenberger comes pre-inverted. At degree ≥ 2 this is **false with an
executable arbitrage**, which is a second independent reason those degrees
must refuse: they need a moment-body admission gate, not merely an ambiguity
rule.

**But the relation canonicalizes; it does not optimize.** A separate lane
produced a counterexample where a feasible rival ties the LP objective and
beats a lexicographic component, and is refused `CandidateMismatch`. So
"best valid submitted candidate" **stays verbatim**. The reconciliation: we
are LP-optimal, provably; what is uncertified is tie-breaking *among* optima.

**The constraint matrix is totally unimodular** under P-a, so there is **no
integrality gap** — the real gap is fairness versus optimality. And ε = 0 with
exact **integer** duals is available in closed form inside `u128`.

**Positions live in a quotient.** The payoff map is a positive unital linear
map; required collateral is exactly the sup-norm; risk lives in `V/span(1)`
with the quotient norm equal to the half-range; total risk across holders is
identically zero. The free-rebalance set is characterized *with its converse*:
diagonal translations plus representation moves, nothing else.

**Two refutations worth heeding.** Cross-market margining: subadditivity is
*equality* on product joint spaces, so different references and
terminal/terminal calendars get provably **zero** model-free relief — the win
is narrower (statistic inequalities on one feed, e.g. nested windows,
checkable at terms freeze) and comes with a keeper claim: netted requirements
are monotone under partial resolution, so **no post-settlement margin call,
ever**. And: **dispersion is not the quotient norm** — refuted, with the exact
relationship — which surfaced an adversarial finding worth acting on: at
boundary prices the dispersion kernel exceeds `span(1)`, so **risk transfer in
a zero-priced outcome is feeless**.

**On verified bytecode** (`docs/research/VERIFIED_BYTECODE_PATHS.md`, 37 cited
sources): no. sBPF already has mechanized semantics (CertSBF in Isabelle/HOL;
a Lean 4 port maintained by the Solana Foundation, Apache-2.0) — but *nobody
models syscalls*, and our correctness rides on address derivation and
`invoke_signed`. EVM is far more mature in Lean and the economics kill it
(~$0.0004 on Solana vs cents on an L2 vs dollars on L1). Lean → SBF is
infeasible. The decisive argument is scope: **every P0 in
`ADVERSARIAL_REVIEW_V0` was a missing predicate, and all six would have
compiled perfectly.** Verified bytecode buys absence of miscompilation and
nothing against our actual defect history.

---

## 5. Decisions queued for ember

Not for us. Listed so they are not silently absorbed.

1. **Filings**: Conflicts NPRM candidate (due Oct 5) and the Data-Q4 insert —
   both drafted with go/no-go headers, awaiting a word. The John review packet
   is structured as **round 1 of 2** with a round-2 delta protocol.
2. **Policy freezes**, now with evidence: residual-pair settlement (1a/1b/1c
   all implemented), lots vs one-hot vs remainder credits, AON poisoning, fee
   carry domain.
3. **The fee-base fork** — implied-measure risk (tails near-free per unit of
   worst case) versus model-free risk (price-free, fee/consideration unbounded
   on cheap claims). Economics, not mathematics; cannot be closed by proof.
4. **`PROJECT.md` §9 forbids cross-market netting**, which is what the joint
   sup-norm result enables. Charter versus capability.
5. **The single-truth token cutover** — delete `ExternalAccount` in favour of
   the mint's supply. The reconciliation check is already in place so the
   cutover becomes a deletion.
6. **One paid Bedrock MPC-TLS session** would produce the first live
   provider-attested transcript (breadstuffs already has a *real*
   `api.coinbase.com` session recorded, 2026-07-11 — so the honest ceiling is
   "no live **model**-provider session").
7. **Vendored `solana-define-syscall` provenance sign-off** (verbatim
   Apache-2.0, checksummed).

---

## 6. Named gaps

`docs/implementation/DRIFT_REVIEW_2026-08-19B.md` carries a **22-row gap
ledger** with owner, file:line admission, and S/M/L estimate. The largest:

- on-chain settlement (streaming verifier integration + ClearWork account);
- the page → `BookV1` projection (canonical ids need the whole set; owner-tag
  bijection unproven; expiry now persisted but epoch freeze absent);
- multi-position closure on the *program* side (the reference has CLO-DELTA-V1;
  the program still enforces the stronger single-position equality, which is
  fail-closed and checkable but diverges);
- `ResolutionAccount` carries a payout *index* and no `resolved_value`, which
  is what keeps the account path bridged through preset membership at
  degree 1;
- account-creation, rent, and `invoke_signed` seeds for the clearing plane
  (48,750 B exceeds the 10,240 B per-instruction growth cap — the analysis and
  both options are written up);
- the manifest does not declare the SVM lane (this session's regression).

---

## 7. Suggested next units, ranked

1. **Land the harness regeneration** and delete the stale blocks. Then
   **declare the SVM gate in the manifest** so this class of staleness becomes
   a refusal.
2. **Fork `leanprover-solanalib` and model the syscalls.** Ember explicitly
   encouraged this. The gap is precisely shaped: their tree has the ISA and
   models `Account`/`Instruction` against the real crates, but treats
   `sol_try_find_program_address`, the `sol_mem*` family, and
   `sol_invoke_signed_rust` as axioms — the exact three our correctness rides
   on. Their validation harness is a dead link; ours is byte-exact against a
   real bank. We would be adding the half nobody has.
3. **Aeneas/Charon spike.** Rust → LLBC → Lean. If it handles our kernel
   (`no_std`, no `unsafe`, fixed arrays, checked arithmetic — unusually
   friendly), the two-implementation cost may be avoidable entirely, closing
   `ARCHITECTURE.md` §10's first arrow with a theorem instead of vectors.
4. **The account/authorization plane in Lean** — where every P0 we have ever
   had actually lived — with the SVM differential repointed as its fidelity
   check.
5. **On-chain settlement**: ClearWork account + streaming verifier
   integration, on the projection spec in `STREAMING_RELATION_DESIGN.md` §10.
6. **The payoff compiler.** In the B-spline basis this is simpler than it
   sounds — coefficients *are* the payoff sampled at knots, and the
   approximation certificate is the interpolation error, exact at degree 1.
   The hard part is the partition compiler that turns a path predicate into a
   finite state space.

One connection worth carrying: **a bounded quoting policy is a guarded
commitment.** Eager shape — inventory limits, payoff regions, max loss,
expiry; late witness — the fill. That is the Dregg calculus exactly, so
"prove every reachable quote stays reserved" has existing machinery aimed at
it.

---

## 8. Operational lessons paid for in real debugging

- **Host green is not SBF evidence.** Four functions overflowed the 4 KiB
  frame invisibly to every host gate; only `cargo-build-sbf` said so. Frame
  checks belong in every lane's gate list. (Also: the stack analyser runs
  *before* `--gc-sections`, so flagged functions may not reach the image —
  check the linker map before believing a diagnostic.)
- **Evidence goes stale, not just docs.** §1 is the example. Any gate that a
  test suite cannot see must be declared somewhere that refuses.
- **A drift gate that errors is worse than one that fails.** `abi-audit` sat
  dead for several commits because an unknown token produced a quiet exit 2.
  Loud beats correct-but-quiet.
- **Refutable pins.** Adopted from breadstuffs: pin a golden *and* test that a
  drifted, truncated, wrong, or absent golden is refused. Same family:
  stored diagnostics must never decide acceptance.
- **Ban `native_decide` in `lean/`.** It can currently prove `False`, and
  Lean's FAQ puts its compiler and codegen inside the TCB.
- **Commit messages**: write prose messages to a file and use `git commit -F`.
  zsh command-substitutes backticks and chokes on brackets inside `-m`.
- **Remote builds**: ship `git archive HEAD | gzip`, never `rsync`. Breadstuffs
  was 213 MB versus 30.7 GB, and it pins exactly the tree tested.
- **Run the narrowest thing that could refute you.** Unfiltered `-p <crate>`
  suites are a resource grab; state your control separately.

---

## 9. Authority — one correction to `CODEX_HANDOFF.md` §10

Ember's direction, 2026-08-18: **ordinary local commits are default work and
need no authorization.** The explicit-commit-authorization rule was a Codex
practice, not project policy; `AGENTS.md` in both repos now says so. Pushing,
tagging, publishing, and releasing remain user-directed — and everything in
this session is pushed under that direction. `persvati` and `hbox` are
ordinary dev infrastructure, not gated remotes.

Unchanged and still absolute: no deployment, no mainnet or devnet, no real
funds, no market creation, no regulator contact, no filing, no solicitation,
no describing any program or URL as official. Gate L0 remains open. The
filings are DRAFT and NOT FILED; identity fields are placeholders; the
designated-reviewer courtesy read has not happened.

---

## 10. Where I was wrong

Recorded because a handoff that only lists wins is not useful.

- I predicted our clearing would decompose into per-tick LP optimality plus
  exhaustive tick comparison. **False**, with a counterexample.
- I called the forked plonky3 issue a soundness hole. It is a **completeness**
  bug — upstream emits proofs its own verifier rejects; nothing forged is
  accepted.
- I said sBPF was formally virgin territory. It is not; the Solana Foundation
  maintains a Lean 4 semantics.
- I oversold cross-market margining before the mathematics came back and
  narrowed it to same-feed statistic inequalities.
- I drifted toward wrap-up twice while ember was still working, and was
  corrected both times.

The pattern in all five: the swarm's exhaustive oracles and adversarial lanes
caught what confident prose did not. Keep pointing them at our own claims.
