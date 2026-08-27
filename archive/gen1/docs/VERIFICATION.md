# Verification architecture

Substrate of record, 2026-08-20: **Lean**
([adr/0005-lean-proof-substrate-of-record.md](adr/0005-lean-proof-substrate-of-record.md),
adopted per [decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md)
item 2, superseding ADR-0003). This document was written against ADR-0003's
assignment — Verus as the executable-kernel gate, Rocq as the independent
shadow — and its §2 and §7 describe a Rocq track whose role is now **retired**.
Those sections are kept as the historical specification of what was planned,
not as live obligations; `rocq/ClutchKernel.v` stays in-tree on the same basis,
and its manifest typecheck gate stays labeled non-proof-content. Read §3's
target list as Lean's, with Verus retained only where a result verifies an
actual executable body. What each substrate holds today is stated in
[EVIDENCE_MATRIX.md](EVIDENCE_MATRIX.md) §1.

## 1. Claim discipline

Dragon's Clutch does not use “formally verified” as a binary marketing adjective.
Every result names:

- the exact property;
- the exact source/specification digest;
- the verified boundary;
- the assumptions and trusted components;
- the tool/version/configuration;
- the unclosed refinement arrows.

The practical V1 stack is complementary:

```text
hand-written Rocq model
  protocol algebra and reachable-state theorems
                 |
                 | named transitions + shared vectors
                 v
Verus-verified Eggcrate Rust
  executable pure transition kernel
                 |
                 | total Result values / CPI intents
                 v
minimal native Solana adapter
  bytes, accounts, PDA/signers, persistence, Token-2022 CPIs
                 |
                 | pinned dual-toolchain build + integration tests
                 v
deployed SBF ELF and Solana runtime
```

`rocq-of-rust` is a post-freeze shadow refinement track intended to tighten the
first correspondence arrow. It is not a V1 release blocker.

## 2. Hand-written Rocq model

**Retired role, kept as historical specification** (ADR-0005). Rocq holds zero
theorems — its obligations are `Definition … : Prop`, never `Theorem` — and one
conjunct of `successful_transition_is_well_formed` is machine-checked vacuous.
The mathematical-model role described below is Lean's now; the property content
carries over unchanged, only the substrate moves. Nothing in this section is a
live obligation.

The Rocq specification should be mathematical rather than Rust-shaped. It defines:

- Realm/Market lifecycle;
- canonical finite partition compilation and unique state-cell selection;
- basis assets and bounded payoff-vector portfolios;
- complete-set issuance and merge;
- internal/materialized supply correspondence;
- finite payout-vector sets;
- Hoard, fee, liveness, and rent separation;
- accumulator coverage and ambiguity states;
- simplex-auction reservation, candidate, verification, and settlement phases.

Target theorems include:

1. every accepted partition is canonical, exhaustive, and disjoint;
2. every ordinary observation selects exactly one partition cell;
3. every admitted payoff portfolio remains within its stated collateral bound;
4. every reachable protocol state is well formed;
5. every collateral-changing transition preserves maximum-liability solvency;
6. materialization and dematerialization preserve total outcome supply;
7. fees cannot enter or leave claimant principal;
8. booked liveness cannot be withdrawn or double-paid;
9. a complete Clutch remains mergeable before resolution;
10. every terminal state permits bounded settlement;
11. accumulator failure cannot give a resolver discretionary outcome choice;
12. simplex settlement cannot double-count reserved assets;
13. accepted prices normalize exactly and candidate fills conserve each asset;
14. integer remainder rules conserve every atom.

Extract the executable Rocq model to an independent test oracle. Canonical vectors
and randomized traces should produce identical outputs in Rocq, ordinary host
Rust, Verus-checked Rust, and SBF integration tests. *(The Rocq extraction
oracle was never built and is not planned; under ADR-0005 the shared-vector
discipline is Lean's — `lean/README.md` records the correspondence as manual,
unproved, and bounded by the semantic vectors both sides evaluate.)*

## 3. The Eggcrate kernel gate

**Lean is the proof substrate of record for this kernel's obligations**
(ADR-0005); **Verus is retained solely for checked-Rust-subset results
verifying actual executable bodies** under digest-pinned contracts, which is
where the one production-bound result below lives. The kernel should be:

- `no_std`, `no_alloc`, safe Rust;
- edition 2021 for the shared Verus/SBF subset;
- fixed arrays/slices and explicit enums;
- unsigned fixed-width financial quantities;
- bounded `u128` intermediates;
- manual fixed-layout codecs;
- free of Solana SDK, Token-2022, oracle SDK, serialization framework, crypto
  dependency, and target-specific execution branch.

The public shape is conceptually:

```rust
pub fn apply(state: State, input: Input) -> Result<Transition, Error>
```

Every untrusted condition is checked by executable code. Exported functions have
no proof-only precondition that the unverified adapter must remember.

The eleven properties below are the kernel's proof obligations. Under ADR-0005
they are **Lean's to close**, as PROVED-MODEL results with the Rust
correspondence disclosed; the property content is unchanged and only the prover
moved. Measured against this list, Verus covers roughly **1.5 of the 11** — the
transfer-arithmetic result recorded below, plus part of the arithmetic-safety
bullet — and that is the honest coverage number, not a pending one:

- successful transitions preserve all local invariants;
- arithmetic cannot overflow, underflow, divide by zero, or silently truncate;
- fixed-array indices and lengths are safe;
- admitted partition boundaries are canonical and uniquely classify values;
- payoff dot products are bounded and round only at the frozen boundary;
- fee and rebate allocation conserves the collected fee;
- the simplex-dispersion fee is translation invariant under complete-set addition,
  homogeneous before carry rounding, relabeling symmetric, and invariant to
  identical-payoff partition refinement;
- accumulator boundaries and states advance monotonically;
- resolution is unique and terminal;
- codecs reject malformed bytes and round-trip canonical states;
- each emitted CPI intent is consistent with the proved logical transition.

### Current production-bound result (2026-08-18)

One deliberately narrow part of that target is checked now. Pinned Verus
verifies the exact executable body in
`crates/clutch-kernel/src/transfer_arithmetic.rs`: under the executable caller's
`quantity <= from` gate, a successful internal-claim transfer subtracts and adds
the same `u64` quantity, conserves the two balances as mathematical integers,
and reports receiver overflow precisely. Underflow and the defensive
conservation refusal are proved unreachable for this helper. Two independent
semantic mutations must fail its postcondition.

This is not a proof of all of `MarketState::transfer_internal`. The caller's
error mapping and delayed writes are source-digest-bound and reviewed, but not
Verus-checked. Shape and phase checks, position/account identity, the other
kernel transitions, the native adapter, Token-2022, SBF code generation, and
the Solana runtime remain outside the result. The machine-readable record and
assumptions are in `verus/kernel/TRANSFER_REFINEMENT.json` and
`verus/kernel/TRANSFER_ASSUMPTIONS.md`.

The scalable auction search algorithm is not automatically inside this proof
claim. V1 verifies candidate feasibility, conservation, score, and deterministic
selection. It may call the winner the best valid candidate submitted during the
proposal window; it may call it globally optimal only if a separately checked
optimality certificate closes that statement.

Official references:

- [Verus project](https://github.com/verus-lang/verus)
- [Verus overview and trust boundary](https://verus-lang.github.io/verus/guide/overview.html)
- [Calling verified code from unverified Rust](https://verus-lang.github.io/verus/guide/call-from-unverified-code.html)
- [Integer arithmetic](https://verus-lang.github.io/verus/guide/integers.html)
- [Nonlinear arithmetic](https://verus-lang.github.io/verus/guide/nonlinear.html)
- [Assumed-specification warning](https://verus-lang.github.io/verus/guide/reference-assume-specification.html)

## 4. Prohibited proof shortcuts

First-party kernel code and proof closure must reject:

- `unsafe`;
- FFI;
- `assume`, `admit`, or project axioms;
- `external`, `external_body`, or `assume_specification`;
- executable behavior gated on `cfg(verus_only)`;
- verifier focus/filter modes in release evidence;
- unbounded casts or target-dependent financial `usize`;
- an unreviewed trusted-specification dependency.

`vstd`, Verus, Z3, and the Rust frontend remain trusted components. A mechanical
CI audit must enumerate every occurrence and dependency addition rather than
relying on code-review memory.

## 5. Dual-toolchain problem

At the time of this design, the current Verus release uses an upstream Rust
frontend newer than Anza's SBF compiler toolchain. Verus verifies source semantics
under one compiler and ordinary Cargo/SBF build the erased source under another.
There is no documented Verus pipeline that directly emits or verifies the SBF ELF.

Therefore V1 must restrict Eggcrate to the common conservative Rust subset and
run a falsifying dual-toolchain spike before architecture commitment:

1. verify the exact source under pinned Verus;
2. compile the erased source under pinned ordinary Rust;
3. compile the same source under pinned `cargo-build-sbf`;
4. compare all canonical and randomized vectors on host and through
   `solana-program-test`;
5. record proof, source, generated artifact, and ELF digests;
6. reproduce the SBF build independently;
7. compare CU, stack, heap, and ELF size against an unannotated equivalent;
8. mutate the fee equation and collateral transition and require verification to
   fail.

Reject the single-source approach if SBF compilation requires divergent executable
branches, first-party assumptions, public unchecked preconditions, or materially
different runtime behavior.

## 6. Solana adapter boundary

The native adapter is intentionally small but not called verified. It owns:

- hostile instruction-byte parsing;
- exact account count/order and duplicate-alias rejection;
- owner, executable, signer, writable, PDA, and stored-bump validation;
- Realm collateral profile and canonical outcome mint validation;
- source account/program/deployment checks;
- Clock/slot and replay-domain checks;
- CPI construction, signer seeds, and return-data checking;
- checked persistence of the kernel-produced transition.

The kernel returns explicit logical writes and CPI intents. The adapter may not
invent economic values. Adversarial program tests, fuzzing, Miri where applicable,
reproducible SBF builds, and independent audit cover this boundary.

Token-2022 and source-program behavior are external assumptions. Proving that the
adapter constructs the intended CPI does not prove the callee or Solana runtime.

## 7. rocq-of-rust role

**Retired with the Rocq role** (ADR-0005). The model-to-source arrow this
section describes is now carried by the **Aeneas/Charon spike** — one pure
kernel function, bounded, with a kill criterion — which the ADR names as the
test of whether Lean can acquire the refinement arrow, alongside solanalib sBPF
scoping as the runtime-plane road. The recorded fallback stands: if the spike
fails, checked-executable-body growth continues in Verus, never by relabeling
model theorems. The description below is kept as the historical assessment of a
route not taken.

[`rocq-of-rust`](https://github.com/formal-land/rocq-of-rust) translates Rust
compiler representations into a Rocq shallow embedding, after which types/traits
are linked and hand-written simulations are proved equivalent. It does not extract
deployable Rust or verify the final Solana binary.

It is active and includes translations of portions of the Anza SDK and SPL Token
programs, but remains work in progress, is pinned to a nightly compiler, and the
existing Solana proof material contains admitted/axiomatized obligations. It is
therefore useful as a post-freeze independent refinement experiment, not the V1
critical path.

Promotion to a release gate would require:

- exact frozen Eggcrate source/config digest;
- zero unsupported-expression markers;
- zero `Admitted` in the dependency closure;
- zero unapproved `Axiom` or `Parameter`;
- a closed equivalence proof for every transition;
- a closed refinement to the hand-written Rocq model;
- reproducible pinned generation and proof checking;
- an audited assumption manifest.

## 8. Trusted computing base

Even after successful V1 proofs, the claim depends on:

- correctness and completeness of the top-level specifications;
- Rocq kernel and definitions for Rocq theorems;
- Verus translation, vstd trusted specifications, Z3, and its Rust frontend;
- manual correspondence until rocq-of-rust refinement closes;
- Anza's distinct Rust/LLVM SBF compiler;
- adapter account/source checks;
- Token-2022 and oracle/DEX program behavior;
- Solana loader, VM, consensus, and runtime;
- deployed ELF matching the audited reproducible build;
- static client not misleading users about program/version identity.

The release README and UI must present this list, not merely a verification badge.

## 9. Release evidence

Archive and publish:

- source, Cargo, Verus, Rocq, Z3, Anza, and Solana version pins;
- full proof/check logs and theorem inventory;
- trust/assumption audit;
- canonical cross-runtime vectors;
- property/fuzz/mutation results;
- SBF benchmark matrix;
- reproducible-build recipe and ELF hash;
- independent audit reports and unresolved findings;
- exact program-data and upgrade-authority status for any deployment.
