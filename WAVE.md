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

## Queue (next linger points)

- **LINGER₂ Sonnet batch** (one batched Opus reviewer behind it): fold all
  satellite workspaces into the root workspace (no dependency skew exists —
  every lock pins solana-program 3.0.0); delete the two empty skeleton dirs
  (`crates/dclutch-series-v2-kernel`, `programs/dclutch-effect-proof-sbf` —
  owner check first); un-gitignore `programs/dclutch-series-sbf/Cargo.lock`;
  clear the 30-warning `dealer_chain` fixture debt.
- **Frontend ABI convergence** (small Opus): `302ad80` made the Registry
  continuation headerless and deleted `DIRECT_NATIVE_EVIDENCE_REGISTRY_BIAS_V3`;
  the web generator + checked-in `lib/generated/directInlineV3.ts` are stale
  against the protocol. Also the two pre-existing verify failures
  (`abi:found` marker, `abi:rational-terminal-v3` ENOENT) and converging
  `productV2.decodeCapability` onto `lib/capabilityManifest.ts`.
- **Cycle-2 General charter item**: general-sbf's activation handler
  (`c1cdc82`-era) needs the missing contract-side counterpart — the
  `GENERAL_ROOT_PDA_DOMAIN_V2` preimage is a protocol fact its owner must mint.
- **Cycle-3 pull-forwards** (from L3): `/markets/:market` detail before the
  wizard; portfolio via direct Position-PDA derivation (`[POSITION_SEED,
  market, owner]` — no indexer needed); wizard = compose `/product-v2` →
  `/found` carrying compiled Product identity. The missing market *indexer*
  remains the one honest discovery gap.

## THE DEMO CUT (organizing target as of 2026-08-26 late)

External signal: direction clarity + a proof of concept is the near-term ask.
The demo is: create → view → trade → resolve → redeem, on devnet, from a
hosted page with a browser wallet. IN: one demo Realm (demo collateral mint,
honest faucet), two SOL/USD range-protection markets on real devnet Pyth,
Direct trading, resolution, redemption, portfolio. OUT of demo v1: General,
Dealer, Series, Structured/Fractional/Rational surfaces, public creation
wizard. Existing lanes W1c (Open) and W2c (Hot budget) ARE the demo path; DA
prepares devnet deploy-and-recycle (deploy itself still requires ember's
explicit named authorization); FD prepares the hosted demo frontend. Deep
internal excellence work beyond the in-flight lanes is PAUSED in favor of the
demo path until further notice.
