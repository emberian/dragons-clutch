# Cohort 18 intermediate eight-link frame evidence — 2026-09-07

This record admits the reproducible frame baseline for source revision
`a628bfd39dc971b5e37f20d9482290da3f9f6160`. It is intermediate evidence for
the release lane. It does not establish current runtime readiness, devnet
deployment, publication readiness, or a final cohort release: later commits
changed sources in SBF link closures and still owe new frame rows.

## Accepted frame pair

The captures were built on hbox from a clean detached checkout with the outer
`swarm-build` wrapper and separate output files:

```text
source checkout:
  /tank/dregg-build/dclutch-a628-frame-source-20260907
capture root:
  /tank/dregg-build/dclutch-a628-frames-20260907
source revision:
  a628bfd39dc971b5e37f20d9482290da3f9f6160
capture command:
  SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=6 swarm-build tools/gate frames \
    --at a628bfd39dc971b5e37f20d9482290da3f9f6160 --capture <file>
```

The exact capture and accepted-baseline digests are:

| Artifact | Hbox path | SHA-256 |
| --- | --- | --- |
| Capture A | `/tank/dregg-build/dclutch-a628-frames-20260907/frame-a.json` | `1fe86248209abb7f1ec4c40bab9666a547009c1890cb2840248959179e7cedb7` |
| Capture B | `/tank/dregg-build/dclutch-a628-frames-20260907/frame-b.json` | `1fe86248209abb7f1ec4c40bab9666a547009c1890cb2840248959179e7cedb7` |
| Accepted run-local baseline | `/tank/dregg-build/dclutch-a628-frames-20260907/frame-baseline.json` | `b4a059eac44cd278e41be32859dd10e46eb749b6c8d457982c8f32b4f0ce7eeb` |

Both manifests name the same revision and have schema
`dclutch-sbf-frame-manifest-v1`, eight links, an empty diagnostics array, and
1,864 function-frame rows. The maximum measured frame is 3,968 bytes against
the 4,096-byte bound; zero rows are at or above the bound. `tools/gate frames
accept` admitted the pair and `tools/gate frames check` passed against capture
A. A recursive scan of the capture output found no `already loaded`,
`executed nothing`, `no-op`, or `Unit run-u*.scope` markers.

The accepted file is now the tracked
`tools/gates/frames-baseline.json`, replaced atomically from the validated
accepted artifact. Its SHA-256 is
`b4a059eac44cd278e41be32859dd10e46eb749b6c8d457982c8f32b4f0ce7eeb`.

## Fresh strict candidate

The separate checked candidate was read from:

```text
/tank/dregg-build/dclutch-cohort18-dealer-strict-a628bfd39-20260907
```

Its `SUMMARY.txt` reports `release_builder=true`,
`sbf_build_freshness_links=8`, `sbf_build_diagnostics_total=0`,
`sbf_build_diagnostics_accepted=false`, `cargo_lock_immutability=passed`, and
`spline_product_handoff=passed`. The checked gate is
`dclutch-checked-upgrade-gate-v1`; the campaign pack identifies the artifact
as `local-reproducible-checked-release-campaign-input` and
`not_a_deployment=true`.

```text
source revision:
  a628bfd39dc971b5e37f20d9482290da3f9f6160
checked gate SHA-256:
  7791a357bdaa390d86a82e3a8483bc598556b6f90b5988822e7221fe79b438ee
RELEASE_GATE.json SHA-256:
  18679fb637d16daab2371588f408a102f6b4da273885da8607664c77fe8feabe
```

The eight ELF artifacts and hashes recorded by the checked gate are:

| Link | Candidate path | ELF SHA-256 |
| --- | --- | --- |
| accelerator | `elf/accelerator.so` | `1fd892a8ad19baa55a046b263fd8f07286beed5a8fc1e967dbfeef67155776bb` |
| claims | `elf/claims.so` | `19a3be2fcb625bf56166f8b638adba2e7d970421d1a08e9fdd1bc63d7ddaef5d` |
| core | `elf/core.so` | `7013924151b5fbd1f3d5abf0c88fb6199adaf2a6ba7f2a91da89014c4cf086a9` |
| custody | `elf/custody.so` | `b936b48790ac89ff65d36b28562674fc95f4ba4e6e0f8a763122a6c4baed6422` |
| registry | `elf/registry.so` | `8eb3ccc0e9d0f895521be92b48f5ce6ac912fdca17c79018148581056150fc54` |
| rent | `elf/rent.so` | `100f211918acc5764fd797b4e2070bc0dd7b1b6ce095351c2494ac6355653143` |
| resolution | `elf/resolution.so` | `c381428fbeb8e266b2824257491c417c1acb0e57dcbd7f25542928e7c27347e5` |
| trading | `elf/trading.so` | `d334cc52c57d796f8d1beccff1d8a273688e626246008ede64b2e500064a1e38` |

No program IDs, deployment signatures, or poststates are asserted by this
candidate record because it is explicitly marked as not a deployment.

## Outstanding frame debt

After the baseline was admitted, `tools/gate frames owed --baseline
tools/gates/frames-baseline.json --until HEAD` refused with four source
commits that changed SBF link closures without carrying frame rows:

```text
4b44dbdc  trading: derive series prepare projected custody
  dclutch-accelerator-sbf, dclutch-trading-sbf
26f5d98b  resolution: derive bounded dynamic funding ledger width
  dclutch-core-sbf, dclutch-resolution-proof-sbf
c4c40793  general: derive order rows from authenticated signed terms
  dclutch-accelerator-sbf, dclutch-core-sbf, dclutch-trading-sbf
238b5aef  resolution: bind funding masks to authenticated policy
  dclutch-core-sbf, dclutch-resolution-proof-sbf
```

Those debts remain owned by their respective lanes. A later final-ready
release must capture the current integrated commit and settle this ledger
before deployment or publication evidence is interpreted.

## Dated precision correction — 2026-09-07

The earlier statement that a recursive scan found no no-op markers refers only
to the three JSON files in
`/tank/dregg-build/dclutch-a628-frames-20260907`; the outer `swarm-build`
stdout and stderr were returned by SSH and were not retained as log files.
That JSON-only scan cannot refute a silent no-op in the outer command and must
not be read as logfile evidence. The retained evidence is the frame tool's
actual freshness enforcement: each capture has all eight links, records the
exact source revision, and the tool requires fresh top-package compile markers
while rejecting incomplete links and both over-bound diagnostic forms.
