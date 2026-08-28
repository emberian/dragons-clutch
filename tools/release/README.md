# Checked-release tooling

`checked-release-candidate.sh` builds the complete offline release candidate.
It is local evidence, not a deployment, and it never signs, submits, funds, or
publishes anything.

## Fresh-build rule

Use a new absolute `--work` root for every candidate you intend to admit. The
runner enumerates `programs/*/Cargo.toml`; that directory is the sole owner of
the frame-gated link set. Ten current packages produce release artifacts and
three are frame-gate-only, so the present summary reports thirteen links.

Each per-link log is truncated and stamped with a new run identifier before
`cargo build-sbf` starts. The run refuses unless every log has both that stamp
and Cargo's top-package `Compiling <package>` line. This distinction is
load-bearing: a warm target can make Cargo emit no compiler diagnostics because
it invoked no compiler. Silence from that run is not a zero-diagnostic build.

Every SBF invocation also passes Cargo's `--locked` admission through
`cargo-build-sbf`. If a root or nested lockfile is stale, the release refuses
instead of silently resolving another dependency graph and modifying the
source checkout.

After the ordinary artifact build is clean, the runner performs a separate
fresh measurement build for each of the same thirteen links with
`-Zemit-stack-sizes`. The measurement objects are never shipped. The runner
refuses a missing top-package compile marker, an empty frame report, or any
frame at or above the 4,096-byte SBPF v0 bound.

`--keep-elf` is retained only to give old invocations a precise refusal. Reused
ELFs and prior logs cannot qualify a new checked-release candidate.

Run the focused adversarial tests with:

```sh
bash tools/release/test-checked-release-freshness.sh
```

## hbox

hbox is shared build infrastructure. Wrap the entire runner once so one cgroup
contains the host-tool and all sequential SBF child builds:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
  tools/release/checked-release-candidate.sh \
    --work /tank/dregg-build/dclutch-checked-<commit>-<unique-run> \
    --commit <commit>
```

The runner deliberately does not discover or invoke `swarm-build` itself. That
keeps the scheduling boundary visible and prevents recursive wrapping. Refuse a
run on hbox if the outer wrapper is absent; do not silently fall back.

The admitted summary must say `sbf_build_freshness=passed`,
`sbf_build_freshness_links=13`, `sbf_build_diagnostics_total=0`, and
`sbf_build_diagnostics_accepted=false`. Preserve `build-links.tsv`,
`build-run.txt`, `source-tree.txt`, every `build-*.log`, every
`frame-build-*.log`, the `frame/` reports, and `build-diagnostics.txt` with the
candidate evidence.

## Upgrade gate

A clean run also emits `CHECKED_UPGRADE_GATE.json` and prints its SHA-256. The
gate is generated only after the fresh all-link build, static frame gate, and
checked release manifests complete. It binds the source commit/tree, exact
thirteen-link identities, run stamps and compile markers, build and frame logs,
frame counts, every release ELF, and each checked manifest.

For this gate, run the runner from the exact source commit and let it build
`dclutch-release-tool` from that archived source. An invocation whose runner or
freshness-checker bytes differ from `--commit` refuses. Supplying `--tool` may
still produce local candidate evidence, but it does not emit an Upgrade gate
because that host binary is not source-pinned by this workflow.

Keep the complete work directory together. It is relocatable only as one
directory tree: every gate path is canonical relative to its root. Do not edit
the gate, move a referenced file within the root, or replace anything with a
symlink. `devnet-upgrade-v1` requires the separately recorded gate digest and
source commit/tree, rehashes every referenced file, and requires `--elf` to be
the selected permanent role's exact canonical gate file. A handwritten
`checked_release_accepted: true` document has no authority.
