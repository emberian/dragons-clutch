# Dragon's Clutch: current truth and control plane

Status date: 2026-08-19. This is the operational entry point for engineering
handoffs. [`PROJECT.md`](PROJECT.md) is the canonical product brief,
[`AGENTS.md`](AGENTS.md) is the authority and correctness policy, and
[`docs/V1_BACKLOG.md`](docs/V1_BACKLOG.md) is the dependency-ordered queue.

This file supersedes current-status, test-count, and next-work claims in
[`GOAL.md`](GOAL.md), [`CODEX_HANDOFF.md`](CODEX_HANDOFF.md),
[`CLAUDE_HANDOFF.md`](CLAUDE_HANDOFF.md), and dated drift reviews. Historical
documents remain useful evidence pointers; they are not a live promotion
ledger.

## 1. Claim vocabulary

These labels are deliberately nontransitive:

| Label | Exact meaning |
| --- | --- |
| **PROVED-MODEL** | A named proof assistant checked a theorem about a named mathematical model. It says nothing about Rust, accounts, CPI, SBF, or runtime unless a separate refinement closes that boundary. |
| **CHECKED-RUST-SUBSET** | A pinned verifier checked a named Rust source subset under recorded assumptions. It says nothing about code outside that subset. |
| **CHECKED-FINITE** | Digest-bound model-computed fixtures agree with production execution on a named finite corpus and named source mutants go red. This is not a universal refinement theorem. |
| **HOST-TESTED** | Ordinary host execution, differential testing, or a bounded finite campaign passed. This is executable evidence, not proof or SBF evidence. |
| **SBF-EXECUTED** | A compiled SBF program executed in a local Agave bank or loopback validator. This is not deployment, public-cluster, audit, or mainnet evidence. |
| **PROFILE-ADMITTED** | An exact measured route clears one selected finite compute/rent/reward policy. This does not extrapolate to unmeasured shapes or imply inclusion, keeper participation, system terminality, or a global liveness policy. |
| **MODEL-ONLY** | A design, reference model, research tool, or cost experiment exists but is not the production transition. |
| **PROPOSED** | A design or policy has not crossed its promotion gate. |
| **IN-FLIGHT** | Shared dirty-worktree bytes are not an accepted baseline. |
| **STOP** | The named surface must refuse or remain undescribed as complete until its acceptance gate closes. |

“Implemented” is too broad for this control plane. Every promoted claim below
names its evidence plane. Passing a differential proves agreement on that
corpus; it does not prove that both implementations express the intended
economics.

## 2. Snapshot boundary

The accepted local evidence ancestry is the 2026-08-20 cycle-E chain:
liveness seal `934bdd6` over runtime ancestry `d77d670`, manifest `cb94c27`.
The current sealed default ELF is **1,979,512 bytes with SHA-256
`4fded7a67a2d8994f4dc2b82c533b978d14d6107f28de7cbbe7674ecdcedf6cb`** — the
TerminalClosure runtime, intents 36–67. Its audit is archived under
[`research/liveness-policy-profile/artifacts/4fded7a67a2d8994`](research/liveness-policy-profile/artifacts/4fded7a67a2d8994/audit/RUNTIME_ARTIFACT_AUDIT.md):
pass 1 = pass 2 byte-identical from the canonical checkout, zero first-party
frame diagnostics surviving final LTO, all 60,135 direct `r10` references at
or below 4,096 bytes, a reviewed ten-symbol import surface (`sol_memmove_`
admitted at `2dbc9fc` after the nine-symbol pin refused), and the declared
source closure at 109 files. Seven predecessor seals are retained in-tree as
historical evidence, each with its own chain (`e8ba31d5…`, `d6929549…`,
`fda59705…`, `187d5ee1…`, `af6bb79c…`, `bd20711b…`, `a5725a3d…`); current CU
rows are always remeasured against the current artifact, never relabeled.
This is exact local artifact/stack/bank evidence, not a release, deployment,
production source-provider, inclusion, audit, or formal-verification claim.

Runtime evidence is artifact-specific:

- the historical 20-step signed committed walk at `882204f` executed ELF
  `98cac8a1e48f629f15d0efbf6295b2c96df5296f6acf6cec28ca76491da4b391`;
- the focused withdrawal bank campaign executed ELF
  `23139487e1a38de73a7f0077fb87cc28a1f1968a9dc8db0e2f5babcd09ebce41`;
- typed artifact and funded-reservation campaigns executed their then-current
  real SBF ELFs in local `solana-program-test` banks; and
- a clean-source 22-step signed, sequential, genesis-assisted local-validator
  walk at source HEAD `c05fe84` executed the joined global-resolution and
  terminal-withdrawal path against ELF
  `70c33c1cd44b475745b0562a79d9107f1d2101cbf698ebd6c233ca167ebab2e6`.
  All 22 transactions reached confirmed commitment, including two expected
  refusals; 18 watched accounts were reloaded, both owners' Position cash and
  the pooled Hoard token balance ended at zero, and a corrupted step-22 Hoard
  expectation made the gate fail specifically on committed bytes. The walk
  still had 11 genesis prerequisites and is focused **SBF-EXECUTED** evidence,
  not a blank-bank venue or the complete schema-v2 baseline. The exact record
  is [`docs/implementation/COMMITTED_SBF_WALK.md`](docs/implementation/COMMITTED_SBF_WALK.md).

The checked-in `MANIFEST.baseline.json` is schema v2, re-emitted for the
2026-08-20 TerminalClosure runtime (`4fded7a6…`, 1,979,512 bytes, liveness
seal `934bdd6`, cycle E) with all 100 declared gates executed in the
emission run, and `check --run-gates` passes after the manifest-only
commit (`cb94c27`). Persvati independently attested the current identity
on 2026-08-20: **45/45 portable gates PASS, 0 STOP** over exact `cb94c27`
— the `4fded7a6…` ELF byte-verified in seven contexts, all seven
historical roots verified as the exact retained set, and a new
contract-driven gate (`build_path_mechanism_crossref`) verifying the
seal's relocation-mechanism attribution itself: 88 mangled-symbol sites
identical across pass 1, pass 2, and the relocated-Cargo-home build, and
different at all 88 in the cross-path build — the recorded
`PATH_TIED_SYMBOL_ORDER` and `PATH_SENSITIVE` dispositions checked, not
narrated. Durable job:
`persvati:/home/ember/jobs/dragons-clutch-portable-attest-cb94c27-20260820-xiVNcu`.
The predecessor attestation: Persvati attested `788581c` (the `e8ba31d5…`
Tier-2-complete seal) with **44/44 portable gates PASS, 0 STOP** — archive and
bundle digests byte-identical on both hosts, the `e8ba31d5…` ELF
byte-verified in six contexts (never built, loaded, or executed there), all
six historical roots verified as the exact retained set, the manifest
digest-only check green in pristine bundle checkouts on both hosts, and the
liveness profile's policy gates plus 53 tests green on the second host under
the pinned toolchain applied fail-closed from the start. Durable job:
`persvati:/home/ember/jobs/dragons-clutch-portable-attest-788581c-20260820-FE7g0W`.
Same-day predecessors, each fully sealed and attested and now historical:
`187d5ee1…` (syscall reseal, 41/41 at `98fb070`), `fda59705…` (Tier-2 wave 1,
43/43 at `6827749`), `d6929549…` (the walk, 44/44 at `7e6066f`). Build-path
truth: the identity is same-path-reproducible; the canonical build lives at
the canonical checkout (docs/reviews/BUILD_PATH_IDENTITY_2026-08-20.md).
The paragraphs below record the R1 attestation and rebuild lineage of the
earlier `bd20711b…` identity and are historical; no independent *rebuild*
of the current identity has been run. Persvati independently attested exact `6743b9d` from a
fresh archive (SHA-256 `f9f25afce1a00f277ad1322787bfc1f757cac26535558d9491fb731e543bf277`)
and minimal hashed Git bundle: 40/40 portable gates PASS, 0 STOP, 528 files
checked twice with zero mismatches, every test count identical to the prior
`b5da74f` attestation, the sealed `bd20711b…` ELF byte-verified on both hosts,
and the digest-only manifest check green in the pristine bundle checkout. The
durable job is
`/home/ember/jobs/dragons-clutch-final-portable-attest-6743b9d-20260819-TChWnu`.
A persvati toolchain drift was refused fail-closed and every cargo gate reran
under the exact pinned prior compiler; the newer research crates
(`terminal-lifecycle-v2`, `tools/*`) remain outside the attested dependency
scope. This is byte-and-reference verification plus portable re-execution of
host gates — no SBF build, execution, or runtime claim — and it is a checked
local evidence baseline, not a checked release manifest.

Hbox independently rebuilt the then-current default ELF from the exact
`6743b9d` archive
under the exact pinned toolchain (`cargo-build-sbf 4.0.0`, platform-tools
v1.53, offline with all 30 locked registry crates checksum-verified): two
fresh builds are byte-identical at
`5e840bb0ca887349e79de79c12d70725602116d4edb553ca16c6f914f3e1b56b`
(1,228,184 bytes). That digest is NOT byte-identical to the sealed
`bd20711b…` (1,228,192 bytes); the divergence is exhaustively classified as
Anza's per-OS platform-tools artifact — two Rust-stdlib CI path strings
(macOS vs Linux runner prefixes), a reordered prebuilt compiler-builtins
intrinsics cluster, and the resulting −8-byte address shift — with zero
bytes derived from source, dependencies, or either build host's paths. That
comparison is historical: it predates the Direct V3 merge, the `af6bb79c…`
reseal, and the 2026-08-19 `187d5ee1…` syscall reseal, and no equivalent
independent rebuild of the current identity has been run.
Cross-OS byte-identity is structurally impossible for this pin; byte-level
reproduction of the seal needs a second macOS host. The durable job is
`/tank/joshibot/dragons-clutch-sbf-rebuild-6743b9d-dd4727` (REBUILD_REPORT.md
plus full comparison evidence). This is an independent same-source rebuild
with classified divergence, not a release, deployment, or audit claim.

No signed tag, macOS byte-level seal reproduction,
official client URL, or value-bearing market
exists. On 2026-08-19 ember explicitly authorized public TESTNET/devnet
deployment with fresh throwaway keys and bounded public-RPC use, superseding
the earlier local-only boundary for that exact scope; the authorized frame is
Track C of docs/DEPLOYMENT_REVENUE_BOUNDARY.md (author-affiliated research
deployment: no real value, exact build identity, measured operation, and no
claim that devnet answers any legal question). Mainnet, real value, customer
anything, the production source-registry flip, filings, and official-claim
language remain outside this authorization. Deployment facts, when they
exist, are recorded in their own dated record and never promote local
evidence. Nothing here authorizes real-value signing, funding, publication,
regulator contact, or an “official” claim.

The scoped adversarial review at `f48b13c` found no hidden active P0 in the
reviewed artifact, reservation, replay, withdrawal, or spline transitions. It
did find the cross-cutting predictable-PDA pre-funding P1 and reaffirmed the
source, native-resolution, and settlement STOPs. See
[`docs/reviews/OVERNIGHT_INTEGRATION_REDTEAM.md`](docs/reviews/OVERNIGHT_INTEGRATION_REDTEAM.md).
Artifact stage/final creation closed its part of that P1 at `e7d975b`; the
typed policy/Realm/Profile/market plane closed its part at `ceac012`/`7cf7150`;
and `a274bef` exercised Market, Reservation, and second-owner Position/Replay
prefunds in six real-bank cases. Over-rent donations remain unowned; hostile
owner/data/executable targets refuse; duplicate and late Token-2022 failure
roll back byte-exactly. The known existing creation families have therefore
closed F-01, but every future constructor must preserve the same gate. This is
a review/test result over named commits, not an audit or proof of absence.

## 3. Native claim semantics

Three constructions must remain separate:

1. **Native basis.** Degree zero is an exhaustive, disjoint, ordered
   categorical partition. Degrees one through three are open-clamped B-spline
   Eggs with overlapping local support, nonnegative weights, and exact
   partition of unity.
2. **Exact coefficient algebra.** A payoff in the selected finite spline span
   is represented exactly by coefficients over native Eggs. A vector is not
   automatically an approximation.
3. **Categorical compatibility lowering.** Sampling or integrating a shaped
   payoff over one-hot degree-zero Eggs is an adapter. It must carry an error
   statement when it is not exact and must never redefine the smooth product.

The exact point evaluator in `crates/clutch-bspline` is safe, `no_std`,
allocation-free, float-free Rust for degrees zero through three. It uses exact
rational basis evaluation and deterministic largest-remainder quantization
with lowest-index ties. Host tests and an independent Python `Fraction`/
Cox-de-Boor campaign support it: **HOST-TESTED**, not proved Rust.

The committed Lean file `DragonsClutch.BSpline` at `8c929a9` contains 159
counted declarations, including 116 theorems, with no `sorry`, `admit`, axiom,
`unsafe`, `native_decide`, or `implemented_by`; its reported theorem axioms are
only Lean's `propext`, `Classical.choice`, and `Quot.sound`. It checks
degree-one through degree-three clamped rational constructions, uniform
stored-knot expansion and pane/internal-boundary linkage, exact BasisFuns split
distances, constructive and unique canonical largest-remainder selection,
integer admissibility, local support, residual bounds, solvency, and
complete-set results: **PROVED-MODEL**. Rust parser/control-flow/Fraction/
selection-loop equivalence remains an explicit refinement boundary, and the
linkage theorem does not cover arbitrary nonuniform degree-one grids.

At `be8eba3`, eight Lean-computed fixtures agree byte-for-byte with the
digest-pinned production `BasisSpec::evaluate`, and five mutations of the
actual Rust source compile, execute, and go red. The campaign also reruns the
34,766-case Python differential. This is **CHECKED-FINITE** evidence. It invokes
no Verus theorem and does not prove the whole evaluator, parser/refusal order,
overflow behavior, compiler, SBF, or runtime.

The surrounding layers have different status:

- `research/bspline-shape-compiler` is a host-tested exact-rational research
  compiler for exact-in-span shapes and certified approximations, including
  ranges/tails, tents, capped call/put spreads, and Gaussian proximity:
  **MODEL-ONLY / HOST-TESTED**;
- `research/bspline-window-semantics` compares point, interval, TWAP, and two
  occupation meanings and records why an arbitrary midpoint is invalid:
  **MODEL-ONLY / HOST-TESTED**;
- `crates/clutch-bspline-accumulator` is the pure fixed-width occupation
  monoid over quantized native basis points, with explicit gaps and exact or
  separately named largest-remainder finalization: **HOST-TESTED**, not source
  authenticated or integrated; and
- `research/fractional-redemption` is a safe fixed-width exact policy model:
  the resolved common lot is `lcm_i D/gcd(D,w_i)` (for example
  `[16,40,8]/64` has common lot 8), while persistent numerator credits preserve
  `D*C >= remaining_weighted_liability + aggregate_credit`. It proves that a
  terminal aggregate numerator remainder cannot be swept without subsidy,
  forfeiture, or a finer unit: **MODEL-ONLY / HOST-TESTED**; and
- `programs/solana-reference` derives native vectors for degrees one through
  three under its conservative evidence rules: **HOST-TESTED reference**.

The 319-byte version-three native Resolution codec is selected by smooth Terms
while degree zero retains the 165-byte version-two preset ABI. At `cae3d90`
atop the source join in `0b96a3a`, degree-one through degree-three **point**
Resolve persists the sole native vector; exact retry rederives it; exact
internal and positionless bearer redemption reconstruct it ephemerally. Bearer
authority is Token-2022 possession and signature, and success burns the exact
lot, transfers the exact payout, and updates mint supply, aggregate liability,
recorded collateral, and token balances once. Nondivisible lots return
`RemainderRequired` before mutation; late transfer failure rolls back the burn.

Resolve now derives and authenticates canonical SourceSpec and sealed
SourceArchive PDAs from immutable identities, verifies their owner, bump,
spec/provider/parser/deployment generation, grid/window, lineage, page
commitment, and sealed cursor, and requires every legacy projection value to
equal the archive. Thus the legacy blob remains transport but no longer value
authority. The joined native real-SBF campaign passed 7/7. For degrees one,
two, and three, resolve/retry/internal/bearer CU were respectively
`1092607/938965/708253/788032`, `1130866/977224/705753/785332`, and
`1166139/1012497/705428/784537`. The independent post-join audit at
`ae2e155` reran 135/135 host tests and those 7/7 cases against focused ELF
`e448f1a9a5fe7c80b2d8ece939dab059ef64ccadab11fa5952328cd31ed35a32`.
That digest is evidence for this focused campaign, not a clean release manifest.

This is not general native settlement yet. Non-point evidence refuses. Source
accounts are still genesis-injected in the focused bank: production has no
onchain create/append/seal route, immediate provider receiver-post/CPI/config
authentication, or live Clock/feed admission, and the deterministic adapter is
a mock. The one-window archive is capped at 32 records. Other post-resolution
consumers still need an explicit audit and the active Kernel mode P1 below
remains. The exact sealed ELF closes its first-party final-LTO stack diagnostic
gate, but that does not close these semantic, source, or terminal gates. No path
may lower a smooth market to categorical portfolios.

The selected R2 Pyth pull profile also has an integrated research-only model:
`SourceSpecV2` is a distinct-domain, 368-byte proposed body that binds the
receiver Config key and full-byte digest, provider feed id, exact ProgramData
key/deployment slot, zero-origin grid, and only closing-boundary
`CROSSING_V1` rule id 2. Its start-aware archive cursor and atomic
receiver/loader/Instructions/Clock join are model checks. No post-cutover
receiver/config identity, production registry entry, runtime codec/parser, or
SBF route exists; official loader, Instructions-sysvar, and Clock projections
cannot be caller assertions. The default registry remains empty and the sealed
default ELF remains on `0x79`.

The v4 lifecycle audit found a P1 active-mode representation gap — the
mode-less persisted Kernel was reconstructed as `FinitePreset` on Split-family
seams that receive neither Terms nor Resolution — and the repair landed at
`3a81b38`, which is an ancestor of current runtime source `83e124d` and is
therefore inside the sealed `bd207...16b60` ELF. KernelAccount v2 (1,255
bytes) persists an immutable `basis_mode` byte derived only from fully
validated Terms at creation; hostile mode bytes and every v1 account refuse;
every Terms-receiving seam cross-checks degree against the stored mode; the
Split family reconstructs the stored mode only after requiring Active; and
mode-flip, wrong-Terms, derived-active-solvency, and resolved-native
phase-refusal/rollback tests exist in-tree
(`docs/reviews/NATIVE_SEMANTICS_AUDIT_V4.md`, REPAIRED P1 / PASS). The honest
residues are per-degree blank-bank joined lifecycle evidence and the absent
refinement theorem joining Terms bytes to evaluator, v3 Resolution bytes, and
payout. The same audit repaired
public `derive_payout` to be degree-zero-only; smooth callers must use
`derive_payout_vector` and can no longer cross a preset-membership bridge.

## 4. Capability matrix

| Surface | Strongest honest status | Established fact | Boundary / STOP |
| --- | --- | --- | --- |
| Product and Realm model | **PROPOSED** | The product is collateral-generic; DREGG is one optional dogfood profile. Native degree-zero through degree-three claim semantics are the intended ceiling. | No real Realm profile is authenticated, frozen, or released. The V1 admission allowlist is FROZEN as built (Token-2022 base mints, extension ceiling zero, ImmutableOwner required on the Hoard, unknown discriminants fail closed — docs/decisions/ADOPTED_2026-08-20.md item 4), which authenticates no Realm; the DREGG dogfood mint has no executable V1 profile. |
| Core claim kernel | **HOST-TESTED** plus separate **PROVED-MODEL** results | Safe fixed-layout Rust executes split, merge, materialize, dematerialize, resolution, and redemption fragments. Lean checks named model properties. | Lean/Rust correspondence is manual; the full kernel is not verifier-checked. |
| Verus evidence | narrow **CHECKED-RUST-SUBSET** plus separate **PROVED-MODEL** | Pinned Verus checked exact debit/credit conservation and overflow refusal for `prepare_internal_transfer`. The scalar batch shadow reports 28 verified obligations and five required red mutants; it proves one-shot dust-choice positivity/progress in its mathematical projection, allocation decomposition/per-fill bounds, unique tick selection, a whole-fill partition conditional on accepted side equalities, and a zero-suffix fold identity. | The scalar shadow does not verify the executable dust loop or its `left`/`assigned` invariants, accepted side equality, production zero-padding validation, checked-arithmetic/source correspondence, coupled V1, or any account/SBF behavior. ADR-0005 (adopted 2026-08-20, docs/decisions/ADOPTED_2026-08-20.md item 2) makes Lean the proof substrate of record and retains Verus solely for checked-Rust-subset results verifying actual executable bodies; it changes no evidence identity in this row. |
| B-spline model/executable bridge | **CHECKED-FINITE** | Eight Lean-computed vectors match digest-bound production evaluator outputs; five actual-source semantic mutants compile/execute and disagree. | No Verus invocation or universal Rust/SBF refinement; finite adapter association remains reviewed. |
| General accumulator | **HOST-TESTED** | Source-neutral adjacent summaries, coverage, interval, TWAP, and terminal calculations have bounded tests. | It authenticates no source, clock, archive, or deployment generation. |
| Native spline stack | point Resolve/exits **SBF-EXECUTED** and measured-profile admitted; broader mixed, see §3 | Degree-selected v2/v3 creation, source-joined exact d1–3 point resolution, sole-vector persistence/replay, and exact-lot internal and bearer redemption execute. The corrected `161f530` fixture passes 15/15 against the sealed default ELF; point-v3 initial Resolve samples clear the selected 25%-headroom policy. | Kernel v2 immutable mode binding is repaired and host-tested inside the sealed runtime; per-degree blank-bank joined lifecycle evidence landed at `896a1cc` (mock-ELF funded segment with four named injections; default ELF asserts the `0x79` boundary). Production source ingestion, other consumer audit, non-point semantics, and a total fragment policy remain open. Monolithic occupation-v4 initial Resolve does not clear that policy; the routed staged lane below is the admitted alternative. |
| Coupled batch relation | **HOST-TESTED** plus a narrow scalar **PROVED-MODEL** shadow | Exact witness checks, bounded candidate comparison, pairing, conservation, and a bounded streaming verifier have finite/adversarial campaigns. The separate Verus shadow proves only the named scalar model statements above. | It supports “best valid submitted candidate,” not globally optimal search. The Verus shadow excludes the coupled relation, streaming verifier, production loops, accounts, and SBF. |
| Funded order admission | **SBF-EXECUTED** focused path | `PlaceOrder` creates a canonical pre-fund-safe per-order reservation and encumbers exact cash or internal Eggs. `CancelOrder` tombstones once and releases only that reservation. Split and Withdraw cannot spend reserved cash. | No frozen reservation-set commitment, permissionless lapse, or general candidate-to-entitlement transition exists. |
| Pooled custody and cash exit | exits **SBF-EXECUTED**; default value admission **STOP** | Endow is the sole inbound token boundary; Split/Merge/internal redemption are pooled-accounting reclassifications; exact unreserved `WithdrawCash` performs Hoard-to-owner Token-2022 transfer. Since `cfea8e8`, default Endow authenticates Terms/SourceSpec then refuses `SourceReleaseUnavailable` (`0x79`) before owner allocation or Token-2022 CPI because the production registry is empty. | Mock-source Endow success requires a distinct `non-production-mock-source` ELF. No production source release or value-bearing market is admitted by the sealed default artifact. Full venue settlement remains open. |
| Outcome-token truth and bearer exit | categorical and exact native lots **SBF-EXECUTED** | Actual Token-2022 mint supply is authoritative; ordinary burns are recognized as forfeiture; transferred positionless degree-zero Eggs and exact-lot d1–3 Eggs redeem through `RedeemExternal` in focused local-bank evidence. | Nondivisible native fragments refuse. The total lot/credit policy and full lifecycle remain open. Outcome mints have no `MintCloseAuthority`; bearer-burn forfeiture and fractional fragments have no selected terminal disposition. |
| Typed artifact transport | **SBF-EXECUTED** | Policy, grid, and Terms use exact typed lengths, ordered 192-byte chunks, restart, seal, abort/reap, idempotent reseal, and rent return. At `e7d975b`, exact rent-shortfall transfer plus PDA-signed allocate/assign closed one-lamport and over-rent stage/final squatting in six real-SBF cases. Native SHA preserves the portable preimage relation. | The transport is not generic and does not authenticate source/archive/clearing artifacts. Excess target lamports remain an unowned donation, not protocol authority. |
| Account construction | initial market plane **SBF-EXECUTED**; wider lifecycle **STOP** | From a bank with no injected Clutch account, a wallet seals policy/grid/Terms, creates Realm/Profile, and creates all initial market state/token PDAs. Degree-zero v2/165 used 916,052 CU and degree-one v3/319 used 909,302 CU. Existing Market/Reservation/later-owner families have real-bank pre-fund and rollback coverage; one narrow Candidate/feed constructor exists. | Terms does not consensus-check referenced Grid existence. Feed/archive/general Epoch/candidate/pot/receipt construction remains incomplete; every new constructor must inherit the pre-fund-safe pattern and tests. |
| Resolution replay | focused **SBF-EXECUTED** | Market-global resolution no longer consumes an owner's replay sequence; exact retry is idempotent and conflicting retry refuses. The subsequent owner redemptions/withdrawal retain their own sequence. The current native Resolve also authenticates the sealed archive described below. | Replay separation alone does not establish production source ingestion or a joined blank-bank lifecycle. |
| Source admission and archive | Resolve join **SBF-EXECUTED**; production/value admission **STOP** | A 292-byte content-addressed SourceSpec and 2,560-byte one-window archive bind provider/parser/deployment/spec/grid/window/lineage and a sealed receipt. Resolve derives and authenticates their canonical PDAs and requires the compatibility projection to equal the sealed archive. Separately, the R2 Pyth pull model specifies a 368-byte distinct-domain SourceSpecV2 and closing `CROSSING_V1` id 2 authentication contract. The default source-release registry is empty and Endow fails closed with `0x79`. | The Pyth cut is model-only: post-cutover identities remain unfrozen, and no registry entry, runtime codec, official loader/Instructions parser, Clock decoder, SBF route, or source account create/append/seal path exists. Focused Resolve evidence injects mock source state; successful Endow requires the distinct non-production mock ELF. |
| ResolutionWork V1 | measured route **SBF-EXECUTED / PROFILE-ADMITTED** | Routed Begin/Fold/Finalize/Abort at account tag 22 and intents 32–35 operate on sealed archive bytes, preserve monolithic-v4 output equality, segregate prefunds/donations, and close/refund Work and Reserve. Same-ELF CU maxima include Begin 810,992, Fold(4) 815,573, Finalize 1,094,832, and Abort 587,197; all measured rows clear the selected 25%-headroom profile. | Admission is only for the measured route and selected zero-charge policy. That zero is now frozen policy rather than a placeholder: all five ResolutionWork charges are permanently zero and no vault is built (docs/decisions/ADOPTED_2026-08-20.md item 6) — the weak form, since a V2 cost schedule may reintroduce charges for new Works under its own digest. It is not system release, deployment, production source, extrapolated shape, terminal-closure, inclusion, or no-stranding evidence. The live archive has no authenticated gap-record encoding. |
| Onchain clearing/settlement | Direct V3 staged lifecycle routed and **SBF-EXECUTED** on a branch campaign, merged at `fb72b34`; profile admission **STOP** | The complete staged lifecycle — InitEpoch, InitOrderPage, Place, Freeze, Abort, Submit/admit with full re-verification, staged Verify, Finalize/Select, Settle with exact Position-transfer legs, and three Lapse phases — routes for intent tags 36–46 through one dispatcher arm with an exhaustive handler match and no fallback. On the syscall-hashed runtime every measured row clears the 1,400,000-CU ceiling with wide margin: worst row FreezeDirectEpochV4 at 383,909 CU (72.6% headroom), candidate replacement 203,128. Both SVM profiles pass (78 default, 85 mock). Legacy and V3 decoders refuse each other in both directions. | The V3 campaign is one bank profile: five candidates on an 11-tick grid, so 64-tick, exact-tie, and reordered-retained-account behavior stay model plus host evidence. V3 is resident in the sealed ELF but **UNPROMOTED in the liveness profile**: it carries no measured CU, rent/refund/close, or terminal-admission row, `live_v3` is false, and the Direct STOPs in that profile remain V2's; the V3 syscall-era sealed measurement campaign is commissioned (docs/decisions/ADOPTED_2026-08-20.md item 10) and V3 stays unpromoted until its rows seal. V3 is an epoch-atomic two-order book by design — no per-order cancellation exists; a placed order's only pre-Freeze exit is the permissionless `AbortUnfrozenDirectV4`, so placement-to-submission-open is a committed window. Direct V2 selection now completes and commits at 226,071 CU (the former exact-1,400,000 exhaustion was the software hasher, not the algorithm — see docs/reviews/COMPUTE_CEILING_REATTRIBUTION_2026-08-19.md); V2 remains STOP in the liveness profile for its unimplemented empty-frozen lapse and stays superseded by V3's lifecycle. No fee shape, no partial fills, and no universal no-stranding claim follows. **Separately, the GENERAL clearing plane (Tier 2, 2026-08-20) is complete end to end in bank evidence**: a multi-page book with portfolio orders and tombstones places through the general arm (tags 47–59), freezes, walks to VERIFIED via the streaming relation with the on-chain verdict byte-equal to the host relation's, selects among retained candidates by re-derived full-width tie digests, entitles per-slice receipts, and settles — including a portfolio full pair through the out-param settlement kernels — with exact whole-plane conservation asserted (cash, per-outcome positions, and the release identity, final Positions byte-equal to the verified summary's implied allocation). All of it is **SBF-EXECUTED (bank) and UNPROMOTED**: sealed in the liveness profile as `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY` with `decision_owner: ember`, fees forced zero at every gate, PartialFillLedger/VirtualPot/TerminalClosure standing as recorded blockers, and the general-plane policy (`GENERAL_CLEARING_POLICY_V1`) plus `CANDIDATE_WINDOW_SLOTS = 1,000` **FROZEN as pinned 2026-08-20** (docs/decisions/ADOPTED_2026-08-20.md item 1; the in-source doc comments still read PROPOSED and ride the next reseal-bearing wave). The freeze promotes nothing on its own: the walk plane is adopted to advance to **rung W1 only** — CU/quote rows, no live flags (item 10) — and those rows are now derived. **Rung W1 is executed**: the liveness profile derives a selected compute limit and keeper reward for 25 general-clearing routes across the four measured families by the same 25%-headroom/10,000-CU-quantum arithmetic every promoted family uses, all 25 PASS, worst `FreezeEpoch` 3 pages/40 orders at 717,825 CU (limit 900,000, reward 1,010,000 lamports) — 64% of the 1,120,000 raw-CU admission boundary. W1 is quotes and nothing else, welded in `require_walk_plane_w1_quotes`: the families keep `UNPROMOTED_SBF_EXECUTED_EVIDENCE_ONLY`, `general_clearing_walk.status` stays `SBF_EXECUTED_EVIDENCE_UNPROMOTED_STOP`, `live_flags` stays `UNTOUCHED` (any walk family acquiring a live flag refuses), no keeper program consumes the quotes and no path/lifecycle total is published, the rent side is NOT quoted (all eight general-plane rows keep their cycle-E STOPs), and tags 60–67 get no row at all because the `terminal_closure` suite labels no per-route CU. Full admission (W2) stays blocked on `RENT.ACCOUNT_REFUND_UNOWNED`, `GENERAL.ABANDONED_RESERVATION_HOLDS_ROOT`, and `PROFILE.STORAGE_INVENTORY_INCOMPLETE` plus five named evidence gaps. Portfolio orders are no longer structurally unclearable; the old orders_batch.rs:888 refusal survives only on the Direct V4 branch where it is correct for its two-order profile. |
| Signed committed walk | 22-step **SBF-EXECUTED** at `c05fe84` | Fresh local keys signed 22 confirmed sequential transactions through global resolution, internal/bearer redemption, and both owners withdrawing all free cash; 18 watched accounts were reloaded and the corrupted terminal expectation went red. | It is genesis-assisted by 11 prerequisites and omits clearing/settlement. It is not a blank-bank lifecycle or release baseline. |
| Static Glass | **HOST-TESTED** inspect-only prototype | A static client can render local terms and unsigned intent material without owning truth. | No frozen release manifest, complete wallet path, browser/accessibility audit, or official hosted instance. |
| Liveness accounting | one routed path **SBF-EXECUTED / PROFILE-ADMITTED**; system policy **STOP** | `clutch-liveness` remains a host-tested pure kernel. Separately, the current sealed liveness profile binds exact ResolutionWork compute, rent, rewards, refund, donation, and close behavior to the `187d5ee1...16bd` ELF; mixed historical/current measurement identities are machine-refused. | No complete global `LivenessPolicy` is emitted. Direct selection, production source work, most rent ownership/close routes, terminal asset disposition, and inclusion/keeper assumptions remain open. Hoard principal and future fees are never liveness capital. |
| Terminal lifecycle V2 | internal-only **MODEL-ONLY / HOST-TESTED** | A hostile-prestate model enforces per-role rent identity, once-only refunds, a separately retained replay tombstone, internal claim/supply/mint equality, exact per-Position lots, ordered close dependencies, and an immutable neutral surplus sink. External bearer issuance fails closed in this profile. | No live account ABI, signer/PDA authority, rent funding, Token-2022 CPI/post-state, SBF route, legacy migration, external-bearer terminal path, or fractional credit/carry closure exists. It is not a protocol terminality or no-stranding result. |
| Terminal economics R4 | **MODEL-ONLY / HOST-TESTED** | A bounded creation-only model distinguishes internal `I`, registered external `E`, and authoritative Token-2022 `A` supply planes, requires `E=A` after authenticated transitions, and retains a segregated CreditVault/CreditRoot and every nonzero claimant credit. It proves why arbitrary raw bearer units plus indivisible collateral cannot generally end in tombstone-only closure. | No live ABI/PDA/authority, persistent supply ledger, Token-2022 post-CPI authentication, CPI/close router, rent/keeper funding, migration, or SBF terminal walk exists. Legacy mints without creation-time close authority remain permanent infrastructure or STOPs. The R4 design is RATIFIED 2026-08-20 (docs/decisions/ADOPTED_2026-08-20.md item 7) with a scope amendment: legacy-rows-permanent holds ONLY for legacy mints and prototype instances, the live general plane is explicitly NOT declared permanent, and the §8 reference-ownership variant is EXPLICITLY DEFERRED. Ratifying a MODEL-ONLY design builds no ABI and promotes no surface. |
| Economics and fees | **MODEL-ONLY / PROPOSED** | Synthetic solvency, cost, fee, manipulation, and allocation experiments exist. `EvidenceOnlyRecoveryV1` is a decided new-research policy: it rejects every numeric data-failure fallback, enters recoverable dormancy after finite independently prepaid repair, selects lot-scaled bearer units for new native markets, and permits only terminal collateral burn after authoritative zero. | No failure-policy ABI, source-specific evidence theorem, lot-scaled mint, repair route, or terminal burn executes. The fee base *shape* is selected — the additive composite `kappa*G(a,p) + kappa'*R(a)` (docs/decisions/ADOPTED_2026-08-20.md item 9) — but **both rates remain undecided**, the bounds freeze is still owed, and every consensus byte stays `FeeBaseV1::None`; the selection is reversible until a rate freezes. The V1 split vector 60/0/40 with `AllRestingMakers` is adopted (item 8) and constrains nothing until a fee-bearing Realm exists; the treasury pubkey is deferred to the first such Realm and reserved to ember. Existing raw-unit bearer mints and fractional credits remain terminal STOPs. Hoard principal is never available. |
| Artifact/release evidence | exact current artifact/stack/bank seal plus checked schema-v2 local baseline; release **STOP** | Runtime source/test ancestry `d8c5034` produced three byte-identical builds of `187d5ee1...16bd` (pass 1, pass 2, and — for the first time — the relocated-Cargo-home probe; single-host evidence, not cross-host closure); final-LTO/stack audit and same-ELF bank campaigns are sealed at `cfba5bb`. The `bd20711b…` and `af6bb79c…` seals are historical only. The checked manifest records all 100 gates executed and passes its post-commit full check. | Cargo-home relocation became byte-identical on this seal — the path sensitivity every prior seal measured left with the software `sha2` crate — but that is single-host evidence only. The hbox rebuild comparison (per-OS platform-tools divergence, exhaustively classified) is historical to `6743b9d`; byte-level seal reproduction still needs a second macOS host, and no independent rebuild of the current identity exists. No complete release SBOM/license closure, external security review, signed tag, or deployment. |

## 5. Accounting truth

For one market let:

```text
H = actual collateral atoms in the Hoard Token-2022 account
L = HoardAccount.collateral_atoms, retained claim backing
P = sum of every Position.cash_atoms
R = sum of every Position.reserved_cash_atoms, with 0 <= R <= P
S = unsolicited unowned Hoard surplus

H = L + P + S
```

Reserved cash is a subset of Position cash, not an additional custody term.
Reserved Eggs remain in the claim-supply identity. Direct token donations
increase only `S`; claim burns may reduce required liability while leaving `L`
conservatively retained. Neither creates a fee, treasury asset, sweep right, or
Position credit.

| Transition | Token effect | Accounting effect |
| --- | --- | --- |
| `Endow(q)` | actor `-q`, Hoard `+q` | owner cash `+q` |
| `Split(q)` | none | free cash `-q`, locked backing `+q`, every native claim supply `+q` |
| `Merge(q)` | none | every native claim supply `-q`, locked backing `-q`, free cash `+q` |
| internal redemption | none | claim `-q`, locked backing `-p`, owner cash `+p` |
| reserve/release | none | exact movement between free and reserved ownership phases |
| `WithdrawCash(q)` | Hoard `-q`, owner `+q` | unreserved owner cash `-q` |
| external redemption | burn Egg; Hoard `-p`, bearer `+p` | external liability `-q`, locked backing `-p` |

The market collateral cap bounds locked claim backing, not unrelated free cash
or unsolicited surplus. Local instructions enforce exact deltas and at least
`H >= L`; the full equality is an inductive market-wide obligation.
The Endow row is the authenticated transition after source-release admission;
the sealed default ELF has no registered release and therefore refuses it with
`0x79` before either delta occurs.

## 6. Non-negotiable STOP ledger

1. **Native mode binding — repaired at runtime; joined evidence landed;
   refinement boundary open:** the Terms-checked immutable basis mode is
   cached in KernelAccount v2 at `3a81b38` (inside the sealed runtime
   source), with mode-flip, wrong-Terms, derived-active-solvency, and
   resolved-native phase-refusal tests landed. At `896a1cc`, one continuous
   blank-bank joined lifecycle per smooth degree (one, two, three) is
   **SBF-EXECUTED** in local in-process banks under both ELF campaigns:
   the funded segment — public artifact seal, Realm/Profile, public
   SourceSpec/Feed/archive create/append/seal, v3 market creation, Endow,
   Split, Materialize, bearer transfer, source-joined point Resolve,
   exact-lot bearer and internal redemption, terminal withdrawal to zero —
   runs on the explicitly non-production mock-source ELF with exactly four
   named injections (the three non-Clutch mock-provider accounts plus the
   program-owned resolution evidence buffer, which no public instruction
   constructs); the default empty-registry ELF campaign asserts the exact
   `0x79` boundary per degree with byte-identical rollback. All steps clear
   the 1.4M-CU ceiling (Resolve 182,425/190,798/197,692 on the
   syscall-hashed runtime). This is
   focused local-bank evidence, not a validator walk, production source, or
   clearing-plane result. Still open: the refinement boundary from Terms
   bytes through the evaluator to v3 Resolution bytes and payout. Never
   infer mode from `resolved_payout`, preset membership, or
   vector-equals-preset.
2. **Complete native live semantics:** degree-selected blank-bank creation,
   source-joined point Resolve, sole-vector persistence, exact replay, and
   exact-lot internal and bearer redemption are live for degrees one through
   three, and the exact R1 ELF's first-party final-LTO stack audit passes.
   The post-resolution consumer audit is done and CLEAN (fifteen consumers,
   zero suspects, four recorded asymmetries:
   docs/reviews/POST_RESOLUTION_CONSUMER_AUDIT_2026-08-19.md). Freeze the
   fragment/credit policy promised to bearers (Arm A live-until-aggregated is
   RATIFIED 2026-08-20 per docs/decisions/ADOPTED_2026-08-20.md item 7). Monolithic occupation-v4 initial Resolve now clears the
   selected headroom on every measured row (172,665–197,766 CU on the
   syscall-hashed runtime; the former overage was the software hasher), and
   both it and the exact measured staged ResolutionWork route are
   profile-admitted.
3. **Authenticated source/archive ingestion:** pin one concrete source program,
   parser, and deployment profile; publicly create, append, and seal its
   canonical history; authenticate the immediate provider/config state and
   Clock admission; and remove genesis-injected mock source prerequisites.
   Until then the default registry remains empty and Endow must keep refusing
   `SourceReleaseUnavailable` (`0x79`); the non-production mock ELF is not a
   substitute.
4. **Complete blank-bank lifecycle:** typed artifacts, Realm/Profile, the
   initial degree-selected market plane, Reservation, and later-owner state now
   tolerate admitted pre-funding with focused real-bank rollback evidence. Add
   a consensus Terms-to-Grid existence join, preserve that constructor rule for
   every new account family, then publicly create and fund source/feed/archive, Epoch/pages,
   candidate/checkpoint, pot/receipt, and cleanup state.
5. **Coupled settlement:** one narrow direct subset can now construct a
   SUBMITTED Candidate/feed, and one separately preauthorized direct receipt
   can be consumed in SBF. Live V2 top-three Select now completes and commits at
   226,071 CU on the syscall-hashed runtime; its former exact-1,400,000-CU
   exhaustion was the software hasher, and V2's remaining profile STOP is
   the unimplemented empty-frozen lapse. The staged V3 successor is
   routed and merged at `fb72b34` with every measured row clearing the
   ceiling (worst 383,909 CU), but it is unpromoted in the liveness profile
   and its campaign covers one bank profile only. The full
   gate still must freeze the exact live reservation set, complete and score
   claims, close the candidate window, verify and select the best valid
   submitted candidate, create every immutable entitlement before resolution,
   support the admitted intent/fee shapes, and consume/refund everything
   exactly once.
6. **Prepaid liveness and economics:** measure final SBF paths and capitalize
   every mandatory unfinished action at admission under zero future volume.
   ResolutionWork now supplies one exact measured, prepaid, physically closing
   runtime path, but there is no global `LivenessPolicy` and no protocol-wide
   no-stranding result. Principal and owner assets never fund this work.
7. **Terminal ownership and retirement:** an empty frozen Direct V2 epoch can
   strand Reservations; no general program-account close exists outside
   artifact stages and ResolutionWork; outcome mints lack
   `MintCloseAuthority`; Hoard donations, external claim-burn forfeiture, and
   fractional fragments lack terminal disposition; and most accounts lack an
   authenticated rent-payer versus donation split. The internal-only Terminal
   Lifecycle V2 research model closes these equations only in its modeled
   profile; it supplies no live authority or Token-2022 transition. The R4
   terminal-economics model adds the incompatible raw-bearer/tombstone-only
   result and therefore retains nonzero claimant credits in a segregated vault;
   it supplies no runtime adapter, migration, or SBF transition. The separate
   `EvidenceOnlyRecoveryV1` model selects no numeric failure payout, finite
   independently prepaid repair, recoverable dormancy, and a lot-scaled
   new-market bearer encoding. Close these exact runtime domains without
   inventing a sweep right over owner or Hoard value.
8. **Evidence promotion:** the sealed artifact/stack/bank evidence, checked
   schema-v2 baseline, fresh Persvati portable attestation, and the hbox
   independent same-source rebuild (internally byte-reproducible; divergence
   from the macOS seal exhaustively classified as per-OS toolchain bytes)
   are retained, and the complete-scope dependency/license closure is now a
   committed artifact (32 manifests, 1,788 unique rows, 0 failures;
   `research/liveness-policy-profile/dependency_license_complete.tsv`, tool
   at `scripts/dependency_license_check.py` with the attested 12-manifest
   default mode byte-stable). Still required before a release claim:
   byte-level seal reproduction on a second macOS host, folding the
   complete-scope closure into a declared gate at the next emission cycle,
   human review of the flagged license rows (MPL-2.0 family, CDLA roots,
   one license-file-only crate), external security review, and a signed
   tag.
9. **Gate L0:** exact legal/entity/control/deployment facts, qualified advice,
   any required relief, and separate current user authorization remain outside
   engineering. No meeting, filing, proof, or local run closes this gate.

## 7. Handoff loop

1. Read `AGENTS.md`, `PROJECT.md`, this file, and `docs/V1_BACKLOG.md`.
2. Run `git status --short`; shared dirty bytes belong to their active owners.
3. Take the first dependency-unblocked STOP and state the exact falsifier.
4. Run the narrowest host/proof/runtime test capable of refuting the change.
5. Commit coherent local paths explicitly. Do not push, tag, publish, deploy,
   sign, fund, use public RPC, or contact a regulator without current authority.
6. Promote only from artifacts produced by the final joined bytes. Never let a
   categorical fallback impersonate native smooth semantics, and never let a
   green model or host test impersonate SBF/runtime evidence.
