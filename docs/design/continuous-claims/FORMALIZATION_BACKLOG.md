# Continuous-claims formalization backlog

Status: **PROPOSED execution map**. Passing an earlier stage does not imply a
later stage. No mainnet, audit, or formal-verification claim follows from this
document.

## C0 — Freeze semantics

- Select the degree-1 finite basis and exact denominator/rounding variants.
- Freeze `ClaimArtifactV1` and `LiquidityPolicyV1` canonical bytes.
- Define payoff approximation, edge, gap, fee, tranche, and withdrawal policies.
- Record rejected alternatives: dual LMSR/sigmoid, mutable per-bin depth, VaR
  solvency, and uncapitalized insurance.

Exit: two independent readers produce identical canonical vectors and digests.

## C1 — Pure exact models

- Implement dependency-light claim compiler and tranche reserve model.
- Generate hard-range, triangular, capped-linear, and Gaussian-table goldens.
- Add dynamic-depth, split-position, correlated-loss, and rounding falsifiers.
- Differentially compare independent compiler implementations.

Exit: exhaustive bounded tests and randomized larger-domain tests close with no
unclassified divergence; all refusals are state-atomic.

## C2 — Kernel and batch refinement

- Join derived basis vectors to exact terms/evidence receipts.
- Add schedule-compiled portfolio quotes as an unprivileged batch input family.
- Prove per-outcome, collateral, fee-pot, tranche, and ownership-phase closure.
- Preserve the selected-candidate—not global-optimum—claim boundary.

Exit: vertical traces cover deposit, quote, partial fill, cancel, opposite flow,
withdrawal refusal, resolution, and redemption without protected-pool leakage.

## C3 — Verus executable proofs

Target the small safe-Rust/no-allocation kernels:

- basis nonnegativity and exact sum;
- maximum-liability collateral theorem;
- claim compiler bounds/rounding;
- tranche reserve preservation;
- fee-pot conservation and refusal transactionality; and
- batch portfolio conservation at frozen array bounds.

Keep Solana account parsing, CPI, runtime ownership, clock/source authentication,
and deployment outside the proved kernel and enumerate them in the TCB. Run the
dual-toolchain spike before promising the Verus source can also produce the SBF
artifact.

## C4 — Rocq economic state machine

Hand-write the authoritative mathematical transition system and prove:

- complete-set and partition-of-unity identities;
- global solvency across every reachable transition;
- no principal flow to fees/liveness/insurance;
- exact tranche share and withdrawal bounds;
- one-shot resolution/redemption and retry idempotence; and
- refinement from portfolio claims to primitive Eggs.

Use rocq-of-rust only as a shadow translation/refinement experiment until its
Solana/runtime axiom boundary is mature. `Admitted`/axiom counts are release
artifacts, not footnotes.

## C5 — Solana composition

- Freeze layouts, owner/signer/alias/replay checks, PDA seeds/bumps, and Token
  program/profile identities.
- Implement account creation and Token CPI only behind the proved pure relation.
- Benchmark transaction bytes, accounts, locks, trace depth, CU, rent, and
  priority-fee sensitivity at `n = 2,4,8,16`.
- Exercise devnet only after offline gates and explicit authorization.

Exit: reproducible build plus exact program/IDL/layout/deployment manifest;
independent security review; no unresolved STOP finding.

## C6 — Optional cost-function maker

Only after schedule liquidity works:

- select one regularizer over the actual payoff polytope;
- prove convexity, coherence, cash invariance, and bounded loss;
- compile canonical integer endpoint charges with persistent carry;
- capitalize loss budget before activation; and
- prove immutable or value-balanced liquidity-parameter transitions.

Exit: independent red team cannot produce circulation, dynamic-depth extraction,
uncapitalized loss, or withdrawal insolvency at the admitted bounds.

## C7 — Static client and release evidence

- GitHub Pages/IPFS build with no server dependency;
- local verification of terms, program, upgrade authority, account bytes, and
  receipts;
- multiple RPC endpoint support without treating RPC as consensus;
- accessible transaction previews and human-readable exact-risk displays; and
- source/tool/font/build/artifact manifest for every published document and
  binary.

Deployment, filing, regulator contact, key use, and public release remain
separate explicit human gates.
