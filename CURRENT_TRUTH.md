# Dragon's Clutch: current truth and control plane

Status date: 2026-08-19. This is the operational entry point for engineering
handoffs. [`PROJECT.md`](PROJECT.md) remains the canonical product brief and
[`AGENTS.md`](AGENTS.md) remains the authority and correctness policy.

This file supersedes the *current-status and next-work* sections of
[`GOAL.md`](GOAL.md), [`CODEX_HANDOFF.md`](CODEX_HANDOFF.md),
[`CLAUDE_HANDOFF.md`](CLAUDE_HANDOFF.md), and the dated drift reviews. Those
files remain useful history, rationale, and evidence pointers; their old test
counts, gap rows, and “next” lists are not the live queue. The dependency-ordered
queue is [`docs/V1_BACKLOG.md`](docs/V1_BACKLOG.md).

## 1. How to read a status claim

These labels do not imply one another:

| Label | Exact meaning |
| --- | --- |
| **PROVED-MODEL** | A named proof assistant checked a theorem about a named mathematical model. This says nothing about Rust without a separately checked refinement. |
| **CHECKED-RUST-SUBSET** | A pinned verifier checked a named Rust source subset under named assumptions. This says nothing about account adapters, CPI, the SBF compiler, or the runtime. |
| **HOST-TESTED** | Ordinary host execution passed the named tests or exhaustive finite campaign. It is executable evidence, not a proof. |
| **SBF-EXECUTED** | A compiled SBF program executed in a local Agave bank or loopback validator. This is not public-cluster, deployment, or mainnet evidence. |
| **MODEL-ONLY** | A reference model, experiment, specification, or synthetic cost result exists but is not the production transition. |
| **PROPOSED** | A design or policy has not crossed its promotion gate. |
| **IN-FLIGHT** | Bytes exist in the shared dirty worktree but are not an accepted baseline and may be internally inconsistent. |
| **STOP** | The named surface must not be promoted, deployed, or described as complete until its acceptance gate closes. |

“Implemented” is too broad for this control plane. Each row below names the
strongest evidence actually held. Passing a differential proves agreement with
its oracle on that corpus; it does not prove that both implement the right
economics.

## 2. Snapshot boundary

There are three different snapshots in play. Do not merge their claims:

1. The last committed local-validator and Token-2022 bring-up record is commit
   `858f408`, with a same-machine twice-built ELF digest
   `59c48c482831626ae9d7cb908f4de0e3f93b1572cdd82105c61f2f87bdaad25f`.
   Its loopback gate recorded 10 accepting transactions, 22 expected refusals,
   and an 11-step conceptual walk; its isolated bank suite recorded 15
   scenarios. Those facts are **SBF-EXECUTED for that source only**. The walk
   used genesis-injected program accounts and mostly noncommitting
   `simulateTransaction`; it is not a permissionless lifecycle.
2. `MANIFEST.baseline.json` is still schema v1 from commit `27ea3b3`, with
   33/33 recorded gates. It predates the current manifest generator, current
   HEAD, and the pooled-custody rewrite. It is historical evidence, not the
   current baseline. The committed generator is schema v2-capable, but no clean
   full v2 manifest has yet been emitted and committed.
3. Subsequent commits contain the typed source relation, fail-closed settlement
   preflight, narrow Verus transfer result, deterministic invariant campaign,
   artifact-audit tool, and signed committed-runner implementation. They have
   not yet been joined into a clean runtime baseline. The shared worktree also
   contains an uncommitted pooled-custody repair and other generated/research
   outputs. Only the committed, individually named rows below inherit their
   focused evidence; nothing after `858f408` inherits its SBF/runtime result
   until the joined gates are rerun.

No checked release manifest, signed tag, independent rebuild, public-network
deployment, official client URL, or value-bearing market exists. Gate L0 remains
open. Nothing in this repository authorizes signing, funding, deployment,
publication, regulator contact, or describing a program as official.

## 3. Current capability matrix

| Surface | Strongest honest status | What the evidence establishes | Boundary / STOP |
| --- | --- | --- | --- |
| Product and Realm model | **PROPOSED** | The canonical brief fixes the intended fully collateralized, collateral-generic, categorical basis. DREGG is one dogfood profile, never a kernel branch. | No Realm profile is a release fact; no real token profile is authenticated or frozen. |
| Eggcrate claim kernel | **HOST-TESTED**; corresponding semantic results are **PROVED-MODEL** | Safe, fixed-layout `no_std` Rust executes split, merge, materialize, dematerialize, resolution, and redemption fragments. Lean 4.33.0 checked named solvency, supply, transition, and complete-set properties of its separate hand-written model. | The Lean/Rust correspondence is manual and vector-bounded, not proved. The full Rust kernel is not Verus-verified. |
| Verus refinement | Narrow arithmetic seam **CHECKED-RUST-SUBSET** | Pinned Verus `0.2026.08.15.7d4628a` checked two properties of the exact executable body of `prepare_internal_transfer`: correct debit/credit with sum conservation, and exact overflow refusal. The record pins the production-source and reviewed call-site digests and includes two expected-red mutations. | This is only `P-SUP-01` arithmetic plus a manually reviewed call seam. Phase, owners, state validity, codecs, accounts, CPI, SBF code generation, runtime, and deployment are outside it. The historical E0 probe remains a separately recorded failure. |
| Rocq shadow | **MODEL-ONLY** | Pure definitions and obligations exist and can typecheck under the installed tool. | They contain no completed theorem inventory or Rust refinement. |
| Accumulator | **HOST-TESTED** | Source-neutral adjacent summary, coverage, interval, TWAP, and terminal calculations have bounded host tests. | It authenticates no external source, deployment generation, clock, archive, or canonical record by itself. |
| Distributional products | Categorical primitive **HOST-TESTED**; broader compiler **MODEL-ONLY / IN-FLIGHT** | Normal resolution can select one exhaustive cell. Bounded payoff shapes can be represented as integer portfolios of primitive Eggs. | Degree 2/3 native payout bases still refuse. Approximate Gaussian/range products are not production ABI or native fractional liabilities. |
| Batch relation | **HOST-TESTED** | The coupled simplex relation, exact witness checks, bounded candidate comparison, pairing, and a bounded streaming verifier have extensive finite and adversarial host campaigns. | The relation canonicalizes accepted candidates and supports “best valid submitted candidate”; it does not prove globally optimal search. It is not yet joined to funded onchain orders. |
| Solana layouts | **HOST-TESTED** | Versioned fixed account/intent codecs, canonical ids, order pages, and candidate/receipt layouts exist with hostile-byte tests. | Several necessary state owners and initialization transitions are absent; proposed source and checkpoint layouts are not frozen ABI. |
| SBF processor skeleton | **SBF-EXECUTED at `858f408`** | Eight reference-oracled instruction families executed byte-exactly against their offline oracle in a loopback validator; real Token-2022 mint/burn/transfer and rollback cases executed in a local bank. `PlaceOrder`/`CancelOrder` have host coverage. | Agreement did not catch the shared pooled-custody error. `SettlePage` is a deliberate stub. The recorded processor depends on injected program-owned state and is not an end-to-end venue. |
| Pooled collateral custody | Accepted baseline: **STOP**; corrected model/code: **IN-FLIGHT** | The repair makes `Endow` the sole inbound token boundary and makes split/merge/internal redemption accounting reclassifications. | The joined host/SBF gates, withdrawal, reservation accounting, and market-wide custody equation are not yet closed. Do not cite the green `858f408` walk as correct custody economics. |
| Token-2022 outcome truth | **STOP** | Real Token-2022 materialization and dematerialization CPIs execute with exact per-CPI deltas. | An ordinary holder can burn outside the program and desynchronize the shadow supply. `ExternalAccount`/`SupplyLedger` cannot remain an authoritative duplicate of mint supply without an explicit burn policy and repair relation. |
| Internal redemption | **SBF-EXECUTED at `858f408`**, but custody semantics are superseded | Evidence-gated internal redemption executed through the local runtime. | The evidence is not source-authenticated, and the old path incorrectly moved custody tokens instead of crediting pooled cash. The corrected path is in flight. |
| External redemption | **STOP** | No production instruction exists. | A transferred materialized winning Egg cannot redeem. This is a claimant-liveness and stranded-principal blocker. |
| Cash withdrawal | **STOP** | No production instruction exists. | A backed Endow can leave free cash stranded in pooled custody. No lifecycle is economically closed without an exact unreserved-cash exit. |
| Account creation | **STOP** | Genesis initializers encode several state families; `CreateMarket` can initialize pre-existing zero program-owned state and create token accounts/mints. | No normal signer can create the eight required program PDAs; later Position triples, Feed/Epoch/ClearWork/CandidateFeed state are also incomplete. Blank-bank permissionless construction has not run. |
| Observation/source admission | Typed admission relation **HOST-TESTED**; runtime path **STOP** | A committed source-independent typed design checks identity, generation, sequence, clock, freshness, confidence, grid, normalization, and resolution-archive lineage. | Live `FeedAdvance` still accepts caller-authored program-owned bytes. Live `Resolve` can consume a separate caller buffer. No concrete audited source parser, archive commitment, Clock join, or unique canonical bucket is load-bearing. |
| Resolution replay domain | **STOP** | Resolution is idempotent through market state. | The live global resolution path also consumes an unrelated owner replay account. Global and owner-scoped replay domains must be separated. |
| Order admission | **HOST-TESTED skeleton / STOP economically** | Page encoding, placement, cancellation, expiry, and tombstones execute at the program-host layer. | Placement reserves no cash or claims. Epoch live cardinality after tombstones is not a single persisted truth. |
| Onchain clearing/settlement | Byte preflight **HOST-TESTED**; economic transition **STOP** | A committed fail-closed checkpoint seam binds the complete page set, epoch, candidate, feed, and checkpoint without writing state, and pins prerequisite rank 1 as unreachable. | Reservations, live count, policy preimage, full-width identities, portable checkpoint codec, authenticated initialization, candidate-set closure, and complete entitlement freeze are prerequisites. Production `SettlePage` still refuses before reading the seam. |
| Signed committed walk | Runner implemented; evidence **STOP** | A committed local-only runner requires real ephemeral signatures, submits committed transactions, confirms them, and reloads watched accounts. | No green Clutch walk is recorded. It remains genesis-assisted until account creation lands and must label free cash/materialized winners as stranded until Withdraw and external redemption land. |
| Static Glass | **HOST-TESTED inspect-only prototype** | A static client can render local terms and inspect unsigned intent material without owning consensus truth. | No release-bound program/spec/source manifest, wallet transaction path, complete browser/accessibility audit, or hosted official instance. |
| Economics and fees | **MODEL-ONLY / PROPOSED** | Synthetic solvency, cost, fee, manipulation, and allocation experiments exist. | Fee base, carry, split, keeper funding, failure payout, and liveness capitalization are not release policy. Hoard principal is never an allowed fee or bounty source. |
| Artifact/release evidence | Tools **HOST-TESTED/implemented**, baseline **STOP** | The manifest generator, vector spine, same-machine SBF rebuild, artifact audit, and supervised read-only review harness provide useful local controls. | Current manifest is stale; no SBOM/license closure, independent rebuild, signed provenance, audit, release bundle, or public-network evidence exists. |

## 4. The pooled-custody correction

The earlier SBF plane combined two incompatible models: it credited internal
cash, then charged the wallet again on Split, and paid the wallet on
Merge/Redeem while also crediting internal cash. Its runtime gates proved that
the program executed that model consistently; they did not make it economically
correct.

The coherent V1 model is pooled custody. For one market, define:

```text
C = actual collateral atoms in the Hoard Token-2022 account
L = HoardAccount.collateral_atoms (locked claim backing)
F = sum of all positions' free collateral cash
R_cash = sum of all positions' reserved collateral cash
U = unsolicited, unowned collateral surplus

C = L + F + R_cash + U        and        U >= 0
```

Reserved Eggs remain inside the separate per-outcome claim-supply identity; they
are not another collateral term. One instruction cannot enumerate `F` or
`R_cash`. It therefore checks exact local
deltas and at least `C >= L`; the full equality is an inductive market-wide
obligation with one semantic owner. Direct token donations increase only `U`.
They never create a Position claim, treasury asset, fee pool, or sweep right.

Required transitions are:

| Transition | Token effect | Accounting effect |
| --- | --- | --- |
| `Endow(q)` | actor `-q`, Hoard `+q` | owner free cash `+q` |
| `Split(q)` | none | free cash `-q`, locked backing `+q`, every primitive claim supply `+q` |
| `Merge(q)` | none | every primitive claim supply `-q`, locked backing `-q`, free cash `+q` |
| internal redemption payout `p` | none | winning internal claim `-q`, locked backing `-p`, free cash `+p` |
| reserve/release | none | exact transfer between free cash/claims and reserved cash/claims |
| `Withdraw(q)` | Hoard `-q`, actor `+q` | unreserved free cash `-q` |
| external redemption payout `p` | burn winning Egg; Hoard `-p`, holder `+p` | external liability `-q`, locked backing `-p` |

The immutable market collateral cap bounds `L`, the claim liability. It must not
silently cap unrelated free cash deposits or `C`. Every transition must preserve
exact CPI deltas, authority/profile admission, rollback, and the protected-
principal rule while moving to the pooled equation.

This design is not promoted merely because its code exists in the dirty tree.
Its gate is a joined host and real-Token-2022 campaign including unsolicited
donations, late CPI failure, multiple positions, withdrawal, reservation, and
both redemption modes.

### Load-bearing evidence anchors

- [`docs/implementation/SBF_BRINGUP.md`](docs/implementation/SBF_BRINGUP.md)
  and [`docs/implementation/LIFECYCLE_WALK.md`](docs/implementation/LIFECYCLE_WALK.md)
  contain the historical `858f408` SBF evidence and its simulation/genesis
  limits.
- [`docs/implementation/SOURCE_ADMISSION_V1.md`](docs/implementation/SOURCE_ADMISSION_V1.md)
  distinguishes the committed typed source kernel from the unauthenticated live
  path.
- [`docs/implementation/SETTLEMENT_CHECKPOINT.md`](docs/implementation/SETTLEMENT_CHECKPOINT.md)
  freezes the settlement prerequisite order and pre-resolution entitlement
  direction.
- [`docs/implementation/COMMITTED_SBF_WALK.md`](docs/implementation/COMMITTED_SBF_WALK.md)
  defines the signed committed-runner claim boundary.
- [`verus/kernel/TRANSFER_REFINEMENT.json`](verus/kernel/TRANSFER_REFINEMENT.json)
  is the machine-readable narrow Rust-verification record; the broader Lean
  model inventory is in
  [`docs/implementation/LEAN_MODEL_PLAN.md`](docs/implementation/LEAN_MODEL_PLAN.md).
- [`docs/implementation/BASELINE_MANIFEST.md`](docs/implementation/BASELINE_MANIFEST.md)
  defines schema v2 and explains why the checked-in v1 manifest is stale.

## 5. Non-negotiable STOP ledger

These are the current release blockers, not optional polish:

1. **Custody closure:** finish and independently test the pooled equation,
   Withdraw, reservation terms, authority meaning, and exact rollback.
2. **Single token truth and external exit:** survive or prohibit ordinary
   out-of-band burns, remove/redefine shadow supply authority, and implement
   possession-authorized external redemption for arbitrary holders.
3. **Permissionless state construction:** create every required PDA through
   System-program CPI from a blank bank, including later users and the
   feed/epoch/clearing plane. No program-owned genesis injection in the
   promotion walk.
4. **Source-authenticated resolution:** select and pin one concrete source
   adapter/parser/deployment profile, persist accepted history, bind Resolve to
   that history, use the Clock sysvar, and refuse source/archive substitution.
5. **Funded venue lifecycle:** reserve assets at placement, freeze the exact
   live order set, verify/select the best valid submitted candidate, freeze a
   complete entitlement set before resolution, and consume each entitlement at
   most once afterward.
6. **Replay and lifecycle closure:** split global resolution replay from owner
   replay, run a signed multi-owner committed walk, restart the process, and end
   with no stranded owned cash, claims, reservations, or principal.
7. **Evidence promotion:** rerun host, fuzz, SBF, Token-2022, stack, artifact,
   and negative gates on one clean tree; emit a clean schema-v2 baseline; then
   obtain independent rebuild/security/release evidence before any release
   claim.

Gate L0 is additional to all seven. Passing engineering gates cannot answer
legal classification or authorize a public-network act.

## 6. Deliberate V1 narrowing

The following choices keep the critical path finite without reducing the
distinctive product:

- Primitive externally composable Eggs remain exact one-hot categorical claims.
  Range, triangle, capped-linear, and finite Gaussian-like payoffs compile into
  integer portfolios over that basis. A native fractional payout vector is an
  ambiguity/failure-policy feature, not the ordinary product.
- A candidate is the **best valid submitted candidate** under the frozen
  comparison. Search/availability remains separate from relation validity.
- Lazy settlement may continue after resolution only for a complete immutable
  entitlement set frozen before resolution. Resolution creates or changes no
  receipt.
- Cross-market collateral netting, margin, leverage, liquidation, subjective
  resolution, generic matching VMs, and V1 privacy machinery remain out of
  scope.
- A static client and external venues are projections/adapters. They never own
  program, source, position, or settlement truth.

## 7. Handoff operating loop

1. Read `AGENTS.md`, `PROJECT.md`, this file, and `docs/V1_BACKLOG.md`.
2. Run `git status --short` before choosing a lane. Shared dirty-tree changes
   are not a baseline and belong to their active owners.
3. Pick the first dependency-unblocked backlog gate. State its exact falsifier
   before editing and keep one semantic owner for each persisted fact.
4. Run the narrowest host/proof/runtime test that can refute the change. Do not
   weaken a refusal or change a vector expectation to obtain green.
5. Commit coherent local work by explicit path. Do not push, tag, publish,
   deploy, sign, fund, use a public RPC, or contact a regulator without current
   authorization naming that act.
6. Only after all integration lanes settle, run the full clean-tree evidence
   ladder and regenerate the manifest. Update this file and the backlog from
   the resulting evidence, not from lane summaries.

When prose and a machine artifact disagree, record the disagreement and repair
the stale side. Neither prose nor a green gate can promote a known economic or
authority defect.
