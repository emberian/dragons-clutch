# Bounded proof arguments

These are executable-model arguments, not machine-checked proofs or a formal
verification claim.

## Bounds

The witness admits at most four outcomes, four Positions, eight registered
bearer accounts, eight claimant credits, and twenty-two rent-role slots.
Every collateral, rent, keeper, supply, weight, and denominator scalar admitted
by ordinary construction is at most `10^12` atoms. Rights products use `u128`:

```text
quantity * weight <= 10^24
four-outcome sum   <= 4 * 10^24
```

This is far below `u128::MAX`. Arithmetic that can approach a type or policy
boundary uses checked operations; the remaining Euclidean-division additions
have explicit `< D` and bounded-recipient preconditions. Codec offsets and
narrowing conversions are checked. Aggregate rent totals are bounded by
`22 * 10^12`, below `u64::MAX`.

## Transition atomicity

Market transitions validate current state, copy it, mutate the copy, increment
the expected nonce, validate the result, and commit only on success.
Post-terminal credit transitions do the same with the CreditVault nonce. Tests
compare the complete model before and after stale/mismatched refusals.

## Supply

Validation recomputes internal supply from Positions and external supply from
registered bearer accounts. For each active outcome it requires:

```text
SupplyTruth.internal == recomputed I_i
SupplyTruth.external == recomputed E_i
MintState.authoritative_supply == E_i
```

Observed third-party burn reconciliation requires a complete canonical bearer
vector for one outcome and accepts only when the sum of its positive deltas
equals the authenticated mint-supply delta. Redemption additionally binds the
expected token account and exact post-burn account/mint values. Per-outcome
`redeemed + direct_burned + internal + external == complete-set issuance`
prevents weighted-value conservation from hiding raw supply reassignment.

## Rights

Complete-set issuance adds one collateral atom and equal raw claim quantity to
every outcome. Since `sum(w_i)=D`, issuance adds equally to both sides of:

```text
D*N = Q + K + D*P + B + X
```

Redemption uses Euclidean division, so
`r + q*w_i = D*p + r'`; moving a claim to payout and residue preserves the
equation. Direct burn moves `q*w_i` from `Q` to `B`. Credit forfeiture moves its
numerator from `K` to `X`. Credit transfer preserves `K` except when a whole
atom is paid, where both sides fall by exactly `D*p`.

## Assets and terminal slack

Hoard and CreditVault backing each have local ingress/disposition equations.
CreditVault unsolicited-token donations have a second disjoint
`donations_in = live + sunk` equation. Sealing
transfers exactly `ceil(K/D)` atoms, so the vault is solvent and its slack is in
`[0,D)`. Surplus disposal and credit forfeiture never draw from rent or keeper
ledgers. Combining rights and asset conservation yields the terminal slack
equation checked by `terminal_slack_equation`.

## Rent/replay

Each role index has at most one record. Closed records preserve their principal
and donation provenance while live balance becomes zero. Permanent records can
never close. The tombstone captures exact terminal role bitsets and totals;
later external-owner credit closure may only move a snapshotted open role to
the closed set and increase cumulative refund/sink totals. Replay identity,
market, generation, receipt, and final market nonce remain fixed.

## Explicit non-proofs

The model does not prove PDA uniqueness, Token-2022 behavior, oracle truth,
transaction atomicity across CPIs, rent sysvar values, compute feasibility,
concurrent account-lock behavior, signature authority, upgrade safety, or
legacy data classification. The fixed arrays do not prove exhaustive live
bearer discovery. The tests are bounded campaigns and regression witnesses,
not universal formal verification.
