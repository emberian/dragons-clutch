# Rust crates

This directory contains offline `no_std` crates
(updated 2026-08-23; none is a deployment or release claim —
`CURRENT_TRUTH.md` supersedes status language here):

- `clutch-kernel` — pure `no_std` collateral-generic complete-claim transition
  kernel (split/merge, materialize/dematerialize, finite resolution, exact
  per-Egg/vector redemption, beneficiary-free claim/collateral donation).
- `clutch-accumulator` — pure `no_std` interval-summary monoid (coverage,
  extrema, exact price-time integrals, TWAP); unsupported statistics refuse.
- `clutch-batch` — pure `no_std` fixed-capacity transparent relation
  (selection, deterministic pro-rata allocation, conservation checks).
- `clutch-bspline` — pure `no_std` exact degree-zero through degree-three
  open-clamped payout-basis evaluator. It owns basis evaluation only; evidence
  authentication and account/runtime binding remain adapter obligations.
- `clutch-bspline-accumulator` — joins the basis evaluator to the interval
  accumulator for windowed smooth-claim evidence.
- `clutch-price-measure` — exact continuous Bernstein witnesses and a separate
  support-bounded certificate for the integer-coordinate, largest-remainder
  quantized payout body. Neither checker is wired into SBF.
- `clutch-general-v2-contract` — dependency-free fixed codecs, identities,
  funding compartments, selection rank, and lifecycle contracts for the
  disabled General V2 account family.
- `clutch-general-v2-runtime` — executable pure-core join from immutable
  Product V2 bodies and a sealed General V2 feed through exact quantized
  degree-two/three price coherence, owner-blind RelationV2, and ScoreV2-Q, plus
  a deterministic fixed-memory builder for owner-blind page projection, exact
  atom/price construction, bounded fill search, and canonical CandidateFeedV2
  serialization. Search completeness is scoped to its named heuristic family;
  it persists no verdict and activates no SBF capability.
- `clutch-liveness` — the host-side liveness/fee-carry kernels
  (`IntentFeeCarry`, `TreasuryServiceLedger`) backing the liveness policy
  profile and the revenue seams.
- `clutch-structured-claim` — exact rational coefficient realization,
  complete-set-compressed native backing, flat wrapper composition, and
  transactional transferable-claim custody/lifecycle semantics.
- `clutch-structured-claim-runtime-contract` — the exact wrapper descriptor,
  deployment/basis identity reconstruction, full-vector wrap/unwind,
  compaction/redemption/retirement, and atomic Position cash/native-Egg
  transfer contract for a future small SBF/Token-2022 adapter.
- `clutch-owner-settlement` — exact owner-aggregated General V2 receipt/cash
  realization across several single or portfolio orders at one named rounding
  boundary.
- `clutch-client-contract` — shared untrusted client provenance, intent-registry
  linkage, and fail-closed settlement-shape classification. It owns no
  persisted protocol fact.
- `clutch-product-series` — strict fixed-layout product/basis/evidence-recovery,
  MarketInstance, recurring Series, attachment, and funding identities plus
  pure schedule/debit projections. It depends only on reviewed no-default-feature
  `sha2`; account, source, collateral, Clock, and token authentication remain
  adapter obligations.

New crates record their semantic owner, dependency direction, toolchain
compatibility, and license/provenance at introduction (each README does).

Proposed boundaries are listed in [the engineering plan](../docs/ENGINEERING_PLAN.md).
Eggcrate must remain `no_std`, `no_alloc`, safe Rust, fixed-layout, total, and free
of Solana, Token-2022, oracle, CPI, FFI, and dynamic-allocation dependencies.

The first implementation should be the smallest E1 falsifier, not a complete
workspace generated in advance of the Verus/SBF compatibility decision.
