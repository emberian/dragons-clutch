# Standing goal (2026-08-18, ember on a walk)

Authorized autonomous work. Private repos pushed: `emberian/dragons-clutch`,
`emberian/degg-research`. Push after each coherent wave.

**Goal:** Dragon's Clutch fully implemented, all aspects, testnet-DEPLOYABLE
(build + program-test + local-validator evidence; no public-network deployment
— that stays human-gated). Dark Egg research agenda progress. Explore further
committee questions. Opus-mostly blend.

## Current thrust

Wave 5 full-width: SBF foundation (module-per-instruction split) -> then
per-instruction fan-out; vector-spine fixtures + checker; portfolio page
encoding; multi-position closure (Fable); Token-2022 probe; degg
relation-IR (Fable), inclusion/availability, refusal-order freeze,
posting-path spec; Draft 7 rebalance still out.

## Next 3 moves

1. CONSOL-TERMS (Fable, running): unified TermsAccount v2 - cap +
   obligation 18 + distributional basis; markets become fundable;
   threshold markets resolve. BATCH-STREAM (Fable, running): streaming
   relation verifier + ClearWork checkpoint + projection spec.
2. HARNESS-REGEN lands -> all-families SVM differential recorded; then
   LAYOUT-WRITE lane (streaming writer, tombstone, Intent rev,
   canonical order ids) on the projection spec.
3. Token-2022 CPI leg into the instruction modules; lifecycle-walk
   script; umbrella + fresh manifest + closing drift review.

## Done log

- BATCH-STREAM landed: streaming verifier at 1,280B frames (was 39KB),
  ClearWork checkpoint with fold-digest tamper refusal, P-BATCH-03
  tested across 210 resume points, 19,520-comparison equivalence at
  zero divergence; projection spec written for LAYOUT-WRITE. Pushed.

- Launched SHIELDED (degg P1-4): the composition test of relation-IR x
  inclusion-availability x frozen refusal order, with the honest core
  being exactly what stays executor-trusted.

- HARNESS-REGEN landed: 52/52 byte-exact SVM differentials across eight
  families through one real bank session; self-falsifying gate; new
  pinned ELF. MEASURED blocker: Resolve exceeds the 1.4M CU ceiling
  (five-fold terms decode) -> fed to CONSOL-TERMS with the numbers;
  redeem at 97%, create at 71%. Pushed.

- ORDERS landed: PlaceOrder byte-exact; SettlePage blocked with MEASURED
  frames (relation needs 39-45KB vs 4KB) -> streaming-relation API +
  page->book projection are the next design round; cancellation needs a
  tombstone slot kind; portfolio placement is a wire gap. Pushed.

- Merge implemented at the semantic owner + program mirror; round-trip
  byte identity; SBF_BRINGUP status now truthful (8 families host-diff,
  Split-only SVM pending regen); pushed.
- Collateral decoder: 13 goldens first-run, 22 refusal parity; honest
  cap answer - needs the unified TermsAccount revision (cap + oblig 18
  + distributional basis = ONE schema rev, queued for CONSOL/Fable).
- Launched: ORDERS (streaming pages, frame-budget analysis for on-chain
  relation verify), HARNESS-REGEN (all 8 families through the real bank).

- observe_resolve landed: full evidence gate on-chain, FeedAdvance
  formats PROPOSED, SBF-frame lesson canonized (host green is not
  evidence); instruction set now Split/Mat/Demat/CreateMarket/Feed/
  Resolve/Redeem + Merge in flight; orders_batch unblocked pending its
  lane. Pushed.

- Manipulation-cost table: 1,080 rows exact, four surprises incl.
  refuting our own window-length line - the FILING was corrected by its
  own experiment before any human read it (perpetuals now 5pp); pushed.
- Perpetuals Draft 1 + operatorless addendum in IAC Draft 8; John packet
  is ROUND 1 of 2; pushed.
- Split->CLO-DELTA port + Materialize/Dematerialize; FINDING: reference
  adapter never implemented Merge -> REF-MERGE lane launched.
- CreateMarket landed; collateral-cap blocker -> policy-decoder lane
  launched.

- Streaming page decoders: on-chain pages unblocked, frames MEASURED
  (1,856 max vs 8,640 buffered); pushed (unsigned - 1Password away).
- CreateMarket implemented (23 negative tests, byte-exact founding
  writes). COORDINATOR ITEMS: collateral-policy decoder needed before
  any market can accept collateral (cap honestly written 0); error-code
  consolidation pass owed.
- Bundling corpus: 683k decompositions, smallest witness [1,0]+[0,1],
  support-union invariance theorem narrows the filing claim usefully;
  census 300/65,536; pushed.

- Vector spine implemented: 25 vectors, first executor, ten findings
  (incl. clutch-sbf parallel error numbering -> consolidation pass;
  G1/G2 re-scope -> ember review queue). Pushed.

- Inclusion/availability model (degg P1-3): MMR log, receipts,
  equivocation verdicts, 125 tests; six build-time findings; pushed.
- Cost lab v3 re-pin + source-derived identifier guard; pushed.

- Token-2022 probe: deps RESOLVE, 6 scenarios green, extension matrix
  proven on real bytes; toolchain split finding (1.93 for program-test).
- SBF foundation: module split, 18 seeds, Split differential PASS;
  OrderPage v3 on-chain decode blocker found -> streaming decoder lane.
- Distributional claims design: PoU theorem + derive-last-and-subtract;
  deg>=2 interval ambiguity narrowed honestly; TermsAccount v3 unified
  with obligation 18.
- Relation-IR landed (degg P1-2): relation as data, frozen check order
  live in the digest, 2.1M-case zero-divergence Clear lowering.
- Wave 6 launched: 3 instruction lanes + streaming decoder; John
  two-round protocol; bundling corpus + manipulation-cost experiments.

- Draft 7 landed: bundling-invariance as a criteria-test, Ariadne/FalconX
  by-name engagements, machine-checked-negatives table, P-I8, addendum
  slack held; pushed.
- Portfolio page encoding v3 (one chain, one fold; 3883B pages); pushed.
- Multi-position closure IMPLEMENTED (CLO-DELTA-V1 inductive invariant;
  single-position refusal retired); adapter doc rewritten; pushed.
- Cost re-pin lane launched for v3; foundation lane briefed on the
  closure port + page v3.

- Refusal-order frozen (18 rules, 3 tiers); differential now ZERO
  divergences over 300.4M cases; custody-bound gap closed; C024 in the
  claim ledger with the conformance-vs-corroboration distinction. Pushed.

- Posting-path spec landed (policy/record shapes, admission relation,
  value-gap finding, E1-E3 ladder); live-session ceiling corrected at
  primary source: a real api.coinbase.com MPC-TLS session IS recorded
  (183d82817, 2026-07-11) - only the model-provider run is absent. The
  attested-exchange-price mechanism is a candidate authenticated feed
  for Clutch observation (synergy with 24/7 positions). Pushed.

- Wave 1-2 (pre-goal): proof tools pinned; coupled BatchRelationV1 + pairing
  (P1-B dead); kernel transfer_internal + complete-set redemption +
  transactionality; vertical model settles through the relation (1a/1b/1c);
  econ lab 83 tests + fixtures; Draft 3→5 filings + audits + legal packet;
  umbrella gate green (108 Rust tests).
- Repos created and pushed (this entry).
- Draft 6 filings: 20 argued positions, audience ontology, 2/3 length; pushed.
- Night drift review A-H pass; fixes committed both repos.
- P0-5 Python defaults removed (behavior byte-identical); VM coupled-path doc.
- Committee memos: 42 Qs triaged, 8 position memos; ember decisions queued
  (no-position reversal for Q12-15 material; 8 sources need verification
  before any filing use).
- B lane: typed WindowResult (substitution = compile error), derive_payout
  spec, digest unification decided + Python side; pushed.
- C lane: P1-C closed - 8 new accounts, cross-page closure, frozen grid,
  per-account versions; 37 layout tests; pushed.
- REF-INT launched: evidence-gated resolution into solana-reference.
- GLASS: equality gates, CSP honesty, kernel-true terms (new digest); pushed.
- S1: reproducible ELF, 6/6 byte-exact SVM differential vs offline adapter,
  72,869 CU Split; commit held until REF-INT lands (shared dep mid-edit).
- MANIFEST: baseline-manifest emit/check tool landed, live-fire validated;
  clean emit queued for wave end; pushed.
- COST: P1-F closed, landed-ABI arm + abi-audit drift refusal, 261 rows;
  seam found: portfolio orders lack persisted page encoding; pushed.
- REF-INT + S1 co-committed: resolution evidence-gated (fail-closed path
  intact), reproducible ELF + 6/6 SVM differential; pushed.
- LANDSCAPE: 11 filed comments surveyed; IAC docket empty of technical
  statements; P-D5 crowded, P-R6 preempted; our formal-methods ground is
  a corpus-wide zero. Draft 7 rebalance queued behind TYPESET.
- TYPESET: STIX Two Text, ten diseases fixed, content byte-preserved; pushed.
- Clean-tree baseline manifest: 28/28 gates, 37 digests; pushed.
- Attestation survey (corrected pass): parsing lane S+D (Lean-emitted
  byte-pinned Dyck/DFA AIRs joined to deployed prover, tamper canaries);
  STARK<->TLSNotary join EXISTS (shared commitment, splice attack closed);
  four named gaps for operatorless loop: R3 whole-history, onchain posting,
  pinned-notary-is-an-operator, public tool-loop spec. Provenance red
  flags stand (forked FRI w/ unmerged fix, restricted-license vendor).
  EMBER DECISION QUEUED: one paid Bedrock session would produce the first
  D-grade provider-attested transcript.
- OPEN-MATTERS MAP landed: IAC agenda published (Session II = agentic
  finance - our addendum answers a printed heading); FalconX CEO +
  Chainlink founder are IAC members; 24/7-perpetuals RFC due Aug 26 asks
  for our manipulation-cost material verbatim (EMBER: go/no-go within
  ~24h); on-point event-contract reporting NPRM closed Jul 31, missed -
  standing watch established; pushed.
- B-CONVERSION landed: 86 attestation tests reproduced green on pinned
  breadstuffs tree 436c2a8 (persvati, 213s); Lean-emit caveat recorded;
  pushed.
- Closing drift review: A-F pass, fixes committed both repos; manifest
  widened to 33/33 gates incl. SBF lane; clean emit pushed.
- OPERATORLESS memo + IAC addendum candidate landed (EMBER go/no-go). Pushed.
- 24/7 candidate drafted, quotes GPO-verified (Q40 digital-asset bracket
  found and confronted); pushed. Decision object ready.
- In flight: Draft 7 rebalance, 24/7-RFC candidate draft.

## Ember decision queue (morning)

0a. Conflicts NPRM comment candidate (Oct 5; zero-artifact, one seam) - go/no-go.
0b. Data Q4 insert candidate (rides the Aug 24 filing; consistency proven) - go/no-go.

1. DONE: IAC Draft 8 carries the operatorless section (8pp; page 8 is
   apparatus only - sanction content cuts if you want 7pp).
2. DONE: perpetuals filing Draft 1 (4pp). John packet is now ROUND 1 of 2.
3. One paid Bedrock MPC-TLS session (first provider-attested transcript).
4. Vendored solana-define-syscall provenance sign-off.
5. Policy freezes (residual 1a/1b/1c, lots, AON, fee carry - evidence in).
6. Draft 7 read + John hand-off + signature-block form.
7. Full list: docs/implementation/DRIFT_REVIEW_2026-08-19.md final section.
- D1: independent FBA oracle, 300M-case differential, zero semantic
  divergences, vectors byte-identical; spec gap (refusal-class priority)
  found and pinned; pushed.
