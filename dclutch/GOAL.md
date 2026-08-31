# GOAL — work until 11am: the protocol as good as it can be, all debt burned down

Refreshed 2026-08-31 ~05:1x (ember re-issued /goal; was "excellent and
complete until 10am"). Standing steer: all public drivable, load simulator
on live devnet, copy at the strike-five bar, design at the Linear/Stripe
bar. Full-autonomy directive in force — nothing left to ember. The debt
ledgers this burns: the Night worklist + Queue below (distilled from
ASPIRATION_LEDGER, OMISSION_INDEX, SLIPPED_THROUGH_SWEEP, SPELUNK's 20).

## Current thrust

Night wave, 10 lanes live (IDs in SESSION_STATE.md header):
FINALIZATION (first local fill on 43080), FEE-TX2, FRACCHECK-2, SIMLIFE-3,
EXPLORER, HYGIENE, TRADE-4 (fires the first public trade on market19, then
keeps devnet alive), CLOSESEAL (E3: collector-keeps-capped), GRICE
(strike-five minimalism), plus claims-route map + narration-string sweep.

## Next 3 moves

1. DESIGN lane the moment GRICE lands (ember 02:30: "text is just too
   small / imbalanced / rethink the graphic design and iterate") — modular
   type scale, mono demoted to values-only, balanced grid; details in
   memory `dclutch-web-aliveness-patterns.md` DESIGN STRIKE entry.
2. Harvest each lane report as it lands; route load-bearing facts first
   (route-before-ritual); land follow-up lanes where sized work unblocks.
3. PUBLISH-4 cut once GRICE + DESIGN + TRADE-4 land: minimal copy, the
   redesign, and the traded market ship together; three-layer verify;
   credential sweep first. Then cohort-8 if warranted; morning report
   by 10am.

## Night worklist (from BACKLOG's ledger digest, 03:5x)

Spawned: GEN-SEVEN (General order placement, worktree, L), TRIPWIRE (0017
per-family continuation test, S/M), TICKETCLI-2 (CLI ticket author, S/M),
BASIS-D (option-D wire-free front: Lean owner + corpus + de Boor port,
worktree, M/L), DESIGN (site typography, on GRICE's landed tree).
Ruled tonight (WAVE.md cf04501b): basis authority = option D; recovery =
one-attempt is NOT forever (funded FailNext chartered post-cohort-8).
Dissolved: D2-CONST already on lane/fee-tx2 (FEE-TX2 told it owns landing
the whole fee stack — fee-core's base never reached main).
Also spawned (04:2x, from SPELUNK's 20-drop haul): GATES (public-repo CI
never ran green on main + live-tree tiers + 4 orphaned reds/scripts +
VALIDATION_BACKLOG's demoted-tier references), AOT-MEASURE (the 33.9%
lever, never measured — U-014's first number). Routed: TRADE-4 told only
CategoricalQ1 founds (variety = semantics/buckets/shapes, never basis);
SIMLIFE-3 told to build the spend-based kill before devnet activity.
04:3x: 429 session-limit wave killed 13 lanes mid-flight — ALL resumed
warm per protocol; SIMLIFE-3/HYGIENE survived; both auxiliary sweeps had
finished (claims-route map + narration sweep, exports in scratchpad/*.md).
Queued (in order): RETIRE-1 (first full wind-down — spawns when
FINALIZATION lands the fill; fold in direct_begin_retiring_v1's missing
on-chain test); VERIFY-THEN-DROP ledger amendments; CREATE-WIZARD
(/create, behind DESIGN); KAPPA-CAP (worktree, cohort-8 rider);
SEMANTIC-OWNER; AGENT-ARTIFACTS; GRICE-2 (operator consoles' copy +
narration-sweep findings: scratchpad/narration-sweep-findings-20260831.md
— routeCensus meanings fix upstream in Rust doc comments); P-007 seal
layout emission (BASIS-D's template applies); continuation-test port (~20
tests off the demoted route); driver→kernel found() conversion (3-5h —
HOLD until SIMLIFE-3 lands, shared driver files); cliff-doctrine design
pass (Fable-class); FeeSole frame retirement (after fee stack lands);
codex's 605-line sha256 patch adjudication; sccache/workspace
consolidation (post-wave, REPO-scoped only); risk-roll design lane.
Morning report must include: the "planned but not launched" short list
ember asked for (never delivered) + Helius rotation reminder.
Deferred (do not re-litigate): protocol revenue, fee rates (M-26 ember's),
mainnet, CFTC, assurance park, dead-market deletion, monolith benchmark.

## Done-log (07:4x additions)

- DESIGN-2 done (7 commits): the type system enforced across all 9 pages
  (mono=values, sentences sans ≥13px); THE SVG FIND — viewBox scaled
  chart labels to 3.2px on small panels, now measured-11px everywhere
  (useFigureScale); /population's 237px mobile overflow → 0 on 8 pages ×
  2 widths; nav 200px → 154px; market cards capped 620px; AGPL Source
  footer live site-wide. 32 before/after screenshots in scratchpad/design
  for ember. Its 2 attributed reds fixed by ME (20d2a9d7 — mirrors
  regenerated after the ticket-crate move; 18/18). Copy leaks on
  /population routed to SIMLIFE-3 (register notes render verbatim).
  → PUBLISH-4 SPAWNED: cut tonight's site to clutch.dregg.pro + first
  green public CI. Second cut follows the devnet trade.
  Queued: ChainExplorer.tsx 3 real TS errors under BigInt noise;
  mobile-nav affordance; operator consoles' type rules (GRICE-2).
- ★ THE FIRST FILL EVER (FILL-2, ~08:5x): sig 4hse1dNh… slot 7576,
  1,282,624 CU, conservation net EXACTLY 0, fee accrued as fee_owed
  500,000 on the maker root (the two-tx design proven on a fill); seal
  fix VERIFIED on chain (739,722 CU where 0x4008 died); walls 3/7-half/10
  down (wall 10 found BY the fill: producer said payer, chain says
  RentCredit refund wallet — fixed). Substrate preserved at
  ~/jobs/dclutch-fill2 (RPC 42888; SIGSTOPped PID 5377 watchdog hazard).
  Routed to TRADE-5: use current-main host tooling for the manifest.
  → RETIRE-1 SPAWNED: settle the real fee debt (first tx2 on a live
  fill), resolve, redeem, retire, closing conservation table +
  direct_begin_retiring_v1's first on-chain test.
- Cut wall + review (~08:3x): resolution's bytes are ALREADY the
  candidate (fee stack never touched it; 815,128B chain dump matches
  digest both sides), and the journal walker refuses every role behind
  a receipt-less already-current one. TRADE-5 refused to self-edit the
  checker; orchestrator REVIEWED + APPROVED a narrow AlreadyCurrentV1
  evidence kind (finalized-slot dump digest, red-proof both ways, never
  masquerades as a receipt, re-verified at activation). Custody upgraded
  (2oY5To6x…); ladder resumes.
- COHORT-8 CHAIN PHASE BEGUN (~08:1x): candidate dfb41be6, checked
  release green (ea7df51a…), floor gate GREEN 1,263,176 vs raised pin
  1,264,676 (51-CU red attributed to FEE-TX2's zero-slack test pin,
  control-reproduced, raised itemized per precedent). Custody upgrading
  (role 1/5); then resolution/claims/trading/core → publish → activate →
  refound 50bps → seal (first on-chain DCLTSEL1 test) → THE TRADE.
- GEN-SEVEN-2 done (1efac500/42c0a631/3250af18, all on main, Lean
  127/127, adapter 226/226, zero frame diagnostics): register bank
  widened ONCE for all seven (90→151, 40→45), OpenBatch+CloseBatch fully
  authored with execution theorems + mutation witness, accelerator heap
  un-tipped. Five choices recorded in WAVE (e46afa56). Five actions
  remain, two walls sized → GEN-SEVEN-3 spawned (order-record wire break
  while it's FREE, then the three order actions + escrow legs; candidate
  pair only if night allows).
- LIFT-1312 done (3be5072c): stop-condition fired HONESTLY — the 1,232B
  packet binds below the record bound (Structured K=3 already unissuable
  at 1,357B full-width; the lift would mint never-issuable descriptors).
  Landed: the four-author derivation replacing the bare literal, the
  coordinate cap solved from the formula, wall ordering as a checked
  assertion; release identity UNMOVED. Doctrine corrected (7e563666).
  Queued: session-split Structured issuance (the real K lift); operation
  counts into a Solana-free crate.
- 33-byte seed fixed (8af9e5fb+bd1370ce): worse than named — first
  derivation would have PANICKED, never caught because no_std claims-svm
  can't derive and no route exists yet. Renamed to 27 bytes + const
  asserts over all four claim-check domains (red-then-green) + a
  discriminating derivation test. Baseline hand-shrunk (3 debts paid,
  4 tripwires armed, no defect baselined). census+seam PASS — the public
  CI blocker is cleared; the next subtree cut turns it green.
- CompactIntentV2 red fixed (00bb24e8, TICKETCLI-2's own extraction, not
  pre-existing): --all-targets clean workspace-wide. Lesson ledgered:
  crate-boundary moves need --all-targets (test-only imports of a moved
  type escape lib-level verification).
- FEE STACK ON MAIN: a7d50d3a (fast-forward, 7 fee: commits, both Lean
  gates green, fee codec 183/183, hot fixture 18/18, zero overlap with
  held work). TRADE-5 told: PIN COHORT-8 HERE + the cold-worktree Lean
  gotcha (build CompiledPhysical first). Pre-existing --all-targets red
  (CompactIntentV2, operator lib test) routed to TICKETCLI-2.
- BRANCHLAND done: 50 unmerged branches → 3 (all live lanes). 47 retired
  as already-landed, three-way evidence each, tombstones-first
  (d8b1e95e); basis-d LANDED to main as ffdc63f1 (verified twice across
  a 12-commit main move); drift guard tools/branch-census/census.sh added.
  Host repo: 6 integrate/* branches zero-unique, recorded not pushed;
  local main 1712 stale — its one unlanded commit preserved on
  rescue/site-successor-explainer (old static site, likely superseded).
  The two /private/tmp mystery clones died in the 01:42 reboot (2 codex
  commits unrecoverable — morning note for ember).
- GATES done (9 commits): CI runs on public main FOR THE FIRST TIME
  (checks + rust verified end-to-end). Four gates were failing for
  reasons unrelated to what they gate — SBOM (50 phantom license fails),
  compute-margin (unset ELF var), custody (wrong repo root in subtree),
  suites (3 red rows, ZERO tests executed — per-row absence miscounted as
  protocol failure; rows now say NOT RUN). One real defect isolated:
  FRACTIONAL_CLAIM_CHECK_SEED_V1 = 33 bytes, underivable → FRACCHECK-2
  resumed to shorten it (free now, nothing can depend on it). Publication
  cut checklist grows: the next subtree cut turns public CI green.

- FEE-TX2 done on-branch: THE PAIR EXECUTES — tx1 fee-bearing fill
  1,280,996 CU (32/32, margin 104,003; floor 131 CU BELOW zero-fee), tx2
  169,590. Fee wall dead. Three defects fixed en route incl. FeeSource
  (maker A settling from maker B's delegation). Q1 answered: Trading's
  self-attestation is signing its own caller-authority PDA — derivation,
  not registration. RESUMED to land onto main NOW; TRADE-5 told to hold
  the cohort-8 pin for the landing hash (one cut carries seal fix + fee
  protocol; rate diversity unblocks). Queued: lane E (builders/panel),
  refusal mirrors, the unrunnable run-fee-second-transaction.sh.
- AOT-MEASURE done (b56c10f6..8b47f287 + evidence doc): transition-AOT
  saves 10,393 CU/invocation — 0.83% of floor, 29.5% of the (now-moot)
  fee gap. The "33.9% lever" contained ZERO TransitionVM CU; the real
  interpreter is the effect kernel (164,289 CU, 14x, unmeasured) → that's
  the future AOT charter if CU matters again. Debt: the AOT crate has
  never compiled for SBF (175 errors, 2-line cfg fix applied-not-committed).

## Done-log (07:2x additions)

- FRACCHECK-2 done (f3f47640..1aac6f43, 8 commits): the §17.4 burn leg
  PROVED on real bytes across a real SetAuthority; split-controller
  disjointness non-vacuous (admissions counted); escrow PDA program-
  derived both sides. Size corrected: RetireCoordinate is Trading-composed
  at 1 layer of 3; re-size 17 commits, 9 remain (~3x estimate; the
  48-account frame is a lane alone) → queued as FRACCHECK-3. Its campaign
  caught the /tmp wipe honestly (refused a mismatched litesvm .so,
  rebuilt bit-for-bit against the pinned audit row).

- TRADE-4 closed: NO trade — the deployed Trading ELF omits DCLTSEL1 from
  the heap-profile list (fix 8c216642 NOT ancestor of deployed a93256c1),
  so every seal write refuses 0x4008 on chain; host-side fix verified
  (grant ships), program-side needs a cut. ALSO: zero-fee markets can
  NEVER trade (direct_token_setup_v1 admits only 50bps — market19
  permanently unfillable; founding help was backwards, killed 3 markets;
  fixed 892aaa39/37fc1b91). Built market20 (DQd8WmU2…3rGW, 50bps, 7
  stages, frozen lookup). Spend 0.2366 SOL, deployer untouched.
- ORCHESTRATOR DECISION under the directive: CUT COHORT-8 NOW → TRADE-5
  spawned as sole devnet writer: pin candidate, floor-gate 32 seeds,
  publish/activate, refound 50bps diverse markets, seal write (verifies
  the fix on chain), manifest, FIRE THE FIRST TRADE, then close cohort-6/7
  stranded seals (CloseSeal rides this cut — first close ever).
  market19/20 become compost by standing stranding ruling.
- Queued: panel shows allowance as ceiling where chain demands equality
  (WALL4-species, TS side — after DESIGN-2 lands); GEN-SEVEN-2 job-dir
  hazard flagged by TRADE-4 (already warned).

## Done-log (07:0x additions)

- TICKETCLI-2 done (8e5c6979, b729d592): author descended into new crate
  dclutch-direct-ticket (3 real deps, signers behind `author` feature —
  operator links nothing that can sign); `dclutch ticket author`/`verify`
  work in the dist binary, sha256-identical to the TS vector incl.
  signature; refusals proven through the binary, none leave a ticket on
  disk. Queue: dawn dist cut v0.1.0-devnet.3 (CLI lockfile already
  rippled; dist-workspace needs no change). Named debt: third copy of the
  duplicate-key JSON reader wants a small owner crate.

- BASIS-D done: de Boor port landed on lane/basis-d-20260831 (aac98afd,
  wire-free, 19/28 Lean spline cases exact, 6-of-9 mutation red-proof;
  items 1-2 + kind-tag guard were already landed — verified, not redone).
  RULED (WAVE 76e2ca3f): spline rounding = cumulative-floor (zero-weight
  claims never take residue; measured superior 11 cases). Branch handed to
  BRANCHLAND to land when main's Cargo.lock quiets. Remaining sized:
  overflow envelope (~half day), alloc-free record path, both ride the
  future wire commit.
- /private/tmp wipe warning routed to GEN-SEVEN-2 + FILL-2 (persistent
  paths; BASIS-D lost its worktree to the reboot and rebuilt from git).

## Done-log (06:4x additions)

- CLIFF landed 6cb1269b: every fixed bound classified physics/purchasable/
  session-splittable; lift list ranked. Headline: finalizedRecordMaxBytes
  = 1312, a rationale-free Lean literal below its own account ceiling,
  generates Structured≤3/Rational≤3 and the 42-instruction cap. → LIFT-1312
  spawned (derive the value, regen 4 crates, red-then-green the cliff,
  measure). Also: MAX_OUTCOMES has FOUR authors (unify before widening);
  commit-don't-inline shortlist recorded in doc §4.
- memguard removed at ember's request (policy answer: macOS has no
  kill-don't-thrash knob; vm_compressor=2 boot-arg is the real option,
  needs recovery-mode security downgrade — ember's call, steps on ask).
- TRIPWIRE final: 09c1c8fc/46083e7a covered 2 families on the demoted
  route only — its Core founding-continuation work was NOT a duplicate.
  13-vs-14 resolved: docs were right when written (9c25e741 moved
  founding_v5 to bump-witness); count now carries its re-measuring command.

## Done-log (06:1x additions)

- EMBER STEER (in force): converge forward, integrate towards main, no
  drift, keep main current all night — no "sin-pleasing"/honesty-chasing
  meta-work. Recorded in route-before-ritual memory.
- LEDGER-TRUE landed bc1af4ad: CloseSeal(f253c4e0)/0017-B/tripwire/web-
  bucket CLOSED; fee band + E5 confirmed branch-only (SESSION_STATE
  corrected aea44159); family root-tails NOT landed (wave claim false);
  GEN-SEVEN consequence corrected (7 ACTIONS x 9 records, 68-record
  publication, nothing deployed → cheapest now — routed to GEN-SEVEN-2).
- TRIPWIRE landed 66f95de5: founding-continuation invoke-depth tripwire
  (Core's first dynamic coverage, red-proved via restored CPI →
  ReentrancyNotAllowed) + S-3 case with attribution control; found the
  vacancy predicate guards TWO places (2,589 vs 20,420 CU). Claims'
  13-site helper + Dealer/Rent sized and left (0017 §9).
- FINALIZATION closed WITHOUT a fill: crash wiped validator 43080 + all
  preserved artifacts (evidence survives in DIRECT_FILL_WALLS_2026_08_31).
  Landed 9c386c57..88a4e9c5: 23 named refusal variants (was 1 for 27
  sites), wall 6 fixed+verified, wall 8 fixed UNVERIFIED. Convicted:
  delegated_amount must EQUAL debit (single-use). Wall 7 debt: producer
  admits only vacant seller token → no market trades twice. Both routed to
  TRADE-4 (its trade + second fill depend on them). → FILL-2 spawned
  (restage, fix wall 7 + probe delegation, land the first local fill).
- BRANCHLAND spawned: adjudicate ~20 unmerged branches (land/retire-with-
  tombstone/hold), per the no-drift steer.

## Done-log (05:4x additions)

- THE MACHINE CRASHED (Chrome OOM → swap-lock; ember rebooted, went to bed).
  All 15 lanes died; 13 resumed warm from transcripts, SIMLIFE-3 restarted
  its world run, DESIGN's transcript was LOST — respawned as DESIGN-2
  inheriting its landed commit ab1e9c36 + uncommitted WIP (globals.css,
  layout.tsx, SiteFooter.tsx). Heavy builders throttled (nice, -j4).
- memguard installed at ember's request: ~/bin/memguard +
  ~/Library/LaunchAgents/com.ember.memguard.plist (running, pid-verified,
  victim selection dry-run matches live renderers). Kills largest Chrome
  renderer at 15s sustained critical pressure; never anything else; logs to
  ~/Library/Logs/memguard.log; removal one-liner in the script header.
- FINALIZATION told to verify validator 43080 survived the reboot before
  building on its substrate (likely did not — restage path named).

## Done-log (05:0x additions)

- HYGIENE closed 8/8 (abi:general-v5 was TWO defects incl. a TS requireZero
  that refused legal mined bumps; twins gate generalized to 130 pairs; SBOM
  red retired; 264.6 GiB reaped). New class finding queued: CEILINGS — the
  hand-named refusal-band ceiling stands in NINE other programs. lane.sh
  gap: cannot express a rename. New rule: diff each named path immediately
  before committing (named paths necessary, not sufficient).
- Killed 6 orphaned buffer-writer lease shells (62h, PPID 1, test debris);
  bounded the wait at its source (upgrade.rs, 900s → loud refusal, 6459c025).
- GEN-SEVEN refused correctly with a byte-verified sizing: the seven General
  actions are Lean-gated quadruples; bank widening re-digests all seven
  deployed settlement artifacts (byte 12). → GEN-SEVEN-2 spawned on the
  strongest model to author them (worktree; re-activation rides next cohort).
- CLOSESEAL landed E3 as code (60a21da6): stranger closes stranded seal,
  keeps rent, 5 real-ELF cases, S-3 tripwire, hostile codes 0x4009-0x400B.
  FOUND+FIXED: seal-WRITE outer dead since 08-30 (0x4008 unconditional,
  DCLTSEL1 never heap-declared) — hypothesis routed to TRADE-4 (may be its
  devnet refusal; remedy = cohort-8 cut, within its authority). First
  cohort-6 seal close queued to TRADE-4 via blocked.json.
- Fixture-mapper harvested → TRIPWIRE (13 sites not 14; founding test fills
  CORE's hole, not the shared helper's; waist lacks Rent/Resolution).
- Queue additions: CEILINGS (after FEE-TX2/FINALIZATION land — trading files);
  fee-core worktree reap (14.7GB, FEE-TX2's word at landing); lane.sh rename
  support; 2 mystery clones (8 commits, codex remote) for ember's morning.

## Done-log

- 02:00 goal adopted; wave already saturated (10 lanes + 2 sweeps), no idle
  capacity to fill without colliding in the shared tree.
- 02:30 ember design strike on /markets (type scale/imbalance) — recorded in
  memory, GRICE told to land fast, DESIGN lane queued on its tree.
- 03:05 ember steer: interesting/granular markets — SOL/USD always resolves
  into the same bucket. Rule routed to TRADE-4 + SIMLIFE-3: buckets centered
  on spot at founding, width ~ vol x window (genuine ex-ante uncertainty);
  vary question types; simulator treats one-bucket dominance as a bug.
- 02:50 EXPLORER landed (13d9359c): 53 summaries + 21 notes rewritten, 9
  notes deleted, 3 test pins renegotiated stricter, 177 in-scope green.
  Routed: HYGIENE's f346ba81 broad add swept EXPLORER's file — warned.
  Known: 10 web test files red from other lanes' in-flight strings
  (mostly GRICE's surfaces); full-suite green is PUBLISH-4's gate.
