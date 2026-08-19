# Overnight integration red-team

Date: 2026-08-19
Reviewed commits: `15c29a1`, `9966af2`, `ef8a3d4`, `86b72f8`,
`ce8c55c`, `33a1d41`, `1758ae2`, `4ea7c16`, `48b669b`, `ead7106`,
`b89d57a`, and `f70cf09` (plus the shared integration metadata now above
them).
Review mode: adversarial, local, read-only except for this report. No deploy,
RPC, wallet, fund, or external-system action occurred.

## Verdict

No hidden P0 value-loss regression was found in the reviewed transitions.
Artifact sealing/refund, funded reservation/release, market-global resolution
replay, pooled-cash withdrawal, and exact clamped B-spline evaluation all have
substantial fail-closed structure and focused executable evidence.

There is one concrete cross-cutting **P1 availability defect**: predictable
PDAs can be permanently denied creation by transferring a small number of
lamports to them before the protocol calls ordinary System `CreateAccount`.
This directly affects the new artifact stage/final and reservation accounts,
and it also affects older market/owner/genesis creation paths. Fix it before
calling account creation permissionless or mainnet-ready.

There are also three important **honest STOPs**, not hidden regressions:

1. the live observation/resolution path is not source-authenticated and its
   evidence buffer is not permissionlessly constructible;
2. the version-three native resolution record is an isolated codec, while live
   SBF remains version-two finite-preset only; and
3. `SettlePage` remains unreachable, so no epoch-freeze path may strand live
   reservations before settlement or a complete lapse/refund transition lands.

The distinction matters: a STOP that refuses is safe research scaffolding. A
STOP silently bypassed to make a demo run would be a deployment P0.

## Severity convention

- **P0**: an accepting value-bearing path can select a false payout, create an
  unbacked claim, steal principal, or irreversibly corrupt consensus state.
- **P1**: cheap targeted denial of a required protocol action, permanent fund
  stranding, or a correctness boundary that must block release.
- **STOP**: a named absent integration whose production path refuses or is
  unreachable. A STOP can describe P0 consequences *if bypassed* without being
  an active P0 in the current program.
- **Observation**: non-blocking semantic, evidence, or documentation debt.

## Findings

### F-01 — P1: one-lamport PDA squatting denies every ordinary `CreateAccount`

**Counterexample.** Let `P` be any predictable, currently absent protocol PDA.
An attacker transfers one lamport to `P`, leaving it System-owned with empty
data. The protocol then invokes System `CreateAccount` for `P`. Ordinary
`CreateAccount` requires zero current lamports and returns
`AccountAlreadyInUse`; only the protocol can sign allocation/assignment for the
off-curve PDA, so neither the intended user nor a janitor can remove the
prefund under the current instruction set.

This is not speculative Solana folklore. The pinned
`solana-system-interface 3.2.0` source says that `create_account` requires zero
lamports and explicitly calls predictable-address prefunding a DoS security
issue (`instruction.rs:17-20`, `322-341`). The pinned System processor rejects
the occupied destination before allocation (`system_processor.rs:165-173`).

**Reached gates.**

- `genesis::require_creatable` checks writable, non-executable, empty data, and
  System ownership, but does not require zero lamports
  (`programs/clutch-sbf/program/src/instructions/genesis.rs:304-310`). It then
  emits ordinary `CreateAccount` tag zero in `create_pda_account`
  (`:336-364`). A prefunded target therefore passes the local preflight and
  fails in the CPI.
- Artifact `BeginArtifact` uses that helper for its uploader-keyed stage
  (`artifact.rs:180-206`), and `SealArtifact` uses it for a content-derived
  Policy/Grid/Terms PDA (`artifact.rs:412-444`). Both addresses are predictable
  from public binding fields.
- `PlaceOrder` requires a creatable canonical reservation PDA and uses the same
  helper (`orders_batch.rs:945-1000`). Its address is predictable from market,
  epoch, owner, position generation, and positional order id.
- The older market/owner construction helper is even more explicit:
  `require_absent_target` rejects any nonzero lamport balance
  (`construction.rs:131-140`), and its module comment deliberately treats a
  prefund as squatting (`:18-23`). This gives a clear refusal class but does not
  restore liveness.

**Impact.** An attacker can cheaply block a specific artifact, uploader stage,
order reservation, market, or owner plane. Content-derived finals and canonical
market state are especially attractive targets because the intended address is
known before the creation transaction. This is availability loss, not principal
theft, hence P1 rather than P0.

**Required gate.** Replace the shared creation primitive with a reviewed
prefund-safe sequence. Two plausible forms are (a) payer transfer of only the
rent shortfall followed by PDA-signed `Allocate` and `Assign`, or (b)
`CreateAccountAllowPrefund` only after the exact target cluster feature is
pinned and exercised. Validate System ownership, empty data, the exact PDA and
canonical bump before claiming; overfunding must remain a donation with no
economic authority. Add real-SVM cases that prefund every creation family by
one lamport and by more than the rent floor, then require successful creation,
correct owner/size, no debit above the rent shortfall, and rollback on a later
semantic refusal.

### F-02 — deployment P0 if bypassed; current honest STOP: resolution source substitution

The current accumulator proves only that supplied records form a valid fold.
It does not prove that the records came from the source named by Terms. More
strongly, `Resolve` folds a caller-provided evidence buffer unrelated to the
records previously passed to `FeedAdvance`. A caller can choose a well-formed
window and the matching requested payout.

This is already described exactly and correctly in
`docs/implementation/SOURCE_ADMISSION_V1.md:14-52`, including the A-versus-B
substitution at `:28-42`. The live code also says that the window identity is a
caller-chosen label (`observe_resolve.rs:97-120`) and that `FeedAdvance.evidence`
is “recorded, not believed” (`:1851-1857`).

It is currently an honest STOP rather than an exploitable accepting mainnet
path for two reasons: source authentication is explicitly non-qualifying, and
the hostile evidence buffer has no canonical creation path
(`observe_resolve.rs:197-200`). If a generic buffer uploader were added without
the source/archive join, this would immediately become a P0 false-settlement
path.

**Required gate.** Keep value-bearing deployment blocked until Resolve consumes
one canonical source-authenticated archive/window artifact and proves its
identity from bytes. The gate must reject the documented A-versus-B
substitution, bind source program/deployment generation, freshness, unique
record selection, grid, cursor, repair generation, and exact window digest.

### F-03 — honest STOP: native resolution persistence is deliberately not live

`NativeResolutionAccount` has a coherent 319-byte version-three shape: exact
mode discriminants, canonical unresolved/preset padding, derived vector sum and
padding checks, market/terms/feed/window joins, and a single persisted vector
owner. However, it is intentionally isolated
(`native_resolution.rs:1-5`; `NATIVE_RESOLUTION_PERSISTENCE.md:3-10`). The live
layout still exports the version-two finite-preset Resolution ABI.

The live resolution path refuses every degree-one-through-three market before
derivation (`observe_resolve.rs:1203-1211`). This is the correct behavior. The
codec alone does **not** prove that `resolved_value` evaluates to the stored
vector; its type comment assigns that obligation to the future resolution
instruction (`native_resolution.rs:43-50`). `effective_vector` intentionally
returns the stored immutable vector rather than recomputing it.

**Required gate.** The eight-step integration list in
`NATIVE_RESOLUTION_PERSISTENCE.md:188-207` must land atomically. In particular,
creation must recompute point-to-vector equality through `clutch-bspline`,
every redemption path must reconstruct its ephemeral effective vector from
Terms plus Resolution, and no live path may fall back to
`KernelAccount.resolved_payout` in derived mode. Until a rebuilt SBF ELF passes
rollback, replay, one-hot endpoint, fractional redemption, and source
substitution cases, “native B-spline settlement on Solana” remains a STOP.

### F-04 — honest STOP: reservations have no production settlement/lapse owner

The new order reservation path is internally exact:

- buy cash is `ceil(quantity * limit / price_scale) + max_fee`, while sells move
  the exact claim vector into the reservation;
- reservation identity binds market, epoch, owner, position generation, and
  order id, while bytes additionally bind Terms, grid, policy, page, family,
  side, fee cap, and remaining assets;
- cancellation recomputes the plan from the live page slot and releases only
  an active same-generation reservation once; and
- Position cash withdrawal and Split both honor the reserved subset.

The page digest is not trusted as a stale header. `verify_page` structurally
walks all slots, recomputes the full digest, and compares it last
(`stream.rs:593-659`); every append/tombstone reseals the post-state
(`stream.rs:906-914`). The native SBF hashv path assembles the same preimage and
still decodes every slot (`stream.rs:554-584`).

But `SettlePage` still calls `settlement::refuse_unintegrated()` directly
(`orders_batch.rs:872-876`). Reservations have no authenticated complete-set
commitment and no production `ENTITLED`/`CONSUMED` transition. This is safe only
while no reachable transition freezes an epoch/page with live reservations.
Current cancellation requires both epoch OPEN and page unfrozen
(`orders_batch.rs:748-760`; `reservation.rs:138-160`).

**Required gate.** Do not enable epoch freeze independently. Freeze must prove
a complete reservation-set join and install either complete immutable
settlement entitlements or a permissionless deadline lapse that refunds every
active reservation. Add a conservation fold over all reservations, not merely
the page slots. Released reservation rent is also currently retained forever;
that is self-funded state bloat, not a solvency bug, but the lifecycle should
decide whether and to whom empty-account rent closes.

### F-05 — no defect found: artifact typing, sealing, rent destination, and atomic refusal

The artifact lane's important gates survived review and real-bank tests:

- there is no generic blob kind; Policy, Grid, and Terms have exact lengths and
  owning hostile-byte codecs (`artifact.rs` layout lines `48-110`, `348-383`);
- stage identity includes funder, kind, context, digest, and canonical PDA bump;
- writes are left-to-right, exact-size, and canonical-zero padded; duplicate,
  overlap, gap, mixed binding, and post-completion writes refuse before mutation
  (`artifact.rs` layout `309-345`);
- seal recomputes the content-derived final PDA, fully validates the body,
  requires the encoded grid/terms bump, and admits an existing final only when
  its exact bytes agree (`artifact.rs` program `400-448`);
- stage rent returns only to the header's recorded funder, including public
  post-expiry reap (`artifact.rs` program `453-490`); and
- final creation/copy/stage close occur in one Solana transaction, so any late
  borrow/CPI failure rolls all writes and lamport changes back.

The remaining blocker is F-01, not an artifact content or refund confusion.
The static clock-key check also relies on the runtime's reserved sysvar address;
that is appropriate on chain, while hostile genesis fixtures should continue to
cover owner/data substitutions.

### F-06 — no defect found: pooled-cash withdrawal preserves custody and reservations

`WithdrawCash` binds actor, request owner, market, canonical Position/Replay,
Profile/policy, collateral mint, Hoard state/authority/token PDA, and an
actor-owned destination. It computes `free_cash = cash - reserved_cash`, debits
only total cash, advances owner replay, executes a PDA-signed exact Token-2022
transfer, verifies both token deltas, and requires the post-Hoard balance still
covers `Hoard.collateral_atoms` (`cash_exit.rs:100-146`, `218-335`).

The Hoard PDA comparison now uses the Hoard account's stored bump and separately
requires `Market.hoard_bump` to equal the canonical derivation
(`cash_exit.rs:245-276`). This closes the reviewed bump substitution. Exact
destination credit, hostile mint/account extension admission, donations,
reserved-cash refusal, and multi-position isolation have bank coverage. A
zero-amount withdrawal can consume the caller's own replay sequence; that is an
odd but owner-authorized no-op, not a third-party loss.

### F-07 — no defect found: market-global resolution replay is state-owned

Removing Position and owner Replay from `Resolve` is semantically correct:
resolution is one market fact. The canonical Resolution account is the replay
owner. First resolution requires unresolved Market/kernel/record state; an exact
retry re-folds evidence and returns all bytes unchanged; a conflicting retry
refuses (`observe_resolve.rs:1606-1756`). `Request::sequence` is bound to both
Terms and sealed-window repair generation, not incremented as a wallet nonce
(`:1693-1702`).

The exact-retry test also advances the feed cursor and supplied recording slot,
then proves the first cursor/slot remain immutable
(`solana-reference/src/lib.rs:4094-4192`). Late external-supply synchronization
occurs only on first resolution and remains transaction-atomic
(`observe_resolve.rs:2060-2103`). The blocker is F-02 source provenance, not the
new replay domain.

### F-08 — no arithmetic counterexample found: clamped splines and largest remainder

The Rust evaluator validates every hostile `BasisSpec`, expands distinct knots
to the actual open-clamped sequence, handles the closed top explicitly, uses
the right pane at internal knots, evaluates exact rational degree-two/three
BasisFuns, and quantizes by floor plus deterministic largest remainder. The
result validates exact sum `D`, zero padding, per-weight bounds, and support at
most `degree + 1` (`crates/clutch-bspline/src/lib.rs:105-217`, `220-417`,
`453-489`).

Focused and oracle checks found no endpoint, span, partition-unity, or tie-order
divergence. Exact ties intentionally favor the lower outcome index. That creates
a transparent at-most-one-atom directional bias at coarse denominators; it is
not a correctness defect, but denominator selection should bound its economic
effect and UI/spec text must not claim reflection symmetry at tied points.

At reviewed commit `f70cf09`, Lean proved substantial exact basis, endpoint,
quantization-certificate, and solvency facts but still named the stored-knot/Rust
linkage and constructive largest-remainder selection as adapter residues rather
than importing axioms. Those are honest proof-boundary gaps, not permission to
say the Rust evaluator is formally verified. A concurrent follow-up was present
in the worktree during this review and is outside this commit-scoped verdict.

## Executed evidence

All commands were offline/local. The source worktree was concurrently active,
so the exact commit list above, not a claim of pristine-HEAD reproduction, is
the review scope.

- `cargo test --manifest-path crates/clutch-bspline/Cargo.toml`: 12/12 passed.
- `python3 crates/clutch-bspline/oracle/check.py`: 31,814 exact differential
  cases passed, fixed seed `880230`, six mutants killed. Mean L1 error of
  largest remainder was lower than either directional residual policy for
  degrees one through three.
- `cargo test --manifest-path programs/solana-layout/Cargo.toml
  native_resolution`: 6/6 passed.
- focused artifact layout tests: 5/5 passed.
- portable/buffered/native page-digest equality tests passed.
- exact market-global resolution idempotency test passed.
- cash-withdrawal pure tests: 4/4 passed.
- reservation pure tests: 6/6 passed.
- real-ProgramTest artifact suite: 4/4 passed, including restart, stale digest
  rollback, idempotent final, public reap, and exact funder refund.
- real-ProgramTest reservation suite: 2/2 passed; observed SBF consumption was
  about 595,624/593,654 CU for buy/sell placement and 470,342/475,015 CU for
  cancellation.

The ProgramTest suites loaded
`programs/clutch-sbf/svm-tests/tests/fixtures/clutch_sbf.so`, SHA-256
`7e81ed90fee58135a798978bb52409a3a41d3750f64538d27c10a40d5fa77bae`.
That is runtime evidence for the checked-in fixture, not a source-attested
rebuild of the concurrently changing HEAD. The final integration gate must
rebuild the ELF from a clean source digest, copy/hash the fixture through the
repository's audited process, and rerun these tests plus the committed walk.

## Release-blocking checklist

1. Fix and real-SVM-test prefund-safe PDA creation across **every** account
   family (F-01).
2. Keep all value-bearing resolution disabled until source/archive/window
   authentication makes the A-versus-B substitution impossible (F-02).
3. Land the version-three Resolution ABI, native evaluator, kernel
   reconstruction, and every redemption consumer as one coherent cut, or keep
   degree one through three refusing (F-03).
4. Land reservation-set closure plus settlement or permissionless lapse before
   enabling epoch freeze (F-04).
5. Rebuild and attest the exact SBF ELF from a clean source digest, then rerun
   artifact, reservation, withdrawal, global-replay, hostile-source, native
   endpoint, rollback, and committed terminal-walk cases.
