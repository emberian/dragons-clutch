# Cargo-default / explicit-full ELF identity audit

Date: 2026-08-23

Program source under test: `0ba5cfa586c9d4065918e301ee6e1e978b188242`

Source tree: `42a4a8cb8bcda8d80225e9d1631d40ba6d374cdb`

Evidence class: offline, source-derived local build evidence; not deployment,
runtime, devnet, release, or production evidence.

## Finding

The two old invocations did not present the same complete feature set to
rustc. Cargo's default route enabled `custom-heap`, `profile-full`, **and the
named marker `default`**. The explicit route enabled only `custom-heap` and
`profile-full`.

The `default` marker does not select any additional program behavior: a search
of the first-party program and its path dependencies found no
`cfg(feature = "default")` or `CARGO_FEATURE_DEFAULT` consumer. It still enters
rustc's crate identity. Under fat LTO, that identity difference changed Rust
symbol hashes and the ordering of otherwise equivalent generic
monomorphizations. The final artifacts had equal size, section layout, syscall
surface, and stable symbol inventory, but different `.text` ordering,
relocations, and SHA-256 identities.

The canonical explicit full invocation is therefore:

```text
cargo-build-sbf ... \
  --no-default-features \
  --features custom-heap,default,profile-full \
  ... -- --locked
```

Narrow profiles must not enable `default`, because that marker expands to
`profile-full`.

## Controlled reproduction before the fix

All four builds used `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, SBF rustc
`1.89.0`, fresh target directories, offline resolution, the same archived Git
tree and source path, release fat LTO, one codegen unit, and overflow checks.
Each route reproduced across two fresh targets:

| Route | Complete `clutch-sbf` feature fingerprint | Runs | Stripped ELF SHA-256 |
| --- | --- | ---: | --- |
| old explicit | `custom-heap`, `profile-full` | 2 | `053df6b05717e4be6ab5926ef47d05b2a8c64110f6e6691413e203602bcb7c89` |
| Cargo default | `custom-heap`, `default`, `profile-full` | 2 | `0dfab671ed8fb528a60e5f2296cad8cc449d7ebc8ddf8e0499822bf635dbbce9` |

Cargo's `lib-clutch_sbf.json` fingerprints agreed on rustc, target, profile,
source path, dependencies, rustflags, configuration, and compile kind. Their
`features` arrays were the observed differing input.

Both deployable ELFs were exactly 2,086,224 bytes with a 1,946,096-byte
`.text`, a 60,521-byte `.rodata`, ten identical sorted undefined dynamic
symbols, 1,080 final text-function addresses, a deepest direct `r10` offset of
4,096 bytes, and no backend-diagnosed symbol surviving final LTO.

Byte-level comparison found 489 differing bytes:

| Section | Bytes | Differing bytes | Explicit SHA-256 | Cargo-default SHA-256 |
| --- | ---: | ---: | --- | --- |
| `.text` | 1,946,096 | 483 | `a61ec1a858152e7aa003a35bd254fcabef5629d25c0de8701017bb7120481c77` | `807302649278c685c2ac0480d615bd0ea2013d32d4ab99a47aff1738558dda4e` |
| `.rel.dyn` | 54,240 | 6 | `db68463e0724afeb3b4ea26c2abc8fb7662e5bd2e4708f872ec6c51fe8389ec8` | `4b0755c3e8703b651226a2f968d78879e7a0c5537ae1d08b9aeae6a2c6200c3b` |
| all other sections | — | 0 | identical | identical |

The last row means `.rodata`, `.data.rel.ro`, `.dynamic`, `.dynsym`, `.dynstr`,
and `.shstrtab` were individually byte-identical; it deliberately does not
sum unlike ELF regions into a protocol quantity.

After removing only rustc's trailing 16-hex symbol hashes, both unstripped
ELFs exposed the same 1,080 stable function identities. Of singleton stable
identities, 594 had different raw rustc hashes. One generic `Vec::from_iter`
stable identity had three monomorphizations whose 504-byte and 528-byte
regions exchanged positions. The changed call displacements and the first
three relocation offsets account for the observed deployable-byte fork; there
was no section-size or syscall-surface fork.

## Controlled reproduction after canonicalization

The diagnostic was rerun against the same archived `0ba5cfa` source and pinned
toolchain after changing only the explicit feature-list construction. Two
fresh explicit builds and two fresh Cargo-default builds all produced:

- stripped ELF SHA-256
  `0dfab671ed8fb528a60e5f2296cad8cc449d7ebc8ddf8e0499822bf635dbbce9`;
- 2,086,224 ELF bytes, 1,946,096 `.text` bytes, and 60,521 `.rodata` bytes;
- the same ten undefined dynamic symbols and the same final-frame audit; and
- an empty strict-comparison mismatch set.

The diagnostic's fail-closed V2-equivalence result changed from `REFUSE` to
`PASS`. This controlled intervention establishes that retaining the `default`
identity marker is sufficient to remove this fork under the pinned build
conditions. It is not a general cross-toolchain reproducible-build claim.

## Regression boundary

`check_capability_profile.cargo_features` is the semantic owner for the linked
producer's complete feature list. It now records `default` for every full
profile, while narrow profiles continue to omit it. The non-promotable size
diagnostic calls that same owner rather than maintaining a parallel rule. Unit
tests pin both call paths, and the actual pinned-toolchain diagnostic remains
the byte-level gate:

```sh
python3 programs/clutch-sbf/scripts/measure_capability_profile_sizes.py \
  --profile full=profile-full \
  --cargo-default-profile full \
  --commit 0ba5cfa586c9d4065918e301ee6e1e978b188242 \
  --output /tmp/default-explicit-identity.json
```

The result is acceptable only when `byte_identical_to_explicit_profile` is
true, `mismatches` is empty, and `strict_v2_default_equivalence_gate` is
`PASS`. The linked producer continues to refuse a full-profile manifest if its
default build differs in any comparable artifact, syscall, frame, or loader
field.
