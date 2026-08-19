# Objective crypto-price source profile V1 research

Status: **conditional parser/model; not a production source profile**.

This directory asks a narrower question than “can Dragon's Clutch read an
oracle?”: can a permissionless submitter be prevented from choosing whichever
valid price is most profitable after seeing several of them?

The best current answer is a Pyth point-at-time profile.  A Pyth
`PriceFeedMessage` carries both `prev_publish_time` and `publish_time`, and its
official definition assigns instant `T` to the unique message satisfying

```text
prev_publish_time < T <= publish_time.
```

That is exactly the missing canonical-selection relation.  It is materially
stronger than “fresh within N seconds.”  The code in [`src/lib.rs`](src/lib.rs)
parses the reviewed `PriceUpdateV2` account format, requires full verification,
implements that crossing relation, and normalizes confidence intervals with
outward integer rounding.

It deliberately does **not** integrate with `clutch-sbf`.  Four adapter facts
remain open:

1. the Pyth Core Solana migration is still changing program and signer
   provenance;
2. the receiver program and its mutable `Config` must both be pinned;
3. a caller-created pull-update account must be proven to have been posted
   immediately under that pinned configuration, not merely owned by the
   receiver; and
4. the current generic Dragon's Clutch source spec assumes one immutable source
   account and a source-native sequence/slot, while Pyth pull updates use
   ephemeral accounts and expose publish times plus a Solana `posted_slot`.

Until those joins have hostile SVM tests, `FeedAdvance` and `Resolve` remain
non-qualifying exactly as stated in
[`SOURCE_ADMISSION_V1.md`](../../docs/implementation/SOURCE_ADMISSION_V1.md).

The full candidate analysis is in [`DOSSIER.md`](DOSSIER.md), and exact reviewed
upstream revisions and fixture derivation are in
[`PROVENANCE.md`](PROVENANCE.md).

## Run

```sh
cargo test --manifest-path research/source-profile-v1/Cargo.toml
cargo clippy --manifest-path research/source-profile-v1/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path research/source-profile-v1/Cargo.toml --no-deps
```

This is original AGPL-3.0-or-later research code.  It copies no upstream source
or fixture bytes; the fixture is a documented schema-derived vector.
