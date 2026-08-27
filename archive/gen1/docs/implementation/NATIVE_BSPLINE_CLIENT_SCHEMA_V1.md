# Native B-spline client/compiler schema V1

Status: **implemented host compiler artifact and dependency-free offline client;
not an on-chain certificate account**, 2026-08-19.

## Result

`research/bspline-shape-compiler/src/artifact.rs` owns canonical bytes for the
exact degree-0 through degree-3 `BasisSpec` and a recompile-verifiable shape
certificate. `apps/static-client/native-bspline-v1.js` independently reproduces
the basis bytes and digests, inspects canonical Terms and certificate bytes,
and builds an unsigned, offline runtime-shaped Terms-upload/market-creation
preview sequence:

```text
BeginArtifact(Terms)
WriteArtifact(cursor=0,    len=192)
...
WriteArtifact(cursor=1536, len=120, wire suffix zero)
SealArtifact(Terms)
CreateMarket
```

This is native spline semantics. Degree zero means the native exhaustive
degree-zero basis. There is no compatibility-lowering tag, branch, or helper in
the artifact/client API, and the client refuses any semantic tag other than
`native-bspline`.

## BasisSpec V1 bytes

The artifact is exactly 304 bytes. Integers are little-endian.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | ASCII `DCBASV01` |
| 8 | 2 | schema version `1` |
| 10 | 2 | basis evaluator version `1` |
| 12 | 1 | semantic tag `1` = native B-spline |
| 13 | 1 | outcome count |
| 14 | 1 | degree, `0..=3` |
| 15 | 1 | active knot count |
| 16 | 1 | uniform log2 spacing, `0xff` = absent |
| 17 | 1 | edge policy, `1` clamp / `2` refuse |
| 18 | 6 | canonical zero reserved bytes |
| 24 | 8 | payout-weight denominator |
| 32 | 16 | admitted coordinate maximum |
| 48 | 256 | sixteen `u128` knots, active prefix then zero padding |

The digest is
`SHA256("dragons-clutch/basis-spec/v1" || exact_304_bytes)`. Admission
repeats `clutch-bspline::BasisSpec::validate`: count relations, strict knots,
canonical padding, degree-0 clamp-only behavior, uniform spacing for degrees
2/3, and the freeze-time arithmetic bound. A different valid domain or knot is
a different digest, not a malformed encoding.

## Shape certificate V1 bytes

The fixed 456-byte prefix is followed by exact unsigned rationals.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 8 | ASCII `DCSHCV01` |
| 8 | 2 | certificate schema version `1` |
| 10 | 2 | compiler semantic version `1` |
| 12 | 2 | evaluator semantic version `1` |
| 14 | 2 | `WEIGHT-ROUND-01`, largest remainder / low-index tie |
| 16 | 1 | native B-spline semantic tag `1` |
| 17 | 1 | exact-in-span `1` / certified approximation `2` |
| 18 | 1 | compiler construction id |
| 19 | 1 | certificate subdivision depth |
| 20 | 32 | canonical Terms digest |
| 52 | 32 | BasisSpec V1 digest |
| 84 | 304 | complete BasisSpec V1 bytes |
| 388 | 64 | fixed source-shape description |
| 452 | 2 | rational count, exactly `outcome_count + 10` |
| 454 | 2 | canonical zero reserved bytes |
| 456 | variable | coefficient and bound rationals |

The seven source-shape tags are hard range, upper tail, lower tail, triangle,
capped call, capped put, and Gaussian. Each has three fixed `u128` coordinate
slots and one `u64` height; unused coordinate slots are zero.

Each rational is `numerator_len:u16 || denominator_len:u16 || numerator_be ||
denominator_be`. Values are nonnegative. Integer magnitudes are minimal big
endian, the denominator is positive, the fraction is reduced, and zero has the
single representation `numerator_len=0, denominator=1`. One integer is capped
at 4,096 bytes and one certificate at 256 KiB.

The rational order is all active coefficients, height, maximum coefficient,
then the eight `ErrorCertificate` bounds in Rust field order. Rust decoding
re-runs `compile(basis, shape)` and requires exact structural equality. The
certificate digest is
`SHA256("dragons-clutch/shape-certificate/v1" || canonical_bytes)`.

## Terms and runtime binding

Terms V3 remains the consensus owner of degree, knots, denominator, edge
policy, outcome count, and all source/window policy. The client recomputes its
historically named current digest domain
`SHA256("dragons-clutch/terms/v2" || exact_1620_byte_body)`, then projects the
basis with `clutch_accumulator::MAX_VALUE = 10^24`. Tests read both Rust owners
and fail if either typed constant/domain drifts.

The typed upload plan contains one 77-byte Begin intent, nine 263-byte Write
intents, one 69-byte Seal intent, and the 139-byte CreateMarket intent. The
first eight writes carry 192 payload bytes. The ninth carries 120 and its final
72 wire bytes are zero. The offline builder mirrors the runtime's `8..=432000`
slot staging-lifetime bound, but the runtime authenticates Clock and rechecks
it. The client emits no account metas, message, signature, or transaction
submission.

## Important certificate boundary

The analytic shape certificate is **offline evidence**. Current Terms commits
to its native basis fields but has no shape-certificate-digest field, and the
program does not parse or persist this certificate. Consequently:

- a Rust decode proves only deterministic recompilation at this compiler
  version;
- JavaScript checks canonical structure and digests but explicitly does not
  re-run the exact-rational compiler;
- neither check turns the certificate into on-chain authority; and
- settlement follows Terms and authenticated evidence, never an analytic shape
  name supplied by a client.

A future on-chain certificate binding needs a versioned Terms/layout change. It
must not be inferred from this host artifact.

## Cross-language fixture and checks

`research/bspline-shape-compiler/fixtures/native-v1-degree1.json` is emitted by
the Rust example and held byte-equal to the Rust renderer by an integration
test. Node consumes that same file; it has no hand-written second fixture.

```sh
cargo run --manifest-path research/bspline-shape-compiler/Cargo.toml \
  --example emit_native_v1_fixture --offline
cargo test --manifest-path research/bspline-shape-compiler/Cargo.toml --offline
cargo clippy --manifest-path research/bspline-shape-compiler/Cargo.toml \
  --all-targets --offline -- -D warnings
(cd apps/static-client && npm test && npm run check)
```

Hostile tests cover degrees 0-3, all seven shapes, exact recompilation, domain
separation, rounding identity/ties, nonminimal and negative rationals,
zero-denominator and size caps, every manually typed Terms offset, all artifact
intent bindings/cursors/lengths/padding/expiry, exact Terms reconstruction, and
byte-identical Rust/JavaScript intents.
