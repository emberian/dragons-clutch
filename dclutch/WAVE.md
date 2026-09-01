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
