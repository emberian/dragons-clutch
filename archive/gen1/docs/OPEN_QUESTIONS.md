# Open design questions

These are intentionally unresolved. Implementations must not silently select an
answer merely because a convenient code path exists.

Rows retired by ember's adoption record of 2026-08-20 are marked in place with
a pointer, not deleted — [decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md)
is the record and the reports it cites carry the counterarguments. A row marked
**Deferred** is a decision ember took *not* to decide yet, with the tension
named; it is not an open question that nobody has reached.

## P0: before kernel semantics freeze

### Failure payout and sabotage

**Ratified 2026-08-20** ([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md)
item 7): the failure-payout decision is ratified as part of the R4 terminal
ratification. Ratification closes the decision, not the runtime promotion
falsifiers, which remain open where the decision document records them.

**Decided 2026-08-19:** `EvidenceOnlyRecoveryV1` — there is no numeric
data-failure payout, because no fixed fallback vector is distribution-neutral
(the equal-sum argument: any fallback unequal to a still-possible completion
has both a gainer and a loser). A market without evidence-selected weights
degrades to recoverable dormancy; failure residue goes to the canonical SDK
incinerator, never to an interested party. See
[implementation/FAILURE_PAYOUT_DECISION_V1.md](implementation/FAILURE_PAYOUT_DECISION_V1.md).
Model-only; its runtime promotion falsifiers remain open there.

### Payout rationality and dust

**Decided 2026-08-19:** lot-scaled bearer units, not persistent remainder
credits — one outcome-token atom represents a creation-time raw-claim lot `L`
with `D | L` (first conservative profile `L = D`), every claim-separating path
preserves the lot, and the profile creates no fractional credits: an imported
nonzero credit numerator is a terminal STOP, not sweepable dust. See
[implementation/FAILURE_PAYOUT_DECISION_V1.md](implementation/FAILURE_PAYOUT_DECISION_V1.md).

### Upgrade posture

**Deferred 2026-08-20, with the tension named**
([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md), "Deferred
with the tension named"): the report recommended
immutable-at-first-deployment; ember's weakest-choice principle favors
upgradeable-then-burn, because burn is always available and un-burn never is.
Mainnet is gated regardless, and the **devnet** posture is settled by item 5 —
deployments use the sealed opt-3 identity only, with the devnet beta authority
ratified as recorded in the deploy job. The mainnet posture stays open:

Decide whether the reference deployment has a time-bounded audited beta authority
followed by irrevocable removal, or is immutable at first deployment. Source code
must support either deployment without pretending the former is the latter.

### Internal venue ownership

Decide whether issuance and simplex venue live in one immutable program or the
venue calls conservation-checking instructions on an Eggcrate-owned Position
program. A separate venue must never write Position bytes directly.

### Realm admission

**Retired 2026-08-20** ([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md)
item 4, on
[decisions/REPORT_realm-admission-and-token2022_2026-08-20.md](decisions/REPORT_realm-admission-and-token2022_2026-08-20.md)):
the V1 allowlist is **FROZEN as built** — Token-2022 base mints, extension
ceiling zero, ImmutableOwner required on the Hoard, and unknown discriminants
fail closed. Fail-closed admission is the deliberate strong choice: it preserves
future options where admission-then-exploitation would foreclose them. The
record states plainly that the DREGG dogfood mint has no executable V1 profile.
Still open and *not* covered by that item: pinning the exact Token-2022 program
artifact (register F5), and the two-synthetic-Realm demonstration below.

The original row, for reference: freeze the V1 collateral-profile allowlist.
Plain SPL Token and narrowly profiled
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
  Still open. Note that the R4 §8 reference-ownership fork that would consume a
  retention horizon is **explicitly deferred** until the provider-horizon
  evidence exists
  ([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md) item 7),
  so this row is not blocking a decided design.
- Reverse-Dutch bounty step count and measured SOL cost quantiles.
- Whether any historical provider dependency is acceptable for repair.

## P2: before simplex-auction freeze

- ~~Uncertainty-shaped versus flat-notional fee policy~~ — **retired
  2026-08-20**: the fee-base fork is decided at the level of *shape*. The
  selected V1 base shape is the additive composite `kappa*G(a,p) + kappa'*R(a)`
  — uncertainty-shaped dispersion with a price-free quotient-norm floor; flat
  and per-leg are eliminated
  ([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md) item 9,
  on [decisions/REPORT_fee-base-selection_2026-08-20.md](decisions/REPORT_fee-base-selection_2026-08-20.md);
  presented in [FEE_GEOMETRY.md](FEE_GEOMETRY.md)). **Both rates remain open**,
  as do the allowed immutable tiers and the bounds freeze, and every byte stays
  `FeeBaseV1::None`. The selection is reversible until a rate freezes.
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
