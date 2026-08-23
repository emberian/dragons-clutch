# Dragon's Clutch

Dragon's Clutch is a greenfield Solana protocol for fully collateralized,
liquidation-free claims over bounded objective states. It represents categorical
outcomes and degree-one through degree-three B-spline outcome bases, clears
coupled portfolio orders at exact integer simplex prices, and settles from
frozen evidence without a discretionary resolver.

The repository contains a substantial local implementation. It does **not**
contain a release, deployed program, official frontend, production source
identity, or value-bearing market.

## Current status

The current bounded implementation can run this lifecycle against a local Agave
bank or loopback validator:

```text
Realm + collateral policy + Terms
              │
              ▼
create market → endow → split complete sets
              │
              ▼
place/cancel portfolio orders → freeze the canonical page set
              │
              ▼
submit candidate → streamed relation check → verified-only candidate admission
                                      │
                                      ▼
             select best valid submitted candidate fully verified by deadline
              │
              ▼
freeze entitlements → settle exact receipts and pots
              │
sealed source archive → resolve → redeem claims → withdraw collateral
              │
              └→ close safe leaf machinery → retain the epoch root
```

The newest retained end-to-end run joins the source and trading sides of that
picture. The 2026-08-23 joined-v4 campaign records 52 signed transactions on a
loopback Agave validator: the captured Pyth router and receiver verify a
deterministic 13-of-19 locally signed observation, wrong-Config and wrong-feed
append attempts roll back with the expected `0x7a`, the admitted update seals,
two funded owners trade one direct outcome pair through General V1, and the
same market resolves, redeems, and returns all 128 collateral atoms. The
campaign ran at repository HEAD `4e83648`; the strict Operator parser landed at
`633a366`, the public-safe transcript at `94f5143`, and its current documentation
at `bbfeaa5`. Read the retained
[campaign evidence](docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-23/)
or render it with Operator's `pyth-local` mode.

That is **unpromoted local SBF execution over a synthetic observation**, not a
production price, devnet or mainnet execution, deployment, live frontend,
economic-demand result, or release. It exercises one simple direct buy/sell
pair under the current zero-fee General V1 path. It does not close the broader
portfolio, virtual, mixed, inexact, fee-bearing, liveness, or terminal-shape
gaps listed below.

The last accepted Cycle-G default SBF artifact is 2,149,672 bytes with SHA-256
`0d52c561909cedef96f571ddeca3a21e621a629be778f775dd7e0a8023956cc7`.
Its checked manifest has 101 declared gates, 100 matching their expected
disposition and one documented documentation-window contradiction. A separate
Persvati run was reported as 49 passing checks across 54 derived gates and found
two seal-description errors that were corrected before the manifest was
re-emitted; unlike older jobs, no durable Cycle-G attestation report is retained
in-tree. Current post-Cycle-G source is **unsealed**. Its clean 129-file SBF
input closure at `ba580c6` passed the complete offline artifact audit: three
byte-identical default builds produced a 2,160,072-byte ELF with SHA-256
`a6381fbe211e400788615e1c588938266bed14bc8f0fc12babf76350bc24cbe2`,
with dependency/syscall and final-LTO stack checks green. These are local
engineering results, not an accepted baseline, independent security audit,
release, deployment, or formal verification of the whole program.

The second-pass runtime input closure at `169a1ba` is still **unsealed**, but it
has passed the complete offline artifact audit. Three independent builds,
including a relocated Cargo home, produced the byte-identical 2,082,320-byte ELF
`193c08723eaefeff9a1c2aa53c9e3feb58960a919fb0bbb7ca5da3bd817aa95b`;
the 129-file source closure, dependency/syscall surface, loader shape, and
final-LTO stack gates passed. The
review found that Position deletion could strand an all-in seller's live
reservation, while epoch-root deletion could both reopen the same epoch
identity and strand candidate/work accounts outside the retained top three.
Those two deletions now fail closed pending persisted reservation/child counts
and a monotone epoch generation or tombstone. Artifact reproducibility is not a
seal, release, deployment, or substitute for the complete bank and
signed-validator promotion campaign. Against those frozen sources, the
complete default production-inert bank profile (one unreachable fixture
release, no production release) passed 165 tests with zero failures using that
exact ELF. The separately compiled `non-production-mock-source`
profile passed 168 tests with zero failures using its distinct 2,110,240-byte
ELF `342fdfcb0e6b0836ec9ecd492d9a8577c87f493b49fd8c35e3cb47c448d06112`.
The mock result exercises laboratory source/value paths and is not production
source evidence.

The older `scripts/run_operator_trade.sh` demonstration remains a useful,
separate interactive bench: its 2026-08-22 run confirmed 54/54 local
transactions and closed six reported conservation identities, but it uses a
non-production mock-source ELF. It is no longer the strongest joined lifecycle
evidence.

### Implemented and bounded

| Surface | Current state |
| --- | --- |
| Eggcrate kernel | Safe, `no_std`, allocation-free, float-free fixed-layout transitions with explicit errors |
| Native claims | Categorical degree 0 and exact rational degree 1–3 open-clamped B-spline evaluation |
| General venue | Structural bounds of 16 outcomes, 64 orders in four pages, and 416 witness slices; the executed scale shapes do not cover that Cartesian maximum |
| Candidate checking | Resumable streamed relation, exact simplex prices, degree-2/3 moment-cone admission, and atomic verified-only retention before the shared deadline |
| Resolution | Frozen source/Terms bindings, authenticated receiver-write seam, canonical nonzero-confidence interval evaluation, and prepaid ResolutionWork profile |
| Local operation | Permissionless keeper, committed local-validator walks, and a human-vs-fixed-automaton Operator Bench |
| Clients | Literate static microsite, offline inspect-only Glass client, and loopback-only Operator Bench |

### Important limits

- The original product thesis is broader than Cycle G. The payoff compiler is a
  research crate, not yet a general market-creation path. Clear/Shielded/Dark do
  not share a deployed relation.
- General settlement is complete only over its admitted domain. Inexact
  portfolio consideration and some multi-order-per-owner rounding shapes refuse
  before receipts are minted.
- The accepted Cycle-G general registry ranks unverified score claims and can
  exclude or evict valid candidates. The current unsealed source repairs that
  defect and its hostile bank campaign passes, but the accepted artifact still
  selects only the best verified **retained** candidate. A successor window
  still needs separate submission and verification deadlines for a fairness
  guarantee beyond candidates fully verified before the shared deadline.
- The receipt codec's stale 128-slice cap is corrected in the current source.
  A maximum four-page/64-order, 416-slice direct Entitle now executes at 803,935
  CU, and slice index 128 creates its receipt at 763,755 CU. Maximum-page
  portfolio full-pair, virtual, and inexact variants still need equivalent
  measurements before claiming the complete settlement envelope.
- The collateral **layout** admits legacy SPL Token and Token-2022, but the
  current SBF market adapter drives Token-2022 only. The documented DREGG
  dogfood profile is legacy SPL Token and therefore cannot currently found a
  market through this adapter. DREGG has no special branch.
- Executable general/demo routes force fees to zero; production rates and a
  treasury/carry policy remain unset.
- ResolutionWork's runtime minimum deposit for 32 records is 49,431,920
  lamports at the accepted constants. The projection separates that protocol
  prefund from named-plan payouts/refunds and external keeper budget. Against
  the unsealed `a6381fbe…` ELF, Fold(4) `[6,2]` measures 514,332 CU / 1,228
  bytes and 171,765 CU / 704 bytes; its external Fold budget is 1,090,000
  lamports. This identity-bound overlay does not promote or relabel Cycle G.
- The local pull-source path authenticates a receiver-written update and can
  resolve nonzero-confidence V2 categorical intervals. The actual deployed
  Pyth receiver/router ELFs now verify a locally signed 13-of-19 guardian VAA.
  The real router first persists that Verified synthetic VAA. In a later
  transaction, the real receiver's `PostUpdate` and `AppendSourceArchiveV2`
  execute adjacently and atomically. Missing adjacency refuses with the archive
  unchanged; wrong Config or feed rolls back both the receiver-created update
  and archive. The Program/ProgramData bytes are captured deployment bytes;
  the router guardian set and receiver Config are freshly initialized local
  fixture state. This is real deployed provider-program/ABI/crypto execution
  over a synthetic local observation. The retained 2026-08-23 joined-v4 run now
  carries that seam through signed market creation, collateral endowment,
  splitting, one direct General V1 trade, candidate verification and selection,
  entitlement and settlement, source-driven resolution, redemption, and both
  withdrawals. Its append-time Clock admits the 210-second-old observation;
  the final authenticated Clock is 530 seconds old only after the fixed
  1,000-slot candidate window and consumes the already sealed archive. This is
  still not devnet price/provider-availability evidence or a production
  profile. No production row is registered, and provider, feed-profile,
  stability, and trust-floor identities remain deliberately unpinned.
- Fixed bounds are one measured capacity profile, not a claim that the concept
  is limited to those widths.
- Frozen ScoreV1 rewards risk-free complete-set wash flow and pubkey
  fragmentation; it is experimental economics, not a release-quality market
  objective. The implemented ScoreV2-Q economic prefix is
  complete-set-invariant and representation-neutral after owner-blind
  admission; later tie fields select the min-zero, lower-churn representative.
  It is not person-neutral or wash-proof: public keys do not establish
  independent controllers. It is not selected by an SBF profile while
  normalization, price-quality, and fee/reward integration gates remain open.
- Degree-2/3 V1b checks describe the continuous exact B-spline cone, not the
  integer-coordinate, largest-remainder-quantized settlement body. Under live
  payout semantics they can both refuse a coherent runtime point price and
  accept a price outside the finite runtime hull. The isolated exact checker
  now keeps continuous Bernstein witnesses separate from support-bounded
  quantized-atom witnesses; neither is selected by SBF. The first public
  coupled profile should remain degree 0/1 until one matching certificate is
  integrated and measured.
- The current 2,082,320-byte audited ELF costs 14.49529272 SOL in persistent
  loader rent. Ten SOL is insufficient by 4.49529272 SOL before fees. The
  static-deduplication wave removed 54,344 bytes; eliminating the redundant
  CreateMarket decode/re-encode removed another 23,408 bytes. Reaching less
  than ten SOL still requires another 645,876 bytes, so it needs a
  product-driven capability profile rather than only micro-optimization.

See the current [architecture review](docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md)
for the source-backed findings and successor designs, the
[state/rent audit](docs/reviews/STATE_RENT_AUDIT_2026-08-22.md) for exact
current-tree account costs and format candidates, and the
[intent archaeology](docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md)
for the requirements recovered from the Codex and Claude session history.

## Try it locally

Everything below is local and offline except the explicitly marked Pyth clone,
which performs bounded read-only devnet RPC calls. Dependency/toolchain
installation is also outside these commands.

### Read the microsite

```sh
python3 scripts/check_site.py
python3 -m http.server 9129 --bind 127.0.0.1 --directory site
# open http://127.0.0.1:9129/
```

The hand-authored site has no build step and no external asset or network
dependency. Its Pages workflow is manual-only and runs the same local reference
check before upload; a push to `main` is not publication authority.

### Check the offline Glass client

```sh
cd apps/static-client
npm test
```

Glass constructs and checks unsigned intent bytes. It has no RPC, wallet,
signer, transaction submission, or release identity. Its bundled capability
ledger is a labeled historical snapshot pending regeneration from current
truth.

### Check the Operator browser source

```sh
cd apps/operator
npm test
npm run check
```

These dependency-free checks cover JavaScript syntax, exact integer display,
and mechanical interaction/accessibility invariants. They do not replace a
real browser/DOM and visual run.

### Clone the real Pyth devnet substrate locally

This command reads canonical devnet, verifies its genesis hash, and boots a
fresh local validator with the upgraded Pyth receiver, Wormhole router,
push-oracle program, feature set, and receiver Config cloned into it:

```sh
programs/clutch-sbf/scripts/run_pyth_devnet_clone.sh
# local RPC: http://127.0.0.1:9147
```

It never reads a wallet, requests an airdrop, signs, or submits a public
transaction. It does perform public RPC reads, so run it only within an
explicitly authorized read window. The resulting validator is real-program
local infrastructure, not a Clutch deployment. The clone command alone does
not exercise an encoded VAA or Clutch; the separate offline local-real campaign
below does. See the dated
[devnet source snapshot](docs/reviews/DEVNET_REAL_SOURCE_SNAPSHOT_2026-08-22.md)
for the exact cloned identities, body digests, and remaining promotion gates.
The [current unsealed SBF snapshot](docs/reviews/CURRENT_UNSEALED_SBF_SNAPSHOT_2026-08-22.md)
records the clean input-closure ELF identity, complete offline artifact audit,
final-LTO stack check, exact local rent arithmetic, bank matrix, and keeper
restart gate; it is not a release artifact.

To run the reproducible signed-RPC provider seam without any public RPC or
faucet SOL, first prepare the pinned loopback Agave binary as documented under
`tools/agave-loopback-validator/`, then run:

```sh
CLUTCH_LOOPBACK_TEST_VALIDATOR="$PWD/.cache/agave-loopback-validator/bin/solana-test-validator" \
  programs/clutch-sbf/scripts/run_local_real_pyth.sh
```

This starts a fresh listener-audited validator, injects 36 exact genesis
accounts including reconstructed deployed Pyth loader bodies and the test-only
Clutch ProgramData account, signs every transaction with disposable local
keys, and removes the ledger and keys afterward. It never reads a default
wallet or Solana CLI config. The retained clean-HEAD campaign and exact claim
boundary are in
[the signed-RPC review](docs/reviews/LOCAL_REAL_PYTH_SIGNED_RPC_2026-08-22.md).

To repeat the current source-to-trade-to-redemption campaign, choose a new,
empty transcript directory and run:

```sh
CLUTCH_LOOPBACK_TEST_VALIDATOR="$PWD/.cache/agave-loopback-validator/bin/solana-test-validator" \
CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR=/absolute/path/to/new-empty-transcript \
  programs/clutch-sbf/scripts/run_local_joined_pyth_lifecycle.sh
```

The runner uses disposable local keys and loopback listeners, never a default
wallet or public RPC. To inspect the already retained result without replaying
or extending it:

```sh
CARGO_NET_OFFLINE=true cargo run --offline \
  --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- \
  serve --mode pyth-local \
  --transcript docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-23
# open http://127.0.0.1:9130/
```

Operator strictly parses the three public transcript files and presents them
as an untrusted projection. It does not contact RPC, load a wallet, or attest
chain state independently.

The faster in-process bank version remains available:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  --non-production-real-pyth-lab \
  real_pyth_router_verifies_then_post_update_and_clutch_append_are_atomic
```

This executes the captured deployed router and receiver programs through exact
Upgradeable Loader accounts. The router persists the pinned Verified synthetic
proof; in a later atomic transaction the receiver posts and Clutch appends it.
The resulting one-record archive seals and resolves a categorical market whose
selected cell contains the entire admitted conservative interval
`[99,980,929, 100,019,071]`. The fixture provenance and every binary/input
digest live under
`programs/clutch-sbf/svm-tests/tests/fixtures/real-pyth-local/`.

### Run the separate mock-source trading bench

```sh
CARGO_NET_OFFLINE=true scripts/run_operator_trade.sh
```

The gate builds a non-production SBF profile, starts a fresh loopback validator,
founds the eight-outcome Friday clutch, posts the same intents as the browser,
and requires the observed settlement identities to close. Expect several
minutes: it waits for a 260-slot freeze deadline and the protocol's fixed
1,000-slot candidate window on the validator's real clock.

To leave the interactive bench open instead:

```sh
CARGO_NET_OFFLINE=true cargo run --offline \
  --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- \
  serve --mode trade
# open http://127.0.0.1:9130/
```

The daemon, not the browser, holds ephemeral local test signers and builds
transactions through the canonical harness. This is test infrastructure, not a
wallet architecture.

The separate `pyth-live` mode uses the real captured local receiver/router
plane. Its daemon owns a private reusable session root and keeps the loopback
validator and signer roster alive after the fixed campaign reaches terminal
state. It also rebuilds one result-bound typed plan from public actor identities
without reading a private key, fetching a blockhash, signing, submitting, or
exporting the wire. That proves construction continuity only; the
browser remains read-only and there is not yet an admitted interactive trading
endpoint.

## Architecture in five minutes

- [`crates/clutch-kernel`](crates/clutch-kernel) — Eggcrate's collateral and
  claim state transitions.
- [`crates/clutch-bspline`](crates/clutch-bspline) — exact native degree 0–3
  basis evaluation and quantization.
- [`crates/clutch-accumulator`](crates/clutch-accumulator) and
  [`crates/clutch-bspline-accumulator`](crates/clutch-bspline-accumulator) —
  fixed-memory path summaries.
- [`crates/clutch-batch`](crates/clutch-batch) — the coupled portfolio relation
  and resumable stream state.
- [`programs/solana-layout`](programs/solana-layout) — canonical account and
  instruction byte ownership.
- [`programs/solana-reference`](programs/solana-reference) — host reference
  adapter for the transitions it models.
- [`programs/clutch-sbf/program`](programs/clutch-sbf/program) — hostile-account
  authentication, PDA and runtime checks, Token-2022/System CPI, and persistence.
- [`programs/clutch-sbf/svm-tests`](programs/clutch-sbf/svm-tests) — real-SBF
  local-bank campaigns and compute measurements.
- [`programs/clutch-sbf/keeper`](programs/clutch-sbf/keeper) — state-derived
  permissionless crank logic.
- [`apps/static-client`](apps/static-client) — offline inspect-only Glass.
- [`apps/operator`](apps/operator) and
  [`programs/clutch-sbf/operatord`](programs/clutch-sbf/operatord) — local
  operator test surface.
- [`research`](research) — models and promotion candidates, not automatically
  runtime capability.

The dependency direction is intentional: mathematical/kernel semantics do not
depend on Solana, token programs, or source SDKs. The small adapter authenticates
those boundaries and may not recreate kernel economics.

## Terms

- **Realm** — immutable collateral and admission namespace.
- **Hoard** — segregated market collateral; its principal pays claimants only.
- **Egg** — one native outcome-basis claim.
- **Clutch** — one complete exhaustive set of Eggs, worth one collateral unit.
- **Hatch** — terminal resolution and redemption.
- **Glass** — a replaceable, untrusted view of canonical onchain state.

Collateral is selected by each Realm. DREGG is a house/dogfood profile, never a
protocol-wide requirement, fee token, governance weight, or liveness source.

## Correctness and evidence

Evidence labels are deliberately nontransitive. A Lean theorem about a model
does not verify Rust; a finite differential corpus does not prove a universal
refinement; a local bank run is not devnet or mainnet evidence. Start with:

- [`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) — current implementation and evidence
  status;
- [`PROJECT.md`](PROJECT.md) — canonical product brief;
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — ownership and trust
  boundaries;
- [`docs/PROTOCOL.md`](docs/PROTOCOL.md) — accounts and transitions;
- [`docs/VERIFICATION.md`](docs/VERIFICATION.md) — proof and unverified
  boundaries;
- [`MACRO_AND_MICRO_OPTIMIZATION.md`](MACRO_AND_MICRO_OPTIMIZATION.md) — measured
  performance queue and constraints.

The protocol is not “formally verified.” Named Lean and Verus lanes check named
models or source subsets under documented assumptions; the Solana adapter,
runtime, CPIs, and whole-program refinement remain outside those claims.

## Safety and release posture

There is no signed release manifest, tag, deployment, official URL, live market,
or financial authority in this repository. Fixtures, simulations, and local
validator executions are not mainnet evidence. Static clients and indexes are
untrusted projections of canonical program state.

Security policy and threat-model entry point: [`SECURITY.md`](SECURITY.md).

## License and provenance

First-party source and documentation are licensed under
[`AGPL-3.0-or-later`](LICENSE). The project is greenfield. It must not import,
copy, or depend on JOSHI, joshibot, leanuweave, minidregg, breadstuffs, Oracle
Pit, or historical DREGG prototypes without an explicit provenance and license
review. See [`docs/PROVENANCE.md`](docs/PROVENANCE.md).
