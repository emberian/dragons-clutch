# Counted-retirement layout adapter

`clutch-retirement-adapter` composes retirement tails with the exact base
account codecs owned by `clutch-solana-layout`. It is production-bound source,
but it has no SBF dispatcher route and is not deployment evidence.

Composition decoders require exact length/tag/version, restore only the frozen
base version byte in a fixed-size copy, and invoke the authoritative base
decoder. Frozen and successor Reservation schemas are deliberately distinct:

Committed authentication/composition APIs retain exhaustive
`RetirementAdapterErrorV1`. Direct Epoch, V7/V8, and successor projection APIs
use `RetirementAdapterErrorV2`, which embeds `RetirementErrorV2`. Only lossless
V1-to-V2 conversions exist; exhaustive compile fixtures freeze both V1 error
variant sets and prove that historical exhaustive matches still compile.

| Schema | Exact bytes | Tail | Deletion-capable |
| --- | ---: | --- | --- |
| general V5 | 627 | 9-byte count | no |
| direct V6 | 627 | 9-byte count | no |
| general V7 | 675 | 9-byte count + 48-byte owner | yes, pure only |
| direct V8 | 675 | 9-byte count + 48-byte owner | yes, pure only |

V7 and V8 have fresh version discrimination and never fall back to V5/V6.
Equal-length general/direct siblings cross-decode only as `WrongVersion`.
Direct V8 also requires its appended payer/principal/donation owner to mirror
the direct V2 base funding ledger exactly.

General Epoch V5 remains exactly 429 bytes. Its frozen codec accepts the
committed nonzero generation semantics; it does not reinterpret V5 by enforcing
`index + 1`. The separate successor projection
`project_live_general_epoch_retirement_v2` maps the exact five authoritative
phase bytes and requires `generation == epoch_index + 1`. Every unknown phase
refuses.

The runtime metadata boundary checks actual key, owner, writable bit, exact
length, tag/version, stored bump, and a canonical PDA already derived from
registered seeds. `AuthenticatedAccountV1` has private fields and can be minted
only by those checks. The exact Direct Epoch V4 bridge then runs its
authoritative codec and projects market, semantic Epoch id, index, canonical
checked Reservation generation, all six lifecycle phases, and its persisted
neutral sink. Direct V8 registration independently requires the projected
phase to be exactly pre-freeze-open; frozen, selected, settled, and
prefreeze-aborted parents refuse.

Most types consumed by the pure crate are still forgeable
`Adapter*ProjectionV1` DTOs. Only the Direct Epoch bridge is implemented end to
end here. Missing exact bridges for CandidateWindowV4, authoritative Budget
funding/disposition, general Realm/Market neutral-sink provenance,
Position/Replay identities, and Replay absence are activation blockers. No
pure plan should be described as runtime authorization until those boundaries
and their SBF routes exist.

The generic counted-child codec is usable only after the authoritative central
registry supplies a tag, frozen/counting version pair, exact base width, and
stored-bump offset. It does not make caller-proposed geometry live. Tombstone
coordinates `0x75/v1`, `0x76/v1`, and Replay `0x7a/v1` are centrally
`ReservedDisabled`; an adapter regression test binds those entries to the
retirement constants. They remain non-executable until exact codecs and SBF
routes land; the Replay 132-byte composition is still external/in-flight.

Run:

```sh
cargo test --manifest-path crates/clutch-retirement-adapter/Cargo.toml
cargo test --release --manifest-path crates/clutch-retirement-adapter/Cargo.toml
cargo clippy --manifest-path crates/clutch-retirement-adapter/Cargo.toml \
  --all-targets -- -D warnings
cargo doc --manifest-path crates/clutch-retirement-adapter/Cargo.toml --no-deps
```
