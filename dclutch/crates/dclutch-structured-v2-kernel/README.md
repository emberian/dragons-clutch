# dclutch-structured-v2-kernel

This standalone `no_std`, `no_alloc`, safe Rust kernel defines Structured
receipts backed by **exact claim shards**. For receipt supply `S`, backing
coefficient `c_i`, and Structured shard custody `K_i`:

```text
K_i = S * c_i      for every representation coordinate i
```

Because the claim-shard layer already denominates one native claim into `D`
transferable atoms (`F_i = D * C_i`), a receipt atom denotes exactly `c_i / D`
native claims. Structured V1 could only admit a recipe whose least realization
lot equalled the Product denominator; Structured V2 removes that restriction
without a residual credit, a remainder ledger, or any rounding.

The representation graph is the deliberately finite depth-two chain

```text
Structured receipt -> exact claim shard -> native Position -> Market liability
```

Each node has one supply owner, one backing edge, and a strictly decreasing
rank, so a receipt can never be backed by a receipt. The kernel enforces the
physical form of that rule by refusing terms whose receipt Mint aliases any
shard Mint.

Structured owns **no quotient/remainder boundary**. Terminal settlement derives
every row through `dclutch_fractional_claim_kernel::divide_exposure_shards_v2`,
and a sub-denominator remainder stays an ordinary transferable shard atom of the
same Mint.

A Structured shard custody account is an ordinary Token account, so anyone may
donate into it. The projection therefore requires only solvency
(`observed >= required`) and names the difference `surplus_shard_custody`. No
plan reads, spends, or distributes it, and retirement refuses while it is
nonzero. Sweeping a donation is deliberately absent: it would need its own
authority and beneficiary argument.

`DClutchSemantics.StructuredV2` proves conservation, exact backing preservation,
replay protection, rank-decreasing acyclicity, terminal-zero honesty, and change
aggregability, and owns the fixed byte layout emitted into `src/generated_abi.rs`.
Finalized terms use schema preimage
`dclutch/schema/structured-receipt-terms-v2|...`.

The kernel does not parse Solana accounts, own replay state, persist balances,
call Token-2022, or authorize a second Claims or Custody writer.

Evidence:

```sh
cargo test --manifest-path crates/dclutch-structured-v2-kernel/Cargo.toml
cargo clippy --manifest-path crates/dclutch-structured-v2-kernel/Cargo.toml \
  --all-targets -- -D warnings
crates/dclutch-structured-v2-kernel/check-generated.sh
```

This is semantic-kernel evidence. Token-2022/Custody composition, SBF verifier,
rollback, rent, packet, CU, and frontend evidence remain separate physical work.
