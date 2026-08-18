# Dragon's Clutch transition handoff

Snapshot date: 2026-08-18. Transition state: **ready for a supervised offline
engineering handoff; not release-ready and not authorized for public-network
use**.

This file is the entry point for the next engineering model. Read
[`AGENTS.md`](AGENTS.md), [`PROJECT.md`](PROJECT.md), and this file before making
changes. The repository now has local baseline history on `main`; use
`git rev-parse HEAD` to identify the exact working baseline. Paths and byte
digests below identify a reviewed local snapshot, not a release provenance chain.

## 1. Claim vocabulary

Every status statement in this handoff uses exactly one of these labels:

- **IMPLEMENTED**: source exists locally and the named offline checks pass. This
  does not imply formal verification, SBF runtime evidence, security review, or
  deployment readiness.
- **MODEL**: a deterministic reference model, specification, theorem statement,
  synthetic experiment, or cost hypothesis exists. It is not consensus code or
  production evidence.
- **PROPOSED**: a design choice, parameter, policy, architecture, or backlog item
  has not crossed its stated evidence gate.
- **BLOCKER**: the named work must refuse promotion until it is closed. A
  blocker is not permission to weaken the claim or bypass the gate.

Preserve the repository's correctness language: “best valid submitted
candidate,” never “optimal clearing” without a checked optimality certificate;
“verification-target Rust,” never “formally verified” without the exact
theorem, digest, toolchain, assumptions, and unverified boundaries.

## 2. Product thesis and non-negotiable shape

**PROPOSED:** Dragon's Clutch compiles a bounded objective state space into an
exhaustive, disjoint, ordered basis of fully collateralized payoff assets. A
complete Clutch can be split from, and merged back into, one Realm's collateral
before resolution. A deterministic observation program selects a frozen payout
vector; no debt, margin, liquidation, discretionary resolver, or socialized loss
is introduced.

Its distinctive conjunction is:

1. collateral-generic immutable Realms, with DREGG as one dogfood profile rather
   than a kernel branch;
2. exact fixed-width categorical claims and bounded payoff portfolios;
3. a coupled simplex batch relation with virtual complete-set conversion;
4. protected Hoard principal and separately prepaid liveness;
5. standard Token-2022 materialization at the composability boundary; and
6. a replaceable, untrusted static client requiring no Dragon-operated service.

The canonical product description is [`PROJECT.md`](PROJECT.md). The architecture
and trust boundaries are in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). Policy
and parameter prose remains **PROPOSED** unless this handoff explicitly says
otherwise.

## 3. Architecture and semantic ownership

```text
handwritten Rocq shadow (unchecked)       Verus shadow sources (unchecked)
                  \                         /
                   canonical semantic vectors
                              |
       +----------------------+----------------------+
       |                      |                      |
  clutch-kernel       clutch-accumulator       clutch-batch
       |                      |                      |
       +----------------------+----------------------+
                              |
                   vertical reference model
                              |
             fixed Solana layouts and intent bytes
                              |
              offline single-position reference adapter
                              |
                  hostile SVM adapter (open)
                              |
              Token-2022 CPIs / SBF runtime (open)
                              |
                   untrusted static client
```

One persisted fact must have one semantic owner:

- `crates/clutch-kernel` owns claim-state transitions, payout liability,
  split/merge, materialization, resolution, and exact redemption.
- `crates/clutch-accumulator` owns source-neutral coverage and associative
  interval-summary semantics.
- `crates/clutch-batch` owns public fixed-book admission, candidate construction,
  candidate verification, allocation, conservation, and deterministic score.
- `programs/solana-layout` owns only canonical account and unsigned-intent bytes.
  It does not authenticate accounts or execute transitions.
- A future Solana adapter owns hostile account metadata validation, PDA/alias/
  signer/owner/replay checks, persistence, and narrow CPI construction. It may
  call the three semantic owners; it may not duplicate them.
- `research/vertical-model` is the composition oracle. It is not a fourth
  production implementation.
- `apps/static-client` is an untrusted projection and inspect-only UI. It owns no
  chain truth, deployment identity, signing authority, or transaction semantics.
- `research/economics` and `benchmarks` own synthetic hypotheses and falsifiers,
  never protocol constants.

Do not import or copy implementation material from JOSHI, Minidregg, Leanuweave,
Breadstuffs, Oracle Pit, or prior DREGG work. Cross-repository movement requires
the provenance process in [`docs/PROVENANCE.md`](docs/PROVENANCE.md).

## 4. Landed surface

### 4.1 Executable semantic prototypes

- **IMPLEMENTED:** `crates/clutch-kernel` is dependency-free, `no_std`, safe
  fixed-layout Rust with `MAX_OUTCOMES = 16` and `MAX_PAYOUTS = 8`. It implements
  finite payout sets, maximum-liability checks, split/merge,
  materialize/dematerialize, finite resolution, and internal/external exact
  redemption. Seven unit tests, strict Clippy, and rustdoc pass offline.
- **IMPLEMENTED:** `crates/clutch-accumulator` is a dependency-free `no_std`
  interval-summary monoid with explicit gaps, coverage, first/last/extrema,
  exact price-time integrals, TWAP, and terminal/TWAP ratio intervals. Unsupported
  threshold crossings, drawdown, and variance refuse rather than invent
  precision. Ten unit tests, strict Clippy, and rustdoc pass offline.
- **IMPLEMENTED:** `crates/clutch-batch` is a dependency-free fixed-capacity
  transparent relation (`MAX_ORDERS = 64`, `MAX_GRID_TICKS = 64`) with canonical
  order sequence, maximum-volume/minimum-imbalance/highest-tick selection,
  deterministic pro-rata allocation, explicit dust policy, candidate
  verification, and conservation checks. Verification recomputes the frozen
  canonical allocation and requires exact fill-vector equality; it rejects
  ineligible fills, pro-rata reweighting, and all-or-none bypasses. Nine unit
  tests, strict Clippy, and rustdoc pass offline.

The sibling files under `verus/kernel`, `verus/accumulator`, and `verus/batch`
are **MODEL** shadow specifications only. No Verus binary is installed and no
proof has been run.

### 4.2 Composition, layouts, and client

- **IMPLEMENTED:** `research/vertical-model` joins the three semantic crates in a
  deterministic host-only lifecycle: create, split, materialize/dematerialize,
  clear/verify/fill, observe/TWAP, resolve/refuse, merge, and redeem. It keeps
  principal, fee revenue, and prepaid liveness distinct. A cumulative
  per-candidate/per-order settlement
  ledger prevents aggregate overfill and makes candidate replay idempotent. The
  ledger binds a typed Market/book/Epoch/policy/order-set domain, full candidate,
  paired canonical buy/sell order identities, side, owner, and outcome; the
  accepted settlement path moves exact clearing-price cash consideration
  opposite the claim leg and consumes both fill allowances. The legacy claim-
  only path and unbound books refuse. All public model mutations use
  clone/apply/conservation/commit staging, so a failed final invariant leaves
  claims, cash, ledgers, trace, and accounting unchanged. Protected accounting
  mutators also use copy/validate/commit staging;
  resolution also requires a frozen three-bucket maturity horizon and explicit
  observation seal. Seven tests and a byte-stable golden trace pass.
- **IMPLEMENTED:** `programs/solana-layout` is a standalone dependency-free
  `no_std` codec prototype. It has strict fixed layouts for Realm, Profile,
  Market, Hoard, Position, Feed, and a 16-record OrderPage; domain-separated
  SHA-256 identities; stored bumps; canonical padding; and fixed unsigned intent
  bytes. Nine codec/adversarial tests, strict Clippy, and rustdoc pass. This is
  not an entrypoint, CPI adapter, RPC client, or deployable ELF.
- **IMPLEMENTED:** `programs/solana-reference` is a `no_std`, safe,
  allocator-free offline transition adapter over the layout and kernel crates.
  It validates hostile metadata supplied to the model, exact replay sequence,
  account links, initial emptiness, split, materialize/dematerialize, and account
  encoding. `Resolve` and `RedeemInternal` unconditionally return
  `ResolutionEvidenceUnavailable`; neither a signer nor forged coherent resolved
  bytes can bypass the missing evidence plane. It requires exact single-position
  closure `internal + external == aggregate supply` before and after each
  transition; multi-position execution refuses. Ten tests, strict Clippy,
  rustdoc, and formatting pass. Its external balance account is an explicit
  model placeholder. There is no resolver signer, and both `Resolve` and
  `RedeemInternal` refuse. A future resolution path must bind a mature, sealed
  `WindowResult`, authenticated feed/source, generation, immutable terms, and
  terms-to-payout mapping before those refusals can be relaxed. It has no Solana SDK,
  `AccountInfo`, PDA derivation, SBF
  entrypoint, CPI, token program, RPC, signing, or deployment behavior.
- **IMPLEMENTED:** `apps/static-client` is plain HTML/CSS/JavaScript with no
  runtime dependency, wallet, RPC, signer, submit path, analytics, or active
  chain capability. It renders local terms and constructs only an inspectable
  JSON intent. Smoke and syntax checks pass. No in-browser visual, responsive,
  keyboard, screen-reader, or CSP-header QA has been performed.
- **MODEL:** `rocq/ClutchKernel.v` defines a Rust-independent pure transition
  model and names five properties. Those properties are definitions of `Prop`,
  not proved theorems. Rocq/Coq is unavailable on the current host.

### 4.3 Laboratories

- **MODEL:** `research/economics` contains 28 passing standard-library property
  tests over solvency, protected pools, liveness booking, fee carry/allocation,
  shared-feed capitalization, failure incentives, and price collapse. The
  current run explores 409 solvency states and 1,338 transitions and rejects 91
  forbidden pool debits. Parameters, including `kappa = 1/250`, are hypotheses.
- **MODEL:** `benchmarks` contains 193 (2026-08-19: now 261, incl. the landed-ABI arm) deterministic synthetic wire/account/CPI/
  rent/accumulator/batch scenarios and 12 passing tests. These are analytical
  layout hypotheses and pinned external constants, not SBF compute measurements.
- **IMPLEMENTED:** the tiny E0 toolchain probe builds identical source bytes on
  host Rust 1.89.0 and Anza SBF 4.0.0, with reproducible SBF `rlib` output and a
  prohibited-source scan. It emits no program ELF and runs no program-test.
- **MODEL:** `research/collateral-profiles` freezes a 266-byte generic Realm
  collateral-policy encoding, domain-separated identity, separate collateral/
  fee/native-SOL-liveness currencies, legacy SPL Token support, and a
  conservative Token-2022 extension matrix. Nineteen adversarial tests pass.
  Only account-level `ImmutableOwner` is admitted; every mint extension refuses.
  DREGG uses the generic dogfood constructor. The current six-decimal/supply
  example is synthetic: token program, decimals, authorities, extensions, and
  supply are unauthenticated, and no DREGG Realm is frozen.
- **IMPLEMENTED:** every current first-party Cargo manifest and the static-client
  package manifest declares `AGPL-3.0-or-later`; the root README uses
  “verification-target,” not “verified,” and nested Cargo `target/` directories
  are ignored. This is a local metadata consistency result, not a completed
  dependency, copyright, notice, or public-release audit.

### 4.4 Adversarial disposition

[`docs/implementation/ADVERSARIAL_REVIEW_V0.md`](docs/implementation/ADVERSARIAL_REVIEW_V0.md)
is the current counterexample inventory. Its governing disposition is **STOP for
integration, release, formal-verification, SVM, and chain-readiness claims**.
The review found and drove repairs for ineligible batch fills, forged canonical
allocation, cumulative settlement overfill, pre-maturity resolution, unaccounted
materialization, license/headline drift, and nested target hygiene. Subsequent
repair also changed the Solana reference adapter to refuse resolution and
redemption unconditionally. The final review reproduced every historical P0 as
closed in the deliberately bounded host subsets. This is the stopping point for
model transfer, not an integration PASS: P1/P2 joins below remain STOPs.

## 5. Verification commands

Run from the repository root. Keep Cargo offline and use independent manifests;
there is intentionally no authoritative root workspace yet.

```sh
for manifest in \
  crates/clutch-kernel/Cargo.toml \
  crates/clutch-accumulator/Cargo.toml \
  crates/clutch-batch/Cargo.toml \
  programs/solana-layout/Cargo.toml \
  programs/solana-reference/Cargo.toml \
  research/vertical-model/Cargo.toml
do
  cargo test --manifest-path "$manifest" --offline --locked
  cargo clippy --manifest-path "$manifest" --offline --locked \
    --all-targets -- -D warnings
done

cargo doc --manifest-path crates/clutch-kernel/Cargo.toml --offline --locked --no-deps
cargo doc --manifest-path crates/clutch-accumulator/Cargo.toml --offline --locked --no-deps
cargo doc --manifest-path crates/clutch-batch/Cargo.toml --offline --locked --no-deps
cargo doc --manifest-path programs/solana-layout/Cargo.toml --offline --locked --no-deps
cargo doc --manifest-path programs/solana-reference/Cargo.toml --offline --locked --no-deps

cargo run --quiet --manifest-path research/vertical-model/Cargo.toml \
  --offline --locked | cmp - research/vertical-model/golden/basic.trace
python3 -m unittest discover -s research/economics -p 'test_*.py' -v
python3 -m unittest discover -s research/collateral-profiles -p 'test_*.py' -v
python3 research/collateral-profiles/run_lab.py
python3 benchmarks/cost_lab.py check
(cd benchmarks/golden && shasum -a 256 -c checksums.sha256)
(cd apps/static-client && npm test && npm run check)
CARGO_NET_OFFLINE=true toolchain/scripts/run_lab.sh
```

Expected unavailable gates:

```sh
toolchain/scripts/run_verus.sh  # BLOCKED until a reviewed exact Verus pin exists
rocq/check.sh                   # exits 2 while rocq/coqc is unavailable
```

The commands above passed on 2026-08-18 except for those two explicitly
unavailable proof tools. They do not exercise an SBF entrypoint, program-test,
Token-2022, RPC, signing, or a public network.

## 6. Current byte identities

These digests are reproducibility aids for the reviewed local baseline, not
release attestations:

- static canonical terms:
  `a21f6cbb1ab3b06afc7c8625f3388835843edb17c48173e8fb57df8b7e0dd8e8`
  (superseded 2026-08-18: terms fixture now matches kernel refuse-on-remainder
  semantics, digest `62b06b2107636686648507e4f9ecd8a4d90733dcebf81177d4a63b25bc698d02`;
  MANIFEST.baseline.json is the living digest record);
- E0 probe source:
  `10b2087683d3c2cb423768eb9c612c00ea929b171835c15d3d16792d6b8b19ac`;
- reproducible E0 SBF `rlib`:
  `d444c0ac118de1cb24d9fe6b509df7beafc1c0f1a8c2828b24e26b170da0ad1c`.
- vertical-model golden trace:
  `ab808dd308e3bdce0fa8cc2d3b9b4a14e87dbd1b41ae7143e897c53f7f3f1639`;
- collateral-profile vectors:
  `5bcf3a6117c4e411a5b9b339093eaf3dcd9ca1eee0bb7a2b6814a42f46639e48`.

The benchmark golden checksums live in
[`benchmarks/golden/checksums.sha256`](benchmarks/golden/checksums.sha256).
Before publication, bind every source, lock, generated fixture, proof result,
ELF, and static bundle to a clean immutable revision and a release manifest.

## 7. Blockers and stop gates

### P0: close before calling the repository handoff/release stable

1. **BLOCKER - no release manifest.** A local Git baseline now supports review
   diffs and source identity, but no remote, signed tag, release artifact, or
   checked source/build manifest exists. (Update 2026-08-19: a private remote
   `emberian/dragons-clutch` exists and is pushed per explicit user direction,
   and `scripts/baseline_manifest.py` generates a checked baseline manifest; a
   signed tag, release artifact, and publication remain user-gated.) Keep subsequent local work in coherent
   commits. Pushing, tagging, publishing, or declaring a release requires
   explicit user direction; do not infer it from this handoff.
2. **BLOCKER - formal-tool gap.** Verus and Rocq are unavailable. The existing
   Rust is tested, not formally verified; the Rocq properties are unproved. Pin
   reviewed tools before expanding proof claims. Never vendor or install a tool
   silently.
3. **BLOCKER - SVM/runtime gap.** The offline reference adapter is not an
   `AccountInfo`/PDA/token adapter and its closed single-position equality is not
   a multi-position aggregate witness. There is no accepted native SBF
   entrypoint, Token-2022 CPI path, program-test lifecycle, runtime atomic-
   rollback evidence, CU/stack/heap/ELF measurement, or cross-runtime vector
   closure.
4. **BLOCKER - resolution and profile joins.** The reference adapter now safely
   refuses resolution and redemption. It may not enable them until payout is
   derived from the exact mature, sealed WindowResult and bound source/feed/
   generation/terms. The canonical collateral-policy digest is also not joined
   to the Realm/Profile layout and enforced by the adapter. Do not call either
   seam implemented, non-discretionary, or policy-frozen.
5. **BLOCKER - policy freeze.** Failure payout, ambiguity, fee/revenue policy,
   dust/remainder, source admission, exact Realm token profiles, and portfolio
   intent language remain proposals. Convenience code may not canonize them.
6. **BLOCKER - Gate L0.** No public-network deployment, real-fund test,
   solicitation, author-affiliated operation, or claim of legal availability is
   authorized. [`docs/ENGINEERING_PLAN.md`](docs/ENGINEERING_PLAN.md) defines the
   human/legal closure conditions.

### P1: next bounded engineering packets

Assign one semantic owner and nonoverlapping paths per packet.

The confirmed redesign queue is:

- fractional payout claims can enter quantities that are individually
  unredeemable; choose one-hot-only, enforced redemption lots, or persistent
  remainder ownership before admitting them;
- the landed batch crate is a scalar call-auction falsifier, not the documented
  coupled simplex/portfolio/virtual-complete-set relation; it strips BoundOrder
  owner/outcome semantics, so it can report `matched=1` and charge liveness for
  a buy/sell pair whose outcome mismatch forces settlement to refuse;
- paired settlement is currently one-shot even when a receipt consumes less
  than both fills, which can strand residual fill; freeze full-pair-only,
  cumulative remaining quantity, or unique match-slice receipts;
- persisted layouts cannot reconstruct full kernel/protocol state and lack
  SupplyLedger, immutable payout/window policy, Epoch/final-pot/receipt closure,
  cross-page closure, and a frozen `limit` to `limit_tick` mapping;
- bare accumulator statistics do not themselves bind complete coverage,
  expected range, source, generation, or Window policy;
- the economics lab and executable kernel admit different payout sets, while the
  vertical fee model does not debit a payer or implement the proposed
  dispersion/carry/allocation policy;
- cost goldens remain a separate hypothesis arm and do not match the landed
  Position, order-page, and order-count ABI;
- the Python collateral-policy digest and Rust Profile digest have no frozen
  parent/subprofile relation or cross-language enforcement;
- the static client duplicates manifest/terms data in JavaScript and has no
  checked equality gate; meta CSP cannot promise header-only protections; and
- Verus shadows contain vacuous placeholders, the Rocq transition obligation has
  an output-shape defect, and no Rust/Rocq/codec/adapter refinement exists.

These are not permission to solve several facts in one catch-all crate. Use the
ownership map above and promote only with language-neutral vectors.

1. **Toolchain/proof owner:** pin Verus, Rocq, solver, Rust, and Anza revisions;
   prove the tiny common-source probe; record assumptions and source/tool
   digests; make a proceed/redesign decision before broad proof work.
2. **Semantic-vector owner:** freeze one versioned error taxonomy and canonical
   vector manifest joining kernel, accumulator, batch, Rocq, Verus, and adapter
   outputs. Do not create a root workspace until this schema and dependency
   direction are reviewed.
3. **SVM-adapter owner:** lower the reviewed offline reference boundary into
   hostile `AccountInfo`/PDA validation and kernel-issued state/CPI intents. Keep
   the layout codec and semantic crates unchanged; solve multi-position aggregate
   closure without scanning an unbounded set, and add alias, owner, signer, PDA,
   replay, rollback, extension, and malicious-token cases under program-test.
4. **Resolution/profile-join owner:** freeze the typed `WindowResult` and
   terms-to-payout derivation, bind its feed/source/generation/maturity/seal to
   the immutable Market, and bind the canonical collateral-policy digest into
   Realm/Profile bytes. Remove the modeled resolver discretion; do not add an
   oracle shortcut.
5. **Formal-shadow owner:** prove the named Rocq reachability, solvency, supply,
   exact-redemption, protected-pool, accumulator, and batch properties. Keep the
   Rust correspondence manual and explicit until a refinement exists.
6. **Mechanism owner:** independently falsify failure vectors, equal-fallback
   sabotage, fee carry/fragmentation, dust, self-cross, candidate withholding,
   and scoring on exhaustive tiny books before freezing policies.
7. **Release/client owner:** keep Glass inspect-only until program bytes, layouts,
   schema, cluster, source revision, and CSP/SBOM/license evidence are bound by a
   checked manifest. Add real browser visual/responsive/accessibility QA and
   serve-time CSP tests. Wallet/RPC/sign/submit is a separate reviewed trust-
   boundary project.

## 8. Regulatory and human-only gates

Engineering may prepare factual architecture and evidence. It may not file,
contact regulators, retain counsel, choose the user's identity/affiliation,
deploy, announce an official venue, or decide the operator/legal-person facts.
Those are human decisions.

The sibling Dark Egg Research packet records two joint CFTC/SEC comment deadlines
on 2026-08-24 and one CFTC IAC written-statement deadline on 2026-08-27. Those
drafts are unfiled. A deadline is not authorization. Dragon's Clutch must keep
its exact product/entity/users/control/collateral/source/fee/affiliate/client/
upgrade/deployment facts synchronized with any later reviewed filing.

## 9. Recommended first Claude session

Do one evidence-and-boundary closure session before adding features:

1. read `AGENTS.md`, this file, `PROJECT.md`, `docs/ARCHITECTURE.md`,
   `docs/EVIDENCE_MATRIX.md`,
   `docs/implementation/ADVERSARIAL_REVIEW_V0.md`, and every other file under
   `docs/implementation/`;
2. run the offline verification commands above and record exact failures without
   weakening refusals;
3. audit all first-party manifests and top-level status prose for license and
   “verified/optimal/deployed” drift;
4. inspect the latest active-lane outputs against semantic ownership and reject
   duplicated transition logic;
5. propose a minimal immutable baseline/release-evidence manifest, but do not
   commit, push, install tools, use RPC, or deploy without explicit authorization;
6. return a short proceed/redesign/refuse memo selecting exactly one P1 packet.

The recommended selection is the bound executable-pairing refinement: make
clearing prove that owner/outcome bindings admit a complete settlement pairing,
then freeze residual-pair semantics. Do not expand toward SVM until that host
relation and its refusal vectors are coherent.

The best next move is not maximum code volume. It is making the current
prototype surface reproducible, provenance-bound, and impossible to overclaim.

## 10. Authority boundary

Default work is offline. Never read keys, wallets, browser sessions, or private
configuration. Never sign, submit, deploy, create a market, transfer or buy a
token, fund an account, mutate a remote host, contact a regulator, publish a
filing, push a branch, or describe a URL/program as official without explicit
current authorization naming the act. Public RPC reads also require an explicit
bounded task. Preserve user-owned changes and do not convert this handoff into
authority it does not grant.
