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
| S1 stabilize | whole-tree git surgery + workspace convergence | launched |
| L3 frontend | `apps/dclutch-web` only | launched |

Lane protocol: named-file staging only; inspect the complete staged path list
before every commit (AGENTS.md); never `git add -A`; never `git stash`; report
incoherent WIP rather than deleting or "fixing" it.
