# dclutch-claims

Wire records, canonical resource derivations, the physical account frame, and
the host-side execution candidate for shard-backed Structured receipts. The pure
kernel stays the sole owner of coefficient, backing, settlement, and lifecycle
arithmetic.

Five records:

- `StructuredRequestV2` — 432 fixed bytes: one action, the identities that must
  join, one receipt quantity, and the optimistic replay revision. No
  coefficient, payout, supply, or custody value appears on the wire.
- `StructuredRootV2` — 128 fixed bytes: replay revision and permanent RentCredit
  beneficiary only. The lifecycle phase is deliberately absent, because it is
  authenticated per transaction from the Market and Product terminal record and
  persisting it would create a second owner for a fact Core already owns.
- `StructuredRootSeedsV2` / `StructuredReceiptMintSeedsV2` /
  `StructuredShardCustodySeedsV2` — the exact seed order for the three derived
  resources, and the anti-aliasing rule. The root is keyed by `terms_id` alone;
  the receipt Mint cannot be, because the terms persist its address, so it is
  keyed by the digest of the terms with that field EXCISED under its own domain;
  custody is keyed by `[terms_id, shard_mint]`, so the mint-to-custody binding
  is a derivation rather than an index lookup. No address is derived here —
  `find_program_address` belongs to the adapter.
- `StructuredFrameSpecV2` — the physical account frame: a fixed base plus one
  (shard Mint, actor shard, custody shard) triple per BACKED coordinate, and the
  sole author of which coordinate each Token effect's five accounts occupy.
  Zero-coefficient rows contribute no accounts at all.
- `StructuredHotCandidateV2` — the opaque bounded candidate. It revalidates
  every amount against the immutable coefficients and exposes borrowed effects
  plus commit-last root bytes.

  **It is a host-side adversary, not a chain seam.** It judges what
  `dclutch-structured-v2-operator` plans; nothing on chain calls it, and
  decision 0011 records why nothing can. A family reaches the Trading executor
  through a sealed artifact closure, never through Rust the family wrote.
  `frame.rs` is likewise not the `AccountProfileV2` the chain expands — thirteen
  of its twenty-three base coordinates name accounts the hot frame already
  fixes or injects.

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
cargo test --manifest-path crates/dclutch-claims/Cargo.toml
cargo clippy --manifest-path crates/dclutch-claims/Cargo.toml \
  --all-targets -- -D warnings
```

These are pure observation tests. They are not evidence that Token-2022 executed,
that SBF account validation is correct, or that transaction rollback works.
