# Capability-profile size audit — 2026-08-23

## Classification and source

This audit measures source-derived SBF artifacts built from selected Git commit
`0ba5cfa586c9d4065918e301ee6e1e978b188242` and tree
`42a4a8cb8bcda8d80225e9d1631d40ba6d374cdb`. That commit equaled repository
`HEAD` before and after both complete diagnostic invocations. The producer exported that
commit with `git archive`; it did not build unrelated dirty worktree bytes.

The JSON record is
[`2026-08-23-current-head-capability-size-diagnostic.json`](../../programs/clutch-sbf/audit/evidence/2026-08-23-current-head-capability-size-diagnostic.json).
It is source-derived/selected-commit artifact evidence for ELF shape only.
Loader rent is a model over those selected artifact lengths. Nothing here is runtime,
deployment, public-cluster, release, production, or funded-account evidence.
The record has no semantic-owner or central-registry identity manifest and is
deliberately ineligible for the schema-V2 deployable-profile checker.

## Reproduction

The exact diagnostic command was:

```sh
python3 programs/clutch-sbf/scripts/measure_capability_profile_sizes.py \
  --commit 0ba5cfa586c9d4065918e301ee6e1e978b188242 \
  --profile full=profile-full \
  --profile direct-v3-source-v2-point=profile-direct-v3-source-v2-point \
  --profile general-source-v2-point=profile-general-source-v2-point \
  --cargo-default-profile full \
  --output programs/clutch-sbf/audit/evidence/2026-08-23-current-head-capability-size-diagnostic.json
```

The entire command completed twice with identical canonical JSON. The final
report SHA-256 is
`33f1253a1e0251ee343e655f50fd470e6c27dd901f148972a18836b80c09037c`.

Each explicit profile was built twice in fresh target directories with:

```text
cargo-build-sbf --manifest-path <archived-tree>/programs/clutch-sbf/program/Cargo.toml
  --arch v0 --offline --skip-tools-install --tools-version v1.53
  --no-default-features --features custom-heap,<profile-feature>
  --sbf-out-dir <fresh-output> -- --locked
```

The toolchain was `cargo-build-sbf 4.0.0`, `platform-tools v1.53`, and
`rustc 1.89.0-dev`. The JSON also pins the builder, rustc, `llvm-readobj`, and
`llvm-objdump` binary SHA-256 digests and all Python producer/input digests.
Reproducibility means two targets beneath a commit-keyed deterministic local
archive path. Repeating the command on this host reuses that absolute path
after removing the prior temporary build. Cross-host temporary-root paths can
still affect Cargo path dependency disambiguators, so this is not an
independent-rebuilder claim.

## Exact selected-commit measurements

The persistent loader model is exact-size Upgradeable Loader v3 allocation:

```text
persistent lamports = (ELF bytes + 45 + 128 + 36 + 128) * 6,960
```

It separates the transient recyclable Buffer. A future deployment may choose a
larger ProgramData `max_len`; that would lock rent for the chosen maximum, not
the current ELF length.

| Profile | ELF bytes | `.text` | `.rodata` | Exact-size persistent lamports | Explicit ELF SHA-256 |
| --- | ---: | ---: | ---: | ---: | --- |
| Full | 2,086,224 | 1,946,096 | 60,521 | 14,522,464,560 | `053df6b05717e4be6ab5926ef47d05b2a8c64110f6e6691413e203602bcb7c89` |
| Direct V3 + Source V2 + point | 1,059,824 | 1,013,488 | 7,457 | 7,378,720,560 | `47219fd0269ce0309f3bc1a25b97103744e34cf40bfb113bd394a9187dc0616c` |
| General + Source V2 + point | 1,438,168 | 1,322,904 | 56,921 | 10,011,994,800 | `bf8027d332fb7cbbeb3fe9813d9c0a2a05eae0eb165734bdd192095ea2f7facd` |

All explicit pairs were byte-identical. Each final ELF is `elf64-sbf`, has the
expected SBF shared-object headers, has no writable-executable segment, and
exposes only the observed syscall surface recorded in JSON. The final
unstripped disassembly covers every text-function address: the deepest direct
`r10` reference is 4,096 bytes in all three profiles. All 29 symbols named by
39 nonfatal backend stack-diagnostic lines are absent after final LTO. This is
a direct-frame/artifact-shape audit, not whole-control-flow or runtime proof.

Compared arithmetically with the comparison-only
[`2026-08-22` schema-V1 record](../../programs/clutch-sbf/audit/evidence/2026-08-22-capability-profiles.json)
at commit `625cd65ac0c17be3ed4371df5ab8f23db67b9eae`, selected-commit ELF growth is
3,112 bytes full, 2,960 direct, and 2,816 general. Those correspond to
21,659,520, 20,601,600, and 19,599,360 additional modeled persistent lamports
under the same exact-size formula.

### The illustrative 10 SOL line

At 6,960 lamports per billable byte, exactly 10 SOL permits at most a
1,436,444-byte ELF under this exact-size formula. This is an illustrative model
threshold, not a deployment quote, funding observation, price forecast, or
recommended ProgramData maximum.

- Direct is 376,620 bytes and 2,621,279,440 modeled lamports below the line.
- General is 1,724 bytes and 11,994,800 modeled lamports above it.
- Full is 649,780 bytes and 4,522,464,560 modeled lamports above it.

General's narrow historical margin has therefore disappeared. Treating a
1,724-byte trim as durable headroom would be fragile: ordinary source/toolchain
changes have already moved all three ELFs by several KiB.

## What materially drives `.text`

The diagnostic binds the exact `.text` base and exclusive end, rejects zero,
out-of-range, overlapping, or gapped regions, and deduplicates only folded
aliases with identical address and size. The canonical region union exactly
covers `.text` for every artifact. The largest groups are:

| Profile | Largest exact final-symbol groups |
| --- | --- |
| Full | `clutch_solana_layout` 404,336; `orders_batch` 327,944; `clutch_batch` 171,240; Direct V3 162,544; `observe_resolve` 103,640; legacy Direct 94,160; `resolution_work` 73,160 |
| Direct | `clutch_solana_layout` 230,376; Direct V3 162,528; `observe_resolve` 79,416; `genesis` 45,328; `market_init` 43,360; `clutch_solana_reference` 40,840 |
| General | `orders_batch` 291,920; `clutch_solana_layout` 270,304; `clutch_batch` 171,056; `observe_resolve` 79,416; `genesis` 44,040; `market_init` 43,360 |

The pairwise results make the capability drivers concrete:

- Full to Direct removes 1,026,400 ELF bytes (49.20%), 932,608 text bytes,
  and 7,143,744,000 modeled persistent lamports. The largest text reductions
  are `orders_batch` 301,168, layout 173,960, `clutch_batch` 170,928, legacy
  Direct 89,536, and `resolution_work` 73,160 bytes.
- Full to General removes 648,056 ELF bytes (31.06%), 623,192 text bytes, and
  4,510,469,760 modeled persistent lamports. Direct V3, layout, legacy Direct,
  and resolution work dominate that reduction.
- Direct to General adds 378,344 ELF bytes and 2,633,274,240 modeled lamports.
  General adds 265,144 `orders_batch` and 170,744 `clutch_batch` text bytes
  while omitting 157,528 Direct V3 text bytes; the net `.text` increase is
  309,416 bytes and `.rodata` increases 49,464 bytes.

Module names are not capability proofs. Narrow artifacts retain a few helpers
from nominally excluded modules because enabled handlers share those functions.
The semantic boundary is the entrypoint's admitted route set, not the source
file containing a reused helper.

## What the narrow artifact reductions establish

They are deployable-format, source-selected artifacts, but this diagnostic
does not establish that they are semantically honest subsets. Static source
inspection shows that:

- Cargo forwards one mutually exclusive profile feature through the program,
  layout, and reference crates;
- dispatch contains feature gates for disabled canonical coordinates; and
- the final linked `.text` reductions show that the feature selections let LTO
  remove material code rather than merely changing a label.

The measurement invocation did not execute the source tree's host tests or an
SVM fixture, and no retained runtime transcript is part of the JSON. Exact
disabled-coordinate refusal, account-read ordering, and admitted-route behavior
therefore remain separate promotion gates. No narrow-profile linked identity
manifest, public deployment, or runtime CU evidence exists here.

## Historical Cargo-default identity fork

The retained JSON records the pre-canonicalization result for source commit
`0ba5cfa`. Its explicit command enabled `custom-heap,profile-full`, while the
no-feature-arguments route also enabled Cargo's named `default` marker. The
marker expands to those same two behavioral features, but it remains a third
rustc feature-identity input. The old explicit and default builds were each
self-reproducible and had the same length, section sizes, syscall surface, and
frame counts, but their stripped ELF SHA-256 identities were respectively
`053df6…` and
`0dfab671ed8fb528a60e5f2296cad8cc449d7ebc8ddf8e0499822bf635dbbce9`.
The retained diagnostic correctly records `REFUSE` for those old invocations
and remains immutable historical evidence.

The cause and fix are now established in
[`DEFAULT_EXPLICIT_ELF_IDENTITY_AUDIT_2026-08-23.md`](DEFAULT_EXPLICIT_ELF_IDENTITY_AUDIT_2026-08-23.md).
Canonical explicit full builds retain the marker with
`--no-default-features --features custom-heap,default,profile-full`. Two fresh
canonical explicit builds and two fresh Cargo-default builds of the same
archived `0ba5cfa` tree all produced the `0dfab671…` artifact with an empty
strict-comparison mismatch set and `PASS`. Narrow profiles still omit
`default`, because enabling it would also enable `profile-full`.

The final-frame reporter now removes only rustc's per-compilation legacy symbol
hash from the named deepest function. This avoids a false mismatch in audit
metadata; it does not normalize or forgive a different stripped ELF hash.

## Current gates and safe optimization order

1. **Choose the product capability first.** If the intended product is the
   Direct profile, the measured 1,026,400-byte reduction is the safest large
   rent optimization candidate because its measured artifact is much smaller.
   Prove disabled coordinates fail closed separately; do not delete validation
   inside an admitted route to save bytes.
2. **For General, optimize the three measured owners first:** `orders_batch`,
   `clutch_solana_layout`, and `clutch_batch` own 733,280 final text bytes.
   Look for duplicate decode/validation passes, avoidable monomorphizations,
   and identical create/funding helpers. Preserve exact integers, refusal order,
   account authentication, and all adversarial tests.
3. **Use a real budget margin, not the 10 SOL near-miss.** A 1,724-byte trim
   would cross today's illustrative line but not provide stable release
   headroom. Freeze an explicit ProgramData `max_len` and rent ceiling only in
   a linked profile manifest.
4. **Keep host-only compilers and model-only successor owners offchain.** The
   product compiler, dealer, recovery, retirement, and Series models should not
   enter SBF until their executable account/intent contracts are selected.
   Current absence is not a runtime capability claim.
5. **Measure multi-program composition as a system before splitting.** A
   sibling program can reduce one ELF while adding a second Program/ProgramData
   rent principal, CPI CU, metas, upgrade coordination, and atomicity risk.
   Compare total persistent rent and runtime behavior, not ELF size alone.
6. **Close promotion gates separately from size work.** The strict producer
   still has no fully linked eleven-owner/central-registry manifests and the
   active worktree closure is dirty. A diagnostic JSON cannot substitute for
   those identities, a clean linked build, narrow SVM tests, or a release
   manifest.
