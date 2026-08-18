# Security policy and threat model

Dragon's Clutch is pre-implementation and has no deployed funds. Do not send funds
to any address claiming to be an official deployment unless a future release
manifest in this repository explicitly identifies it and its verification status.

## Security objectives

- No reachable state owes more collateral than its Market Hoard holds.
- No non-claim instruction can spend Hoard principal.
- No Market term, source, payout, fee, or rounding rule changes after activation.
- No authority can freeze, seize, or arbitrarily mint an Egg.
- Every admitted state partition is canonical, exhaustive, and disjoint.
- Every accepted payoff portfolio is bounded by its frozen collateral semantics.
- Every final simplex candidate conserves collateral and each Egg and satisfies
  every filled intent's exact limit.
- Every paid work item is unique, bounded, and paid at most once.
- Every liveness obligation is capitalized before admission.
- Every resolution follows authenticated evidence and a deterministic rule.
- Every static client remains replaceable and non-authoritative.

## Primary adversaries

- Malformed or aliased accounts and instruction bytes.
- False signer/PDA/owner assumptions.
- Token extension or decimal mismatch.
- Malicious Realm profiles, fee-on-transfer collateral, extension upgrades, and
  assumptions accidentally specialized to the house DREGG Realm.
- Overlapping, gapped, unordered, unit-confused, or byte-ambiguous partitions.
- Payoff-vector overflow, repeated rounding, and coefficients that exceed the
  promised bound.
- CPI account substitution or confused-deputy calls.
- Arithmetic overflow, rounding capture, and payout dust.
- Direct external burns and unusual Token-2022 behavior.
- Oracle/pool upgrade, stale data, confidence failure, thin-pool manipulation.
- Observation censorship, bounty theft, duplicate work, and priority-fee griefing.
- Failure-outcome sabotage by economically interested claim holders.
- Order withholding, cancellation races, invalid clearing proposals, self-crosses,
  wash rebate farming, and marginal-fill splitting.
- Portfolio-intent conflicts, incomplete-set creation, candidate score gaming,
  solver cartel/withholding, and an inferior submitted candidate presented as a
  mathematical optimum.
- Writable-account hotspots and denial through lock contention.
- Static-client supply-chain compromise, RPC equivocation, phishing manifests,
  and cluster/program confusion.
- Upgrade-authority or deployment-key compromise.

## Disclosure

A private reporting address and coordinated-disclosure process will be added before
any public test deployment. Until then, report findings through a private channel
agreed directly with the repository owner; do not assume a social-media account or
token chat is authenticated.

No bounty exists until explicitly announced and funded. Researchers should use
local validators and synthetic assets only.
