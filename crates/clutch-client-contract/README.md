# Client contract

`clutch-client-contract` is the shared vocabulary for **untrusted client
projections**. It does not own an account byte, protocol identity, relation
fact, transaction builder, RPC observation, or historical index. Persisted
facts remain owned by `clutch-solana-layout`; relation values remain owned by
`clutch-batch`; intent allocations remain owned by the central registry in
`clutch-solana-layout::registry`.

This first wave centralizes three seams used by current local infrastructure:

- exact provenance labels, separate from claim-strength labels such as
  `SBF-EXECUTED`;
- typed classification of observed intent tag/version coordinates through the
  central registry, without copying registry constants;
- the Operator's deliberately narrow settlement capability classifier. It
  admits only an exact direct single-Egg settlement that the current Operator
  can construct completely. Extra pages, churned pages, virtual legs, potted
  conversion, mixed or portfolio pairs, duplicate receipts, and incomplete
  coverage all refuse before candidate submission.

The settlement input borrows the authoritative layout and relation types. The
returned plan is an ephemeral instruction projection, never onchain state and
never evidence that an instruction executed. A fresh chain snapshot cannot be
promoted to evidence of historical completion; the evidence API makes that
specific promotion unavailable.

## Duplication audit, 2026-08-23

The audit preceding this wave found three related but non-identical walks:

- `programs/clutch-sbf/operatord/src/session.rs` projected one raw page into a
  relation book and contained the only client-side settlement capability gate;
- `programs/clutch-sbf/harness/src/lib.rs` projects fixture pages while
  constructing expected program transitions, including portfolio settlement
  shapes the Operator cannot construct;
- `programs/clutch-sbf/keeper/src/crank.rs` walks decoded multi-page state to
  discover permissionless work. Its RPC view is a liveness hint and every
  emitted instruction is authenticated again onchain.

Only the capability gate was truly the same semantic question, so this wave
moves that gate and its adversarial matrix here and wires Operator to it. It
does not force the harness's expectation generator or the keeper's multi-page
liveness discovery through a one-page Operator projection. A later shared
page-set observation type can replace those walks only after it preserves
physical page, slot, stored order id, and live relation rank as distinct
coordinates.

## Dependency and provenance boundary

There are no new external dependencies. Both dependencies are first-party path
dependencies:

- `clutch-batch`: authoritative V1 relation types;
- `clutch-solana-layout`: authoritative persisted-layout types and central
  intent registry.

No third-party notice or source-offer entry is introduced. The crate is
AGPL-3.0-or-later, is not published independently, and is runtime/client
classification code rather than proof or deployment code. This records the
dependency decision required by `docs/PROVENANCE.md`; transitive external
packages are unchanged from the existing layout graph and pinned by each
consumer's lockfile. This crate's lock digest is
`5cfab61b36d2c02cbbede4d45647209375c09f2a2c8014993adb249fcc66f961`.

## Checks

```sh
cargo test --manifest-path crates/clutch-client-contract/Cargo.toml --offline --locked
cargo test --release --manifest-path crates/clutch-client-contract/Cargo.toml --offline --locked
cargo clippy --manifest-path crates/clutch-client-contract/Cargo.toml \
  --offline --locked --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc \
  --manifest-path crates/clutch-client-contract/Cargo.toml \
  --offline --locked --no-deps
cargo fmt --manifest-path crates/clutch-client-contract/Cargo.toml -- --check
```
