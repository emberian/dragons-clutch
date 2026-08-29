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

- κ ENFORCEMENT (trigger: the LBV2 layout slice / RECORDS-MIGRATE cluster):
  the predicate exists and is proven (KAPPA), but no on-chain route calls it —
  Found sees the Source not the principal; FoundingV5 the reverse; and a
  founding-only check is not a cap since principal grows per complete-set
  split. Real shape: the cap on the Market root, checked at founding AND at
  split. Interacts with the founding-root ADR; design queued at
  MAINNET_STATE_RELAY §11.2.

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
