# WAVE — living swarmcycle state

Updated: 2026-08-26 (cycle 1 launch). This file exists so a usage cutoff never
costs orientation: read it first, then `AGENTS.md`, `PROJECT_METHOD.md`,
`docs/OMISSION_INDEX.md`. It records the current cycle, active lanes, and
gates. It is not release evidence.

`docs/INTENT.md` is the other half of that orientation and reads before all of
them: this file is what execution has found, INTENT is what the project is
*for*, in ember's own recoverable words. It is a DRAFT FOR EMBER'S EDIT.

## Standing decisions (2026-08-26, with ember)

- Local-first for several cycles. **Devnet deploy-and-recycle is deferred**
  (45 devnet SOL banked; seven-role successor ≈ 29 SOL rent; explicit named
  authorization required before any deploy).
- **Assurance work is parked** beyond keeping every claim honestly labeled:
  what is unproven says so, in the surface that shows it. Finish and polish
  first; iterate on assurance in public from a complete basis.
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
| W2b hot-heap | `hot_v3.rs` heap wall + AccountObservationV1 shape | **landed 2026-08-27 (W2p): 42,784 → 29,895 bytes, gate 15/15** |
| W1b foundability | founding-root ADR + projected-Custody wiring + ELF fast-path adoption + campaign rerun | active |
| ST structured-v2 | Lean/kernel/operator, new files | active |
| LB liability-basis-v2 | ramp/complement theorem + kernel + corpus | active |
| RL checked release | release-tool pipeline + candidate + rerun script | active |
| reviewer | batched Opus review of the four Sonnet outputs | active |
| VX vinext-upgrade | unblock client-side `<Link>`: vinext beta.8 + `@vitejs/plugin-rsc` + rolldown, together | **queued** |

Cross-lane board: /private/tmp/dclutch-wave-board.md (append-only, not authority).

### VX — the vinext upgrade, queued not taken (2026-08-27, NAV-FIX)

**What is broken.** vinext 1.0.0-beta.3's `next/link` shim is inert in every
production bundle. Its click handler calls `preventDefault()` and then awaits
`import("./navigation.js")` for `navigateClientSide`; in the built bundle that
specifier resolves to an export-less namespace, the prefetch throws, and the
click is swallowed with no navigation. Measured under `vinext start` and
against the Pages artifact alike, so it is the shim and not the host. Typed
URLs and plain `<a>` were unaffected throughout — which is how it was isolated,
and what the fix rests on.

**What was done instead, and why it is not a workaround.** The app ships as a
static export: every route is a separate prerendered document that reads
whatever chain the viewer points it at, and no route carries state another
route needs. A full page load *is* the navigation model here. So the app
navigates through plain anchors (`apps/dclutch-web/components/Anchor.tsx`), and
`next/link` appears nowhere in it. That is correct on its own terms and would
be worth keeping even with a working shim; this lane is not blocked on VX.

**What VX is for.** Three packages have to move together and none of them moves
alone: beta.8 does not drop in — it requires `@vitejs/plugin-rsc` >= 0.5.34,
which then breaks `rolldown:vite-resolve`. Take it as one coordination, with a
measurement of what the upgrade actually buys before adopting `<Link>` anywhere:
today the answer is nothing the export needs.

**What holds the line meanwhile.** `tools/gauntlet/frontend/pages-nav-check.mjs`
clicks every link in the assembled artifact in a real browser. The static link
checker in `tools/genref/render-site.mjs` proves an href RESOLVES; only this
proves clicking it navigates, which is the exact gap the live breakage fell
through.

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

**THE HEAP GATE LANDED (W2p, 2026-08-27, commits f6e41b0…4a711e5), so Tranche A
is unblocked.** The canonical Direct continuation executes to completion at the
real 32,768-byte heap: peak 42,784 → 29,895, `registry_hot_continuation` 15/15
across three runs (from 12/3), `late_custody_refusal` reaching its named depth.
The bump allocator grew a second, downward end and the short-lived banks live
there. **The wall that replaces it is COMPUTE**: the shipped path spends
1,336,865–1,386,359 CU depending on the PDA bump-search depth for the keys in
play, and one draw in twenty (fixture seed 10) exceeds 1,400,000 outright. That
is a protocol cost, not measurement noise, and the recommended fix — store each
canonical bump in the record it belongs to, as the capability seal already does
for the sealed roles — is an authority decision on record layout. See the board
for the phase-level attribution.

**W2q ruled and executed that fix (2026-08-27, commits 569b582…34bebe8), and it
is NOT enough on its own.** The three Market-selected record coordinates — the
manifest, the program set and the config — are now READ from bumps the
activation stored, never searched for: four in `CapabilityRootHeaderV1`'s
reserved word, two in the embedded selection's, so no offset moved, no width
changed and nothing regenerated. `process_activation` is the sole writer, a
selection on the wire must still carry none, and `create_program_address` under
the stored bump is the check. Measured: `artifacts-strategy-effect` spread
across fixture seeds is now **ZERO** (W2p measured 21,000 between seeds 1 and
10), the phase costs 82,294 → 76,426 CU, and **seed 10 — W2p's failing draw —
now lands at 1,377,761 from 1,401,761**, under the ceiling by this change alone.
The 20-seed sweep goes 17/20 → 18/20 (min 1,343,261, max 1,388,260); every seed
is strictly cheaper.

**Seeds 1 and 7 still exceed 1,400,000, and the wall has moved to a phase whose
searches CANNOT be stored.** `commit-lifecycle-closes` carries 24,001 CU of
cross-seed spread (16 bump attempts) — four times the +6,002 W2p reported, which
was a two-seed sample of a random variable, not a bound. Those are the child
CPIs' caller-authority PDAs, seeded by per-execution request digests; there is
no record to carry a bump for a coordinate that exists only for the length of
one instruction. `root-product` holds 6,000 more and `request-lifecycle-preplan`
4,500. **The next lever is not a bump: it is fewer or cheaper child authorities**
(or a wire-carried bump the callee's own canonical derivation checks, which
moves the cost rather than removing it). DECOMP owns the residual.

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

- **OPEN PROTOCOL DECISION — the published/selected capability release is ONE
  gap, not two** (GEN-SER, 2026-08-29; accepted open, not ruled mid-wave).
  Neither General nor Series reaches Trading's commit half, and the reason is
  the same for both. **There is no missing Trading dispatch site**: `hot_v3`
  is family-neutral, already dispatched at `trading-sbf/src/lib.rs:454`, and
  walks whatever child routes the *selected* EffectProgram declares by
  `FixedRole`. No family has an arm there and none needs one. The
  ~20 `series-family` cfg blocks inside `hot_v3` are those family-neutral role
  routes, gated so a `--features series-family` build still compiles the roles
  a Series effect declares — they are not Series dispatch, and reading them as
  such sends you to build the wrong thing.
  `stage_series_consume_execution_v3` (`series/execute_v3.rs:211`) has zero
  callers because it belongs to the superseded adapter design.
  What is actually missing is a release **published and selected**: the
  artifacts finalized as Registry records, named by a `CapabilityProgramSetV2`,
  and chosen by a founded Market's capability manifest. Only Direct has that
  pipeline (`successor/src/{market,direct_market}.rs`, a whole records
  compiler); `plan_general_capability_activation_v3` has test callers only.
  Sizing it as one shared piece of infrastructure rather than two family
  efforts is the point of this entry — GEN-SER's round-2 General campaign runs
  against the accelerator ELF for exactly this reason, and that is not a
  General-specific debt.
- **OPEN PROTOCOL DECISION — Series declares no `StateLifecyclePolicyV5`**
  (GEN-SER, 2026-08-29; blocks every Series release). Every other family has
  one (`encode_general_state_lifecycle_v5_atomic`; Direct has two).
  `series/lifecycle.rs` is about FUNDING — `FundingStateV1`, top-ups, refunds
  — not the lifecycle *artifact*. Until this exists there is no admissible
  Series release at all, because
  `authenticate_series_consume_artifacts_v4` decodes the policy, runs
  `validate_account_profile` against the Series Consume Profile13, and
  requires `action_plan_count(Consume)` to be **nonzero** — a policy covering
  only Prepare or Expire decodes, validates, and is still refused. So it is
  not a field that can be left empty and filled in later. Three things need
  deciding, none of them a caller's call:
  (a) **which created states it covers** — Series creates a root *and* a
  Ticket, where General's precedent covers primary+terminal;
  (b) **which rent-quote generation it pins**;
  (c) **who receives the refund** — and note the hazard:
  `series/lifecycle.rs:149 ticket_capability_refund` already suggests the
  Ticket's capability rent is spoken for by the funding path, so a policy that
  also claimed it would be a **second author for one lamport flow**, the exact
  class of bug the escrow work keeps fixing.
  Do not hand-write it against prose. The requirement is derived off the
  verifier, in order, as `SERIES_CONSUME_LIFECYCLE_REQUIREMENTS_V4` in
  `trading-sbf/src/series/release_v4.rs` (8b37cc52) — check the answer against
  that. The assembler in the same file (e4aa2bbd) takes `lifecycle` and
  `strategy` as typed parameters rather than defaulting them, so the moment a
  policy exists the bundle assembles and authenticates with no other change.
  **RULING (ORCH, 2026-08-29 12:30 EDT, ember-endorsed: selection is the
  spine now).** The single-author principle — the day's entire defect
  taxonomy was "two authors for one fact" — governs all three:
  (a) the policy covers the states Series routes CREATE AND OWN: the root.
  The Ticket appears as a referenced coordinate only; its lamport flow is
  authored by the funding path (`ticket_capability_refund`) and the policy
  claiming it must be a PINNED REFUSAL, not merely absent.
  (b) the rent-quote generation pin is DERIVED at emit time from the
  publication context the release set already binds — never supplied, never
  a second copy.
  (c) the refund recipient is a RULE, never an identity: the beneficiary
  fixed at state creation (Dealer checkpoint precedent — every lamport
  reaches the creation-fixed beneficiary).
  Implementations are checked against `SERIES_CONSUME_LIFECYCLE_REQUIREMENTS_V4`,
  not this prose. While in there: fix the naming collision — series/lifecycle.rs
  is about FUNDING, the protocol-wide term means the artifact; a name that
  misleads is the found_request_digest trap waiting for a third lane.
  **RESOLVED (SER-POL, 2026-08-29).** b20256ee (rename: series/commit_plans.rs),
  8f579821 (the Consume bank grows append-only so the Profile13 projects the
  root header's own seven derivation fields — the only honest seed source),
  4f4de38e (`lifecycle_policy_v5.rs`: root-only Authenticate plan, everything
  derived, no lamport authorship), d5a24df2 (the Ticket claim is a PINNED
  REFUSAL: `SeriesArtifactErrorV4::TicketAuthorship`, alias-resolving, own
  hostile per claiming shape; the spec constant grew to five entries with the
  verifier), b43062fc (`series_consume_selected_release_v4` + DCSRPB04
  publication in the FRAC house shape; `authenticate_series_consume_artifacts_v4`
  ACCEPTS a real assembled bundle for the first time, with the Prepare-only and
  Ticket-claiming negative controls refused at their exact conjuncts). The
  ShadowAot certificate identity remains the release's one typed deployment
  parameter — `dclutch-series-shadow-sbf` builds fail-closed with no generated
  release selected, so no local certificate exists to read; filling it is the
  accelerator-staging work, not a release-compiler gap.
- **RULING — Reaffirm disposition (ORCH, 2026-08-29 ~12:50 EDT): APPROVED AS
  DESIGNED, IMPLEMENTATION DEFERRED to a window when upgrade.rs is quiet.**
  CONV's investigation (board ~13:58 entry) governs: third
  `CheckedDeploymentDispositionV1::Reaffirm` variant, gated on the gate's
  already-authenticated carry-forward closure (upgrade.rs:6101-6122: disposition
  carry-forward AND cohort base AND requires_new_artifact false AND
  changed_inputs empty AND base==candidate digest) as its receipt; journal
  schema v3→v4 explicitly; the NINE ordinal `index < 2` sites become one named
  predicate FIRST as a standalone commit. Rationale: the current refusal forces
  a downgrade-bounce — two real Loader mutations on live programs to satisfy
  bookkeeping — which is strictly worse for safety than accepting the closure.
  Conditions: hostiles are REWRITTEN TO PIN THE NEW BOUNDARY, one per closure
  leg (wrong disposition string / non-base cohort / nonempty changed_inputs /
  digest mismatch → each REFUSED with pinned code); the old blanket-refusal
  hostiles are replaced, not deleted-and-forgotten. Ember veto window open —
  recorded here precisely so it can be overruled before implementation.
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
4. **AOT/interpreter semantic divergence — CLOSED** (73f0793, 20f28e0,
   225af89). The premise was worse than recorded: `DIRECT_ORDINARY_PRELUDE_V3`
   was never Lean-emitted, and the whole V3 TransitionVM line had no Lean
   counterpart. `TransitionVMV3.lean` + `DirectOrdinaryV3.lean` +
   `EmitDirectOrdinaryV3Rust.lean` now author it, gated on byte-identity with
   the 1,616 bytes the hand-written array produced. The `outcome >= tail_count`
   guard is deleted and replaced by a stronger authored clause: the item body
   accumulates each Claims quantity and the epilogue requires the total to equal
   the transferred quantity, so exactly one Product item must carry the traded
   outcome. `policy_fee_bps <= fee_denominator` landed in the same prelude
   (decision: prelude authoritative, `DirectExecutionConfigV1::new` is defence
   in depth). u64::MAX rent principals: bound REFUTED with argument — the
   principal is a fail-closed floor that never sizes a lamport movement, and
   Create pins it to the Rent sysvar minimum by equality; what is owed is a
   composition theorem at `direct/inline.rs:308`, queued below. Identities moved
   once and swept tree-wide; the differential now runs with zero ignored tests.
   **Successor debt CLOSED 2026-08-27 (TR-A-DIR)**: `registered_fill_artifacts_v4.rs`
   is Lean-authored. `DirectRegisteredFillV4.lean` +
   `EmitDirectRegisteredFillV4Rust.lean` emit `generated_registered_fill_v4.rs`,
   gated byte-for-byte on the 2,408 bytes the hand-written array produced, with
   the transcription kept decidable in the module as `transcribedProgram`.
   Strengthened by exactly one clause — `policyFeeBps <= feeDenominator`, the one
   73f0793 landed on ordinary and this program never had; the conservation clause
   is an identity in the fee deltas and bounded nothing.
   `the_transcription_admitted_the_out_of_bound_rate` decides that the shipped
   object admitted it. 73f0793's OTHER clause (the Product-tail Claims total)
   does NOT apply and is recorded as such: this program has no item body, a zero
   item stride, and no per-item quantity. The AOT translation carries the clause
   too, with a hostile-corpus twin verified adversarially (removing it fails the
   differential). **There is now no hand-written V3 program left in the tree.**
5. **RegisterBuy topology defects — CLOSED 2026-08-27 (TR-A-DIR).** The System
   Program's width-0 pin is gone and its rule is `opaque(executable)`; the
   Custody `TokenMint`/`TokenAccount`/`TokenProgram` data kinds are opaque across
   all three frames, and the source-account nonzero pin went with them.
   `chain_owned_record_widths_do_not_change_profile_identity` emits the baseline
   bytes against 21- and 14-byte System records, fixed-loader Rent and Custody
   programs, a 278-byte Token-2022 mint, a 170-byte ImmutableOwner source and a
   1 MiB token-program ELF. Coordinates 15/16 STAY pinned to
   `LOADER_V3_PROGRAM_BYTES` deliberately: checked-release requires those two to
   be Loader-v3 records exactly, as the ordinary profile also pins.
   **Found doing it**: the registration Transition's two `identity_eq`
   instructions compared the maker's SIGNED rent-credit keys against identity
   registers the AccountProfile NEVER WROTE — an equality against zero that the
   request decoder's own nonzero check then made unsatisfiable. Invisible because
   no registered creation has ever executed on a chain. The profile now projects
   the sole credit and the Rent program, and both signed fields bind to the
   credit. **Still open, named not fixed**: coordinate 9 is a second
   self-representative SIGNER whose own doc calls it an alias — 52f14fa found
   exactly this on ordinary and ruled two signers do not fit the continuation
   packet. The registered packet has not been measured.
6. **Dealer equity profile migration** to the FixedDataPredicate profile
   (d64d0c2 "QUEUED, NOT DONE") so its callee can be `opaque(executable)`.
   Attach to tranche-A Dealer.
7. **blocked.json's two UNASSIGNED owner decisions — CLOSED 2026-08-27
   (DELDEC), both answered "delete".** RentCreditV1 Create/Withdraw are gone
   (`LifecycleRentCreditV2` is the exercised path; OMISSION_INDEX P-005 lifted),
   and registry/batch_v2's standalone DCLTRGB2 route is gone (the five per-role
   activation pins sum to 2,407,858 CU against a 1,400,000 ceiling, so a batch
   was never executable; its read-only `authenticate_request` stays, live under
   both continuation routes). blocked.json now carries no UNASSIGNED row at all:
   44 rows → 41, census 101 routes → 98, refusal codes unchanged at 193.
   **Carried row CLOSED 2026-08-27 (TR-A-DIR).** Registered artifact
   coordinates 7 and 10 are now a 128-byte `LifecycleRentCreditV2` and the Rent
   program that owns it, and `RentCreditV1` is deleted tree-wide with its width,
   PDA domain, magic, schema version, field offsets, seed projection and the
   `CreditBindingMismatch` refusal. **Two readers nobody had recorded**: the
   svm-harness Resolution and Relayed Markets each planted a V1 record as their
   rent beneficiary. Nothing decodes a beneficiary's bytes — Core compares it by
   key and credits lamports — so both now plant a Rent-owned account and say so.
   They are NOT V2 credits, structurally: V2 is keyed by
   [domain, market, generation] and in those two fixtures the Market address is
   derived from an identity already carrying the beneficiary, so a V2 credit
   cannot be the beneficiary of the Market it is keyed by. Reordering them is
   that family's work. Left behind: the rent `Error` enum's five variants that
   lost their last constructor when DELDEC took the V1 routes (`UnknownAction`,
   `InvalidAccountPrivilege`, `InvalidSystemProgram`, `InvalidSystemWallet`,
   `AccountAlias`) — Rust does not warn on an unused public variant.
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
- ~~Candidate permissionless upgrade to verify: Wormhole Queries (guardian-signed
  Solana account reads, verifiable against the devnet core bridge) — MR lane
  owns pinning whether this actually supports what we need.~~ **ANSWERED: NOT
  AVAILABLE.** `docs/design/MAINNET_STATE_RELAY.md` §3 concluded Wormhole
  Queries is *"not a candidate for v1 and not a near-term upgrade path"* —
  §3.2 gives the reason (on devnet the guardian set is a single test key, so a
  guardian signature there proves nothing a relayer signature does not), and
  `:64` marks the row **"not available."** Flagged as a stale line by
  `docs/design/ORPHAN_DESIGNS_TRIAGE_2026_08_30.md` §3.9 and left for whoever
  next edited this file.
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
   product-payoff-sbf, product-evidence-sbf; and **the whole DCLTCAT1 stratum**
   (STRATUM lane, 2026-08-27) — claims/custody/controller-proof-sbf and their
   three svm-harness campaigns, market/collateral/direct/terminal-contract,
   realm-contract's PositionV1, dclutch-kernel's CategoricalLedger, the
   operator's foundation/compiled_direct/registered_direct/source_resolution
   modules AND its 1,474-line crate root, the gen-2 local-validator launchers
   and old bootstrap, the browser's Rust fixture generator, and the
   MarketRoot-reading test-only modules (product-admission-contract,
   registry-contract's authority, capability-contract's readiness_frame).
   Plus **DCLLBX02**, the liability-basis V2 issuance route. ~38,000 lines.
   The repo's contents converge to ONLY the active built system.

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

- ~~TWO live Market representations on chain: DCLTCAT1 alongside DCLTCOR2~~
  **ANSWERED AND EXECUTED (STRATUM, 2026-08-27): DCLTCOR2 is the one Market
  truth.** DCLTCAT1 had no writer in the tree — its only founding route needed
  the deleted monolith ELF — so it was never a second authority, only a second
  vocabulary waiting to become one. Buried with carve-outs. Two residues are
  NAMED, not swept: (a) core-contract's `MarketRoot`/`MarketIdentity` now have
  exactly ONE production reader left — general-config's test-only
  `plan_general_activation_v2`/`v3`. The second, the Dealer accelerator's
  232-byte `MarketRoot` decode of the 352-byte `CoreState` the chain actually
  holds, is FIXED (DLR-HOT, a6d68ab4): it reads `CoreState` with the canonical
  Claims-aggregate join set, and `MarketRoot` survives in that file only inside
  the regression test that pins the two representations apart;
  (b) the browser's `lib/decoders.ts` DCLTCAT1 arm waits on
  `lib/economicSuccessor.ts`, which is itself stratum and belongs to the
  economic-web lane.
- The Hoard vault has no chain-derivable address (namespaced by caller-chosen
  founding context) — frontend refuses-with-reason today; decide whether the
  context belongs in a discoverable record.

- ~~DCLLBX02 (liability-basis V2 route): live route or superseded second
  truth?~~ **ANSWERED AND EXECUTED (STRATUM, 2026-08-27): deleted.** Dead on
  both ends — no producer built the instruction, nothing on chain finalized the
  `DCLTLNK2` record it decoded — and its own deletion note had queued it behind
  "whoever retires the V2 liability-basis kernel", an event that will never
  arrive because that kernel has an active lane. The shared LBV2 state
  vocabulary (`MarketViewV2`, seeds, widths, encoders; ~20 live consumers
  including the web portfolio) is INTACT in
  `dclutch-claims-svm::liability_basis_state_v2`. Correction for whoever reads
  the old note: `test-programs/liability-basis-caller` is LIVE and was NOT
  deleted — it is the `trading` program in the protocol-position ProgramTest.
  Census note: the route was never enumerated by the census inventory in the
  first place, so no denominator moved; its dispatch shape
  (`instruction_data.get(..MAGIC.len()) == Some(...)`) is invisible to the
  enumerator, which is a census gap worth closing.
- Relayer daemon slice note: consumption (1,534 B) and full-body append
  (1,377 B) exceed legacy packets — witnessed by label in the relayed tier;
  the daemon must build v0/ALT for those two when it goes live. The failure
  walk deliberately stays legacy-fitting (991 B) — it must never depend on an
  ALT a silent operator never published.

## DECOMP charter expansion (dispatches at W2q's yield)

DECOMP now carries, beyond the palimpsest split: M-27 the effect-kernel
visitor seam IN FULL (the O(R^2*I) resolved_invocation + route_request_start
re-scans — "the single highest-value item in the tree"); M-28 the
sysvar-parser convergence (one owner, the adversarial corpus on the
heap-admission path); M-29 the AccountInfo migration per W2g's spec (the
4,776-byte floor); M-31 the allocator-cfg ruling for no-entrypoint library
builds; M-34 converging the four ProgramTest-evidence emitters. One lane, the
hot-path's whole residual debt, on the decomposed shape.

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

## The closing pattern language (orientation, 2026-08-27)

~45 open items collapse into seven patterns; five have one-stone moves:
1. BUNDLE BUILDER: one family-generic chain-fixture builder derived from the
   emitted artifacts themselves (direct-hot is the reference to generalize).
   Kills GEN-HOT/DLR-HOT/physical verticals/most census reds AND the
   fixture-bent-to-wrong-side genus. Dispatch after W2n lands (it owns
   direct-hot fixtures today).
2. EXACT-PIN GENUS WITNESS: census extension enumerating every Exact
   width/count/length constraint in profiles+dispatchers, verdicted against
   live-cluster shapes. RegisterBuy's defects become rows; unknown genus
   members surface.
3. ONE-AUTHORITY COMPLETION: DCLTCPR1 encoder + coreFound convergence + a TS
   BACKEND on the Lean emitters (kills the web hand-mirror genus permanently),
   with a grep-driven completeness inventory as the done-criterion.
4. THE DEMO VERTICAL: recovery leg + daemon runtime + v0/ALT wires + product
   records + wizard defaults + devnet rehearsal = ONE journey-shaped lane
   ending in a real graduation market resolving on devnet.
5. GENERATED REFERENCE: the protocol manual generated from the same
   authorities (ABIs, refusal tables, census, budgets, ADRs); guides
   hand-written thin on top; one pipeline also ships the real app to Pages.
6. UNIVERSAL LEDGER: the six-law conservation engine becomes every family
   campaign's gate once (1) exists.
7. LANE WRAPPER: tools/lane.sh — enforced --only, pinned rustfmt, board
   helper; retires four recurring accident classes.
Sizing: patterned ≈ 10–14 lanes ≈ 2–3 swarmcycles to closed-or-explicitly-
parked (vs ~25 bespoke). Parked: the universal round-trip and refinement theorems — today's evidence
is per-case corpora and emitter checks, which prove the cases and nothing
else. That gap is real debt, parked by decision, not covered by anything.

## Doctrine, distilled by the Fable derpage review from the whole board

1. Never-executed is the default: a route/guard/PDA domain ships with the
   census row that executes it, or ships marked NEVER-EXECUTED.
2. The fixture is never the authority: a fixture field chosen to make a test
   pass carries a derivation or citation; address-vs-digest gets a newtype.
3. One fact, one author — and a guard's other side is a different author.
4. Sweep the siblings before you close (including the one that fails the
   opposite way).
5. No estimate is a total; no silence is a result.
Anti-finding, recorded: entrypoint_adapter.rs is the named standard for an
unsafe membrane (every SAFETY audited and holding).

## Fable-wave verdicts, remaining dispositions (2026-08-27)

DISPATCHED: HOARD (ADR-0007, custody-namespace owner — ruled, ember veto
window open), STRATUM (CAT1 world + DCLLBX02 burial with carve-outs), REFCODE,
DELDEC, LEANGUARD, WEBGHOST-pending (economic/generalSuccessor deletion +
productV2 split + DCLTPRQ2 collision + abi:verify into npm test — launches
when DELDEC yields to avoid productV2.ts collision).
QUEUED with owners:
- Tranche-A Dealer: CLOSED by DLR-HOT (a6d68ab4). v3_accelerator_accounts.rs
  decoded a bare MarketRoot where the chain holds CoreState (Fable P5a) — it
  refused on LENGTH before reading one join. Now the canonical join set, with
  the address re-derived from MarketCoreStateSeedsV2 per that type's own
  contract. The campaign is no longer representation-broken; what it still
  lacks is the chain fixture (below).
- U-014 owner: the Direct AOT inversion — deployed accelerator accelerates
  the superseded V2 descriptor; the V3 AOT is selectable by nothing and
  carries one recorded admission disagreement (P5g).
- Post-W2 decomposition lane: hot_v3 palimpsest split (accelerator auth out
  to a contract crate, seal to its own module) BEFORE GEN-HOT lands (derp 8).
- Static-assert genus lane: import-don't-restate + asserts at every literal
  pin; controller-proof's length-only dispatch gets a magic tag (derp 9).
- Islands needing owner decisions, sized (P5f): product-admission island,
  representation-composition-v3-operator + rational-lifecycle-hot-v3 (9,448
  LOC), fractional-claim-operator (8,582 LOC), market-retirement-v1-operator,
  dclutch-svm-harness CAT1-era tests (~15k, mostly stratum).
- Renames (small batch): resolution-proof-sbf -> resolution-sbf;
  fractional-claims-kernel doc-rename; DEMO_ACTIVATION profile id (verdict
  settled, P6).
- Docs pattern-5 opener: the REPRESENTATION MAP (fact -> live magic -> owner
  -> writer) — the highest-value artifact for every future reviewer (P5i);
  ARCHITECTURE.md still narrates the MarketRoot era.
- Banish checklist gains: sweep apps/dclutch-web for the deleted program's
  vocabulary in the same commit; abi:*:verify wired into npm test.

- U-014 owner, sharpened by the late sweep: the accelerator migration is
  PARTIAL — schema identity is V2 tree-wide (~20 consumers) but
  direct-aot-sbf still decodes the V1 wire (AcceleratorRequestV1/AckV1).
  The two orphan V1 schema IDs are clean deletes; the wire types are NOT
  until that program migrates. Never delete "V1 accelerator" as one unit.

- Tranche-A Direct: DONE 2026-08-27 (TR-A-DIR) — see items 4, 5 and 7 above.
  Three commits: the Lean authorship + byte-identity gate, the Profile14 credit
  migration + width defects, the V1 record deletion.
- Small batch: fixtures:verify provenance regen for realm-contract/src/lib.rs
  (moved by TSGEN's f5dfe5d); the direct-inline alias-table retirement once
  W2o yields; the inline.rs:308 composition theorem LEANGUARD queued.

- **THE REGISTERED CAMPAIGN IS NOT A HARNESS EXTENSION. Sized 2026-08-27
  (TR-A-DIR), because the tranche-A charter asked for "registered fills through
  the Registry continuation at 1.4M/32KB, extend the gate or the gauntlet with
  create/fill/cancel/expiry/terminal" and that reads as a day of work. It is
  not.** The registered family has ten actions (`DirectExecutionActionV3` 2..11).
  What exists per action, verified by grep at HEAD:
  - **requests**: all ten have encoders/decoders in `registered_requests_v4.rs`.
  - **RegisterBuy**: the only complete one. RequestProfileV2 + Transition +
    Strategy + LifecycleV5 + EffectV4 + Profile14 + `build_direct_register_buy_hot_bundle_v4`.
    Never executed on any chain: its bundle builder is called only from its own
    test module, and this lane just fixed three defects that would each have
    refused it plus a fourth (signed rent-credit fields bound to unwritten
    registers) that no host test could have caught.
  - **RegisterSell**: shares the creation Transition and LifecycleV5. No
    AccountProfile, no Effect, no bundle — `registered_account_artifacts_v4.rs`
    is titled "for registered Buy creation" and means it.
  - **FillRegisteredOrdinary**: RequestProfileV1 + Transition + Strategy only.
    **No AccountProfile, no LifecycleV5, no EffectV4 (its Claims transfer and its
    three Custody legs have no route program at all), no bundle, no descriptor.**
    Its only consumer in the tree is the host-side AOT translation and its tests.
  - **Split, Merge, Cancel, Expire, CloseInvalidated, CancelThrough,
    CloseMakerReplay**: request encoders only.
  And `programs/dclutch-trading-sbf/program-test/` plus `tools/gauntlet/direct/`
  contain ZERO occurrences of "registered", "RegisterBuy" or "RegisterSell`. The
  15/15 gate is entirely the inline-ordinary bundle.
  So the campaign's real shape is: build the Fill's AccountProfile + LifecycleV5
  + EffectV4 + bundle, build RegisterSell's three missing artifacts, build a
  registered chain fixture + program set + descriptor, wire it into the gate, and
  only then measure CU. That is the artifact set the ordinary path took several
  lanes to produce, not a case list. **BUNDLE's family-generic chain-fixture
  builder is the right lever** and should land before anyone starts hand-building
  a registered fixture. Nothing in this lane was blocked by it; it is the next
  lane's premise, corrected.

- Tranche-A Direct: all three items DONE 2026-08-27 (TR-A-DIR). Correction to
  this entry's own premise, for whoever writes the next charter: the coords-7/10
  credits and the width defects were never in `registered_fill_artifacts_v4.rs`
  — they live in `registered_account_artifacts_v4.rs` (rules + `validate_lengths`)
  with their names in `registered_state_artifacts_v4.rs`. And there was NO
  identity to regenerate: the registered family pins no content identity
  anywhere (`registered_bundle_v4.rs` digests every artifact at runtime) and
  `apps/dclutch-web/lib/generated/registeredDirect.ts` carries layout offsets and
  magics only. Nothing regenerated; no stale window existed to close.

- DLR-HOT — RAN 2026-08-27 night. The two top-of-lane checks are ANSWERED, the
  decode defect is FIXED, the frontier instrument is landed and advanced two
  stages, and DIAG-82's gate is green on the accelerator link. The round trip
  does NOT reach Accepted and this entry does not pretend otherwise.
  - (i) Dealer's selector-9 descriptor DOES name `effect_kernel::v4::`
    `SCHEMA_RELEASE_ID_V4` (`v4_scenario_release.rs`, `CapabilityArtifactsV4`
    `.effect`). No General V3/V4 cascade here; that check cost a lane and cost
    this one nothing.
  - (ii) The span selector is NOT request-written, so **the family is forced
    AdmittedAot and inherits the whole extras frame.** Of Profile13's nine
    dynamic spans the RequestProfile writes exactly two selectors (scalar 0
    position count → span 4; scalar 99 evidence count → span 7). The six
    optional Custody route-span selectors (scalars 7..12) are written by
    `project_scenario_custody_bank_v4` in the ADMITTED CANDIDATE BANK — the
    accelerator's own output — and span 8's scratch-page count (scalar 101) is
    Hot-derived, documented in place as written by neither Request nor Effect.
    Consequence with teeth: `dealer_scenario_scratch_page_count_v4` refuses
    inline transport outright and admits only `AuthenticatedScratchPages` at
    exactly 6 pages, so the fixture has no inline shortcut.
  - What DLR-HOT still lacks is one thing, sized: the Dealer chain fixture.
    `dealer_chain.rs` is still 210 lines of staged imports with zero `pub fn`.
    BUNDLE's builder does NOT model the admitted-AOT frame (`bundle.rs` emits
    `fixed(39) ++ runtime[5..]`; `general.rs:82-92` names the gap and calls it
    the next builder-side lane), and its `derive_dynamic_span_widths` admits at
    most ONE profile-only span while Dealer has seven. The lever nobody has
    used: `operator/dealer_scenario_hot_v4.rs` (605 lines) already computes the
    exact `fixed(39) ++ extras(8) ++ authorities ++ suffix` layout AND all nine
    span counts, for Dealer specifically.
  - The frontier now clears six stages and stops in
    `authenticate_product_runtime_v3` on the Product graph — four finalized
    Registry records. Everything before it is bought and differentially proven.
- RECORDS-MIGRATE (charters at wave convergence, batched with the combined
  identity regen): the layout audit's wire-breaking migrations — FundingStateV1
  63.8% (drop `released`, totals, allocation pads), seal stored-PDA→bumps
  (372 B; the release-set pattern), profile-family width narrowing, manifest
  entry 101 B, the derivable-identity records; genref derives all limits
  (retire the copied-1312 class). Kernel-side caps (QNT item 9) ride the same
  lane as one-liners.
- Mystery for hygiene: target/deploy/dclutch_sbf.so is 9.0MB, rebuilt TODAY,
  referenced by nothing — find what rebuilds a deleted program's ELF and stop
  it (63 SOL if a deploy glob ever eats it).

- Journey trading stages: three independent walls named by JRNY-2 — prestate
  (Claims admission is behind the Hot gate; CUSTROLE's replay-creation is the
  pattern for the wallet-side), SHAPE, and PACKET (1,228 +
  SetComputeUnitLimit = 1,268 > 1,232: the journey's trades ride v0/ALT from
  day one). **SHAPE IS DOWN, and was never a wall (GEO-ART, 2026-08-27,
  4b67d29e + 1b0fe8be).** Its premise — "the shipped Direct profile is emitted
  for the 3-claim/3-cut canonical geometry; a 4-claim/2-cut market needs
  geometry-parametric artifact emission" — was wrong in both halves. There is
  no 3-claim/3-cut geometry: Product Runtime V2 pins `region_count =
  cut_count + 1` and `outcome_count = region_count + 1`, so a geometry is ONE
  number, `outcome_count = cut_count + 2`, and the canonical demo is 3
  outcomes at ONE cut. And no geometry needs its own emission: every
  runtime-width account is stated as an affine `(base, stride)` rule against
  the transaction's own Product tail, `item_account_stride` is 0 family-wide,
  and the artifact bundle, ProgramSet and all pinned identities are
  byte-identical across fifteen geometries. Measured on real ELFs through the
  Registry continuation: EVERY geometry from 2 to 30 outcomes trades on the
  one artifact set (31 exhausts the 1.4M ceiling, and the sweep asserts a
  too-wide market runs out of COMPUTE rather than being refused its shape).
  `DirectOrdinaryGeometryV3` (`crates/dclutch-direct-codec/src/
  ordinary_geometry_v3.rs`) owns the arithmetic and derives the four record
  widths a market of a given width must present. DEPLOY-1's flagship — cuts
  12,000/18,000, coefficients 1,0,1,0 — IS the 4-outcome market, and it needs
  no artifacts of its own; the activation recipe is on the board. What SHAPE
  did NOT touch: prestate, and the fact that no Direct entry has been
  activated on a public cluster.
- CU-BUDGET rows owed: CreateFund (86% of ceiling) and VerifyFundReady (84%)
  are unbudgeted; CreateFund's frame is 2,016 B on ALTs. Add at next tier run.

- The /create wizard charter (cycle-3, with TWIN's §12.3 window-width table)
  now has its recovered founding document: docs/recovered/
  TRADING_UI_FLOW_BRIEF_2026-08-25.md (M-8, recovered from session JSONL —
  the brief's /markets and /portfolio halves shipped independently; the
  /create wizard and /activity halves never did). Reconcile, then build.
- For the next Sonnet reviewer: the rent Error enum carries 7 MORE
  unconstructed variants beyond the 5 deleted (SN6's flag, named in its
  yield) — verdict each (dead vs awaiting-constructor) and act.

- κ ENFORCEMENT — **CLOSED, and it closed earlier than this file said**
  (KAPPA-CAP, 2026-08-31, correcting a row that read "no on-chain route calls
  it" for four days after routes did). The ruled shape landed whole in
  `ff008fea`/`e5933c4d`: `CoreState.principal_cap_sets` at offset 288 INSIDE
  the existing 368 bytes (so κ never moved `STATE_BYTES` — the widening this
  file kept queueing was already spent); the Found frame carrying the three
  `(raw, staging)` pairs at indices 16-21 that let `Found` authenticate the
  profile and the named floor and derive the cap; and the check at founding
  AND at all three growth sites — `founding_v5`, `signed_delta_v3`,
  `affine_batch_v2`, plus the legacy complete-set mint. A zero cap refuses at
  `Found` outright.
  RECORDS-MIGRATE row (b), `SourceCapacityProfileV1.floor_content_id`, is
  **superseded, not owed**: the hole it was ruled to close — two floors with
  identical bindings both validating, caller picks the biggest — is closed at
  a better site by `SourceMaterialV3.principal_policy =
  BoundedByFloor(selected_floor_id)`, which `derive_principal_cap_sets`
  refuses to run against any other floor id, and which requires `None` under
  `ExplicitlyUnbounded` so no floor can be smuggled into that policy. It also
  could not be built as ruled: the profile's free tail is 16 bytes at offset
  96 and a `ContentId` is 32.
  What KAPPA-CAP itself added is the missing half — the refusal had no NAME.
  All four sites flattened the kernel's named refusals into a neighbouring
  generic variant, so a capacity refusal read like a malformed record. Four
  appended `PrincipalCapacity` variants (`0x500D`, `0x5168`, `0x518A`,
  `0x5208`) now say it, proved red before green.
  The vacuity that hid all this is also closed at one site: every program-test
  fixture founded at `u64::MAX`, so the refusing arm had never executed on a
  real ELF. `affine_batch_v2`'s program test now founds at an exact cap — with
  supply 7 and cap 10, a credit of 3 commits and 4 refuses as
  `PrincipalCapacity`, refused bytes unchanged — and was proved red twice
  (unbound the cap and the excess commits; the code is matched structurally).
  **Still owed, and named as debt:** the other three enforcement sites —
  `founding_v5`, `signed_delta_v3`, and the legacy complete-set mint — are
  enforced but still have no on-chain test that founds a BOUNDED market; their
  fixtures remain at `u64::MAX`. The affine-batch test is the pattern to copy
  (bind the cap into `core_state`, credit past it, assert the named code).
  Also standing: κ = 1/4 is still **Provisional** and its lifting plan —
  measure the realisable fraction per venue, then state a `Measured` envelope
  — is unstarted, so AGENTS.md:125 is satisfied only in the sense that the
  plan is written down.

- Next small batch: series-shadow-sbf + the fractional crates still carry
  production sha2 — convert to dclutch-sha256-adapter (the landed backend;
  one-shot hashv only, never the incremental Hasher). GIT-SCAN item 8 row
  CLOSED (69ea61fe, verified by SHASEAM).

## DEVNET-SMOKE charter — AMENDED by SMOKE-0 + decision 0012 (2026-08-27 evening)

SMOKE-0 ran (docs/evidence/DEVNET_SMOKE_0.md): preflight + mutable
deploy/recycle rehearsal on devnet, 0.0096 SOL total cost, wallet 65 →
64.99. Four walls found pre-spend; ember ruled W1 LIVE: **the devnet
substrate is mutable and iterated** (decision 0012 — the slot pin replaces
revocation in the fast path's soundness; iteration by Upgrade at fee-cost;
~31.7 SOL parked, never burned; the immutable ceremony is reserved for the
final public demo substrate). The devnet PythReleaseV1 row is MINTED
(11f249ff + the 9b08090d nibble fix). ALL LANDED by 18:50: the 0012
admission tree-wide (PIN-0012, 0e34c036 — eight ReleaseSuperseded bands,
census 209), the producer minting policy from observation (DEVNET-DRIVER,
636230ef), the external campaign driver (DRIVER, d94dc438..1040e918 — W3
closed; publication+init proven locally, Pyth row re-authenticated 8/8 on
devnet read-only, W1 measured as a live 0x1004 refusal pre-admission), the
82-diagnostic frame regression (DIAG-82, 9dc2a6bb + the d1378427 gate;
root-caused to 3071fbe8 by ACCEL-FRAME's independent bisection), the
checked-release candidate green again (7c12af9c), and the kappa carried-cap
kernel with its equivalence theorem (KAPPA-ENFORCE, c953b640..74275738;
storage ruled onto CoreState with profile floor_content_id, RECORDS-MIGRATE
rows). **DEPLOY-1's two triggers are both met.** Its named debts: the 0012
CU claim needs the 20-seed re-measure; founding-stage spec-vs-plan wiring;
journey externalization + the persistent-founder decision; the explicitly-
unbounded V5 founding declaration before a fail-closed kappa check can land.
The charter below otherwise stands, with "recycled at the end" now meaning:
closeable because never revoked.

## DEVNET-SMOKE charter (expanded scope, awaiting ember's go)

The smoke is a small public exchange, not a market. Deploy the seven roles
once (checked-release, ~29 SOL, recycled at the end), then:
1. The Pyth market: SOL/USD range protection under kappa, resolved by REAL
   devnet Pyth in a widened window, redeemed, retired.
2. The mainnet-observer market: a devnet market about a REAL mainnet
   pumpfun/DBC graduation — the daemon reads live mainnet, relays signed
   frames. The thesis, live.
3. The abandoned market: relayer goes silent ON PURPOSE; the funded failure
   walk runs in public with its bounty collectable by any devnet wallet;
   walk instructions published on Pages.
4. N=16 per market: distribution, ring, redemption of real atoms, refusals,
   retirement.
5. The conservation ledger runs against the PUBLIC chain; its verdict and
   transcripts publish into the Pages reference as the evidence appendix.
6. The browser live against devnet: list, detail, portfolio, redemption.
7. Recycle: programs closed, rent recovered, transcripts kept.
Stretch (gated on lanes landing first): General batch auction with real
order flow; continuation trading at chosen-green seeds.
Triggers: DECOMP-r compiling HEAD + DEMO-VERT-r yield. Authorization:
ember's explicit go.

- DOCTRINE (STRUCT-PHYS-r's generalized correction): NO effect operation
  moves a token — effects write lamports/scalars/identities/child-requests
  only; token work happens exclusively through FixedRole children (closed at
  four; only Claims touches Mints). Any design assuming effect-program token
  work is wrong — Fractional's twin inherits this.
- RULED: Structured adopts the Rational child ABI (0011 §3a Option A — the
  ABI already names the Structured operations; zero new program code; the
  binding requirement rides it). STRUCT-CHILD dispatched.

- M-46 hunt (small batch): main sat at 18/20 somewhere in
  211079f6..a4be9a83 (~+23,000 CU unboarded); 7ead0716 MASKS it. Bisect with
  the per-phase tables and attribute — margin honesty demands the name.

- DOCTRINE (STRUCT-CAMP): an artifact builder with no caller outside its own
  crate is not landed, it is PARKED — it has no gate. (The bearer operator
  sat at 5/20 through two sweeps; sixteen tests passed a join no chain-
  acceptable descriptor could satisfy because the fixture was bent to the
  wrong side.)
- RECORDS-MIGRATE row (from 27dbcca0): root_id's dead consumer + the
  graph_id/graph_digest double-booking in the representation descriptor.
- FEE-GEO row (2026-08-27, study landed: docs/design/FEE_GEOMETRY.md — the
  N-1 reconciliation). RULED there: flat `fee_basis_points` CONFIRMED for
  v1-devnet as the recorded placeholder; ADOPTED_2026-08-20 item 9's
  composite `kappa*G + kappa'*R` remains the selected target shape (nothing
  reversed; rates still open, still ember's). Post-smoke lane, in order:
  (1) geometry kernel in Lean — monotone-accrual `feeAt` generalizing
  `cumulativeFee`; the telescoping lemma (DirectProofs.lean:145) already
  covers any monotone feeAt; (2) the composite on the fee-free General
  batch relation (gen-1's native threat model; gen-2's RevenuePolicyV2 /
  0x82-0x84 registry vocabulary is the precedent); (3) Direct keeps flat,
  optional time-bracket ramp; (4) RevenuePolicyV2-style registered
  destination record (treasury pubkey stays reserved to ember, M-26);
  (5) bounds frozen BEFORE implementation (the B2 ordering two generations
  violated); N-15 (formalize the characterization before any rate freezes)
  rides step 1-2. STANDING REFUSALS: source-adaptive fees (Mango/kappa
  lens + hot-path CU), redemption fees (the objective), geometry-as-code,
  floats, fee-record mutation. Smoke demo: per-market signed rate
  diversity + conservation ledger fee take — zero new code. Trigger:
  cycle 3 / post-DEVNET-SMOKE.

- CORRECTION (STRUCT-CAMP-2, derived not asserted): Structured's binding K
  ceiling is 2, not 3 — the PACKET, not the 1312 profile bound: full-width
  Issue at K=3 = 1,357 B on a live ALT vs 1,232 (168 B/coordinate). A K=3
  product can be denominated/reconstituted/redeemed but never issued on a
  cluster. The cliff doctrine's second exhibit (first: the copied-1312).
  The K-lift is a SESSION-SPLIT question (paged Issue) — belongs to the
  cliff-doctrine design pass, which still awaits ember's go.

- RULED (GEN-TRIPLES, 2026-08-27): NO fifth profile count — ADR-0010 §4's
  7-and-14 grammar stands; a partially-authored action set must have no legal
  release. GEN-SEVEN queues as ONE coordinated unit (all seven triples + the
  campaign + one batched regeneration, one commit series) at the current
  wave's convergence. Every rung is laid: GeneralTransitionV3.lean (first-run
  byte-identical), the four state envelopes, the OpenBatch root-write answer
  at common_rule's coordinate-0 arm. Evidence: GENERAL_TRIPLES_2026_08_27.md.

## DEPLOY-1: EXECUTED — THE SUBSTRATE LIVES ON DEVNET; THE FOUNDING STOPPED AT ONE NAMED WALL (2026-08-27/28)

Run under keyDEPLOY-1. Full record: docs/evidence/DEPLOY_1.md (citable;
deployment sections final). THE DURABLE SUBSTRATE IS UP, ACTIVATED AND
PERMANENT: seven roles deployed mutable per 0012 (TPU, ~2.5 min, byte-verified
both sides), plan minted from the real ProgramData observations, nine records
+ profile + five-role activation under the slot-pin admission on the public
cluster (Trading 697,109 CU), all detector-confirmed. The founding seam
(DRIVER's spec-vs-plan) is WIRED — campaign --market, chain-reading detector,
partial-refusal, detector==verifier — and proven end-to-end locally (DCLTGMF1
at 1,199,823-class CU under the driver); on devnet it drove Found31 + both
DCLTPCB1 lanes with real signatures and stopped at the chain's own answer:
**TooManyAccountLocks — the DCLTGMF1 atomic frame locks >64 unique accounts,
devnet lacks increase_tx_account_lock_limit (local has it, 128), so the
five-stage atomic founding cannot execute on today's devnet.** The wall is an
authority item with a recommended answer (narrow the frame under 64 via the
queued RECORDS-MIGRATE rows; atomicity re-decision is the fallback; a devnet
feature activation is weather). Paid lessons, all fixed + committed: the
forge-peek drift, Pubkey::new_unique probe addresses existing on devnet, the
resubmit/rebuild-on-drop machinery, priority fees on campaign transactions,
the replica-spoofed expiry verdict, and the meta-derived fee-only rollback
proof. New driver surface: devnet-market (cadence-floor-refusing Pyth
flagship input), graduation-market (relayed input from ONE author with the
vertical, real mainnet venue facts), ledger-census (the journey's seven-law
engine against any cluster). Web: default endpoint = public devnet, cluster
named from its genesis hash, smoke pages one-record from live. Wallet 64.99
-> 32.19 (31.77 parked rent; peak < 33 of the 40 cap). Doctrine earned:
**send one preflighted probe of a new frame shape before spending a ladder on
it** — skipPreflight hid the lock-limit refusal for four diagnosis cycles.

## DEPLOY-1 queue (triggers: PIN-0012 + DIAG-82 green)

The durable devnet deploy and the first market living there. Charter =
decision 0012's substrate (mutable + slot-pin, ~31.7 SOL parked) deployed
via the runbook's TPU path, then DRIVER's two honestly-open wirings: the
founding stage (spec-vs-plan — execute_found_market is origin-agnostic
already; wire the market input into the driver's plan without half-wiring
principal) and the journey runner externalized onto the driver's persisted
per-role forge. Then: the three-market exchange per the DEVNET-SMOKE
charter (SMOKE-1), web pointed at devnet, the ledger against the public
chain. Cost basis measured: deploy ~31.7 parked + 0.0287 activation + dust.

- RECORDS-MIGRATE rows (KAPPA-ENFORCE's mapped finding, ruled 18:55): (a)
  CoreState gains the principal cap (zero reserved bytes today — the cap is
  part of the wire break); (b) SourceCapacityProfileV1 gains floor_content_id
  (32 bytes; profile has 16 free) — closing the REAL hole that two floors
  with identical bindings both validate, letting a caller pick the biggest.
  The Found-frame +6 accounts rides the same migration.

- M-61 (DIAG-82), a reporting rule: the sweep's per-seed CU is a BUMP-SEARCH
  LOTTERY re-rolled by the trading ELF digest itself (deltas = n×1,500 ± ~50;
  ±46,000 across seeds). "Worst margin" is not a property of the code — CU
  claims report the PASS COUNT and 20-seed MEAN; M-46's bisect uses those.
  Watchlist the new frame gate prints: four functions within 512 B of the
  4,096 wall (3,904/3,840/3,776/3,776/3,584).

- DEPLOY-1 addendum (PAYOUT's FE handoff): browser redemption step 2 rides
  the deployed markets' published ALTs (DCLTSQ03, role byte 1, wallet at
  coordinate 0 possibly writable; 1,869 B legacy vs 1,006 on an ALT) and the
  builder evaluates the Product off-chain (coordinate 23's PDA digests the
  payout). The FE wiring lands when the smoke's markets publish their tables.
- SN7 batch: a workspace-enumerating check gate — root cargo check CANNOT
  see the nested test-caller workspaces (PAYOUT's enum change left root
  green + three crates red; only the campaign build caught it). The SBOM's
  39-workspace discovery is the enumeration to reuse.
- LINGER item: genref regen once the tree quiets (two lanes deferred it
  rather than publish unlanded line numbers as generated truth — correct).

- Decision 0012's CU debt: CLOSED BY MEASUREMENT + REFRAME (d20837fd): the
  ExactAuthority arm runs 20/20, and the cost is **+73 CU exactly** — the
  +2,098 first reported is the difference-of-means UPPER BOUND, and pairing
  the seeds (same seeds, same ELF, so `delta = n*1500 + c` solves per seed)
  gives c = +73 on all twenty, with the immutable-pinned control returning
  c = 0 as the method's self-check. Ledger M-65 is the general form: a
  lottery you cannot remove you can often CANCEL. On the Direct Hot route
  NEITHER arm ever hashed — the decision's purchase is "mutable: refused →
  admitted at parity", now with the parity measured to the compute unit. The
  700k figure belongs to the Shadow path's hashing site only; its
  counterfactual is not owed (a hashing variant nobody ships). Substrate arms
  live in waist::FixtureSubstrateV1 (DCLUTCH_FIXTURE_SUBSTRATE).
- SN7: registry-sbf lib.rs:378-380 doc prose is pre-0012 ("keeps the full
  current-ELF hash" — no longer true of the function it documents).

- DLR-ACCEPT (codex-ready, sized by DLR-HOT): the Dealer Accepted transcript
  + pool campaign. The lever: crates/dclutch-operator/src/
  dealer_scenario_hot_v4.rs already computes the exact fixed(39)+extras(8)+
  authorities+suffix layout AND all nine span counts — wire it into
  dealer_chain.rs (210 lines, zero pub fn today); BUNDLE's builder lacks the
  admitted frame (general.rs:82-92 names its own gap). Frontier stops at
  authenticate_product_runtime_v3; the seeds-exclude-own-address probe trick
  is landed. Also: dealer/README.md's 35-account packet exemption is stale
  under an admitted bundle — re-answer, don't inherit.

## HANDOFF — Claude session closes, codex takes the baton (2026-08-27 night)

WHAT IS TRUE TONIGHT, all publicly verifiable:
- Seven programs PERMANENT on devnet (addresses in DEPLOY_1.md §2), mutable
  per decision 0012, five-role activation measured on the public cluster.
  31.77 SOL parked; wallet ~32.2.
- clutch.dregg.pro is the real app: opens on content (baked deployment
  manifest, live-verified), lists the six Founding-phase devnet markets
  cold, decodes all 51 record types + refusals by name, charts per the
  dataviz discipline, one nav, consoles with provenance. HTTPS cert may
  still be provisioning (re-triggered; flip https_enforced when it exists).
- The whole market life is proven on local validators (found → trade →
  resolve by real Pyth → redeem atoms → retire-to-Retiring); geometry is a
  non-wall (2..30 outcomes on one artifact set, proven in Lean); the gate
  is 20/20 seed-proof; the relay daemon runs armed-ready on the anchor
  with a hash-chained public log.
- Filings: 1717 FILED (2026-08-27). 1388 ready to file (date bump + ember's
  one bracketed line; citation gate satisfied at dragons-clutch a9e587ab).

CODEX MISSION #1 — THE FOUNDING LOCK WALL (the one gate to the first open
devnet market): DCLTGMF1's atomic frame locks >64 unique accounts; devnet
lacks increase_tx_account_lock_limit (64 vs local 128). DEPLOY_1.md §8 has
the yielded recommendation: unique-lock census first, then frame narrowing
via the RECORDS-MIGRATE rows, shipped as a 0012 Upgrade cycle. The wall
also blocks: the flagship + graduation + abandoned markets (driver
subcommands exist, one command each once founding lands), the browser buy
on devnet (GEO-ART's recipe: emit nothing, activate the existing artifacts),
and SMOKE-1's story pages (dark-launched, ready).

THE QUEUE, every lane with its lever named in its entry above: DLR-ACCEPT
(the operator's layout computer), RECORDS-MIGRATE + the cliff-doctrine
pass (two exhibits priced; ride the same Upgrade as the lock narrowing),
GEN-SEVEN (one unit, rungs laid), Fractional twin (road mapped by
STRUCT-CHILD/CAMP), VX vinext upgrade, SMOKE-1 cleanup rows (sol6/7/9
prestates), M-46 attribution, SN7 smalls. Standing rules that bind: lane.sh
for commits/fmt; the frame-diagnostic SCRIPT over every shipped link;
M-61 margin reporting (pass count + mean); parked-is-not-landed; the
board (/private/tmp/dclutch-wave-board.md, archived copy in docs/) is
append-only coordination, never authority; reader-voice on anything public.

DOCTRINE, from the Dealer accepted-transition lane (2026-08-29): EVERY NEW PDA
DOMAIN GETS A CONST LENGTH ASSERTION. Four Dealer domains shipped at 35-36 bytes
against Solana's 32-byte maximum seed length, which does not make an address
unusual — it makes it underivable, so the whole Custody reservation/escrow/
activation family could never be created by Custody nor authenticated by
Trading. Every component test passed; none of them derived the address. The
guard is one line beside the constant — `const _: () = assert!(DOMAIN.len() <=
32)` — and it turns the next over-long seed into a build failure instead of a
family of addresses nobody can reach. dclutch-dealer-codec carries them now;
there was no guard of this class anywhere in the tree before. The sibling
lesson from the same lane: two programs agreeing on a PDA by coincidence is not
agreement — Trading and Custody derived the reservation batch under different
seed counts, so give a cross-program address ONE supported derivation and pin it
from both sides.

## CYCLE 3 CHARTER — the coherently extrapolated platonic dClutch (2026-08-29 ~15:40 EDT, ember: "fold it all in")
Extrapolate the protocol's own principles to completion. Spawned immediately:
1. SELECTION COMPLETENESS (RAT-SEL): Rational selected release + publication +
   founding via the neutral seam; Structured as its child campaign. The seam's
   neutrality test: each family = config + publication, no new driver code.
2. MULTI-CAPABILITY RULING (folded into SEL-SEAM): the manifest encoder
   forbids two entries of one kind; does it PERMIT one entry each of several?
   Read it off the codec, prove or weld shut deliberately.
   RESOLVED (SEL-SEAM, 2026-08-29): both, deliberately. The CODEC permits
   one entry each of several distinct kinds (a five-entry manifest with two
   trade kinds encodes canonically; up to 16 entries fit
   CAPABILITY_MANIFEST_MAX_BYTES_V1) — coexisting capabilities are not a
   wire impossibility, and per-entry roots and funding quotes already exist
   on chain. Founding is WELDED to one selected trade capability at two
   independent seams: the selection seam pins the four-entry shape and only
   grows the three-entry Resolution base by one
   (several_distinct_kinds_encode_but_the_selection_seam_welds_to_one,
   selected_capability.rs), and the funding census requires every entry
   funded by exactly one of the two controllers with Trading funding exactly
   the selected entry — refusing symmetrically whichever of two coexisting
   kinds is selected
   (founding_masks_weld_the_manifest_to_one_selected_capability, market.rs).
   Widening to several selected capabilities per Market is therefore a
   deliberate future decision (driver + census change, no wire change), not
   an accident any lane can back into.
3. PERMISSIONLESS COMPLETION UNIVERSALIZED (LIVENESS): census every
   "someone must act" point in every lifecycle; each gets a funded,
   anyone-can-act path or a precisely named gap. The protocol's one-sentence
   differentiator: no liveness dependency on any identified party.
4. SINGLE-AUTHOR, MECHANIZED (SEAM-CI): SEAM's audit method as a standing
   automated gate (pin-vs-census, derivation-vs-restatement, seed lengths,
   default-pubkey, privilege-merge — across every program pair).
5. UPGRADE-PROOF CLIENTS (ERA): frame selection by on-chain release/generation
   identity — the era-coherence maze dissolved permanently.
6. THE ECONOMY MADE COHERENT (LEDGER): one reconciliation answering "where
   does every lamport come from and go" across fees, rent, bounties, permits.
Gated on the devnet address (not spawned yet): the exchange story — observer
market via relay, graduation/abandonment live, simulator as public heartbeat.
Queued, scoped-not-spawned: Lean as single author of seam contracts (extend
formal/ generation); Series prepare/expire through Trading; split-route live
run; reaffirm implementation (approved, waits for quiet upgrade.rs);
sccache/workspace consolidation (waits for cold gates).
- **RULING — ShadowAot certificate self-reference (ORCH, 2026-08-29 ~18:30 EDT):
  APPROVED: the certificate binds `semantic_release_id` (source-derived, the
  identity ERA proved stable across cohorts and that
  `authenticate_role_semantic_release` already refuses on on-chain), NOT
  `elf_digest`. The ELF digest stays bound where it already lives — the
  ArtifactReleaseV1 record — so the end-to-end guarantee is two facts with
  one author each instead of one self-referential fact. IMPLEMENTATION SHAPE:
  the field is shared with AdmittedAot — the implementer reads ALL consumers
  and picks the narrower change: if every consumer tolerates semantic binding,
  change the shared field; otherwise version the certificate struct (ShadowAot
  V2 semantic, AdmittedAot keeps V1). Wrong-certificate hostiles at the
  evaluator AND on-chain verifier levels ship with it (SER-ACCEL's exist only
  at the generator). Cohort-7 scope. ALSO QUEUED with it:
  checked-release-candidate.sh sets DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE
  when a selected release exists (every checked release to date shipped the
  empty fail-closed series-shadow ELF as a signed artifact) — post-freeze,
  with the seam-audit gate wire-in.
- **EMBER RULINGS 2026-08-30 (~10:20 EDT), binding:**
  Q1: no superseded-cache carve-out EVER (option b rejected outright); devnet
  stranding accepted meanwhile; the real fix is design (a) — release-set
  lineage/re-point migration — chartered as a proper design. Cohort-7 may
  proceed. Q3: option (c) ratified — perpetual CLAIM, not perpetual account:
  post-deadline compaction to a durable claim-check; market accounts close;
  the holder's right survives redeemable forever. No arbitrary actor may
  insert arbitrary delays into protocol operations. Q6: CloseReplay gated on
  the terminal receipt, shaped by Q3(c). Licenses: all five questions ruled
  (MPL/LGPL/permissive/dual-arm allowed — the project is AGPL; the OG-image
  and CSS tool stack is build-time only per ember; solana-config-interface
  allowlisted as stock Apache-2.0) — the 69-row SBOM queue closes
  mechanically. CFTC 1388: deliberately unfiled ("nothing unique to say
  about perps") — never re-raise.

## Rulings — ember, 2026-08-30 (afternoon)

- **ALL KEYS MUST TRANSACT.** "It's completely unacceptable that ANY numerator
  of keys fails to transact. That's a bomb waiting to go off for whoever's key
  fails to fit. We MUST ALWAYS ALLOW ALL KEYS." A key-dependent refusal is a
  product defect, not a tail statistic. Target: zero `find_program_address` on
  the public hot path — every bump stored at creation or caller-supplied and
  verified by `create_program_address` (the derivation is the check). The
  0.032% tail goes to exactly zero.
- **Trust should ratchet forward as state mutates.** Per-transaction
  re-verification of write-once, program-owned records is the part of the
  "trust nothing" posture that overreaches; caller-supplied data stays
  untrusted (O-016 stands). Extend the seal/verify-once pattern.
- **Multi-transaction lifecycles are acceptable** where one transaction cannot
  hold the work (e.g. fee-bearing trades). "I don't care about multi-tx
  lifecycles; if that's what we need to do."

## Rulings — ember, 2026-08-30 (evening, on the decision packet)

- **E1 protocol revenue: DEFERRED, build nothing.** "While it is on devnet it
  doesn't matter. Mainnet is a loooong way away." The as-built shape (per-
  venue fee_recipient, no protocol take) stands by default, not by
  ratification; revisit pre-mainnet. 0014 D1 closes as deferred-as-built.
- **E2 dead markets: option B — resolve, redeem, retire.** "Delete that shut
  and get rid of it; it burdens the reader with a detail that only matters to
  us." The honest-bucket work (C) is mooted by B. Execution is a devnet
  write → TRADE-2's queue.
- **E3 seal rent: leaning collector-keeps ("garbage collect it and keep it
  seems somehow fine"), burn as fallback; final call deferred** until the
  consequences are laid out. No CloseSeal implementation yet.
- **E4 unpaid-fee receivable: accepted, no deadline.** Noted discomfort about
  protocol semantics — addressed in the packet discussion.
- **E5 maker lockout: accepted CONDITIONAL on guaranteed self-cure** — "as
  long as they are always free to unblock themselves." The implementation
  must prove the debtor can always settle unilaterally, including when the
  fee recipient's token account has vanished (create-idempotent or
  equivalent). That condition is a charter requirement, not a suggestion.

- **E2 final (ember): the founder keys are gone from all wallets — the
  write-off stands.** "It's just devnet sol it isn't a big deal." The two
  dead markets remain standing as unretireable; no pre-cut retirement
  contingency; the reader-burden concern is solved editorially (0015 option
  C — the honest bucket), not by deletion.

## Rulings — 2026-08-31, ember's full-autonomy directive

- **Ember (verbatim): "You need to feel empowered to work autonomously and to
  just DO anything yourself including operating any CLI etc. Do NOT leave
  anything to me. Feel free to tear down and redeploy the markets, make new
  releases, CHANGE EVERYTHING AND ANYTHING ABOUT THE PROTOCOL."** The
  orchestrator-invented reservations (the browser-click ceremony, ember-held
  rulings that had a sound recommendation) are void. Coordination rules
  (single devnet steward per window, preserved substrates, lane.sh, named
  refusals) remain — they are anti-collision engineering, not permission.
- **E3 RULED (orchestrator, under the directive): seal rent goes to the
  closer, capped** — the funded-crank pattern; reward carved only from rent
  the close liberates; no Market's funding may receive it (SEALWIDE's
  constraint); burn rejected because it preserves the stranding. CloseSeal
  is chartered.

## Rulings — 2026-08-31 night, orchestrator under the directive

- **BASIS EVALUATOR AUTHORITY RULED: adopt BASIS_ABI_UNIFICATION_V1 option D
  verbatim (§5).** The live `ProductBasisV3` evaluator is and remains the
  sole authority for the basis wire; its ABI moves to a Lean owner with an
  emitted byte-guarded conformance corpus; degrees 2–3 arrive by porting the
  kernel's de Boor INTO it at the live wire's widths and rounding rule;
  `dclutch-liability-basis-v2-kernel` retained as a non-authoritative
  differential reference (O-005), its `product_claims.rs` (retired DCLTLNK2)
  deleted. Grounds: the assurance inversion resolves toward the code that
  runs; the 221 theorems become the spec of the live path; BASIS-ENUM's
  landed fail-closed variant already conforms. The wire-free front (commits
  1–3 + the §1.6.1 kind-tag byte-guard) is chartered NOW as lane BASIS-D;
  the wire change (accepting kind 3 + the DCLTPGT1 slot + the schema-id
  bump §1.6.2 demands) waits until the corpus and port are green AND no
  founding lane is mid-flight on the old wire.
- **RECOVERY RULED (ORPHAN_DESIGNS_TRIAGE §3.2): v1 does NOT ship
  one-attempt markets forever.** A market whose single source attempt can
  strand holder principal on a transient (~1 in 3 foundings hit one
  tonight) fails the E5 standard — the lockout there was accepted only
  because self-cure is guaranteed; a welded-shut recovery ladder has no
  self-cure. Disposition: funded FailNext over RecoveryPolicyV2
  (MAINNET_STATE_RELAY §13's shape) is CHARTERED as the LIVENESS successor,
  post-cohort-8, pre-mainnet-mandatory; devnet's one-attempt state is
  tolerable and stays honestly documented. Not built tonight — weeks-class.

### Consequences recorded — 2026-08-31 night (LEDGER-TRUE)

Not rulings. Facts a future lane must price in, verified against the tree
rather than taken from a lane's report.

- **A General register-bank widening re-digests General's whole settlement
  substrate, so GEN-SEVEN-class work re-publishes it at the next cohort cut.**
  The mechanism is real and was already written down independently at
  `decisions/0006-family-neutral-hot-dispatch.md:211-213`: *"a root-lifecycle
  scalar changes `GENERAL_HOT_COMMON_SCALARS_V3`, which moves the bank width,
  the page count, and every artifact digest in the family."* Verified at HEAD:
  the width constant is `crates/dclutch-general-adapter-contract/src/hot_candidate_v3.rs:26`
  (`= 90`), and it is read off the wire at **byte 12** as a `u16` —
  `crates/dclutch-transition-vm/src/v3.rs:146`, `common_scalars: read_u16(bytes, 12)?`
  — so the byte the report names is the right byte. Because every artifact's
  identity is the digest of bytes containing that field, widening is not an
  edit to one artifact; it is a new identity for all of them, and identities are
  what the publication pins.
- **Three corrections to how that consequence was reported, each of which would
  mis-size the work.** (1) **The seven are ACTIONS, not artifacts** —
  `GENERAL_ACTION_PROGRAM_COUNT_V3 = 7`
  (`crates/dclutch-general-adapter-contract/src/release_v3.rs:49`). Each action
  carries **nine** artifacts (`GeneralSelectedBundleV1`: descriptor, account
  profile, lifecycle policy, request profile, strategy, certificate, admission,
  transition, effect), and the publication is pinned at **68 records** —
  `assert_eq!(records.len(), 2 + 9 * GENERAL_SELECTED_ACTION_COUNT_V1 + 3)`
  (`crates/dclutch-operator/src/general_selected_release_v1/tests.rs:380`). The
  blast radius is an order of magnitude larger than "seven". (2) **Nothing is
  DEPLOYED.** GENPUB published and finalized the records, but *"No root created:
  the founding refuses first"* — `0x5182 ClaimsFoundingSbfErrorV5::Release` at
  the DCLTGMF3 Open leg, family-independent (`SESSION_STATE.md:792`). So this is
  a re-publication cost, not a migration of live state, and **it is cheapest
  now**, before anything is activated against those digests. (3) **The widening
  has not happened.** `GENERAL_HOT_COMMON_SCALARS_V3` is still `90`, and a
  pickaxe over all refs returns exactly one commit ever touching it (`3aaa20fe`,
  the original binding). This is a forecast, not a report of an event.
- **What to do with it:** whoever widens the bank owns the re-publication of all
  68 records in the same change, and should land it before General's founding
  wall (FOUND-5182) clears — not after, when the substrate has live dependents.

- **SPLINE APPORTIONMENT RULED (orchestrator, on BASIS-D's measurement):
  cumulative-floor is the spline rounding rule.** The option-D directive
  "adopt the live rounding rule" was under-determined for splines: the live
  floor-plus-complement rule is well-defined only because the graded family
  structurally reserves its last claim; a spline reserves nothing, and a
  literal transliteration pays rounding residue to a claim whose de Boor
  weight is exactly zero. Measured (11 cases, both degrees): cumulative-floor
  keeps every claim within one atom of its exact share and preserves
  zero-outside-support; floor-plus-complement does neither (2/11 diverge,
  worst 2 atoms). Binding on the commit that first accepts a kind-3 body;
  both implementations ship measured in `aac98afd` — the wire commit blesses
  cumulative-floor and deletes the other.

- **GEN-SEVEN-2 choices recorded (landed 1efac500/42c0a631/3250af18):**
  (1) CloseBatch's artifact requires an Active root — stricter than the
  pure fn (which admits Retiring); discipline: begin_retiring only at zero
  open batches. (2) batch.max_orders := config.max_orders_per_candidate.
  (3) Batch windows derive from config + trusted CurrentSlot — never
  caller-supplied. (4) V3 admission requests ride the V2 carrier with
  result-bump required zero; widening the carrier is VerifyCandidateRow's
  named prerequisite. (5) Root data-effect grant is action-selected to
  exactly the two root-writers. Re-digest inventory: all 9x7 settlement
  records + descriptors + ProgramSet + seal re-publish at the cut that
  ships fourteen actions (publication 68→131 then, not before); config
  record alone survives. Nothing deployed strands.

- **GEN-SEVEN-3 choices recorded (landed 001dc90c/03e70826/62181cd5/
  08a73840; General at 12/14 actions):** (6) PlaceOrder pins valid_until
  == batch settlement_close_slot exactly (the one coordinate that could
  strand escrow past every window; makes ReleaseOrder batch-free). (7) No
  optimistic revision in order grammars — replay is address occupancy.
  (8) Claims-refund row count unpinned — an omitted row fails closed at
  the zero-vector Position close. (9) Subject honesty unproven in Hot by
  design: a wrong subject yields an unfillable order, escrow maker-
  recoverable. (10) Per-outcome terms rows ride the runtime bank channel
  (profile grammar refuses item ops in dynamic-span profiles); the future
  invocation path owns terms-to-bank fidelity. OPEN for the fourteen-cut:
  ControllerActionV3::CloseCandidate (tag 14) ships with them or not —
  ruling needed at that cut. Critical path to EXECUTION: the runtime-
  dispatch unit (invocation_v1 V3 topology + accelerator bank paths).

## Consequences recorded — 2026-08-31 morning (the complete-life drive)

- **WALL 22, the night's biggest protocol finding (LIFECYCLE-REDEEM):
  `CloseMakerReplay` is ENCODER-ONLY** — the Direct dispatch refuses it in
  two lines, the selector table has no entry 11, and both counter-writing
  transitions are add-only (the released one structurally incapable of
  decrementing). The zero-open-maker-roots retirement gate is enforced in
  FIVE independent places with no override, and selected_release_set has
  no setter — so EVERY market filled under a release set without the
  action is PERMANENTLY UNRETIRABLE, rent unreclaimable; building it
  helps only markets founded after the cut. The Lean model already
  specifies the decrement and proves the invariant: spec-vs-
  implementation divergence, 9-11 pieces, a RELEASE-SET change.
  **COHORT-9 CHARTER (in order): (1) CloseMakerReplay end to end;
  (2) ZeroBump seal recovery (the cohort-6 stranded seal); (3) General's
  14-artifact re-publication when the runtime-dispatch unit lands.**
  Devnet's stranded state is acceptable per standing ruling; pre-mainnet
  this is mandatory.
- The first complete redemption: collateral round-tripped 550,250,000
  atoms to the atom; the first market ever to satisfy CoreBeginRetiring's
  zero-claims gate. Life table: 82 acts, residual +0, drift +0.

- **HELIUS KEY RULED (ember, 2026-08-31 morning): not compromised; rotate
  on an appropriate schedule as mitigation.** On-disk keys are fine and
  local printing is fine here — the file already lives on the filesystem
  and the key carries strict spending limits. No lane re-raises this.
- **COHORT-9 AUTHORITY (ember): any bumps and any/all breaks needed to
  make things live are authorized.** Plan review chartered to a Fable
  lane before the design-sensitive items build.

- **FRAC-RULE §17.8 SIGNED OFF (orchestrator, veto window exercised):**
  ruling 2's removal of TradingCallerAuthority from the compaction frame
  is APPROVED — the root's signature (ruling 1's extended gate) is the
  strictly stronger Trading anchor, the native sibling requires nothing
  from Trading, the close is owner-signed and deadline-entitled, and
  witness w8 pins that a no-Trading entry still refuses at the burn
  hand-off. "Trading-composed" for this route = composed for SIGNATURE,
  not authority. Witnesses w1-w8 are binding on the builder.

- **CLOSEMAKER's RETIRING AMENDMENT BLESSED (orchestrator, veto window
  exercised):** the four begin-retiring count gates relax (incl. the
  fourth site the review missed — the transition bytecode in release
  content); the invariant stands unmoved and Lean-proved
  (retired_requires_zero_open_makers; begin_retiring_admits_open_maker_
  roots; close_conserves_fee_receivable — a close is never the event
  that ends a nonzero obligation). Donation slice provisionally 0 (all
  principal to rent_owner) pending ember's ruling 1 — the refusal
  alternative rejected as a 1-lamport permanent-stranding grief.

- **CANONICAL-GENERATION MANDATE (ember, 2026-08-31 afternoon, on the
  Talisman panel refusal):** "it seems like we could be doing better to
  be generating from something canonical." Standing design rule for
  every client expectation: an expectation is either (a) DERIVED from
  chain state and verified for internal consistency the way the chain
  verifies it, (b) GENERATED from the single Lean/Rust author with a
  byte-identity gate, or (c) one of the irreducible roots (program ids,
  decode-grammar versions) — and (c) gets release-aware selection with
  self-describing refusals ("this build predates release X") rather
  than schema accusations. Hand-carried pins are a defect class, not a
  style choice. CANON lane executes this against the surface PANEL-FIX
  names.

- **FRACCHECK 50th-ACCOUNT RULED (orchestrator): the Rent program joins
  the compaction frame NOW (49→50), before the cut freezes the
  declaration.** Grounds: the route decodes a LifecycleRentCreditV2 it
  cannot fully authenticate (PDA derivation under the Rent program id
  unproven — the same class as the raw/staging lesson: a record is not
  authenticated by its content alone); one read-only account is cheap
  pre-cut and a re-digest after. FRACCHECK-7 implements it with the
  campaign; authenticate_rent_credit runs; the foreign-credit refusals
  stay.

- **PROFILE-RULE BLESSED (orchestrator, veto window exercised):
  ProfileV2 succession rides the cohort-9 cut** (f985bede; P-008
  documented). Slot-tolerance REFUSED AS UNSOUND — it would let the
  deployer key alone put arbitrary bytes behind every route while the
  authentication returned a digest no longer true. The ceremony:
  DeclareSuccessor's conjunct geometry applied to the infrastructure
  pair, V1 never mutated, consumers read V2 only. Resolution-proof
  JOINS the redeploy set (else every cohort-9 market is unresolvable);
  Custody = the unmoved role exercising d6e43b11's fixed arm; Rent's
  pent-up debt ruled OUT of this cut (deferral now a decision). For
  ember, non-blocking: the ceremony's dual-signer estates question
  (mainnet-era) and Rent's deferral list (§9 of the ruling).

- **FRACCHECK-7's rulings answered (orchestrator):** (1) the invented
  seam verdict tag `benign-typed-nonzero-wire` is CONFIRMED — an
  upstream private-constructor guard is not the "fails downstream is an
  argument" class, and filing it as hazard-unset-pin would claim 19
  unguarded frames where the tree has 18; the tag now exists for the
  next honest case. (2) The Economic error-collapse (inner refusals
  flattened to one code on the compaction route) is QUEUED to cohort-9
  polish — codes are additive and CEILINGS' exhaustive bands make the
  widening safe, but it is a refusal-surface change and rides a
  deliberate commit, not a debug session. (3) The opener-shortfall
  economics (one crank leaves the opener 1,348,376 lamports short with
  zero residue — the amended order working as designed; multi-crank
  markets repay progressively, single-crank markets never do) is put
  to EMBER as an economics ruling: accept as the cost of opening, or
  redesign the order so the opener is made whole before the cranker.

- **TICKET-BOARD's two decisions CONFIRMED as rulings (orchestrator):**
  (1) the board keeps no clock — expiry filters responses at the
  caller's slot and never mutates state; the alternative hands any
  caller a lever to expire everyone else's offers, the one power a
  relay structurally lacks and must keep lacking. Cost accepted: no GC,
  reclaim by restart. (2) A full board refuses (BOARD_FULL) rather than
  evicts, same reason. Also binding on the site: the signature chip
  says WELL-FORMED, never verified — the TS decoder does not check
  Ed25519 (the board does, at admission) and a test forbids the word.
  Known limit for any public deployment: no rate limiting — loopback
  default stands until that rung is built.

- **CLOSE-DRIVER's verdict tag CONFIRMED (orchestrator):
  `checked-caller-excludes-payer`** — for a payer-exclusion check that
  lives across a crate boundary no proximity reader will bridge: the
  tag says the standing question was asked, answered yes, and closed
  in code; `hazard-*` would send the next reader to redo finished
  work. Second honest tag this session (after
  benign-typed-nonzero-wire); the register's vocabulary grows only
  when a finding genuinely fits nothing — both did. Also binding: its
  discipline of fixing the UNFLAGGED bump-bearing derivations beside
  the flagged ones ("retiring a finding while the tuple stays spelled
  retires the finding, not the defect").

## 2026-09-01 — MOST SERIOUS FINDING: the redeemer chooses the payout matrix
## on generic terminal settlement (unreached route, no loss, no PoC)

**Scope first, so this is not overstated.** The route census reports **0 of 159
routes ever executed**. This is a defect in an UNREACHED route, not evidence of
any loss, and no proof-of-concept was built or run. The claim is strictly about
what the code paths admit when read, verified line by line by the reporting
lane and re-verified rather than taken on a subagent's word.

On `terminal_settlement_v3` (dispatched at `claims-sbf/src/lib.rs:416`), the
**redeemer supplies the Product-to-Claims exposure** — the matrix deciding
which claim gets paid. `exposure_id` / `exposure_digest` are ordinary
instruction fields (`claims-svm/src/terminal_settlement_v3.rs:151-154`).
`authenticate_finalized_record` proves only PDA-from-(schema,digest), owner,
non-signer/writable/executable, `hash(bytes) == digest`, rent exemption and
vacant staging — **no market, no product**. `verify_execution_for` joins five
32-byte fields that the record's own author writes, plus two widths. And
Registry record publication is explicitly permissionless.

**The check that looks like it would catch a substitution is a TAUTOLOGY:**
`bundle_id` is assigned from `admission.selected_id` (`exposure.rs:274`), and
the adapter sets that from `input.exposure_id`
(`terminal_settlement_v3.rs:393-401`) — it compares the instruction to itself.
Same family as "the builder as its own witness" below: a comparison whose two
sides move together proves only that they move together.

Founding pins nothing — `generic_founding_v1.rs` contains zero occurrences of
exposure/graph/composition/descriptor — so there is no upstream recipe for the
redeemer's choice to be measured against.

**A written premise in a decision document is false for one of the two routes
it governs.** Decision 0011 (`docs/decisions/0011-...:510-522`) recorded that
the live route "checks the bundle's identity, digest and width and never the
coefficients", and judged that tolerable because *"a wrong recipe is a wrong
founding rather than a forgeable request."* True for `rational_terminal_v3`,
which takes the exposure identity from an authenticated
`RepresentationDescriptorV2`. **False for generic settlement.**

What the solvency refusal does and does not cover: `product_basis_terminal_v3.rs:442-444`
caps the SUPPLY-WEIGHTED sum at the Hoard balance, so aggregate over-payment is
caught. Under-payment, and paying the right total to the WRONG coordinate, are
not — the inequality says nothing about which claim is paid.

**CLOSED `a968858c`, as a VALIDATION change — no wire, no release event.**
`require_identity_exposure_v3` refuses any exposure that is not the identity
embedding, before a payout is derived or a byte written, and all four entries
into the route funnel through `authenticate_and_prepare` so one check covers
them. It is admissible precisely because this route CANNOT express `N != K`:
the Product's `basis_width` is forced equal to `market.claim_count` both at
settlement and at founding, which is the sole creator of every LBV2 aggregate.
The canonical publisher already emits exactly the identity and says so in its
module doc — so this moves an invariant the tree stated in PROSE into a place
the chain enforces. New refusal `ClaimsSbfError::ExposureNotIdentity = 0x500E`,
band 5, append-only; census green at 297 codes.

**The tautology is deleted, and the obvious replacement was MEASURED FALSE.**
Reading the record's own `graph_id()` reddens four fixtures, because the
exposure record's `graph_id` header and the descriptor's `graph_id` are
DIFFERENT IDENTITIES in this tree — the double-booking already filed as a
RECORDS-MIGRATE row. The lane reverted rather than rewrite four tests to match
semantics it could not prove. That row is now the blocker on giving generic
settlement a real identity join, not a tidiness item.

**What is still owed, and it is ember's:** pinning an exposure digest at
founding, the way the descriptor pins it for Rational, needs a new persisted
field in the LBV2 aggregate — a wire change and a release event. Until then the
invariant is *"the only admissible matrix is the identity"*, NOT *"the matrix
was chosen by someone other than the redeemer"*. Those are different
guarantees and the weaker one is what currently holds.

The hostiles are the right ones: every hostile is a CANONICAL record that
round-trips the kernel's own encoder, because the attack was never a malformed
record but a well-formed one stating a different recipe — sum-preserving
permutation (the case solvency structurally cannot see) and scaled denominator
(under-payment, which solvency also admits) among them. Eight mutations, six
killed at the intended discriminant; two survivors documented rather than
hidden, being two conjuncts redundant with each other, with the test renamed to
name the conjunct that actually owns it.

## 2026-09-01 — THE CLASS: declarations never executed against reality

Three separate lanes convicted the same shape tonight, in three families, none
of them looking for it. A declaration or guard is authored in one place, is
never run against a real account, and is therefore *unsatisfiable by anything*
— which reads as strictness and is actually a route that cannot execute:

1. **Direct Hot** — `authenticate_lifecycle_credit_v3` pinned the lifecycle
   rent credit's owner to the fixed Registry coordinate on a written premise
   ("that owner is the already authenticated fixed Registry coordinate") that
   measurement falsified: the RENT program owns it. Unsatisfiable by every
   honest transaction. Repaired `ff8ca269`.
2. **Dealer** — `derivation_policy` is pinned per-descriptor to that
   descriptor's own lifecycle digest AND per-root to the manifest entry, and a
   multi-selector set cannot satisfy both. Selector 9 is unexecutable for ANY
   manifest. Convicted on-chain; migration is ember's.
3. **Structured** — the account profile declares `Exact{0}` for sixteen
   coordinates including the RENT SYSVAR (17 bytes observed) and the SYSTEM
   PROGRAM (21 bytes). Nothing can make a sysvar zero bytes.
   `open_structured_v3.rs:617` vs `:625`: index 15 is in the `executable` set
   but missing from the `opaque` set — every other executable index (6, 19,
   21, 23, 27) is in both, so that lone asymmetry drops the System program to
   `Exact{0}`. `:634-643` then takes `fixed_data_lengths[index]` verbatim for
   everything not overridden, and that array is all zeros except [4] and [29].
   **This ships**: production `structured_selected_release_v1.rs:646-656`
   builds the same array, and the route reaches
   `validate_accounts_with_dynamic_spans` from `hot_v3.rs:12531`. The lane
   tried to falsify it and could not — the only other consumer checks encoding
   and digests only. These widths had never been run against real accounts
   anywhere; this campaign was the first to try.

The common cause is not carelessness, it is that **an authored declaration and
a real execution never met**. This is the canonical-generation mandate's
sibling (WAVE `c2eb4f63`): there, an expectation must be derived or generated
rather than hand-carried; here, a declaration must be EXECUTED before it can be
believed. A component test that checks encoding and digests will pass forever
over a route no account can satisfy.

**The design principle the Structured lane extracted, which is the useful
generalisation:** an account profile is part of a RELEASE ARTIFACT, authored
before any market it will ever meet exists. So the correct split is not
created-vs-read — it is **knowable at authoring time, or not**:

- Knowable (a protocol constant, or a function of a release input like K) must
  be declared EXACTLY. Coordinate 22 is `market_core_codec::STATE_BYTES`;
  the Claims aggregate is `header + claim_count*8`; the capability root is
  `header + root_state_bytes`, already an input. Typing these as literals, or
  leaving them zero, is a promise the release cannot keep.
- NOT knowable — a width that is a property of the market rather than the
  release (raw product, portfolio, descriptor, graph, result-domain records) —
  must be OPAQUE, which is exactly why coordinates 25 and 26 already are. Those
  are authenticated by record digest instead, which is the stronger check.
- Builtins (the Rent sysvar, the System program) are trivially not knowable as
  zero and must be opaque too. The live defect was one asymmetry: index 15 sits
  in the `executable` set but not the `opaque` set, while all five of its peers
  (6, 19, 21, 23, 27) are in both.

Stated as the lane put it: *a release is authored before the market it will
meet — a promise about widths made by something that had not yet seen them.*

**FOURTH INSTANCE, and the purest one — Series' `Creation` compartment.**
`FundingCompartment` has exactly two `NativeLamportsOnly` members, `Rent` and
`Creation` (`funding.rs:291-316`). A founding parks both; Core's
`validate_native_custody` requires the exact declared total present;
`activate_in_place` releases both and RETURNS
`ActivationDebitV1 { rent_lamports, creation_lamports }`; `release_in_place`
then refuses Rent and Creation forever. And `outer.rs:1606` **discarded that
return value**, pinning the root to `rent.minimum_balance(...)`. So a nonzero
`Creation` quote was simultaneously unactivatable (the ledger poststate wanted
the lamports gone while the effect moved only rent) and unreleasable. A
declared, authenticated, WIRE-VISIBLE compartment with no transport and no
reader — General pins it to zero explicitly, which is why nothing had noticed.
Repaired `e75b279c`, with the control that families declaring no creation
principal keep byte-identical artifacts.

**FIFTH INSTANCE — and it confirms the axis exactly.** The structured
transition fold (`open_structured_v3.rs:918-921`, surfacing at
`bundle-builder/registers.rs:566`, which only wraps the error) is `4 + K`
instructions. Measured: it refuses at op index 4, the first per-row
instruction, because that instruction is
`scalar_eq(coefficient[row], DENOMINATOR)` — **every coefficient must EQUAL
the denominator**. Corpus `COEFFICIENTS = [2, 3, 5]`, `DENOMINATOR = 7`, so
`2 != 7` and it refuses. The kernel that owns the descriptor enforces only
`denominator != 0` and not-all-coefficients-zero; there is no relation between
a coefficient and the denominator anywhere in it. The check admits only a
degenerate basket where every coordinate has exposure exactly 1.

The axis predicts this precisely: the four instructions that DO pass are the
structural ones — counts nonzero, asset count equals outcome count — which a
release genuinely knows at authoring time. The fifth asks a MARKET's arithmetic
(coefficients and denominator are lowered per market from the composition
exposure) of an artifact authored before any market exists. Same file as the
width defect, same absent test: the only other consumer builds the artifact and
checks its encoding and digest, and contains no `execute_fold_atomic` at all.

A LEAD, explicitly not a claim: three independent fixtures satisfy
`sum == denominator` where none satisfies `each == denominator`, which is
suggestive of an intended partition-of-unity authored as a per-row equality.
But the kernel enforces no such rule either, and the corpus's `[2,3,5]` sums to
10 rather than 7 — so a corrected sum check would still refuse it, and there is
a fixture question immediately behind the route-owner question.

**A HEDGE IN YOUR OWN REPORT IS A TASK, NOT A DISCLAIMER.** The Dealer lane
took back four claims across the night — a CU figure, a refusal code, a
"necessary and not sufficient", and a cfg-gating inference. It caught three by
re-measuring something it already believed. The fourth it caught only when
asked to close a gap it had itself labelled "leading candidate" and moved past.
Its own words: *that label was the tell; I should have treated my own hedge as
a task rather than a disclaimer.* When a report says "likely", "candidate" or
"probably", that is the sentence to go back and settle before anyone builds on
it — most cheaply by the lane that wrote it, while the evidence is still warm.

## THE SECOND CLASS: guards whose two sides move together

Three instances tonight, in three unrelated layers. Each looks like a check and
proves only self-consistency:

1. **Claims terminal settlement** — `bundle_id` is assigned from
   `admission.selected_id`, which the adapter sets from `input.exposure_id`.
   The comparison that appears to prevent exposure substitution compares the
   instruction to itself.
2. **The Series activation validator** — it rebuilt the profile with the same
   builder it was validating, and the projection helper EMULATED the
   compartment reads instead of running a real observation. A mutation deleting
   the projection stayed green; on chain that bundle would have stranded a
   principal nothing could release. Fixed by reading the DECODED operation list.
3. **The browser's capability board** — `implementation` was a string someone
   typed, and `operatorSurface.test.ts:207` asserted the same string back
   (`toMatchObject({ implementation: 'browser-wallet' })`). So changing what the
   browser CLAIMED was a two-line edit, while changing what it DID changed
   nothing at all. Replaced by derivation from the app's own import graph, with
   no status field remaining (`d71113e4`).

The test for this class is one question: **could this assertion fail if the
subject were wrong?** If both sides are computed from the same source, the
answer is no, and the check is decoration. It is the same defect as a vacuous
`P -> P` theorem — green, and about nothing.

**A related test hazard, worth its own name: THE BUILDER AS ITS OWN WITNESS.**
While mutation-proving that repair, one mutation ("the profile never projects
`Creation`") stayed GREEN — because the validator rebuilds the profile with the
same builder it is validating, and the projection helper *emulates* the
compartment reads instead of running a real `AccountObservationV1`. On chain
that bundle would read a zero scalar, move only rent, and strand a principal
nothing could ever release. A test whose fixture and subject share a builder
proves only that the builder is self-consistent. The fix was to read the
DECODED operation list instead, which turns the mutation red. This is the same
family as a vacuous `P -> P` theorem: green, and about nothing.

## 2026-09-01 — DIRECT: the registered family has no route, and the manifest
## defect is confirmed across two families

- **`hot_v3.rs:5372` refuses every Direct-kind Hot action except
  `InlineOrdinary`.** RegisterSell, RegisterBuy, the fill, both cancels,
  expiry, every close, both splits and merges — none has an admitted
  generic-Hot route at all. Measured `UnsupportedContent` (0x4000) at 323,523
  CU, 451 CU past the `preflight-children` checkpoint on a profiled build, so
  the action is a complete admitted preflighted act at the moment its KIND is
  rejected. Behind a probe returning `Ok(None)` there, the registered Sell
  **executes at 374,455 CU** with exact root/maker/record poststates and Claims
  untouched — the first registered Sell ever created on a chain. The probe was
  NOT committed: it is a refusal relaxation and needs a ruling.

- **SCOPE SHARPENED by the lane that first reported it (it had understated
  its own finding): the ordinary lifecycle is a THIRD distinct record.** A
  Direct market founded to trade inline pins the ordinary policy in its root
  header, so it cannot admit ANY registered action even after the action gate
  opens. This is not Sell-versus-Buy; it is every-action-versus-every-action.
  **But it may need no contract change at all.** `StateLifecyclePolicyV5`
  already selects plans BY ACTION — `action_plan_count(action: u32)` is on its
  public surface (`lifecycle_v3.rs:778`, verified) and `hot_v3` already carries
  `selected_action` into lifecycle selection. One Direct policy per market with
  an action-keyed plan bank would satisfy `validate_selection` for every action
  under one root, with `derivation_policy` never disagreeing. What decides it
  is whether the RENT-QUOTE BANK can be safely unioned:
  `LifecycleCurrentRentQuoteV5` carries `exact_data_len` and a scalar
  destination, has no action field, and is addressed by ordinal. **That is step
  one and nothing else should start before it**, because it decides whether
  this is a codec change or a contract ruling.

- **The manifest/derivation-policy defect is NOT Dealer-specific — CONFIRMED
  in Direct, independently.** A capability root persists ONE manifest-entry
  index, and Buy's `LifecyclePolicyV5` carries two more rent quotes than
  Sell's (the Custody replay and vault), so different width -> different digest
  -> different `derivation_policy`. **One Direct root cannot admit both
  creation actions.** Convicted by controlled experiment: mint the manifest
  entry from the Sell descriptor and the Sell executes while Buy refuses
  `Content` at 133,173 CU; rebuild from the Buy descriptor and the refusal
  moves to the Sell at 125,433 CU, same band, nothing else changed. This is the
  Dealer lane's finding reproduced in a second family by a different route, so
  the ruling above is protocol-wide, not a Dealer quirk. Padding Sell's
  lifecycle to match Buy's would make it quote a vault it never opens.

- **`reserved_claims` is a CAP, not a reservation — and must never be shown as
  solvency.** A registered Sell escrows NOTHING: `register_intent_v2` writes
  `reserved_claims = maximum_fill` into the record, the Sell `EffectProgramV4`
  has zero routes, and the Sell profile's 13 coordinates include neither a
  Claims Position nor the aggregate. A registered Buy genuinely escrows (three
  Custody routes move `reserved_collateral` into a record-keyed
  `TradingPrincipal` vault and drain the delegate allowance to zero). So: one
  maker may register N Sell records each reserving full supply; a resting Sell
  can become unfillable for free at the taker's CU expense; and
  **`sum(reserved_claims)` over live records is not bounded by supply and is
  not a conservation quantity.** The browser and the simulator must never
  display it as one. Real conservation is enforced at fill, where
  `claim_custody_debit: fill` moves actual claims and rolls the whole
  transaction back if they are gone.

## 2026-09-01 — GENERAL: nothing writes the root register, and 14 of 15
## actions cannot pass Trading's geometry

- **Nothing writes `identity::GENERAL_ROOT` (register 27).** Every General
  lifecycle state recipe in `state_seeds_v3.rs` — Batch, Order, Selection,
  Settlement, Candidate, Verifier, VerifiedCandidate, Terminal — seeds on
  `CommonIdentity(GENERAL_ROOT_IDENTITY_REGISTER_V3)`, and nothing populates
  it: OpenBatch's twenty AccountProfile operations are fully enumerated, and
  the trusted environments supply only `CURRENT_SLOT`, `TRADING_PROGRAM` and
  `RESULT_OWNER`. Measured on chain: **32 zero bytes.**
  `general_hot_v3.rs:2238-2243` compensates HOST-SIDE, so the artifact derives
  a rootless PDA while `GeneralStateAddressSeedsV3::batch` refuses a zero root
  outright — the seed helper and the lifecycle policy disagree about whether a
  rootless General state exists.
  **Why it hid, and this is its own hazard:** the accelerator's admission joins
  `STATE_BUMP` to `PRIMARY_CANONICAL_BUMP` and never compares the two
  ADDRESSES. While the bumps differed it refused (`InvalidCoordinate`, 255 vs
  254); once an unrelated fixture fix made the bumps collide, the guard went
  SILENT while the addresses stayed different. A guard on a derived byte is not
  a guard on the thing derived.

- **General's artifacts cannot satisfy Trading's `require_geometry` — 14 of
  General's 15 actions cannot execute through Trading Hot at all.**
  `artifacts_v3.rs:620,634` DELIBERATELY requires
  `account.item_account_stride() == 1` and `effect.item_account_stride() == 0`,
  because under Profile13 the item-rule table is the dynamic fixed-span
  template bank rather than a Product-N account stride. `hot_v3.rs:12237`
  requires them EQUAL, with no dynamic-span case — though the same function
  already special-cases dynamic spans when counting logical accounts. Both
  cannot hold. Only `CloseCandidate` passes; Direct passes only because both
  its strides are zero. Measured `Content` (0x4003) at 370,977 CU, localized
  between checkpoints `runtime-observations` and `p5-geometry-rent`; the frame
  was ruled out first (host and chain agree on 12 logical accounts, 4 scratch
  pages, 4 chunk authorities, a 58-account v0 route).
  The lane landed a test RED ON PURPOSE naming it
  (`general_dynamic_spans_v1.rs`), which converts an undifferentiated on-chain
  refusal into a one-second host assertion. That is the right shape for a wall
  someone else must resolve.

**THE PATTERN ACROSS THREE FAMILIES.** Direct: every Hot action except
`InlineOrdinary` has no admitted route (`hot_v3.rs:5372`). General: 14 of 15
actions cannot satisfy the geometry check. Dealer: the manifest can describe
only one selector. Each family authored a full action set; the generic Hot
route admits a small fraction of it. These were found independently, by three
lanes, in one night — so the question for ember is not three bugs but whether
the generic route's admission surface was ever intended to carry what the
families declare, and what the plan is for closing that distance.

## 2026-09-01 — open seams from the completion swarm

Two jointly-unsatisfiable constraint pairs were convicted by measurement
tonight. Neither is an economic ruling; both are architecture decisions with
cross-family reach, and both are being analysed by their convicting lane
rather than parked. Recorded here so they are not lost if a lane dies.

- **SERIES activation funding seam.** Option B (Template-authenticated
  `closeRent` is separately prepaid principal) is refused by the
  family-neutral seam: `outer.rs:1639-1646` requires an activated root to end
  at exactly `rent.minimum_balance(descriptor.root_account_bytes())`, a pure
  function of declared width with no family-varying term, while
  `series/terminal.rs:132-139` refuses Close unless the root holds
  `root_rent + close_rent_remaining`. So a Template with nonzero `close_rent`
  describes a root activation may not fund and Close can never open, and
  nothing in between tops it up. QUESTION: does the activation seam gain a
  family-declared prepaid principal (changing `outer.rs` plus the shared
  activation codec, and requiring the outer to authenticate an amount against
  a config record it deliberately never decodes), or does Series' prepaid
  principal move out of the root? No zero-principal rule was imposed; the
  bundle composes the honest tail either way and a verdict artifact reports
  the wall instead of shipping roots that cannot be closed (`01e866b0`).

- **DEALER manifest vs per-descriptor derivation policy.** `derivation_policy`
  is pinned per-descriptor to that descriptor's own lifecycle digest
  (`hot_v3.rs:3370`, `:1235`) but `validate_selection` pins it to the per-root
  manifest entry (`crates/dclutch-capability-program-contract/src/v4.rs:201`),
  and a `CapabilityManifestV1` carries ONE entry per root while a
  `CapabilityProgramSetV2` carries many selectors whose lifecycle bodies have
  necessarily different widths. Exactly one selector can ever satisfy it.
  QUESTION: give each selector its own manifest entry (possibly the only
  coherent answer, since a single-entry manifest may be structurally incapable
  of describing a multi-selector set), or stop pinning `derivation_policy` to
  the per-action lifecycle digest? A persisted-layout change is a much larger
  act than a validation change; that asymmetry is the crux.

  **UPDATE — measured on-chain, and the answer got harder.** Per-selector
  manifest entries were shown UNREPRESENTABLE (`entry_index` is a PDA seed of
  a write-once root header, and `validate_manifest` needs entries strictly
  ascending by `kind_id` while all nine Dealer selectors share one kind), and
  so was a shared lifecycle body (coordinate 6 is the LP Position in the LP
  frame but a custody-transfer account in the equity frame — a shared body
  would name the WRONG ACCOUNT). Selector 9 is unexecutable under R2 for ANY
  manifest, two-branch: carry the per-root constant and R2 refuses at
  `hot_v3:3370`; carry anything else and R3 refuses at `:3318`.
  An isolated spike convicted R2 on-chain — with R3 satisfied for every
  selector, R2 alone stops the first Hot execution with `UnsupportedContent`
  (0x4000).

  **CORRECTION, and it simplifies the picture: an earlier entry here claimed
  dropping R2 was "necessary and not sufficient" because of a fourth binding
  in the activation cache. THAT WAS WRONG and is withdrawn.** There is no
  fourth binding. The `0xd001` came from R2's OWN second site
  (`hot_v3.rs:1235`), reached through the accelerator — the spike had dropped
  R2 only in the Trading tree, never in the tree the accelerator builds from.
  With both corrected the Dealer Add clears the entire seam: 166,768 ->
  **582,773 CU**, past `artifacts-strategy-effect`, LP Open executing, the
  substituted-identity hostile still refusing `Content`, and all ten rollback
  keys byte-unchanged. **R2's three sites are the whole runtime story; they
  merely span two ELFs.** (`capability-activation-codec:551` is a real site but
  is a self-consistency check inside the release compiler — the whole crate is
  `cfg(not(target_os = "solana"))`, so the SBF artifact cannot reach it.)

  **What actually makes this a release event** is not the site count. The field
  sits at offset 144 of the fixed 600-byte V4 record. It is NOT a direct PDA
  seed — but the capability-root seed tuple is
  `[domain, market, generation, manifest, entry_index, kind, capability_release, config]`
  and seeds 4 and 7 are digests that CONTAIN the field. So changing its value
  transitively **moves every capability-root address**: existing on-chain
  markets cannot be migrated in place, they must be re-founded. Persisted
  carriers beyond the descriptor: the Market manifest account (offset 160),
  selected-release publication ids, the Series-Shadow bundle manifest, and —
  newly found — `HotExecutionAckV3.execution_digest`
  (`direct_finalization_v3.rs:438`), which is UNGATED and lives in the Trading
  ELF. Grouped sweep: ~84 host-side producer sites, ~43 runtime validation,
  ~60 persisted/digest-bearing, ~205 tests/Lean/TS/docs.
  EMBER RULES THE SCOPE; no lane may start it. Spike patch preserved, nothing
  landed in the shared tree.

## 2026-09-01

- **Codex's completion wave landed and handed back.** `AGENTS.md` +
  `docs/MASTER_COMPLETION_CONTRACT.md` (C-00..C-16) are now the standing
  authority; `docs/LETTER_TO_CLAUDE_2026_09_01.md` carries the frontier,
  the five honest walls and the counterfactual ten-hour dispatch board.
  GOAL.md and this file are historical ledgers from here, not queues.
  Cohort-cut vocabulary is retired in favour of checked release
  candidate -> authorized devnet flight.

- **DEVNET IS DISPOSABLE; REDEPLOY BEATS SUCCESSION (ember, 2026-09-01):**
  asked whether the never-executed `ProtocolInfrastructureProfileV1 -> V2`
  ceremony (P-008) must run before the next devnet flight, ember ruled:
  it is all just devnet — tear everything down, forget the old, and
  redeploy anew when we are ready; and there is enough devnet SOL to
  stand the new set up WITHOUT tearing the old one down first.
  Consequences: the succession ceremony is NOT a prerequisite for a
  checked release candidate or for the next devnet flight; a fresh
  deployment from exact current sources is the preferred path; the
  cohort-8 devnet programs are abandoned in place, not migrated. The
  ceremony code is KEPT, not deleted — it is the machinery a
  non-disposable deployment will need — but it is demoted from blocker
  to capability, and nothing may report it as executed until it runs.
  OPEN COROLLARY, not assumed here: whether C-01 still wants
  succession/migration EXECUTED before assurance at all, or whether
  "redeploy fresh" stands until a deployment exists that someone would
  lose something by abandoning. That one is ember's.

## 2026-09-01 — three corrections to the handoff letter's Dealer evidence

The letter's pinned Dealer wall was re-measured against its own pinned
artifacts (Trading `af5d955e…`, accelerator `3f73d43c…`, profiled ELF
`a2e62944…`, `accepted.rs` at `e1bac1e8…`, all re-verified before and after).
Three of its statements do not survive the re-measurement.

**1. The pinned CU fingerprint is stale.** Same command, same artifacts:
**149,593 CU**, not 148,093. +1,500 unexplained. Recorded as a corrected
fingerprint rather than hedged; what moved is not yet known.

**2. "A substituted-position selector-1 Add refuses correctly" is not
established.** The substituted-Position Add and the honest Add refuse at the
*identical* 149,593 CU with the *identical* 0x4003 — both are hitting the same
site, upstream of anything that could examine a substituted identity. The
hostile control passes only because the honest path is broken in the same
place. **This is the second class — a guard whose two sides move together —
appearing in a control rather than in production code**, which is worse,
because a vacuous control licenses everything downstream of it. Nothing about
substituted-position handling is currently known. Re-verify after the wall
moves.

**3. The window is right; its implication is wrong.** All 19 direct raise sites
in `hot_v3.rs:3222-3525` were instrumented with distinct custom codes and
every one is excluded. No predicate written in that window refuses. The wall is
inside a *helper called from* the tranche.

That negative result is only admissible because it carries a positive control:
an ungated marker fails LP Open at `0x9063`/557,448 CU (channel live, window
reachable), and the same marker gated to `selected_action == 1` lets LP Open
succeed at 1,059,071 CU while the Add still refuses with no marker. The first
instrumentation attempt used `sol_log_64` and produced nothing — and there were
**zero `Program log` lines anywhere in the run, including the successful path**.
The channel was dead and its silence meant nothing.

**THE RULE THIS EARNS: an absent signal is evidence only if something present
proves the channel works.** "I instrumented it and nothing fired" and "my
instrument was disconnected" produce identical logs. Every negative result in
this tree needs a positive control in the same run, or it is not a result.

Also refuted, so nobody re-chases it: this is **not** the manifest/derivation-
policy defect already convicted (that raises `UnsupportedContent` 0x4000 at
`hot_v3:3372`; this raises `Content` 0x4003).

## 2026-09-01 — the Dealer wall is one predicate, and correction 3 is withdrawn

**Convicted:** `descriptor.derivation_policy != entry.child_derivation_id()`,
in `validate_selection` (`crates/dclutch-capability-program-contract/src/v4.rs`),
reached from `authenticate_descriptor_root_selection` at `hot_v3.rs:3319`.

Bisected on the pinned evidence with gated early-return probes — one build and
run per row, every build reporting `FRAMES=0` and `S5-RECOMPILED=1`. Probes
past `authenticate_capability_seal_v3` and `decode_capability_program_boxed_v3`
fired (`0x9034`, `0x9035`); the probe after `authenticate_descriptor_root_
selection` did not, so the refusal is inside it. Splitting its two-branch
conjunct gave `0x903c` (branch A, `validate_selection`); splitting that into
seven distinct variants gave `0x904c`. One predicate, not a range.

**WITHDRAWN: correction 3 of the Dealer corrections above.** It asserted this
was *not* the R2 manifest/derivation-policy defect, on the evidence that R2
raises `UnsupportedContent` 0x4000 at `hot_v3:3372` while this raises `Content`
0x4003 elsewhere. The site and the code do differ — but **the predicate is the
same one already convicted**, reached earlier on this path.
`authenticate_descriptor_root_selection` discards the reason with `.is_err()`
and re-raises a bare `Content`.

That discard is the finding behind the finding: **it made one defect look like
two, and cost a full bisect to undo.** Same disease as `Content` carrying 2,086
raise sites (25.0% of the protocol's total) — a refusal that drops its reason
is a refusal that cannot be reasoned about. Corrections 1 and 2 stand; both
were re-measured. Addendum offered without claiming it: the clean *instrumented*
build refuses at 148,083 CU, ten from the letter's 148,093, which is suggestive
that the pinned figure came from a lightly-instrumented ELF rather than the
pristine one. `af5d955e` costs **149,593**, measured three times.

**The release-event claim, verified in source rather than inherited.**
`CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET = 144`. The field is *not* a
direct PDA seed — the tuple is `[domain, market, generation, manifest,
entry_index, kind, capability_release, config]` — but the `manifest` seed is the
manifest digest whose entries carry `child_derivation_id`, the very field
compared, and `capability_release` is the program-set digest, which
transitively covers the descriptor bytes at offset 144. So changing
`derivation_policy` **moves every capability-root PDA**: existing markets
cannot be migrated in place and must be re-founded.

### THE RESERVATION IS ANSWERED BY EMBER'S OWN LATER RULING

`derivation_policy` was reserved ("EMBER RULES THE SCOPE; no lane may start
it") **because** the change forces re-founding. On 2026-09-01 ember ruled devnet
disposable: tear down, redeploy fresh from exact current sources, abandon the
old cohort in place rather than migrating it. There are no mainnet markets.
Under that ruling, "existing markets must be re-founded" is not a cost — it is
the plan. The objection that reserved the predicate has been answered by the
person who reserved it.

Lane authority set accordingly: **implement and prove it as code; do not
deploy.** The release event itself remains ember's named act. And the standing
condition holds — if nothing else binds the descriptor to its entry once that
comparison stops carrying the binding, this is a weakening and the lane stops.

### A reusable constraint, paid for the hard way

`authenticate_and_execute_hot_v3` has **zero SBF frame headroom**. Fifteen
per-call-site `.map_err` guards produced **95 frame-overwrite diagnostics** and
the program aborted `ProgramFailedToComplete` instead of refusing; eight
out-of-line guards still produced 95; the clean tree measures 0. **Anyone
instrumenting this function must use gated early-return probes only.** The
aborts were nearly reported as measurements, and what caught them was checking
the frame count — the same move that caught the dead `sol_log_64` channel.

## 2026-09-01 — THE THIRD CLASS: the browser can sign, but it cannot originate

Three of C-12's nine capabilities came back **"present, unreachable by a
stranger"**, and they are one architectural fact wearing three hats. Each stops
at a byte-exact artifact that only a Rust binary can author:

- **maker/taker trade** — `components/JoinPanel.tsx:22-28`: admission "needs the
  position owner's signature over a frame the browser cannot yet assemble
  byte-exactly."
- **redemption** — `components/RedeemFlow.tsx:332`: "This browser never creates
  or completes a payout plan." The producer is
  `dclutch-local-successor-bootstrap wallet-terminal-payout-input`.
- **creation** — `components/CoreFoundWorkspace.tsx:56-83`: `/found` asks for
  **14 pasted base58 addresses**, five of which the file's own comments call
  derivable.

Liquidity is the same shape one step further along: it signs and never submits,
exporting to an external submitter. The browser is a signing surface bolted to
a Rust authoring monopoly, and C-12 asks for a stranger-operable product.

**The instrument that hid it, and the general lesson.** `/console` advertised
`claims.redeem` as *"This browser · one wallet signature, sent from here"* —
every clause derived from the import graph, every clause true — over an act
whose step two opens a file picker for a plan only a Rust binary authors. The
census asked what an act *does* and never asked what it cannot be *started*
without.

> **A derivation can be sound and still answer the wrong question.** Deriving
> from executable truth removes the lying-status-string failure; it does not
> remove the wrong-question failure. 12 of 218 modules cannot start without an
> outside file.

Fixed in `fba9a63e` by reading a second fact off the same import graph, each
with a canary that throws when it stops matching.

### An instrument that could not have failed

Accessibility was ungated for a measured reason, not an oversight: 171 test
files contain **zero** `getByRole`, **zero** `getByLabelText` and **zero** axe
runs, because `vitest.config.ts:12` is `environment: 'node'`. **No assertion in
the repository could observe a label association or a focus order.** That is
how "28 of 29 files nest `<Nav>` inside `<main>`" — so *Skip to main content*
lands inside the main it was meant to skip — stayed true indefinitely.

Two classes are now gated (`c94f9684`): 13 scroll regions no keyboard could
reach, and 1 unnamed control, both at zero. **The mobile repair had created the
keyboard defect** — wide tables pushed into `overflow-x:auto` became
keyboard-inert. Repairs here trade one class for another, so gate both before
moving either.

### Refused, correctly

`abi:route-census:verify` and `abi:refusal-registry:verify` are stale at HEAD
while 41 other-lane files are mid-edit. The lane refused to regenerate:
that would commit another lane's uncommitted Rust into a browser mirror — the
"browser becomes last authority" hazard running backwards. Mirror regeneration
is a convergence step for after the Rust lanes settle, not a lane's to take.

## 2026-09-01 — third instance: encode it, digest it, never run it

WAVE's sentence — *a component test that checks encoding and digests will pass
forever over a route no account can satisfy* — has a third instance, and it is
the one that explains two other findings at once.

`programs/dclutch-dealer-accelerator-sbf/program-test/tests/accepted.rs` builds
six `MultiLpCustodyRequestV3` compartment pairs in `scenario_templates()`.
**Slot 2 is literally `(TradingPrincipal, FeeVault)`** — the exact cross-class
movement L8 was built to catch — with slots 3 and 4 `TradingPrincipal ↔
HoardPrincipal`. They are constructed by `equity_transfer()`, the same
`v3_equity` path whose `residual_at` sums collateral atoms and claim units in
one `u64`.

But they are **artifacts, not transactions**: they feed
`encode_dealer_scenario_base_effect_program_v4`, baked into the emitted
**selector-9** scenario `EffectProgramV4`. The only two on-chain tests in that
module execute **selector one** and LP open/close. **Nothing in that file
executes selector nine** — while the same file encodes the account profile
convicted as unsatisfiable (`:115-116`, `:419-421`) and builds bundles from it.

So the campaign encodes selector nine's artifacts, checks their digests, and
never runs them. The cross-class route and the mixed-unit solvency kernel have
never met a real account, **which is exactly why neither was caught.** Two
independent findings — a C-10 conservation hole and a C-06 kernel defect —
share one cause and one uncrossed boundary.

**Dependency, recorded so nobody builds it backwards:** mounting a conservation
ledger on the Dealer campaign cannot produce the eight-class table until
selector nine executes. The order is **unblock selector nine → mount the ledger
→ then the table.** Building the instrument first is building it for a room
nobody can enter — the same error, one level up, as writing L2 for the only
compartment in the room.

## 2026-09-01 — the class stopped being theoretical

Every prior instance of *guards whose two sides move together* was
unexploitable: a check that could not fail, on a route where something else
already bound the value. **One was not.** `bf362312`.

`coordinates.terms` was unpinned on the **permissionless fractional compaction
crank** — the one route in the protocol that requires no permission at all.
Registry publication is permissionless, and the only joins were fields the
record's own author writes. So a cranker could publish terms with **D = 40,
rate = 16**, satisfy the conservation law **exactly**, and write a record under
which a holder of 20 of the 40 shards redeems **zero, forever**.

Conservation held. The holder was robbed. That is the whole lesson of the
class in one sentence: **a law that checks a value against its own author
proves nothing, and conservation is not integrity.**

Repaired the way the three sibling routes already did it —
`authenticate_selected_config` derives the config identity from the
*authenticated* terms and pins it to the Market's manifest — with a new named
refusal `SelectionConfig = 0x565C`. **The hostile was measured ACCEPTED before
the fix**, which is what separates this from a theory.

**A second instance of the same shape is live in Trading**,
`claims_composition_v3.rs:1151`: on the current V2 root both sides are read
from the same account in one function and the caller's `terms` is never
consulted; only the historical V1 arm pins it. Routed with the patch. Note the
lane's own caveat: Claims closing its side makes Trading's arm no longer
load-bearing *for that route*, which is not the same as correct.

### A routing of mine that was wrong, corrected by the lane

I relayed `0x5644` as a **missing guard**. It was not. The invariant *was*
enforced by the wrapped settlement and refused as `TerminalIdentity`, so the
published `0x5644` code was merely unreachable and my accusation was wrong.
Settled by measurement — the phase hostile returned 22095 where 22084 was
asserted. A dead refusal code has at least three causes (route never built,
guard removed, guard present under another name) and only measurement
distinguishes them.

### Owed, and now costed

- **The Rational replay cursor strands rent.** `RATIONAL_REPLAY_SEED_V2` has
  exactly one on-chain use — its own derivation. No close, no drain, no resize
  anywhere, while `rational_lifecycle_v2.rs` retirement closes everything else
  and never touches it. **One per `(descriptor, actor)`, so it grows with
  holders.** An account class with no route home that scales with adoption.
- **`the_terminal_settlement_has_headroom_…` is RED at HEAD**: 1,236,375 CU
  against `< 1_120_000`. The two new security guards add **+6,010**, reported
  rather than absorbed into the pre-existing red.

## 2026-09-01 — item 1 of the continuation order, and a half-landed repair

The `derivation_policy` predicate is repaired on the branch that extends
(`a153f08e`, host/builder half). Measured on real ELFs at the pinned tree with
only these changes:

| state | equity Add | LP Open |
|---|---|---|
| pinned | `0x4003` @ 145,093 CU | ok @ 1,030,550 |
| host half only | `0xd001` @ 946,935 | **refuses** |
| both halves | **`0x4003` @ 591,781 CU** | ok @ 1,057,494 |

**145,093 → 591,781 CU**: the Add clears the entire immutable-artifact tranche
and hits a further, different wall, not yet localized.

**The gate question, answered in source rather than assumed.** What still binds
the descriptor to its entry is unchanged — `validate_selection` still compares
`kind`, `release_id`, `config_id`, `capacity_profile`, `root_schema`. What binds
it to its own lifecycle is `artifacts.lifecycle.program`, which **is** the
lifecycle content digest: `borrow_record_against` refuses unless
`hash(&data) == digest` *and* the record sits at the Registry PDA derived from
`[RAW_RECORD_PDA_SEED_V1, schema, digest]`, after which `sealed_token` binds the
bytes to the execution seal. **R2 restated an identity already authenticated by
digest and added no authentication of its own** — it only demanded one field be
a per-action value and a per-root value simultaneously. Re-proof on the other
side, not a weakening.

### A half-landed repair is a regression

R2 **spans two ELFs**. With the host half landed and the runtime half not, it
fails for **all nine** Dealer selectors instead of eight of nine. Routed to S3:
drop only the second conjunct at `hot_v3.rs:3370` **and** `:1235`, and rebuild
the accelerator as well as Trading. Recorded because the lane said so plainly
rather than reporting the improvement and omitting the cost.

### Two ways a build lies

- `cargo build-sbf` reported **`EXIT=0` with the ELF byte-identical after a real
  source change.** Caught by hashing the artifact instead of reading the log.
  (Legitimate there — those builders are `cfg(not(target_os = "solana"))`.) For
  a genuine runtime change, a byte-identical ELF means the build did not happen.
- `authenticate_and_execute_hot_v3` has **zero SBF frame headroom**: per-call-site
  `.map_err` instrumentation produced 95 frame-overwrite diagnostics and an
  abort rather than a refusal. Gated early-return probes only.

### A lane convicting itself

`468f66b3` is **red on purpose** — it pins the mixed-unit solvency gate:
`residual_at` sums `collateral` (SPL atoms) and `claims[s]` (claim units) in one
`u64`, and that scalar is the sole `Insolvent` verdict. Verified independently:
**zero** occurrences of `basis_scale`/`payout_scale` in the entire dealer stack,
while `basis_scale` is a live founding-time `u64` guarded only `!= 0`. The
vector's *width* is authenticated eight-plus places; its *scale* never is. One
pool described twice, claim leg worth 20 atoms either way: `Ok(residual 108)`
versus `Err(Insolvent at 99)`. Both directions asserted, because which is live
depends on whether `obligations` is atom- or claim-denominated and **the type
does not say** — a gate whose safety direction cannot be read off its own types
is not a gate.

The lane reported against itself: **its first two drafts of that test passed
while proving nothing** — all four calls returned `InvalidShareSupply`, then
`InsufficientAssets`, never reaching the gate, so it was comparing two refusals.
The same defect it had spent the session convicting in other people's tests. The
anti-vacuity guard it then wrote caught the second draft, and is committed
beside the test.

## 2026-09-01 — C-09's stranded loser goes home

The hole is closed with a route, not a relaxation. `40217014`.

`AbandonSubmission` (`DCLTPAB3`, action 3, 18 accounts) is **the other half of a
partition**. The consumed gate did not move and could not have: the consumed
wire *physically cannot express* an abandoned submission —
`ProviderReclaimRequestV3::decode` refuses a zero `terminal_sequence` and any
zero identity, `certificate` among them, and a `Submitted` lifecycle carries
zero in both by construction. Where the consumed route proves a submission
became truth, this one proves it never can.

The gate is a **conjunction**: the submitter's own `reclaim_after_unix_seconds`
has passed **AND** the Source can no longer consume (program-owned past
`Primary`, or the vacant System account `CloseFund` leaves). Both, never either
— the deadline alone is the stranger-deletes-a-live-answer failure. The
lifecycle's `terminal_sequence` / `certificate` / `provider_evidence` are each
checked zero rather than trusted from `status`, because this route's whole
admissibility rests on that statement.

Refusal `SubmissionStillConsumable = 0x8017` earns its own code because it is
the only refusal here an honest well-formed request triggers: **right route,
wrong moment.** Evidence on a real ELF, 4/4 green: against a live market the
*builder* refuses (griefing stopped before a transaction exists); one second
early the chain refuses `0x8017`, red-proven, with lifecycle, update and
beneficiary byte-identical after; past the deadline a stranger reclaims and the
rent lands on the same persisted recipient the winner's did. **The strand
assertion is gone, replaced by the assertion that the loser goes home** — that
transition is the proof.

`docs/evidence/LIVENESS_CENSUS_2026_08_29.md` row Q8a had already reached this
exact construction — new magic, new request type, new terminality conjunct, the
same griefing argument — **and stopped at the analysis.** The analysis was right
and sat unbuilt for three days.

### The stub with an alibi

`resolution_successor.rs`'s `primary_instruction` and `funded_caller_instruction`
were replaced by `panic!` stubs on 2026-08-26 00:19. The `#[ignore]` landed
**47 minutes later.** The campaign body has been unreachable for six days
**while the README quoted its ten-row compute table as evidence.**

Four layers of cover, removed one at a time with a rerun after each: the
`#[ignore]`; six env vars nothing in the repo set; a pinned Resolution identity
at V4 while the program authenticates **V7**; and an activation-cache address
derived and **never seeded**, refusing `RegistryError::ActivationCache`. Under
all four: the panics.

Dispositioned as **convergence, not repair** — rebuilding those builders means
porting the current Core-effect and funded ABIs into a *second* fixture, while
`resolution_core_v3_lifecycle.rs` already carries them against current ELFs. The
`#[ignore]` stays with the true reason and a pointer, because an un-ignored
permanently-red test in a shared checkout makes every other lane's run red for a
cause they did not create.

### Delegated verification, swept and stated

No ed25519/secp256k1/keccak in the Resolution program or `dclutch-pyth-svm`;
`FullPriceUpdateV2::parse` reads a tag byte. Soundness is the Receiver-owned
account plus a release record pinning that Receiver by ProgramData, deployment
slot, upgrade policy and config digest. Swept docs, web components and console
strings for first-party verification claims: **none found** — a negative result
with the search named. Also corrected "performs no provider CPI": the transport
CPIs the Receiver in three places.

## 2026-09-01 — C-09 closes on evidence, and a hostile corrects the lane

All six C-09 clauses now carry real-ELF evidence, and **both halves of the S2
gate — a provider life and a fallback life, each from founding through fund
close — execute in one campaign.** 4/4 → 5/5. `e1a4191d`.

### The hostile that corrected the lane

The lane predicted the second failure-walk would refuse `Transition` on
monotonicity grounds: the Source has left `Primary`, so the transition must
refuse. It does — **but it never runs.** `plan_deadline_failure_v1` debits
*before* it transitions, deliberately, so the replay dies a step earlier in
`release_in_place` against an already-empty Bounty compartment: **`Funding`
(`0x800E`), not `Transition`.**

> The bound on how many times this walk pays is not the state machine's
> monotonicity — it is that the market escrowed exactly one bounty and it has
> been spent.

And the sentence that settles the house rule better than any prose has:
**"A bare `is_err()` would have shown me the refusal I predicted instead of the
one that exists."** The two hostiles now carry different codes and are
distinguishable.

### A deleted route's nouns outlive it

The lane had reported that the funded fallback walk — `FailNext` / `Exhaust` /
`CommitFailure` — executes on no real ELF anywhere. **Wrong twice over.** Those
three are the **V1** walk and the program *deleted* them: `funded.rs` says so in
its own module doc, and `exhaust_after_primary_deadline` refuses any material
carrying a recovery policy, so no prestate in this tree can reach them. They
survive only in the codec enum, the dead successor file and the receipt-caller.
There was nothing there to execute. The live walk is one transition —
`Primary → Exhausted → FailureCommitted` — and it has always executed on a real
ELF in `relayed_mainnet_state.rs`.

**The lane had read a dead file's vocabulary as the live one's.** Corrected in
the README in place rather than quietly dropped. Worth naming as a hazard: a
deleted route's *nouns* outlive it, and a lane reasoning from them will hunt
something that cannot exist.

### What was genuinely missing, and is now closed

The walk had never been driven against a market whose evidence family is Pyth,
and the Pyth campaign reached its failure terminal only from a **seeded**
`TerminalFailure`. Now walked end to end from an ordinary open market founded to
be answered by a price feed, where the only thing that differs is that **nobody
submits**: CreateFund → ActivateFund → VerifyFundReady → silence → the walk →
AdmitTerminal → BeginRetiring → CloseFund.

- The walker is a `Keypair::new()` holding no role and no relationship to the
  manifest that pays it.
- Standing **exactly on** the deadline refuses `Transition` (`0x800C`,
  red-proven): *the last second an honest resolution may land and the first
  second a walk may run are different seconds.*
- The certificate carries `provider_evidence` and `route` **both zero** —
  nothing a provider said stands behind this terminal, and the certificate says
  so.
- `funding_allocation` equals the market's own material, which the program
  **found** by matching config ids rather than accepting as an index. That is
  the difference between authenticating and trusting.
- The walker is paid the capability's own quoted bounty, stated three
  independent ways: `work_paid`, the walker delta, and the escrow delta.

One route serves both evidence families, and that is not a coincidence needing a
per-transport re-test: the 22-account frame carries **no provider account and no
relay account**. It lives under the relay instruction magic only because that is
where the dispatcher put it.

## 2026-09-01 — an instrument that would have measured a different application

The browser lane needed a rendered tree to gate landmark nesting. The obvious
move — `@vitest-environment jsdom` — **flips resolve conditions to `browser`,
after which `@solana/web3.js` fails to import**. The suite would have run
against a different application than the one that ships, and passed.

It used **jsdom as a library** instead, rendering every shell a source survey
finds and parsing the output. Same family as the dead `sol_log_64` channel and
the stale nested checkout: **verify the instrument, not just the reading.**

What the working instrument then found: **`<header>` was inside `<main>` on 28
of 28 page shells**, demoting the site header out of the `banner` landmark on
every page of the site. Unfindable by any assertion that existed before, and
obvious the moment one did. `9090ba0d`. Contrast followed — 29 ad-hoc greys
below 4.5:1 collapsed to one token (`8d440ea3`).

### Shape was never the missing property

`composeRangeProtectionV1` **passed the degenerate market with full marks**:
cuts strictly increasing, regions = cuts + 1, portfolio gcd-normalized. Every
structural property held. *Where the coordinate sits* was the missing one — the
cleanest statement of why the partition-quality gate had to exist at all.

### Three refusals worth keeping

- **Would not reimplement `require_interesting_partition_v1` in TypeScript** —
  its triangular mass model in a second language is *the identical defect* to
  the Studio mirror it was fixing. Refusing to fix a mirror by building another
  mirror. Shipped a strictly weaker, **exactly decidable** unit-sanity condition
  instead, labelled provisional in source *and* in the UI.
- **Would not render `cell_share_bps`** when no loadable bundle carries it; the
  page says so where the number would go. Routed the producer instead.
- **Left `evaluateProductV2` "untouched, unexcused"** — naming a mirror you
  cannot yet remove beats pretending it is not one.

### The Studio was already decoding the truth and dropping it

`decodeCoreFoundProductGraphV2` validated the operator's cuts for ABI, width,
identity and strict increase — **and then discarded them**, while the page
rendered TypeScript-derived "interpolation segment N" labels instead. Extracting
them wrote down not one new byte coordinate.

### A latent bundle bug, found by a test that failed for the right reason

`operatorSurface.ts` is **load-order dependent**: past the eighteenth component
import in one module graph its module-scope ProgramData derivation throws
`Unable to find a viable program address nonce`; alone it evaluates fine.
Bisected, count pinned in the file. **A page that happens to import one more
component ships broken** — module-scope derivation that can throw is the
hazard, not the test that surfaced it.

## 2026-09-01 — the recursion: no ledger-bearing campaign runs at all

The conservation ledger was blind to eight of nine compartments because the
campaigns it is mounted on only ever open one. Underneath that:

**Those campaigns do not execute.** Measured today at `2bf8a582` — the journey
built all seven SBF roles with zero frame diagnostics, then **exited 1** at
stage tool:

```
demo-market is retired: a standalone registry address cannot authenticate the
checked local Direct deployment.
```

`run-journey.sh:445` calls `$JOURNEY_BIN demo-market`, which
`journey/src/main.rs:243` now refuses **unconditionally**, and does so *before*
the campaign invocation at `:480`. The whole-life journey cannot run at HEAD.
`docs/evidence/LOCAL_CAMPAIGN_SERIES_2026_08_30.md` documents it, notes
`tools/gauntlet/run.sh` dies at the same boundary, gives the ordering a fix
needs — and it has been open two days. The sibling `relayed-vertical`, which
links the same seven-law ledger by `#[path]`, is recorded in that same document
as not compiling on main.

> Instrument and risk not in the same room — one level below where the last
> instance was found.

**And the gate shape is the same class yet again:** public CI has a job named
*"the journey campaign compiles."* **Compiles.** Nothing anywhere runs it. A
refusal introduced two days ago sits in the campaign's own entry point and no
gate noticed, because the only question ever asked was whether it builds.

### A law that invents violations

The lane caught its own regression **before** landing it. Its derived per-class
split assumed classification works; `relayed-vertical` calls `admit_founding`
**without** `admit_custody_namespace`, so its Hoard classifies as
`unclassified`, and the derivation would have attributed the Hoard's movement to
the wallets — **manufacturing an L8 violation out of its own arithmetic rather
than out of that market.**

A conservation law that invents violations is worse than one that misses them:
it burns the credibility of every true one. Guard: derive nothing when no
namespace has been admitted.

### The provenance rule applies to your own shell

The lane nearly reported *"the runner exits 0 on a failed campaign."* It exits
**1**, correctly — the 0 was its own `nohup` wrapper's status. Second false
finding caught by provenance discipline before leaving the lane, and the general
form is the useful one: **most lanes check the repository's provenance and trust
their own harness implicitly, and that asymmetry is exactly where a wrong number
gets in.**

## 2026-09-01 — one side tested, the other not

The journey tier's own suite **already asserted that `demo-market`'s refusal
exists.** Nobody asserted that the runner **stopped calling it.** One side of
the contract tested, the other not — and that is the whole reason a CI job named
*"the journey campaign compiles"* was true and useless for two days while the
campaign's entry point refused unconditionally.

> **Any tier whose runner and binary are separate artifacts wants this test.**

The shape, worth copying: parse the runner for every subcommand it invokes on
the binary — resolving *both* call shapes, the literal and the
`JOURNEY_ARGS=(run …)` array — and refuse any that is retired or undispatched.
Then a **second** test that keeps the retired-list honest by requiring every
name on it to actually refuse when dispatched, so nobody can edit the list to
make the gate pass.

That second test is the part that matters. A gate whose allow-list can be
edited to satisfy it is a gate that only tests the diligence of the person
editing it — the same defect as an exemption register without a required
verdict, and as a status field a guard asserts back at itself.

Also landed in that shape: the runner now dies **immediately** with the exact
bootstrap command when no market is supplied, instead of dying six minutes of
SBF builds later inside the binary with a message that does not say what to do.
**Runnable-by-supply beats runnable-by-nobody**, and it needed no edit to the
producer another lane owns.

## 2026-09-01 — the author of the finding walked into the finding

Lane S11 convicted `fdfbe0dd` for renaming a route and leaving its bindings
behind, so a campaign that founded a market **every single run** read
NEVER-EXECUTED. It wrote that up. **Four commits later it did the same thing.**

`a19d93b1` moved `RecordActionV1::Begin` from `1` to `5` — the R-13 repair. The
census route id encodes the action literal, so `registry/process_begin#1` became
`#5` and tier 1's two bindings kept pointing at the old id. Tier 1 publishes a
record every run; its route would have read never-executed again.

Caught **within the hour**, by the stale-reference check the same lane had added
for the first instance. Repaired in `cce8705f`.

> **Every convention in this tree enforced by attention rather than by a gate
> should be assumed already broken somewhere.**

The author who had just documented the failure mode still walked into it. That
is the argument for the gate, and it is stronger than any argument the lane
could have made from its correct findings.

### C-16 has six categories and an instrument for four

The entry list (`docs/evidence/C16_ENTRY_LIST_2026_09_01.md`) reports what it
**cannot** say: **nothing in the tree measures *unexplained authority* or
*unowned economic flow***. The only place either phrase occurs is the contract
row demanding them. The census enumerates dispatch surface and refusal taxonomy;
it says nothing about which signer or cached role authorises an act, nor which
lamport and atom flows have a named owner.

Recorded as **an honest blank rather than a zero** — because a reviewer handed a
page of green numbers reads silence as coverage. **Unswept ground, not clean
ground.**

The four with instruments, re-measured at HEAD rather than remembered: **57 of
161 never-executed** (0 stale binding refs), **19 selectors** reachable from
neither CLI nor SDK, **20 routes** over-counted as browser-reachable, **6**
unfixed stale guide claims, **1 dead refusal code of 297** — down from 12.

## 2026-09-01 — General: an identity that was the same for everyone

`GENERAL_ROOT_IDENTITY_REGISTER_V3` (27) opens all eight seed orders and
**nothing ever wrote it.** No `ProjectKey` targets it; the fifteen Lean-emitted
RequestProfiles write identity registers `{0, 3, 29}` only; the trusted
environments supply three other values. Because `27 < 45`,
`validate_seed_against_profile` accepts, and `identity_register` returns **32
zero bytes with no zero check anywhere.**

That is a **well-formed address, identical for every General root** — so two
roots collide on one occurrence identity. And `general_hot_v3.rs:2239` injected
the key host-side, so **host and chain derived different addresses from the same
artifact.** A silent collision and a silent host/chain divergence, from one
register nobody wrote. Fixed in `4180175a` as the last fixed operation, so every
earlier ordinal's bytes are preserved; two tests, both proved red first.

**"14 of 15 fail geometry" was wrong: it is 15 of 15** — the 14 was a test
iterating a 14-entry array. And **the two findings are independent, measured
rather than assumed**: 15/15 before the register fix, 15/15 after. A lane that
had assumed causation would have read a correct repair as a failure.

### Structurally unsatisfiable, not merely mis-ordered

`OP_REQUIRE_KEY` / `OP_REQUIRE_OWNER` read `input_identities` while
`OP_PROJECT_*` write a **separate bank** — so **no guard can ever see what the
same pass projects, whatever the order.** Census of all 22 General guards: 19
name `TRADING_PROGRAM`/`RESULT_OWNER` and hold; three name `identity::OWNER` and
are fail-closed. Upgrades three ordering bugs into one structural fact.

### Two live authors for one law

Proving the optimality clause clean turned up a real defect:
`runtime_candidate_key_better_v2` (the runtime) and `candidate_better` (the
differential oracle) each carried a **private little-endian id encoding**, and
nothing compared them. 576-comparison join added, proved red by flipping one
`<`. Two authors for one law is the same class as two implementations of one
constant — and nobody noticed, because both were right.

## 2026-09-01 — a CU delta with no control is not a measurement

A lane was asked to own +6,010 CU its two new security guards appeared to cost.
It measured instead, on a terminal route that **calls neither guard**:

| claims ELF | change | CU |
|---|---|---|
| `84866d9c` | HEAD before the guards | 1,236,375 |
| `7a8af549` | + two compaction guards | 1,242,385 (+6,010) |
| `958e4d34` | + guards + an entire new replay-close route | **1,228,897 (−7,478 vs HEAD)** |

**Adding *more* unreachable code moved it DOWN, below the original.** A ±13,000
CU swing under changes that cannot execute on that route is compiler layout and
inlining noise. The guards' true cost is on the route they guard: inner Claims
compaction 544,064 → 562,805, **+18,741 CU**, on a transaction finishing at
605,326 of 1,400,000.

> **A CU number without a control on a route the change cannot reach is not a
> measurement.**

This project has already paid for the general form once: a compute figure
published this session was read out of an interleaved parallel test log, belonged
to a *passing* test, and had to be withdrawn. Same disease, different vector —
a number attributed to the nearest recent change rather than to a control.

The pre-existing ceiling stays red at 1,228,897 against `< 1_120_000` — **7,478
CU better than when the lane found it**, and owned elsewhere.

## 2026-09-01 — the check's name was true, and that was the problem

The journey tier ran `cargo check`, and its CI job was called *"the journey
campaign compiles."* It did compile — every day, while the runner called a
subcommand the binary refuses (it still *dispatches* it, so it builds) and while
281 of 282 tests passed and one failed.

The tier now runs `cargo test --bins`, and the job is *"…compiles and its tests
pass."* The original author's reasoning was right about the **campaign** — a
real journey needs a validator and tens of minutes, which belongs to the cut —
and wrong about the crate's own **host tests**, which need no validator and cost
seconds on a build the tier already pays for. Both hidden defects lived in
exactly that gap. It still does **not** claim the campaign passes against a
chain, and the tier says so.

### The stale number that was not General's

The failing seam assertion measured `left: 140, right: 68`. 68 is the test's own
hand-carried `2 + 9*7 + 3`; 140 is `2 + 9*15 + 3`, and fifteen is not a
coincidence — `GENERAL_ACTION_PROGRAM_COUNT_V5 = 15`, and the builder emits nine
records per action. **The builder was self-consistent and correct; the test's
`7` was stale.** So it belonged to the bootstrap, not to General's semantics —
settled by measurement rather than routed on a guess.

The repair is a **derivation, not a new number**: the count now comes from the
release contract's own constant that the builder is typed against, plus a second
assertion pinning per-action multiplicity independently, so a drift in what each
action contributes cannot hide inside a total that still balances.
**Writing `15` would have bought exactly what `7` bought and rotted the same
way.**

## 2026-09-01 — sixty percent of a program's lib.rs was dead, and it lied

Closing C-09's generic-header refactor turned up **fourteen `#[cfg(any())]`
blocks — 775 of 1,253 lines** in one program's `lib.rs`: a superseded V1 path
kept beside its successor, opening with a 248-line block named
**`removed_legacy_v1_direct_instruction`** that was never removed.

Two things make it a class rather than an anecdote:

1. **One block stated a third, contradictory meaning for the very field being
   fixed** (`transport_profile_id == release.adapter_id()`). Dead code that
   disagrees with live code is worse than dead code — a reader cannot tell which
   sentence is the specification.
2. **It caused a misreport earlier the same day.** The lane read
   `FailNext / Exhaust / CommitFailure` out of that dead dispatch and took them
   for live vocabulary. *A deleted route's nouns outlive it* — and this is where
   they live.

**The control is the reusable part: the shipped ELF is byte-for-byte identical
before and after** — `ee33f9e9…` across a 775-line deletion, a refactor and a
rustfmt. Exact, cheap, and stronger than any suite, because it proves the
deletion touched nothing that ships. Use it for every dead-code removal.

### Measure before ruling, and the ruling gets cheaper

`PROVIDER_EVIDENCE_DOMAIN_V3` was assumed expensive to change because it is
baked into an on-chain content identity. Measured: **it is never a PDA seed** —
nothing derives an address from `provider_evidence`. So the change is
values-only. State, certificate, lifecycle and receipts re-digest; **no address
moves and no market needs re-founding.** A scary wire change turned out cheap,
and only checking could have shown that.

Found while measuring and deliberately **not** fixed: the constant is declared
**twice**, hand-mirrored across program and operator with nothing welding them.
It **fails closed** — the operator would build a request the program refuses —
so it is a hazard rather than a hole. **Hand-mirrored constants that fail closed
are debt; ones that fail open are defects.**

### The seam existed everywhere except in the type that needed it

The argument for the refactor came from inside the family, not from a
hypothetical second one: the sponsored-push release type had exposed its own
`transport_profile_id()` all along, and `market.rs:12051` had **already
hand-written the dispatch**. Four call sites were each restating "for the pull
family the transport profile is the router ABI." A unit test now pins
`transport_profile_id() == router_abi_id()`, so a later shape that gives it a
real field announces itself at one assertion.

## 2026-09-01 — the band is required, and a lane deleted its own work

`founding_band` is now a **required** field of the spline authoring input —
`{anchor, volatility_bps, window_slots, plausible_half_widths,
max_cell_share_bps}`, every one required, **no serde default anywhere**
(`c8356a5f`). An input that declines to declare refuses at parse naming
`founding_band`; a *partial* band refuses naming the missing field. `compile()`
runs `require_interesting_partition_v1` **before any record is built**, so a
degenerate partition writes nothing.

Red-proved the right way: moving spot onto the cut makes the **identical** input
compile at `[5000, 5000]`, so the refusal tracks **placement**, not band
machinery.

**Premise corrected again**: spot and window are not available on the spline
authoring path either — the other nineteen fields are pure geometry — so all
three are author declarations there. On `market.rs` a Pyth observation and
deadline slots do exist, so only `volatility_bps` is genuinely new.

### A green that proved nothing about the branch that changed

`tools/release/successor_campaign_pack.py:526` validates the report with **exact
set equality**, so a new report key would have refused the release pack
outright — on the very path cohort-9 is waiting on. Its own 13 tests pass, and
**never construct a compiler report**, so green there said nothing about the
branch being changed. Caught by running the real binary and diffing the two key
sets directly.

**A suite that passes without ever building the object under test is the same
class as a job named "compiles."**

### The partition and the payoff have to be fixed together

Wall 1 closed by absorption, not deletion-by-preference: the successor
(`MarketQuestionV1` — `ThresholdFromSpot`, `CentredRangeProtection`,
`CentredBands`) fixes partition **and** payoff in one act, because a caller
supplying coefficients separately can build beautifully centred bands **the
payoff ignores.** Not hypothetical — the lane's own earlier real-ELF test paid
one unit on every ordinary outcome. `payoff_distinguishes_cells` now reports it.
Authored SOL/USD admits on the real ELF at 30,046 CU.

### A lane deleted its own work because someone else had already done it

Wall 5 was closed by another lane's `544a0feb` while this one was building a
generator, a generated module and a verify test for the same property. It
**removed its own version entirely** rather than ship both — *"two mechanisms
for one property is the exact defect I am arguing against"* — and cross-checked
the other lane's parser against its file instead.

It also **weakened its own earlier claim**: "no record carries
`{target, deadline, abort}`" was too strong. `PreMarketFundingRequestV2` is real
and executes and has a target and an abort route — that target is a subset
ledger's lamport rent shortfall, not market principal. What is genuinely absent
is `FundingPlanV1`, now written down with its signature and its semantic owner
named.

## 2026-09-01 — the gate proves itself, and a dependency that was not there

`tools/ci/run.sh journey --commit 5e206c89`, clean-archive mode rather than the
shared working tree:

```
test result: ok. 282 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
=== verdict ===  PASS      journey
```

The strengthened tier running the tests it previously only type-checked, **on
the commit that fixes both defects it was blind to.** Behaviour and the
workflow's name landed as a pair, and the workflow was diffed in full before
committing because it had been dirty at session start from something not the
lane's — all five hunks confirmed as its own, nothing foreign riding along.

### A dependency that was asserted and turned out not to exist

I relayed a warning that a market founded today "resolves into one bucket", and
the lane **checked it instead of inheriting it**. `LocalMarketShapeV1::default`
is a **four-outcome** market — so the defect is **placement, not width**: the
cuts are cent-scale while the source returns raw price atoms unrescaled, so
every outcome exists and one takes all the mass (measured elsewhere as
`0 / 0 / 0 / 0 / 10000 bp`). My relay had compressed those into one phrase and
lost the distinction, and the lane held it as *relayed and unverified* rather
than acting on it.

Then the correction that actually mattered: **L8 is bounded by how many Custody
compartments a market opens, not by how its outcomes resolve.** The journey
opens `HoardPrincipal` and nothing else whatever shape it is founded with, so
even a perfectly centred market yields a two-class ledger.

> A market nobody could lose would make the campaign economically
> uninteresting; it would not make the table wrong.

So the conservation work never depended on the volatility input at all, and the
release lane was carrying a blocker it did not have. **Confirmed against a
second candidate explanation rather than merely asserted** — which is the only
way a dependency claim is worth anything.

## 2026-09-01 — the dead-code class was one file's, and the backlog is empty

Swept tree-wide after the 775-line deletion. **`#[cfg(any())]` real attribute
blocks across the whole repository: zero.** All fourteen had lived in one
program's `lib.rs`; no other crate ever used the idiom. `cfg(FALSE)`,
`cfg(not(all()))`, `if false`: zero. And the insidious relative — a
`#[cfg(feature = "X")]` where no manifest declares X, which compiles never and
looks live — **zero undeclared across 14 features and every crate**, counting
optional dependencies as implicit features.

That one program accumulated sixty percent dead lines while every neighbour
stayed clean makes the finding **sharper, not weaker**. And it settles what the
instrument is for:

> **The instrument's value is as a tripwire, not a backlog — the backlog is
> empty.** One grep plus the ELF-identity control is the whole thing.

**Dead blocks that contradicted live code: 1 of 14** — the one already
convicted, stating a third meaning for a field the live route defines
differently. Both gone.

### A new category, and it is the one that misleads

**Live prose citing deleted symbols.** Five comments across four files cite
`funded::process_funded_transition` **in the present tense**. That function has
**no definition anywhere in the tree** — deleted before this session — and now
the `cfg(any())` block that referenced it is gone too, so both nouns are
unfindable.

One is worse than stale: `core-sbf/src/resolution.rs:880` is a **lifting plan
instructing someone to resurrect a symbol nobody can locate.** These are not
decorative — they are the stated justification for a live on-chain refusal,
`CoreSbfError::RecoveryWalkUnavailable = 0x3011`, which welds `CreateFund` shut
against recovery-bearing material.

**This is what turns dead code from untidy into misleading**, and it is what
caused this lane's own misreport earlier today.

**The gate still holds, checked rather than assumed**: `exhaust_after_primary_
deadline` still refuses `recovery_policy().is_some()` outright, so the live half
of the premise stands even though the dead half lost its names. `cfg(any())`
never compiled, so removing it removed no route.

The correction went into `funded.rs` — **the module all four foreign comments
point at**, so anyone chasing the symbol lands on the note. Routing by
construction rather than by a document nobody opens, and it survives whoever
adds a fifth citation.

Attached as evidence to the open **"recovery ontology: keep or cut"** ruling
rather than opening a new one: *the plan to revive it names a symbol that does
not exist* is the sharpest argument yet that it needs deciding.

**Control:** one ELF digest, `ee33f9e9…`, across the 775-line deletion, the seam
refactor, a rustfmt, and the doc edits.

## 2026-09-01 — ownership and correctness are different questions

C-16 §6 read *"unowned economic flow — no instrument."* The **atom half** now
has one (`5858ad0c`), and it is careful about what it proves.

**What it measures:** all 81 ordered compartment pairs through
`CustodyRequestV1::validate`, stated as one equivalence — **a side being `None`
is the only thing that wire refuses about a pair.** 64 pairs are
shape-admissible, and **`HoardPrincipal → FeeVault` is among them** — principal
paying a fee, the movement C-10 forbids.

**The contract does not enforce that and was never the place it lived.** Every
compartment rule in the protocol lives in a *calling* program. Deliberate — but
**undocumented and unmeasured**, which is exactly the state in which an
invariant quietly stops being true. It is a test rather than a comment so that a
reviewer asking *"what stops the Hoard funding the fee vault"* is sent to the
callers immediately instead of discovering by reading that the answer was never
here; and so that if a compartment rule is ever added *there*, the census goes
red and whoever adds it says so on purpose.

Three details keep it from being decoration: the `External` conjuncts are
satisfied per side so it measures the **pair** rule and nothing else; the `None`
refusal is asserted by **exact discriminant**, not a count; and the
principal-to-fee pair is named explicitly, because *"64"* does not say which 64.

**The caller census:** every site that *sets* a compartment — 54 source-side, 49
destination-side — **is owned.** Pinned literal, two-literal conditional on a
closed flag, closed match with catch-all `Err`, direction accessor whose arms
are literals, in-contract pass-through, or the wire decoder, where the
compartment *is* caller-supplied and the owner is the **authenticated calling
program**. The projected-founding decode additionally refuses
`{None, External, HoardPrincipal}`: a founding can never be funded out of a
Hoard.

**Nothing pins `HoardPrincipal → FeeVault`** — both FeeVault-funding sites take
`TradingPrincipal`. Reached by enumeration rather than by failing to find a
counterexample, which is the stronger form.

### The qualifier that must travel with the number

> **Ownership and correctness are different questions; this instrument answers
> the first.**

A site pinning a wrong-but-literal pair reads as *owned* here and still violates
C-10. So §6's atom half moves from *honest blank* to **swept clean at the
construction sites, correctness not asserted** — never to "C-10 verified". The
other stated limit: `#[cfg(test)] mod tests` blocks inside production files were
not excluded, so the totals are an **upper bound** on production sites.

**The lamport half is a separate unit** — rent beneficiaries, funding-compartment
releases, account closes, crank rewards — and it forks where the atom census did
not: an atom's compartment tag is a PDA seed, so ownership is *derivable*, while
a lamport's owner is often a caller-supplied `refund_recipient` and must be
**authenticated rather than read**.

## 2026-09-01 — a blob can match its digest and still come from a different tree

The browser-side admission planner is **built, not specified** (`6bf8eba7`
Rust + `c1676a1c` browser). Two walls, both measured rather than argued:

1. **getrandom 0.4** arrives via `dclutch-direct-ticket → solana-signature[verify]
   → solana-ed25519 → rand`. That `verify` feature is deliberate and
   unconditional — the ticket reader verifies every detached signature it parses
   — so the edge gets *told about the browser* rather than dropped.
   **`RUSTFLAGS --cfg getrandom_backend` alone does not clear it; the cargo
   feature is required.** One full build to learn it, and now nobody argues.
2. **Structural.** `dclutch-operator` links `dclutch-trading-sbf`, which pins
   layouts for a 64-bit target — `ExecutedReceiptV3` is 72 bytes on SBF and 60
   on wasm32, `ChildExecutionStateV3` 216 and 140. **Those assertions are right
   and were not relaxed.** The resolution: **the planner never needed the
   program.** Trading belongs to 8 Dealer/Series modules of 37, behind a
   default-on feature. Green with defaults, green on wasm32 without.

Refusing to weaken a correct assertion and finding the decomposition instead is
the "construction that extends" case, done properly.

### The canary, and the threat model behind it

Three `const _: () = assert!` read the frame width, the 8-byte magic and the
owner coordinate from the contract **by constant name** — so a rename or resize
fails the *build* rather than silently emitting a 26-account frame the runtime
refuses unhelpfully. The snapshot DTO is field-for-field with the planner's, so
**adding a field there fails to compile here until it is carried.**

Then the browser re-checks: bytes length- and SHA-256-pinned before execution,
and after loading, the loader asks the planner its own width and refuses if it
disagrees with the contract's.

> **A blob can match its digest and still come from a different tree.**

### An instrument nobody could read

`tsc --noEmit` was red app-wide on a pre-ES2020 target. ES2017 → ES2022 takes it
from **1,794 errors to 31**, and `npm run build` is green on the raised target —
**the reading changed, the application did not.** An unusable signal becomes a
legible backlog.

And the `operatorSurface` load-order bug was chased past the symptom: the real
culprit was not the web copy but the **SDK twin** resolving its own nested
`@solana/web3.js` through the barrel. Fixing it unblocked the landmark gate —
all 28 page shells now gated, **none excused.**

**One unit remains between a wallet and a trade**: the 25-account acquisition
snapshot. Every address is already reachable from code that exists.

## 2026-09-01 — a basket that may only be expressible when it is not a basket

The 44th row — the last standing in the Structured campaign — is now localized
to **one uncommented line** (`3932e396`).

**Layer one ruled out the family everyone was primed to suspect.** The previous
phase had failed on geometry, so geometry was the obvious answer. Declared
against observed: `common_scalars=21 item_scalar_stride=0 common_identities=23
item_identity_stride=0` against `observed scalars=21 identities=23`. **Every
width agrees** — so the refusal is a predicate that evaluated false, not a shape
that disagreed.

**Layer two, because a class is not a position.** The VM reports no operation
index. Rather than edit a kernel crate for a diagnostic, the probe re-runs the
**same public `execute_fold_atomic`** over successively longer prefixes of the
same program — so the answer comes from the authority itself rather than a
second interpreter in the harness — and degrades honestly to *"could not be
localized"* when truncation yields a program the validator rejects.

```
transition fold refused at operation 4 (CheckFailed in prelude)
```

Decoded: opcode `0x01` = `OP_SCALAR_EQ`, operands 9 and 3. Register 3 is
`SCALAR_DENOMINATOR` = 7; register 9 is row 0's `ITEM_SCALAR_COEFFICIENT` = 2.
Emitted at `open_structured_v3.rs:927`, once per row, **with no comment**:

```
scalar_eq(coefficient[row], denominator)
```

**The released Structured transition requires every coordinate's coefficient to
equal the denominator** — which makes the family expressible only for degenerate
products where every coefficient is identical. *The one shape a basket exists
not to be.*

**No kernel states this equality.** `prepare_issue` consumes
`quantity * coefficient[i]` per coordinate and relates it to nothing but that
coordinate's free shards; `prepare_denominate` uses the denominator as the
shards-per-claim ratio; grepping the two words together across both kernels
returns nothing.

And the campaign's own basis **closes exactly** with coefficients `[2, 3, 5]`
against denominator 7 — 14+14=28, 28+21=49, 21+35=56. So the arithmetic works
with varying coefficients and the transition refuses the very basis that closes.

**Not repaired, deliberately**: a gate moves only when the law it guards is
re-proven on the other side, and *absent* is not the same as *nothing wanted it*.
The settling measurement is queued — remove the emission in a probe build and
drive issue → denominate → redeem → retire, checking conservation closes with
zero remainder at every step. If it does, the constraint guards nothing and the
gate may move. If it breaks, the constraint was load-bearing under a name nobody
wrote down.

**The blast-radius objection is gone**: the transition bytes digest into the
descriptor and thence into every derived identity, which was the reason this
looked like ember's call — and a standing full-redeploy grant with cohort-9
already deployed from a named commit absorbs re-derived identities for free.

## 2026-09-01 — owned is not the same as deliverable

The lamport census forked where the atom census did not, and the fork is the
finding. An atom's compartment tag is a **PDA seed**, so ownership is *derivable
from an address*. A lamport destination is a **field** — `rent_refund`,
`beneficiary`, `rent_beneficiary`, `refund_wallet`, `refund_recipient` — so
ownership is **claimed, and must be authenticated rather than read.**

Four classes, of which one is the defect: **(4) caller-supplied and only
self-consistent** — the destination is whatever the caller says, checked only
against another thing the same caller supplied. **That is THE SECOND CLASS
applied to lamports.**

**391 real set-sites** across the five fields. Seven wire-decoded
`refund_recipient` sites carried to a verdict, and it is a **verified negative**:
on chain the check *looks* like class 4 — `frame.account(4).key ==
request.refund_recipient` proves only that the caller agreed with themselves —
but two further refusals bind it to a lifecycle value written at creation under
a `validate()` that refuses `provider_submitter == refund_recipient`. **Frame ==
request == persisted-at-creation: class 3, owned.**

> Stopping at the consistency check would have produced a false positive.

### The correction to C-16 §6 itself

Of the three lamport defects this session found by accident, **two were not
unowned — they were unreachable.** The Rational replay cursor had a named owner
and **no route home**. The loser-reclaim hole had a named owner and **6,389,280
lamports per loss that no instruction could deliver.** Only the crank reward was
an ownership question, and it was answered by *bounding* — out of rent that was
leaving anyway.

**So §6 as written names one failure mode and the defects actually found were
mostly the other.** The census has to ask two questions per flow:

1. **is the destination owned?**
2. **is there an executable route that delivers it?**

A category that only ever asks the first will keep reporting clean while the
defects that actually strand money go unnamed.

**Stated not-done, exactly:** 384 of 391 set-sites are **enumerated, not
classified**; the lamport half is **not swept**. What exists is the scheme, the
method, the population, and one cluster carried to a verdict.

## 2026-09-01 — a negative reached by enumeration, and a false positive that proved its own caveat

**The join ran**: 159 routes / 65 never-executed (the *claim* register, stale
against the measured 57 of 161, and said so) × the five lamport-destination
fields. **28 distinct never-executed modules; 9 set a destination, covering 18
never-executed routes.** Then the third step on each, because a candidate is not
a defect.

- **Seven have an in-tree campaign** — the register's own already-named gap:
  campaigns that pass against real ELFs and emit no census evidence. Binding
  work, not money holes.
- **`sponsored_push_v1`** was the strongest possible candidate — never-executed
  **and** no test file — and cleared on the third step: `refund_recipient` is set
  to `payer.key` and bound against the persisted values. **Class 3, owned.**
  *Never-executed means the delivery is undemonstrated, not that the owner is
  missing.*
- **`dealer_reservation_v1` was a false positive, and instructive**: both
  set-sites sit **below** the `cfg(test)` module declaration — test fixtures
  inside a production file, exactly the limitation the lane had stated in
  advance, now producing a concrete hit. Correction: exclude by **line position**
  relative to the tests module, not by path.

**No "named owner, no route home" defect at the never-executed frontier.** Worth
as much as a finding, because it came from **enumeration rather than from
failing to find a counterexample** — all three known instances of that class were
found by accident, and this says the population does not obviously hold more.

### The provenance rule, generalised

Four tooling details in one unit each nearly produced a false finding: `-r ln`
silently rewriting grep output; a zsh glob-no-match aborting a compound `ls`; a
content matcher missing what a **filename** said; and **backticks in a board post
being command-substituted** — the hazard `AGENTS.md` documents for commit
messages, which nobody had noticed applies to board posts too. The last was
caught only by reading back what actually landed.

> **Verify the instrument reported what you think it reported, not just that it
> reported something.** Check the artifact, not the command you believe you ran.

## 2026-09-01 — tightened by a defect, not by a filter

The census filter was corrected to exclude test scope by **line position**
rather than by path — and the first attempt was itself wrong, in the direction
that looks like success.

Taking the **first** `#[cfg(test)]` as the start of test scope is wrong for
every `no_std` crate here: they carry an item-level
`#[cfg(test)] extern crate std;` near the top, so the whole file got excluded.
The measurement came back **43 atom / 139 lamport** — smaller, tighter,
apparently better.

> **Tightened by a defect, not by a filter.**

A correction that moves a number the direction you wanted is the hardest kind to
catch. It was caught because **the custody contract's own request decoders had
vanished from the production set, which they had no business doing** — a
positive control on a *filter*: a thing that must survive, surviving. Same move
that caught the dead `sol_log_64` channel and the disconnected instrument.

The rule: require the next item after the attribute to be a **module**
declaration. An item-level `cfg(test)` is not a cutoff.

**Corrected figures, and the caveat retires:**

| | path filter | corrected |
|---|---|---|
| atom set-sites | 103 | **51** (22 source, 29 destination) |
| lamport set-sites | 391 | **152** |

The old numbers carried roughly **2× in declarations and test fixtures**. *"Upper
bound on production sites"* is replaced by an exact figure, so the C-16 category
closes on a measurement rather than a bound.

**And the correction validated itself on the case that motivated it**:
`dealer_reservation_v1` dropped out of the never-executed join exactly as
predicted, because its two set-sites were the test fixtures that produced the
original false positive. The join went 9 modules / 18 routes → **4 / 5**, and
the conclusion held on the cleaner denominator: **no "named owner, no route
home" defect at the never-executed frontier.**

The atom conclusion also survived its denominator changing underneath it — 39
literal-at-site, 4 wire decodes inside the contract's own decoder (owner is the
authenticated calling program), 8 traced pass-throughs. **Every production site
that sets a compartment has a named owner.**

## 2026-09-01 — the browser can originate one of the three

**Maker/taker trade is stranger-operable** (`d0c2839a`). `JoinPanel` no longer
says the browser cannot build the admission transaction, and no longer publishes
a `--execute` command — and the superseded runbook, the loopback spelling checks
and the `$POSITION_KEYPAIR` line were **deleted in the same cycle as their
successor**, not kept beside it.

Both constraints are structural rather than incidental. **Every address is
derived**: a caller supplies a Market, an owner and this deployment's programs;
the aggregate comes from the Market, the Position and admission records from the
aggregate and owner, the four record raw/staging pairs from content digests
under the Registry's own PDA, the ProgramData accounts from the Loader, the
RentCredit from the Market and its generation. **Every read is finalized at one
floor**, taken once before anything is read and passed to every read after. One
v0 transaction, because the planner says the two rent transfers and the Trading
outer must roll back together.

> **"The browser can sign but cannot originate" is now two capabilities, not
> three.**

### The ratchet caught the author, not a stranger

The ABI ratchet caught the lane writing two Claims seed domains **by hand, under
a comment claiming they were imported rather than restated.**

> **My prose was ahead of my code, and the gate was not fooled by either.**

That is what a ratchet is for: catching an author mid-self-deception, at the one
moment nobody else is looking.

### A private copy would have accepted it

Using the app's own `decodeMarketCoreStateV2` rather than writing a second
header check paid on the first fixture: 360 bytes, and the real decoder said
*why* — the exact current width is 368, that older devnet Market generation is
incompatible. **A private copy of that check would have accepted it.** The
mirror hazard refuted by construction rather than by argument.

### The same refusal, recognised in a smaller costume

The lane declined a sync that would have **created** an SDK twin of the client
operation journal — *"the 132-file drift I refused earlier, wearing a smaller
costume."* Recognising one's own principle at reduced scale is the hardest time
to recognise it.

And two tests moved **with** the behaviour rather than being deleted quietly:
the console assertion that pinned *"Published command · your own key, after an
explicit authorization"* now asserts its **absence**, because `market.join` was
the only listed act that asked a reader for their own key. A test flipping from
asserting a thing to asserting its absence is the cleanest record that a
capability changed.

## 2026-09-01 — a declined citation is not a passing citation

`tools/doc-citations/` (`bc3c7556`), sibling of `tools/seam-audit`: static, no
build, **~7s over 1,143 files**. It instruments the category that made a lane
misreport this morning — **live prose citing deleted symbols**.

**11,875 spans · 1,032 judged · 10,843 DECLINED · 11 unresolved.** The declined
count is *printed*, and that is the design:

> **A declined citation is not a passing citation.** Letting 10,843 read as a
> result would be the same failure the category is about.

Judged: a namespaced path whose leading segment belongs to this workspace,
resolved if its final segment is declared anywhere here. Declined: prose in
backticks, file paths, code fragments, anything rooted in a crate whose items
are not visible. The index covers items, **enum variants and struct fields** —
adding fields alone took the report from 24 findings to 11, because *a report
that is mostly noise is one nobody reads twice.*

**The trade is stated, not hidden:** precision about half, deliberately, because
**a false positive costs a second to dismiss; a false negative cost this tree a
day.** Both false-positive classes are named in the README with examples. A
third is deliberately *not* suppressed — a comment correctly reporting a symbol
as missing still cites it, and **the tool cannot tell a warning from a claim.**

**Results:** all five `process_funded_transition` citations fire, plus a second
true dangle nobody was looking for — `ClusterOriginV1::may_use_seeded_keys`, a
rustdoc intra-doc link naming a method that does not exist, where the live
function is the free `seeded_keys_admissible`.

### Widening an index, checked the honest way round

Three controls, because *a checker that cannot fail is indistinguishable from a
clean tree, and a tripwire that cannot fire is worse than none*: it still
reports the citation it was built from; a synthetic tree resolves items,
variants and fields and refuses the absent one; `--check` exits nonzero on a new
dangle, proven by injection. The synthetic control runs in a **temp dir**,
because a shared checkout must not have a control injecting doc comments into
another lane's file.

It earned its keep immediately, catching the indexer missing members of
single-line `enum E { A }` and `struct S { a: u8 }`. And the widening was
verified the right way: **the real tree's count held at 11**, so a more
permissive index did not quietly resolve the signal away. Most people widen an
index and check the controls still pass; the check that matters is that the
*findings* survived.

Exit code zero unless `--check`: **the category is worth watching before it is
worth gating, and a reporter nobody can turn off is one everybody routes
around.**

**Coverage boundary, flagged rather than left implicit:** Rust doc comments
only. Ordinary `//` comments — where four of the five original citations
actually live — are not scanned yet.

## 2026-09-01 — the tree does not ask who you are

**95 of 120 lamport destinations classified. Zero class 4.** And the population
shrank a *third* time, for a third distinct reason: **32 of the 152 "set-sites"
were not lamport destinations at all** — `LifecycleRegisterCoordinateV3`
register-slot indices in artifact builders, `identity_u16` register ids,
`.to_string()` display code, a function signature.

> **A field named `beneficiary` in a lifecycle-artifact builder names a
> register, not a recipient.**

391 → 152 → **120**. A census that never questioned its own matcher would have
reported 391 owned flows and been wrong three times over, sounding more thorough
at every step.

| class | count |
|---|---|
| **1** owned by the code at the site | **63** (25 canonical zero/absent, 29 derived from a frame account, 9 literal or named constant) |
| **2** read from persisted authenticated state | **27** |
| **3** caller-supplied, bound to a persisted value | **5**, each carried to a verdict individually |
| **4** caller-supplied, only self-consistent | **0** |
| *unresolved* | *25*, stays enumerated |

### The architectural fact underneath it

Every caller-supplied lamport destination carried to a verdict is owned the same
way: **named once when a record is created, then compared against that record
forever after.** Not one is owned by an authority check at time of use.

> **The tree does not ask who you are, it asks whether you match what was
> written down.**

That is why class 4 keeps coming up empty — and it says exactly where such a
defect would have to live: **a flow whose destination is named at *use* time
rather than at *creation* time.** A 120-site grind becomes a narrow hunt.

The new `CustodyRequestV1.rent_refund` verdict is the pattern in miniature:
refused unless it matches the `CustodyReplayV1` record written at
`InitializeReplay`, with `custody-sbf` separately requiring the frame account to
be that key and refusing it being the payer or the replay itself. **Frame ==
request == persisted-at-initialize**, three independent bindings.

### The rule the three corrections earned

> **When a filter makes a number move the way you wanted, name something that
> must survive it and check that it did.**

The generalisation of the positive control — and what caught the worst of the
three, when the custody contract's own decoders vanished from a production set
they had no business leaving.

## 2026-09-01 — a strengthening wearing the costume of a relaxation

The General accelerator's `authenticate_top_level` pinned Trading to instruction
index 1 with the heap grant at index 0 — **two laws where only one was
intended** — and the action provably needs ~516k CU, hence a
`set_compute_unit_limit`, which puts Trading at index 2. Deadlock, proven
jointly: remove the CU limit to satisfy the position and the transaction gets
202,850 CU and dies at 202,842.

**Repaired (`1db08a4e`), and the first guarantee is now *stricter* than the
pinned index ever made it:** every instruction ahead of the current one must
belong to the **ComputeBudget program — which can only price a transaction, and
can neither move value nor touch an account** — and one of them must be the
exact heap grant. Every shape the old rule admitted still is; every newly
admitted shape differs only by additional ComputeBudget instructions.

> **A strengthening wearing the costume of a relaxation.** "We removed a
> positional check" reads as weakening to anyone who does not know the conjunct
> that replaced it — so the conjunct goes in the record beside the removal.

Six top-level shapes now execute against the real ELF. `HeapThenLimit` — the
shape the old rule made impossible — accepts. `Nothing`, `LimitOnly`,
`WrongHeap` and `ForeignBefore` refuse by exact discriminant.

### Nearly shipping the defect you are auditing for

The first red proof caught a defect in the lane's **own** hostile.
`ForeignBefore` sent no heap grant, so deleting the ComputeBudget conjunct left
it **still refusing — on the missing heap**, proving nothing about the conjunct
it exists to exercise. That is M-38, committed by the lane that had spent the
session removing M-38 from other people's tests. Second instance today of an
author walking into their own finding.

Fixed so that deleting the conjunct now yields `Ok(())` rather than a refusal,
which is the only proof that counts.

### What moved

`freeze.rs` had been sending the Trading instruction alone **and then asserting
the execution committed** — two contradictory claims, since the accelerator
authenticates that the heap it runs in was granted. It grants the heap now, and
**width 1 commits for the first time.** Width 258 refuses `0xC003
InvalidScratchBank` — a real defect one layer deeper than the spurious top-level
refusal that had been hiding it.

**The next wall, named by the lane that hit it:** OpenBatch still refuses
`0xC002` at N=2 from a *later* conjunct — and **eight distinct conjuncts share
that one discriminant**, so the program cannot say which.

> **A refusal that cannot name its own cause is how this one stayed hidden
> behind the geometry wall for so long.**

## 2026-09-01 — C-16 has no blanks left, and none of it is finished

The `AUTHORITY` class landed as seam-audit's seventh (`9f9f943c`), closing the
last unmeasured C-16 category. **Six categories, six instruments, none
finished** — and that sentence is the deliverable, because a reviewer handed
*"C-16 fully instrumented"* reads completeness while *instrumented, not
finished* reads a starting line.

**The question it asks:** does an act establish who may perform it, or read an
answer somebody else supplied? Subject is the Registry activation cache, because
**a cached role is where authority most easily goes unexplained** — the
authentication happened once, in another program, and everything downstream
reads the result.

**7 → 3 findings across four refinement passes, every one by measurement, none
by loosening.** Three scoping decisions carried it:

- **One hop of intra-crate call resolution**, principled rather than convenient:
  *a cross-crate helper is a seam, and seams are this tool's whole subject.* A
  reader that could not see one hop called three functions unexplained when the
  explanation was one call away — widened by exactly one, not until the noise
  stopped.
- **Scoped on `try_borrow_data`, not on the signature.** The first attempt
  scoped on `AccountInfo` and **silently dropped a real candidate** that passes
  accounts inside a typed frame. A scope rule that quietly removes true findings
  is the worst kind, and it was caught by checking what *left* rather than what
  remained.
- **The blessed crate's real vocabulary**, discovered by reading it rather than
  assuming it.

### The difference between the two verdicts is the discipline

**Custody: benign, verified.** Provenance is carried **in the type** —
`authenticate_market_admission` resolves the cache once, both variants of
`AuthenticatedMarketAdmissionV1` carry a `cache_bump`, and the realm functions
are reachable **only by matching on it**. Authenticate once, prove it in the
type.

**Trading `selected_role_programs_v3`: hazard, open.** The same token argument is
*plausible* and **was not established** — the lane read Custody's chain end to
end and did not read Trading's.

> **A tag is a claim, so it gets a hazard. *Not shown to be wrong* is not
> *shown to be right*.**

That is what separates a census from a scoreboard, and writing "benign, same
pattern" would have been effortless.

The class docstring carries both epistemics in the same words as the atom
census: **silence means provenance, never correctness** — a function that
derives the right address and reads the wrong role reads as derived and is still
wrong. And the scope is stated because it bounds the claim: one authority
object, `programs/` only, one hop.

`AUTHORITY` and `PRIVILEGE` are distinguished in the README header because they
look alike and **point opposite ways** — one hunts under-constraint, the other
over-constraint.

## 2026-09-01 — four families, one root

**99 of 120 lamport destinations classified. Still zero class 4.** And the four
strongest candidates — sorted by *named at use time vs at creation time* — all
cleared to the **same root**.

The real candidate was `ResolutionRoleRequestV1/V2.beneficiary`: required nonzero
at `CreateFund`, zero at `AdmitTerminal`, and **nonzero again at `CloseFund`** —
named at creation *and again at use*, which is class 4 unless close checks. It
does: refused unless the request matches **both** `state.rent_beneficiary` **and**
the frame's close-beneficiary account, with `VerifyFundReady` carrying the same
double binding.

### The structural result

**Every caller-supplied lamport destination carried to a verdict roots in one
value: the Market state's `rent_beneficiary`, written once at founding.**

- Custody's `CustodyReplayV1.rent_refund` was itself seeded from
  `state.rent_beneficiary` at open;
- Direct replay setup and token setup are bound to / derived from
  `market.rent_beneficiary`;
- Resolution funding is bound to `state.rent_beneficiary`;
- provider transport and sponsored push are bound to a lifecycle
  `refund_recipient` persisted at creation.

**Four independent families. One root.**

> **The protocol's answer to "who may receive lamports" is nowhere an authority
> check — it is a single founding-time value that everything else is compared
> against.**

Two consequences, and both narrow the search rather than widen it:

1. **The founding is the only place this ownership can be got wrong** — a defect
   at the root is a defect in all four families, and no downstream comparison
   would notice, because they would all agree with each other perfectly.
2. **A class-4 defect must therefore be a place where the comparison is
   *missing*, not weak.** "Audit 120 sites for weak checks" becomes "find a path
   with no check at all."

| class | count |
|---|---|
| 1 — owned by the code at the site | 63 |
| 2 — read from persisted authenticated state | 27 |
| 3 — caller-supplied, bound to a persisted value | 9 |
| **4 — unowned** | **0** |
| unresolved | 21 |

**The caveat, kept in the lane's own words:** *"I have not proven no unowned
lamport flow exists. A hostile reviewer should read this as sweeping a
well-defined population with a stated method — not as a proof."*

## 2026-09-01 — the instrument indicted the reader who built it

The doc-citation scanner's coverage boundary — *Rust doc comments only* — was
closed by measurement rather than documented as a caution (`47edccea`).

| corpus | spans | judged | resolved | **dangling** | declined |
|---|---:|---:|---:|---:|---:|
| `///` and `//!` | 11,878 | 1,032 | 1,021 | **11** | 10,846 |
| ordinary `//` | 2,571 | 130 | 127 | **3** | 2,441 |

**Not because the ratio is better — it is worse**, 95% declined against 91%.
Because the signal filter is what handles a bad ratio: prose does not
accidentally contain `crate::module::symbol`. Widening by 2,571 spans added
**130 judged and 3 findings**. It did not explode, so the boundary closes,
**stated with a number instead of an expectation.**

### And the number indicted the hand count

Four of the `process_funded_transition` citations live in `//` comments — and
the total is **six, not five.** Two had been missed by eye: `core-sbf/tests.rs`
and a second in `local-validator/.../market.rs`.

> That is the whole argument for an instrument rather than a careful reader —
> **made against the reader who had been careful about this exact symbol an hour
> earlier.**

**A third true dangle, visible only once `//` was in scope:**
`hot_v3::authenticate_hot_artifacts` at
`trading-sbf/src/dealer/v3_hot_artifact.rs:1346` has no definition anywhere; the
nearest live names are `authenticate_hot_program_v3` and
`authenticate_hot_invocation_v3`. A comment explaining what a refusal derives,
naming a function that does not exist — **in a program the lane never touched.**

The two corpora are reported **separately rather than summed**, because they
have different ratios and *a future reader deciding whether to gate one of them
needs the split, not a total.* Controls extended: the synthetic tree now dangles
a symbol in a `///` **and** a `//` comment and both must be reported.

**Routed with owners:** `trading-sbf` owns `authenticate_hot_artifacts`; the
local-validator lane owns `ClusterOriginV1::may_use_seeded_keys` and the second
`market.rs` citation.

## 2026-09-01 — one capability, not three

**"The browser can sign but cannot originate" is now one capability.** Trade is
stranger-operable; creation's authoring path is a transport question with a
producer and a copy-out handoff already; **redemption is the one that still
stops at an artifact only a Rust binary can author — and it now has a
decomposition rather than a shrug.**

### The impurity is not authority

`produce_wallet_terminal_input_v1` is not a pure planner — file I/O, its own
RPC, a cluster-origin policy. But *where* the impurity sits is the finding:

- from `--plan` it takes **six values, nothing else** (enumerated by walking
  every `plan.*` access): five program ids and a release-set id. The browser
  holds five from the deployment and reads the sixth from the Market's own Core
  state — which the admission snapshot module already does.
- from `--evidence` it takes a routing table of addresses to observe, plus a
  `plan_sha256` that binds **the CLI's two files to each other**. Not a
  protocol check.
- **SBF program crates the derivation touches: zero.**

> Two artifacts that are a deployment table and an address book — and the
> browser derives both.

The blocker is structural, not semantic: the derivation is `pub(crate)` in a
**binary** crate that declares its own `[workspace]` and depends on
`dclutch-claims-sbf` for its *other* subcommands. Extraction is needed, exactly
as `dclutch-operator` had to shed `dclutch-trading-sbf` — which turned out free.
**The lane did not dissolve that architectural boundary unilaterally.**

### Consent, not convenience

`/found` asked a stranger to transcribe **their own public key**; the payer now
fills from the connected wallet, with an edit still overriding, because the
payer need not be the wallet reading the page.

The refund wallet deliberately does **not** follow: it is **immutable once the
Market-bound RentCredit embeds it**, so defaulting it silently would decide
something permanent for a reader who never looked at the field. A test pins that
it may still differ.

### The model refused to be impressed

The regenerated capability surface still reports `authority: "none"` for that
page —

> a page that grew a wallet directory and kept *"This browser · no key, no
> signature"*, because reading an address reaches no wallet request. **The model
> refusing to be fooled by the appearance of one.**

A derivation earning its keep against its own author's expectation — the third
time today an instrument has done that.

## 2026-09-01 — the measurement declined to authorize the removal

`coefficient == denominator` was to be settled by measurement: remove the
emission, drive issue → denominate → redeem → retire, and see whether
conservation still closes. **The removal did not break conservation.
Conservation was never reached.**

With the constraint removed the fold stopped refusing and the campaign advanced
past `build_bundle` for the first time — into `TradingSbfError::HeapFrame`
(`0x4008`), because the operator returns a bare `Instruction` and nothing was
adding a `RequestHeapFrame`. Grant added (`a0bd979f`). Behind **that**:

```
admitted (not hostile, not corrupted) IssueStructured, 203,408 CU,
inside a real Token-2022 PermissionedBurnExtension CPI:
  request  65,536 -> Access violation writing 8 bytes at 0x30000fcf8
  request 262,144 -> Access violation writing 8 bytes at 0x30003fcf8
```

### The third instance, and this one faults instead of refusing

**`require_extended_heap_admitted_v1` — the check that says the grant arrived —
reads the ComputeBudget *request* from the instructions sysvar.** What the
program **asked for**, never what the runtime **gave**. Both sides of the check
move together: the transaction asks for N, the check reads N, the check passes.

The bump heap reserves 776 bytes at its floor and its scratch half bumps **down
from that admitted ceiling** — so **both faults land exactly 776 bytes below the
requested ceiling, and the fault tracks the request.** Raising it only moves the
write further out.

> **Guards whose two sides move together, third instance tonight — and the first
> that ends in a memory fault rather than a refusal.**

The crux, which is a ruling about what a program may trust from its own
instructions sysvar: **a Solana program cannot directly observe the heap it was
granted**, so any check claiming to verify the grant is necessarily reading the
request, and the name promises what the mechanism cannot deliver.

### Why the constraint went back

The admitted path faults **identically to the hostile — same CU, same address**
— so the fault is upstream of every economic step and **settles nothing** about
the coefficient law. It also does not harden it: the constraint is not what was
preventing the fault. **Restored**, `open_structured_v3.rs` byte-identical to
HEAD, suite back to 44/45 with the identical
`transition fold refused at operation 4`.

Measure-then-remove worked exactly as intended: **the measurement declined to
authorize the removal.** The ruling is now *blocked* rather than merely
undecided — it cannot be settled until the heap fault is fixed, because nothing
downstream of it executes.

One more M-38 from the same episode: the hostile's `!accepted` half **passed
while the reason was completely wrong** — an extended-heap refusal standing in
for a substituted-digest refusal — and **only the discriminant assertion caught
it.**

## 2026-09-01 — the address is not the recipient

The root of every lamport ownership chain — `CoreState.rent_beneficiary`,
written once at founding — is **class 3 in substance, class 4 in form.**

**The authentication is the strongest in the scheme.** `generic_founding_v1.rs`
requires the rent-credit account to be owned by the Rent program, exactly
`LIFECYCLE_RENT_CREDIT_BYTES_V2` wide, rent-exempt at that width, decodable as
`LifecycleRentCreditV2`, **and** to have a key equal to `create_program_address`
over the credit's own seeds — domain, market, generation, bump. For a given
market and generation, **exactly one account can satisfy it.**

**But the address is not the recipient.** The real lamport recipient is the
`refund_wallet` *inside* the credit, and it is compared against a
**caller-supplied** `beneficiary` argument. If the founder created the credit,
both sides of that comparison are the founder's — the class-4 shape, at the one
site where it would propagate to all four downstream families at once.

**Why it is not a hole:** the lamports being directed are **the founder's own
prepaid rent**. A payer naming where their own refund goes is ownership by the
payer — the same principle that cleared `refund_recipient: payer.key`. And it
does not loosen afterwards: the Rent program refuses any sweep whose wallet is
not the persisted `refund_wallet`, and the only write is at creation. **Set
once, compared forever, no mutation path.**

*A weaker census would have seen `create_program_address` and stopped.*

### The sharpened question, worth more than the verdict

> Safe **exactly as long as the only lamports reaching `rent_beneficiary` are
> the rent the founding prepaid.** The moment any *other* lamports route there —
> a fee, a bounty, a crank reward, a surplus, another party's rent — the founder
> is directing money that is not theirs, **and no downstream comparison would
> notice, because all four families agree with each other by construction.**

So the whole census reduces to one subject: **enumerate what credits a
`LifecycleRentCreditV2`, and check that every source is rent the same party
prepaid.**

The arc, each step narrowing the last:
*audit 120 sites for weak checks* → *find a path with no check at all* →
**find a path that puts foreign money into the one account everything trusts.**

### A lane that miscounts its own commits

The Structured lane wrote "eight commits" and listed seven, then corrected it
unprompted:

> A lane that miscounts its own commits in the same message that asks others to
> trust its measurements should say so.

Nobody would have checked. That is why it counts — the same standard as
retracting the fourth-binding claim, restoring the coefficient constraint when
the measurement declined to authorize its removal, and repeating both evidence
caveats every time: phase 1 runs a test-caller ELF in Trading's slot and is
**not Trading evidence**, and the ledger's Token-2022 is the **macOS-arm64 audit
artifact** digest-gated against provenance, not the canonical Linux one.

## 2026-09-01 — one side that does not exist, and a field refused

**Selector 9's account admission is unsatisfiable by anything.**
`v3_trade_profile.rs:271` puts `RequireKey{ account: OBLIGATION_V4, expected:
common(116) }` in the **account** profile. Register 116 is written **only** by
the **request** profile. The runtime order settles it: `project_accounts_atomic`
→ `mem::swap` → `request_profile.project_atomic`. The account pass runs **first**
and reads `input_identities`, so 116 holds unwritten zeros.

> **Not two sides moving together — one side that is never written at all.**

Independent of the `derivation_policy` wall, and convicted entirely from the
pass order.

**No test was written, and the reason is the finding's twin.**
`AccountProfileV2` exposes no public operation accessor, so the only available
assertion would have been against the builder's own input array — **builder as
its own witness**, the precise hazard. *Convicted by reading, not faked into an
assertion.*

### A field refused because adding it would have looked like a fix

The `basis_scale` repair needs the value carried into `MultiLpContextV3`. The
lane declined to add the field on its side:

> A scale field that nothing authenticates would default to 1 at every call
> site, **reproduce today's behaviour exactly, and look fixed** — a declaration
> never executed against reality, which is the class this whole night convicted.

And the axis argument for where it must come from: the **authenticated Core
market state**, because `basis_scale` is a per-market founding value and a
descriptor is a release artifact authored *before any market exists*. Restating
it in the descriptor is the same axis violation that produced `derivation_policy`.
The intentional red stays red until the authenticated value exists.

### Two operational traps, both new to the ledger

- **`pgrep -f "cargo test"` matched another lane's waiter shell**, so a liveness
  probe reported a dead run alive. The dead-channel shape wearing process-table
  clothes; **log mtime** was the positive control that caught it.
- **Two runners contended on one target dir**, both truncating the same log with
  `: > "$log"`. Killing one probably killed the one holding the lock.

And the staging that made the evidence worth having: built from a clean
`git archive HEAD` plus the preserved patch rather than the working tree that
produced the ambient `Geometry` cluster last time — then **verified the tree
under test actually carried the repair** (`derivation_policy() !=
descriptor.lifecycle().program()` at zero occurrences; per-root constant in 3/3
builders). *Check that the thing you are measuring is the thing you think you
are measuring.*

## 2026-09-01 — the check that answers it lives outside the four families

**No foreign-money path into `LifecycleRentCreditV2`.** The Rent program's
instruction surface is exactly three — `Create`, `Sweep`, `Close` — and every
lamport movement was read: `Create` moves payer → credit; `Sweep` and `Close`
move credit → wallet plus the crank reward. **Not one ingests lamports from a
third party.**

**The cross-program route is where the defect should have been.** Other programs
credit a lifecycle credit on account close — the registered Direct profile
requires the credit writable *"so a Close may credit it"* — and in registered
Direct creation the **maker** pays for both records it creates while the credit
is the **market's**. That is exactly the shape that puts one party's prepaid rent
into another party's directed account.

**It is gated by a fifth, independent check nobody had counted.**
`account-profile-contract/lifecycle_v3.rs:3174-3183` refuses `InvalidRent`
unless `credit.beneficiary` equals a beneficiary identity register **the
AccountProfile projects** — and the registered profile projects each created
record's own `RENT_OWNER`.

> **Rent ownership is tracked per record, not pooled.** A record whose rent one
> party paid cannot be closed against a credit whose refund wallet is another's.

And it answers the question **precisely because it lives outside the four
agreeing families.** Four families agreeing with each other proves nothing; a
fifth thing that would refuse them all proves something.

So `rent_beneficiary` stays **class 4 in form, class 3 in substance** — but the
substance is now enforced by a register equality, not by the founder happening
to own both sides of a comparison.

**Verified and not-verified kept apart.** Verified: the three-instruction
surface and its moves; the full credit-identity derivation; the register-equality
gate; the registered profile's `RENT_OWNER` projection. **Not verified:** that
*every* family's lifecycle policy points its `selected.beneficiary` register at
the record's own `rent_owner`. **A family that pointed it elsewhere would reopen
exactly this hole** — one read per policy, and the only place left for it to
hide.

The arc, complete: *audit 120 sites for weak checks* → *find a path with no
check at all* → *find foreign money in the account everything trusts* → **and
the check that stops it turns out to live in the account profile, not in any of
the four families.**

## 2026-09-01 — convicted by 248 bytes

The experiment that decides the heap ruling ran, and returned **outcome 3** — by
the smallest possible margin.

| requested frame | form | runtime | program |
|---|---|---|---|
| 32,768 (= default) | legal | accepted | refuses `HeapFrame 0x4008` — equal to default is not an *extension* |
| **33,792 (default + 1 KiB)** | legal | **accepted, runs, grants the default** | write at 33,016 = 33,792 − 776 → **violation 248 bytes past the default heap** |
| 65,536 | legal | accepted, grants the default | violation at 65,536 − 776 |
| 262,144 (= max) | legal | accepted, grants the default | violation at 262,144 − 776 |
| 524,288 (> max) | malformed | **rejected before execution, zero logs** | never runs |
| 65,000 (not a 1,024 multiple) | malformed | **rejected before execution, zero logs** | never runs |

**The runtime does not refuse a transaction whose `RequestHeapFrame` was not
honoured.** It validates the **form** — bounds and granularity — rejecting
malformed requests before execution with no logs at all. It then **accepts a
well-formed, in-bounds, above-default request and does not apply it**, running
on the default 32,768 while the instructions sysvar still reports what was asked.

The 33,792 row is the whole proof: a legal request one kilobyte above default,
the ceiling tracking it exactly at `request − 776`, and the write landing **248
bytes past** where the default heap ends. The two malformed rows are the control
that makes it airtight — the runtime *is* rejecting things, just not this.

> **What the check may soundly conclude is "the request was well-formed and
> above the default."** What it claims is that the frame was **granted**, and the
> program has no way to observe the difference.

### The caveat that may be bigger than the finding

Measured in `solana-program-test 4.3.0-beta.2`. *"I cannot distinguish from
these runs whether **no** runtime applies the frame or whether **this harness**
does not."* The repair is warranted either way — the program still cannot
observe the grant, and **there exists a runtime inside this project's own
evidence base where request ≠ grant.**

But if the gap is ProgramTest's:

> **Every ProgramTest measurement of a route that actually allocates past 32 KiB
> has been running on a smaller heap than the route asked for** — so any capacity
> number from such a run describes a program that believed it had more heap than
> it had.

Routes never exceeding 32 KiB are unaffected. The settling experiment is a
`solana-test-validator` submission of the same instruction at 33,792: fault means
the runtime class does not apply it; success means a sweep of affected routes and
recorded numbers is owed.

## 2026-09-01 — the wall spoke

Eight causes behind eleven refusal sites became eight codes
(`0xC005`–`0xC00C`, `e92d759f`), allocated contiguously from the registered
base, **welded to the enum by the exhaustive `ordinal` match the band assertions
walk — so a fourteenth variant will not compile until its author answers for
it.** `0xC002` keeps its numeric value and **narrows** to the cause nearest its
old name, so a code already seen in a log still means a *subset* of what it
meant, never something else. The only safe way to split a published
discriminant.

The program-test sweep stops asserting one code six times: each row now names
the single cause it exercises — *which is what makes them six tests rather than
one test written six ways.*

**Then the wall spoke.** OpenBatch through real Trading ELFs at N=2 refuses
**`0xC00A InstructionsSysvarAccount`**: the account at
`ADMITTED_INSTRUCTIONS_ACCOUNT_V3` is not the readonly instructions sysvar. The
lane then measured the same bundle host-side — **the sysvar is present in the
transaction, readonly and unsigned, at instruction index 29.** So the account is
right and its **position in the admitted-AOT transport frame** is wrong.

> **That is a claim nobody could have made an hour ago, because the program
> could only say "top level."**

The refusal narrowed it to a coordinate; the host measurement proved the account
exists and is correctly shaped. Neither alone would have located it. That is the
argument for refusal granularity, demonstrated rather than asserted.

### The two framings, kept

**Stricter, not looser** — because the diff reads as a relaxation without it:
*the pinned index constrained one instruction by position; the new rule
constrains all of them by capability.* Every shape the old rule admitted still
is; every newly admitted shape differs only by additional ComputeBudget
instructions.

**And the author's own mistake**, which is the more valuable of the two: an
M-38 nearly shipped into the audit *for* M-38 — a hostile that sent no heap
grant, so deleting the conjunct under test left it still refusing on the missing
heap.

> **Every convention enforced by attention rather than by a gate should be
> assumed already broken somewhere, including by the person who wrote it** — the
> mutation test caught it, not review.

**C-05 moved from *cannot execute at all* to *executes and refuses somewhere
specific enough to fix*, in six commits.**

## 2026-09-01 — the fifth check is real but NOT uniform

**Correction to the entry above.** The register equality at
`lifecycle_v3.rs:3181` was credited with closing the rent-credit hole. It does —
**for four of the five live families. For Series the same check cannot fail.**

| family | register projected from | verdict |
|---|---|---|
| Direct registered | the maker replay's and record's own `RENT_OWNER` | **good**, traced |
| Dealer LP (v3+v4) | LP record offset 152 = that record's own `rent_refund` | **good**, traced |
| General (8 sites) | same `OBSERVATION` convention | **good by naming** — not all eight projection sources read, and said so |
| Direct inline | same shape as its registered sibling | not separately traced |
| **Series** | **the rent credit account itself**, at `LIFECYCLE_RENT_CREDIT_REFUND_WALLET_OFFSET_V2` | **vacuous** |

`funding_artifacts_v5.rs:314-318` projects that register **out of the credit's
own `refund_wallet` field**, so `:3181` compares `credit.beneficiary` against a
value read from the same account's same field. **x == x.** A guard whose two
sides move together — **sitting inside the very mechanism that had just been
credited with closing the root hole.**

**Series is still bound, just not by that.** `core-sbf/series_consume.rs:844`
refuses unless `request.beneficiary == ticket.refund_owner()`, and `:884`
refuses unless `credit.refund_wallet() == request.beneficiary()`. Transitively
the credit's wallet equals the ticket's own refund owner — the party who
prepaid. **No hole.**

### The correction a reviewer needs

> **Do not tell anyone the account profile gates this uniformly.** It gates
> Direct, Dealer and General; for Series the account-profile gate is
> **decoration** and Core carries the weight alone.

Anyone hardening Series who deleted those two Core conjuncts believing the
profile still covered them would open the hole — **and every downstream
comparison would keep agreeing**, which is the exact failure mode this census
exists to find.

And it is why the residual was worth doing rather than trusting the previous
turn's answer: **the mechanism was real, but it was not uniform, and "four
families agree" was never the thing that made it safe.**

## 2026-09-01 — the lamport census closes, and its own arithmetic was wrong

**Correction, and it invalidates two figures recorded above.** The tallies
reported as *"99 of 120 classified, 21 enumerated"* were built from **different
regex passes**, and 63 + 27 + 42 is **132, not 120**. Rebuilt as one
authoritative pass:

| class | count | |
|---|---|---|
| **1** owned by the code at the site | **54** | 25 canonical zero/absent · 16 frame-derived · 13 literal or named constant |
| **2** read from persisted authenticated state | **40** | 30 by shape, incl. every record's own `rent_owner` · 10 decoders into state records or receipts |
| **3** request-borne, bound to a value persisted earlier | **26** | |
| **4** caller-supplied, only self-consistent | **0** | |
| | **120** | |

Of 152 raw set-sites, **32 are not lamport destinations at all** — register-slot
coordinates, `identity_u16` register ids, display code, a function signature, a
comparison, type positions. *Anyone who wrote down "99 of 120" should replace
it — including this ledger, which did, twice.*

The last two resolved by reading rather than pattern:
`direct_inline_route_v3.rs:2022` is the branch where the maker replay is
**vacant and about to be created**, so its `rent_beneficiary` is the
creation-time naming — with the observation register zero precisely because
there is nothing to observe yet. Class 3 at its creation event.
`provider_finalized_projection_v3.rs:1872` builds a `CoreState` fixture with a
literal. Class 1.

### The result, stated as narrowly as it deserves

> Every lamport destination in the production tree is owned, and the ownership is
> always one of three things: **the code chose it, it was read out of a record
> that already existed, or the caller named it and the chain refused unless it
> matched something written down earlier.** Not once is it an authority check at
> time of use.

The same structural fact the atom half produced, reached from the other
substance.

**Not proven, verbatim:** *I have not proven no unowned lamport flow exists. I
swept a well-defined population — five destination-naming fields, production
code only, `cfg(test)` modules excluded by line position — with a stated method.
A hostile reviewer should read this as sweeping a well-defined population with a
stated method, not as a proof.*

**Three gaps stay named:** General's registers are **good by naming** (not all
eight projection sources read); **Direct inline is not separately traced**; and
the **Series account-profile gate is decoration**, with Core carrying that
binding alone.

## 2026-09-01 — no cross-LP subsidy, in actual numbers

C-06's acceptance question is one sentence — *can LP A's capital ever fund LP
B's outcome* — and it is now answered over **the real planner**,
`plan_pool_equity_v3`, the same function the accelerator runs on chain
(`f45cfd78`).

**The tests never re-derive its arithmetic**, and the reason is the discipline:

> A test that recomputed `floor(burn * residual / supply)` and asserted the
> planner agreed would assert that **one copy of a formula equals another**.

The terminal life, every split computed by the planner:

```
LP A founds with 100 cash           -> 100 shares
venue trades: 40 cash + [0,60,120]  -> residual [40,100,160]
LP B joins LATER at that value      -> 200 shares, residual [80,200,320]
venue earns 10                      -> residual [90,210,330]
both exit, in BOTH orders
measured: A [45,105,165] · B [45,105,165] · pool [0,0,0]
```

- **Conservation exact per scenario**: 45+45+0 = 90, 105+105+0 = 210,
  165+165+0 = 330. Nothing created, no dust stranded.
- **No first-mover subsidy**: `a_first == a_second`, `b_first == b_second` —
  exiting first is worth exactly what exiting second is worth.
- **B took none of A's capital**: contributed `[40,100,160]`, extracted
  `[45,105,165]` — a gain of exactly **5**, exactly half the **10** the venue
  earned.

### Teeth, checked rather than trusted

The corpus test **refuses to pass on an all-continue sweep**. Tightening B's
bound from +5 to +4 makes the life test **fail on the real numbers**, so the
assertion binds **exactly rather than slackly**. And the sharpest control:

> A mutation minting B 150 shares for the same basket **does not reach any
> assertion at all — the planner refuses it**, because issuance is exact
> cross-multiplication. **Mis-minting subsidy is structurally impossible, not
> merely detected.**

### Scope, stated plainly by the lane

This is evidence about **the production planner arithmetic, executed** — **not**
end-to-end on-chain evidence. Selector 9's account admission is separately
unsatisfiable (register 116 is written only by the later request pass), so the
physical venue cannot run today, and the two inventory moves are applied to the
pool directly because a trade is exogenous to the equity kernel.

## 2026-09-01 — a hostile that reached its subject and had no word for it

`InvalidScratchBank` carried every cause in `assemble_input_bank` — mis-privileged
page, undecodable page, page belonging to another request, page out of streamed
order, and a bank that reassembled wrong — all one code. Split into six
(`0xC00D`–`0xC012`, `4c90cdf5`), with `0xC003` keeping its numeric value and
narrowing, same migration discipline as `0xC002` before it.

**Length is split from digest deliberately, and that split *is* the diagnosis:**
a length mismatch means the pages do not add up to the declared bank — transport
arithmetic. A digest mismatch means they add up **exactly** and carry different
bytes — content. **Nothing else distinguishes them.**

`freeze.rs` at width 258 now refuses **`0xC011 ScratchBankDigest`**: the pages
sum to exactly the declared length and the bytes differ. Width 1 still commits.
The lane **stopped there rather than guessing** — unlike OpenBatch, where a host
measurement located it, here there is no equivalent measurement yet, so *"the
refusal narrowed it from scratch bank to content-not-arithmetic at one width;
that is real but not yet located, and I would rather say so."*

### The refinement to M-38, and it is a real one

`AGENTS.md` says a bare `is_err()` is a test of nothing. True — and it has been
read as an accusation of laziness. There is a second cause:

> **A bare `is_err()` is sometimes a symptom of an undifferentiated refusal
> upstream, not of a lazy test.** Splitting the code is what makes the assertion
> writable.

`corrupted_scratch_page_refuses_without_mutating_selection` had survived because
**there was no code to name instead** — *a hostile that reached its subject and
had no word for what it found*, as against the ones this campaign keeps finding
that never reach their subject at all. **Different defects, different repairs**,
and the ledger had only described one.

**And the assertion was predicted before it was run.** The fixture flips one byte
of the **last** page's payload, so the page still decodes and still binds to its
request, and what fails is the bank those pages reassemble to — therefore
`ScratchBankDigest`. The run returned `Custom(49169)` = `0xC011`. **Prediction
then confirmation** is the strongest form a refusal assertion takes, and it is
only available once the vocabulary exists.

**C-05, in seven commits: from *cannot execute at all* to *executes, and every
refusal on the path says which conjunct it is*.**

### The log was the dead channel; the filesystem was the live one

Third instance of one shape from a single lane in a single session. A run was
reported stalled on a file lock. `lsof` showed **its own** cargo holding the lock
— 40 target files touched in 120 seconds, one rustc live — and the
`Blocking waiting for file lock` line was **stale text**, because cargo's
progress output is block-buffered to a redirected file.

Preceded by the dead `sol_log_64` channel (no output, and no `Program log` line
anywhere in the run, including the successful path) and by
`pgrep -f "cargo test"` matching **another lane's waiter shell** and reporting a
dead run alive.

> **Any single signal can be the disconnected one, and you only find out by
> checking a second.**

All three were caught that way: a positive control, a process-table cross-check,
and the filesystem against the log.

## 2026-09-01 — the heap exposure is the build flag, not the route list

The census that bounds the heap finding. **The exclusions are the useful half:**

1. **Only `dclutch-trading-sbf` carries the `custom-heap` feature.** Claims,
   Core, Custody, Registry, Resolution and Rent use the stock allocator and
   **cannot exceed the default, whatever a transaction asks for.**
2. **Only two files** touch the scratch region that bumps down from the admitted
   ceiling — `hot_v3.rs` (35 sites) and `entrypoint_adapter.rs` (17, the
   implementation). Nothing else in the tree can produce the fault.
3. The Registry **continuation** route was structurally closed on 2026-08-27: it
   fits the default, carries no grant, and its packet has **four spare bytes**,
   so it could not carry one anyway.

**The route list is adapter-owned and exhaustive** — `entrypoint_adapter.rs:1294`
returns true for exactly four things: `DCLTHOT3`, `DCLTSEL1`, generic market
founding v3, and **every route when built `--features hot-cu-profile`**. Grant is
65,536.

`DCLTSEL1` was added on 2026-08-31 **because the Hot arm had left it dead —
every seal write refusing `HeapFrame` unconditionally** — which independently
corroborates an earlier session's finding that the seal outer had been dead since
08-30.

### The sharpest exposure

> **`--features hot-cu-profile` lifts *every* route onto the extended heap**, so
> any phase subtotal from a diagnostic build was measured with a grant in play.

Two figures from this session are explicitly diagnostic-build: the DIRECT
campaign's **323,523 CU**, and the **311,068 CU** Buy. Being re-measured both
ways — if the number moves, the diagnostic build is measuring a different
program than the one that ships.

**The CU budgets file is not contaminated and says so itself**: no green number
to pin for the canonical Hot bundle because *"the tail is over the 32,768-byte
heap at phase 7"*, and the phase subtotals *"recorded from ADR 0005's own
measured table rather than re-measured."*

**Not verifiable from inside a program**, and stated as such: whether any
specific recorded number was taken on a run where the grant was actually
*applied*. The adapter reads the ceiling the request **asked for**, never what
the runtime **gave**.

**And the positive control, fourth instance from this lane:** the filter must
not lose `hot_v3.rs`, the route the whole question is about. It survived with 35
scratch sites. *Had it vanished, the census would have been wrong.*

## 2026-09-01 — the diagnostic build is a different program, by 15,682 CU

Measured both ways: same route, same harness, **five role ELFs held
byte-identical**, only the trading ELF's build flag differing.

| build | `REGWALL sell action-wall refusal cost` |
|---|---|
| ordinary (ships) | **331,274 CU** |
| `--features hot-cu-profile` | **346,956 CU** |
| **delta** | **+15,682 CU (+4.7%)** |

> **Any phase subtotal recorded from a `hot-cu-profile` run is not a measurement
> of the program that ships**, and every such figure should carry that label.

**The control could have failed.** The ordinary trading ELF was rebuilt *after*
both runs and is byte-identical to the one used (`c5d5444…` both times), so the
source did not move despite HEAD advancing ~6 times during the unit. And the two
ELFs genuinely differ — `c5d5444…` vs `c90a82a…`, 2,288,328 vs 2,294,328 bytes —
which is the precondition the experiment needs.

**Not attributed, deliberately.** Two candidate causes this measurement cannot
separate: the ~10 `hot_cu_checkpoint!` / `hot_heap_mark!` sites becoming real
`sol_log` syscalls, and `hot_cu_profile_lifts_every_route_v1()` returning true so
*every* route takes the extended-heap path. **The second is the one that matters
for the heap question**, and guessing between them would have been the finding's
only weak point.

### Two corrections to figures recorded above

1. **This ledger recorded 323,523 CU as a diagnostic-build figure. It is not** —
   it is the *ordinary*-build measurement, and its diagnostic sibling is the
   *451 CU past `preflight-children`* beside it. Corrected.
2. **323,523 does not reproduce at HEAD.** The ordinary run today is **331,274**,
   +7,751 higher. **The recorded figure is stale**, so a reader comparing against
   it now would conclude something moved when what moved is the tree.

### The fifth instance, and the first one that was its own

The lane's first source-stability control hashed nothing and reported
`d41d8cd98f00b204e9800998ecf8427e` **before and after — the md5 of the empty
string.** It compared nothing to nothing and would have passed whatever
happened. Caught by **recognising the constant**, and replaced with a
rebuild-and-compare that can fail.

> *Name something that must survive the filter and check that it did* —
> **including when the filter is your own measurement harness.**

## 2026-09-01 — three hypotheses eliminated, and a refactor reverted unverified

The `0xC011 ScratchBankDigest` cause at width 258 is **narrowed, not located**,
and the narrowing is four eliminations rather than a guess:

1. **Geometry tiles exactly at every width.** At 258:
   `total_bank_bytes == bank_len == 15032`, 18 pages, offsets contiguous at 880,
   final page exactly 72 bytes. Same at 1, 2 and 257. **No boundary or length
   fault.**
2. **The page codec round-trips byte-for-byte at both widths** — carve → `new` →
   `encode_into` → `decode` → concatenate reproduces the bank exactly, with
   **position-dependent filler** that would expose any offset shift.
   `first_diff = None`. **No padding fault.**
3. **The page window aligns at both widths** — the fixture writes
   `vec![DUMMY; 18 + fixed_count]`, the program derives
   `ADMITTED_RUNTIME_ACCOUNTS_START_V3 + fixed_count`, the constant **is** 18,
   and `fixed_count` for Freeze is width-independent.

**The fixture's page transport is exonerated.** What remains: *what the program
reads at those coordinates is not what the fixture wrote, at 258 only* — the same
family as the OpenBatch finding, where the account was correct and its
**position** was not. Named rather than claimed, because it is unmeasured.

### A refactor written, then reverted rather than committed unrun

`18` is `ADMITTED_RUNTIME_ACCOUNTS_START_V3` **restated as a bare literal, four
times**, in the file whose scratch pages sit at the far end of exactly that
offset — a second author for a frame constant, in the place a drift would be
hardest to see. The derivation was written and could not be verified: the
accelerator's **nested workspace resolves `dclutch-core-contract` under two path
spellings**, so `dclutch-operator` and `dclutch-direct-codec` compile against
different copies.

> The refactor is provably behaviour-preserving — but **"provably" is not
> "verified"**, so it was reverted rather than committed as code that could not
> be run.

That nested-workspace duplicate now blocks the whole accelerator program-test,
and is its own named wall.

### The `AGENTS.md` amendment, and why it is usable

The M-38 bullet now carries both causes: the hostile that never reaches its
subject, **fixed in the test**; and the hostile that *does* reach its subject and
has no word for what it found, **fixed by splitting the discriminant** — until
which point the bare `is_err()` is *the most precise assertion its author could
have written*. It closes with the operative instruction:

> **Before reaching for the test, ask whether the code it needs exists.**

Most rules in that file say what not to do. That one says what to check first.

## 2026-09-01 — declared, convicted, built, observed

**The route register, corrected against observation** — a fresh six-program SBF
set (zero frame diagnostics), `fractional_compaction` run with the evidence dir
set, **47 transactions**, folded, and `census observe` admitting **94
observations with zero problems**:

| | before | after |
|---|---|---|
| witnessed | 69 | **73** |
| blocked, stated + owned | 35 | **33** |
| **never executed, no stated reason** | **57** | **55** |

Two `never → witnessed` (exactly the two predicted) and two `blocked →
witnessed`, with their `blocked.json` entries **deleted the moment their routes
executed** — *keep an entry only while it is true* — and the remaining entry
narrowed to the three targets that still emit nothing rather than covering the
workspace.

### The instrument refused its own author, twice

**First pass:** labels auto-generated from the enclosing *helper*, and folding
showed **one label spanning both executed and refused transactions** — which no
binding can describe.

**Second pass:** `census observe` **refused the bindings outright.** Six hostile
crank substitutions shared one label, and the refusal code had been read off the
first transaction and written for all six. **Five of them raise different
codes** — `Rent`, `TerminalIdentity`, `SignedDeltaRelease`, `SelectionConfig`,
`Phase`.

> Had I written those bindings from expectation, all six would have read
> *witnessed* with a code five of them never raise — **the exact false green
> this lane exists to remove, published by the lane that exists to remove it.**

**The census refused to record coverage it could not corroborate, and it was
right.** An instrument that will not accept its own author's word is the only
kind worth having. The rule that falls out: verify every label carries **exactly
one outcome and one refusal code** before a binding is written.

### One code, four states, three lanes, one session

`0x5644` was **declared and unraisable** when this ledger opened; **convicted**
in §1.3 as a guard declared and never written; the **guard landed** by another
lane at `fractional_claim_check_v1.rs:1196-1209`; and this fold is the **first
census evidence of it firing on a real ELF** — via a hostile that rewrites the
Core Market to `Open` with its terminal receipt dropped, at its own derived
address, so the phase is the sole discriminator.

> **Declared, convicted, built, observed.**

## 2026-09-01 — a stale caveat outlives a stale total

**Correction to entries above.** This ledger recorded *"384 of 391 lamport
set-sites enumerated, not classified"* more than once. **Both numbers are dead.**
The population went **391** (raw grep, test code included) → **152** (production
only) → **120** (after 32 turned out not to be destinations), and closed at
**120 of 120 classified, class 4 zero.** 384 was 391 minus seven wire decodes —
arithmetic that only ever made sense against the pre-filter count.

The lane traced how it survived: written once *before* the census completed, then
**carried forward verbatim into three later posts without re-deriving it.** A
snapshot from upstream of two successive filters, quoted as though live.

### The lesson, and it is the sharper of the two

The legend line inflated 57 to 58 — a number that still reconciled against
nothing. This line kept a **superseded population alive by being copied rather
than recomputed.** Both are off-by-a-filter errors, and both survived because
the sentence around them read as settled.

> **A lane that corrects its own totals but copy-pastes its own caveats has only
> half the habit.**
>
> **A stale caveat is more durable than a stale total, because nobody audits the
> thing that says work remains.**

A carried-forward *not-done* line needs re-deriving every time it is repeated,
exactly like a total does. Everyone checks the number that claims progress;
nobody checks the number that claims debt — so the debt figure is where a dead
count lives longest.

**What actually remains from that row**, re-derived rather than copied:

1. **The heap delta is unattributed** — 15,682 CU between the shipping and
   `hot-cu-profile` builds, split unknown between checkpoint logging and
   `hot_cu_profile_lifts_every_route_v1()` lifting every route onto the extended
   heap. Separating them needs a build that keeps the checkpoints and drops the
   lift.
2. **The recorded 323,523 CU no longer reproduces at HEAD** — the ordinary run
   reads **331,274**, +7,751.

The lamport half **is** swept.

## 2026-09-01 — a build hazard the handoff letter itself carries

**My diagnosis was wrong and the lane corrected it with evidence.** I called the
accelerator program-test's *"multiple different versions of crate
`dclutch_core_contract`"* a manifest path-normalisation problem. It is not:
`cargo metadata` resolves **exactly one** copy at a canonical absolute path,
uniform `../../../crates/…` spellings, no `..` oddities, no symlink prefixes. The
graph is clean, and every other duplicate in it is a genuine semver-incompatible
registry crate.

**The cause was an overridden `CARGO_TARGET_DIR`.** That program-test is **its
own workspace**; pointing its target directory at the root workspace's mixes
rustc invocations made from two workspace roots, so one path-dependency crate
compiles twice — once relative, once absolute — and the link fails **blaming
crates nobody touched.** Dropping the override built it in **nineteen seconds.**

> **`cargo metadata` is the discriminator: one copy in the graph means the
> manifest is innocent and the target directory is the culprit.**

`cargo update --workspace` never clears it because there is nothing in the graph
to clear — right prediction, wrong reason.

**And the trap is in the handoff letter itself**: its `general-hot` command
carries that override, so the next lane inherits it from the document meant to
orient them. Recorded in `AGENTS.md` and appended to the letter's corrections.

### Re-running your own conclusions when the build turns out to have been mixing

Every OpenBatch measurement that lane reported today was taken under the
override — and it was the lane that raised the doubt. It re-ran them: with
`general-hot` building in its own target directory, OpenBatch still refuses
**`0xC00A InstructionsSysvarAccount`** at N=2, accelerator CPI reached, same
frame counts. **The finding stands independent of the hazard.**

> **A session that discovers its own build was mixing artifacts has to re-run
> its conclusions, not assume them.**

The refactor also landed, verified this time: `18` written out four times **is**
`ADMITTED_RUNTIME_ACCOUNTS_START_V3` — the same constant the accelerator derives
its scratch-page window from — a second author for a frame offset **in the one
file whose pages sit at the far end of exactly that offset**, where a drift
surfaces as a bank-content refusal rather than as a missing account. 24/1 before
and after.

## 2026-09-01 — no single step is evidence; the sequence is

The `0x5644` arc, stated as the ledger now records it:

> A census found an **absence**; a second method **convicted** it as a defect
> rather than an artefact; an owner **built** the guard; and a third instrument
> **watched it fire**.
>
> **No single step of that is evidence. The sequence is.**

Which is also the honest reading of the double catch that produced it: the first
instrument said *absent*; only the second said *why*; and only `census observe`,
**refusing its own author's bindings**, said whether what was then written down
was true.

> **Three different things had to disagree with me in turn before the record was
> right.**

### The 55, owned per row

`docs/evidence/UNWITNESSED_ROUTES_BY_ROW_2026_09_01.md` names every route with
its declaring `file:line`:

| row | unwitnessed |
|---|---|
| C-09 Objective resolution | **14** |
| C-10 Claims, Custody, terminal lifecycle | **13** |
| C-06 Dealer | **8** |
| C-02 Product entrance | **5** |
| C-08 Structured/Fractional | **5** |
| C-01 Infrastructure/Registry/Rent | **4** |
| C-04 Direct | **4** |
| C-07 Series | **2** |

C-09 and C-10 carry **half the residue between them**. Dealer's eight are **one
family** — every `dealer_scenario_checkpoint_v1` stage, create through cleanup.

**A route sits in the row whose *capability* it serves, not the program that
hosts it**, because the lane that would drive it is the capability's lane. The
mapping is hand-authored and says so: *if a row lists a route its lane does not
own, say so and it moves.*

**The pointer lives in the contract's own matrix preamble**, with the counts
inline — so a lane reading its row finds it **without having to know the
document exists.** That is what makes it a work queue rather than another
artifact.

Two things the list states explicitly, because a bare list of 55 invites both
errors: **an unwitnessed route is a statement about coverage, not correctness**;
and **a route that proves structurally undrivable belongs in `blocked.json` with
a reason and an owner**, not left in the queue looking like unstarted work — so
the number stays honest as it moves instead of decaying into a backlog nobody
trusts.

**Register: 73 witnessed / 33 blocked-with-a-reason / 55 unwitnessed, of 161,
zero dangling binding refs. C-16: six categories, six instruments, none
finished.**

## 2026-09-01 — the satisfying set is empty

The `coefficient == denominator` constraint is now **convicted by proof rather
than by absence.** Not *"no kernel states this law"* — **the law contradicts the
kernel**:

- the transition requires `coefficient[i] == D` for every `i`
  (`open_structured_v3.rs:927`);
- the composition kernel requires **`gcd(D, coefficients…) == 1`**
  (`translation.rs:231`), because a canonical root payoff must be in lowest
  terms and **the coefficients *are* the numerators** — the call site is
  `composition(basis, &basis.coefficients, basis.denominator)`, and that
  function's own doc says *"the same recipe in the same lowest form"*;
- together these force **`D == 1`**;
- but `D <= 1` refuses `NonFractionalDenominator` and `D == 0` refuses
  `ZeroDenominator`, so **`D >= 2`**.

> **The satisfying set is empty.** A guard that can never admit anything is an
> **unconditional refusal of the entire Structured family wearing the shape of a
> check** — and it is why nothing in this family has ever crossed the Trading
> Hot route.

**Demonstrated from the other side**, not merely derived: setting
`COEFFICIENTS = [D, D, D]` — a basis that *satisfies* the constraint — against
the **unmodified** operator collapses the suite to 1/45 on `NonCanonicalPayoff`.

### Vacuous-permit and vacuous-deny are not the same defect

Still not removed, and the reason is a distinction worth keeping:

> **Deleting a never-*refusing* tautology is free.** Deleting a
> **never-*admitting*** guard is **not symmetric: it lets through what was
> blocked**, and the family cannot be shown to work afterwards while another
> wall stands behind it.

*The conviction is complete; the authorization waits on the wall behind it* —
which is a different sentence from "unresolved". `Content` (`0x4003`) is that
wall, revealed rather than caused: with the constraint's shape kept and only its
second operand made unfailable, the transition passed and the admitted issue
**still refused** at 367,084 CU. A fourth, independent wall.

### A campaign that never asks cannot notice something else was answering

The forced-budget migration landed. **+40 bytes** (the ComputeBudget program id
entering static keys, plus its compiled instruction) and **+150 CU** (its builtin
cost) are **what a real transaction has always paid and this campaign was not
counting.** *A packet figure that omits what a real transaction carries is a
packet figure for a transaction nobody sends.* Executable full-width K unchanged
at 2 — checked, not assumed.

The `claim_check` submodule then failed **8 of 9** without the limit: the same
defect from the other side.

### A test that was not weak but impossible

The campaign's last log-contains check asserted `contains("Custom(16387)")`.
Beyond also accepting `Custom(163870)`, **it could not pass however the route
behaved**: the runtime writes `custom program error: 0x4003` in the failure line
and renders `Custom(N)` only in the transaction error, **which never reaches
`logs`.** Measured under probe, the hostile refuses exactly `Content` — **the
predicate was right all along and the format was unreachable.**

## 2026-09-01 — a question with no correct answer

The three LP `Geometry` failures were **neither** of the two candidates offered:
not the action-scoping change, not the LP profile. **A category error in the
validator**, convicted by instrumenting every stage and printing the comparison:

```
PERM FAIL: item=false index=7 granted=0b0001 required=0b0010
```

Fixed slot 7, granted `DEBIT_LAMPORTS`, required `CREDIT_LAMPORTS`. **The LP
frame puts the Open payer and the Close RentCredit at the same slot 7.** A payer
must be debitable and a RentCredit creditable, so **no single permission set
satisfies both — and none has to**, because the encoder takes the *action* and
builds a different frame for each. The lifecycle policy does not: it is **one
policy carrying an Open plan and a Close plan.**

So `validate_account_profile` was asking **whether the Close plan fits the Open
frame.**

> **That question has no correct answer, and its refusal was never a finding
> about the artifacts.**

**And the decoder already knew.** Four lines on,
`validate_protected_output_uniqueness` **skips pairs whose plans carry different
actions**, for exactly this reason. The plan loop simply never carried the same
notion. Repaired by adding an action-aware form beside the whole-policy one,
which stays correct for families whose single profile covers every action.

**Exonerated by measurement:** the failure reproduces at `c8396b0b^` *and*
`a153f08e^` — it predates both halves of the `derivation_policy` repair.

### A check that cannot fail, arrived at while repairing one

The new action-aware API was **vacuous on its first draft**: perturbing the call
to name the wrong action left it **green**, because the filter skipped every plan
and the join checked nothing. Now it refuses an action the policy carries no plan
for, re-proven wrong-action-FAILS / right-action-passes.

### The same shape, found latent and closed before it bit

Since `6fed9720` the **registered Direct** policy carries both sides' plans while
the account profiles differ per side — the crate's own test asserts it. Same
shape, same exposure. **Direct's coordinates do not collide today, so it passed**
— and both registered bundle joins were switched to the action-aware form rather
than left latent.

> **LP is where the shape had teeth; Direct is where it was waiting.**

trading-sbf lib **446/6 → 448/4**, none of the four remaining being the
validator and none newly broken. The LP frame reusing slot 7 for two roles is
**legal but load-bearing** — it is what made a shared policy unvalidatable.

## 2026-09-01 — extracted, and it compiles for the target that matters

`crates/dclutch-wallet-terminal-payout-operator` (`3853fb6e`): **2,068 lines
lifted verbatim** out of the binary's `wallet_terminal.rs`, which goes from
**2,130 lines to 144** — argument parsing, two file reads, RPC, stdout. Every
moved item is **re-exported at its old path**, so the eight modules reaching it
still resolve. **Additive, not a merge**: the binary keeps its own
`[workspace]`, the boundary the lane had declined to dissolve.

**The finding it rests on, enumerated rather than assumed:** every `plan.*`
access in the producer is one of **six coordinates** — five program ids and a
release-set id. The browser holds five from its deployment and reads the sixth
from the Market's own Core state. `--evidence` contributes a routing table and a
`plan_sha256` binding the CLI's two files **to each other**. *A deployment table
and an address book.*

**One semantic change, and it is the boundary itself:** `from_rpc` →
`from_observed`, taking a four-field observed value instead of an `RpcAccount` —
**removing the crate's last tie to a socket** — with a four-field adapter kept in
the binary so its call sites read unchanged. That is the difference between
moving code and extracting a library.

**And it compiles for the target that matters, on the real code:**
`cargo build --target wasm32-unknown-unknown --release` on the extracted
derivation, **not a stub**. It declares **no SBF program crate** — proved by
construction rather than argued — with the 64-bit layout assertions untouched.

Two details nobody asked for: **`solana-sdk` is gone**, because it was there for
one type on one production line and that type has its own crate; and **one
shared fixture under a `test-fixtures` feature rather than two copies**, because
*two copies of a fixture drift, which is what an extraction exists to prevent.*

**Stopped deliberately at a clean point.** The WASM boundary and the browser's
snapshot acquisition both remain, both unblocked, both with the technique proven
twice — and *"landing a half-wired boundary at the end of a session is the
failure mode this project fears."* A boundary that compiles and is not yet
called is a clean state; one half-wired to a snapshot is not.

> **C-12: one capability, not three — and the third one's wall is now a code
> path rather than an unknown.**

## 2026-09-01 — RETRACTION: the width-258 wall does not exist

**Entries above are wrong and must be replaced.** This ledger recorded a
`0xC011 ScratchBankDigest` wall at width 258 — twice — and recorded *"width 1
commits for the first time."* Both came from **interleaved test output.**

Isolated with a name filter and `--test-threads=1` on a restored ELF:
`real_sbf_freeze_accepts_runtime_widths_one_and_258` commits at **width 1**, the
accelerator CPI succeeds at 20,264 CU, and the returned ack carries disposition
**`Refused`** rather than `Accepted`. The test fails **there**, at the
disposition assertion — so **width 258 has never run.** The `0xC011` was emitted
by `corrupted_scratch_page_refuses_without_mutating_selection`, which corrupts a
page **on purpose**, running concurrently in the same binary with its
`Program log:` lines interleaved into the same stream.

**And "width 1 commits" was true only of the *transaction*. The action was never
accepted.**

### The on-chain probe is what caught it

Logging `cursor`, output length and computed-against-declared digest inside
`assemble_input_bank` printed **exactly once** per run — `pages=4, cursor=2696,
outlen=2696`, digests equal — and never for a second width.

> **One emission where two outcomes had been claimed is what a width-dependent
> story cannot survive.**

And the reason host reasoning could never have found it: *every host link was
genuinely clean, which is exactly why the contradiction kept pointing somewhere
that could not be seen.*

### The second instance of this exact failure tonight

**The first was mine**, early in the session: a compute figure published from an
interleaved parallel test log, belonging to a *passing* test, withdrawn publicly.
This is the same defect one layer up — **a refusal read out of a shared stream
and attributed to the panic sitting next to it.**

`AGENTS.md` now carries the rule: `cargo test` runs a binary's tests
concurrently, **every one prints into the same stream**, and a refusal read from
that stream belongs to whichever test emitted it — not to the one whose panic is
adjacent. **Re-run with a name filter *and* `--test-threads=1` before believing
a width, an ordering, or a count read from interleaved output.**

### What survives, and it is better supported than before

The transport was never the question and is clean: geometry tiles exactly at 1,
2, 257 and 258; the page codec round-trips byte-for-byte under **nonzero,
all-zero and zero-tail** banks (the last two added precisely because a non-zero
filler could have masked a padding rule); the eighteen page keys are distinct and
collide with no fixture constant; frame coordinates derive from the same
constants the accelerator uses; the caller forwards the frame verbatim; and the
accelerator's `content()` is the same `hash()` the fixture uses.

The refusal-splitting in `4c90cdf5` stands on its own merits and its hostile
assertion is still exactly right — **it simply did not make a 258 wall legible,
because there was none.**

**The real wall is one step earlier and semantic:** at width 1 the transaction
commits and the accelerator **refuses the action**, returning a `Refused` ack
rather than a program error. That is a General transition refusal, not a
transport fault, and a fresh investigation rather than a continuation.

## 2026-09-01 — both halves agree with each other, and neither agrees with the consumer

`Content` on the admitted common-Hot issue is localized **four layers down**, and
the last layer needed no build.

1. **Phase**, from the tree's own 33 `hot-cu-profile` checkpoints rather than a
   new instrument: `p7-effect-projection → p7-local-effect-discipline →
   heap:downgraded-effects → 0x4003`.
2. **Callee** — that interval has zero inline `Content` sites, so it is inside a
   call. Markers between three candidates resolved it in **one** build: enters
   `decode_claims_composition_boxed_v3`, never returns.
3. **The discarded reason, surfaced** — the only remaining site is
   `decode_selected_with_external(…).map_err(|_| TradingSbfError::Content)?`.
   Naming the inner variant gave **`ClaimsCompositionErrorV3::Route`**: *"an
   active Claims route used unsupported geometry or packet bytes."*
4. **Which conjunct** — and this needed **no instrument at all**.

`validate_rational_representation_route` splits on `selected_outcome()`.
`IssueStructured` is not selected-outcome, so it takes the `else` arm, which
requires:

| composition requires | operator declares |
|---|---|
| `kind == RouteKindV3::AffineOnce` | **`RouteKindV3::Once`** |
| `item_account_count == RATIONAL_ASSET_ACCOUNT_COUNT_V2` | **`0`** |
| `repeated_item_count == header.asset_count` (K=3) | **absent (0)** |
| — | `fixed_account_count: CLAIMS_FIXED + K * ITEM` |

**The operator flattens the K coordinate rows into one fixed span; the Claims
composition requires them as an affine repeated item span.** Two authors of one
geometry, structurally disagreeing.

> **Both halves of the operator agree with each other, and neither agrees with
> the composition.**

Its own validator also checks for `Once`, so the operator is **internally
consistent with its own wrong answer** — guards-whose-two-sides-move-together at
the scale of two subsystems rather than two registers. The only instrument that
could catch it is one that crosses the boundary.

### `map_err(|_| …)` has destroyed the reason at three walls tonight

The transition fold, this composition decode, and the heap admission's
request-versus-grant. Three lanes, three subsystems, one idiom — and **the
recovery was identical every time**: surface the inner error, then compare
declared against observed.

> **A `map_err` that discards its cause converts a located defect into a
> search.** This session spent hours on each of the three.

And the localization's own lesson:

> **The cheapest instrument is the one you stop needing.**

**The Structured chain, four walls, all now known and independent:** transition
(convicted, satisfying set provably empty); heap (closed); **Content/Route — the
live frontier, and upstream of the coefficient question**; and whatever stands
behind it, unknown because nothing has ever executed past it.

## 2026-09-01 — the same category error, at the runtime site the fix did not reach

The Dealer suite went **26/3 → 27/2** after the validator repair, and the two
survivors moved to a different wall: `Projection("profile-join")`.

**`validate_account_profile_join` delegates straight to the whole-policy form**
(`lifecycle_v3.rs:1345`), and its own doc says so — *"It proves exactly what
`validate_account_profile` proves."* So the action-aware form was added and
**no `validate_account_profile_join_for_action` exists**, leaving two callers
still asking the question with no correct answer:

1. `program-test/bundle-builder/src/registers.rs:296` — host-side.
2. **`src/hot_v3/seal.rs:629` — the runtime**, on the **generic Hot path every
   family crosses**, raising `Content`.

So the LP family cannot pass the join host-side **or** on chain, and any family
whose single lifecycle policy carries plans for actions with differing frames
meets it at runtime.

### A scope limit, stated so nobody over-reads it

This does **not** explain the unlocalized equity-Add wall at 591,781 CU.
Equity's V5 lifecycle is the canonical **empty** policy —
`DEALER_EQUITY_LIFECYCLE_BYTES_V5 == LIFECYCLE_HEADER_BYTES_V5`, and the
no-create-or-close-authority test passes — so it carries no plans and **the join
is vacuous for it.** That wall remains unlocalized and separately owned.

### The collision guarded, from the decoded artifacts

`7bab57df` pins the slot-7 collision by reading the **decoded** artifacts rather
than the builder's inputs: slot 7 grants `0b0001` (DEBIT) under Open and `0b0010`
(CREDIT) under Close, independently reproducing the `granted=0b0001
required=0b0010` the instrumentation found. The assertion is **the disjointness
itself** — `open & close == 0` — *because that is the property making a shared
answer impossible.* A third action added to that frame surfaces there.

### Provenance, when the name lies

The hbox build tree is named `s5-accepted-7641794a` and now holds `3853fb6e` —
the stale-checkout hazard in its purest form. Resolved by writing
`S5-TREE-HEAD.txt` into the directory rather than renaming it, **because
renaming would invalidate cargo's absolute-path fingerprints and cost a cold
rebuild.** Correct the record, not the name.

## 2026-09-01 — look for a case where the accused shape is admitted

**Correction to the entry above, made by the lane before it would accept the
verdict.** *"The operator flattened, therefore the operator is wrong"* would have
been **a wrong inference from a true observation.** The composition is **not**
`AffineOnce`-only: its sibling arm `validate_rational_lifecycle_route` requires
exactly the operator's shape — `Once`, `item_account_count: 0`, and a
`coordinate_count`-scaled **flattened** fixed span. A flattened K span is not
inherently rejected anywhere.

> **The discipline: look for a case where the *accused* shape is admitted,
> before concluding it never is.** The lifecycle arm nearly cost a wrong verdict,
> and finding it is what saved it.

The composition assigns a kind per family, and per **action** only for this one —
`REPRESENTATION` selected-outcome takes `Once` + flat; **full-width takes
`AffineOnce` + item span + `repeated_item_count == header.asset_count`.**

**Verdict: the operator is wrong, on four grounds — and the strongest is not the
obvious one.**

1. **Request-bound versus artifact-bound.** The composition ties account
   geometry to the **request's own** `asset_count` in *both* branches. The
   operator's flattened `fixed_account_count` is a **release-time constant**
   baked from `representation_outcome_count`; it **cannot track the request at
   runtime.** *It coincides for a K=3 release and binds nothing.*
2. **The kind's own definition is the frame** — `AffineOnce` *is* "fixed prefix
   plus all authenticated item tails".
3. **Precedent that executes**: `AFFINE_BATCH` is the tree's proven affine Claims
   child, with its own real-ELF program-test, and it uses `AffineOnce`.
4. Consumer beats declarer — **the weakest of the four on its own**, and the one
   I had offered.

**The repair is specified and deliberately unlanded**: it cannot be shown green
while wall #1 stands, so it would land unverified — *and a restructure of a
release artifact's route declaration is exactly the change that should arrive
with evidence, not ahead of it.* The diagnosis is complete enough that landing it
is **a short lane rather than a search.** `composition_v3.rs` untouched: the
finding is that it is right.

### The rule, landed beside the refusal-code law (`bafc289d`)

> That law says a refusal must **name** what it refused. This one says a wrapper
> **may not un-name it on the way out.**

Three citations, the cheapest-first localization order that actually worked, and
permission for a coarse code when the causes are genuinely one accusation. Plus
the bullet the lane nearly got wrong itself: **a probe measures what it touches,
not what you meant** — a heap probe that *allocates* measures `entrypoint!`'s
hardcoded `HEAP_LENGTH` and reported the opposite answer until rewritten.

### And the class, stated at its own scale

*Both halves of the operator agree with each other, and neither agrees with the
composition* is guards-whose-two-sides-move-together **at subsystem scale** —
and:

> **The only instrument that can catch it is one that crosses the boundary.
> Every check confined to one side passes.**

## 2026-09-01 — a ratchet that had been slightly weaker than it claimed

The redemption WASM boundary landed (`a2686852`): generator, a committed 712 KB
artifact, browser loader, **not wired to a snapshot** — the larger half is its own
run. Three `const _: () = assert!(...)` read the settlement frame width, request
width and candidate domain **out of Claims** by constant name; the client pins
length and SHA-256 so unverified bytes never execute, then **asks the loaded
derivation its own frame width and refuses if it disagrees with Claims.**

One design choice that shrinks the next unit: **the boundary hands the caller the
derivation's own address list** rather than letting a client assemble one —
*which is what stops a second routing implementation existing when the
acquisition is built.* The mirror hazard prevented at the point it would
otherwise be introduced.

### A dead field asked what it was for

The compiler flagged the snapshot's per-account `key` as unread. Deleting it was
the easy answer; instead it is **cross-checked against the address slot it
arrived in** — because *an observation paired with the wrong slot is the single
corruption a snapshot can suffer that still decodes cleanly and still
authenticates, **against the wrong account.*** A defect nothing else in the chain
could catch, found by asking what an unused field was **for** rather than whether
it was needed.

### And the ratchet was wrong since it was written

The ABI pairing ratchet sorted generators and verifiers **independently** and
compared them after stripping `:verify`. With `-` at `0x2D` and `:` at `0x3A`,
the moment one generator's name is a **prefix** of another's, the two lists come
out in different orders and the check **fails a real bijection.**
`abi:wallet-terminal` beside `abi:wallet-terminal-payout` is the first pair with
that shape.

> **Renaming mine would have dodged it and left the trap for whoever next names
> an `abi:foo-bar` alongside an `abi:foo`.**

The sort moved **after** the strip, where order is meaningful.

> **The ratchet has been asserting something slightly weaker than it claimed for
> as long as it has existed; it just never met the shape.**

Eighth instrument this session found wrong by its own author — and the first
found by a **name collision** rather than a measurement.

The capability surface's only delta was `OPERATOR_CRATES_V1` gaining the two new
crates — exactly what that census exists for, *so an act cannot name an owner
that is gone*, and confirmation that adding a compiled derivation changed no
act's standing.

## 2026-09-01 — vacuous for every action equally, not wrong for each

Both callers of the profile join are now action-aware (`f87dae3c`) — the runtime
at `hot_v3/seal.rs` and its host-side twin in the bundle-builder.

**And the first repair would have broken equity the moment the runtime site
switched.** The non-vacuity guard refused **any** action the policy carried no
plan for — correct for a policy that *describes some action and was asked about
another*, and **wrong for the canonically empty policy.** Equity's V5 is exactly
`LIFECYCLE_HEADER_BYTES_V5`:

> There is nothing for any action to answer for, so the join is **vacuous for
> every action equally**, rather than **wrong for each**.

Shipped blind, that would have turned a vacuous pass into a `Content` refusal —
at the runtime site, on the generic Hot path — for the one family whose policy is
legitimately empty, and it would have read as a new Dealer defect.

### Two perturbations that proved nothing, reported rather than hidden

`input.action ^ 1` still named a **real** General action. And the
bundle-builder's own suite stayed green under `0xFFFF_FFFF` because **it never
reaches `run_engine` at all** — so that host-side site is **under-covered**, and
the lane said so instead of claiming a proof it did not get.

> **A perturbation that does not go red has told you about your test, not your
> fix.**

Third way this session the tree has learned the same thing: a hostile that
cannot reach its subject; a hostile that reaches it and has **no word** for what
it found; and now a perturbation that **cannot move** the thing it perturbs.

### Naming the property beats naming the failure

The Dealer lane's `open & close == 0`, read off the **decoded** artifacts, was
credited by the lane that had instead printed the mismatch from inside
`require_permissions` — *"the assertion I should have written and did not."*

> Printing the mismatch names **what broke**. Asserting the disjointness names
> **what cannot be added.**

trading-sbf lib **446/6 → 449/4**; the same four remain, none the validator,
none newly broken.

## 2026-09-01 — one of the two stages left the browser's path

The redemption acquisition landed (`eed52c57`). `RedeemFlow` no longer says
*"This browser never creates or completes a payout plan."*

**Every address comes from the derivation's own list** — the boundary is asked
which accounts it authenticates and exactly those are read, in exactly that
order. **Not one is computed in TypeScript.** The mirror hazard prevented at the
point it would otherwise be introduced rather than corrected afterwards. One
finalized floor, taken once. A vacant account is carried as vacant rather than
refused, because **the derivation decides which of the 36 may be empty, with its
own reason** — and each observation carries the address it is *of*, so the
boundary's cross-check has something to check.

### What it refuses to claim

**The reader still imports JSON.** Stage one — `wallet-terminal-payout-input` —
reads two operator artifacts and its own RPC, was **not** extracted, and stays a
CLI command. **A test pins that the page still says so.**

> The browser now performs the **authenticated derivation** itself — the
> 36-account frame, the lookup-table geometry and the report come from compiled
> Rust reading finalized chain state *here*, instead of arriving as a manifest
> computed elsewhere. **One of the two stages left the browser's path. Saying
> both did would be a claim this lane did not earn.**

An already-complete manifest is still accepted — **not a parallel authority, but
two artifacts at different stages of one**: whichever arrives, what reaches the
checks is the same derivation proved against finalized devnet by the same code.
The stage-one format name is emitted **from the operator crate**, so the browser
recognises the artifact without writing its name down.

### The derivation recorded the change; nobody typed a status

The regenerated capability surface moved **223 → 226 modules** and **21 → 22
generated authorities**, and `/portfolio` and `/redeem` gained the payout facts
in their reach.

And a test flipped from asserting a wall to asserting **its absence** — the
second time this session. The gate also caught the lane's own ratchet fix
diverging a twin; synced, `twinIdentity` 157/157.

**C-12: trade and redemption are both stranger-operable in the browser now**, to
the limit each earned — admission end to end, redemption from the payout input
onward. Creation's remaining debt is a transport question plus one blocked
constant.

**The named next line**, for whoever takes it: **extract stage one the same way
stage two went.** Its impurity is two file reads, an RPC and a cluster policy;
its inputs are the six coordinates the browser already holds. *The last CLI
command standing between a stranger and a redemption — and its shape is now
known rather than guessed.*

## 2026-09-01 — the registered Sell executes, and wall A is a missing implementation

With wall A relaxed **as a probe only** (reverted; it stands at
`hot_v3.rs:5413`), the acceptance case un-ignored:

```
REGSELL compute units consumed: 365,011   — Program ... success
```

**The registered Sell executes on real ELFs** — six System-program CPIs creating
the maker replay and registered record, passing **every** Sell assertion
(exact root, maker-replay and record poststates, Claims conservation) before the
Buy is even submitted. That has never happened.

### The Buy's new wall, localized in four measured steps

`0x4003` at **308,354 CU with no child CPI at all** — it dies inside Trading.
Checkpoints → `p5-geometry-rent`; probes → inside `project_accounts_atomic`;
kernel error logged → **17 = `IdentityMismatch`**, raised from exactly one helper
with two call sites; coordinate distinguished → **account 34 = `MINT_ACCOUNT`**.

`require_key(MINT_ACCOUNT, REGISTERED_IDENTITY_MINT_V4)`, whose identity is
projected from the Realm's `COLLATERAL_MINT`. **The mint in the Buy's frame is
not the collateral mint the Realm account records.** Buy-only, consistent with
the Sell passing — the Sell has no collateral block. **A pre-existing latent
defect, newly reachable because wall B was crossed** — the same pattern as the
four activation reds.

### Wall A is not a gate to remove

Measured rather than assumed: `DirectInlineHotCrosscheckV3` is an **independent
re-derivation of every account's expected poststate**, checked after the children
run — *the "two implementations agree" check for Direct*, the planner's opinion
against the effect kernel's.

> **That is why `src/direct/{sell,buy}_escrow.rs` have no caller: 1,604 lines of
> registration, fill, close and terminal planners are exactly the input a
> registered crosscheck would consume.**

So crossing wall A is **not relaxing a refusal** — the refusal is correct,
because *a crosscheck that cannot check an action must refuse it.* It is
**writing the registered analogue** of ~500 lines of inline machinery across nine
functions. The 1,604 orphaned lines were never dead; they are the input to a
check nobody wrote.

Not started, because the probe **changed the order**: the Buy's mint mismatch
sits *before* the crosscheck on the same path, and building a crosscheck for an
action that cannot get through account projection would be building against a
known wall.

### A wall that was true when written

The file's header records a previous lane running this same probe and stopping
the Buy at wall B. **That claim was true when written and is now false**, and
only re-running it made that visible.

> The same shape as the stale caveat: **a recorded wall decays exactly like a
> recorded total, and nobody re-derives the sentence that says you cannot get
> further.**

## 2026-09-01 - the accelerator refused for eight months and never said why

The width-1 `Refused` ack was a **semantic** refusal with no reader. The whole
of `process_instruction` ended:

```rust
Err(_) => AcceleratorAckV2::refused(request, request_digest),
```

**Ninety-six refusal sites, three semantic variants, one indistinguishable
`Refused` ack, and not a single `msg!` in the program.** The wire cannot carry
the distinction and should not -- the refused ack is one canonical shape so
Trading can separate a transport fault from a failure-atomic semantic refusal --
which makes the validator log the only place the cause can live, and there was
none.

`GeneralAcceleratorSemanticErrorV3` carries no `#[repr]` and never becomes a
`ProgramError`. **It is not protocol-visible, so granularity there is free of
decision 0007's band ceremony** -- a fact worth writing down, because the same
reflex that correctly slows a wire-code split had been silently slowing a
diagnostic one. Six causes split out of `State`, all inside the three helpers
every one of the fifteen actions calls before anything else: `RuntimeAccount`,
`EvidenceCoordinate`, `ConfigDecode`, `ConfigIdentity`, `ConfigMarket`,
`ProductIdentity`. `log_line` is a `&'static str` per variant under an
exhaustive match rather than a `{:?}` format, because peak heap at runtime width
258 is this program's binding constraint and `sol_log` takes a `&str`.

**The instrument convicted in one run**: `ConfigIdentity` -- the config
account's digest is not the `general_config_id` the input bank declares.
`freeze.rs` built a bank carrying the outcome count, the settlement flag and the
Product digest and nothing else, so `general_config_id` was thirty-two zero
bytes. `lifecycle.rs` had always written them; this file never had.

> **The fixture had been failing domain authentication before any Freeze
> transition ran, for as long as the file has existed, and the program had no
> word with which to say so.**

At `51a4df9e` in a clean worktree, accelerator ELF sha256 `e9544323`, zero
frame diagnostics, name-filtered with `--test-threads=1`:

| width | transaction | ack | accelerator CPI | top level |
|---|---|---|---|---|
| 1 | commits | **Accepted** | 21,108 CU | 35,693 CU |
| 258 | commits | **Accepted** | 45,758 CU | 68,220 CU |

**Width 258 had never executed.** The accelerator program-test goes 24 passed /
1 failed to **25 / 0**, and the corrupted-page hostile still names
`ScratchBankDigest` exactly. Control, separately: perturbing the bank's declared
generation by one turns it red on `config rejects the bank's generation or
basis` -- a different line from the identity one, so two of the six new causes
are reachable and separable, and the added scalars are load-bearing.

Committed as `b876c340`. Its first draft said the pre-fix CPI cost 20,264 CU,
which is the **retraction entry's** figure and not one this lane measured; the
real reading was 17,187. Amended before the reader saw it, and recorded here
because a number carried forward from someone else's run is the cheapest way to
publish a measurement you did not take.

**And the ELF hash caught the second instance of the same thing.** The figures
above were first taken against a build made before `rustfmt` ran, hash
`105961a4`; the committed source builds `e9544323`, deterministically and from a
clean worktree at `51a4df9e`. The CU readings turned out identical, so nothing
downstream changes -- which is exactly why only the hash could notice. **A
measurement whose artifact no commit names is not a measurement, even when it
happens to be right.** The co-tenant Trading lane was mid-edit in a shared
dependency at the time, twice; a detached worktree at `HEAD` measures the
commit rather than the ambient tree and does not wait on anyone's timing.

## 2026-09-01 - two frame tables, and only one of them is produced

**The OpenBatch N=2 `0xC00A InstructionsSysvarAccount` wall is not a coordinate
typo and not a one-line fix.** It is two authors of one geometry, at the scale
of two programs.

The General accelerator is the **only** reader of
`crates/dclutch-execution-strategy-contract/src/admitted_v3.rs`: an 18-account
prefix with the caller authority at 0, the instructions sysvar at **4**, the
Trading program at 5 and the runtime accounts starting at **18**.

Trading has exactly **one** admitted-accelerator CPI site,
`programs/dclutch-trading-sbf/src/admitted_composition_v3.rs:410`, family-neutral
and reached by General like everyone else. `fixed_cpi_accounts` builds the frame
as caller authority, then `ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4` = 39 hot
fixed accounts, then 8 strategy-evidence accounts:

| coordinate | V3 table says | the real frame has |
|---|---|---|
| caller authority | 0 | 0 |
| instructions sysvar | **4** | **30** (`1 + HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3`) |
| Trading program | **5** | **26** (`1 + HOT_TRADING_PROGRAM_ACCOUNT_V3`) |
| runtime start | **18** | **48** |

So at index 4 the real frame puts `HOT_MANIFEST_STAGING_ACCOUNT_V3`, a vacant
CapabilityManifest staging cursor. **The refusal is telling the exact truth.**
The V3 table is also not merely offset -- it has a different membership, with
capability and strategy raw/staging as top-level slots and no accelerator-program
account at all.

**The Dealer accelerator does not have this problem, and why is the whole
lesson**: it authenticates through
`dclutch_trading_sbf::hot_v3::authenticate_accelerator_invocation_v4`, the same
function that owns the producer's layout. General re-derived its own.

> **Every General harness builds the V3 frame and the General accelerator reads
> the V3 frame, so they agree with each other, and neither agrees with the
> producer.** The third instance of that shape this session.

**Correction to an earlier report.** "The sysvar is present in the transaction,
readonly and unsigned, at instruction index 29" -- the 29 is
`HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3`, an **account coordinate**, off by the
one-account authority prefix from its real position at 30. Not an instruction
index.

**Routed, not edited**, because the repair is a cross-lane convergence and not
this lane's to slam in: derive the four coordinates the accelerator actually
reads from `HOT_*` plus the V4 composition, then follow it through
`freeze.rs`, `lifecycle.rs`, `crates/dclutch-operator/src/general_hot_v3.rs`
(whose campaign-frame test compares against executed evidence and says in its
own comment to re-run the campaign rather than edit the numbers), and
`tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs:142-146`
-- **which restates `18`, `0` and `4` as bare literals**, the same second-author
defect `ecc43002` removed from `freeze.rs` twelve hours ago, in the one file
that would go stale silently.

## 2026-09-01 — a refusal with no reader, and both widths execute

C-05's width-1 wall is closed. The `Refused` ack was **semantic and unreadable**:
`process_instruction` ended `Err(_) => AcceleratorAckV2::refused(...)` — **96 refusal
sites → 3 variants → one indistinguishable ack, and zero `msg!` in the program.**

`GeneralAcceleratorSemanticErrorV3` carries no `#[repr]` and never becomes a
`ProgramError`, so it is not protocol-visible and **granularity there needs no band
allocation** — six causes split out of `State` with a `&'static str` `log_line` per
variant under an exhaustive match (peak heap at N=258 is the binding constraint;
`sol_log` takes a `&str`).

**Convicted in one run: `ConfigIdentity`.** `freeze.rs` built a bank with only the
outcome count, settlement flag and Product digest, so `general_config_id` was 32
zero bytes — **failing domain authentication before any Freeze transition ran, for
the file's whole history.** `lifecycle.rs` had always written them.

| width | transaction | ack | accelerator CPI | top level |
|---|---|---|---|---|
| 1 | commits | **Accepted** | 21,108 CU | 35,693 CU |
| 258 | commits | **Accepted** | 45,758 CU | 68,220 CU |

**Width 258 had never executed.** Accelerator program-test 24/1 → **25/0**, at
`51a4df9e` in a clean detached worktree, ELF `e9544323`, zero frame diagnostics,
name-filtered, `--test-threads=1`. Mutation control: perturbing the declared
generation by one goes red on a *different* line (`config rejects the bank's
generation or basis`), so two new causes are reachable and separable.

### The hash caught the author, twice

The first figures came from a **pre-`rustfmt` build** (`105961a4`); the committed
source builds `e9544323`, deterministically. **CU readings were identical, so only
the hash could notice.** And a commit-message CU figure had been carried forward from
the retraction entry rather than measured — 20,264 → **17,187**. *A stale caveat
outlives a stale total*, again: the number that was copied was the one nobody
re-derived.

### OpenBatch `0xC00A`: the refusal was telling the exact truth

Two frame tables, one producer. `crates/dclutch-execution-strategy-contract/src/
admitted_v3.rs` (18-account prefix) has **exactly one reader** — the General
accelerator — and Trading has **exactly one** admitted-accelerator CPI site
(`admitted_composition_v3.rs:410`, family-neutral: authority + 39 hot-fixed + 8
strategy-evidence).

| coordinate | V3 table | real frame |
|---|---|---|
| instructions sysvar | 4 | **30** (`1 + HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3`) |
| Trading program | 5 | **26** |
| runtime start | 18 | **48** |

At index 4 the real frame carries `HOT_MANIFEST_STAGING_ACCOUNT_V3`, a vacant
manifest staging cursor. **The Dealer accelerator never hits this because it
authenticates through the producer's own authority
(`hot_v3::authenticate_accelerator_invocation_v4`).** Membership differs, not only
offsets. And `tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs:
142-146` **restates 18/0/4 as bare literals** — the second-author defect `ecc43002`
removed from `freeze.rs`, alive in another file.

**Correction to this ledger**: "the sysvar is at instruction index 29" — that 29 is
`HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3`, an *account coordinate*, off by the authority
prefix from its real position at 30. Not an instruction index.

## 2026-09-01 — the registered Buy reaches the end of its own execution

Four defects, all convicted by MEASUREMENT rather than reading, each one hidden
behind the last. Behind the wall-A probe (`hot_v3.rs:5413` relaxed to `Ok(None)`
in a throwaway worktree, never committed) the registered Buy moved:

```
308,354 CU  Content 0x4003       account projection, no child CPI      <- was here
571,047 CU  Release 0x4001       Custody preflight, no child CPI
776,043 CU  AccountFrame 0x6001  inside the FIRST Custody CPI
1,205,519 CU Commit 0x4005       after all THREE Custody children succeed
```

`InitializeReplay` 123,796 CU, `OpenVault` 141,105 CU, delegated deposit
136,253 CU — a registered Buy now creates its maker replay and record, opens a
Custody replay and a TradingPrincipal vault, and moves the maker's collateral
into it, on real Core/Claims/Custody/Registry/Rent ELFs. None of that had ever
happened.

### The four, and what each one teaches

1. **Two `require_key`s that no transaction could satisfy.** `OP_REQUIRE_KEY`
   reads the INPUT identity bank; `project_identity` writes the OUTPUT bank. The
   Buy required its frame mint and token program against registers it projected
   *in the same pass*, so both compared a real key against thirty-two zeros.
   `ordinary_account_artifacts_v3` had the whole argument written out at length,
   twenty lines, and the registered family did the opposite anyway.
   **A comment in the sibling file is not a check.**

2. **A content digest written as an address.** `project_key(REALM_ACCOUNT, ..)`
   put the Realm record's ADDRESS in `CustodyRequestLayoutV1::REALM`, where
   Custody re-derives the record address FROM that field and requires
   `hash(realm_account.data)` to equal it. The Core Market's `identity.realm_id`
   is where the digest lives, and it is the value Custody cross-checks anyway.
   The inline family projects it from the Custody replay — which a creation
   cannot, because it is the instruction that CREATES the replay.

3. **A rent refund that Custody explicitly forbids.** The Effect wrote the
   record beneficiary into `RENT_REFUND`; `initialize_replay` requires the
   frame's refund account to equal `request.rent_refund` AND to DIFFER from the
   payer, and this family's record beneficiary IS the payer. The profile's own
   `ROUTE_ALIASES` already said the coordinate holds the lifecycle RentCredit.
   **I got this one backwards first** — moved the fixture to match the Effect,
   committed it, and the next wall said the Effect was wrong. The mirror was
   right and the reason was a Custody conjunct neither side had read.

4. **WALL C, open: the commit's lamport plan has no word for "a child made
   this."** `output_lamports` is seeded from the OBSERVED prestate; the Custody
   replay is vacant then, so the plan says zero, and `commit_output_lamports_v3`
   writes that zero back over the rent the child deposited —
   `require_committed_rent_exemption_v3` then refuses on coordinate 20 at 0
   lamports against 288 bytes needing 2,895,360. The commit skips only
   coordinates an `EffectProgramV5` FUNDING ACTION names, and funding actions
   describe accounts *Trading* creates through the rent lifecycle. Declaring a
   local `TransferLamports` is not the repair:
   `require_child_disjoint_from_local` refuses a child invocation that reaches a
   coordinate the Effect's own operations mutate, and coordinate 20 is in the
   child's frame. **The missing thing is an exemption, it is family-neutral
   machinery, and registered creation is the first route in the protocol that
   opens a Custody account — which is why no family needed it before.**

### The instrument that found three of them

Decoding the 672-byte `CustodyRequestV1` the runtime actually assembles
(`sol_log_data(&[request_bytes])`) and diffing it field by field against the
fixture's host mirror. Both defects 2 and 3 were ONE 32-byte field, and the
symptom was identical and useless: the request's hash is the sixth seed of the
Trading caller authority, and `require_custody_frame_shape_v3` requires the
frame's coordinate 0 to BE that PDA — so any wrong byte anywhere produces
`Release` with two PDAs and no clue which field disagreed. **A digest-seeded
identity turns every content defect into the same refusal. Diff the content.**

### Reproducing

```text
git worktree add /private/tmp/wt <commit>          # never the shared tree
# relax hot_v3.rs's `selected_action != InlineOrdinary` arm to `return Ok(None)`
for p in registry trading core claims custody rent; do
  cargo build-sbf --manifest-path programs/dclutch-$p-sbf/Cargo.toml \
    --sbf-out-dir <elves> --features hot-cu-profile   # trading only for the feature
done
SBF_OUT_DIR=<elves> cargo test \
  --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \
  --test direct_registered_creation_hot -- --ignored --test-threads=1 \
  --exact registered_sell_then_buy_execute_on_current_elves --nocapture
```

`hot-cu-profile` prints the phase ladder, which is what turned "dies somewhere
in Trading" into "dies between `pf-invocation-resolved` and the Custody arm" in
one run. **Every figure above is diagnostic** — the checkpoint calls cost
compute — and none is comparable with a production number.

## 2026-09-01 — the registered Buy through four walls, and the first route to open a Custody account

Behind a wall-A probe (worktree only, never committed), the registered Buy moved
four walls in one session on real ELFs:

| | code | CU | depth |
|---|---|---|---|
| start | `Content` 0x4003 | 308,354 | account projection, no child CPI |
| after (1) | `Release` 0x4001 | 571,047 | Custody preflight, no child CPI |
| after (2)+(3) | `AccountFrame` 0x6001 | 776,043 | inside the **first** Custody CPI |
| after (4) | `Commit` 0x4005 | 1,205,519 | **all three Custody children succeed** |

`InitializeReplay` 123,796 · `OpenVault` 141,105 · delegated deposit 136,253. A
registered Buy now creates its maker replay and record, opens a Custody replay and
a TradingPrincipal vault, and moves the maker's collateral into it. Diagnostic-build
figures; not production-comparable. The Sell executed end to end at **every** step
— no fix moved a refusal backwards.

**The four defects.** (1) `require_key` read the **input** bank while
`project_identity` wrote the **output** bank in the same pass — unsatisfiable by
any transaction; `ordinary_account_artifacts_v3.rs:589-617` had the whole argument
written out and the registered family did the opposite. (2) the Realm record's
**address** was projected into `CustodyRequestLayoutV1::REALM`, a **content
digest** field. (3) the Effect wrote the record beneficiary into `RENT_REFUND`,
which `initialize_replay` requires to differ from the payer — and this family's
beneficiary *is* the payer. **The lane got (3) backwards first**: moved the fixture
to match the Effect and committed it; the next wall said the Effect was wrong.
Reverted in the same commit.

### Wall C — an exemption nobody needed until now

`require_committed_rent_exemption_v3` refuses on the Custody replay: **0 lamports,
288 bytes, needs 2,895,360.** `output_lamports` is seeded from the **observed**
prestate (vacant), and `commit_output_lamports_v3` writes that zero back over the
rent the child deposited. It exempts only coordinates an `EffectProgramV5` funding
action names — accounts *Trading* creates. A local `TransferLamports` is not the
repair: `require_child_disjoint_from_local` correctly refuses a child reaching a
coordinate the Effect mutates.

> **Registered creation is the first route in the protocol that opens a Custody
> account.** The missing exemption — a coordinate funded by a child CPI — is
> family-neutral machinery in `hot_v3.rs`, and nothing ever needed it before.

### Two shared-index lessons, reported against the lane itself

`git add` + `git commit` on the shared index swept in another lane's staged rename.
**`git commit -o <paths>` from here on.** And restoring `hot_v3.rs` from a snapshot
to revert a probe can erase another lane's edit in the window — the affected lane
was told to verify rather than left to discover it.

## 2026-09-01 - the frame table was a design, and the producer never read it

**`0xC00A InstructionsSysvarAccount` is repaired, and the repair was derivation,
not adoption.** `admitted_v3.rs` wrote out an eighteen-account admitted CPI
frame. Nothing has ever produced it. There is exactly one admitted-accelerator
CPI site in the tree -- `admitted_composition_v3.rs:410`, family-neutral, reached
by General like everyone else -- and it emits caller authority, the **whole**
common Hot fixed frame, eight strategy-evidence accounts, then the runtime slice.

**Which repair to write was established before either was written**, because the
two are different acts. The eighteen coordinates are not a strict prefix of the
forty-eight -- but every account the table *names* exists in the real frame, and
the real frame carries one the table never named at all, the accelerator program
itself at 46. **No contract gap: a second author for an offset.** So the fix is
to derive from `HOT_*_ACCOUNT_V3`, not to adopt
`authenticate_accelerator_invocation_v4` -- that is the Dealer accelerator's
path and it re-derives certificates, admissions and deployments, which the
General accelerator deliberately does not do because that authentication stays
in the SVM adapter. **It needs coordinates, not a second verifier.**

| | table said | producer sends |
|---|---|---|
| instructions sysvar | 4 | **30** |
| Trading program | 5 | **26** |
| runtime start | 18 | **48** |
| accelerator program | *unnamed* | **46** |

At index 4 the real frame carries a vacant CapabilityManifest staging cursor.

### The A/B that attributes it exactly

Same worktree, same six other ELFs, same bundle; **only the accelerator ELF
swapped**, through the test's own `DCLUTCH_GENERAL_ACCELERATOR_ELF_PATH`:

| accelerator | OpenBatch N=2 through real Trading ELFs |
|---|---|
| `e9544323` (literal table) | `InstructionError(2, Custom(49162))` = **`0xC00A`** |
| `53ce2075` (derived table) | **past it** -- `Custom(16388)` = `0x4004 Transition` |

And the next wall named itself on the first run, in the log line the previous
commit added: **`general: refused, config rejects the bank's generation or
basis`** -- `ConfigMarket`. The instrument built for the Freeze wall paid for
itself on a wall in a different action, in a different workspace, the same day.

### The harness had been under-loading the accelerator by thirty accounts

**This is the finding underneath the finding.** With the real frame, three
lifecycle rows at runtime width 258 die *inside* the accelerator on `Error:
memory allocation failed, out of memory` --
`hostile_n258_initializes_and_refuses_candidate_substitution`,
`real_sbf_runs_full_settlement_at_runtime_widths_one_and_258`,
`real_sbf_verify_candidate_executes_every_row_and_terminal_result_at_runtime_widths`.
InitializeSettlement at N=258 is 135 accounts where it was 105. Every one of
those rows passed yesterday against a frame **thirty `AccountInfo`s smaller than
the one Trading actually sends**, on a 64 KiB heap whose peak this program's own
source already records as having been pushed past 32 KiB by the GEN-SEVEN
register widening.

> **The suite goes 25/0 to 22/3, and 22/3 is the truer number.** The three that
> fail are failing at a width the old frame could never have exercised honestly.

The frame is not free at width 1 either. Freeze, same test, before and after the
frame correction: accelerator CPI **21,108 -> 27,045** CU at width 1 and
**45,758 -> 51,695** at width 258; top level 35,693 -> 49,670 and 68,220 ->
84,718. That is what the thirty accounts cost to deserialize, and it was never
on any budget.

**A probe that measured the wrong thing, recorded because it nearly convinced
me.** To test the heap hypothesis I doubled the requested heap frame in
`lifecycle.rs` -- and *all ten* rows failed rather than three, because
`authenticate_top_level` requires the grant to be the **exact**
`request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1)` and a doubled grant fails a
different conjunct entirely. The probe touched the heap and measured the heap
*rule*. Ledger's own line, earned again: **verify the instrument before
believing the reading.**

### Two restatements deleted, and the one that proves the point

`lifecycle.rs` derived the runtime *start* under a comment explaining that
restated constants stop agreeing silently -- and two lines below that comment
wrote out `4` and `5` for the sysvar and the Trading program. `freeze.rs`, made
to derive all three in `ecc43002` twelve hours earlier, **needed no edit at
all.** That is the comparison that shows what derivation buys, and it is the
same file, the same week, one commit apart.
`family_hot_campaign.rs:141-147` restated `18`, `0`, `4`, `5` as its own consts
and now imports them.

The evidence suffix is the one span the contract still states rather than
derives, so a `const` assertion pins its count to its last named coordinate: a
ninth evidence account stops compiling instead of shifting every runtime
coordinate by one.

**Routed, not edited.** `crates/dclutch-operator/src/general_hot_v3.rs:4682`
reads 78 where its recorded campaign frame says 48 -- exactly as predicted, and
its own comment says re-run the campaign rather than move the number, so it is
the operator lane's re-run. `admitted_composition_v3.rs:65-72` should make the
four `ADMITTED_ACCELERATOR_*_V4` constants aliases of these, which removes the
last restatement of this layout in the tree; Direct lane's file.

## 2026-09-01 — a second author for an offset, and thirty accounts on nobody's budget

**Derive, not adopt — established before writing either.** Every account the
admitted-V3 table *names* exists in the real frame; the real frame carries one the
table never named (`accelerator_program` at 46). **No contract gap — a second
author for an offset.** Adopting the Dealer accelerator's
`authenticate_accelerator_invocation_v4` would have been the wrong repair: that path
re-derives certificates, admissions and deployments, and the General accelerator
deliberately does not — that authentication stays in the SVM adapter. It needed
coordinates, not a second verifier.

| | table said | producer sends |
|---|---|---|
| instructions sysvar | 4 | **30** |
| Trading program | 5 | **26** |
| runtime start | 18 | **48** |
| accelerator program | *unnamed* | **46** |

All coordinates now derive from `HOT_*_ACCOUNT_V3` plus two offsets the contract
owns and names once. The evidence suffix is the only stated span, pinned by a
`const` assertion to its last named coordinate — **a ninth evidence account stops
compiling instead of shifting every runtime coordinate.** `freeze.rs`, made to
derive all three at `ecc43002` twelve hours earlier, **needed no edit at all** —
which is the point of derivation.

**Red then green with the variable isolated** — same six other ELFs, same bundle,
only the accelerator swapped: literal table → `0xC00A`; derived table → past it, to
`0x4004 Transition`. **And the next wall named itself on the first run**, through
the log line the Freeze-wall commit had added: *`config rejects the bank's
generation or basis`* — `ConfigMarket`. An instrument built for one action in one
workspace paid for itself on a different action in a different workspace the same
day.

### The harness had been under-loading the accelerator by thirty accounts

With the real frame, three lifecycle rows at width 258 die **inside** the
accelerator on `memory allocation failed, out of memory` — InitializeSettlement
N=258 is **135 accounts where it was 105**, on a 64 KiB heap the program's own
source records as already past 32 KiB. Suite **25/0 → 22/3**, and 22/3 is the
truer number: those rows had never run against the frame Trading sends. Not free at
width 1 either — Freeze accelerator CPI **21,108 → 27,045** CU, top level
35,693 → 49,670.

> **That is what thirty `AccountInfo`s cost, and it was on nobody's budget.**

**A probe that measured the wrong thing, kept because it nearly convinced:**
doubling the heap grant to test the OOM hypothesis failed **ten** rows, not three —
`authenticate_top_level` requires the **exact** grant, so the probe measured the
heap *rule*, not the heap.

Two shared-index collisions during commit were **waited out, never unlocked by
hand.**

## 2026-09-01 — under-collateralized issuance of n·(basis_scale − 1), invisible at scale 1

**The mixed-unit solvency gate is closed by its real owner** (`5ec149fa`), and the
scholar's cost was wrong in the good direction: **the Dealer needs no new account.**
The shared Hot prefix already hands both accelerators an
`AuthenticatedProductRuntimeV3` whose `payout_scale` *is* the value, and
`v3_accelerator_accounts.rs:546` / `v4_equity_accelerator_accounts.rs:439` already
pin that record's `semantic_basis_id` to the Claims aggregate's `basis_id` — an
aggregate that is a PDA of the Market. Zero new accounts, zero new CPI; the
authenticating join was already there and already checked.

Units are now stated by the types, as `dclutch-dealer-scenario-kernel` had already
documented: collateral and obligations are **atoms**, Claims inventory is
**units**, split/merge are **sets** — converted once at each meeting point.

### The dangerous direction was real, and it was in the composer

`v3_composer.rs` `PrincipalToHoard` moved the **set count** as an **atom amount**: a
split of *n* sets funded the Hoard with *n* atoms while Claims minted *n* sets
against it — **under-collateralized issuance of `n · (basis_scale − 1)`.** Invisible
only because every in-tree fixture uses 1. Zero now refuses by name at every read;
not on `DealerConfigV4`, not on `CoreState`.

**Proof:** `468f66b3` green; a second test describes one width-2 pool at scale 1
and 97 and demands one plan in atoms. Teeth: a floor one atom above the candidate
refuses at both, one atom below admits at both. **Red proved** by neutralizing the
conversions — both scale tests red, the four scale-1 tests green, which is also the
control: nothing changes at scale 1. **Residual, named:** no in-tree market has
scale ≠ 1, so the obligation denomination is settled by doc, not by a live witness.

### Register 116, repaired by General's precedent (`322de4b2`)

The account pass **projects** the observed obligation key into register **117**
(bank 117 → 118, still six scratch pages at every admitted width); the request
profile keeps authoring 116; the transition carries `identity_eq(116, 117)`. Two
witnesses, both reading **artifacts** — `writes_register` over the encoded Profile13
(writes 117, does not write 116); the encoded transition's instruction region
containing the `identity_eq` record byte-for-byte from a one-instruction reference
program — because the previous lane wrote no test precisely to avoid the builder as
its own witness. Both proved red. **28/1 on rebuilt ELFs**; the widened bank re-pins
every selector-9 artifact with no regression.

### The equity-Add wall, localized further

Dies at **573,103 CU, `0x4003 Content`, invoking no child program** — entirely
inside Trading. The LP Open immediately before it in the same test consumes
1,042,690 CU and invokes the accelerator successfully, so a near-identical route
works. **And the hostile beside it is a universal donor**: `accepted.rs:7751`
demands a substituted Position identity refuse `Content` with the accelerator
uninvoked — exactly what the honest Add does. Named debt behind it: **2,126
`TradingSbfError::Content` sites** in trading-sbf (785 in `hot_v3.rs`) and **2,386
`map_err(|_| …)`** (492 in `hot_v3.rs`).

### A test with two causes, and a process error owned

`lp_descriptors_rederive_every_successor_artifact` had two: the fixture declared zero
bytes at the one `AdapterAuthenticatedVariableData` prestate; and behind it,
`finalize_dealer_lp_descriptor_v3` required `LIFECYCLE_PRESTATE_ARTIFACT_PROFILE`
while the LP encoder stamps `DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE` — **no LP
descriptor could be finalized through that route at all**, masked by the first.
The lane flagged that it committed the first fix without re-running the test.

**Shared-tree protocol, earned:** two `accepted` runs died on another lane's
half-applied refactor mid-SBF-build. **A `cargo check` gate before the SBF build**
is what got the third through.

## 2026-09-01 — REVERSAL: the operator was right, and the composition was wrong

**This ledger recorded, twice, that the Structured operator's route geometry was
wrong on four grounds.** The scholar verified four facts; the coordinator
spot-checked them and they held; **the inference from them was inverted**, and the
lane that went to build the repair found it by looking for a case where the
accused shape is admitted. Four grounds, strongest first:

1. **`claims_composition_v3.rs:639-641` refuses a representation route whose kind is
   not `Once`** — pinned by a green hostile at `:2336,:2347`. The specified
   `AffineOnce` repair would have passed one check and died two frames later.
2. **`AffineOnce`'s `repeated_item_count` *is* the effect tail count**
   (`effect-kernel/src/v3.rs:968`), forced equal to the **Product** width by
   `require_tail_count_agreement_v3`. Requiring it `== header.asset_count` binds
   **K == N** — which the Structured family exists to deny; the operator's own green
   test builds K=3 against N=258.
3. **No producer ever emitted the affine shape.** Its only author was the
   composition's own test fixture. `AFFINE_BATCH` is a different request family
   with its own magic.
4. The rule `a6a56e0c` replaced (empty commit body) was already request-bound —
   `physical_account_count = 32 + asset_count*4` — exactly what the operator
   declares.

> **Three true observations and a wrong conclusion, for the third time today.** The
> cross-boundary instrument is what caught it: `dclutch-claims-svm` is now a
> test-only dep of the operator, and
> `the_claims_composition_admits_the_full_width_route_this_operator_emits` builds
> the real artifact and asks the real composition. Red with the exact discriminant
> `ClaimsCompositionErrorV3::Route`, green after, at N=258/K=3 deliberately
> (`368459c9`).

### Then the route moved four walls on real ELFs

Canonical Token-2022 built on hbox (`e2acdfb7…`) and re-verified locally; seven
dclutch ELFs built here.

- **#1 transition — corrected, not deleted** (`0f661415`). The probe named it:
  *operation 4, `CheckFailed`, register 9 (=2) vs register 3 (=7)* — the guard
  refused the tree's own fixture. Replaced with `nonzero`, **the check the
  executing sibling uses on the same register**
  (`rational-lifecycle-hot-v3/src/artifacts.rs:368-370`). Landed one step earlier
  than the stated rule, with the reason in the commit: a *correction to the
  sibling's law* is a re-proof on the other side, not a deletion, and it advanced
  execution past the fold. One line to back out.
- **#4, new** — the Claims preflight required the child program **exactly once**
  and counted **raw keys**, so the representation wire's inactive-slot sentinel
  (the Claims program id) counted twice: `occurrences=2, required=1`. Only
  `IssueStructured`/`UnwrapStructured` leave a slot inactive, which is why nothing
  had reached it. Fixed alias-aware — an alias carries no authority.
- **#5** — the same predicate duplicated in `claims_composition_v3.rs:164-171`. One
  shared alias-aware counter now.
- **#6, where it stops** — `TradingSbfError::Release` at
  `claims_composition_v3.rs:178-184`: the child frame's first account is not the
  Trading-derived `CallerAuthoritySeedsV1` PDA. The common-Hot fixture derives its
  caller against `CLAIMS_PROGRAM_ID`, and the file's own comment says the Claims
  route is the one child route with no preflight derivation to reuse.

#4 and #5 are **uncommitted by discipline**: they live in files another lane is
mid-refactor in, and committing them would sweep that work. Isolated patches of
only those hunks are handed to the file's owner. `hot_v3.rs:9584` still carries
`map_err(|_| Content)` over the composition error — the surfacing that found the
wall was never landed.

Both red witnesses **corrected, not deleted**: `1357 → 1397` with the reason; the
claims witness's *ID* carried the number too, so the number left the ID.

## 2026-09-01 - the grant was always twice what the program could see

**Measured before touching the grant, and the numbers force neither option.**
The accelerator declares a `custom-heap` **feature** in its `Cargo.toml` and
never implements one, so `solana_program::entrypoint!` installs the default
`BumpAllocator::with_fixed_address_range(HEAP_START_ADDRESS, HEAP_LENGTH)` --
and `HEAP_LENGTH` is hardcoded at **32 * 1024** in solana-program-entrypoint
3.1.1, bumping downward from the top of that 32 KiB.
`DIRECT_HOT_HEAP_FRAME_BYTES_V1` is **65,536**.

A new `heap-profile` feature logs the allocator's outstanding bytes beside both
ceilings, and says which question it answers: **not** the granted frame, which
the ledger's own rule says only a raw write into the heap region can measure and
which this crate cannot do because it forbids `unsafe`. It measures what this
program's allocator has handed out and the ceiling it refuses past -- the pair
that decides whether an allocation fails.

Settlement at runtime width 258, 135 accounts, the row that dies:

| mark | outstanding |
|---|---|
| entry (after entrypoint deserialization) | 7,369 |
| frame-validated | 11,482 |
| input-bank | 26,515 |
| next allocation | **fails** -- 6,253 left of 32,768 |

> **The answer is not a larger declared grant and not a narrower frame. The
> grant was always large enough; the program has never been able to see half of
> it.**

The fix is to declare a custom heap over the granted region, which Trading
already does at `entrypoint_adapter.rs:697`. `BumpHeapV1` lives inside
`programs/dclutch-trading-sbf`, so sharing it is an extraction from another
lane's program and writing a second one here means `unsafe` in a crate whose
policy forbids it. **Routed, not done unilaterally** -- the shape is settled and
the measurement is the argument.

**This revises what 22/3 means.** Those three rows do not fail because the real
frame is too big for the heap the protocol grants. They fail because thirty more
`AccountInfo`s pushed a program already spending 26,515 of a 32,768 ceiling past
it, while half its paid-for heap sat unreachable the whole time. **The old frame
was hiding a headroom figure, not creating one.**

**Byte-identical was the wrong claim to reach for.** With the feature off the
ELF hash still moves (`53ce2075` -> `3a1393da`) because 49 added lines shift
line tables -- the diff is 49 insertions and zero deletions, and the committed
`lib.rs` rebuilt inside the same dirty tree reproduces `53ce2075` exactly. The
right control for an added line is identical compute on a real run: 27,045 /
49,670 / 51,695 / 84,718 CU, all four unchanged.

## 2026-09-01 - the producer cannot satisfy two of the four conjuncts

**`ConfigMarket` localized in one run, because the refusal had a reader.** The
path prints both sides of both conjuncts now, and `require_market` stays the
sole authority for the comparison -- called, not reimplemented.

| conjunct | config | bank |
|---|---|---|
| generation | 9 | 9 |
| semantic basis | `0x56` x32 | **`0x00` x32** |

**It is zero because nothing writes it.** The General AccountProfile sources
eighteen identity registers and `identity::SEMANTIC_BASIS_ID` is not among them
-- no `ProjectDataIdentity` rule anywhere in `account_rules_v3.rs`. Nor is there
a field to source it from: `GeneralRootV2` records magic, header word, market,
config id, generation, revision, next batch sequence, open batches and
lifecycle, and **no basis**.

The conjunct behind it is the same defect: `identity::PRODUCT_RECORD_DIGEST` is
sourced **zero** times in that file, so `authenticated_general_domain`'s fourth
comparison is queued to fail identically the moment the third stops.

> **Two of the four conjuncts every one of the fifteen General actions crosses
> first are unsatisfiable on the real route -- and it is the frame's shape
> again. The harnesses hand-write these registers into the bank, so the harness
> and the accelerator agree and the producer cannot.**

I wrote `CONFIG_CLAIM_BASIS_ID` into `freeze.rs`'s bank by hand earlier today.
That is the same act, and it is why that file passes. **Three findings this
session, one shape**: a table, a frame, and now a register bank, each with a
harness that could satisfy it and a producer that could not.

**The constraint any fix must meet**, stated because getting it wrong has three
recorded instances in this tree: **the source must not be the config.**
`config_id` is the pattern -- the *root* records the config identity as a field,
the profile projects that field, and the accelerator recomputes
`hash(config_data)` and compares. Two independent authorities. A basis register
projected out of the config account would compare the config to itself and pass
forever.

## 2026-09-01 — half the granted heap has never been addressable

**The General accelerator's ceiling is 32,768, not 65,536.** It declares a
`custom-heap` *feature* in `Cargo.toml` and **never implements one**, so
`entrypoint!` installs the default `BumpAllocator::with_fixed_address_range` with
`HEAP_LENGTH` hardcoded at `32 * 1024` in solana-program-entrypoint 3.1.1.
`DIRECT_HOT_HEAP_FRAME_BYTES_V1` is 65,536. **Half the paid-for frame sits
unreachable.**

Measured (`f0a4c4f6`, a `heap-profile` feature that states which question it
answers — the allocator's outstanding bytes and the ceiling it refuses past, not
the grant, which only a raw write could measure and this crate forbids `unsafe`).
Settlement at width 258, 135 accounts:

| mark | outstanding |
|---|---|
| entry (after deserialization) | 7,369 |
| frame-validated | 11,482 |
| input-bank | 26,515 |
| next allocation | **fails** — 6,253 left of 32,768 |

**Neither candidate repair is forced** — not a larger grant, not a narrower frame,
and the exact-grant rule does not move. The fix is a custom heap over the granted
region, exactly Trading's `BumpHeapV1::with_base`
(`entrypoint_adapter.rs:697`) — which lives inside `programs/dclutch-trading-sbf`,
so sharing it is an extraction, and a second implementation here means `unsafe` in
a crate that forbids it. **This revises 22/3: those rows fail because thirty more
`AccountInfo`s pushed a program already at 26,515 of 32,768 past it, while half its
heap sat unreachable. The old frame was hiding a headroom figure, not creating
one.**

**A wrong control, caught**: feature-off is *not* byte-identical (`53ce2075` →
`3a1393da`) because 49 added lines shift line tables — 49 insertions, zero
deletions, and the committed `lib.rs` rebuilt in the same tree reproduces
`53ce2075` exactly. **The right control for an added line is identical compute**
— all four figures unchanged.

### `ConfigMarket`: zero because nothing writes it

`require_market` stays the sole authority — called, not reimplemented; a reader
was added around it (`3b7a1025`). Generation 9 = 9; semantic basis `0x56`×32 in
the config against **`0x00`×32 in the bank.** The General AccountProfile sources
eighteen identity registers and `SEMANTIC_BASIS_ID` is not among them — no
`ProjectDataIdentity` rule, and `GeneralRootV2` has no basis field to source from.
`PRODUCT_RECORD_DIGEST` is sourced **zero** times in the same file, so the fourth
conjunct is queued to fail identically.

> **Two of the four conjuncts every one of the fifteen actions crosses first are
> unsatisfiable by the producer** — the frame's shape a third time today: the
> harnesses hand-write those registers, so harness and accelerator agree and the
> producer cannot.

The lane named its own instance: it hand-wrote `CONFIG_CLAIM_BASIS_ID` into
`freeze.rs` this morning — the same act, and why that file passes.

**Constraint on the fix, written down because the tree has three recorded
instances of getting it wrong: the source must not be the config.** Copy
`config_id`: the *root* records it as a field, the profile projects the field, the
accelerator recomputes `hash(config_data)`. A basis projected out of the config
account compares the config to itself and passes forever.
