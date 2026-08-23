# ClearWork V2 kernel evidence

This note records evidence for the production-bound active-width primitives in
`relation_v1_stream_v2`. It is not SBF compute-unit evidence and does not claim
that a V2 Solana account route exists. Dispatcher, handler, layout-profile, and
deployment integration remain separate work.

## Ownership boundary

The intended runtime path is `ClearWorkFeedV2` over borrowed exact-width
account bytes. It performs no allocation and never constructs `ClearWorkV1`.
Its scalar execution state is 1,776 bytes on the host 64-bit ABI; active
matrices, allocation rows, settlement ledgers, pairing accumulators, and the
canonical summary remain directly byte-backed. `ClearWorkViewV2` and
`ClearWorkViewMutV2` remain the smaller typed region APIs. All active indexing
uses authenticated frozen widths.

The V1 bridge is deliberately different. Migration and differential testing
may call `project_clear_work_v1_wire_into_v2` or
`expand_clear_work_v2_into_v1_wire`, but the caller must supply the 47,846-byte
V1 wire image/scratch. That cost is visible in the function signature and is
not a runtime V2 allocation hidden inside the kernel.

V2 preserves the three relation-owned folds byte-for-byte. The persisted
sealed-fold comparator does not replace the layout-owned candidate/order-set,
page continuation, or consumed-fold checks, and `DigestFoldV1` remains a
deterministic consistency device rather than a cryptographic commitment.

## Frozen geometry

The exact body formula is:

```text
678 + 73*N + 68*U + 336*O + 16*N*O + 16*U*O
```

where `O` is active outcomes, `N` is frozen live orders, and `U` is frozen
distinct owners. Representative body sizes are:

| `(O,N,U)` | V2 body bytes | V1 body bytes |
|---|---:|---:|
| `(2,0,0)` | 1,350 | 47,846 |
| `(2,1,1)` | 1,555 | 47,846 |
| `(2,4,3)` | 2,070 | 47,846 |
| `(4,16,8)` | 5,270 | 47,846 |
| `(8,32,16)` | 12,934 | 47,846 |
| `(16,64,64)` | 47,846 | 47,846 |

The eight canonical regions are contiguous and exhaustive. Tests pin every
boundary and the maximum-width identity with V1.

## Host release stack and text probes

The probe used detached source commit `1f8f521`, rustc
`1.98.0-nightly (91fe22da8 2026-06-21)`, target
`x86_64-unknown-linux-gnu`, release optimization, and
`RUSTFLAGS=-Zemit-stack-sizes`. The resulting rlib SHA-256 was:

```text
494163d2f97a8f09a275924b43b0c216d83e4d20ae0e070dc5182b1d87c5be59
```

`llvm-readobj --stack-sizes` reported these representative compiler frames:

| Function | Host frame bytes |
|---|---:|
| `validate_clear_work_v2` | 264 |
| V1-to-V2 projection bridge | 168 |
| V2-to-V1 expansion worker | 136 |
| region slicing | 136 |
| sealed-fold comparison | 72 |
| immutable/mutable view open | 56 each |
| native idle initialization | 40 |
| matrix read/write | 40 |
| owner-unit read/write | 8 |

On the host 64-bit ABI, each borrowed view is exactly 32 bytes. The frozen test
asserts both immutable and mutable view sizes. Direct symbols attributed to the
whole V2 module total 12,113 bytes of release x86-64 text. This is a direct
symbol lower bound, not a promised ELF delta: inlining, shared V1 policy/idle
callees, LTO, and the target backend change final ownership.

These measurements establish the absence of a fixed 48 KiB native view or a
4,096-byte-class host frame. They do not predict SBF frames or CU. The V2 route
must be measured again with the pinned SBF toolchain after adapter integration.

The complete engine probe used the same compiler and target against the
cycle-three source, release optimization, and `-Zemit-stack-sizes`. Its rlib
SHA-256 was:

```text
fea88867791d8e6967238a2171458415b8cb0af89d3107b1c735e17848de9066
```

Representative engine frames were:

| Function | Host frame bytes |
|---|---:|
| `begin_with_basis` | 3,624 |
| state decode / `open` / `initialize` | 3,080 / 1,800 / 1,800 |
| order transition worker | 616 |
| `end_pass` | 504 |
| composite-fee numerator worker | 392 |
| scalar persistence | 184 |
| slice transition | 104 |

Direct symbols attributed to the engine total 64,956 bytes of release x86-64
text; codec plus engine symbols total 77,508 bytes. These are conservative
archive-symbol sums, not an ELF delta, and include compiler outlining. The
3,624-byte host begin frame is below but close to Solana's 4 KiB frame limit;
it is a promotion warning, not SBF evidence. The SBF adapter must measure and,
if necessary, split that frame before enabling a dispatcher route.

## Test evidence

The focused corpus covers 37 exact V1 lifecycle snapshots: idle, begin, every
push and pass boundary for two- and three-pass policies, early relation
refusal, poisoned resume, claims-disabled mode, the empty frozen set, and every
explicit-slice cursor boundary. Its frozen compact fingerprint is
`0x7d85446a0e51325a`.

Additional gates:

- flip every byte of a completed compact image; validation remains total, and
  every accepted image is closed under V2 -> V1 decode/re-encode -> V2;
- refuse short and extended bodies, wrong active-width bindings, noncanonical
  booleans/selectors/slots/masks, and representative omitted V1 padding;
- compare direct native idle initialization byte-for-byte with projected V1
  idle;
- execute 384 bounded deterministic books through batch, V1 stream, and V2
  stream oracles, comparing the exact compact checkpoint after every call;
- execute the minimum and maximum active widths, including all 64 orders, 64
  owner identities, and 16 outcomes at the maximum;
- compare every explicit-slice cursor boundary and both two-pass and three-pass
  policy topologies after reopen;
- preserve exact poison transitions for changed-fill resumptions and byte
  atomicity for wrong-phase/not-in-progress feed errors;
- continue 23,339 structurally accepted single-byte checkpoint mutations
  without a V2 panic and compare exact V1 results/images wherever V1 itself has
  a defined debug transition (23,337 cases); the two excluded V1 transitions
  hit its known unchecked score multiplication, while V2 uses checked math;
- bounds-check first/last and one-past-end matrix and ledger accesses;
- run all 126 `clutch-batch` tests and Clippy with warnings denied.

Commands:

```text
cargo test --manifest-path crates/clutch-batch/Cargo.toml
cargo clippy --manifest-path crates/clutch-batch/Cargo.toml --lib -- -D warnings
RUSTFLAGS='-Zemit-stack-sizes' cargo build \
  --manifest-path crates/clutch-batch/Cargo.toml \
  --lib --release --target x86_64-unknown-linux-gnu
```

## Remaining promotion boundary

The complete Relation V1 feed engine now executes against V2 active-width
bytes, including admission, all allocation folds, explicit pairing slices,
settlement, composite fee hooks, score reconstruction, resumability, poisoning,
and canonical verdict persistence. V1 remains the migration/differential
oracle only. The remaining boundary is adapter promotion: authenticate widths
from frozen state, bind the outer codec version, measure SBF frames and compute
units, and enable a separately reviewed route. No V2 dispatcher or account
migration has been enabled by this lane.
