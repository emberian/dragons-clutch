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
    --work /tank/dclutch-build/checked-<commit>-<unique-run> \
    --commit <commit>
```

The runner deliberately does not discover or invoke `swarm-build` itself. That
keeps the scheduling boundary visible and prevents recursive wrapping. Refuse a
run on hbox if the outer wrapper is absent; do not silently fall back.

The admitted summary must say `sbf_build_freshness=passed`,
`sbf_build_freshness_links=13`, `sbf_build_diagnostics_total=0`, and
`sbf_build_diagnostics_accepted=false`. Preserve `build-links.tsv`,
`build-run.txt`, every `build-*.log`, and `build-diagnostics.txt` with the
candidate evidence.
