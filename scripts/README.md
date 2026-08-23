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
setup exits are failures. `MANIFEST.baseline.json` is schema v2 (101 executed gates) and
cannot pass the v2 checker until a later clean v2 emission; this task must not
edit it. A full `--run-gates` run is deliberately slow (potentially tens of
cache-cold minutes, including bounded local SBF rebuilds), not a presubmit. See
[`docs/implementation/BASELINE_MANIFEST.md`](../docs/implementation/BASELINE_MANIFEST.md).

Focused declaration checks are standard-library only:

```sh
python3 -m unittest scripts/test_baseline_manifest.py
```

### `rent_capital_time_audit.py`

Reproduces the dated rent/capital-time review's exact integer arithmetic for
loader-v3 persistent rent, active-width ClearWork and CandidateFeed comparisons,
ReceiptPage crossover and full-book projection, finite-Series capital-time, and
the compressed claim-basis model. Its JSON output labels historical artifact,
source-derived, and model-only values separately and expressly makes no current
linked ELF, CU, account-meta, stack, deployment, or cluster claim.

The check mode also verifies SHA-256 provenance for every source used by its
frozen examples:

```sh
python3 scripts/rent_capital_time_audit.py --check
python3 -m unittest scripts/test_rent_capital_time_audit.py
python3 scripts/rent_capital_time_audit.py
```

See
[`docs/reviews/RENT_COMPUTE_CAPITAL_TIME_AUDIT_2026-08-23.md`](../docs/reviews/RENT_COMPUTE_CAPITAL_TIME_AUDIT_2026-08-23.md).

### `dependency_license_check.py`

In-repo original of the dependency/license closure checker the Persvati
portable attestation jobs run (job copy name: `dependency_license_check.py`).
The default mode is the attested fixed twelve-locked-manifest scope and is
byte-stable: at the `6743b9d` archive it reproduces the attested
`SUMMARY manifests=12 unique_rows=888 failures=0 status=PASS` byte-for-byte.
Do not extend or reformat the default mode; revise it only together with a new
attestation methodology.

```sh
scripts/dependency_license_check.py [root]            # attested 12-manifest scope
scripts/dependency_license_check.py --complete [root] # every tracked Cargo.lock
                                                      # + package.json, writes the
                                                      # SBOM TSV under
                                                      # research/liveness-policy-profile/
```

Per package it requires offline lock resolution, a registry checksum in the
lock, a license expression or digest-pinned license file, path dependencies
inside the repository, and no git/unknown sources; failures print as `FAILURE`
rows and exit 1, never suppressed. The vendored crate's standalone lock is
recorded as `VENDORED covered-by=programs/clutch-sbf/Cargo.toml` because cargo
cannot process it outside its vendoring workspace and its packages are checked
through that workspace. `--complete` is not yet a declared baseline-manifest
gate; folding it into the gate inventory belongs to the next manifest emission
cycle so the sealed gate outputs stay byte-stable.

```sh
python3 -m unittest scripts/test_dependency_license_check.py
```
