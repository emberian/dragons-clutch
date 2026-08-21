# Standing goal (2026-08-19): swarmcycle until project is complete

> Status/claim authority stays with [`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) and
> [`docs/SWARM_ROADMAP_2026-08-19.md`](docs/SWARM_ROADMAP_2026-08-19.md). This
> file is the standing-goal execution trail (Fable session, ember authorized).

**Goal (UPGRADED by ember, 2026-08-19 ~18:00):** close out the remaining
engineering (V3 atomic promotion, R2 pull promotion, R4 runtime authorities)
AND deploy to public testnets, pushing the programs through their paces.
Ember's explicit authorization covers devnet/testnet deployment with fresh
throwaway keys and bounded public-RPC use. Still human-gated: mainnet, real
value, market creation for real users, the production source-registry flip,
filings, and Gate L0. Codex is co-resident and quiescing;
its dirty paths belong to it until the tree goes quiet.

## Current thrust

R1 manifest close is SEALED: schema-v2 MANIFEST.baseline.json committed at
94/94 (d78f299, stabilized 3294dcd, checked 9625100, bound 6743b9d — codex's
endgame), fast check binds the tree exactly. The fresh Persvati portable
attestation of 6743b9d PASSED 40/40 (recorded in CURRENT_TRUTH §2). Post-R1
design track is moving: the R2 successor and Verus dust model are
research-only, R4 runtime design is proposed, and Draft 10 + John packet are
ready in degg-research.

## Overnight mandate (ember, 2026-08-19 ~23:00): "authorized and deputized to
## work all night iterating on the system implementing everything we had
## planned." Quantum 10k + batched-fold routes BOTH BLESSED; other planned
## directions blessed. Human gates unchanged (mainnet, real value, real-user
## markets, registry flip, filings, L0).

## Standing directive (ember, 2026-08-21 ~07:30): NO MORE HOLES. Do it all,
## systematically, in parallel, Opus swarms. Local sim tests ARE complete
## testing. Stop deferring to external calendars (the Pyth cutover matters
## only for pinning a live mainnet identity — one const, later). Stop
## per-wave reseals: ONE batched cycle-G reseal when the swarm lands.

## The swarm (all Opus, SHARED MAIN TREE — no worktrees per ember. Rules:
## commit only named files, small and often; retry on index lock; never
## stash/add -A/branch-switch; serialize full-suite runs behind the
## mkdir-spinlock /tmp/claude-501/suite.lock; hbox (swarm-build, co-tenant
## with codex) and persvati (nice, shared) absorb heavy overflow runs.
## Concurrency set 1: A, C, E, F (disjoint territories). Set 2 on slots:
## D, G, I, J. Chained after A: B, H.)

A. PARTIAL FILLS (reservation v2 + seam generalization + campaigns) — then
   chains: B. VIRTUALPOT + rounding-pot realization + Position close, and
   H. AON/min-fill sibling profile machinery.
C. FEE RELATION ARITHMETIC (composite base in the relation, rates
   parameterized, nonzero TEST consts; frozen consts stay zero).
D. R2/PYTH FULL LOCAL INTEGRATION (decoders merged, ingestion wired,
   fabricated Pyth-shaped accounts, Endow-with-real-source end to end,
   registry machinery tested — the live-identity pin is one const later).
E. KEEPER BINARY (the cranker, real validator, fold-batch composition —
   discharges the packet caveat).
F. FRONTEND (operator bench M0-M1 per the design: harness lib split,
   operatord, watch mode, trade mode).
G. DEGREE 2-3 SMOOTH CLAIMS (admission + payout, differential vs Lean).
I. CAMPAIGNS AT SCALE (64-tick, max books, exact-tie scale, second
   profile, multi-market epochs — the W2/V2 evidence).
J. SMALL-FIX BATCH (stale blocker row, mock path-sensitivity, queued
   rustdoc + doc gates, the macOS relocation-probe protocol amendment).

## Ember decision queue (2026-08-19 morning)

1. R2 successor closes the model choice to closing `CROSSING_V1` id 2:
   368-byte SourceSpec-v2, exact ProgramData/config pins, zero grid origin,
   decoded-body duplicate collapse, start-aware contiguity, and overflow
   refusals. It remains research-only; post-cutover Pyth identity freeze and
   every loader/Instructions/Clock/registry/SBF adapter gate remain open.
2. R2 production identity freeze deliberately waits for the 2026-08-26
   cutover (docs/design/SOURCE_PROVIDER_V1_SELECTION.md); the model does not
   authorize an interim registry entry or value admission.
3. R4 design ratification (docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md):
   notably the frozen-incinerator-sink choice, fractional Arm A
   live-until-aggregated, legacy rows declared permanent, and the Section 8
   reference-ownership variant (maturity horizon vs refcount).
4. V3 findings B/C (verify_lease tautology, FROZEN_EMPTY pinning) were
   closed unilaterally by the codex lane at 6267fde/081bd81 — your sign-off
   on those two closures is still owed; review them on codex/r3-direct-v3.
5. Filing ops (human-only): send John packet ROUND 1 (degg-research 55ce13a);
   signature block + dual-route answers needed before Aug 24; deadlines
   Aug 24/24/27, perpetuals RFC Aug 26.

## Done log (2026-08-19 session)

- W1 PROMOTION EXECUTED (396e11d, evidence-only, identity unmoved at
  4fded7a6): 25/25 walk-plane routes quoted at the CURRENT seal — the
  lane re-derived rather than transcribed and caught that 5 of the
  promotion report's limits were stale against the superseded root.
  Worst route sits at 64% of the admission boundary; compute is not this
  plane's problem. Batch-variable advance routes quoted as
  BATCH_SHAPE_VARIABLE_OBSERVED_MAXIMUM_ONLY (bounding no unmeasured
  composition, said out loud); tags 60-67 honorably unquoted; live flags
  welded UNTOUCHED with refusal teeth; rent rows keep their STOPs; W2
  blocked on the named ids. 75/75 profile tests; both policy gates green
  post-commit. The fee-plumbing lane is the last out; its merge opens
  cycle F.

- R2 RUNBOOK + HOUSEKEEPING MERGED (doc-only, seal untouched): the
  Phase-0 runbook (490 lines; r2-caps-rebase-trial re-measured CLEAN
  against current main, zero closure commits in the 60 behind), the
  svm-tests README staleness repaired. TWO ITEMS CORRECTLY REFUSED AS
  TRAPS: svm_run.txt regeneration (a worktree build can never produce the
  canonical identity — a fork-identity capture is worse than a stale one)
  and the rustdoc fixes (the "two warnings" are actually 13 across NINE
  in-closure files; precedent 9c371fe forked the ELF for one such fix —
  rides the cycle-F reseal). NEW GAPS QUEUED: (1) the default SVM gate's
  manifest key pattern matches nothing the runner emits — the default
  ELF identity is in NO manifest key line (fix the declaration at cycle
  F's emission); (2) the Pyth receiver id + Config PDA are not
  transcribed in-tree (E2-trigger prep item); (3) rustdoc -D warnings
  parity for clutch_sbf after the cycle-F doc fixes.

- THE SPW CONCEPTUAL CANON AUTHORED AND MERGED (.spw/canon/, six layer
  files + index): 40/40 inventory concepts, zero stubs, typed relations
  mapped onto the workbench's sigil vocabulary (uses ~, refines ^,
  refuses =, measures %), unmappable verbs kept verbatim in binds rather
  than coerced, four hub roots declared, 164/164 transcribed strings
  byte-faithful to the inventory (the check caught and fixed nine en-dash
  slips), four seed discrepancies RECORDED not repaired, all workbench
  gates green (strict lint 10/10, doctor ok, 86/86 path refs, 95/95
  edges). Flagged: the .gitignore trailing slash means a symlinked
  workbench is not ignored (worktree trap); spw mount's fail is
  workbench-shaped-root checking, not a consumer failure.

- CYCLE E ATTESTED: Persvati 45/45 PASS, 0 STOP over exact cb94c27 — the
  4fded7a6 ELF in seven contexts, seven historical roots asserted exact,
  and a NEW CONTRACT-DRIVEN GATE the lane invented because the seal's
  central retraction claim had no checker: build_path_mechanism_crossref
  verifies the relocation-mechanism ATTRIBUTION itself (88 symbol sites:
  pass1 == pass2 == relocated-home, != crosspath at all 88) — cycle-D's
  inverted reading would now FAIL the assert. Bonus: the sealed account
  probe reproduces byte-identical cross-architecture. Truth and memory
  advanced. EVERY CHAIN IS NOW CLOSED. Holding on: ember's filing reads +
  submissions (Fri pair / Sun-Mon perps / Tue statement), the api.data.gov
  key, the IAC transcript, the faucet.

- THE WEEKEND WITNESS WAVE LANDED, BOTH BUILDS: (1) the OFF-HOURS SWEEP
  (8,640 rows, v1-anchored two independent ways, hand-verified row,
  monotonicity clean): windows inside a thin session lose manipulation
  cost linearly with depth, max clawback sixfold, nothing survives
  tenth-depth — published beside v1 with BOTH honesty rails
  (min-across-sessions; inventory-carry excluded and named), and the
  perps filing now FILES the computation it demands. (2) the
  DISAGREEMENT EXHIBIT: two named estimators (Gaussian fit vs empirical
  resample, pre-registered rules, seed 20260827) state densities over the
  Friday-clutch basis; the general plane clears and settles their
  disagreement in the bank ON THE SEALED CYCLE-E ELF (T0: degree-1 terms
  ADMIT through the general plane — new evidence), conservation exact,
  126-check script green, and the headline identity holds: both models
  think they won and the sum of self-assessed edges IS the disagreement
  traded (196.115 + 289.11 = 485.225 atoms). The design's one universal
  claim was REFUTED by the build's independent derivation (76/77 in-band
  candidates clear; exception named on the page) — the honesty apparatus
  eating its own cooking. site/disagreement.html live; Position 3's
  witness clause landed in the FILING statement. CURRENT_TRUTH advanced
  to cycle E (caught stale again by an exhibit lane); cycle-E attestation
  dispatched (was interrupted by the filings pivot).

- TERMINALCLOSURE MERGED (966ee2c; 27/27 targets green; canonical identity
  4fded7a6/1,979,512): intents 60-67 — owner-signed terminal release plus
  permissionless closes down an enforced dependency DAG, exact recorded
  principal to exact recorded payers via the new GeneralFundingLedgerV1,
  surplus only to the frozen incinerator. HEADLINE: a cleared epoch
  reclaims 531,639,600 of 531,652,377 lamports (residual = exactly the one
  declared-permanent policy artifact; burns = exactly the injected
  donations); a lapsed epoch reclaims 47,167,920. One argued DAG deviation
  (pages before pot — page-absence is the only executable
  no-pending-proof), recorded residuals honest (unregistered accounts
  stand; abandoned ACTIVE reservations hold their page at recorded rent).
  Blocker tail is now exactly PartialFillLedger + VirtualPot. CYCLE-E
  RESEAL DISPATCHED (Opus): new root 4fded7a67a2d8994, TerminalClosure
  evidence sealed, walk-plane rows reclassified REFUNDABLE_TRANSIENT where
  close routes are sealed, the OrderPage blocking id added, the cross-path
  digest-list convention adopted, and --check-current green again as a
  required outcome.

- V3 MEASUREMENT CAMPAIGN SEALED (Opus lane; merged at the e8ba31d5
  identity): the full venue CU table across 3 agreeing bank runs (worst
  390,272 vs the pre-syscall 1,127,892 STOP; per-run variance explained
  exactly — 1,500-CU PDA bump probes, all diffs exact multiples), every
  close route driven with ASSERTED zero-sum conservation (byte-identical
  across runs), DIRECT.V3_CLOSE_EVIDENCE_UNSEALED honestly RETIRED with
  two-directional teeth (14 -> 13 blocking ids; four rows now
  REFUNDABLE_TRANSIENT; live_v3 still false, no admission rows). THREE
  ESCALATIONS FOR CYCLE E: (1) V3 per-epoch structural strand is 5x the
  reported figure — OrderPage (28,814,401 lamports) has NO close route;
  honest total 35,941,440 lamports/epoch, projection corrected, a
  DIRECT.ORDER_PAGE_RENT_PERSISTS id owed at the next profile wave;
  (2) cycle-D's cross-path byte-identity was a one-sample coincidence (two
  more paths, two more digests, same 486 .text bytes) — cycle-E audit
  absorbs; (3) --check-current red on main since the walk merge's
  harness-lock lines (ELF-neutral, source_blob drift) — cycle-E reseal
  closes it. Profile suite 60/60.

- DECISIONS ADOPTED under ember's delegation + weakest-choice principle
  (docs/decisions/ADOPTED_2026-08-20.md; ADR-0005 adopted at docs/adr/,
  ADR-0003 marked superseded): ten adoptions, one principle-inverted
  deferral (upgrade posture — immutable-at-first-deployment loses to
  upgradeable-then-burn under the principle; mainnet-gated anyway), section
  8 explicitly deferred, ember-reserved list unchanged (submissions, E2/E3
  acts, treasury key, counsel, mainnet, L0). UNLOCKED WAVE LAUNCHED:
  TerminalClosure (Fable — close paths + dependency DAG + hostile walk
  extension + the headline reclaimed-lamports number), the V3 syscall-era
  measurement campaign (Opus, evidence-only at e8ba31d5 — MERGE BEFORE
  TerminalClosure forks the identity), and decision-doc propagation (Opus).
  Also: Pages ENABLED via gh — the site is LIVE and public at
  https://emberian.github.io/dragons-clutch/ (deploy green, 200 OK).

- GENERAL-PLANE SIGNED VALIDATOR WALK MERGED — the devnet-free evidence
  ceiling reached: 44 signed, CONFIRMED transactions on a real loopback
  validator driving the ENTIRE Tier-2 lifecycle (policy artifact staged/
  sealed on-chain, epoch+window, six placements from four distinct signing
  owners, cancel tombstone, real-clock deadline freeze, staged ClearWork +
  feed wire, the walk with the reservation sweep at 333,195 CU under a
  real 256-KiB heap frame, selection, entitlement, and SettlePage
  consuming BOTH the single slice and the portfolio full pair at 221,927
  CU); 38 watched accounts byte-reloaded per step; conservation re-derived
  from OBSERVED bytes (cash 32+17==49, eggs [17,17], custody 49 — exact);
  three expected refusals; falsifier RED as required; legacy 22-tx walk +
  both SVM suites (102/109) still green; canonical identity CONFIRMED
  e8ba31d5 on merged main. Honest boundary list recorded (genesis-assisted
  prerequisites, mock-source funding, slot-stamped fields, standing
  refusals). NEW FOLLOW-UPS: the mock ELF is build-path-sensitive at this
  HEAD (eBPF code clusters, pre-existing — register-worthy); gate-izing
  the 950-second walk into the manifest is a cost decision (would slow
  every emission ~35 min) — left as a next-wave M-item.

- MICROSITE V1 MERGED (site/, 7 pages ~7,600 words, 7 authored SVGs
  dataviz-validated both themes, 122/122 links, zero external resources,
  96 KB total, JS-optional, one scope note): the Friday clutch threads all
  seven pages with its arithmetic backed by a CHECKED script
  (docs/site-plan/friday_clutch_check.py — all exact); braided
  degen/builder/scholar callouts that re-say and never smuggle; every
  number cite-verified with five deviations recorded as plan errata.
  Pages workflow added (.github/workflows/pages.yml — the repo's first;
  enabling Pages + visibility are ember's console decisions). BONUS CATCH:
  the site's verification pass found CURRENT_TRUTH section 2 three seals
  stale (still 41/41-era) — now advanced to the cycle-D chain.
  NEXT-WAVE ROADMAP committed (docs/design/NEXT_WAVE_ROADMAP_2026-08-20.md):
  M (TerminalClosure first) -> S (partial fills, VirtualPot, fee plumbing
  to the boundary) -> O (measured only) -> A (ADR-0005 first, formal
  verification last per directive).

- ALL TEN DECISION REPORTS COMPLETE + THE PACKET SYNTHESIZED
  (docs/decisions/: register + 9 report files + DECISION_PACKET). The
  packet orders everything: six decide-now items (policy freeze as-pinned,
  ADR-0005 adopt-ready, E1 ratify, Realm allowlist freeze, deploy at
  sealed opt-3 with immutable-at-first-deployment posture, Plane-L charges
  permanent zero), four wave-unlocking items (R4 with the legacy-permanent
  scope amendment BOTH reports independently caught, RevenuePolicy
  remainder, composite fee base kappa*G + kappa'*R proven in the lab,
  promotion at rung W1/V3-campaign), and the Aug-24/26 calendar spine with
  E3 last-and-never-pre-authorized. CONVERGENCE: three lanes independently
  identified TerminalClosure as the highest-leverage unlocked wave. Also:
  opt-z is RED again on the current tree (stale premise caught by
  measurement); the deploy script's four-seals-stale artifact pointer was
  caught and repointed to e8ba31d5; the R2 caps branch rebased CLEAN
  across 153 commits (r2-caps-rebase-trial seeds Phase 0).

- CYCLE D ATTESTED: Persvati 44/44 PASS, 0 STOP over exact 788581c —
  identical gate roster to cycle C (contract found zero manifest changes),
  ELF six contexts, all SIX historical roots verified as the exact set,
  1,466 comparisons x2 zero mismatches. Job:
  persvati:/home/ember/jobs/dragons-clutch-portable-attest-788581c-20260820-FE7g0W.
  CURRENT_TRUTH capability matrix now records the complete general clearing
  plane; memory moved to the Tier-2-complete baseline. THE OVERNIGHT
  MANDATE'S IMPLEMENTABLE SURFACE IS DONE: every planned engineering unit
  that did not require ember's decision or devnet SOL is merged, sealed,
  manifested at 100/100, and independently attested. Standing waits:
  ember's decision queue (below) and the faucet.

- CYCLE D SEALED (liveness 3bcdeec, manifest 788581c at 100/100 FIRST TRY,
  post-commit check OK, pushed; attestation running): root e8ba31d582be3939,
  31 files, 15 bank suites / 77 tests, T2-7 selection + T2-8 entitlement
  evidence sealed unpromoted. The audit's nine-symbol import pin REFUSED
  the artifact — T2-8's portfolio copies lower to sol_memmove_ — reviewed,
  provenance matched to the admitted memcpy/memset shims, admitted as its
  own commit 2dbc9fc with the refusing console retained. Twist worth
  savoring: the FINAL artifact carries no surviving hash-sorted tie, so
  stripped cross-path builds are byte-identical again AND Cargo-home
  relocation is back to INDEPENDENT — the amended protocol keeps it honest
  either way. Terminal 47 rows (epoch.final_pot 262 B, epoch.receipt 217 B,
  no close paths, TerminalClosure standing). 57/57 profile tests.

- T2-8 MERGED — TIER 2 COMPLETE END TO END (26/26 targets green on merged
  main): the eighth join closed. A portfolio pair entitles atomically and
  SETTLES with exact whole-plane conservation asserted on-bank (cash sums
  exact to the atom, per-outcome position totals exact, the release
  identity buy-envelopes == consideration + refunds + pot, final Positions
  byte-equal to the verified summary's implied allocation). Receipts are
  the consumption latch (each consumed exactly once); consumed reservations
  archive with initial state intact; the 0x0017 SettlePage refusal retires
  for entitled receipts and stands honestly elsewhere. Blocker ledgers
  split truthfully: 5 SETTLEMENT_BLOCKERS + 6 portfolio blockers retired;
  PartialFillLedger, VirtualPot, TerminalClosure, FeeCarryAccount stand.
  ROOT-CAUSED along the way (docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md):
  the ELF is same-path-reproducible only — cargo -C metadata embeds the
  workspace path for path deps, so symbol hashes differ per checkout and
  hash-sorted layout ties flip (a 5-byte two-function swap, introduced
  T2-7; prior seals' path-independence was luck, not property). Seal
  protocol amended: canonical identity builds in place at the canonical
  path (e8ba31d5/1,914,432); cross-path builds recorded as
  PATH_TIED_SYMBOL_ORDER probes. CYCLE-D RESEAL DISPATCHED (root
  e8ba31d582be3939, T2-7/T2-8 evidence families sealed unpromoted).

- T2-7 MERGED AND PUSHED (8fe5f9e; identity 75ba075a/1,843,272 verified on
  merged main, 25/25 targets green): candidate submission is a staged
  four-tag wire (54-57 — a 6,266-byte feed cannot ride one transaction;
  stage-built feeds proven byte-identical to direct-writer feeds), EpochWindow
  v2 carries the selection deadline + retained registry (bound 3, displacement
  by verified components only — an unverified claim can NEVER displace a
  verified candidate), FinalizeSelection re-derives every tie digest and
  refuses mismatch, zero-verified lapses honestly. Selection CU tiny
  (~49k worst). PROPOSED pin flagged for ember: CANDIDATE_WINDOW_SLOTS=1000
  freeze-stamped (operator-chosen variant needs a tag-49 wire revision).
  T2-8 LAUNCHED — the final join: entitlements, FinalPot, generalized
  SettlePage consumption with whole-plane conservation.

- CYCLE C ATTESTED: Persvati 44/44 PASS, 0 STOP over exact 7e6066f (+1
  contract-driven gate: audit_internal_consistency, born from the
  relocation-finding evidence shape; the relocated-equality assert honestly
  FLIPPED to the recorded PATH_SENSITIVE disposition — asserting equality
  would now be a false claim). ELF d6929549 five contexts; all FIVE
  historical roots byte-verified with the set asserted exactly; 1,394 file
  comparisons x2 zero mismatches. Job:
  persvati:/home/ember/jobs/dragons-clutch-portable-attest-7e6066f-20260820-JY7Zte.
  Memory updated to the walk-era baseline. T2-7 SELECTION LANE LAUNCHED
  (tags 54-55, total-order over verified components only, displaced-close
  and tie-digest gates).

- CYCLE C SEALED (liveness be24ded, manifest 7e6066f at 100/100,
  post-commit check OK, pushed; attestation lane running): root
  d692954949d57db22, 28 files, 12 measurement families — the walk's own CU
  evidence sealed as UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY with derive()
  refusing any walk family that loses the declaration, decision_owner:
  ember. Terminal 45 rows (new: epoch.window 84 B, closed by no handler —
  TerminalClosure stays the ranked blocker). Existing-family drift <=0.8%
  except blank-bank creation (+2-10%, noted honestly, no quote derives).
  Two batch quotes moved one quantum. NEW RELOCATION FINDING, mechanism
  pinned exactly: relocated-Cargo-home is PATH_SENSITIVE again via THREE
  registry-crate panic-Location strings (solana-address/account-info/
  program-entrypoint) — relative at ~/.cargo, absolute when relocated; the
  canonical artifact embeds zero registry paths; supersedes the two prior
  seals' unqualified byte-identity, alongside the workspace path-length
  bound. Verus stream excluded-source digest re-recorded once more (T2-6
  driver cursors; scope unchanged).

- T2-6 MERGED — PORTFOLIO ORDERS CLEAR ON-CHAIN (87fd342): the general
  epoch lifecycle + streaming walk, tags 49-53. THE HEADLINE: a 40-order,
  3-page, 4-outcome book with 2 portfolio buys and 3 tombstones placed
  through the general arm, frozen, and walked to VERIFIED across ~20
  transactions with the on-chain verdict AND persisted FullScoreV1 + tie
  digest byte-equal to the host relation's — the crown-jewel basis is no
  longer inert as a shape (SBF-EXECUTED in bank, UNPROMOTED, fees zero).
  Full tamper battery green incl. the anchor comparison catching wholesale
  checkpoint substitution (the codec's 29 residual regions closed).
  custom-heap upward bump allocator (default-on) makes the requestable
  256-KiB region reachable; walk fixed floor ~250k CU/advance
  (decode+encode), everything under the ceiling, Pod fallback unneeded.
  Both suites 93/0 + 100/0 on the branch; 24/24 targets green both
  profiles on merged main. Deviations documented: 64-byte policy artifact
  (not the 96-byte wrapper), freeze grid re-verification dropped with the
  4-page-breach arithmetic stated. Cost lab re-pinned (interner +2,050,
  CandidateRecord v3 +32; abi-audit refused the drift as designed).
  NEW BOUNDED FINDING: a ~110-char workspace path produces a 492-byte
  codegen divergence; ordinary paths byte-identical — canonical identity
  d6929549/1,785,904 verified at three paths. CYCLE-C RESEAL DISPATCHED
  (root d692954949d57db22, walk CU families sealed as evidence-only).

- REVENUE POLICY V1 DESIGNED (docs/design/REVENUE_POLICY_V1.md, PROPOSED,
  485 lines): the missing fee-destination object, two denominations
  sequenced lamports-first, per-Realm policy record + revenue vault PDAs
  with day-one terminal identity (no new permanent-rent rows), Plane-C fee
  atoms credit an ordinary treasury-owned Position (zero new atom-holding
  families), IntentFeeCarry semantics wired via the next reservation
  version, 60/0/40 split with executor deferred, the five zero-gates
  each mapped to exact relaxation requirements, eight falsifiers required
  before any nonzero charge. Base/rate/promotion explicitly undecided;
  six ember decisions queued in its section 11. Mail PR #1908 MERGED
  (03:40Z). SOL collector restarted (pid 26807). T2-6 walk lane resumed
  after the usage-limit window.

- CYCLE B ATTESTED: Persvati 43/43 portable gates PASS, 0 STOP over exact
  6827749 (grew 41 -> 43 contract-driven: the reseal's own closure expansion
  forced clutch-batch test+clippy gates). ELF fda59705 byte-verified five
  contexts; all four historical roots verified with the set asserted
  exactly; manifest digest-check green both hosts pristine; 1,326 file
  comparisons x2, zero mismatches. Job:
  persvati:/home/ember/jobs/dragons-clutch-portable-attest-6827749-20260820-VwFoCv.
  The overnight runtime is now sealed, manifested at 100/100, and
  independently attested — same chain depth as the morning baseline, four
  hours after the code stopped being the same code.

- CYCLE B SEALED END TO END (manifest 6827749 at 100/100, post-commit
  check OK, pushed; attestation lane dispatched): liveness profile resealed
  at fda59705/1,527,640 (new 25-file root, all 11 measurement families
  re-derived from fresh logs — drift +4 CU on folds, worst +274 = Tier 0's
  measured cost, blank-bank creation FELL 7.3%; relocation byte-identity
  PERSISTS; audit closure gap closed, declared+digested 94 -> 104). The
  emission's drift refusals all did their jobs and were repaired honestly:
  four more stale locks from the T2-2 dep edge, the Verus EXCLUDED-source
  digests re-recorded (exclusion scope unchanged, nothing new claimed
  proven), and the cost-lab abi-audit refusing the codec width drift it
  exists to refuse — every downstream pin moved by exactly the 746-byte
  packing saving, goldens regenerated. Three emissions total (8 then 2
  then 0 contradictions). T2-6 (the walk, tags 49-53) building in its
  worktree meanwhile.

- TIER 2 WAVE 1 COMPLETE, ALL FIVE JOINS MERGED (through the T2-3 merge):
  T2-1 codec (ENCODED_BYTES 47,846; 210/210 resume points on real
  encode/decode; 106k-flip hostile battery; 35-region tamper boundary with
  the 29 residual regions explicitly assigned to the anchor comparison),
  T2-2 projection (phantom-tag falsifier), T2-4 live-cardinality binding,
  T2-5 policy profile, T2-3 staged creation (tags 47/48 LIVE, five-in-one
  creation at 87,090 CU total after reconciling onto the real codec — the
  provisional-vs-canonical collision was forced visible by the pin test
  exactly as designed, and the real codec made the final grow 3x cheaper).
  Also merged: Tier 3 host hygiene (participation_from_fills 24,704 -> 320;
  ELF byte-identical) and the V3 terminal classification (37 -> 44 rows,
  11 -> 14 blocking ids; NEW FINDINGS: Epoch V4 receipts and final policy
  artifacts have NO close route — permanent rent by design, recorded as
  DIRECT.EPOCH_RECEIPT_RENT_PERSISTS / DIRECT.POLICY_ARTIFACT_RENT_PERSISTS;
  V3 close evidence exists in tests but is unsealed —
  DIRECT.V3_CLOSE_EVIDENCE_UNSEALED). Full gates on final main: default
  85/0, mock 92/0. New identities: default fda59705/1,527,640, mock
  17d031c4/1,556,112. CYCLE-B RESEAL LANE DISPATCHED (new root, all 11
  measurement families re-derived, audit closure gap closing with
  batch-policy-identity added to declared source_paths). Next after seal:
  manifest emission, attestation, then T2-6 (the walk).

- CYCLE A SEALED + MERGE TRAIN LANDED AND PUSHED (through 6bc6628): cycle-A
  manifest 100/100 at 9d84e55 with post-commit check OK (batched-fold routes
  + 10k quantum + the Tier 1 source-identity refresh, all bound to the
  attested 187d5ee1 artifact; a clean detached double-build at the merge
  commit reproduced the identity byte-exactly on both passes, which is what
  made the refresh honest). Then the train: T2-5 (general clearing policy,
  55 tests), T2-2 (projection + interner, 206 layout tests), T2-4
  (live-cardinality binding), frame Tier 0 (ten overflowers under the line;
  opt-z suite now FULLY GREEN at a 1,092,928-byte ELF, -23.4%) — all merged
  clean, full gates on the merged tree: default 80/0, mock 87/0 (+2 batch
  tests each). NEW IDENTITIES (drift window knowingly open until cycle B):
  default a982a080/1,426,904, mock 9dba6b26/1,455,376. T2-3 (staged
  ClearWork creation) launched off merged main; T2-1 (codec) still out;
  cycle-B reseal runs when both land.

- TIER 2 PLAN COMMITTED (5c95b6c, docs/design/TIER2_PORTFOLIO_CLEARING_PLAN_2026-08-20.md)
  and WAVE 1 LAUNCHED: route-family decision argued (NEW general-epoch
  family; V3's one-u64-price candidate wire cannot represent a simplex, so
  extension is structurally impossible — five load-bearing two-order
  constraints tabulated with file:line). Eight joins ordered: five parallel
  (codec T2-1, projection T2-2, creation T2-3, tombstone cardinality T2-4,
  policy domain T2-5), then the walk (T2-6, tags 47-53 sketched), then
  selection T2-7 and entitlements T2-8. Lanes running now: T2-1/T2-2/T2-4/
  T2-5 in isolated worktrees; T2-3 (SVM-heavy) queued for a build slot.
  Flagged for ember: T2-5 pins GENERAL_CLEARING_POLICY_V1 as PROPOSED
  (zero-fee, one dust choice, RefuseOverlap) — sign-off freezes it.

- FRAME TIER 1 MERGED (d2858ee): portfolio_settlement::{prepare,apply}_full_pair
  to &mut out-slots — apply 6,784 -> 128 bytes, prepare 4,224 -> 2,688
  (extra ~800 over the plan's estimate is two Result sret temporaries;
  still 1,408 under the line). Module lives in programs/solana-layout (the
  no_std layout crate), not clutch-batch as the plan guessed. Six full_pair
  frame diagnostics vanish from the build log (97 -> 91). ELF IDENTITY
  PRESERVED — I rebuilt on merged main and got exactly 187d5ee1/1,420,608
  (zero callers, LTO-eliminated), so NO reseal owed for this merge. 7/7
  module tests + clippy verified on main. ZEROED placeholders added per the
  crate idiom for no_alloc callers.

- PERSVATI ATTESTATION OF THE NEW IDENTITY PASSED: 41/41 portable gates,
  0 STOP, over exact 98fb070 (prior 40 gates + a new bare policy.py
  pristine-checkout gate). Archive ac0efaa7/bundle e84da342 identical both
  hosts; 187d5ee1 ELF byte-verified in FIVE contexts; manifest digest-check
  green in pristine checkouts both hosts; 45/45 policy tests on the second
  host; toolchain pinned fail-closed from the start (drifted default never
  invoked). 1,256 file comparisons run twice, zero mismatches; three
  historical ELFs byte-verified. Honest wrinkle preserved as a blocker
  record: the two new TrackedEvidenceTests correctly REFUSE outside a real
  git repo (failed 2/45 in the extracted-archive context, 45/45 in the
  bundle checkout) — the hardening working as designed. Durable job:
  persvati:/home/ember/jobs/dragons-clutch-portable-attest-98fb070-20260819-kCWSaj.
  CURRENT_TRUTH section 2 updated. Full reseal chain now: seal -> gates ->
  manifest 100/100 -> post-commit check -> independent portable attestation.

- FEE ECONOMICS PHASE 1 LANDED (9f7c155 docs + a88201d lab, verified and
  pushed): FEE_GEOMETRY/ECONOMICS/OPEN_QUESTIONS corrected per all twelve
  findings (zero-price kernel channel now in the section-5 threat list with
  Prop 9 cited; Prop 10 refutation + Props 11-12 characterization absorbed;
  the eight unsupported assertions fixed; two decided OPEN_QUESTIONS rows
  retired). Lab: FeeBasis grew PER_EGG_LEG (arm 3) and QUOTIENT_RANGE
  (arm 6); tests 25 -> 33 OK. DECISION-RELEVANT SURPRISE from the executable
  Prop-9 falsifier: on risk transfer supported entirely on zero-priced
  outcomes, THREE of four arms charge exactly zero (dispersion, flat-cash,
  AND per-Egg all share the hole — zero-priced legs have zero consideration);
  only the quotient-norm arm charges (exactly M/1000 at every tested scale
  up to 10^30). The evasion channel is not dispersion-specific; it is a
  property of every consideration-proportional base. Ember's fee-base
  decision should weigh this.

- MANIFEST RESEALED AT 100/100 EXECUTED GATES (98fb070; emission provenance
  400bcbf; post-commit check --run-gates fully green: digests, toolchain,
  declarations, exit codes, key lines all match). The combined reseal the
  sha-syscall and V3 merges owed is COMPLETE: seal cfba5bb + gate repairs
  2196111 + reattribution e6d477e + truth binding 400bcbf + manifest
  98fb070, all pushed. First 100-gate emission found 9 contradictions; all
  diagnosed and repaired (4 stale locks, 1 SBOM row, 3 undemotions, 1
  unused-mut); second emission 99/100, third 100/100.

- LIVENESS PROFILE RESEALED to the syscall-hashed runtime (d8c5034 enabling
  lock + cfba5bb seal): new root 187d5ee16f72946a, 24 files all regenerated,
  every CU row re-derived from fresh bank logs (5-13x cheaper across all 10
  families). BOTH RECORDED STOPS DISSOLVED IN SEALED EVIDENCE: V2 select
  completes at 226,071 CU (derive() still keeps V2 STOP on the
  empty-frozen-lapse blocker, live_v3 stays false), all six occupation rows
  clear admission. NEW: the relocated-Cargo-home build is byte-identical for
  the first time — the path sensitivity left with the software sha2 crate.
  I re-verified the gates myself (45/45, 24/24 tracked, policy.py both
  modes). Inversion found: cancel now costs MORE than place (282,868 vs
  185,807) — no policy reads it, but it is a real fee/UX fact.

- THE CEILING'S LAST TENANTS EVICTED (2196111): the 9 gate contradictions
  from the first 100-gate emission were diagnosed — 4 stale Cargo.locks
  (same solana-sha256-hasher edge), 1 SBOM row (devnet-paces), and the good
  one: THREE test cases authored with full oracle expectations had been
  demoted to `exhausted` because two instructions could not fit 1.4M CU.
  Undemoted, all three EXECUTE AS AUTHORED FOR THE FIRST TIME: Resolve
  repeat-idempotency ACCEPTED (263,317 CU, 7 accounts oracle-compared),
  late-conflict payout refusal 0x0057 with whole-transaction rollback, and
  the committed walk's duplicate-bearer-exit atomicity witness. Bringup and
  committed gates green at the same ELF identities; walk now declares
  refusals=2 / exhaustions=0.

- REATTRIBUTION PROPAGATED (e6d477e + degg ef10f29): CURRENT_TRUTH rows and
  STOP ledger, sophistication section 3, succinct-clearing premise, and
  planned-vs-built each carry a same-day dated correction with original
  text preserved; BOTH Draft 13 filings rewritten from the sealed merged
  measurement — the IAC ceiling section now leads with the misattribution
  as a committee-relevant finding ("a measured stop is evidence about an
  artifact, not an architecture"), pages held at 9/8.

- DRAFT 13 COMMITTED (degg-research 8f9cfef): all four documents compressed
  to their content floor with meaning as the hard constraint. IAC 4,228 ->
  3,602 words / 10pp -> 9pp; definitions 3,341 -> 2,754 / 8pp -> 7pp;
  data-reporting 4,009 -> 3,188 / 9pp -> 8pp; cover found to be AT its
  floor (699 words, zero free cuts — four passes, every candidate rejected
  as meaning-bearing). Packet 29pp -> 26pp, 12,277 -> 10,243 words (-17%).
  Every doc independently verified by me: builds green, zero numbers lost
  or invented, citation multisets identical, all requests/positions
  survive in imperative form. CONVERGENT FINDING from all three body
  lanes, with arithmetic: Draft 12 was already de-hedged, and protected
  content (tables, appendix basis rows, measurements, worked examples) is
  ~2,400-2,500 words in definitions alone / ~27% of data-reporting — the
  55-65% target is unreachable without deleting claims, which stays
  ember's editorial call. Each lane's report carries an itemized
  borderline list (passages compressed hard enough to argue about).

- SHA-SYSCALL MERGED TO MAIN at 6c25df4 (rebased onto main, fast-forward,
  pushed). Both declared gates green on the merged tree: default profile
  78 passed / 0 failed, mock 85 / 0, all targets. Default ELF reproduces
  the verified post-SHA identity EXACTLY: 187d5ee1..., 1,420,608 bytes
  (mock: 9c8a86e1..., 1,449,080). RESEAL LANE DISPATCHED: new artifact
  root 187d5ee16f72946a regenerated from scratch (audit + 9 bank logs +
  evidence.json re-derived, never transcribed), old root retained
  historical, admission-shape re-examination as report-only. Manifest
  emission + Persvati attestation remain mine after it lands.

- FILING PROCESS + FORWARD CALENDAR memo landed (degg-research 3f021fe,
  FILING_PROCESS_AND_CALENDAR.md, 47 verified primary sources): the IAC
  MEETING IS AUG 20 (tomorrow), 1-4pm EDT, public access listen-only; the
  Aug 27 date is the written-statement "should submit by" deadline (modal
  is "should", the FR structured close date is null). NO speaking channel
  exists or ever did across 8 CFTC advisory-committee notices 2023-2026 —
  nothing was missed. Statements land on the regulations.gov docket
  unreviewed and are NEVER surfaced on event pages (live test: AAC met
  Jul 29, zero statements posted). Only NPRM comments and part-13
  petitions create agency obligations; all four of our matters are RFCs/
  advisory statements. FORWARD HEADLINE: Compute Derivatives RFC (RIN
  3038-AF77) issued TODAY, 60 days from FR publication, asking about
  unobservable/unsurveillable reference prices and perpetual compute
  futures — squarely our material. Eight corrections to companion memos,
  two big: SEC comment lists DO exist (7 filers + 5 ex parte memos the
  CFTC dockets don't show; sec.gov wants a declared-identity UA), and
  ISDA/SIFMA filed 2026-05-20 via the SEC-CFTC Harmonization Initiative
  log — a joint channel the earlier memos didn't know existed.

- FRAME BUDGET PLAN committed (docs/design/FRAME_BUDGET_PLAN_2026-08-19.md):
  the frame blocker for the general relation is ALREADY SOLVED in-repo and
  unconsumed — relation_v1_stream verifies one order at a time (push_order
  frame 1,280 B vs verify_inner's 39,104, measured in the same build) with
  Portfolio support and 19,520-verdict equivalence, plus the clearing.rs
  checkpoint codecs. Portfolio clearing is JOIN-blocked (8 named joins), not
  frame-blocked. Impossibility recorded: verify_inner/canonical_candidate
  need 28 KB co-live state (7x frame, N<=3 at K=16 if reduced);
  propose_best_valid is a host solver by construction. INVERSION: binary
  size is gated by the frame ceiling — opt-z saves 23% (~2.3 SOL rent) and
  fails only because TEN reachable functions go 64-896 bytes over; ~12
  resident handlers sit at exactly 4,096. Tier 0 (the ten overflowers) +
  Tier 1 (portfolio_settlement out-params) dispatched on the sha-syscall
  branch; Tier 2 (consume the streaming relation) is the real portfolio
  unblock, next cycle.

- DRAFT 12 COMPLETE, all four documents (degg-research e4429c9 + 4caeb80):
  IAC 10pp, definitions 8pp, data-reporting 9pp, cover 2pp. The
  triple-statement pattern (summary + argument + requests saying the same
  thing three times) collapsed to asks-first-argued-once; compute
  measurements got their own led section; every scope caveat kept verbatim
  and enumerated in the report as scope-not-fear. HONEST FLAG from the
  lane: the 6-8pp target is NOT reachable without cutting real content —
  per-page density is unchanged since draft 6 (484 -> 509 w/pp); what grew
  is the compute measurements, fourth negative, operatorless section, and
  leakage table. At 10/8/9 the packet sits at corpus median. Which
  addition leaves, if any, is ember's editorial call. Note: data-reporting
  positions renumbered (6 positions -> 5 requests); any external "Position
  N" citation into that document is stale — John packet pointer update
  belongs at ember's filing freeze.

- THE COMPUTE CEILING WAS A HASHER, NOT AN ARCHITECTURE
  (docs/reviews/COMPUTE_CEILING_REATTRIBUTION_2026-08-19.md; work on
  fable/sha-syscall, NOT merged). The 53,952-byte sha2::compress256 entered
  through ONE unconditional dependency edge in batch-policy-identity, not
  the call sites I briefed; target-gating it (syscall on SBF, portable
  retained as differential oracle) removes it. I VERIFIED the headline
  myself by clean rebuild: 1,420,608 bytes, 0 compress256 symbols,
  U sol_sha256 in the dynamic imports. No digest value changed; identities
  pinned against independent Python hashlib bytes.
  EVERY measured instruction got 3-8x cheaper. Tightest row 1,120,392 ->
  198,483 (80% of ceiling -> 14%). TWO RECORDED STOPS DISSOLVE: Direct V2
  full selection now COMPLETES at 226,071 CU instead of exhausting 1.4M and
  rolling back, and all six occupation admission rows now clear the
  25%-headroom gate. The STOPs were correctly measured; their cause was
  misattributed to the algorithm.
  This invalidates the ARCHITECTURAL generalization in
  SOPHISTICATION_GAP section 3, the premise framing in
  SUCCINCT_CLEARING_FEASIBILITY, and the "single-transaction re-execution
  does not scale" passage in BOTH draft filings. V3's staged design still
  stands on its own merits; V2 completing is NOT a promotion.
  Measured negative recorded so nobody repeats it: opt-level z/s shrink the
  binary 21-23% and FAIL tests with 205/139 SBF frame overflows.

- FEE ECONOMICS MAP + CORRECTION RECORD
  (docs/reviews/FEE_ECONOMICS_FINDINGS_2026-08-19.md): twelve places where
  FEE_GEOMETRY/ECONOMICS assert properties the project's own research
  refutes. Headline: RISK_SUMMED_POSITIONS Prop 9 PROVES that at boundary
  prices the dispersion kernel is strictly larger than the risk quotient, so
  risk transfer on zero-priced outcomes is LITERALLY FEELESS however large
  its range — a proved evasion channel absent from the fee's own §5
  laundering list, with no zero-price falsifier in the lab. Also: dispersion
  is NOT the quotient norm (Prop 10 refuted) but G IS characterized, not
  merely constructed (Props 11-12) — neither correction absorbed. The
  promotion gate is UNSTARTABLE not pending: the per-Egg control arm the
  design was built to beat does not exist in any language, a sixth
  quotient-norm arm is owed, and four of eight measurement axes need a
  market-quality simulator that exists nowhere. Corrected the earlier
  scorecard: clutch-liveness IS transitively in the ELF and reached at
  runtime (DonationLedger on the V3 path), 1 of ~14 types wired.
  IntentFeeCarry already implements the recommended intent-scoped
  terminal-ceil design with zero consumers. The layout is already
  fee-capable; max_fee_atoms is forced zero at FIVE gates and there is
  nothing to pay a fee TO — RevenuePolicy is four documents of prose and
  zero code.

- SEAL TRACKED-NESS HARDENING LANDED (ecfd552): check_tracked_evidence now
  asserts every one of the 73 sealed-evidence paths is present, git-tracked,
  AND byte-equal to its committed blob at HEAD — closing the hole where
  policy.py read from disk and would have PASSED a half-committed seal. The
  lane REPRODUCED the actual near-miss out of band (11 of 24 files, the exact
  ratio) rather than asserting the check works, and git failure raises
  TrackingUnavailable rather than reading as "tracked". 7 new adversarial
  tests incl. a bundle-clone test for the Persvati attestation context;
  suite 38 -> 45. It also caught that a bare digest swap in the docs would
  have falsely bound the OLD CU table to the NEW ELF, and re-derived all 13
  rows from the sealed logs.
- IDENTITY RETIREMENT completed (7ec0f51 + follow-up): CURRENT_TRUTH, both
  handoffs, the formalization backlog, and two manifest gate notes now cite
  af6bb79c/1,490,544 sealed at 7931e23, with bd20711b kept as explicitly
  historical; the hbox rebuild paragraph is scoped as a historical
  comparison since no independent rebuild of the current identity exists.
  Gate note corrected 37 -> 45 tests. Emission is now unblocked of stale
  notes.
- DRAFT 12 COVER (degg-research 4caeb80): the packet critique landed — the
  filings are armored, not written, and the "stated once" paragraph I added
  in Draft 11 fixed the distribution of hedging, not the ratio. Rewrote the
  cover to lead with the system, three findings, and six concrete asks;
  surfaced the operatorless-agent question from page 7 into its own section;
  collapsed the repeated approval/opinion/deployment disclaimers into one
  short scope paragraph. Awaiting ember's read before running the same
  inversion through the statement and both joint comments.

- PLANNED-VS-BUILT SCORECARD committed
  (docs/reviews/PLANNED_VS_BUILT_2026-08-19.md): ~14% of originally planned
  commitments are in the sealed runtime, against V1_BACKLOG's self-reported
  53%. CORRECTION I OWE: the merged venue is a TWO-ORDER crossing engine
  (common.rs:510 forces order_count==2, same outcome, equal quantity,
  opposite sides, different owners, zero fee), roughly 12 transactions and
  ~7M CU per trade — narrower than "the venue exists", which is what I said.
  Portfolio/coefficient orders are placeable but structurally unclearable
  (orders_batch.rs:888), so the crown-jewel basis is inert as a shape.
  ADR-0003 is inverted with no superseding record: Rocq has ZERO theorems,
  Verus ~1.5 of 11, and Lean — explicitly warned against becoming mandatory
  — carries 184 theorems with zero sorry. NEW REGRESSION: the V3 merge added
  six persistent account families, none classified in the 37-row terminal
  inventory; the terminal ledger regressed the moment the venue landed.
  Also: no CI exists at all (.github/ absent); benchmarks/constants.json is
  pinned 3,109 layout lines stale and only soft-notes the drift it exists to
  refuse; of 100 manifest gates only 4 are SBF runtime gates.

- V3 CLAIM PROMOTION propagated: CURRENT_TRUTH capability matrix rewritten
  for the routed staged lifecycle (8162bae) with its exact boundaries —
  one bank profile, unpromoted in the liveness profile with live_v3 false,
  epoch-atomic with no per-order cancellation, V2 still a measured STOP.
  Draft 11 IAC + data-reporting updated in degg-research (9ceab9b): the
  "successor selection design exists as model and design only" sentence is
  now false and is replaced by the stop-and-the-redesign-that-answers-it
  passage, keeping the one-bank-profile and unpromoted caveats explicit and
  preserving the best-valid-submitted-candidate ceiling. IAC 11pp.
- Hardening lane dispatched for the seal tracked-ness hole plus the three
  docs still citing the superseded bd20711b identity.

- LIVENESS PROFILE RESEALED to the V3 runtime at 7931e23: new artifact root
  af6bb79cc3766bd0 (24 files, all regenerated not copied; report
  39a8b19c..., 50-file ledger e433c17d... verified by shasum -c). Identity
  reproduced independently in a fresh detached worktree, confirming it is
  source-path-independent. EVERY ResolutionWork route moved by exactly
  +1 CU — the precise cost of one added dispatcher arm — so no measured
  STOP and all routes still clear 25% headroom. Blank-bank market creation
  actually FELL (~9k CU each). Source closure grew 88 -> 94 files.
  WATCH ITEM: Finalize at 1,094,833 is only 25,167 CU below its 1,120,000
  admission boundary — the tightest row in the profile, and one more
  dispatcher arm's drift is not a large budget.
  EVIDENCE-INTEGRITY FINDING: the repo root ignores *.so and *.log, so a
  plain `git add <artifact-dir>` silently committed 11 of 24 files; caught
  and amended with -f, now verified 24 tracked == 24 on disk. The hazard is
  that policy.py reads from DISK, so a half-committed seal would still have
  PASSED its gate. Worth a tracked-ness check in a later hardening pass.
  policy.py was strengthened to fail closed on overwriting a superseded
  artifact root or dropping files from a superseded seal.
  Honest scoping: Direct V3 is resident but UNPROMOTED in the profile — no
  CU, rent, or terminal-admission rows, live_v3 stays false, and the Direct
  STOPs remain the V2 ones.
- DECISION: holding the 100-gate emission for ONE combined cycle with the
  SHA-syscall size/CU work rather than resealing twice. Rationale: the
  deploy is faucet-blocked so there is no deadline pressure, and I adopted
  "reseal at meaningful checkpoints, not after every wave". The tree is
  knowingly unsealed meanwhile (manifest drift 554 -> 599 entries recorded
  above), which is honest rather than hidden.

- DIRECT V3 MERGED TO MAIN at fb72b34 (15,074 insertions, 23 files). The
  venue exists: staged clearing routed for tags 36-46 behind one dispatcher
  arm. Rebase (not cherry-pick) was directed and paid for itself twice —
  it exposed a FOURTH instance of the 1e8b8a3 non-exhaustive-match defect
  that only a rebase could reveal, and my predicted post-rebase counts
  (78/85) caught the lane running a "default" suite against a leftover mock
  ELF. Final: default 78/0 (af6bb79c), mock 85/0 (3ae97767), offline suites
  green, strict clippy clean on all touched crates. Re-measured CU keeps
  the conservative figure per row; tightest is Submit-replacement at
  1,127,892 = 80.6% of ceiling. Watch item: Finalize at 659,231 now clears
  its documented "under 660,000" claim by only 769 CU — do not restate that
  margin loosely. Per-order cancellation answered: DESIGN DECISION, V3 is
  an epoch-atomic two-order book (page requires tombstone_count == 0,
  committed into the digest); a placed order's only pre-Freeze exit is the
  permissionless AbortUnfrozenDirectV4, so no value is trapped but
  placement-to-submission-open is a committed window — a venue property,
  not an implementation detail. Lane also self-reported erasing the word
  "Cancel" from a predecessor-audit sentence and restored it.
  RESEAL CYCLE NOW MINE: new ELF identity by construction.

- DIRECT V3 ROUTED AND COMPLETE on codex/r3-direct-v3-successor (b00dea1,
  10 commits): the full lifecycle — InitEpoch, InitOrderPage, Place,
  Freeze, Abort, Submit/admit, staged Verify, Finalize/Select, Settle,
  three Lapse phases — with tags 36-46 live through ONE dispatcher arm,
  exhaustive handler match, zero NotYetImplemented in the family, and the
  legacy/V3 decoders refusing each other. THE 1.4M WALL IS BEATEN: no
  measured row is a STOP; worst is Submit-replacement at 1,123,392
  (19.8% headroom). Two cost corrections, not squeezes: single-site epoch
  validation cut PlaceOrder 1,249,641 -> 784,232 and InitOrderPage
  641,047 -> 407,028 by not re-running identical decode-time SHA
  validation up to four times; no refusal removed and the substitution
  suite still refuses every mutation. The predecessor's exact failure mode
  regresses in both directions. Product gap found and named: there is NO
  per-order V4 cancellation — an order can only be retired by aborting the
  whole unfrozen epoch. Claim plane: model + host + focused SBF-EXECUTED
  on a branch. RESOLVED after refutation: the lane claimed its 4 test
  failures were "pre-existing and not mine — proven, not asserted", and
  measurement refuted the proof. Clean main, full logs, both declared gates: default profile
  16 targets / 75 passed / 0 failed (native_resolution 15/15), mock profile
  16 targets / 82 passed / 0 failed (native_full_lifecycle ran and passed).
  Sent back to root-cause properly. The corrected answer was neither of our
  hypotheses: all four failures are ONE bug, Custom(0x0050)
  ReferenceError::Window on native Resolve, and it predates the branch —
  the merge-base 414d6e4 itself fails. Main had already repaired it at
  161f530 ("Bind native fixture source version consistently"), a
  HARNESS-ONLY fixture fix binding an inconsistent source version, which is
  itself the proof no program refusal was involved. The branch was ~20
  commits stale and never received it; the failures were unobservable
  earlier because 1e8b8a3 broke branch compilation under both profiles.
  Two method errors, both reusable lessons: (1) branch test sources run
  against a base-built ELF proves nothing about the base; (2) no
  --no-fail-fast, so "42 passed" was 9 of 16 targets — a fail-fast result
  is not a suite result. Fixed by cherry-pick 028e7cb; both profiles now
  green at exactly main's numbers (75 default / 82 mock). Full rebase onto
  main directed before merge, since staleness that hid four failures is
  what a rebase eliminates.
  Process note on myself: my first read of this claimed a gate-coverage
  hole, which was an artifact of piping the gate through `tail -40`. The
  house rule about teeing build logs exists for exactly this; a truncated
  log nearly became a false evidence-integrity alarm.

- SUCCINCT CLEARING FEASIBILITY SCOUTED
  (docs/design/SUCCINCT_CLEARING_FEASIBILITY.md): the compute wall is NOT an
  architectural dead end. The adjacent breadstuffs tree's Groth16 verifier
  is native SBF, measured at ~255k CU in a 795-byte transaction — 5.5x
  margin against the 1.4M ceiling that killed V2 — and Cert-F's Lean
  keystone IS verify-not-find (certifies_epsilon_optimal quantifies over
  all feasible flows), refinement-proved over the EMITTED descriptor, zero
  sorry, axiom-audited, with refusal teeth exhibited and the
  modular-to-integer boundary carried as honest hypotheses. Dragon's Clutch
  independently derived the same dual object in DUAL_IS_THE_MEASURE.
  Two gating conditions: dev single-party trusted setup (toxic waste known
  — ceremony problem, longest lead time) and the missing Cert-F-to-Groth16
  wiring (today the wrap consumes a 25-lane turn statement). Folklore
  corrected: the forked FRI is a COMPLETENESS defect not a soundness hole,
  and the restricted-license vendor is off the Cert-F path entirely. Debt
  surfaced: fhegg-solver/src/air.rs is a hand-written Rust AIR twin;
  Dragon's Clutch itself has zero AIR debt. Unmeasured and not estimated:
  Cert-F proof size/time at real batch width, and shrink feasibility.

- R2 RUNTIME CAPABILITIES LANDED on fable/r2-runtime-capabilities (f9045a0):
  the two decoders with zero precedent in the tree — Upgradeable Loader
  ProgramData and Instructions sysvar — verified against pinned published
  crate sources (loader-v3-interface 8.0.1, instructions-sysvar 3.0.1,
  layout byte-identical across 2.2.2/3.0.1/4.0.0), fixtures captured from
  the REAL serializers not hand-tables, 42 adversarial tests (truncation at
  every byte boundary, off-by-one sweep on the current instruction,
  non-adjacent post unreachable), 24 refusal variants, clippy+fmt clean,
  199 lib tests total with exact +42 delta, wired into nothing.
  Two findings: (1) a revoked upgrade authority serializes to 13 bytes but
  the loader's metadata region is fixed at 45, so bytes [13..45) still hold
  the PREVIOUS authority — a naive decoder reports a live authority on an
  immutable program; ours never reads them and proves it. (2) the current
  instruction index lives in a 2-byte trailer outside the documented
  layout, so every body read is bounded by len-2. Also surfaced: two
  pre-existing private_intra_doc_links rustdoc warnings
  (observe_resolve.rs:52, split.rs:54) that would block any future strict
  rustdoc gate. Merge needs a reseal cycle; disjoint from the V3 lane.

- SOPHISTICATION GAP ASSESSMENT committed
  (docs/reviews/SOPHISTICATION_GAP_2026-08-19.md): the joins are the
  fiction (three seams structurally impossible on a public cluster, not
  merely untested), toy dimensions tabulated with citations (16 outcomes /
  16 order slots / 32 archive records / 8 presets), the compute verdict
  (1.07M CU for the simplest resolve against a 1.4M ceiling is an
  architectural result, not a tuning one), absent layers (fees, LP,
  terminal closure, upgrade governance, client), and the two strategic
  moves: make the spline claims tradeable via V3, then answer the compute
  wall with succinct verification — joining the consumerless Lean-authored
  STARK stack in breadstuffs. Stop adding verified components.

- DRAFT 11 COMPLETE, all four documents (degg-research 0dd6601/4ce0ce6/
  e4bfabf): definitions 9pp, data-reporting 10pp, IAC 10pp, cover 2pp.
  Named system throughout, one status paragraph each with [DEVNET RECORD]
  fill-ins, all VERIFIED/STOP label blocks unwound, appendix bases one line,
  positions/tables/citations intact. Freeze-sensitive: the V3
  successor-selection sentences; update at ember's filing freeze.

- DEVNET PACES HARNESS LANDED (9bee35f): standalone devnet-paces binary,
  24 unit tests, pedantic clippy; dry-run PASS both profiles on a blank
  validator at fresh program ids (devnet's exact shape) plus a required-red
  negative control. Honest finding: the mock provider trio cannot exist on
  a public cluster, so devnet paces = 28 accepted public transactions
  (tokens, artifacts, Realm/Profile, full market plane incl. native v3
  record) + exact refusal boundaries (0x79/0x7a/0x4) with byte-identical
  rollback; the funded lifecycle remains local evidence until the real
  Pyth-pull build. Mainnet double-refused by URL allowlist + genesis hash.
  Ready to fire the moment the deployer is funded.

- DRAFT 11 x2 (degg-research 0dd6601, 4ce0ce6): both Aug-24 documents
  rewritten in the named-system register — definitions (9pp) with the
  smooth-claim worked example and Pyth crossing-rule source note;
  data-reporting (10pp) with the four mid-paragraph VERIFIED/STOP blocks
  unwound and appendix bases cut to one line. One Track-C status paragraph
  each with [DEVNET RECORD] fill-ins. IAC statement + cover (Aug 27) next;
  their V3-model-only sentences stay freeze-sensitive to the promotion lane.

- POST-RESOLUTION CONSUMER AUDIT: CLEAN, zero suspects across all fifteen
  resolution-fact consumers in the sealed runtime + public adapter; the
  never-infer rule holds everywhere; four honest asymmetries recorded
  (window-id depth, non-local zero-vector refusal, caller-trusted indices,
  SettleDirectV2 lifecycle-blind receipt). Backlog item checked, STOP 2
  audit-half closed: docs/reviews/POST_RESOLUTION_CONSUMER_AUDIT_2026-08-19.md.
- DRAFT 11 EXEMPLAR delivered (degg-research 0dd6601): definitions comment
  rewritten around the real named system — smooth-claim worked example with
  partition-of-unity as the complete-set theorem, Pyth crossing-rule source
  note, one Track-C status paragraph replacing eleven scattered negations;
  9pp, builds clean. Awaiting ember's register check before propagating to
  data-reporting + IAC + cover.

- AMBITION UPGRADE executed: R2 promotion plan committed (c364630 — three
  phases, 36-delta bridge, gates incl. ember's explicit registry-flip go);
  devnet job created (~/jobs/dragons-clutch-devnet-20260819: fresh deployer
  4zrxtw5c..., program ids 3SLhMAFm... default / EbWhsDm4... mock; sealed
  bd20711b ELF verified for deploy); faucet rate-limited so a patient SOL
  collector runs in background; V3 ATOMIC PROMOTION lane launched on the
  successor branch (merge settle port -> full lifecycle handlers -> one
  all-or-nothing route family -> hostile + real-bank campaign, with the
  predecessor's legacy-intent failure mode as a mandatory regression).

- HANDOFF PRIORITY 1 COMPLETE: dependency/license closure cataloged as two
  declared gates (d2e1cd5; complete-scope + SBOM byte-equality, catalog
  regenerated to 33 manifests / 1,790 rows), 100/100 clean-tree emission,
  manifest committed at bd89de3, post-commit check --run-gates fully green.
- Settle port secured: the successor branch had dropped 8608385's settle
  Position-transfer semantics; cherry-picked clean onto
  fable/v3-settle-port (c5d2081), 44+177+10 tests + clippy green on the
  successor base. Parked for the atomic V3 promotion.

- BASELINE REFRESHED: clean-tree emission at c4688da matched 98/98 (gate
  inventory grew with codex's R2-auth and failure-payout crates), manifest
  committed at 5b68601, post-commit check --run-gates fully green. The
  post-seal wave (SBOM closure, hygiene, truth updates, terminal-identity
  crate) is now inside the checked baseline.

- R4 interim step 3 LANDED at eb1215a: TerminalIdentityV1 56-byte header
  research crate (PROPOSED pending ratification), delegating to the
  clutch-liveness DonationLedger kernel; 16 tests incl. four falsifiers
  (prefund-never-reaches-payer, monotone donations, exact close
  conservation, deficit-refuses), clippy clean. Baseline content identity
  drifted 546->554 entries across the post-seal wave; re-emission claimed
  and launched below.

- SBOM/LICENSE CLOSURE LANDED at b10f1ea: the dependency/license checker
  now lives in-repo (previously only inside the persvati attestation job),
  attested 12-manifest default mode proven byte-stable three ways,
  --complete mode covers all 32 manifests (1,788 rows, 0 failures) with a
  committed SBOM TSV; vendored solana-define-syscall handled structurally;
  notable-but-green license rows flagged for release eyes. STOP 8 residue
  narrows to macOS byte-repro, gate-folding at next emission, license-row
  review, external review, signed tag.

- Codex convergence wave (2f55cfc..8e7f827) composed onto my lanes: R2
  contract RESOLVED (closing-boundary rule id 2 only, ProgramData+slot
  pinning, grid origin 0, distinct-witness refusal) with the codec
  reconciled on top of my spec_v2/crossing_v1 base (368-byte spec,
  auth_v2.rs, 32 tests); scalar batch dust-loop progress proved in Verus;
  failure-payout economics decided as evidence-only recovery
  (FAILURE_PAYOUT_DECISION_V1 + research/failure-payout-v1, consistent with
  my R4 frame: rejects equalization/pro-rata/expiry as non-neutral); fresh
  manifest re-emission at 8e7f827. Decision queue items 1-2 are therefore
  resolved-by-codex pending your ratification pass.

- HBOX INDEPENDENT REBUILD LANDED (job dragons-clutch-sbf-rebuild-6743b9d-
  dd4727): exact pinned toolchain installed from checksummed official Anza
  artifacts, 30/30 locked crates verified offline, two fresh builds
  byte-identical at 5e840bb0... — divergence from the macOS-built seal
  exhaustively classified as per-OS platform-tools bytes (CI path strings,
  intrinsics reordering, -8-byte shift), zero bytes from source/deps/hosts.
  Cross-OS byte-identity structurally impossible for this pin; macOS second
  host is the remaining byte-reproduction gap. UNAVAILABLE/STOP blocker
  closed; CURRENT_TRUTH Sections 2/4/6.8 + handoff updated.

- Luna hygiene sweep landed at 7e45371: ten stale-seal/manifest/evidence
  fixes across implementation docs (a5725a3d references labeled historical,
  schema-v2 status corrected, v4-audit P2 wording repaired), zero broken
  links in 136 files, forbidden-claim scan clean. Its flags fixed here:
  CURRENT_TRUTH matrix row (joined evidence landed), GOAL thrust line,
  merged R2 worktree pruned. Program-source P2 wording flags stay parked
  (sealed runtime; next unseal cycle).

- STOP-#1 successor gate LANDED at 896a1cc: per-degree (1/2/3) continuous
  blank-bank joined lifecycles, SBF-EXECUTED under both ELF campaigns —
  funded segment fully public on the mock ELF (incl. public source
  create/append/seal) with exactly 4 named injections; default ELF asserts
  the 0x79 boundary per degree with byte-identical rollback; every step
  under the 1.4M CU ceiling; suites 75+82 green; sealed runtime untouched.
  Remaining to fully-public: production registry release, a real provider
  program, and public evidence-buffer construction (new runtime surface —
  next seal cycle). CURRENT_TRUTH STOP 1 updated.

- V3 Settle unit LANDED at 8608385 on codex/r3-direct-v3: consumed
  reservations archive exact consumed amounts, seller remainder refund per
  frozen semantics, in-kernel value-plane conservation, receipt binding;
  4 anchored tests (44 research + 189 layout green, clippy clean). Lane
  collision documented: codex is ACTIVE on that branch (advanced b49c497 ->
  e8ba1e4 mid-unit, reset the shared tree three times, independently
  implemented most of the settle kernel at e77238f, closed findings B/C at
  6267fde/081bd81 despite ember-pending status, and is now writing the V3
  SBF dispatcher route). My delta survived via a parked patch; committed
  atomically on disjoint files. V3 lane ownership is codex's from here.

- FRESH PERSVATI ATTESTATION PASSED: 40/40 portable gates, 0 STOP, over exact
  sealed 6743b9d — archive f9f25afc..., 528 files checked twice zero
  mismatches, every count identical to the b5da74f run, bd20711b ELF
  byte-verified both hosts, new digest-only manifest gate green, toolchain
  drift refused fail-closed then rerun at the pinned compiler. Job:
  ~/jobs/dragons-clutch-final-portable-attest-6743b9d-20260819-TChWnu.
  R1 exit criterion is now met in full (one commit, one identity, one ELF,
  one checked ledger, one independent portable reproduction). Recorded in
  CURRENT_TRUTH Section 2 + handoff.

- R1 MANIFEST SEALED (codex endgame): schema-v2 94/94 emitted, stabilized,
  committed, post-commit checked, and bound to the handoff at 6743b9d. My
  fast no-gates check confirms the manifest matches the tree byte-exactly.
- Fresh Persvati portable attestation of 6743b9d launched (fresh job dir,
  archive+bundle digests both hosts, previous b5da74f methodology, bd20711b
  artifact checks) — in flight.
- R4 runtime design PROPOSED at ad3ece9
  (docs/design/TERMINAL_LIFECYCLE_RUNTIME_V1.md): TerminalIdentityV1 header,
  37-row dispositions, economic-close-before-rent, fractional Arm A,
  MintCloseAuthority on new mints, two reference-ownership variants. Built
  on a full ground-truth map (V2 model internals, machine inventory,
  ResolutionWork/artifact/Direct-V3 funding precedents, 30 decision points).
- R2 v2 codec MERGED at 3b20ea6: SourceSpecV2 + CROSSING_V1 (both variants,
  falsifiers executable), 26 tests + strict clippy re-verified on main.
- John review packet refreshed in degg-research (55ce13a): Draft-10-aligned,
  15-row judgments table, five answerable questions with tentative answers.

- Runtime/profile repair resealed at `b5700a9`: current default ELF
  `bd20711b…b60`, runtime-artifact report `626a299d…e038`, and 52-file ledger
  `dbf55f8e…5f35`. The prior `83e124d` full run was 93/94 with only the strict
  liveness source-drift refusal. The old `a572…` / `7e8f6b1` / `b5da74f` seal
  remains historical evidence only. All provider, liveness, Direct V2,
  terminal, deployment, release, security, and legal STOPs remain open.

- Default-ELF identity fork surfaced and ledgered: 9c371fe's rustdoc-link fix
  touches closure file resolution.rs, so clean HEAD builds bd20711b... while
  the sealed a5725a3d... reproduces only at ec77d0b (proven by fresh builds
  at both commits). Since the doc fix is required for the rustdoc gate, the
  94/94 rule forces adopting the new identity + re-sealing the liveness
  profile, stack audit, truth Section 2, and a fresh portable attestation.
  Recorded in BASELINE_MANIFEST_DIAGNOSTIC; old seal remains valid
  historical evidence for 7e8f6b1. My bringup repair lane stood down
  correctly on discovering codex mid-flight on the same gate (its worktree
  dragons-clutch-bringup-split-79 + dirty main-tree files are codex's).
- R2 pull-profile v2 baseline landed on its isolated worktree branch
  (f0e7516 + 01291de) and is superseded there by 4daddd4: a 368-byte
  SourceSpecV2 body, closing-only CROSSING_V1 rule id 2 (ids 1/3 refused),
  exact ProgramData key/slot, zero grid origin, decoded-body duplicate
  collapse, start-aware contiguity, and named overflow refusals. Its 32 model
  tests plus clippy/rustdoc are green; its successor is integrated as
  research-only, with no post-cutover identity, runtime registry,
  loader/Instructions/Clock adapter, or SBF route. The default source registry
  remains empty and refusal 0x79 is preserved.

- Scalar Verus batch shadow now reports 28 verified obligations and five
  required red mutants. Its dust-choice results remain a digest-pinned
  mathematical correspondence review: executable loop completion,
  `left`/`assigned` invariants, source refinement, coupled relation, accounts,
  and SBF remain STOPs.

- `EvidenceOnlyRecoveryV1` is decided only for a new research profile: no
  numeric fallback, finite independently prepaid repair, recoverable dormancy,
  and exact-lot bearer units. It supplies no live ABI, source route, migration,
  Token-2022 CPI, or terminal authority.

- Terminal-economics R4 is MODEL-ONLY / HOST-TESTED research. Its `I`/`E`/`A`
  supply-plane and CreditVault model rule out general tombstone-only closure
  for arbitrary raw bearer quantities; it changes no runtime, mint, authority,
  migration, or R1 artifact identity.

- V3 blocker fixes LANDED at b49c497 on codex/r3-direct-v3 (jobs worktree,
  clean): order-body 107 with a red-demonstrated cross-crate model/live
  digest tripwire, zero-envelope creation refusal keeps release total,
  epoch-bound 96-byte direct_policy_v3_id enforced by all three validators.
  37+177+10+2 tests, strict clippy clean both crates, independently rerun
  before commit. Design doc now carries the open items (Settle kernel,
  verify_lease tautology, FROZEN_EMPTY pinning). Truth docs reconciled.
- Postmark doorstep cleared: five letters (stella-letta x2, postmaster,
  iris — yes to Garcin, aion-solare) shipped as PR #1880.

- STOP #1 reconciled with git truth: the basis-mode binding the STOP demanded
  landed at 3a81b38 (ancestor of frozen runtime 7e8f6b1; program-src diff to
  HEAD is empty; NATIVE_SEMANTICS_AUDIT_V4 reads REPAIRED P1 / PASS with all
  four named test families in-tree). CURRENT_TRUTH Sections 3/4/6, both
  handoffs updated; successor gate is per-degree blank-bank joined lifecycle
  evidence plus the Terms-to-payout refinement boundary. Verified the
  ancestry/diff/audit chain independently before editing.

- V3 blocker verification (cross-audit addendum): all three 9fd1ef1 blockers
  CONFIRMED with two-sided file:line evidence — model hashes a 99-byte order
  body vs live 107 (omits expiry_epoch; digests can never match on nonempty
  pages), zero-envelope buys are admissible yet unreleasable (abort/lapse
  permanently refuse; FrozenEmpty epochs stuck), and the reservation domain
  binds the legacy 64-byte policy digest while claiming the epoch-bound
  96-byte V3 identity. Verdict amended COMMIT→HOLD. Minimal fixes dispatched
  to the audit lane in the jobs worktree (no commit until my diff review).

- Ran the first full 94-gate schema-v2 emission from clean ec77d0b: 86/94
  match, content id 172ef191…; results consumed into codex's b837be7 ledger
  (v1 manifest retained under its 94/94 promotion rule). Emission raced
  codex's commits — quiet-tree convention adopted for the redo.
- Banked root causes: reference-lock drift = new path deps post-c05fe84;
  bringup walk-01/04/05 = 0x40 AlreadyInitialized + 0xb MismatchedState
  (intra-script bank/PDA collision), only committed-walk step 5 is the real
  0x79 fail-closed boundary.
- V3 adversarial cross-audit (read-only, jobs worktree): all six claimed
  closures real at model/codec level, none vacuous; found tautological
  verify_lease sink check (LOW), unpinned FrozenEmpty admission fields
  (LOW), Settle omission unlabeled in artifacts (MEDIUM), doc test-count
  drift 33/17→34/18; solana-layout strict clippy needs codec:12/:16
  cfg(test)-gated. Codex's three 9fd1ef1 blockers not yet examined by this
  audit — verification lane re-dispatched.
- R2 provider primary-source memo landed (scratchpad
  r2-provider-profiles.md, retrieval-dated): Pyth pull is the only
  candidate with a documented per-bucket uniqueness rule; Switchboard
  violates bucket uniqueness; Chainlink Solana is allowlist-gated with
  unverifiable mainnet identity. Pyth caveats: ephemeral update accounts
  vs SourceSpec exact-account field; DAO upgrade cutover 2026-08-26.
- Draft 10 LANDED in degg-research (a1b8aea): all four filing documents
  forked from frozen Draft 9 Typst sources with the ten-claim delta table
  applied (two-artifact 0x79 source truth, ResolutionWork PROFILE-ADMITTED
  maxima, occupation-v4 measured STOP, Direct V2 1.4M-CU STOP + V3
  model-only, exact-lot bearer redemption, tightened walk language, Clear
  energy fourth machine-checked negative, TFHE candidate-only, operatorless
  + formal-proof boundary sentences). PDFs built, vocabulary audited, zero
  unverified citations. Filing gates (docket recheck, legal review,
  submission) remain human.
- R2 selection landed (765ca81): Pyth pull selected on the bucket-uniqueness
  discriminator; SourceSpec v2 pull-profile deltas, CROSSING_V1 semantics
  with falsifiers, post-2026-08-26 identity-freeze sequencing. PROPOSED/
  MODEL-ONLY; registry stays empty, Endow keeps refusing 0x79.

---

# Historical execution log (2026-08-18 goal)

# Standing goal (2026-08-18, ember on a walk)

> **Historical execution log.** For current capability claims, STOPs, and the
> dependency-ordered queue, use [`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) and
> [`docs/V1_BACKLOG.md`](docs/V1_BACKLOG.md). Entries below record what agents
> believed or measured when written; later implementation and adversarial review
> supersede several “current,” “next,” and completion statements.

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

- THEORY WAVE landed, all six lanes. Headlines: the dual IS the measure
  at deg 0-1 (proved) and refuted at deg>=2 with an executable
  arbitrage; the relation is a disassembled Cert-F checker; the
  constraint matrix is totally unimodular so there is no integrality
  gap; dispersion is NOT the quotient norm (refuted, exact relationship
  given) plus a feeless zero-price laundering vector; verified bytecode
  is the wrong plane (every P0 we ever had would have compiled); and
  lean/ now holds 86 theorems with zero sorry, having found four
  corrections to the design's proof sketch incl. an unstated u128
  bricking hazard whose absence depends on partition of unity.

## NEXT SESSION - start here

1. **solanalib fork scoping** (ember-encouraged): the Solana Foundation
   maintains a Lean 4 sBPF semantics (Apache-2.0) whose refinement layer
   is open and whose validation harness is a dead link. Nobody models
   syscalls anywhere - and our correctness rides on address derivation
   and invoke_signed. Scope: what their tree gives us, what the three
   syscall models must say, whether our byte-exact differential can
   serve as the validator they lack. See docs/research/
   VERIFIED_BYTECODE_PATHS.md.
2. **Aeneas/Charon spike** - Rust to Lean, may remove the
   two-implementation cost entirely. Our kernel is unusually
   Aeneas-friendly (no_std, no unsafe, fixed arrays, checked arith).
3. House rule to add: ban native_decide in our Lean tree (it can
   currently prove False; Lean's compiler is in its own TCB).
4. Ember decisions still queued: filings go/no-gos, policy freezes, the
   fee-base fork, PROJECT.md section 9 vs cross-market netting, the
   single-truth token cutover.

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
