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

It deliberately does **not** integrate with `clutch-sbf`. The executable
authentication contract in [`src/auth_v2.rs`](src/auth_v2.rs) closes the model
join, but four production facts remain open:

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

## PROPOSED SourceSpec v2 pull profile and CROSSING_V1 (MODEL-ONLY)

[`src/spec_v2.rs`](src/spec_v2.rs),
[`src/crossing_v1.rs`](src/crossing_v1.rs), and
[`src/auth_v2.rs`](src/auth_v2.rs) implement the spec revision, selection rule,
and atomic authentication join proposed in
[`SOURCE_PROVIDER_V1_SELECTION.md`](../../docs/design/SOURCE_PROVIDER_V1_SELECTION.md)
as research models with hostile-byte and falsifier tests:

- a 368-byte canonical pull-profile spec body under the **new domain**
  `dragons-clutch/feed/v2`: the V1 exact source data-account key is replaced by
  the receiver `Config` PDA key plus a SHA-256 digest of its full account
  bytes; the provider feed id, exact ProgramData key and deployment slot,
  zero Unix grid origin, and boundary-grace policy are part of the immutable
  identity;
- the single registered model rule id `2`, closing-boundary `CROSSING_V1`,
  with `T(k) = (k+1)*B`. Opening-boundary id `3`, V1 finalized-bucket id `1`,
  and nonzero grid origins are explicit refusals;
- exact duplicate collapse: only identical decoded update bodies collapse.
  A differing write authority or receiver-posted slot is a second witness and
  refuses the boundary; and
- a start-aware checked archive cursor, so a missing first bucket, a gap, a
  repeat, or an unrepresentable next cursor cannot be silently accepted.

The authentication model distinguishes the canonical v2 feed identity from
the ephemeral update-account key. It checks the receiver/ProgramData link and
deployment slot, full Config-byte digest, canonical Clock identity and
cutover, exact adjacent post projection, parser/feed/owner, both freshness
clocks, boundary grace, confidence policy, and the crossing rule. The future
SBF adapter must derive the loader and adjacent-instruction projections from
the official loader and Instructions-sysvar formats; they are never valid as
caller-asserted instruction data. RPC commitment/finality is intentionally
not modeled as an in-program bit because an executing instruction cannot
prove it.

This is **not a runtime transition**: no registry entry exists and no
post-cutover deployment/config bytes are frozen. An absent crossing witness is
an explicit stall (never a fabricated `Missing`), and the default ELF keeps
refusing `SourceReleaseUnavailable` (`0x79`).

## Run

```sh
cargo test --locked --offline --manifest-path research/source-profile-v1/Cargo.toml
cargo clippy --locked --offline --manifest-path research/source-profile-v1/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --manifest-path research/source-profile-v1/Cargo.toml --no-deps
```

This is original AGPL-3.0-or-later research code.  It copies no upstream source
or fixture bytes; the fixture is a documented schema-derived vector.
