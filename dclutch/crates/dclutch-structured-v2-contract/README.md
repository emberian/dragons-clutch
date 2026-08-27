# dclutch-structured-v2-contract

Wire records and the onchain-safe execution candidate for shard-backed
Structured receipts. The pure kernel stays the sole owner of coefficient,
backing, settlement, and lifecycle arithmetic.

Three records:

- `StructuredRequestV2` — 432 fixed bytes: one action, the identities that must
  join, one receipt quantity, and the optimistic replay revision. No
  coefficient, payout, supply, or custody value appears on the wire.
- `StructuredRootV2` — 128 fixed bytes: replay revision and permanent RentCredit
  beneficiary only. The lifecycle phase is deliberately absent, because it is
  authenticated per transaction from the Market and Product terminal record and
  persisting it would create a second owner for a fact Core already owns.
- `StructuredHotCandidateV2` — the opaque bounded candidate consumed by common
  Trading Hot. It revalidates every amount against the immutable coefficients
  and exposes borrowed effects plus commit-last root bytes.

There is **no Claims child**. A Structured receipt's single backing edge points
at the exact claim-shard layer, so every Structured effect is an ordinary
Token-2022 effect on the receipt Mint or on one shard Mint. Native claim
redemption and collateral payout stay with the shard layer, which already owns
them.

Effect order is canonical: the receipt effect is first for every supply-changing
action and last for retirement (so the custody sweep runs before the Mint
closes), and the shard sweep is strictly ascending in Mint coordinate.

Evidence:

```sh
cargo test --manifest-path crates/dclutch-structured-v2-contract/Cargo.toml
cargo clippy --manifest-path crates/dclutch-structured-v2-contract/Cargo.toml \
  --all-targets -- -D warnings
```

These are pure observation tests. They are not evidence that Token-2022 executed,
that SBF account validation is correct, or that transaction rollback works.
