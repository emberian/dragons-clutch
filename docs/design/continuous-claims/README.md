# Continuous claims and passive range liquidity

Status: **RECONCILED ARCHITECTURE / MIXED IMPLEMENTATION STATUS** (2026-08-19).
This packet began as a proposal. Native degree-0--3 settlement, immutable basis
mode, exact coefficient-portfolio models, and an offline shape compiler have
since landed at separately named boundaries. Passive liquidity and atomic live
portfolio settlement have not. This directory changes no consensus bytes or
release status; the implementation notes linked below remain the evidence
owners.

Reconciliation pins: immutable native mode `3a81b38f62c172da588166dcc8b9a8f4d62cd6a3`,
occupation-v4 runtime `10c4309e90c385bf783d9aac6e06f07fc9dd051d`,
host shape compiler `1d80a1f8d4a5fefb2e90f4d6c225515b97c97503` plus its
canonical host/client artifact boundary
`3391f9b816d448f46364c9691cd07d295ba58884`, finite Lean/Rust bridge
`be8eba3815bf27e79f845d6aed006d77dfb899ef`, and pure
coefficient-portfolio seam `5efdd47e62ef900034be974e5b9f6c43f0fcef1f` plus its
entitlement correction `a489fd235a8c797f0307495bc2167fc3642fffdd`, and
bounded liquidity successor
`e58a5a674d52d481343f304de4e7a7a16fe65193`.
Recorded-resolution-only internal redemption subsequently landed at
`17f1bb6cdef256c7e48a4e823795d63c62184cb2`; its focused SBF run used a
provisional joined artifact, so final artifact attribution remains open.
The fail-closed source lifecycle ABI landed at
`60fd8bcef97a3cd6e02e327cfd7625071f8a468b`, with a non-production mock-bank
lifecycle at `ce445acdec360724068c34589ef6e67da09caf61` and final substitution
evidence at `44bed19e9db83f0691e9c27e8d61bfbab74716ec`. The default registry is
empty; those commits do not constitute a production provider release.
The production fixed-denominator evaluator, source-pinned refinement campaign,
and exact span-1--3 occupation measurements were released together at
`87d2dbd60fa13d50e4f8b9e1c3697cd680697ce3`.
The bounded direct-selection model and arrival-invariance evidence landed at
`471462f804d9018190b7d67a56685f9371b3a153` and
`14d18325d9dd2f4f72b2580f1dcd5b60abe9753b`, with account codecs at
`e360a359671bbd66dba037669bd9c705f7fec1ae`. The narrow direct source authority
landed at `2fdd7cdcc5ea7a0dcfbef9b2c8a0e0c23ed03d65`; committed real-bank evidence
`e874db13ceaf552ce37d2fc3878a2895763dd65c` proves its maximum top-three Select
hits exactly 1,400,000 CU and rolls back. The staged Direct V3 successor remains
model-only at `ef32495b6b97f6f5c5212e84dedd3cacd217b2a7`.
Resumable occupation work began as an isolated model plus layout codec at
`482b395f47fa7af5b26b34719789a407899fcfc1` and
`a6da4015b0228bae0422d96caa3d26e91da0ec3d`; route tags 22 and 32--35 plus
real-SBF evidence landed at `0e4bd51c3de62f5ae965907ff3aba036dae9607c`.
Finally, `cfea8e8c4b9306df937f509fd9fbcd3a94d1039a` makes `Endow` reauthenticate a
source release registered in the exact ELF before any collateral enters.

> **Supersession notice.** Any earlier sentence in this packet that treated a
> degree-1 basis, a highest-index residual, or generic smooth TWAP as the chosen
> protocol semantics is retired. The current native evaluator is the
> open-clamped degree-0--3 evaluator with `WEIGHT-ROUND-01`; live smooth
> resolution is either point-v3 or quantized-basis occupation-v4. Smooth TWAP
> remains a refusal.

## Decision

Dragon's Clutch now has a first-class bounded native settlement primitive. A
market freezes an open-clamped B-spline basis of degree `0..=3`; authenticated
evidence resolves the native Eggs to one exact integer simplex vector. At fixed
`N`, every admitted nonnegative coefficient vector is then exact algebra over
those native Eggs. An analytic name such as range, triangle, or proximity curve
may be exact in that finite span or only a certified approximation to its named
target.

The remaining product surfaces are not new payout semantics. They are (1) a
live authority for atomic coefficient-portfolio selection and settlement, (2)
canonical admission of a compiler artifact and its disclosed analytic meaning,
and (3) passive range liquidity. The V1 liquidity direction remains a
proof-constrained `LiquidityPolicy` that compiles an LP range into fully funded,
ordinary portfolio-shaped quotes. It is a pure model, not a live account or
batch route; its `MAX_QUOTES = 8` bound is not continuous availability. A
cost-function maker remains optional future work, admissible only as one frozen
convex potential with a checked, prepaid loss certificate.

## Subsumption boundary

For one admitted finite basis of `N <= MAX_OUTCOMES` claims:

| Useful continuous/range surface | Clutch construction |
|---|---|
| continuous outcome at a bounded resolution | Terms-frozen open-clamped basis, degree `0..=3` |
| range position | exact native coefficient vector when in span; otherwise a disclosed certified approximation |
| Gaussian/proximity payout | host-compiled rational coefficient vector plus rational error enclosure; no analytic label is committed onchain |
| graded point settlement | live native point-v3 vector for degrees `1..=3` |
| graded path settlement | quantized-basis occupation-v4 for statistic 6 or 7, with a bounded resumable route under exact measured shapes |
| smooth TWAP | deliberately refused; it is not an alias for point or occupation |
| early exit | live transfer of materialized Eggs; portfolio placement/reservation is live, while atomic portfolio settlement remains a pure seam |
| automated pricing | a narrow two-order single-Egg direct candidate authority is routed; no general portfolio or LP pricing authority |
| passive range LP | non-netted, fully funded eight-quote schedule/tranche model only; no live policy account or authority |
| concentrated depth | modeled as more separately reserved quote capacity in selected coefficient regions |

This is a parametric algorithmic inclusion, not a claim of unbounded capacity.
The landed kernel currently fixes `MAX_OUTCOMES = 16`; a market requiring more
states must pass a new account, compute, arithmetic, and proof gate. At a fixed
admitted `N`, the inclusion is strict as a coefficient language: all admitted
range and kernel coefficient vectors are members of `[0, S]^N`, while the
algebra admits every other bounded vector in that set. This is not a claim that
every such vector already has a live atomic order, wrapper, or LP lifecycle.

The label "Gaussian" or "continuous" does not enter consensus. Native consensus
currently binds Terms and the resolved basis vector. The offline compiler can
reproducibly construct rational coefficients and a conservative approximation
certificate. It now has canonical, domain-separated host certificate bytes and
a Rust decoder which recompiles them, but current Terms do not commit that
certificate digest and the onchain program does not parse it. A later admission
layer must freeze the live integer coefficient scale and onchain claim identity.
No floating-point analytic curve is silently recomputed onchain.

## Architecture and semantic owners

```text
authenticated SourceSpec + canonical sealed SourceArchive
                         |
          +--------------+----------------+
          |                               |
 admitted point statistic          per-bucket native weights
          |                               |
 point Resolution v3              occupation Resolution v4
          +--------------+----------------+
                         |
          exact native weight w in the D-simplex
                         |
       exact integer coefficients a over native Eggs
                         |
 live placement/reservation -> pure atomic portfolio seam
                         |
             modeled schedule policy / LP tranche
```

- `clutch-accumulator` owns domain-bound folding, not source authentication or
  payout choice.
- `clutch-bspline-accumulator` owns quantized native basis occupation and its
  two finalizers, not source-account authentication.
- `clutch-kernel` owns basis liabilities, exact redemption, and
  maximum-liability collateral, not venue pricing.
- `clutch-bspline` owns open-clamped degree-0--3 point evaluation and canonical
  largest-remainder quantization. Resolution v3/v4 owns the persisted vector.
- `clutch-batch` owns order eligibility, exact portfolio valuation, and asset
  conservation, not a privileged solver or market-maker formula.
- the Solana adapter authenticates bytes, accounts, sources, clocks, and CPIs;
  it remains a separately named unverified boundary.
- the host shape compiler owns construction and explanatory error certificates,
  not consensus admission or transferable claim identity.
- the modeled `LiquidityPolicy` owns quote-generation and LP-tranche rights
  only. It cannot change resolution, mint naked liabilities, or spend Hoard
  principal.

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
- simultaneous independent price formulas for the same trade;
- oracle brands in place of exact adapter/deployment/window semantics;
- floats, `exp`, `log`, or `erf` in Eggcrate; and
- a formal-verification claim without the exact theorem, source digest, pinned
  toolchain, assumptions, and unverified boundaries.

These refusals remove unsupported mechanics without reducing the legitimate
range, kernel, liquidity, transfer, or early-exit surfaces.

## Current implementation boundary

- **Live SBF semantics:** immutable degree-0--3 basis mode; degree-1--3
  point-v3 and occupation-v4 resolution; exact-lot internal and bearer
  redemption, with internal redemption reading only the canonical v2/v3/v4
  record; native Egg placement, reservation, cancellation, and transfer.
  Monolithic occupation admits no measured initial span-1--3/degree-1--3 case
  at the selected 25% CU-headroom gate. Routed `ResolutionWork` now bank-passes
  Begin/Fold/Finalize/Abort for its exact measured shapes, with measured maxima
  810,992 / 815,573 / 1,094,832 / 587,197 CU; those rows do not extrapolate to
  unmeasured spans. Its candidate ELF is not yet the repository-wide integrated
  release identity. The narrow Direct V2 Init/Freeze/Submit path executes, but
  maximum top-three Select reaches exactly 1,400,000 CU and rolls back every
  watched byte and lamport; no live Direct V2 settlement claim follows.
- **Pure/host-only semantics:** certified shape compilation and comparison to a
  categorical lowering; canonical coefficient-portfolio identity, funding,
  valuation, and paired settlement; staged Direct V3 lifecycle/work-budget
  models; and a passive-liquidity model restricted to
  one immutable owner per tranche, nontransferable accounting shares, and one
  owner-aggregated fixed-grid terminal fee allocation. These are not live
  authority claims; the liquidity verdict is scoped green only for its isolated
  model.
- **Release STOPs:** a reviewed production provider/parser registry entry;
  integrated artifact/supply-chain identity for the routed ResolutionWork
  profile and admission limited to measured shapes; canonical onchain claim
  artifact/certificate commitment; general/portfolio candidate selection and
  vector receipt; replacement or staged repair of Direct V2 Select plus empty,
  preselection, and postselection lapse; live LP accounts/authority; and
  universal source-to-SBF refinement. The default registry remains empty and
  source construction refuses, but `Endow` now also refuses with
  `SourceReleaseUnavailable` (`0x79`) before collateral custody. Success under
  the mock source feature belongs to a distinct non-production ELF.

There is no universal terminal/no-stranding claim. Outside artifact stages,
program accounts have no general close route or authenticated rent-payer versus
prefund-donation split; current outcome mints have no `MintCloseAuthority` and
are permanently unclosable. An empty frozen Direct V2 epoch or a top-three
Select stop can strand Reservations. Hoard token donations, external claim-burn
forfeiture, and native fractional fragments have no frozen terminal
disposition. None may be relabeled as protocol revenue, keeper funding, or LP
reserve.

## Documents

- [CLAIM_ALGEBRA.md](CLAIM_ALGEBRA.md) defines exact units, native
  range/kernel compilation, canonical rounding, and the
  conservation/max-liability theorem.
- [LIQUIDITY_POLICY.md](LIQUIDITY_POLICY.md) specifies the bounded non-netted
  schedule/tranche model, its live-authority STOPs, and the optional
  cost-function gate.
- [TERMS_AND_EVIDENCE.md](TERMS_AND_EVIDENCE.md) distinguishes current market,
  source, window, basis, and resolution identities from still-proposed compiler
  and liquidity identities.
- [FORMALIZATION_BACKLOG.md](FORMALIZATION_BACKLOG.md) records landed evidence
  and the remaining refinement and Solana composition work.
