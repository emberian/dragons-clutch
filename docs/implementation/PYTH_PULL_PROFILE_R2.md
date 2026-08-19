# R2 Pyth pull-profile executable model

Status: **ISOLATED MODEL / DEFAULT RELEASE STOP**. This slice reconciles the
selected Pyth pull profile from `765ca81` with the isolated SourceSpec-v2 and
`CROSSING_V1` work represented by `f0e7516`/`01291de`. It is not integrated in
main, exports no `clutch-sbf` instruction, adds no source-registry entry, and
does not change the default `SourceReleaseUnavailable` (`0x79`) refusal.

## Frozen model contract

The executable model now fixes the six formerly ambiguous rules:

1. **Deployment identity:** SourceSpec v2 binds the receiver program, exact
   Upgradeable Loader ProgramData key, and deployment slot. Authentication
   requires the executable program's loader owner, the program-to-ProgramData
   link, the ProgramData loader owner, and the decoded slot. A supplied
   generation number is not evidence.
2. **Rule id:** the v2 model registers closing-boundary `CROSSING_V1` as id
   `2`. Opening-boundary experiment id `3` and V1 finalized-bucket id `1` are
   refused, so there is no rule negotiation inside one feed identity.
3. **Grid origin:** the 368-byte canonical spec contains a signed Unix-seconds
   grid origin. This rule accepts only zero; a different origin is a new
   rule/spec generation.
4. **Duplicate collapse:** only identical decoded update bodies collapse.
   Different wrapper write authorities or receiver `posted_slot` values are
   distinct qualifying witnesses and refuse the boundary.
5. **Bucket contiguity:** an immutable cursor pins the first bucket and exact
   next bucket. The first append, every `+1`, gaps, repeats, and reorderings are
   checked explicitly.
6. **Overflow:** boundary `k+1`, multiplication, `i64` comparison conversion,
   and next-cursor overflow are named refusals. None is reclassified as a
   missing witness, wrapped, clamped, or silently capped.

The spec also binds a SHA-256 digest of the full receiver Config bytes,
provider feed id, adapter/parser identity, ProgramData deployment slot,
asset/orientation/grid identity, both freshness bounds, boundary grace, and
confidence policy. Its feed identity is
`SHA256("dragons-clutch/feed/v2" || canonical_368_byte_body)`.

## Authentication seam

`research/source-profile-v1/src/auth_v2.rs` is a no-std executable contract
between a future Solana adapter and the pure parser/model. It distinguishes:

- the canonical source identity, derived from immutable SourceSpec-v2 bytes;
- the ephemeral, caller-created update-account key; and
- the exact adjacent receiver-post operation that names that update, Config,
  receiver program, and write authority.

The model checks receiver/ProgramData/config identities, full config bytes,
deployment slot, exact Clock sysvar identity and cutover time, adjacent post,
update owner/discriminator/length/full verification/feed, posted-slot and
publish-time freshness, boundary maturity, confidence bounds, and the closing
crossing rule before returning an authenticated archive record.

`LoaderStateV1` and `ImmediatePostV1` are typed outputs expected from future
official parsers. They are deliberately not wire types and must never be
accepted from caller instruction data. Likewise, there is no `finalized`
boolean: RPC commitment and ledger finality cannot be established by an
executing instruction. Operators must submit only after the selected release
and post transaction are observed at the required commitment, while the
program independently enforces canonical Clock and boundary grace.

Hostile tests cover ProgramData/config/release substitution, ephemeral versus
canonical identity confusion, set/post/restore non-adjacency, wrong update,
post program/config/write authority, wrong Clock, pre-cutover use, stale and
future slots, stale and future publish times, immature boundary, wrong parser
owner/feed, missing/double witnesses, exact versus near duplicates, missing
first bucket, gap/repeat, sequence drift, and arithmetic/cursor overflow.

## Default-release STOPs

No default registry entry is justified until all of the following are closed:

- post-cutover receiver program, ProgramData key and decoded deployment slot,
  complete Config bytes/digest, SDK/repository revision, and parser/adapter
  identities are frozen from primary sources;
- the exact Upgradeable Loader state decoder and reviewed receiver-post ABI /
  Instructions-sysvar parser replace the model projections and prove account
  indices, mutability/signers, instruction adjacency, and deployment link;
- a canonical Clock-sysvar `AccountInfo` decoder validates the exact sysvar
  key, owner, non-executable account state, and canonical Clock data before
  projecting slot/time; caller-provided `ClockViewV1` values never qualify;
- a production SourceSpec-v2 account codec, feed-domain registration, archive
  append adapter, and compiled closed registry are reviewed together; none is
  present in this slice;
- hostile real-bank tests cover post/config/update substitution, set/post/
  restore, stale update reuse, same-slot alternatives, ProgramData upgrade,
  Clock/cutover edges, missing/double witnesses, and rollback/prefund behavior;
- SBF stack, compute, account-count/size/rent, and exact deployed-ELF evidence
  exists for create/append/seal and downstream resolution; and
- retention/recovery horizon, Terms trust floor, operational finality policy,
  and provider/legal constraints are explicitly accepted.

Any one of these remaining failures keeps the source registry empty and the
default runtime on refusal `0x79`.

## Offline gates

```sh
cargo test --locked --offline \
  --manifest-path research/source-profile-v1/Cargo.toml
cargo test --release --locked --offline \
  --manifest-path research/source-profile-v1/Cargo.toml
cargo clippy --locked --offline --all-targets \
  --manifest-path research/source-profile-v1/Cargo.toml -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --no-deps \
  --manifest-path research/source-profile-v1/Cargo.toml
```
