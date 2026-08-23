# Counted-retirement live-layout adapter seam

`clutch-retirement-adapter` composes ADR-0007's new tails with the exact base
account codecs owned by `clutch-solana-layout`. It is production-bound source,
but it is not connected to the SBF dispatcher and is not deployment evidence.

The composition decoders require an exact promoted header and length, restore
only the legacy version byte in a fixed-size copy, and invoke the authoritative
base decoder. This avoids duplicating Market, Position, Epoch, or Reservation
semantics. General Reservation `v4→v5` and direct Reservation `v2→v6` remain
distinct even though both share tag 19 and both promoted bodies are 627 bytes.
Direct V3 is not reusable: it is a retired general-Reservation wire schema.

The runtime account boundary separately checks:

- the actual key against a canonical PDA and bump already derived from exact
  seeds by the Solana adapter;
- the actual owner against the authenticated program id;
- writability before a mutation path;
- exact account length, tag, version, and stored bump; and
- the complete semantic body through its owning decoder.

The generic counted-child codec is usable only after a child tag/version,
legacy width, and bump offset are allocated by the authoritative registry. It
does not make a caller-proposed schema live.

The proposed tombstone tags `0x75/0x76` are collision-free at the audited HEAD,
but remain codec-local and non-wire until the central account registry exports
them. See
[`COUNTED_RETIREMENT_LIVE_PROMOTION.md`](../../docs/implementation/COUNTED_RETIREMENT_LIVE_PROMOTION.md).

Run:

```sh
cargo test --manifest-path crates/clutch-retirement-adapter/Cargo.toml
cargo test --release --manifest-path crates/clutch-retirement-adapter/Cargo.toml
cargo clippy --manifest-path crates/clutch-retirement-adapter/Cargo.toml \
  --all-targets -- -D warnings
cargo doc --manifest-path crates/clutch-retirement-adapter/Cargo.toml --no-deps
```
