# Continuous claims and passive range liquidity

Status: **PROPOSED architecture packet** (2026-08-18). This directory changes
no implementation, immutable market terms, or release claim. It is an
independent design analysis of continuous/range claims and passive liquidity; it
imports no third-party code, terms, or brand assets.

## Decision

Dragon's Clutch already contains the full bounded **finite payoff language** at
an admitted discretization: its finite basis admits every exact bounded
coefficient vector on `N` states, including ranges and sampled or integrated
kernels. The missing product surface is **passive range liquidity**, not another
settlement primitive.

The near-term design is therefore a proof-constrained `LiquidityPolicy` that
compiles an LP's range into a bounded schedule of ordinary, fully reserved
portfolio quotes. It reuses the existing claim algebra and batch conservation
relation. A cost-function maker is a later optional policy, admitted only as one
frozen convex potential with a checked loss certificate. LMSR and sigmoid
curves are never overlaid as independent prices.

## Subsumption boundary

For one admitted finite basis of `N <= MAX_OUTCOMES` claims:

| Useful continuous/range surface | Clutch construction |
|---|---|
| continuous outcome discretized into bins | exhaustive finite partition or frozen B-spline basis |
| range position | exact nonnegative portfolio coefficient vector |
| Gaussian/proximity payout | exact scaled kernel table or range-kernel matrix compiled to that vector |
| graded settlement | derived basis weight vector `w`, with `sum(w) = D` exactly |
| early exit | opposite portfolio intent in the native batch venue, or transfer of materialized Eggs |
| automated pricing | untrusted candidate constructor or registered `QuoteRelation` |
| passive range LP | fully funded schedule-compiled range policy and tranche |
| concentrated depth | more reserved quote capacity in selected coefficient/risk regions |
| TWAP settlement | exact immutable `WindowDomain` and sealed `WindowResult` |

This is a parametric algorithmic inclusion, not a claim of unbounded capacity.
The landed kernel currently fixes `MAX_OUTCOMES = 16`; a market requiring more
states must pass a new account, compute, arithmetic, and proof gate. At a fixed
admitted `N`, the inclusion is strict as a payoff language: all range and kernel
vectors are members of `[0, S]^N`, while Clutch admits every other bounded vector
in that set, finite categorical states, and admitted product partitions.

The label "Gaussian" or "continuous" does not enter consensus. The consensus
object is the exact integer artifact and its terms digest. An analytic curve may
reproducibly generate that artifact with a disclosed approximation error; it is
not silently recomputed with floating point onchain.

## Architecture and semantic owners

```text
authenticated source adapter + immutable WindowDomain
                         |
                  sealed WindowResult
                         |
          frozen statistic / ambiguity / edge policy
                         |
         exact basis weight w in the D-simplex
                         |
      exact portfolio coefficients a in [0, S]^N
                         |
  batch orders <-> schedule LiquidityPolicy <-> LP tranche
```

- `clutch-accumulator` owns domain-bound folding, not source authentication or
  payout choice.
- `clutch-kernel` owns basis liabilities, exact redemption, and
  maximum-liability collateral, not venue pricing.
- `clutch-batch` owns order eligibility, exact portfolio valuation, and asset
  conservation, not a privileged solver or market-maker formula.
- the Solana adapter authenticates bytes, accounts, sources, clocks, and CPIs;
  it remains a separately named unverified boundary.
- a `LiquidityPolicy` owns quote generation and LP tranche rights only. It
  cannot change resolution, mint naked liabilities, or spend Hoard principal.

## Canonical V1 refusals

Canonical V1 refuses:

- trader or LP leverage, margin borrowing, liquidation, and liquidation NFTs;
- insurance or a principal floor funded by expected token fees, future volume,
  governance discretion, or an adjustable post-deposit threshold;
- minting claims before maximum liability is collateralized;
- calling LP trading loss "impermanent loss" or promising it is capped absent
  separate escrow equal to the maximum promised shortfall;
- dynamic liquidity parameters that retroactively change the cost potential of
  open inventory;
- simultaneous LMSR and a second sigmoid price for the same trade;
- oracle brands in place of exact adapter/deployment/window semantics;
- floats, `exp`, `log`, or `erf` in Eggcrate; and
- a formal-verification claim without the exact theorem, source digest, pinned
  toolchain, assumptions, and unverified boundaries.

These refusals remove unsupported mechanics without reducing the legitimate
range, kernel, liquidity, transfer, or early-exit surfaces.

## Documents

- [CLAIM_ALGEBRA.md](CLAIM_ALGEBRA.md) defines exact units, range/kernel
  compilation, and the conservation/max-liability theorem.
- [LIQUIDITY_POLICY.md](LIQUIDITY_POLICY.md) specifies schedule-compiled range
  LP, `QuoteRelation`, tranche rights, and the optional cost-function gate.
- [TERMS_AND_EVIDENCE.md](TERMS_AND_EVIDENCE.md) freezes market, source,
  window, compiler, rounding, and liquidity identities.
- [FORMALIZATION_BACKLOG.md](FORMALIZATION_BACKLOG.md) maps the proposal into
  `clutch-kernel`, `clutch-accumulator`, and `clutch-batch`.
