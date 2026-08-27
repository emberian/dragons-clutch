# WAVE — living swarmcycle state

Updated: 2026-08-26 (cycle 1 launch). This file exists so a usage cutoff never
costs orientation: read it first, then `AGENTS.md`, `PROJECT_METHOD.md`,
`docs/OMISSION_INDEX.md`. It records the current cycle, active lanes, and
gates. It is not release evidence.

## Standing decisions (2026-08-26, with ember)

- Local-first for several cycles. **Devnet deploy-and-recycle is deferred**
  (45 devnet SOL banked; seven-role successor ≈ 29 SOL rent; explicit named
  authorization required before any deploy).
- **Assurance work is parked** beyond keeping every claim fail-closed and
  honestly labeled. Finish and polish first; iterate on assurance in public
  from a complete basis.
- Frontend/demo excellence is first-class: browser-wallet support
  (Wallet Standard; verify Talisman et al.), transaction-complete workflows,
  and demo-quality Products for the eventual devnet/Pages demo.
- hbox (stronger; `/tank` has space; always `swarm-build`, co-tenant with
  codex/datacake) and persvati are build/execution nodes.
- Deliberate LINGER between swarm elements: the orchestrator reviews the tree
  and lane reports before launching the next element. No blind chaining.

## Cycle plan

1. **Cycle 1 — Stabilize + Unblock (ACTIVE)**
   - S1 stabilize: commit stranded coherent cuts from the 2026-08-26 cutoff
     (98 dirty files, 3 stashes), fix the `series_v2` test seam, satellite
     workspace convergence audit.
   - L3 frontend: wallet-standard integration, stale test copy fixes,
     `/markets` discovery route per the recovered product-flow brief.
   - Then, after linger/review — W1: local bootstrap through atomic
     Lock→Found→Realize→Claims→**Open-last** (first locally open market).
     W2: common-Hot CU fast path, ~2.87M → under the 1.4M ceiling,
     structural reduction only.
2. **Cycle 2 — Family wave** (unlocked by W1+W2): Direct, General, Dealer,
   Series, Claims→Custody→Token-2022, Source closure, Rational, Fractional
   campaigns; **Structured V2 implementation** (the one large functional gap).
3. **Cycle 3 — Product & demo**: create wizard, portfolio, market discovery
   detail, demo-scenario Products (Pyth range/tail protection, Dealer pool,
   recurring Series), product-first site, docs truth pass (READMEs and
   OMISSION_INDEX currently lag the code).

## The two waists (why this ordering)

- **W1 real creation**: Registry publication → RentV2 → Found31 → atomic
  Claims/Custody → Open-last. `60d4562` landed the atomic generic founding
  route (`DCLTGMF1`, 8-byte outer, four readonly request accounts); the
  local bootstrap campaign was mid-run at the cutoff and has never completed
  through Open.
- **W2 real execution**: common Hot measured ~2.87M CU + heap exhaustion at
  the 1.4M ceiling; profile splits ~1.18M root/Product authentication +
  ~1.24M artifact authentication before the transition executes.
  `310d018` (immutable-Registry fast path) is the in-flight fix direction.
  Ten `hot_cu_checkpoint!` phases exist in
  `programs/dclutch-trading-sbf/src/hot_v3.rs` for remeasurement.

## Active lanes

| Lane | Scope | State |
|---|---|---|
| W2b hot-heap | `hot_v3.rs` heap wall + AccountObservationV1 shape | active |
| W1b foundability | founding-root ADR + projected-Custody wiring + ELF fast-path adoption + campaign rerun | active |
| ST structured-v2 | Lean/kernel/operator, new files | active |
| LB liability-basis-v2 | ramp/complement theorem + kernel + corpus | active |
| RL checked release | release-tool pipeline + candidate + rerun script | active |
| reviewer | batched Opus review of the four Sonnet outputs | active |

Cross-lane board: /private/tmp/dclutch-wave-board.md (append-only, not authority).

## Cycle-1 results (LINGER₂ snapshot, 2026-08-26 evening)

- **W2 (CU)**: pre-transition Hot 2,949,172 → 831,953 CU (−71.8%); compute is no
  longer the Hot blocker; the 32KB heap wall at phase 4/10 is (W2b active).
  Fast path = immutability-argument ELF/record authentication, strengthened
  never weakened; commits 48ece27 + 76279bd. cc228cd had silently broken every
  Profile14 emission — fixed producer-side.
- **W1 (founding)**: gate honestly NOT met — three measured blockers, all owned
  by W1b: (A) founding capability root is circular (root creation needs an
  existing Market; DCLTGMF1 Locks before Found) — needs ADR 0004; (B) the Lock
  stage consumes projected-Custody state no live route can create
  (projected_custody_composition_v4 is undispatched dead code); (C) on-chain
  release auth hashes whole ELFs — Core's 1.0MB twice per Found31 (~1.19M CU),
  and five-role activation with real artifacts exceeds 1.4M outright; the
  existing immutable fast path must be adopted at registry-sbf/src/lib.rs:367
  and core-sbf/src/infrastructure.rs:314. Also fixed en route: real System
  Program metadata acceptance (c25de27), the capability-root selection SHA-256
  fixed point (386f254), and Found31 was 10 bytes over the legacy packet limit
  and now rides a finalized ALT as v0 (4e1c4db). Evidence:
  docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md. A real demo-market
  run-spec subcommand exists (SOL/USD range protection, synthetic-local Pyth).
- **Frontend**: Wallet Standard discovery (Talisman confirmed conformant),
  /markets, /markets/:address, /portfolio (indexer-free Position derivation);
  web suite 126 → 200 passing; all six abi:verify green; the shared DCLTCAP1
  decoder now enforces the FundingQuoteV1 grammar (browser refuses what the
  chain refuses).
- **Workspace**: satellites folded; exclusions down to general-sbf (missing
  GENERAL_ROOT_PDA_DOMAIN_V2 protocol fact) and series-shadow-sbf (subtractive
  feature breaks additive-feature contract; fix = extract the shadow
  authenticator crate or invert the feature). SDK aligned on the
  program-test 4.3.0-beta.2 line (bd4d85d) — bump the whole tree together when
  4.3.0 goes stable.
- Stashes: the two superseded hot_v3 stashes dropped (patches archived under
  /private/tmp/w2-lane/); stash@{0} wip-source-borrowed-view remains,
  uninspected, unowned.

## Cycle-2 sequencing (revised at LINGER₂)

Tranche A (Direct, Claims→Custody→Token-2022 terminal, Series chains) launches
when W2b lands the heap gate — family ProgramTest campaigns do NOT wait for the
open market. The open-market path (W1b) runs in parallel and gates only the
live-chain demo substrate + creation evidence. Queue additionally: founding
root ADR fallout, coreFound manifest-validator convergence, Lean emission of
POSITION_PDA_DOMAIN + DCLTCAP1/DCLTFQ01 to the web ABI, funding-STATE
(DCLTCFS1) browser decoder, general-sbf's missing protocol fact, the
series-shadow feature refactor, fixtures:verify provenance drift.

Lane protocol: **commit with `git commit --only -- <paths>` exclusively** —
staged-list inspection alone has a proven race (two collisions on 2026-08-26);
never `git add -A`; never `git stash`; report incoherent WIP rather than
deleting or "fixing" it. Unfiltered `-p <crate>` test suites are forbidden.

## Cycle-1 log

- S1 + L3 complete (2026-08-26). Root workspace gate fully clean; web suite
  171 passed; `/markets` route exists; Wallet Standard discovery landed
  (Talisman ships Solana wallet-standard support since v3.0.0). The
  contaminated commit was re-split at LINGER₁: the gen-2 deletion sweep is now
  its own commit ("sbf: delete superseded bearer and structured authority
  paths", −16,662 lines). The `series_v2` seam fix revealed 54 Series unit
  tests that had never compiled; they pass.
- Stashes: `stash@{0}`/`stash@{1}` touch `hot_v3.rs` (W2 inventories them;
  `{1}` is substantive dynamic-span WIP); `stash@{2}` is source-contract WIP,
  unowned.

## Queue (next linger points) — reconciled by GIT-SCAN 2026-08-27

- **LINGER₂ Sonnet batch — DONE.** Satellites folded (root `exclude` is now
  empty; nested program-test harness workspaces stay per 5c663da precedent);
  both skeleton dirs deleted; `dealer_chain` warnings cleared (21df8e5).
  The series-sbf lock item is OBSOLETE — the program was banished.
- **Frontend ABI convergence — DONE** (d2f2e60, 48ece27, 4478897, c25de02,
  127c5a4, 413c3db; every `abi:*:verify` plus `fixtures:verify` green since
  839edc8/822e5da).
- **Cycle-2 General charter item — OBSOLETE**: 5b19626 ruled
  `GENERAL_ROOT_PDA_DOMAIN_V2` must NOT exist (decision 0003) and deleted
  general-sbf. The real remaining General work is the next-dispatch queue
  below (eighth set entry, exactly-seven relaxation, GEN-HOT, DCLTCPR1
  encoder).
- **Cycle-3 pull-forwards**: `/markets/:address` detail (73da1ab) and
  `/portfolio` (fbb926b) are DONE. Remaining: the wizard (compose
  `/product-v2` → `/found`), and the missing market *indexer* — still the one
  honest discovery gap.

## GIT-SCAN still-open ledger (2026-08-27 — named in commits, carried nowhere)

Sweep of all 1,509 commit messages; each item below was promised/deferred in a
commit and is NOT covered by any queue above or by blocked.json. Ranked by risk
of silent loss.

1. **activation-role-resolution CU budget WILL red-row the next genesis run**
   (9fbbab4; CU_BUDGETS.md "mode caveat"): the Resolution artifact grew 18,944
   bytes at 87e4590 (the funded failure walk — legitimate) and its owner lane
   yielded. Re-pin the row with provenance. Small batch, do before the next
   tier-1 run.
2. **Relayed recovery leg unsupported**: `RecoveryMaterialSlotV1::new` is still
   Pyth-only (source-contract lib.rs:2286), so §4.8's "silent relayer degrades
   to a named alternative source" has no relayed form — it walks straight to
   the failure outcome (425a3c9 §10.5). Relayer/Source lane, or an explicit
   decision that the v1 demo accepts direct-to-failure.
3. **Relayer daemon gaps** (2b920d6): publication-log public push NOT
   implemented (§4.11 unsatisfied, tools/relayer/README.md:211); submission
   never run against any cluster; root-workspace promotion an open decision.
   Plus the carried v0/ALT note for its two oversized transactions.
4. **AOT/interpreter semantic divergence, permanently #[ignore]d**
   (55616a8; direct-aot-v3-contract/src/tests.rs:520): the
   `outcome >= tail_count` guard exists only in hand-written Rust, absent from
   Lean-emitted DIRECT_ORDINARY_PRELUDE_V3. Unreachable today. Owner decision:
   add the clause in Lean or delete the Rust guard.
5. **RegisterBuy topology defects, reported-not-fixed** (9b99662):
   `validate_lengths` still pins System at width 0
   (registered_account_artifacts_v4.rs:562) and Exact loader/program widths the
   opaque ruling should cover — refuses on any real validator. Attach to the
   tranche-A Direct family charter.
6. **Dealer equity profile migration** to the FixedDataPredicate profile
   (d64d0c2 "QUEUED, NOT DONE") so its callee can be `opaque(executable)`.
   Attach to tranche-A Dealer.
7. **blocked.json's two UNASSIGNED owner decisions**: RentCreditV1
   Create/Withdraw supersession (delete V1 or state why both survive — O-005
   pressure), and registry/batch_v2 reachability at real ELF sizes. Fable
   wave / ember.
8. **sha2 default-features latent no_std breakage** in 7 on-chain-reachable
   manifests (7123164; verified: general-config-contract,
   rational-representation-v2-lifecycle-contract, registry-svm,
   structured-v2-contract, structured-v2-kernel, token-svm,
   fractional-claim-kernel). Mechanical small batch.
9. **DEVNET_DEMO_DEPLOY.md blocker C is STALE**: the doc says the web
   Core/Registry conflation "is open"; it was removed (3645eed) and exercised
   against a real chain (5129362). Runbook correction — a stale blocker on
   deploy day misdirects.
10. **stash@{0} wip-source-borrowed-view**: still uninspected, unowned
    (verified). Inspect, land or drop.
11. **claims-svm test-module clippy debt** (b82feed): product_basis_terminal_v3
    `too_many_arguments`, terminal_settlement_v3 `indexing_slicing` — left for
    owner. Trivial.

Doctrine-debt audit (same scan): 01a2246 annotates its mixed lock hunk in its
own message; ea4954a's carried tier2/README is annotated in cc21a7d; the
46f03df-era contaminated commit never reached main (dangling; re-split as
f26863c, recorded in the Cycle-1 log above). The 35fb8ed→2f55c81
revert/reapply sandwich of 2a35720 (29 seconds, net zero) carries only stock
messages — its explanation lives on the W2c board entry; recorded here so the
history stays legible after the board expires. 15ac612's revert of the
demo-cut WAVE edit is explained by the no-scope-cut direction above.

## The demo is the completed dClutch (direction set by ember, 2026-08-26 night)

There is no scope cut. The near-term goal is to FINISH everything intended —
all families, representations, creation, operator/frontend — as fast as the
swarm can converge it: roughly 3–8 more swarmcycles of closing loose ends and
implementation before any "what's in the demo" pruning conversation. Edit-heavy
lanes that converge later are acceptable. Assurance re-enters incrementally
during this phase (we iterate on it in public); it no longer waits for "done."

**The demo shape:** the mostly-completed protocol LIVE ON DEVNET, resolving
markets about the state of SOLANA MAINNET (pumpfun/DBC graduations, mainnet
pool prices, majors). Cross-cluster truth transport:
- v1 accepts a disclosed proof-of-authority relayer as the cost of doing this:
  an off-chain daemon reads finalized mainnet account state and signs
  attestations of RAW BYTES + slot + owner (+ the owner program's ProgramData
  digest, so the Loopscale-class program-identity defense works cross-cluster).
  The relayer attests OBSERVATIONS, never interpretations — all decoding stays
  in the on-chain adapter under the CHAIN_STATE_SOURCES decoding rules, so
  swapping trust roots never moves semantics.
- Majors' prices need no relayer at all: Pyth's devnet deployment already
  carries mainnet-derived prices under the existing adapter.
- Candidate permissionless upgrade to verify: Wormhole Queries (guardian-signed
  Solana account reads, verifiable against the devnet core bridge) — MR lane
  owns pinning whether this actually supports what we need.
- Later hardening path: multi-relayer quorum, TEE-attested signer.

## Close-out doctrine (ember, 2026-08-27)

1. **Holistic over combinatorial.** The census answered "does each route run at
   all." The next evidence tier is JOURNEYS: whole flows and use cases under
   simulated load (create → many traders → resolve → redeem → retire; replay
   pressure; concurrency), orchestrated at high abstraction. Route-level tiers
   remain as regression floors; new testing effort goes to journeys.
2. **Subagents yield.** A lane collects context, implements, commits, and
   YIELDS. Campaigns, integration, and cross-lane convergence happen at the
   orchestrator or in a dedicated integrator lane. No do-everything marathons.
3. **Commit early and often.** A commit does not need to be a tested or
   integrated unit. Git is the safety net; use it liberally.
4. **No more re-measuring into tables.** Record verdicts and deltas; a number
   is written down only when it is load-bearing for a decision.
5. **The purge**: reference/stale/superseded code is banished to
   ~/dev/dclutch-legacy (copy for grep convenience; git history is the
   authoritative record). Banished so far: the gen-2 monolith
   (programs/dclutch-sbf), series-sbf, effect-sbf, economic-sbf,
   product-payoff-sbf, product-evidence-sbf. In progress (integrator lane):
   the verticals live/dead split, dealer/general contracts, the remaining
   gen-2 cascade, and the census denominator correction. The repo's contents
   converge to ONLY the active built system.

## MILESTONE: THE MARKET IS OPEN (2026-08-27, run 6, 67e441d)

DCLTGMF1 executes end-to-end on a real validator: 1,189,823 CU, reproduced
three times, whole-chain rollback hostile case green, gauntlet 23/23 witnesses,
42/~119 routes executed. Eight founding blockers found and killed across six
runs. AbortSourceAndClose landed + executed (was a stranded-collateral hazard).
Remaining to the joined trading gate: hot tail heap (2,383 over at phase 7,
tail >=39,521 vs 32,768) + first-ever phase-8+ child CPI territory (W2i lane).
URGENT unowned->now owned: DCLTGMF1 CU grew 84.6%->91.3% of ceiling in one
evening from unrelated changes; CU-BUDGET lane adds checked-in budgets.
The monolith is fully deleted (legacy copy removed; git history only).

## Post-cook plan (ember, 2026-08-27 — after current lanes land)

- Devnet budget is now 55 SOL (was 45).
- 1. ADDRESS every item the returning lanes name (standing rule: reports'
  named items get actioned, not archived).
- 2. THE FABLE REVIEW WAVE: Fable-tier reviewers over the scar tissue —
  derpage hunt, "omg I can't believe we're doing that" audit, cross-cutting
  design coherence — after this swarmcycle + followups cook.
- 3. GIT-MESSAGE ACTION SCAN: sweep all commit messages for named-but-unactioned
  items (queued/flagged/deferred/not-done claims) and cross-check each against
  reality; everything named gets actioned or explicitly retired.
- 4. THE PUBLIC FACE (audited 2026-08-27, all "no"): Pages currently ships the
  gen-1 microsite verbatim (manual dispatch, honestly labeled). Needed:
  protocol reference manual, user guide, trader guide, operator guide, Pages
  building + shipping the real frontend, and the dragons-clutch wrapper tidy
  (= the graft: dclutch becomes the current tree; gen-1 handoff docs and
  stale site retired to history).

## Doctrine amendment (ember, 2026-08-27): CUT THE KNOT

Naming a blocker is not a deliverable; it was triage for an era of silent
walls, and that era is over. Default: CUT — fix it, decide it, delete it, even
across "someone else's" seam; commit; let the swarm heal any breakage (git and
the gates are the safety net). Yield back a blocker ONLY for a genuine
authority decision (trust surface, principal, deploy, scope) — and then as a
question with a recommended answer, not an inventory row.

## Fable-wave agenda seeds (from FD3, 2026-08-27)

- TWO live Market representations on chain: DCLTCAT1 (native Realm/Position
  family, own Rust fixture generator) alongside DCLTCOR2 Core state — one
  truth or a second authority? Architecture-coherence question for the Fable
  reviewers, NOT a quick knife.
- The Hoard vault has no chain-derivable address (namespaced by caller-chosen
  founding context) — frontend refuses-with-reason today; decide whether the
  context belongs in a discoverable record.

- DCLLBX02 (liability-basis V2 route): campaign green + census EXECUTED, but
  its ONLY issuance path (Split) composes an External source compartment that
  Custody refuses by design (84b1426), and nothing on chain finalizes its
  record type (CL's earlier finding). Live route or superseded second truth?
  Fable-wave architecture call; the candidate fix (compose the DCLCUDQ2
  delegated V2 wire in liability_basis_v2.rs) is real protocol work either way.
- Relayer daemon slice note: consumption (1,534 B) and full-body append
  (1,377 B) exceed legacy packets — witnessed by label in the relayed tier;
  the daemon must build v0/ALT for those two when it goes live. The failure
  walk deliberately stays legacy-fitting (991 B) — it must never depend on an
  ALT a silent operator never published.

## Next-dispatch queue (at W2k's yield)

- GEN-ART: public encode modules for the three artifact generations
  (AccountProfileV1 / transition ProgramV2 / EffectProgramV2) in their owning
  crates (effect-kernel shared with W2k — hence queued, not parallel), then
  General's own activation artifacts from GEN-V3ACT-r's board design, then the
  zombie refusal EXECUTED through the real runtime path (reachable as a
  refusal even before phase-8 success). build_general_hot_instruction_v3
  finally gets its caller.
- Small batch: core-sbf tests.rs:141 packet claim (13 accounts narrow);
  resume-validator.sh unsupervised exec; family runners' shared ledger.json
  defaults; --keep-elf stale diagnostics re-stamp.

- Dead-vocabulary web tests (SN4): ECONOMIC_* width tests assert a banished
  wire schema; PRODUCT_EVALUATOR/ADMISSION account counts describe
  instructions NO program implements. Delete-or-succeed decisions for the
  Fable wave (what else in apps/dclutch-web speaks to programs that no longer
  exist?).

- GEN-HOT (a lane, not a follow-up, per GEN-ART): the General Hot bundle in
  trading's program-test (~4,000-line analog of direct-hot) — two-thirds of
  its inputs now exist; it carries the zombie refusal THROUGH the ELF and,
  with W2's walls down, General's first hot execution.
- The eighth CapabilityProgramSetV2 entry + relaxing the exactly-seven rule:
  what stands between "General's artifacts exist" and "a live General release
  activates".
- CapabilityProgramV1 (DCLTCPR1) has NO public encoder — hand-written in five
  places, two in trading-sbf (same defect class GEN-ART just cured for three
  generations; fourth generation, trading-sbf owner).
- Small batch: GENERAL_ACTIVATION_ACCOUNT_PROFILE_ID_V1 is a test-demo digest
  with zero consumers whose name now collides with the real thing.

## Cook summary (2026-08-27, the wall series)

Twelve walls found by execution, eleven down: heap phases 4→7→8 (arenas,
borrowed identities, the seal), the reentrancy (cache-read auth, one shared
crate, five families), the receipt-append (typed ReceiptDeliveryV3 split; the
unread append was corrupting five Claims digests), the n²-resolution CU pass
(−343k), the walk-sharing (−78k), the commit bitset (−116k, test-first).
Standing at W2n: the Realm address-vs-digest emitter defect
(ordinary_account_artifacts_v3:571, identity regen #7), 2,747 B of heap
(p7e-banks +5,166 / runtime-observations +7,440), and residual CU (1,378,546
spent at the Custody refusal). The bundle now executes Claims TO SUCCESS and
enters Custody's body. Census 56+/121; journey CONSERVED at N=4 and N=16;
General root real; three artifact generations have one encoder each; the
funded failure walk executes; terminal windows have width and a one-answer
proof. Formatting: use `rustup run 1.97.1 rustfmt --edition 2024` — bare
rustfmt is unpinned and reflows ~178 lines of hot_v3.
