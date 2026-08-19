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

Derives and checks the schema-v2 baseline evidence manifest: git content
identity, the current documented offline gate inventory, named tree and derived
identities, and pinned toolchain records. Standard library only, offline, no
writes outside the manifest path.

```sh
scripts/baseline_manifest.py emit --run-gates    # refuses on a dirty tree
scripts/baseline_manifest.py emit --allow-dirty --run-gates
scripts/baseline_manifest.py check [--run-gates] # exit 1 on drift
```

`emit` is `--strict` by default and refuses to write a manifest from a dirty
working tree, because such a manifest would pair a clean commit id with bytes
that are not in that commit. `--allow-dirty` emits a snapshot marked
`"dirty": true` for mid-flight use only.

It attests no release, signature, reproducible-build closure, whole-system
formal proof, global liveness, production provider closure, direct-selection
promotion, or terminal closure. The sealed R1 gate verifies a bounded measured
ResolutionWork profile from committed ELF/log evidence; it does not broaden
those claims. The batch scalar shadow, narrow transfer refinement, and finite
B-spline Lean/Rust bridge are named bounded lanes, not whole-system proof. The
signed committed-walk gate records 22 local signed transactions from an
11-prerequisite genesis-assisted prestate; it is not blank-bank, deployment, or
public-cluster evidence. The
root Verus probe accepts only its intended exit 1; missing/off-pin/digest-drift
setup exits are failures. `MANIFEST.baseline.json` is historical schema v1 and
cannot pass the v2 checker until a later clean v2 emission; this task must not
edit it. A full `--run-gates` run is deliberately slow (potentially tens of
cache-cold minutes, including bounded local SBF rebuilds), not a presubmit. See
[`docs/implementation/BASELINE_MANIFEST.md`](../docs/implementation/BASELINE_MANIFEST.md).

Focused declaration checks are standard-library only:

```sh
python3 -m unittest scripts/test_baseline_manifest.py
```
