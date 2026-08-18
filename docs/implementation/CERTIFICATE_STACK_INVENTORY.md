# Certificate stack inventory — `/Users/ember/dev/breadstuffs`

Status: **read-only survey. Nothing has been copied, moved, or committed.** This
document establishes what would be required before any artifact could cross the
greenfield boundary in [AGENTS.md](../../AGENTS.md) §1 and
[PROVENANCE.md](../PROVENANCE.md) §2. It is not a decision and not an approval.

Survey date: 2026-08-18. Surveyor: read-only inventory lane.
Source tree HEAD at survey time: `436c2a865` (breadstuffs, branch `main`).

## 0. Method, and what "measured" means here

Everything below is labelled **MEASURED** (I ran it or read the bytes),
**READ** (I read the source/doc and am reporting its own claim), or
**CLAIMED** (the tree asserts it and I did not independently check).

- Rust test runs used `CARGO_TARGET_DIR` redirected into a scratchpad and
  `--offline --locked`, so no build artifact, lockfile, or source in
  `breadstuffs/` was written. Confirmed: `git status --porcelain` over the
  surveyed paths is unchanged by this lane.
- **Skipped as heavy, deliberately:** `cargo test -p fhegg-fhe` (pulls
  `tfhe-rs` 1.6 + `fhe.rs`; the crate's own docs record minutes-to-tens-of-
  minutes per config), `cargo test -p dregg-circuit-prove` (full Plonky3
  recursion tower), and any `lake build` of `metatheory` (a 7.5 GB `.lake`).
  For the Lean side I report the presence and dates of built `.olean`
  artifacts instead of a fresh rebuild, and say so at each point.
- Where the tree's own audit log (`TESTQALOG.md`, last updated 2026-07-26)
  disagrees with the code as it stands today, I flag the code as authoritative
  and note the drift. Several defects that log records were closed afterwards;
  several were not.

---

## 1. Inventory

### 1.1 Component summary

| Component | Path | Lang | Size | Last commit | Runs? |
|---|---|---|---|---|---|
| fhEgg Stage-1 solver + certificates | `fhegg-solver/` | Rust | 18 modules, ~9,970 LoC src + 10 bins ~2,420 LoC | 2026-07-25 | **MEASURED green** (see §1.2) |
| fhIR typed product DSL + cert transport | `fhir/` | Rust | 9 modules, ~7,040 LoC | 2026-07-22 | **MEASURED green** (73 lib tests) |
| Cert-F / Cert-QP STARK bridges | `circuit-prove/src/cert_{f,qp}_air.rs` | Rust | 1,949 LoC | 2026-07-25 / 07-19 | not run (heavy) |
| Lean certificate cores + descriptors | `metatheory/Market/Cert*.lean`, `SddPsd.lean` | Lean 4.30.0 | 4,531 LoC / ~777 KB | 2026-08-06 | `.olean` present, Aug 8–11 |
| Byte-pinned AIR descriptors | `circuit/descriptors/dregg-cert-*.json` | JSON | 425 KB (3 files) | 2026-08-05 | n/a (data) |
| Stage-2 FHE convex engine | `fhegg-fhe/` | Rust | 50 src modules, `convex_step` 500 + `convex_engine` 923 LoC | 2026-08-14 | not run (heavy) |
| Self-audit log | `TESTQALOG.md` | Markdown | 3,761 lines / 307 KB | 2026-07-26 | n/a (doc) |

### 1.2 `fhegg-solver/` — file-level

License: **`AGPL-3.0-or-later`** (declared in `fhegg-solver/Cargo.toml`).
Crate deps: `wgpu 24`, `pollster 0.4`, `bytemuck 1`, `rayon 1`, `rand 0.8`,
`serde 1`, `serde_json 1`. **No feature gates exist in this crate at all** —
`grep -c 'cfg(feature' fhegg-solver/src/` is 0 — so every consumer of the crate
takes `wgpu` and the GPU stack. This is the single most important fact for
checker-only consumption; see §2.

| File | LoC | KB | Numeric domain | Role |
|---|---|---|---|---|
| `pdhg.rs` | 638 | 22.8 | f64 | PDHG flow-LP solver (untrusted search) |
| `clearing.rs` | 628 | 23.7 | **u128, no f64** | uniform-price fold/cross/allocate |
| `cert.rs` | 350 | 13.0 | **f64 only** | Cert-F carrier + `check_with` |
| `air.rs` | 334 | 11.6 | f64 | **hand-written Rust `ConstraintSystem` emit** (see §4) |
| `fisher.rs` | 454 | 16.2 | f64 | Eisenberg–Gale / `CertEq` |
| `cfmm.rs` | 410 | 13.3 | f64 | waterfill routing / `CertRoute` |
| `qp.rs` | 610 | 19.9 | f64 | OSQP-form ADMM **and** `CertQp::check` (mixed) |
| `qp_exact.rs` | 843 | 31.0 | **i128** (53 sites, 34 `checked_`) | exact fixed-point KKT lift + checker |
| `qp_strict.rs` | 267 | 9.4 | **i128, no f64** | exact zero-KKT verifier |
| `package.rs` | 732 | 27.6 | f64 | package/all-or-none + `CertPackage` |
| `discriminatory.rs` | 287 | 10.8 | f64 | pay-as-bid, reuses Cert-F |
| `smooth.rs` | 712 | 26.3 | f64 | SGD gradient-norm `CertGrad` |
| `pricecert.rs` | 813 | 30.4 | f64 | derivatives price cert / Snell |
| `uniform_allocation_cert.rs` | 1042 | 35.3 | **u128, no f64** | exact largest-remainder allocation cert |
| `gpu.rs` | 499 | 18.6 | f64 | wgpu fold + PDHG matvec |
| `wire.rs` | 945 | 35.2 | u128 | JSON settlement wire |
| `book.rs` | 324 | 13.0 | f64 | order-book → FlowLP |
| `lib.rs` | 83 | 4.4 | — | module roots + honest-scope doc |

Ten benchmark/runner bins (`bench`, `e2e`, `fhegg_clear`, `fhegg_settle`,
`fhegg_uniform`, `package_bench`, `package_clear`, `pricecert_bench`,
`pricecert_clear`, `smooth_bench`), ~2,420 LoC — the mission brief said 8; the
tree has **10**.

**MEASURED test results** (this machine, macOS arm64, offline, 2026-08-18):

```
cargo test -p fhegg-solver --offline --locked --lib
  → 124 passed; 0 failed; 0 ignored     (0.57 s; includes 2 live wgpu/Metal tests)
cargo test -p fhir -p fhegg-solver --offline --locked          (default parallel)
  → tests/qp_exact_hotpath.rs  FAILED 2/2
cargo test -p fhir -p fhegg-solver --offline --locked -- --test-threads=1
  → fhegg-solver lib      124 passed
  → tests/qp_exact_hotpath  2 passed
  → tests/qp_strict         7 passed
  → fhir lib               73 passed
  → 0 failed overall
```

**The parallel red is real and worth recording.** `tests/qp_exact_hotpath.rs`
installs a `#[global_allocator]` with process-global `AtomicUsize` counters and
asserts exact allocation counts (`assert_eq!(owned_allocations / ITERATIONS, 7)`
at line 303, `baseline_allocations / ITERATIONS == 7` at line 231). Under
cargo's default parallel harness those counters are polluted by sibling test
threads, so the assertions fail (measured 16 vs 7, and 2 vs 7). Single-threaded
they pass. This is a **test-harness defect, not a certificate-soundness
defect** — but it means "the crate is green" is only true with
`--test-threads=1`, and a consumer wiring these into CI would inherit a
flaky red.

### 1.3 `fhir/` — file-level

License: **`AGPL-3.0-or-later`**. Deps: `fhegg-solver` (path), `serde`,
`serde_json`, `sha2`. Transitively therefore also `wgpu`.

| File | LoC | KB | Role |
|---|---|---|---|
| `compile.rs` | 1829 | 68.2 | product → `ConvexProgram`; **exact SDD⇒PSD admission certificate** (`FHSDD001`) |
| `qp_certificate.rs` | 1373 | 49.9 | `FHQPB001` bundle: admission ∧ KKT, self-verifying from bytes |
| `optimizer_protocol.rs` | 1291 | 45.9 | fail-closed transport for **untrusted external optimizer workers** |
| `solver_bridge.rs` | 914 | 36.6 | dispatch into `fhegg-solver`; returns the engine's own cert report |
| `types.rs` | 681 | 29.1 | product/tier types |
| `products.rs` | 357 | 13.7 | product catalogue |
| `ast.rs` | 267 | 9.6 | DSL AST |
| `lib.rs` | 220 | 9.9 | re-exports |
| `tier.rs` | 107 | 4.1 | privacy tier lattice |

`optimizer_protocol.rs` is the most interesting file in the whole survey for
Dragon's Clutch, because it is *already* the shape we want: a relying party
constructs a request from a canonical problem, pins a solver manifest, and
accepts a response **only** through `verify_optimizer_worker_result`, which
binds request, session, nonce, solver identity, certificate format and
certificate bytes, then delegates the bytes to an independent checker before
consuming a replay identity. Domain-separated SHA-256 throughout
(`fhir/optimizer-request/v1`, `.../optimizer-certificate/v1`,
`.../optimizer-replay-id/v1`).

### 1.4 `metatheory/Market/` — the Lean cores

Toolchain: **`leanprover/lean4:v4.30.0`** (`metatheory/lean-toolchain`).

| File | LoC | KB | `sorry` | `axiom` | `native_decide` |
|---|---|---|---|---|---|
| `CertF.lean` | 327 | 18.6 | 0 | 0 | 0 |
| `CertQp.lean` | 408 | 23.3 | 0 | 0 | 0 |
| `CertQpRustDenotation.lean` | 355 | 15.7 | 0 | 0 | 0 |
| `CertFDescriptor.lean` | 1665 | 88.5 | 0 | 0 | 0 |
| `CertFGolden.lean` | 32 | 440.6 | 0 | 0 | 0 |
| `CertQpDescriptor.lean` | 1491 | 66.7 | 0 | 0 | 0 |
| `CertQpGolden.lean` | 7 | 114.9 | 0 | 0 | 0 |
| `SddPsd.lean` | 246 | 8.8 | 0 | 0 | 0 |

**MEASURED:** clean of `sorry`, `admit`, first-party `axiom`, and
`native_decide` across all eight files. Built `.olean` artifacts exist for
every one, dated 2026-08-08 to 2026-08-11 — i.e. *newer* than every
corresponding source file. That is evidence the Market cert modules elaborated
green in the second week of August; it is **not** a fresh re-verification by
this lane, and a consumer must rebuild before relying on it.

Key theorem statements, read directly (not vacuous, not `P → P`):

- `CertF.weak_duality` — for every primal-feasible `f` and dual-feasible
  `(π,s)`, `wᵀf ≤ cᵀs`. Generic over any ordered commutative ring `R`.
- `CertF.certifies_epsilon_optimal` — the keystone: gap ≤ ε ⇒ every feasible
  `f'` has `wᵀf' ≤ wᵀf + ε`.
- `CertF.gap_nonneg` — a negative-gap "certificate" is vacuous.
- `CertQp.qp_certifies_epsilon_optimal` — the QP analogue over ℚ, keyed on
  `PsdSymm P`.
- `SddPsd.sddCheck_implies_psd` — an executable integer SDD check implies PSD,
  with `psdOutsideSdd2_refused` honestly recording that SDD is **strictly
  narrower** than PSD (the matrix `!![1,2;2,4]` is PSD and is refused).
- `CertFDescriptor.certFDescriptor_emit_sound` — emit-soundness **generic over
  the program** `p : CertFProg`, with per-program integer admissions
  `ring3Prog_integerAdmission` / `market4Prog_integerAdmission` discharged
  separately.

### 1.5 `fhegg-fhe/` — Stage-2

License: **`AGPL-3.0-or-later`**. The mission brief named `pdhg.rs`,
`convex_step.rs`, `convex_engine.rs`. Located precisely:

- `fhegg-fhe/src/convex_step.rs` (500 LoC) — one iteration of
  `x ← prox(x − τ·A·x)` over encrypted BFV state, purely additive. Its own
  doc's key claim: `A` is public so `A·x` is scalar-mul by public constants, no
  relinearization. Validated against `fhe.rs`'s own repeated addition in
  `tests/convex_step_oracle.rs` (READ, not run).
- `fhegg-fhe/src/convex_engine.rs` (923 LoC) — `T>1` composition in the
  `tau_den`-scaled domain, no descale, refusing on `WindowExceeded`. Depends on
  `metatheory/Bfv/Noise.lean`'s T-composition bound.
- `fhegg-fhe/src/bin/pdhg.rs` (190 LoC) — a **bin**, not a library module.
- `fhegg-fhe/MEASURED-ENVELOPE.md` (252 lines) — honest, and the most
  credible perf document in the tree. Its own bottom line: no-viewer FHE
  clearing "**works and is CORRECT; NO it is not fast**" — 24 s at N=8/K=16,
  ~5 min at N=128/K=64, ~8.8 min at N=32/K=256 on 24-core CPU with tfhe-rs
  1.6.3; N=512/K=256 extrapolated to ~76 min. It also documents a *superseded*
  earlier measurement and a real rule bug it fixed (sum-of-crossing-bits
  under-clears; counter-witness D=(10,9), S=(5,20)).

**Not run.** Building this crate pulls `tfhe 1.6` and the vendored `fhe.rs`
fork; the numbers above are the crate's own, read, not reproduced.

### 1.6 `TESTQALOG.md` — what the self-audit says

3,761 lines, last entry 2026-07-26. **A naming correction first:** the "five
sibling certificates" section (`## 2026-07-17 — 4swarm/fhegg-siblingcerts`,
lines 1828–1902) audits **CertQp, CertEq, CertRoute, CertPackage, CertGrad** —
*not* pricecert and *not* uniform-allocation. Those two files exist but are
never audited in this log. Cert-F is audited separately (lines 1403–1469 and
3597–3676).

Defects it flags, verbatim in substance:

| Cert | Defect |
|---|---|
| **CertQp** | "PSD of `P` is caller-pinned, not checked (a saddle certifies nothing)". A non-convex `P` yields a meaningless certificate that still reports valid. |
| **CertEq** | Comment-vs-code lie: `fisher.rs:196` says "(mirrors what a Lean checker proves)" — **no Fisher/EG Lean file exists** in `metatheory/`. |
| **CertRoute** | No per-cert defect. |
| **CertPackage** | **Accepted-but-wrong**: an EMPTY clearing (W=0, α=0) reports `valid=true`; `valid` gates no ratio floor. `bound_sound` is "a theorem that can only fail on a checker bug" — i.e. it constrains the prover not at all. And `fhir::RunOutcome::certificate_valid()` surfaces that bit as `Some(true)`. |
| **CertGrad** | `ε` is prover-chosen, so `valid` means only "the claimed ε is met" — not optimality. |
| **all five** | **The deepest one.** "each cert CARRIES ITS OWN program (P/q/A/…) — no program binding/registry … and no descriptor/STARK chain. ε is descriptive everywhere." The prover supplies the instance, so the certificate says "I solved *some* problem", not "I solved *the agreed* problem". |
| Cert-F (lane2) | The old `certFDescriptor_emit_sound` "exposed `g ≥ 0` but NEVER extracted the gap GATE congruence — so the keystone as stated never touched ε at all." Found and fixed 2026-07-17. |
| Cert-F (lane2) | `cert_f_descriptor_matches_lean` "is now tautological (parses the same committed file twice)". |
| Cert-F (07-24) | `POST /prove-shielded` "cannot succeed for ANY user-submitted order, ever" — the registry held ε ∈ {0, 2000} while `fhegg_clear.rs` hardcoded `epsilon = 0.5f64`. |

The log records **"Committed NOTHING — supervisor gates"** for all three lanes.

**Freshness corrections I measured against today's code** (the log is 3+ weeks
stale and several of its defects were closed afterwards):

- **CLOSED.** `cert_f_descriptor_matches_lean` is no longer tautological.
  `circuit-prove/src/cert_f_air.rs` now `include_str!`s
  `metatheory/Market/CertFGolden.lean`, decodes the Lean `String` literal, and
  byte-compares against the committed JSON — with a **refutability test**
  (`the_lean_golden_pin_is_refutable`) covering a drifted golden, a truncated
  golden, the wrong golden, and an absent golden.
- **CLOSED.** `fhegg_clear.rs` no longer hardcodes `epsilon = 0.5f64`. It
  carries a declared `AccuracyBudget` with an `integer_epsilon` resolved
  against a registered program, and the source comment reads "Never a literal."
- **CLOSED, narrowly.** CertQp's unchecked-PSD hypothesis is now gated at the
  fhIR boundary: `compile.rs` mints an `ExactSddPsdCertificate` (`FHSDD001`),
  `qp_certificate.rs` makes admission-without-KKT unrepresentable at the
  transport boundary, and `SddPsd.lean` proves `sddCheck_implies_psd`. The gate
  is **conservative**: SDD ⊊ PSD, and the Lean file says so with an explicit
  refused-PSD witness.
- **CLOSED for QP and uniform-allocation only.** The "each cert carries its own
  program" gap is closed for exactly two paths, by work postdating the audit:
  `canonical_qp_program_digest` + the `FHQPB001` bundle, and the `FHUAC001`
  uniform-allocation certificate whose verifier takes the price grid from the
  **caller**, "never a certificate-selected grid".
- **STILL OPEN.** CertEq, CertRoute, CertPackage, CertGrad carry every defect
  the log recorded. No later entry revisits them. `package.rs`, `fisher.rs`,
  `cfmm.rs`, `smooth.rs` have not been touched since 2026-07-14.
- **STILL OPEN.** `CERT_F_REGISTRY` in `cert_f_air.rs` still contains exactly
  **two** programs — a 3-node/3-edge ring toy (ε=0) and a 3-asset/4-order
  "market4" batch (ε=2000, scale=100). Any other public program is refused with
  "Cert-F public program is not registered as a Lean-emitted descriptor". This
  is *correct fail-closed behaviour* and simultaneously the hard ceiling on the
  whole STARK path.

---

## 2. Checker separability — the verdict

**Verdict: separable in the source, NOT separable in the crate. Extraction is
real work, not a dependency line.**

### 2.1 What crosses the boundary

For the certificate path a consumer needs these types, and nothing else:

| Certificate | Carrier type | Verifier entry point | Exact? |
|---|---|---|---|
| Cert-F (flow LP) | `fhegg_solver::cert::CertF` {`n_nodes, m_edges, edges, w, c, f, pi, s, epsilon`} + 4 diagnostic fields | `CertF::check_with(feas_tol) -> CertReport` | **No — all f64** |
| Cert-QP (approx) | `fhegg_solver::qp::CertQp` | `CertQp::check()` | No — f64 |
| Cert-QP (exact) | `fhegg_solver::qp_exact::CertQpExact` | `lift_cert`, `CertQpExactReport` | **Yes — i128, 34 `checked_` sites** |
| Zero-KKT QP | `qp_strict::VerifiedZeroKktQp` | `verify_zero_kkt_qp{,_ref}` | **Yes — i128, no f64** |
| Uniform allocation | `uniform_allocation_cert::UniformAllocationCertificate` (`FHUAC001`) | `verify_uniform_allocation` | **Yes — u128, no f64** |
| fhIR QP bundle | `fhir::ExactQpCertificateBundle` (`FHQPB001`) | `verify_certified_qp`, `verify_zero_kkt_certified_qp` | Yes |
| Optimizer transport | `fhir::OptimizerJobRequest` / result (`FHIROQ01`/`FHIROS01`) | `verify_optimizer_worker_result` | Yes |

The checkers are genuinely **recompute-from-witness**, not re-solve. `CertF::check_with`
reconstructs `A` from the public edge list and recomputes `wᵀf`, `cᵀs`, the gap
and `‖Af‖_∞`; the stored `primal_obj`/`dual_obj`/`duality_gap`/`feas_residual`
fields are explicitly diagnostics and there is a test
(`stored_objective_forgery_cannot_hide_a_real_gap`) proving a forged diagnostic
cannot decide acceptance. `verify_uniform_allocation`'s own doc states it
recomputes "without calling `fold_curves`, `crossing`, `allocate`, or
`Allocation::validate`". Nothing in the checker path calls the solver.

### 2.2 Why the crate is nevertheless not separable

Three concrete obstructions, all measured:

1. **No feature gates.** `fhegg-solver` has zero `cfg(feature …)` and no
   `[features]` table. `pub mod gpu;` is unconditional, so `wgpu 24`,
   `pollster`, `bytemuck` and `rayon` are unconditional dependencies of anyone
   who writes `fhegg-solver = { … }`. A Solana-adjacent verifier crate cannot
   take that.
2. **`cert.rs` imports the solver module.** `use crate::pdhg::FlowLp;` — but
   only for the incidence struct and its `a_times`/`at_times` matvecs. That is
   ~20 lines of pure linear algebra, not a solver. The coupling is a file-
   layout accident, not a real dependency.
3. **`qp.rs` mixes solver and checker in one file.** `solve_admm` (the ADMM
   search) and `CertQp::{from_solution, check}` are siblings. Splitting is
   mechanical but is an edit to their tree, not ours.

### 2.3 The checker-only slice, sized

Modules with **no intra-crate imports at all** (verified by grepping
`use crate::` in every module): `clearing.rs`, `qp.rs`, `pricecert.rs`,
`package.rs`, `smooth.rs`, `cfmm.rs`, `fisher.rs`. The checker slice is a
closed subgraph:

```
clearing.rs (628)  ──> uniform_allocation_cert.rs (1042)
qp.rs (610)        ──> qp_exact.rs (843) ──> qp_strict.rs (267)
cert.rs (350)      ──> [needs a ~20-line FlowLp shim]
```

That is **≈3,740 LoC** and needs only `serde`/`serde_json` — no `wgpu`, no
`rayon`, no `pollster`, no `rand`. Adding the fhIR transport layer
(`qp_certificate.rs` + `optimizer_protocol.rs` + the SDD parts of
`compile.rs`) roughly doubles it and adds `sha2`.

**Bottom line for a "checker only, never the solver" posture:** the property we
want is real and is architecturally honoured in the source. Getting it as a
*dependency* is not possible against the tree as it stands; it requires either
an upstream refactor in breadstuffs (feature-gate `gpu`, split `qp.rs`, lift
`FlowLp`) or a re-specification on our side.

---

## 3. Licensing and provenance

### 3.1 First-party licences

| Crate / file | Declared licence |
|---|---|
| `fhegg-solver` | `AGPL-3.0-or-later` |
| `fhir` | `AGPL-3.0-or-later` |
| `fhegg-fhe` | `AGPL-3.0-or-later` |
| `fhegg-core` | `AGPL-3.0-or-later` |
| `dregg-circuit-prove` | `license.workspace = true` |
| breadstuffs `[workspace.package]` | `AGPL-3.0-or-later` |
| repo root `LICENSE` | **GNU Affero GPL v3, 19 Nov 2007** (confirmed by reading it; 34,523 bytes / 661 lines — byte-identical in length to Dragon's Clutch's own `LICENSE`) |
| `dregg-circuit` | `license.workspace = true` |

**This is the good news.** Dragon's Clutch intends `AGPL-3.0-or-later`
([PROVENANCE.md](../PROVENANCE.md) §1) and the surveyed crates already declare
it. There is no *outbound* licence conflict for the first-party fhEgg/fhIR
source. Authorship is the same author. That removes the hardest class of
blocker — but it does **not** remove the AGENTS.md §1 greenfield prohibition,
which is a project-boundary decision, not a licence question, and which
explicitly requires "an explicit current user decision and a recorded
provenance/license review" regardless of licence compatibility.

### 3.2 Vendored and patched third-party code

The workspace `[patch]` table redirects several upstream crates to local forks,
and `circuit-prove` also takes four crates from a second Plonky3 fork by git
rev. Anything reaching the STARK path inherits all of these.

| Fork | Path / source | Upstream | Licence | Notice file present? | Delta |
|---|---|---|---|---|---|
| **p3-fri** 0.5.1 | `vendor/plonky3-fri-82cfad73` (`[patch]`) | `Plonky3/Plonky3` @ `82cfad73cd734d37a0d51953094f970c531817ec` | MIT OR Apache-2.0 | **yes** (both) | injectable TwoAdic FRI matrix-fold backend **+ the `get_evaluations_on_domain` row-order fix** |
| **p3-challenger** 0.5.1 | `vendor/plonky3-challenger-82cfad73` (`[patch]`) | same rev | MIT OR Apache-2.0 | **NO — see below** | ordered rayon PoW finders (`find_first`) for byte-determinism |
| **p3-recursion, p3-circuit, p3-circuit-prover, p3-poseidon2-circuit-air** | git `emberian/plonky3-recursion` @ `fc3c6dfac26e2082653d2a617a1740446ce33f05` | Plonky3 | MIT OR Apache-2.0 | yes (both) | recursion tower; **direct deps of `circuit-prove`** |
| **fhe.rs** | `vendor/fhe-dregg` (`[patch]`) | `fhe` 0.1.1 | MIT | **NO — see below** | one feature-gated wire codec for `mbfv::RelinKeyShare` |
| **curve25519-dalek** | `vendor/curve25519-dalek-dregg` (`[patch]`) | 4.1.3 | BSD-3-Clause | yes | feature-gated GPU-parity qualification API |
| **bulletproofs** 5.0.1 | `vendor/bulletproofs-r1cs-wgpu` (`[patch]`) | — | MIT (© 2018 Chain, Inc.) | yes | wgpu MSM |
| **poly-commitment, mina-poseidon, mina-curves** | git `emberian/proof-systems` @ `c5305e63df5474c8cca81f7951264ea6aafe81db` | `o1-labs/proof-systems` | Apache-2.0 | yes | relaxes `rayon = "=1.10.0"` exact pin |
| **ark-serialize** | git `emberian/algebra@serde-integration` | arkworks | MIT/Apache | yes | serde integration |

**The full dependency graph is otherwise clean.** A resolved offline
`cargo tree -e normal` across `fhegg-solver`, `fhir`, `dregg-circuit`,
`dregg-circuit-prove`, `fhegg-core` and `fhegg-fhe --features
tfhe-integer,amm-input-binding` is **275 distinct packages**, and a grep for
`non-commercial`, `research`, `Business Source`, `BUSL`, `PolyForm`, `SSPL`,
`Elastic` across every workspace and vendored manifest returns **zero hits**.
Apart from the Zama family (§3.3) everything is MIT / Apache-2.0 / BSD-2 /
BSD-3 / CC0 / Unicode-3.0 — all inbound-compatible with AGPL-3.0-or-later. So
the anticipated "restrictive research licence" hazard **did not materialise**;
the real hazards are the two below.

#### The unmerged Plonky3 fix, stated precisely

From `vendor/plonky3-fri-82cfad73/Cargo.toml` `[package.metadata.dregg]` and the
header of `src/two_adic_pcs.rs`: upstream `82cfad73` (and "every `p3-fri`
through 0.6.3, and `main` at `f5b7977e`") applies `bit_reverse_rows()` once too
many on the re-interpolation fallback of `get_evaluations_on_domain`. The
caller then evaluates AIR constraints on permuted trace rows, builds a wrong
quotient, and "emits a complete, well-formed proof its own verifier rejects
with `OodEvaluationMismatch`." It was introduced by upstream PR #1352 /
`b9863b6b` (2026-03-02), the same commit that deleted a guarding
`assert!(lde.height() >= domain.size())`, "turning a loud panic into a silently
invalid proof." **Upstream PR #1982 is this same one-line change; it is open
with CHANGES_REQUESTED as of 2026-08-14.** Local evidence cited:
`circuit/tests/fri_extrapolation_row_order.rs` and
`circuit/tests/fri_blowup_global_knob_survey.rs`. Nothing in
`breadstuffs/docs/` discusses it — the rationale lives only in the vendored
source header and the manifest metadata.

Three qualifications a manifest must carry:

- **It is a completeness/correctness defect, not an accepted-forgery soundness
  hole.** The described failure mode is a *rejected* proof, not a forged
  accepted one. Calling it "the unmerged upstream soundness fix" overstates it,
  and we should not repeat that phrasing. What it does additionally establish
  is that the widely-repeated "a degree-7 S-box requires `log_blowup ≥ 3`"
  floor "was never a soundness bound, a FRI property, or a fact about the field
  — it was this row permutation."
- The sibling `p3-challenger` delta is **explicitly not** a soundness change,
  and says so in its own header: the PoW predicate and the honest prover's work
  are identical; only *which* valid witness is returned changes.
- Depending on this fork means owning a **divergence upstream has declined in
  its current form**, and a bump off rev `82cfad73` silently reintroduces the
  bug — with the guarding assert already deleted upstream, so it comes back
  quiet.

#### Two MIT/Apache notice-retention gaps in their tree

Both are git-tracked and therefore redistributed:

- **`vendor/plonky3-challenger-82cfad73/`** declares `MIT OR Apache-2.0` and
  ships **no `LICENSE-MIT` or `LICENSE-APACHE`**. `git ls-files` returns only
  `Cargo.toml`, `CHANGELOG.md`, `src/*`.
- **`vendor/fhe-dregg/`** declares `MIT` and ships **no `LICENSE`**.

MIT ("The above copyright notice … shall be included in all copies or
substantial portions of the Software") and Apache-2.0 §4(a)/(d) both require the
notice to travel with a redistributed copy. This is an **existing compliance
gap in breadstuffs, independent of AGPL** — and it is a gap Dragon's Clutch
would *inherit* on any copy. It is cheap to fix (copy the upstream notices) but
it must be fixed *before* anything crosses, not after.

### 3.3 tfhe-rs / Zama — the one genuine licence blocker

`fhegg-fhe/Cargo.toml:44` takes `tfhe = { version = "1.6", features =
["integer"], optional = true }`, gated behind the **off-by-default** feature
`tfhe-integer` (line 13). Resolved in `Cargo.lock`: `tfhe 1.6.3`,
`tfhe-csprng 0.9.1`, `tfhe-fft 0.10.1`, `tfhe-ntt 0.6.1` + `0.7.1`,
`tfhe-safe-serialize 0.1.1`, `tfhe-versionable 0.8.0` + derive. **All eight
declare `BSD-3-Clause-Clear`.**

The operative clause, from the shipped `LICENSE` ("Copyright © 2026 ZAMA"):

> **NO EXPRESS OR IMPLIED LICENSES TO ANY PARTY'S PATENT RIGHTS ARE GRANTED BY
> THIS LICENSE.**

And from the shipped `README.md`:

> Zama's libraries are free to use under the BSD 3-Clause Clear license **only
> for development, research, prototyping, and experimentation purposes**.
> However, for any **commercial use** of Zama's open source code, companies
> must purchase Zama's commercial patent license. … **Yes, all Zama's
> technologies are patented.**

**Why this is an AGPL blocker and not a formality.** BSD-3-Clause-Clear's
*copyright* grant is AGPL-compatible — it is BSD-3 minus the implied patent
grant. The collision is on the patent leg. **AGPL-3.0 §11 ¶3** obliges a
conveyor to grant recipients a patent licence covering the conveyed copy;
**§7** forbids adding terms that restrict downstream use; **§12** states that
patent-license conditions contradicting the License "do not excuse you from the
conditions of this License." We cannot grant our AGPL recipients Zama patent
rights we do not hold, while Zama asserts every downstream commercial user
needs a separate paid licence. The copyleft is satisfiable; the *effective
freedom* AGPL promises recipients is not. (Zama also requires a CLA for
contributions.)

**Mitigating, and decisive for scoping:** `tfhe` reaches **only** `fhegg-fhe`,
and only with `--features tfhe-integer`. `fhegg-core`, `fhegg-solver`, `fhir`,
`circuit` and `circuit-prove` do **not** pull it; breadstuffs' own shipped
`node` manifest states it "does NOT enable `tfhe-integer` (no tfhe-rs in the
node graph)". **Scoping the certificate stack to exclude `fhegg-fhe` makes
this question moot, and that is what §6 recommends.** If Stage-2 FHE is ever
wanted it needs counsel, not a code review.

### 3.4 No licence gate exists upstream

`breadstuffs/audit.toml` is a **`cargo-audit` security-advisory policy only —
it records no licence policy at all.** There is **no `deny.toml`, no
`about.toml`, no SBOM (SPDX or CycloneDX), no `THIRD-PARTY` manifest, and no
`cargo-deny` invocation in `.github/workflows/`.** A future dependency with a
BUSL/SSPL/PolyForm licence would land there silently.

Worth passing along as a courtesy even though it is outside this survey's
scope: `audit.toml` documents its own footgun — CI builds the `--ignore` list
with `grep -oE 'RUSTSEC-[0-9]{4}-[0-9]{4}' audit.toml` over the **whole file**,
so any advisory id merely *mentioned in a comment* silently becomes a
suppression. That is a security-process bug in their tree, not a licensing one,
and it is not ours to fix — but if we ever adopt that pattern we should not
adopt that bug.

### 3.5 What CANNOT cross without a human decision

Ranked, most blocking first.

1. **The AGENTS.md §1 boundary itself.** `breadstuffs` is named in the
   prohibited list. Nothing here crosses on a licence argument. This is the
   gate; everything below is what the gate would have to weigh.
2. **`fhegg-fhe` / tfhe-rs.** §3.3. Not reconcilable with AGPL-3.0-or-later
   distribution without a Zama patent licence. **Recommend: out of scope,
   permanently, unless the user decides otherwise with counsel.**
3. **The two notice-retention gaps.** §3.2. Must be closed before, not after,
   any copy — PROVENANCE.md §6 asks "Are third-party notices and source-offer
   duties captured?" and today the honest answer for those two forks is no.
4. **The Plonky3 forks.** Not a licence blocker (MIT/Apache is inbound-
   compatible), but a *governance* blocker: taking them means owning a
   maintained divergence from an upstream that has requested changes to the
   same patch, plus a second fork (`emberian/plonky3-recursion@fc3c6df`) that
   `circuit-prove` depends on directly. PROVENANCE.md §3 requires
   "maintainer/release authenticity" and §7 requires "known provenance
   exceptions (normally empty)". This would be a standing non-empty exception.
5. **The Lean toolchain and Mathlib as a proof dependency.** PROVENANCE.md §3
   is explicit that "proof dependencies need the same review as runtime
   dependencies" and that "a trusted specification, axiom, code generator,
   compiler plugin, or binary tool expands the evidence boundary". Taking the
   Lean cores means taking Lean 4.30.0 + Mathlib into our evidence boundary.
6. **The byte-pinned descriptor JSONs.** `dregg-cert-f-ir2.json` (152 KB),
   `dregg-cert-f-market4-ir2.json` (185 KB), `dregg-cert-qp-portfolio6-s3-ir2.json`
   (88 KB) are **generated artifacts** under PROVENANCE.md §5 and must carry
   generator, version, input digests, and a deterministic reproduction command
   (`scripts/emit_descriptors.py`). They are also worthless without the Lean
   sources that emit them, so they cannot cross alone.

### 3.6 The manifest PROVENANCE.md would require

Per §3 (dependency admission), for **each** of `fhegg-solver`, `fhir`, the
Lean `Market/Cert*` set, each vendored fork, and the Lean toolchain:

```
name and purpose
upstream repository/package
exact version and commit/content digest
license and notice obligations
source availability
maintainer/release authenticity
features enabled
transitive dependency lock digest
runtime/proof/build/dev classification
security and reproducibility notes
reviewer and date
```

Per §5 (generated artifacts), for each descriptor JSON and each golden Lean
string: generator + version, all source/config/input digests, the deterministic
reproduction command, its status (reviewed source vs build output vs evidence),
and embedded-third-party notice handling.

Per §7 (release ledger): `NOTICE` entries for MIT/Apache Plonky3 (both forks),
`fhe.rs`, dalek, bulletproofs and proof-systems; the toolchain lock; an SBOM
(**breadstuffs has none — we would author it, not inherit it**); source-offer;
the **proof assumption/trust inventory** (Lean 4.30.0, Mathlib rev, the
undischarged FRI/STARK soundness floor); and the non-empty **known provenance
exceptions** list containing at minimum the PR #1982 divergence and the two
notice gaps until they are closed.

Because breadstuffs has **no `deny.toml`, no SBOM, and no licence gate in CI**
(§3.4), none of this is inheritable evidence — every line of the manifest is
work we would do from scratch. That cost belongs in the decision, not after it.

Three things the manifest must state that are easy to omit and would be
misleading to omit:

- **Nothing here is "verified" end to end.** The Lean theorems are real and
  `sorry`-free, but the STARK path inherits an undischarged FRI/STARK soundness
  floor, and the hiding PCS has no simulator theorem — `cert_f_air.rs`'s own
  doc says "a formal simulator theorem for the complete batch-STARK transcript
  remains a separate proof floor." Per AGENTS.md §"Correctness vocabulary" we
  may not call this formally verified without naming exactly that.
- **The f64 checkers are not exact.** Cert-F, CertEq, CertRoute, CertPackage,
  CertGrad and the approximate CertQp all decide acceptance in `f64` with a
  scaled tolerance. That collides directly with AGENTS.md's
  "Portfolio payoffs and simplex prices use exact scaled integers and one named
  rounding boundary."
- **Four of the five sibling certificates carry open, unrepaired defects** from
  the 2026-07-17 self-audit (§1.6). A manifest that lists `fisher.rs`,
  `cfmm.rs`, `package.rs` or `smooth.rs` without naming those defects would be
  a misleading record, not a provenance record.

---

## 4. The Lean discipline — CONFIRMED, with one named exception

Our house rule: **circuits are authored in Lean, never hand-written in Rust;
Rust only calls into the Lean artifact.**

### 4.1 CONFIRMED for the STARK AIRs

`circuit-prove/src/cert_f_air.rs` and `circuit-prove/src/cert_qp_air.rs` are
**genuine consumers of Lean-emitted, byte-pinned descriptors.** This is not a
comment claiming it; it is the mechanism:

- Neither file constructs constraints. Both `include_str!` a committed JSON
  descriptor and `parse_vm_descriptor2` it. `cert_qp_air.rs`'s module doc says
  flatly: **"Rust authors no constraints."**
- The pin is checked against the Lean source itself. `cert_f_air.rs`
  `include_str!`s `metatheory/Market/CertFGolden.lean`, decodes the
  `def NAME : String := "" ++ "…"` literal, compares **byte for byte** with
  length and first-differing-byte diagnostics, and fails on mismatch.
- The pin is **refutable**, with a test that proves it. `the_lean_golden_pin_is_refutable`
  exercises a drifted golden (one emitted trace width changed, same length), a
  truncated golden, the wrong golden (market4's bytes against ring-3), and an
  absent golden — each must be *refused*. This is exactly the anti-vacuity
  discipline we ask for, and it is the fix for the tautological-pin defect
  TESTQALOG flagged on 2026-07-17.
- The Lean side `#guard`s the emission:
  `#guard emitVmJson2 certFDescriptor == Market.CertFGolden.CERT_F_RING3_GOLDEN`
  and the market4 twin, plus shape guards (`traceWidth == 465`,
  `constraints.length == 482`; market4: 581 / 602) and a field-capacity guard
  `#guard 3 * 2 ^ certFValueBits < 2013265921`.
- Unregistered programs are **refused**, not specialized at runtime.
  `try_cert_f_descriptor` returns an error naming the required procedure
  ("emit `certFDescriptorOf(program)`, byte-pin, commit, and register").
- The emit-soundness theorem is **generic over the program**
  (`certFDescriptor_emit_sound {hash} (p : CertFProg)`), with integer range
  policies discharged per program. That is the right shape: not a per-instance
  tautology.

**Verdict: this stack's AIR discipline is compatible with ours, and in the
Cert-F case is a slightly stronger version of it** (we do not currently have a
refutable-golden test anywhere in Dragon's Clutch).

### 4.2 The exception: `fhegg-solver/src/air.rs` is hand-written Rust AIR

`fhegg-solver/src/air.rs` (334 LoC, unchanged since 2026-07-14) defines
`Var`/`Relation`/`Term`/`Constraint`/`ConstraintSystem` and a
`ConstraintSystem::emit(cert: &CertF)` that **authors the `n + 4m + 1` Cert-F
constraint rows in Rust**, with its own `evaluate`/`AirReport`.

By the letter of our rule this is exactly the pattern named as debt: a
hand-written `air.rs` with a `ConstraintSystem` builder. Two mitigating facts,
stated so the judgement is informed rather than reflexive:

- It is explicitly **not** the proving path. Its own doc says "It is
  deliberately NOT a full STARK", and the real STARK path
  (`cert_f_air.rs`) does not import it — it consumes the Lean descriptor.
- Its role is a self-check that the emitted system accepts exactly what
  `CertF::check` accepts.

That does not make it safe to inherit. It is a **second semantic owner** of the
Cert-F constraint set alongside the Lean descriptor, which PROVENANCE.md §5
forbids ("Generated files never become a second semantic owner"). If any of
this stack is ever consumed, `air.rs` should be excluded by name.

`fhegg-solver/src/cert.rs`'s `check()` is a related but milder case: a Rust
mirror of the Lean checker, self-described as such ("the authoritative decision
is the verified checker's, not this one's"). It is a legitimate fast pre-filter
as long as nothing treats it as authority.

---

## 5. Reachability

**The brief's premise is substantially right and literally wrong. Correcting
it precisely, because the correction matters.**

| Claim | Verdict |
|---|---|
| "fhir has no reverse dependencies" | **REFUTED as stated.** `circuit-prove/Cargo.toml:111` deps `fhir`, and `dreggnet-surfaces/Cargo.toml:102` deps it optionally. |
| "the stack has NO deployed consumer" | **CONFIRMED.** |

The precise shape:

- `circuit-prove` deps both `fhegg-solver` (line 110) and `fhir` (line 111)
  **under `[dev-dependencies]`** — tests only, never in a shipped library.
  The manifest comment is explicit: "DEV-ONLY … Dev-dep-scoped so the library's
  normal + wasm32 builds are untouched."
- `dreggnet-market` deps `fhegg-solver` behind an **optional**
  `certified-clearing` feature (line 22/184).
- `dreggnet-surfaces` deps `fhir` behind an optional feature (line 35/102).
- `sdk` deps `fhegg-solver` behind an optional `fhegg` feature (line 59/142).
- `fhegg-fhe` deps `fhegg-solver` as a normal path dep (line 113) and again in
  dev-deps (line 132) — but `fhegg-fhe` itself has no consumer.
- **`fhegg-solver`, `fhir`, `fhegg-fhe` and `fhegg-core` are workspace
  `members` but NOT `default-members`.** A bare `cargo build` / `cargo test` in
  breadstuffs does not build any of them. The root manifest says so directly:
  kept out "so the bare dev loop stays light."
- `grep -rn "fhegg\|CertF\|cert_f" programs/ apps/` in **this** repo: no hits.
  Nothing in Dragon's Clutch touches it.

So: **no non-dev, non-optional consumer anywhere; not in the default build;
nothing on-chain.** TESTQALOG's own line 1860 agrees — "NONE is deployed" —
and names the intended deploy route (`portfolio_clear` runner +
`/offering/portfolio`) as "spec'd, NOT built."

### 5.1 The smallest honest consumption path

Ordered by increasing commitment. My assessment is that the first two are the
only ones worth serious consideration.

**Path A — ideas only, zero artifact movement (smallest).** Take the *shape*:
verify-not-find; the certificate is the interface, not the solver; the checker
recomputes everything and treats stored diagnostics as untrusted; ε is
**prescriptive** (a budget the program grants) not **descriptive** (whatever
the solver achieved) — that distinction is the single most valuable lesson in
the entire tree, and it was learned there the expensive way. Write our own
spec, attribute the prior art in prose, copy no bytes. Requires no manifest
under PROVENANCE.md §2 beyond a note that prior ideas informed a clean-room
spec.

**Path B — one certificate, re-specified, checker-only.** Pick
**`uniform_allocation_cert.rs` (`FHUAC001`)** and nothing else. It is the best
candidate in the tree by a wide margin:
- **u128 throughout, zero `f64`** — the only certificate that already satisfies
  AGENTS.md's exact-scaled-integer rule;
- a genuinely independent verifier that recomputes the volume-maximising price
  including the lowest-index tie rule, the exact cleared volume, caps,
  inactivity, both side sums, and the unique largest-remainder allocation with
  index tie-break, **without calling any finder**;
- the price grid comes from the caller, "never a certificate-selected grid" —
  i.e. the program-binding defect is already closed here;
- it depends only on `clearing.rs`'s types;
- a differential test against the live finder over exhaustive small books
  exists and **passed in my run**.
Re-specify it from the mathematics (largest-remainder apportionment is
textbook), write our own Lean-authored AIR if we want one, cite the prior art.
Still no bytes crossed.

**Path C — copy the checker slice with a manifest.** The ≈3,740-LoC subgraph in
§2.3, excluding `air.rs`, `gpu.rs`, `pdhg.rs` and every bin. Needs the full
PROVENANCE.md §3 manifest per file, an upstream refactor to be maintainable
(feature-gating, `qp.rs` split), and a decision on whether we track upstream or
fork permanently. **Cost is dominated by the ongoing divergence, not the copy.**

**Path D — take the Lean + descriptor + STARK chain.** Pulls in Lean 4.30.0,
Mathlib, the Plonky3 fork with the unmerged PR, the whole `dregg-circuit`/
`dregg-circuit-prove` tower, and a registry that today admits exactly two
public programs. This is not a consumption path; it is adopting a second
project.

---

## 6. Recommendation set

**These are options with costs, not a decision.** The AGENTS.md §1 boundary is
the user's to move, and nothing here should be read as arguing that it should
move.

### Per component

| Component | Option | Reasoning |
|---|---|---|
| **Cert-F idea (verify-not-find, prescriptive ε)** | **Fresh specification + attribution** | The idea is the valuable part and it is not ours to be shy about re-deriving — it's standard LP duality. The prescriptive-vs-descriptive-ε lesson and the "stored diagnostics never decide acceptance" discipline are cheap to adopt and expensive to rediscover. |
| **`uniform_allocation_cert.rs`** | **Fresh specification** (preferred) *or* reuse-by-copy with manifest | Best-engineered artifact surveyed; exact u128; program-bound; independently verified. But it is ~1,040 LoC of readable, textbook mathematics — re-specifying is cheaper than owning a cross-repo provenance exception forever. |
| **`qp_exact.rs` / `qp_strict.rs` / `FHQPB001`** | **Fresh specification, if we need a QP at all** | Exact i128, well-tested, Lean-pinned vectors. But it exists to serve a Markowitz portfolio product we do not have. Do not import a solution to a problem we have not got. |
| **`optimizer_protocol.rs`** | **Fresh specification — but study it first** | The *design* (bind request+session+nonce+solver identity+cert bytes, delegate to an independent checker, then consume a replay identity) is exactly right for an untrusted-worker boundary and worth reading closely before we design ours. The implementation is 1,291 LoC of our-license code we would then own. |
| **`cert.rs` / `pdhg.rs` (f64 Cert-F)** | **Do not consume** | f64 acceptance with a scaled tolerance violates AGENTS.md's exact-scaled-integer rule at the point where it matters most. If we want a Cert-F, it must be integer from the start. |
| **`fisher.rs`, `cfmm.rs`, `package.rs`, `smooth.rs`, `pricecert.rs`** | **Do not consume** | Every one carries an open, unrepaired defect from the 2026-07-17 audit — including `CertPackage`'s `valid=true` on an empty clearing, which is an accepted-but-wrong case with a live consumer path. Untouched since 2026-07-14. |
| **`fhegg-solver/src/air.rs`** | **Do not consume — flag as debt** | Hand-written Rust AIR. Violates our house rule directly, and is a second semantic owner of the Cert-F constraint set. |
| **`gpu.rs` + all 10 bins** | **Do not consume** | Solver-side. We want the checker, never the solver. |
| **`metatheory/Market/Cert*.lean`** | **Study; consume only with a full proof-dependency review** | Genuinely good work — `sorry`-free, non-vacuous keystones, generic emit-soundness, honest about SDD ⊊ PSD. But adopting them means adopting Lean 4.30.0 + Mathlib into our evidence boundary per PROVENANCE.md §3, and the theorems are stated over *their* `CertFProg`, not over anything we have. |
| **`circuit-prove/src/cert_{f,qp}_air.rs`** | **Study as the pattern; do not consume the code** | The Lean-emitted byte-pinned refutable-golden pattern is the best thing in this survey and we should copy the *pattern* into Dragon's Clutch. The code drags the Plonky3 fork and a 2-program registry. |
| **`fhegg-fhe/`** | **Do not consume — hard scope-out** | The only component with a real licence blocker: `tfhe` is BSD-3-Clause-Clear with an express patent withholding plus Zama's stated commercial-patent-licence requirement, which cannot be reconciled with AGPL-3.0 §11/§7/§12 (§3.3). It is also not on the certificate path, and its own measured envelope is minutes-to-tens-of-minutes. Read `MEASURED-ENVELOPE.md` as a model of honest benchmarking and leave the code. Scoping it out removes the entire licence question. |

### The global options

**Option 1 — do not consume anything (status quo).** Cost: we re-derive
verify-not-find ourselves. Benefit: AGENTS.md §1 stays intact with no
exception; no cross-repo provenance exception; no Plonky3 divergence; no
inherited f64 checkers or unrepaired certificates. **This is the cheapest
option and the one the current project rules already select.**

**Option 2 — reuse-by-fresh-specification (ideas + attribution).** Take Path A
and, if a batch-clearing product needs it, Path B. Human-authored clean-room
spec per PROVENANCE.md §2; the record distinguishes concept from expression.
Cost: real engineering, but engineering we would do anyway and would own
cleanly. **This is what I would put in front of the user as the realistic
option if any of this is wanted.**

**Option 3 — reuse-by-copy with a manifest.** Only coherent for the §2.3
checker slice, and only after an upstream refactor in breadstuffs makes it
extractable (feature-gate `gpu`, split `qp.rs`, lift `FlowLp`). Cost: a
per-file PROVENANCE.md §3 manifest **authored from scratch** — breadstuffs has
no SBOM, no `deny.toml`, and no licence gate to inherit — plus closing the two
notice-retention gaps before the copy, a permanent non-empty "known provenance
exceptions" entry, and a maintenance relationship with a repo we have declared
out of bounds. **The provenance cost exceeds the code value for everything
except possibly `uniform_allocation_cert.rs` — and that one is cheap enough to
re-specify.**

A fourth option exists and should be named even though it is not ours to
choose: **ask for the upstream refactor first.** If the user wants this stack
consumable at all, the cheapest sequencing is for breadstuffs to feature-gate
`gpu`, split solver from checker in `qp.rs`, lift `FlowLp` out of `pdhg.rs`,
publish the checker slice as its own crate, and close the two notice gaps.
Then the question in front of Dragon's Clutch is an ordinary dependency-
admission review of a small, exact, `serde`-only crate, rather than a
cross-repo excavation. That would take the decision from "should we copy 3,700
lines out of a prohibited repo" to "should we take this dependency" — a
question our PROVENANCE.md §3 already knows how to answer.

### Four things worth taking regardless of the decision, because they cost nothing

1. **The refutable-golden test pattern.** A byte-pin that is never tested
   against a *drifted* golden is not a pin. `the_lean_golden_pin_is_refutable`
   exercises a drifted, a truncated, a wrong, and an absent golden, and demands
   refusal of each. That is the correct shape, and we do not have it anywhere
   in Dragon's Clutch.
2. **Prescriptive ε.** A tolerance that reports what the solver achieved
   certifies nothing. A tolerance that names the budget the program *grants*,
   pinned in the descriptor, certifies something. That distinction cost them a
   production-blocking bug to learn.
3. **Stored diagnostics never decide acceptance.** Every checker in that tree
   recomputes objectives and residuals from the public program plus the
   witness, treating the certificate's own reported numbers as untrusted wire
   data — and proves it with a forged-diagnostics test. Cheap discipline,
   easily forgotten.
4. **Fail closed on an unregistered program.** `try_cert_f_descriptor` refuses
   any public program that is not a byte-pinned Lean emission, and names the
   procedure to register one. The registry being *too small* is a real ceiling
   (§1.6), but refusing rather than specializing at runtime is the right
   default and we should adopt it.

---

## 7. What this lane did not do

- Did not copy, move, or modify any file in `/Users/ember/dev/breadstuffs`.
- Did not commit anything, here or there.
- Did not rebuild the Lean `metatheory` tree; the Lean evidence above is
  source-reading plus `.olean` timestamps, not a fresh elaboration.
- Did not build or test `fhegg-fhe` or `dregg-circuit-prove`.
- Did not verify the upstream state of Plonky3 PR #1982 against GitHub; the
  "open with CHANGES_REQUESTED as of 2026-08-14" claim is the vendored fork's
  own, read from its source header, and should be re-checked before it is
  relied on.
- Did not obtain legal advice. The AGPL/BSD-3-Clause-Clear analysis in §3.3 is
  a licence-text reading to support a scoping decision, not counsel.
