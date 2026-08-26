# dclutch-fractional-claim-kernel

This standalone `no_std`, `no_alloc`, safe Rust kernel defines the exact
categorical claim-shard successor. For every outcome while a Market is open,
and for the selected winning outcome after terminal resolution:

```text
shard_supply_i = denominator * locked_native_claims_i
```

One named quotient/remainder boundary converts an arbitrary selected shard
amount into whole native claims plus explicit change. Only the whole multiple
is burned. Change remains the same ordinary Token-owned, transferable shard
instrument; it is never a wrapper credit or a second balance ledger.

After an authenticated terminal result, winning shards still redeem only in
whole-denominator multiples. Losing shards may burn for zero individually.
Once every shard Mint supply is zero, retirement burns the remaining
zero-payout native claims and permits the projection to become empty.

The kernel hostile-decodes immutable terms and an adapter-owned runtime-width
projection, then prepares exact effects for wrap, ordinary transfer, open
unwrap, terminal redemption, losing-shard burn, and retirement. It does not
parse Solana accounts, own replay state, persist balances, call Token-2022, or
authorize a second Claims writer.

`DClutchSemantics.FractionalClaimV1` proves denomination preservation, exact
quotient/remainder decomposition, bounded winning redemption, and losing-burn
conservation. It also owns the fixed byte layout emitted into
`src/generated_abi.rs`. The finalized terms Record uses schema preimage
`dclutch/schema/fractional-claim-terms-v1`; the immutable capability selects
that schema and the SHA-256 identity of the exact terms bytes.

Evidence:

```sh
cargo test --manifest-path crates/dclutch-fractional-claim-kernel/Cargo.toml
cargo clippy --manifest-path crates/dclutch-fractional-claim-kernel/Cargo.toml \
  --all-targets -- -D warnings
crates/dclutch-fractional-claim-kernel/check-generated.sh
```

This is semantic-kernel evidence. Token-2022/Custody composition, SBF verifier,
rollback, rent, packet, CU, operator, and frontend evidence remain separate
physical work.
