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

1. Running: LAYOUT-WRITE (page v4: writer/tombstone/derived ids/intent
   rev), TOKEN-CPI (real mint/burn/transfer + program-test evidence),
   LIFECYCLE (the PROJECT.md section-10 walk as one recorded SVM gate),
   degg VERDICTS + C-1 refund closure.
2. Integration: orders module onto page v4; ClearWork/candidate accounts
   onto the streaming verifier; cost re-pin (terms 1656 + page v4).
3. Wave close: umbrella both repos, fresh clean-tree manifest with the
   new ELF, closing drift review, push everything.

## Done log

- THEORY WAVE launched (the B-spline consequences, with ember):
  DUAL_IS_THE_MEASURE (Fable - is the LP dual literally the implied
  measure? then certificate and density are one object),
  RISK_SUMMED_POSITIONS (Fable - sup-norm margining over a joint outcome
  space, model-free; the fee-as-quotient-norm derivation),
  OPTIMALITY_CERTIFICATE_MAPPING (Opus - our relation as a Cert-F LP,
  three quantified gaps, the claim-language delta),
  CERTIFICATE_STACK_INVENTORY (Opus - breadstuffs' Lean-proven cert
  stack: separability, licensing, provenance gate).
  Finding that triggered it: breadstuffs fhegg/fhir/CertF already
  implements dual-certificate verify-not-find with zero-sorry Lean and a
  real STARK - and has NO consumer. We are the consumer it never had.

- Wave 9 landed and pushed: genesis plane, token completion (real mints,
  collateral wired, atomic-revert shown), kernel resolve_with_vector,
  degg settlement relation (P1-7). Pace slowed per ember.

- Wave 8 CLOSED: drift review committed, both structural P2s fixed
  (manifest not-attested text truthful, handoff knows the program
  exists), fresh strict manifest 33/33 (1a537bc). Everything pushed.
- Wave 9 open on the gap ledger: GENESIS lane (init instructions +
  endowment + system-CPI creation + ClearWork/candidate codecs) and
  TOKEN-COMPLETE lane (CreateMarket makes real mints, collateral leg
  wired, mandatory token plane, E5 rollback demo).

- TOKEN-CPI landed: real Token-2022 mint/burn on a real bank, exact
  deltas, ~95K CU/leg, extension refusals live, seed bug caught; the
  out-of-band-burn DoS measured as the cutover argument. Pushed.
- Clean-tree gates: bring-up + lifecycle PASS at 5c88505, ELF recorded.
- Grand umbrella: 400 Rust + 152 Python tests green, both traces
  identical, goldens OK; strict manifest 33/33 pushed (c3517a3).
- Closing drift review (waves 5-8, both repos) running - produces the
  consolidated remaining-gap ledger and the single morning-decision list.

- THE LIFECYCLE WALK PASSES (one SVM gate; section-10 items 4-7, 9, 10
  driven plus the market half of 1; items 2, 3, 8 carried as explicit
  skips — see LIFECYCLE_WALK.md's skip list; terminal identity closed,
  self-falsifying). Sharpest named gap: no endowment instruction.
  Pushed.
- abi-audit resurrected + hardened (34 owed drift lines delivered);
  re-pinned to v4; goldens stable. Pushed.
- INTEGRATE landed earlier this wave; TOKEN-CPI is the last lane out.

- INTEGRATE landed: orders on v4, CancelOrder + portfolio placement
  live, write path -115 lines net; 113 tests. Pushed.

- Page v4 landed (e780d5b): derived-rank ids kill the griefing vector,
  tombstones, per-order expiry, streaming writer, intent v2 closes the
  portfolio wire gap. Finding: abi-audit DEAD since 927d4bc -> repair
  lane. INTEGRATE (orders onto v4) + COST-REPAIR launched; LIFECYCLE and
  TOKEN-CPI briefed on the v4 fallout in their files.

- VERDICTS reconciled (ladder with per-rung status; V9/V10 added);
  C-1 refund path closed with conservation demonstrated; pushed.

- SHIELDED (degg P1-4) landed: composition of all three packets by path
  dependency; executor freedom MEASURED (377/1,125 admissible alt
  publications - the proof rung justified by experiment); 51 tests,
  90,082-book differential. degg P1 packets 1-4 now ALL landed tonight.
  VERDICTS reconciliation + C-1 refund fix launched. Pushed.

- CONSOL-TERMS landed (927d4bc): TermsAccount v3 unifies cap + oblig 18
  + distributional basis; threshold markets resolve end-to-end; deg-1
  derivation via preset membership (kernel residue named); error
  registry consolidated, lossy-projection pin green; decode-once facts
  API takes Resolve from CEILING-ABORT to 536K CU (38%). Bring-up PASS,
  0 undrivable. Markets are now FUNDABLE at founding (the cap decision
  is structural; cash arrival still has no endowment instruction).
  Pushed.

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
  exercised on real bytes; toolchain split finding (1.93 for program-test).
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
