# Planned offline scripts

Future scripts may reproduce proofs/builds, run trust and provenance audits,
generate canonical fixtures, measure local program-test resources, and build the
static release artifact.

Scripts must default to offline/local operation, avoid wallet and secret paths,
record exact inputs and tool versions, and refuse any remote mutation or network
deployment action. A convenience command may not weaken an invariant or proof
gate to obtain a green result.

## Landed

### `baseline_manifest.py`

Derives and checks `MANIFEST.baseline.json`, the baseline evidence manifest:
git baseline identity, the `CODEX_HANDOFF.md` §5 gate inventory plus documented
low-cost research/model/frontend extensions with their reviewed dispositions,
the §6 byte identities recomputed from the tree, and the pinned toolchain
records. Standard library only, offline, no writes outside the manifest path.

```sh
scripts/baseline_manifest.py emit --run-gates    # refuses on a dirty tree
scripts/baseline_manifest.py emit --allow-dirty --run-gates
scripts/baseline_manifest.py check [--run-gates] # exit 1 on drift
```

`emit` is `--strict` by default and refuses to write a manifest from a dirty
working tree, because such a manifest would pair a clean commit id with bytes
that are not in that commit. `--allow-dirty` emits a snapshot marked
`"dirty": true` for mid-flight use only.

It attests no release, signature, reproducible-build closure, formal proof
content, production source release, direct-selection promotion, or system-wide
terminal/liveness closure. The isolated Verus batch shadow remains absent until
its source and reproduction gate are committed. See
[`docs/implementation/BASELINE_MANIFEST.md`](../docs/implementation/BASELINE_MANIFEST.md).

Focused declaration checks are standard-library only:

```sh
python3 -m unittest scripts/test_baseline_manifest.py
```
