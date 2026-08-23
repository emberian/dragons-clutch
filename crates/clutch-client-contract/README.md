# Client contract

`clutch-client-contract` is the shared vocabulary for **untrusted client
projections**. It does not own an account byte, protocol identity, relation
fact, transaction builder, RPC observation, or historical index. Persisted
facts remain owned by `clutch-solana-layout`; relation values remain owned by
`clutch-batch`; intent allocations remain owned by the central registry in
`clutch-solana-layout::registry`.

This first wave centralizes four seams used by current local infrastructure:

- exact provenance labels, separate from claim-strength labels such as
  `SBF-EXECUTED`;
- typed classification of observed intent tag/version coordinates through the
  central registry, without copying registry constants;
- the Operator's deliberately narrow settlement capability classifier. It
  admits only an exact direct single-Egg settlement that the current Operator
  can construct completely. Extra pages, churned pages, virtual legs, potted
  conversion, mixed or portfolio pairs, duplicate receipts, and incomplete
  coverage all refuse before candidate submission;
- the General V2 chain-derived owner projection. It consumes exact verified
  order rows, one explicit fee row per owner (including zero), selected
  candidate owner count, buy/sell price units, selected fee atoms, rounding
  pot, receipt-end count, and current Position cash. It emits lexicographically
  owner-sorted 288-byte open bodies plus exact prospective debit, credit, fee,
  released cash, residue, and Position post-state fields. Many filled orders
  may aggregate into one owner row; owner count is not equated with filled
  order count.

The settlement input borrows the authoritative layout and relation types. The
returned plan is an ephemeral instruction projection, never onchain state and
never evidence that an instruction executed. A fresh chain snapshot cannot be
promoted to evidence of historical completion; the evidence API makes that
specific promotion unavailable. The General V2 projection likewise does not
claim that the current General V1 runtime created an owner row, authenticated
every receipt, or executed its prospective disposition.

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

There are no new external dependencies. All three dependencies are first-party path
dependencies:

- `clutch-batch`: authoritative V1 relation types;
- `clutch-owner-settlement`: authoritative owner aggregation, 288-byte semantic
  body, terminal rounding, and disposition types;
- `clutch-solana-layout`: authoritative persisted-layout types and central
  intent registry.

No third-party notice or source-offer entry is introduced. The crate is
AGPL-3.0-or-later, is not published independently, and is runtime/client
classification code rather than proof or deployment code. This records the
dependency decision required by `docs/PROVENANCE.md`; transitive external
packages are unchanged from the existing layout graph and pinned by each
consumer's lockfile. This crate's lock digest is
`1a1a0ccfa630db5cf247cdb2983bf26c1beb4e7922d17bbe46b206d81aaeb699`.

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
