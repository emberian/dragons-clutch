# dClutch Market Core codec

This standalone crate is the generated, fixed-memory interpreter for
`DClutchSemantics.MarketCore`. It is intentionally not wired into the workspace
or an SBF adapter yet.

`DClutchSemantics.MarketCoreAbi` owns the canonical field order, widths, and
offsets. `EmitMarketCoreRust.lean` emits both those constants and the safe Rust
interpreter. The fixed Market header is 1,416 bytes and the request is 72 bytes.
Claim vectors are exact-length borrowed slices whose length must equal the
Product's runtime `outcome_count`; the ABI imposes no width-specialized Market
semantics or provisional maximum N.

The interpreter validates all inputs before applying a transition. It separates
rent, unclassified donation, Source work funding, deferred custody rent, and
claimant Hoard principal. It consumes current Registry/Core execution-release
receipts but does not authenticate accounts, derive addresses, move tokens,
perform CPI, or supply transaction rollback. Those remain named adapter duties.

Run `./check.sh` to rebuild the Lean ABI, compare the checked-in Rust against
fresh generator output byte-for-byte, and run formatting, tests, and strict
Clippy.
