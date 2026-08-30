# Series Shadow runtime SHA adapter, 2026-08-28

This records the exact Series Shadow source and SBF link after its evaluator's
content digests moved from a software SHA-256 implementation to the named
runtime adapter.

## Decision

The shipped evaluator now calls `dclutch_sha256_adapter::digest`. On the Solana
target that adapter calls `solana_sha256_hasher::hashv`, whose backend is the
`sol_sha256` runtime syscall. The old `sha2` dependency remains only as a
development dependency so host tests can compare the new result byte for byte
against the old implementation.

This preserves the committed preimage exactly. Each changed production call
still hashes one complete byte slice; no separator, length, domain, or slice
order changed. The projection helper in tests uses `digestv` over the same
ordered four-slice preimage that its previous streaming hasher absorbed.

## Exact source and tests

The source under test is commit `7a4b4c51`:

- `10140806` routes Series evaluator content digests through the adapter and
  adds hostile byte-identity tests;
- `7a4b4c51` refreshes the two nested workspace lockfiles without unrelated
  dependency churn.

The following host tests pass:

- `cargo test -p dclutch-series-shadow-sbf --lib`: **8/8**;
- generator source-derivation test: **1/1**;
- nested real-SBF harness library tests: **3/3**.

The hostile adapter test covers preimage lengths 0, 1, 31, 32, 55, 56, 63,
64, 65, 127, 128, 129, 255, 256, 257, and 4,288 bytes. At every boundary the
adapter digest equals the legacy software digest, and a one-byte hostile
mutation refuses byte identity. A second test proves that the four-slice
projection preimage equals the legacy streaming digest and that exchanging the
two Merkle children changes the result.

The locked source was transferred to hbox as an archive of exactly
`7a4b4c51`. Its lockfile SHA-256 identities are:

| workspace | `Cargo.lock` SHA-256 |
|---|---|
| root | `eb3ce3dde753fb89baee7aaf8d59b1ecc2027eca43399b2af451b958ab024ee4` |
| Series generator | `e451f3e30d484382f2682b56b5e219ae57f2c475a7ac6e1a70cdc7f75baf3fe3` |
| Series program-test | `9657e1fd04b8338c304ac9ffd42bac970933e636318b1166a564a2f5fa52748e` |

The build used hbox's `swarm-build`, `cargo-build-sbf 4.0.0`, platform tools
v1.53, and Rust 1.89.0. A generator unit test emitted an explicitly ephemeral
selected include only to make the evaluator SBF-reachable:

| selected input | SHA-256 |
|---|---|
| generated include | `29ad12fc4865d07188ccf5fb13777f5b75fdf4336dd807674265f1319a7a9612` |
| source manifest | `9f66c70235c1c5d2fad0dcebdf04f5018f957957829daad16c9e48231c40bcc9` |

This generator fixture is not a checked release and is not deployment
evidence.

## Exact selected SBF link

The locked selected build produced:

| fact | value |
|---|---|
| ELF | `dclutch_series_shadow_sbf.so` |
| bytes | 376,792 |
| ELF SHA-256 | `95f107fa11356a3e3210071dd9707b138d2c39409994cce7e038c54379d61948` |
| build-log SHA-256 | `11ec0ccb8dae7a1073e393ce117cf6247316825d5b9bbb14b1bd5a25dc6ba8ec` |
| SBF frame-overwrite diagnostics | **0** |

The SBF build log does not compile `sha2 0.10.9`, `digest`, `block-buffer`, or
`crypto-common`. `strings` and `readelf -Ws` find no software SHA symbols such
as `sha2::`, `sha256::compress`, `compress256`, or
`sha256_hasher::Hasher`. The one cryptographic import in the selected Series
ELF is the undefined global `sol_sha256` symbol. Thus host `cargo tree` output,
which necessarily shows the adapter's non-Solana `sha2` backend, is not the
shipped-link dependency graph.

## Frame diagnostic

A second locked selected build used `-Zemit-stack-sizes --emit=obj,link` under
`swarm-build`. Its inspected Series object has SHA-256
`a7cc4fcc80275f62cc30e34183b20ce5910cb0db4ab3b6963778d347e22c03aa`.
The frame build log has SHA-256
`7a8257b8fe6a4462f6500a15fcf76fddb622e56f76a20d83d711687225f25b91`;
the 36-frame report has SHA-256
`47f242dfe88ffa1abc2f861f33a4e6709fe304810c8ea4a2edb8660085815a29`.

| function | frame bytes | spare below 4,096 |
|---|---:|---:|
| `evaluate_series_shadow_aot_v4` | 3,264 | 832 |
| Series semantic core request | 2,368 | 1,728 |
| `evaluate_authenticated_invocation` | 2,176 | 1,920 |
| `publish_acknowledgement` | 2,112 | 1,984 |
| `evaluate_selected_and_publish` | 1,664 | 2,432 |

No measured frame reaches or exceeds 4,096 bytes. The frame diagnostic ran
after the successful build; build success alone is not the evidence.

## Compute status: no substitute draw

There is not yet a caller-backed Series Shadow transaction in this tree. The
nested `program-test` workspace deliberately provides loading, selected-build,
route-order, and rollback support only; its README says the eventual
integration test will invoke the projected Trading outer. Existing Series
tier-4 execution reaches Core Series consume, not this Shadow accelerator.

Therefore this change does **not** report an action CU or a margin. An invalid
invocation would refuse before the changed evaluator, and a different
program's draw would measure the wrong ELF. The next honest compute evidence is
a real Trading caller that reaches this exact selected Series ELF, followed by
the M-61 pass count and 20-seed mean.

For scale only, not as a Series measurement or margin, the independent General
accelerator campaign measured one 4,288-byte candidate digest at 456,008 CU in
software and 2,234 CU through the syscall. Its two affected whole actions moved
by 453,813 and 453,829 CU. Those measurements explain why this link-level
repair matters, but they remain General evidence.

## Adjacent SBF runtime-primitive frontier

This was a bounded source and direct-link scan, not a claim that every
transitive host dependency is absent from every artifact.

### Already using runtime facilities

- Permanent program SHA-256 call sites generally use
  `solana_program::hash::{hash, hashv}`. Examples include Core
  `retire_v1.rs`, `generic_founding_v1.rs`, and `series_consume.rs`; Claims
  `affine_batch_v2.rs` and `founding_v5.rs`; Resolution `core_effect.rs` and
  `provider_v3.rs`; Custody `projected.rs`; and Trading `hot_v3.rs` and
  `generic_market_founding_v1.rs`. These calls reach the runtime SHA-256
  primitive on SBF.
- Trading native signatures use the Ed25519 precompile, not software curve
  verification. `native_signature.rs:354` authenticates the preceding
  instruction's program as `ed25519_program::ID`, after reading the
  Instructions sysvar. The adjacent-instruction binding is part of the
  protocol and must not be replaced by caller-asserted signature bytes.
- The selected Series ELF has no direct or linked software SHA-256, SHA-3,
  Keccak, Blake3, curve, secp256k1, or Ed25519 implementation. Solana SDK
  helper crates visible in a host dependency tree are not proof that their
  software fallback is present in this ELF; the symbol and build-log audit is
  the direct-link evidence.

### PDA search is the next broad compute target

The permanent program sources contain many canonical bump searches, including
real hot-path calls in Trading `hot_v3.rs:1193,1343,1609,1740,10030,10048,
10169,10174`, five separate founding caller searches in
`generic_market_founding_v1.rs:626,675,732,815,900`, and the Series raw and
staging record searches at `entrypoint.rs:424,426`. The common Shadow
authentication adapter adds searches at
`dclutch-shadow-accelerator-auth-v4/src/lib.rs:236,275`.

This cost is bump-dependent. Where a PDA has already been canonically created,
its semantic owner should persist the canonical bump and hot paths should
validate the exact seeds with `create_program_address`; the caller must never
select a bump. Where no canonical owner exists, a preparation/checkpoint
instruction can perform the search once and bind its result to the release,
request, and lifecycle revision. The checkpoint must be replay-safe and stale
on any bound-input mutation. Searches whose result authorizes signing or
selects a unique protocol account cannot merely be omitted.

### Sysvars, memory, and logs

- Series parses Rent and Clock accounts at `entrypoint.rs:169,197`. Many other
  permanent routes parse Rent or Clock more than once across a composed
  lifecycle. A caller can parse once and pass an immutable scalar/view within
  one invocation, but it must continue to authenticate the canonical sysvar
  account and must not create a second persisted truth. `Rent::get` or
  `Clock::get` is only an optimization candidate after exact CU measurement;
  changing the AccountProfile binding is a semantic change.
- Series collects every runtime account borrow into a `Vec` before projection.
  A later evaluator pass should prefer bounded borrowed views or one-pass
  projection so the 161-plus-account profile does not allocate and retain more
  wrappers than the fold needs. That work must preserve atomic projection: no
  caller-visible acknowledgement or state effect may escape after a late
  refusal.
- Static `msg!` calls in the permanent paths are sparse and concentrated in
  Trading activation/refusal arms (`outer.rs:376,1094-1106,2301`). Preserve
  diagnostic refusal labels, but keep formatted logging out of success loops.
  No direct first-party `sol_mem*` hot call was found in the selected Series
  link; generic SDK memory helpers are not by themselves an optimization
  target.

### Fractional and host-only software SHA

The current permanent Claims program depends on
`dclutch-fractional-claim-kernel` (singular), not the prospective
`dclutch-fractional-claims-kernel` and `dclutch-fractional-claim-contract`
stack. Consequently the following production software SHA call sites are not
in today's permanent Claims ELF, but they become SBF-reachable if the
Fractional twin is linked without another adapter pass:

- `dclutch-fractional-claim-contract/src/artifacts.rs:476`;
- `dclutch-fractional-claim-contract/src/hot_v2.rs:315,343,614,714,749`;
- `dclutch-fractional-claims-kernel/src/exposure_v2.rs:271`;
- `dclutch-fractional-claims-kernel/src/lib.rs:1271,1281,1285`.

Fractional integration must convert each complete preimage to
`dclutch_sha256_adapter::{digest,digestv}`, retain legacy host vectors, build
the actual selected Claims/Fractional ELF, and prove by build-log plus symbol
audit that software SHA did not enter it. It then needs caller-backed action CU
with an M-61 pass count and 20-seed mean.

`dclutch-product-compiler/src/lib.rs:805` and
`noncategorical_v3.rs:340,354,358` also use software SHA in production code,
but the compiler is reached by the host-only Product Runtime operator and is
not a dependency of a permanent program. It should remain host software unless
an SBF consumer is deliberately introduced.

## Required follow-through

1. Build the real projected Trading caller into the Series nested campaign and
   prove it reaches this exact selected evaluator, including late-refusal
   rollback.
2. Record source-bound Series action compute as pass count plus a 20-seed mean,
   along with packet bytes, unique locks, ALT contents, and frame depth.
3. Replace hot-path repeated PDA searches with owner-persisted canonical bumps
   or replay-bound preparation checkpoints, one route at a time, and measure
   the bump-distribution delta rather than one draw.
4. Route every prospective Fractional production digest through the adapter
   before it becomes part of a shipped Claims/Fractional link.
5. Keep host-only generators, compilers, fixtures, and legacy comparison
   oracles classified as host-only. Their `sha2` dependencies do not by
   themselves identify an SBF defect.
