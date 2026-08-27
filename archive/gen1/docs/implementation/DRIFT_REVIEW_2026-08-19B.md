# Drift review — 2026-08-19 close B (waves 5-8: dragons-clutch + degg-research)

Closing review of the wave-5/8 swarm output: dragons-clutch `d936eaa..HEAD`
(63 commits, ~74k inserted lines: multi-position closure, evidence-gated
resolution + the SBF instruction set across all families, OrderPage v4 +
streaming writer + orders integration, TermsAccount v3 + degree-0/1
resolution + cap flow, streaming relation verifier, the Token-2022 CPI leg +
svm-tests, the lifecycle walk, the vector spine, abi-audit death and
resurrection, the static client, cost re-pins, baseline manifests) and
degg-research `a1a57c1..HEAD` (19 commits: relation-IR,
inclusion/availability, shielded baseline, refusal-order freeze, VERDICTS
reconciliation, C-1 closure, bundling + manipulation corpora, Drafts 5-8 +
perpetuals + operatorless addendum + landscape/matters maps, legal packet,
typography).

Method: actual diffs, code, and test bodies read — never lane summaries. The
reviewer read the dimension-A ownership seams (token.rs, resolution.rs, the
streaming relation verifier, the v4 stream writer vs the buffered encoder,
the CLO-DELTA closure code in both adapters, dispatch.rs and the family
modules), the dimension-B refusal code paths, LIFECYCLE_WALK end-to-end with
its terminal arithmetic re-derived by hand, the actual recorded ELF's dynamic
symbols, and the manifest structure directly. Scoped sub-reviewers read the
degg night commits, the docs-coherence surface, the vacuous-test sample
(~49 test bodies across both repos), and the gap-ledger evidence, each
reporting file:line; every load-bearing sub-reviewer claim used below was
spot-verified against the tree, and one was **refuted** (see the P1 note —
the reference adapter did *not* gain a CancelOrder arm; lib.rs:4837 is a
test pinning `UnsupportedIntent`).

Suites re-run locally, all green: kernel 16, accumulator 24+2, batch 61
(was 44), layout 97+2 (was 38), reference 38 (was 20), clutch-sbf 119+11,
benchmarks unittest 41, `cost_lab.py check` 261 scenarios, `cost_lab.py
abi-audit` PASS at the `e780d5b` pin, vector spine 25/25 (93 steps, 439
asserted facts), degg relation-ir 21 (incl. the 551,784-case domain-C
differ), inclusion-availability 131, shielded-baseline 52, bundling 39,
manipulation-cost 53. The SVM gates (`run_bringup.sh` incl. the lifecycle
walk, `svm-tests`, the token2022 probe) were **not** re-run — their evidence
is hours old, recorded, and digest-pinned (clean-tree ELF `d8a9267c…` at
`5c88505`); the reviewer instead verified the recorded artifacts (including
reading the actual ELF's symbol table).

This review applied fourteen wording/status fix clusters (listed below) and
made no code-semantics or refusal change. Nothing here is a claim upgrade:
both repos remain **tested, not verified, not deployed**, and
"testnet-deployable" appears nowhere as an achieved claim (checked: the only
occurrences are the GOAL.md goal statement and SBF_BRINGUP's explicit
negative).

## Summary

| Dim | Scope | Verdict |
|---|---|---|
| A | Semantic ownership (token CPI, derive_payout, preset bridge, streaming verifier, v4 writer) | **PASS** — two flagged-dual-impl P3s |
| B | Refusal integrity (evidence-absent, degree≥2, R-16, cap-zero, versions, DarkTarget, sealing) | **PASS** — one P3 (Debug on sealed plaintext, degg) |
| C | Vacuous-test hunt (~49 bodies, both repos; lifecycle numbers hand-traced) | **PASS** — genuinely strong; zero vacuous |
| D | Claim vocabulary (both repos) | **PASS after fixes** — overclaims concentrated in the commit-subject/GOAL layer, not the docs |
| E | Cross-doc coherence (SBF_BRINGUP vs dispatch, manifest, decision lists, cost goldens) | **FAIL found → repaired in-review**; two structural P2s remain (manifest, handoff) |
| F | Remaining-gap ledger | **22 rows** below |
| G | Morning decisions | consolidated below (one list, both repos) |

Worst finding severity: **P1** (documentation-truth, repaired in-review; no
correctness or refusal regression found anywhere in the wave).

## The P1, and why it happened

**SBF_BRINGUP.md contradicted the dispatched code in both directions.** At
review start, the doc's status ¶1 claimed SVM evidence for "seven of the
eight" families "except `Resolve`" while its own ¶2 and Results recorded
`Resolve` executing at 536,123 CU; the module map said "`Resolve` does not
fit a transaction, see deferred check 15" while check 15 said **Closed**;
the refusal table and the normative **"Correct description"** section said
cancellation was a stub while `dispatch.rs:81-84` routes
`Intent::CancelOrder` to a real handler (`orders_batch.rs:774-792`,
`cancel_order`, landed with the orders-on-v4 integration `3b6404a`); and
deferred check 9 plus the dynamic-symbols paragraph said "no CPI and no
token code" while the recorded clean-tree ELF (`d8a9267c…`, whose digest the
same doc cites) **imports `sol_invoke_signed_rust`** — verified by reading
the ELF at `programs/clutch-sbf/svm-tests/tests/fixtures/clutch_sbf.so`.
Cause trace: `3b6404a`'s commit message claims "stale cancellation-refusal
claims corrected" but touched only README/crate docs; `5c88505` (token leg)
updated TOKEN2022_PLAN.md but not SBF_BRINGUP; `01d0008` and `1bad84d` later
edited SBF_BRINGUP without re-truing those rows. Four stale-in-the-same-
direction crate docs mirrored it (`instructions/mod.rs` — with an editing
splice that ate half a sentence, `dispatch.rs`, `program/src/lib.rs` — with
a second splice, `README.md` — "fix is in flight" for a fix that landed).

All of it is now corrected against the dispatch map (fixes 5-8 below), and
the corrected instruction truth is: **ten families implemented** — the eight
reference-oracled ones all with byte-exact SVM evidence, `PlaceOrder` and
`CancelOrder` host-tested with no oracle and no SVM leg — and `SettlePage`
the one honest stub. One important correction to a sub-reviewer claim is
recorded here so it does not propagate: the *offline reference adapter still
models no order family* — `programs/solana-reference/src/lib.rs:4821-4865`
is a test asserting `CancelOrder`/`SettlePage`/`FeedAdvance` refuse
`UnsupportedIntent` — so CancelOrder's on-chain implementation is
oracle-less by design, exactly like PlaceOrder's.

## Findings by severity

### P2 (structural — not applied here)

1. **The wave-8 manifest does not witness, and now misdescribes, the SVM
   lanes (dim E).** `MANIFEST.baseline.json` (regenerated clean at
   `c3517a3`) carries 33 gates and 37 digests; none of them touch
   `programs/clutch-sbf/svm-tests` (its own workspace, toolchain pin, and
   committed evidence), the lifecycle-walk gate, `toolchain/probes/token2022`,
   or `tools/vector-check`, and the digest inventory has no ELF digest and no
   clutch-sbf lock. That alone repeats prior-review P2-2 one layer up (the
   617a521 widening covered the host commands and stopped there; the
   deliberate-exclusion addendum `docs/implementation/BASELINE_MANIFEST.md:276-283`
   predates all three newer lanes). Worse, the hardcoded `NOT_ATTESTED`
   text (`scripts/baseline_manifest.py:89-91`, emitted into the manifest and
   repeated at `BASELINE_MANIFEST.md:229-230`) still asserts "no SBF runtime
   evidence: no entrypoint, program-test lifecycle, Token-2022 CPI,
   CU/stack/heap measurement" — accurate when authored, **false as repo
   description** at a regen whose own `baseline.commit_subject` is
   "Clean-tree ELF record: gates PASS at 5c88505". The scope-of-attestation
   meaning survives; the wording does not. Fix belongs in the generator
   (reword `NOT_ATTESTED` to scope it to "not witnessed by this manifest's
   gates", add gates or declared exclusions for the three lanes, pin the ELF
   digest) followed by a re-emit — a coordinator action, not a review edit.

2. **CODEX_HANDOFF.md was untouched by the whole wave (dim E).** Zero
   occurrences of "clutch-sbf" anywhere in it; §5 lists only the pre-SBF
   commands (five current manifest gates missing); §7 blocker 3 still says
   "no accepted native SBF entrypoint, Token-2022 CPI path, program-test
   lifecycle" — all false now. This is the un-done half of prior-review
   P2-2 ("the gates **and** a handoff §5 entry").

3. **degg: candidate-gate headers went stale against landed conversions
   (dim D/process) — annotated, decision preserved.** The perpetuals
   candidate said "not in the Typst tree, converted nowhere" while
   `typst/perpetuals/` + `output/pdf/cftc-perpetuals-comment-draft-1.pdf`
   landed; the IAC addendum candidate said "not part of any draft" while the
   operatorless section sits in Draft 8; `JOHN_REVIEW_PACKET.md:15` already
   counts four filings. The go/no-go is recorded nowhere in-repo. Both
   headers now carry dated status notes (fixes 13-14) stating the
   conversions landed, nothing is filed, and the go/no-go remains the
   author's. The **process** finding stands for filing week: conversions
   outran their decision surface — ratify or retract explicitly (G.1).

### P3

4. **`observe_resolve.rs` re-implements C1+C2 inline (dim A).**
   `check_closure` (observe_resolve.rs:1556) restates the closure checks
   that `accounts.rs` (`require_two_term_closure`:332,
   `require_representation_bound`:362) exists to share — and which
   `split.rs:562,923` and `market_init.rs:1119,1195` do compose. One
   decision procedure, two in-program transcriptions, held together only by
   the SVM differential. The accounts.rs header comment now names this
   (fix 5); unifying it is an instruction-lane change.

5. **The mirrored payout-vector validator has no cross-crate agreement test
   (dim A).** `PayoutVectorBytes::validate_active`
   (solana-layout lib.rs:2516) restates `clutch_kernel::PayoutVector::validate`
   (kernel lib.rs:69) because the layout crate is deliberately
   dependency-free. Drift is fail-closed — the preset-membership bridge
   resolves only into the kernel-accepted frozen set, so a divergence can
   at worst over-refuse — but the reference crate (which depends on both)
   pins no agreement matrix. Same lineage as the prior review's
   mirrored-bounds nit; recorded as debt.

6. **degg: `SealedLocalOutput` derives `Debug` while holding plaintext
   (dim B).** `experiments/shielded-baseline/src/seal.rs:309` —
   `format!("{:?}", sealed)` yields every owner-output field with no
   capability, technically falsifying the "no accessor without a
   capability" claim (seal.rs:27-28; SHIELDED_BASELINE.md:115). The
   compile_fail doctests pin field access and `open()` arity, not `Debug`.
   (`PartialEq` also gives an equality oracle.) One-line fix in the lane's
   own code, not applied by a review.

7. **degg: the 300,436,169-case "zero divergences" has no committed
   machine-readable full-run artifact for domains A/B (dim C).** The differ
   program is committed and reproducible, the doc tables
   (DARK_FBA_RELATION.md:556-561) are internally consistent (arithmetic
   re-checked), and domain C (551,784 cases) runs in-test; but the full A/B
   sweep's counts live only in prose tables. Prior precedent (the 08-19
   review reproduced the 300M sweep in 7.4 s) suggests committing the run
   log or keeping the reproduction habit.

8. **degg: `OPERATORLESS_AGENTS.md` uses "deployed" in two senses (dim D).**
   ":6-7 no artifact discussed here is deployed" vs ":128/:178 the deployed
   plonky3 IR-v2 prover/verifier" (a third-party artifact that genuinely is
   deployed). Accurate but ambiguous in the one memo where the word is
   load-bearing; a qualifying clause ("deployed by its upstream project")
   would close it. Also **VERDICTS.md** never links the AGENTS.md status
   legend, so an external reader can misread rung-1/2 "VERIFIED" as formal
   verification (each use is individually bounded).

9. **Prior-review P3s still open:** the vendored crate still ships no
   Apache-2.0 license text beside the verbatim tree
   (`programs/clutch-sbf/vendor/` holds only PROVENANCE.md + the crate);
   the attestation survey still has no artifact home in either repo.

### Nits (recorded, no action)

- SBF_BRINGUP:919 "sound because the terms account … already proved the
  digest" — proof vocabulary for a SHA-256 recheck; and "deployable" as a
  bare adjective (SBF_BRINGUP:51, clutch-sbf README:3) is drifting toward
  the goal phrase — both docs disclaim deployment, so recorded only.
- The v4 byte-pin fixtures use `expiry_epoch = u64::MAX` (layout
  lib.rs:4865,4887), so a finite expiry never appears in the pinned bytes;
  expiry semantics are covered separately (lib.rs:8072).
- The commit `a7f513e` phrase "1,280-byte frames" is the `push_order` SBF
  *stack-frame* size (STREAMING_RELATION_DESIGN.md:328), not a wire-frame
  size.
- degg date inconsistencies: C024 says the differ ran 2026-08-19,
  DARK_FBA_RELATION §13.6 says 08-18; several records are future-dated
  2026-08-19 in 08-18 commits; commit `d3394d9` counts "four surprises"
  where VERDICTS V10 counts "three results against expectation".
- The cost lab's landed-intent order rows (`place_order` 7 accounts,
  SUMMARY.md:91) are labeled account-set *hypotheses* — correct labeling,
  but the on-chain instruction now exists with **4** accounts
  (orders_batch.rs:479-493), so the rows are reconcilable (G.7).

## Dimension detail

**A — semantic ownership: PASS.** (1) *Token CPI*: `token.rs` owns exactly
observation/admission and CPI construction (module contract, token.rs:1-17);
grep-verified **zero** `invoke`/`invoke_signed`/`Instruction{` construction
anywhere else in the program; quantities and ordering stay with the family
modules (split.rs:662-706 decides when the CPI happens and pins exact
post-CPI deltas); the deliberately re-written TLV walk names svm-tests as
its divergence detector. (2) *derive_payout*: exactly one home
(`programs/solana-reference/src/resolution.rs:799`, plus the §5.1 sibling
`derive_payout_vector`:846); the SBF gate **imports** it
(observe_resolve.rs:217-218) and its module header declares "no economic
logic". (3) *Preset-membership bridge*: `derive_payout` on degree ≥ 1
derives the validated vector and returns the index of the byte-equal frozen
preset, refusing R-16 otherwise (resolution.rs:822-833) — the membership
scan duplicates no kernel validation; the one shape check runs through the
layout-owned `validate_active` (finding P3-5), and membership in the
kernel-accepted preset set is the operative guard, so validator drift is
fail-closed. The missing kernel `resolve_with_vector` transition is named
in the module docs as "the one named residue" (resolution.rs:56-66) —
honest. (4) *Streaming relation verifier*: `relation_v1` owns the policy
types (`AllocationPolicyV1` etc. are imported, relation_v1_stream.rs:66-72);
the streamed loop is a declared second implementation with the batch
verifier as truth ("a divergence there is a finding, never a tune",
relation_v1_stream.rs:17-22), held by exhaustive equivalence gates with
anti-vacuity counters (2,592 books × 9 coordinates × 7 mutation families;
`compared > 8000` guards; 210-point checkpoint resumption; tamper
falsifiers). (5) *Page v4 one-byte-truth*: held by
`the_streaming_writer_produces_exactly_the_buffered_encoders_bytes`
(layout lib.rs:8150) — full-page array equality against the buffered
encoder at three stages (empty, order+portfolio appends, tombstone
retirement) plus refusal-parity and 14-position hostile-byte decoder
equivalence. The module doc's "no second transcription" claim was
overstated for the header prefix (the field sequence *is* transcribed twice,
lib.rs:1990-2004 vs stream.rs:824-838) — corrected to name the pin as the
guard (fix 1). CLO-DELTA-V1 closure reads clean in both adapters (reference
lib.rs:1766-1856: C1 two-term vs kernel aggregate, C2 one-sided counterfeit
bound, C3 checked delta write-back; SBF composes the same shapes), with
finding P3-4 the one seam.

**B — refusal integrity: PASS.** Evidence-absent still fails closed
everywhere: the reference refuses via `ok_or(ResolutionEvidenceUnavailable)`
(lib.rs:1255,1273) with both regression bodies intact, and in the SBF gate
the absent case is *structurally inexpressible* — the evidence plane is a
parameter, not an `Option`, and `process` cannot build one without the
terms/resolution/buffer accounts the account count requires
(observe_resolve.rs:1281-1285); the malformed-blob suite pins ten-plus
distinct refusals (observe_resolve.rs:3004-3050). Degree 2|3 refuse
`TermsMalformed` and degree > 3 `BasisMalformed` (resolution.rs:425-437,
tested at reference lib.rs:4468-4474). Unrepresentable derived vectors
refuse **R-16** `DerivedVectorUnrepresentable` (resolution.rs:822-833) with
a real falsifier (D=64 derives (40,24), no preset, refuses; positive control
at the knot — lib.rs:5445). Cap-zero refuses at decode on **both** paths
(`validate`/`validate_prehashed` via `decode_into` and
`decode_unchecked_into`, layout lib.rs:2874-2877, tested with digest
re-freeze at lib.rs:5818). Superseded layout versions refuse through both
readers: terms v1/v2 (lib.rs:5642, decode and decode_unchecked) and page
v1/v2/v3 (lib.rs:5360-5372, buffered and streaming). degg: DarkTarget
refuses at three levels (typed lowering refusal, mandatory-first-rule
structural check, batch-evaluator class — relation-ir lower.rs:304-324,486-493)
with differential coverage; the shielded public role cannot reach sealed
data by construction (private fields + keystream on the wire form,
compile_fail pins, byte-window scans; the empty `PublicVerifier` type takes
only public inputs — receipt.rs:493-497, roles.rs:202-214) with finding
P3-6 the one hole; the refusal-order freeze is pinned by tests whose
expectations come from an independent naive re-derivation, and §13.6
correctly downgrades post-freeze agreement to *conformance, not
corroboration*.

**C — vacuous-test hunt: PASS, unusually strong.** ~49 bodies read across
terms-v3 adversarial (byte-patch + digest-re-freeze + distinct codes),
degree-1 exactness (exhaustive pane sweep with the design formula as the
stated expectation, hand vectors at the off-by-one-sensitive points 7/8/9,
partition-of-unity, two-path cross-agreement), lifecycle
(`walk-terminal` derives every number twice — const walk arithmetic vs
decoded reference terminal — then reads a third copy out of bank bytes at
offsets located by *probing the frozen codec*, and both falsifiability
self-checks are recorded firing), token_leg.rs (bank-read-back deltas
against hand literals, four genuine negatives incl. the out-of-band-burn
DoS and wallet-signed outflow), streaming equivalence (verdict identity
including error payloads, mutation families, anti-vacuity counters, tamper
refusals), v4 byte-pinning (above), the vector spine (real-crate executors,
handwritten provenance vectors hand-checked, TAX-6 machine-guards against
silent coarsening, eleven drift findings recorded none suppressed), and the
degg suites (equivocation with genuinely forged divergent logs and a
32-construction negative sweep; C-1 closure inverts the old gap-test
assertions and adds full conservation — fails on the first assert against
the old behavior; bundling witnesses re-validated semantically and pinned
against live recomputation; manipulation table cross-checked bisection vs
closed form). The reviewer hand-traced the walk's terminal numbers
independently: cash 64−20+4+13 = **61**, hoard 20−4−13 = **3** =
unredeemed externals 8−5, supplies (0, 3), conservation 61+3 = 64, and the
CU column sums to the claimed 2,489,442. All correct.

**D — claim vocabulary: PASS after fixes.** "Testnet deployable" appears as
an achieved claim nowhere; "evidence-gated is not authenticated" holds
verbatim in the new material; TOKEN2022_PLAN's status table and "Correct
description" match the code exactly (including the optional-leg hole and
"constructed, not wired"); STREAMING_RELATION_DESIGN states host-only
plainly; LIFECYCLE_WALK's hedging (skip list, "What this walk is not",
recorded self-checks) is exemplary — and its header now points at the skip
list (fix 8). The overclaims sat in the *log layer*: GOAL.md's
"section-10 items 1-10" (items 2/3/8 are skipped, 1 and 6 half-driven),
"Markets are now FUNDABLE" (the cap decision is structural; no endowment
instruction exists), "proven on real bytes" — all corrected (fixes 2-4);
commit subjects `01d0008` and `3b6404a` carry the same overshoot and are
immutable, recorded here. degg holds the line remarkably well (the honest
core is titled as such; the lying-executor residual is documented three
times; V-39/V-41 forbid "deployed"/"verified" wording in filings), with
P2-3/P3-8 the residue.

**E — cross-doc coherence: FAIL found → repaired; two P2s remain.** The P1
above was the failure; after fixes 5-8 the instruction map, refusal table,
Correct description, crate docs, and README agree with `dispatch.rs` and
with each other. MULTI_POSITION_CLOSURE's stale coordinator note and
accounts.rs's stale "nothing calls them yet" are annotated with the landed
truth (fixes 5, 7). COST_LAB's superseded v3 pins are marked; its constants
themselves check out against the v4 codec (ORDER_RECORD 107, PORTFOLIO 235,
TOMBSTONE 80, SLOT 236, page 4,012, TERMS 1,656 schema 3 — abi-audit PASS
at `e780d5b`). SOLANA_LAYOUT.md correctly describes v4 and names the
version refusals. The manifest and handoff findings (P2-1/P2-2) are the
open structural work. The three morning-decision lists reconcile as
follows — 08-18 items: residual/fractional/T-phase/fee freezes OPEN
(deliberately; evidence executed), econ-defaults DONE (`7ca01ad`),
VM-INT pair OPEN (trace still named `coupled.trace`, R-b still
unexercised), vector-spine gates PARTIAL (the spine landed `89e329c`;
"G1-G7 are human decisions and none of them is made by this drop"),
E0/Verus posture OPEN, VERTICAL coupled section DONE (`7ca01ad`);
08-19 items: obligation-18 terms revision **DONE** (`927d4bc`, "obligation
18 discharged" in the adapter doc), manifest/handoff coverage PARTIAL
(gates half at `617a521`; handoff half not done; the newer SVM lanes now
also uncovered — P2-1/2), vendored-crate sign-off OPEN (no license text
added), degg filing week PARTIAL (Draft 8 + perpetuals Draft 1 landed as
*candidates*; sends and freezes remain), Bedrock session OPEN.

## F — the remaining-gap ledger: what "testnet-deployable" still requires

Every row is the wave's own admission, read at the pointer. Size: S ≲ a
session, M ≈ a lane, L ≈ a design round + lanes.

| # | gap | owner | evidence | size |
|---|---|---|---|---|
| 1 | Endowment/prepay instruction — nothing moves collateral into a Position; the walk credits opening cash in the fixture | `clutch-sbf/program/src/instructions/` + a reference-adapter oracle transition | LIFECYCLE_WALK.md §item 2 — "**SKIPPED.** No endowment, prepayment, or deposit instruction exists … the sharpest gap in the walk" | M |
| 2 | Realm/Profile/price-grid/terms init instructions — the Realm plane is loaded at genesis as frozen bytes | `market_init.rs` family + reference oracles | LIFECYCLE_WALK.md §item 1 — "There is no Realm, Profile, price-grid, or terms initialization instruction in this program" | L (M per account family) |
| 3 | Account creation via system-program CPI (rent exemption, `invoke_signed` plumbing) | `clutch-sbf/program` (seeds.rs exists; CPI layer absent) | SBF_BRINGUP.md deferred check 2 (:417-427) — "unwritten and untested"; restated "CreateMarket … cannot create an account" | M |
| 4 | Mint creation at CreateMarket (outcome mints + Hoard token account) | `market_init.rs` + `token.rs` | TOKEN2022_PLAN.md:31,43 — "**not implemented**"; blocked behind row 3 | M |
| 5 | Collateral leg wiring — `TransferChecked` into/out of the Hoard for Split/Merge/RedeemInternal | `token.rs` → `split.rs`/`observe_resolve.rs` | TOKEN2022_PLAN.md:29-30 — "constructed, not wired"; the CPI pattern to copy landed at `5c88505` | M |
| 6 | On-chain streaming-verifier integration + ClearWork account (48,592-byte body, `consumed_fold` bound to `order_set`) | `clutch-batch` (done host-side) + layout account types + a new crank instruction | relation_v1_stream.rs:3-5,24 — "nothing here is an SVM relation"; STREAMING_RELATION_DESIGN.md:369-374 | L |
| 7 | SettlePage — refuses `NotYetImplemented`; the streaming verifier removed only the *frame* blocker | `orders_batch.rs:796` | STREAMING_RELATION_DESIGN.md:383-388 — "Until the projection lands, SettlePage keeps refusing"; second blocker: page→`BookV1` projection (orders_batch.rs:271-283) | L (= rows 6+9+10 + freeze + reservation) |
| 8 | Multi-position program-side residue — C1-C3 are ported and compared on-chain; missing: a position-init instruction (behind row 3) and any multi-position *measurement* | `clutch-sbf` accounts.rs/instructions | SBF_BRINGUP checks 10/13 (**closed**) vs LIFECYCLE_WALK "Not an envelope … multi-position closure … unmeasured" | M |
| 9 | Owner-tag bijection — an owner tag into `owner_count` is an unchecked claim | layout admission; enforcement designed as ClearWork first-appearance interning | SOLANA_LAYOUT.md:368-369,885-886; STREAMING_RELATION_DESIGN.md:358 ("bijective … *by construction*" — design, not code) | M |
| 10 | Order-set commitment binding at batch time — nothing on-chain freezes: `init_page`/`frozen_set_commitment`/`seal_page` "exist and are called by nothing", `order_set` stays zero | layout `stream` + an epoch-freeze instruction + ClearWork binding | orders_batch.rs:330-336; SOLANA_LAYOUT.md:844-847 ("a contract", not a proof) | M |
| 11 | Notary/attestation posting (degg) — the spec "does not retire the gap it addresses"; six unbuilt items (codec+vectors E1, admission checker E2, end-to-end verifier E3, transport, value leg, hash profile) | degg `ATTESTATION_POSTING_PATH.md` → artifacts | ATTESTATION_POSTING_PATH.md:692-698, 838-853 | M (E1-E3) / L (on-chain) |
| 12 | Lying-executor residual (degg) — the composed verifier constrains tick/volume "not at all"; 377/1,125 alternative publications admissible; rung 5 is open research | degg shielded-baseline + relations | SHIELDED_BASELINE.md §6; VERDICTS.md:90-93 | L |
| 13 | Kernel `resolve_with_vector` — degree-1 resolution works only through preset membership; the direct vector-install transition does not exist | `clutch-kernel` | resolution.rs:56-66 — "the missing kernel transition is the one named residue" | M |
| 14 | Epoch freeze / page creation unrepresentable on the wire | layout intents + new instruction | orders_batch.rs:330-336; SOLANA_LAYOUT.md:887-888 | M |
| 15 | Reservation seam — "an order placed by this program is unfunded"; must land before freeze or settle | `orders_batch` + position/hoard plane | orders_batch.rs:326-329 | M |
| 16 | PlaceOrder/CancelOrder oracle + SVM leg — the reference models no order family; "a green result would be the program agreeing with itself" | `solana-reference` (an order-family transition) + harness | SBF_BRINGUP deferred check 16; LIFECYCLE_WALK §item 8; reference lib.rs:4821-4865 | M |
| 17 | Window identity + feed summary digest recorded, never verified — the program owns no hash primitive | accumulator + layout + `observe_resolve` | observe_resolve.rs:86-93; SBF_BRINGUP "the program owns no hash primitive" | M |
| 18 | Committed-transaction evidence — everything is `simulateTransaction` with `sigVerify: false`; "the actor signed" is a message-header fact | harness (a committing walk) | SBF_BRINGUP:1155-1157; LIFECYCLE_WALK "What this walk is not" | M |
| 19 | Runtime obligations untested: rent exemption, close/reopen resurrection, cross-transaction replay, post-write atomicity | `clutch-sbf` + harness | SBF_BRINGUP deferred checks 4-8 (:435-449) | M |
| 20 | Token optional-leg hole — "a caller may present the smaller plane and get the weaker instruction"; closes when row 4 deletes the `Absent` variants | `split.rs`/`token.rs` | TOKEN2022_PLAN.md:33-38 (measured, named transitional) | S |
| 21 | Policy freezes — residual settlement, fractional payout, transfer phase, fee arms; all implemented+tested, none promoted; **user decision, not code** | ember | G.2 below; DRIFT_REVIEW_2026-08-18 §Morning 1-4 | S (decision) |
| 22 | Distributional claims beyond degree 1 — degrees 2/3 refuse by design (no proven interval-ambiguity rule); the B-spline design is PROPOSED with §15 marking what landed | `resolution.rs` + design doc | DISTRIBUTIONAL_CLAIMS_DESIGN.md:3 + §15; resolution.rs:425-433 | L |

Also standing but out of the deployable path: `error.rs` frozen with
SettlePage's real refusal code unallocated (orders_batch.rs:435-437, S);
PlaceOrder/CancelOrder compute unmeasured (orders_batch.rs:405-410, S); the
degg operatorless-loop research gaps beyond posting
(OPERATORLESS_AGENTS.md:186-190, L).

## G — consolidated morning decisions (one list, both repos)

Time-ordered where a clock exists:

1. **Filing go/no-gos (degg).** (a) **Data Q4 insert** — the soonest forced
   call: its window *is* the Aug 18-22 joint-filing edit window
   (`CANDIDATE_DATA_Q4_INSERT.md`). (b) **Perpetuals filing** (RFC due Aug
   26): the go/no-go object *and* a landed Draft 1 PDF now both exist —
   ratify or retract the conversion (candidate header carries the status
   note); the filing was already corrected by its own manipulation-cost
   experiment. (c) **IAC operatorless addendum** (statement due Aug 27,
   11:59 p.m. ET hard; meeting Aug 20): the section sits in Draft 8 (8pp;
   page 8 is apparatus — cut to 7pp if wanted); its one filing gate (the
   recorded attestation-suite re-run) is met. Ratify the insertion and
   decide the cuts. (d) **John round-1 send** — the packet is restructured
   as ROUND 1 of 2; sending is your act. (e) **Conflicts NPRM** (Oct 5) —
   deliberately unhurried; decide in September. (f) Filing-day gates:
   the single evidence-section re-pin at a frozen commit **and** the ledger
   gate-4 PDF hash re-pin (rows still deliberately stale since the
   typography rebuild); docket re-checks before Aug 24 / Aug 26 / Aug 27.
2. **Policy freezes, evidence in (carried, dragons-clutch).** Residual
   settlement 1a / 1b-canonical / 1c (1b-free refused at clear time);
   fractional-payout a1/b1/c + the complete-set primitive (cross-boundary
   set-assembly question; lot-gating under (b)); transfer phase T-a vs T-b
   (needs the §14.2 epoch/resolution ordering rule); fee arms
   (terminal-ceil vs dropped-carry, κ, 60/15/25, executor cap — all
   unpromoted).
3. **The single-truth cutover (TOKEN2022_PLAN open decision 3, plus 4-7).**
   The measured out-of-band-burn DoS (`Custom(30)`, a holder can brick an
   outcome's seam instructions by burning outside the program) is the
   argument *for* cutting over from the two-truth shadow to token-account
   truth. Decide: checked mirror vs removal for `collateral_atoms`; ratify
   ImmutableOwner-required, outcome-mint decimals 0, ATA-or-not; and pick
   the pinned Token-2022 ELF (a program id is not a pin — open decision 7).
4. **Bedrock session** (carried): one paid MPC-TLS session to produce the
   first D-grade provider-attested transcript.
5. **Vendored-crate sign-off** (carried): accept `solana-define-syscall
   5.1.0` (verified verbatim, checksum-matched) or drop it; if kept, add
   the Apache-2.0 text beside the verbatim tree — still absent.
6. **Manifest + handoff structural repair (P2-1/P2-2):** reword the
   generator's `NOT_ATTESTED`, add gates or declared exclusions for
   svm-tests / the lifecycle gate / the token2022 probe / vector-check, pin
   the ELF digest, re-emit; give CODEX_HANDOFF §5 its clutch-sbf entry (the
   still-open half of the prior review's P2-2).
7. **place_order row-inventory:** the cost lab's landed-intent order rows
   carry hypothesized account sets (place_order **7**) while the landed
   instruction takes **4** accounts (orders_batch.rs:479-485) — decide to
   promote the rows to landed account sets and queue the CU measurement
   (S).
8. **VM-INT pair** (carried): accept or rename `golden/coupled.trace` vs
   §14.3's `relation_v1.trace`; R-b's rounding boundary still needs an
   exercising test before any R-b freeze.
9. **E0/Verus probe posture** (carried): re-author a reviewed probe (new
   digest) or keep the recorded failure; E1 remains NO-GO either way.
10. **Vector-spine gates G1-G7** (carried, sharpened): the spine is landed
    and self-guarded; the proposal's own words — "G1-G7 are human decisions
    and none of them is made by this drop"; G1/G2 re-scope to the twelve
    error enums is flagged for your ruling.
11. **degg small repairs from this review:** remove `Debug`/`PartialEq`
    from `SealedLocalOutput` (P3-6); commit a machine-readable A/B differ
    run log or re-reproduce at freeze (P3-7); qualify the two "deployed
    plonky3" mentions and link the VERIFIED legend from VERDICTS.md
    (P3-8).

## Fixes applied (all wording/status; no code semantics, no refusals touched; both crates re-tested green and `cargo doc` clean after edits)

1. `programs/solana-layout/src/stream.rs` — module contract item 1 and
   `write_header`'s doc: the header field sequence *is* a second
   transcription; named the byte-for-byte equivalence test as what pins it
   (was: "there is no second transcription").
2. `GOAL.md` — lifecycle entry: "section-10 items 1-10" → items 4-7, 9, 10
   driven plus the market half of 1; 2, 3, 8 explicit skips per the doc's
   skip list.
3. `GOAL.md` — "Markets are now FUNDABLE" → fundable **at founding** (the
   cap decision is structural; cash arrival still has no endowment
   instruction).
4. `GOAL.md` — "extension matrix proven on real bytes" → "exercised on real
   bytes".
5. `programs/clutch-sbf/program/src/accounts.rs` — stale "Nothing in this
   program calls them yet" (pre-port) → the actual call map (split.rs +
   market_init.rs compose C1-C3; observe_resolve.rs carries an inline C1+C2
   transcription — finding P3-4 stated in place).
6. `docs/implementation/SBF_BRINGUP.md` — the P1 repair set: status ¶1
   "seven of the eight … except Resolve" → all eight; the
   cancellation-stub sentence → PlaceOrder+CancelOrder no-SVM-leg /
   SettlePage sole stub; module-map rows (Resolve fits at 536,123 CU,
   check 15 closed; CancelOrder implemented); refusal-table row split with
   CancelOrder's true status; **"Correct description"** rewritten to the
   ten-family truth; the trailing restatement corrected; deferred check 9
   → dated half-closed (mint CPI live since `5c88505`, collateral escrow
   still unwired); dynamic-symbols paragraph → dated superseded note
   (`sol_invoke_signed_rust` present in the recorded `d8a9267c…` ELF —
   verified against the actual ELF file).
7. `docs/implementation/MULTI_POSITION_CLOSURE.md` — coordinator note:
   dated done-since annotation (the SBF port landed; remaining program-side
   gap is position-init, behind system-CPI account creation).
8. `docs/implementation/LIFECYCLE_WALK.md` — header now routes the reader
   through the skip list (items 2/3/8 not driven; 1 and 6 half-driven)
   before quoting; "Not a token movement" corrected (the walk presents no
   token plane; the ELF itself carries CPI code since `5c88505`).
9. `docs/implementation/COST_LAB.md` — the two OrderPage-v3 pin lines
   marked superseded by the `e780d5b` re-pin recorded beneath them.
10. `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md` — header note
    pointing at the §15 addendum for what `927d4bc` landed vs what stays
    PROPOSED.
11. `programs/clutch-sbf/program/src/instructions/mod.rs`, `dispatch.rs`,
    `program/src/lib.rs`, `programs/clutch-sbf/README.md` — the four crate
    docs re-trued against the dispatch map; both mid-sentence editing
    splices repaired; README's "fix is in flight" → the landed 536,123 CU
    figure.
12. `programs/clutch-sbf/program/src/instructions/observe_resolve.rs` —
    "authenticated cursor" → "advanced, replay-guarded cursor
    (digest-chained and signer-authorized — nothing here authenticates the
    observation *sources*)".
13. `degg-research/docs/regulatory/research-memos/CANDIDATE_247_PERPETUALS_COMMENT.md`
    — dated status note: the Typst conversion + Draft 1 PDF landed; the
    go/no-go remains the author's; nothing filed.
14. `degg-research/docs/regulatory/research-memos/IAC_ADDENDUM_CANDIDATE.md`
    — dated status note: the operatorless section sits in IAC Draft 8; the
    go/no-go remains the author's; nothing filed; the insertion cuts as
    easily as it was added.
