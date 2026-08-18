# Open design questions

These are intentionally unresolved. Implementations must not silently select an
answer merely because a convenient code path exists.

## P0: before kernel semantics freeze

### Failure payout and sabotage

How should a Market settle when authenticated evidence is incomplete after the
repair window?

- Separate invalid-data Egg makes the failure incentive directly tradeable.
- Equal payout over all outcomes can transfer nearly the full Hoard to cheap tails.
- Compatible-outcome payout preserves partial authenticated information but
  requires a precise monotone compatibility algebra.
- Delayed refund is not neutral once individual Eggs have circulated.

Required result: finite payout-vector set, exact divisibility/remainder behavior,
common-mode exposure cap, and adversarial payoff analysis.

### Payout rationality and dust

Choose between redemption lots and persistent remainder credits. Prove that every
collateral atom remains attributable and no treasury dust rule silently changes
the payoff.

### Upgrade posture

Decide whether the reference deployment has a time-bounded audited beta authority
followed by irrevocable removal, or is immutable at first deployment. Source code
must support either deployment without pretending the former is the latter.

### Internal venue ownership

Decide whether issuance and simplex venue live in one immutable program or the
venue calls conservation-checking instructions on an Eggcrate-owned Position
program. A separate venue must never write Position bytes directly.

### Realm admission

Freeze the V1 collateral-profile allowlist. Plain SPL Token and narrowly profiled
Token-2022 collateral may have different parsing/CPI requirements. Decide whether
transfer-fee, transfer-hook, interest-bearing, confidential, rebase-like, or
freezable collateral is rejected categorically. Demonstrate generic semantics with
two synthetic Realms; DREGG must not create a special branch.

### Partition and payoff compiler

Freeze the V1 Statistic/Partition closed enum, canonical boundary representation,
unit algebra, and Template equivalence rule. Decide whether empty cells are
rejected and whether portfolio coefficients are arbitrary bounded integers or an
audited product subset. Prove that human labels cannot change semantic identity.

## P1: before accumulator implementation

- Exact initial source presets for the house DREGG Realm and other long-tail
  Solana-token Templates.
- Whether DREGG/PumpSwap has any acceptable cumulative native history source; this
  affects DREGG market Templates, not collateral-generic issuance.
- Pyth, Raydium CPMM/CLMM, Meteora DLMM adapter versions and upgrade binding.
- Security tiers, per-feed exposure limits, and multi-source aggregation.
- Supported monoidal feature family and what is explicitly not derivable.
- Archive page size, retention horizon, recycling proof, and Window cache identity.
- Reverse-Dutch bounty step count and measured SOL cost quantiles.
- Whether any historical provider dependency is acceptable for repair.

## P2: before simplex-auction freeze

- Uncertainty-shaped versus flat-notional fee policy and allowed immutable tiers.
- Definition of standing maker: at least one full frozen Epoch is the leading
  candidate.
- Treatment of same-Epoch crossings and self-crosses.
- `PRICE_SCALE`, exact simplex normalization, integer price bounds, and tie rule.
- Exact admitted portfolio-intent language: proportional divisibility, partial
  fills, limit semantics, and maximum coefficient/term count.
- Candidate public score and whether small-book exact-rational or primal/dual
  certificates can establish optimality for a restricted fragment.
- Candidate replacement window, proposer bond, withholding resistance, and the
  distinction between best submitted and globally optimal.
- Complete-set virtual split/merge equations and fee treatment.
- Pro-rata remainder distribution and resistance to order splitting.
- Dense page size/shard count and preallocation versus incremental realloc.
- Cancellation cutoff and invalid-proposal bond.
- Manifest adapter surface and how the client labels external per-Egg prices that
  need not form a coherent simplex.
- Optional commit/reveal and its non-reveal/refund behavior.

## P3: before public release

- Bare versus immutable in-mint metadata for outcome tokens.
- Static-client framework and wallet adapter with no hosted backend.
- IPFS pinning diversity and canonical release-manifest location.
- Program upgrade and verified-build UX.
- Dependency, license, AGPL source-offer, and SBOM review.
- Independent security audit and public disclosure process.
- Legal/regulatory review for public real-money deployments, clearly separated
  from the correctness of publishing open-source code.
- Whether the project publishes only code or also a reference devnet deployment.

## Explicit future research, not V1 dependencies

- Succinct proofs for sublinear batch verification.
- Richer combinatorial basket clearing and certified LP/integer optimization.
- Commit/reveal, MPC, FHE, vFHE, or proof-carrying confidential orders.
- Cross-market collateral netting.
- Arbitrary source predicates or Turing-complete payoff DSLs.
- Subjective resolution/dispute systems.
- Native deployment on a future DREGG chain.
