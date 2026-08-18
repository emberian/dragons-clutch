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

1. Foundation lands -> fan out instruction lanes (merge/materialize,
   market-init, observe/resolve, orders/batch) on the module ownership map.
2. Token-2022 probe verdict -> CPI leg into its module; multi-position
   scheme -> adapter + program.
3. Lifecycle-walk script (PROJECT.md section 10 items 1-10) once the
   instruction set closes; then umbrella + manifest + drift review.

## Done log

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

1. IAC addendum go/no-go (answers published Session II heading; text ready).
2. 24/7-perpetuals RFC go/no-go - deadline Aug 26, decision needed ~24h.
3. One paid Bedrock MPC-TLS session (first provider-attested transcript).
4. Vendored solana-define-syscall provenance sign-off.
5. Policy freezes (residual 1a/1b/1c, lots, AON, fee carry - evidence in).
6. Draft 7 read + John hand-off + signature-block form.
7. Full list: docs/implementation/DRIFT_REVIEW_2026-08-19.md final section.
- D1: independent FBA oracle, 300M-case differential, zero semantic
  divergences, vectors byte-identical; spec gap (refusal-class priority)
  found and pinned; pushed.
