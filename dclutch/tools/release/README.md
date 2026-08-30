# Checked-release tooling

## Final generated convergence

After every protocol and caller source lane is frozen, converge all generated
repository projections once from one clean, exact commit:

```sh
tools/release/final-generated-convergence.py --plan
tools/release/final-generated-convergence.py \
  --write --expected-head <full-40-hex-commit>
tools/release/final-generated-convergence.py \
  --check --expected-head <same-full-40-hex-commit>
```

The writer runs entirely offline and in this fixed order: every tracked Cargo
workspace lock, every paired SDK/web `abi:*` writer, both ABI coverage
ratchets, `tools/genref/generate.sh`, `tools/sbom/sbom_check.py`, then the full
read-only verification again. It refuses a moving or dirty source tree, a
workspace without its adjacent tracked lock, an ABI writer without a matching
byte verifier, and any generated change outside these owners:

- adjacent `Cargo.lock` files for tracked Cargo workspace roots;
- `packages/dclutch-sdk/lib/generated/` and
  `apps/dclutch-web/lib/generated/`;
- `docs/reference/`, wholly owned by GENREF;
- `tools/sbom/SBOM.md` and `tools/sbom/NOTICES.md`.

ABI coverage is intentionally not rewritten by this batch. A new hand-stated
magic, domain, or byte coordinate must be converted to a generated authority
or receive explicit review before its ratchet baseline changes. A stale lock
that cannot resolve offline likewise stops the batch; do not weaken a version
constraint merely to make SBOM generation proceed.

`checked-release-candidate.sh` builds the complete offline release candidate.
It is local evidence, not a deployment, and it never signs, submits, funds, or
publishes anything.

## Fresh-build rule

Use a new absolute `--work` root for every candidate you intend to admit. The
runner enumerates `programs/*/Cargo.toml`; that directory is the sole owner of
the frame-gated link set. Ten current packages produce release artifacts and
three are frame-gate-only, so the present summary reports thirteen links.

Each per-link log is truncated and stamped with a new run identifier and the
exact build invocation before `cargo build-sbf` starts. The old deploy output
for that artifact is removed first. The run refuses unless the invocation emits
a new regular, non-symlink artifact and every log has both stamps and Cargo's
top-package `Compiling <package>` line. This distinction is
load-bearing: a warm target can make Cargo emit no compiler diagnostics because
it invoked no compiler. Silence from that run is not a zero-diagnostic build.

Every SBF invocation also passes Cargo's `--locked` admission through
`cargo-build-sbf`. If a root or nested lockfile is stale, the release refuses
instead of silently resolving another dependency graph and modifying the
source checkout.

The runner also hashes every `Cargo.lock` in the archived source before the
first build and after the complete candidate, including the source-pinned host
release tool. It refuses any added, removed, or changed lock and preserves both
manifests as `cargo-locks-before.tsv` and `cargo-locks-after.tsv`. The summary's
`cargo_lock_count`, `cargo_lock_set_sha256`, and
`cargo_lock_immutability=passed` bind that repository-wide check. The Upgrade
gate's v1 JSON shape is unchanged; its source-tree digest already binds every
committed lock byte. The host tool itself builds `--locked --offline`.

After the ordinary artifact build is clean, the runner performs a separate
fresh measurement build for each of the same thirteen links with
`-Zemit-stack-sizes`. The measurement objects are never shipped. The runner
refuses a missing top-package compile marker, an empty frame report, or any
frame at or above the 4,096-byte SBPF v0 bound.

The two builds are joined rather than confused. For every link the runner emits
`provenance/<label>.json` with schema `dclutch-sbf-link-provenance-v1`. That
descriptor binds the named role/package, source commit and source-tree digest,
build run, exact plain invocation/log/compile marker, exact shipped ELF, exact
frame invocation/log/object/report, and their hashes. A same-named output in an
adjacent target directory, a digest-identical renamed copy, a symlink, or an old
deploy output is not selectable through this descriptor.

Before the source freeze, forecast the one batched hbox build instead of
discovering changed links one rebuild at a time:

```sh
tools/release/plan-sbf-release-batch.py \
  --base <last-accepted-source> \
  --candidate <candidate-source> \
  --output /private/tmp/dclutch-sbf-batch-plan.json
```

The static plan must enumerate exactly thirteen links. It computes each SBF
package's local non-dev dependency-closure digest and names the changed inputs.
It is scheduling evidence only: it predicts which content-addressed artifacts
must be built once after the family freeze. The actual provenance descriptor is
what frame diagnostics, import/symbol audit, caller proof, and CU consumers must
select; none may rediscover or rebuild a nearby ELF.

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
`sbf_build_diagnostics_accepted=false`. It must also say
`cargo_lock_immutability=passed`. Preserve `build-links.tsv`,
`build-run.txt`, `source-tree.txt`, every `build-*.log`, every
`frame-build-*.log`, the `frame/` reports, `build-diagnostics.txt`, and both
Cargo lock manifests with the candidate evidence. Preserve every `provenance/`
descriptor and its referenced frame object as well.

## Upgrade gate

A clean run also emits `CHECKED_UPGRADE_GATE.json` and prints its SHA-256. The
gate is generated only after the fresh all-link build, static frame gate, and
checked release manifests complete. It binds the source commit/tree, exact
thirteen-link identities, run stamps and compile markers, build and frame logs,
frame counts, every release ELF, each per-link provenance descriptor, and each
checked manifest.

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

The bounded selector used by downstream consumers is:

```sh
tools/release/artifact_provenance.py select-gate-role \
  --gate /absolute/candidate/CHECKED_UPGRADE_GATE.json \
  --gate-sha256 <separately-recorded-gate-sha256> \
  --role trading
```

It refuses a noncanonical gate path, anything other than the exact all-thirteen
set, a wrong role/package map, a stale source/run/log/object/report/ELF, and an
adjacent or renamed file. Its JSON result names the only ELF path that a
consumer may open.

## All-workspace gate

The checked candidate owns the shipped SBF link set. It does not replace the
repository-wide Cargo gate: the root workspace cannot see independent fixture,
program-test, generator, and tool workspaces. Run the dynamic archived-source
gate at the same accepted commit:

```sh
tools/release/check-all-workspaces.py \
  --work /private/tmp/dclutch-all-workspaces-<commit>-<unique-run> \
  --commit <commit>
```

The output root must not exist. The checker discovers every archived
`Cargo.toml` with its own `[workspace]` table, requires its adjacent committed
`Cargo.lock`, and runs `cargo check --workspace --all-targets --locked
--offline` in a fresh isolated target. It hashes every archived `Cargo.lock`
before and after the full run, including stray member locks that Cargo never
reads, so the summary's `cargo_lock_count` is discovered rather than frozen to
a historical number. A release result is green only when the pass count equals
the workspace count and complete lock immutability says `passed`.

On hbox, place the work root on `/tank` and keep the whole run under the shared
machine's scheduler:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
  tools/release/check-all-workspaces.py \
    --work /tank/dregg-build/dclutch-all-workspaces-<commit>-<unique-run> \
    --commit <commit>
```

`--inventory-only` is a quick way to inspect the discovered workspace and lock
counts. It intentionally emits no admitted summary.

## Nested lockfile sweep

Every workspace above refuses a lock it cannot resolve, so one stale nested
`Cargo.lock` reds the gate from a crate the lane never opened. Adding an
in-repo crate edge to a widely-depended crate (`dclutch-operator` is the usual
one) drifts every nested workspace that reaches it, and the drift is invisible
until a `--locked` build. It has cost this repository a full gate rebuild more
than once. The check is seconds and needs no build:

```sh
for lock in $(git ls-files '*Cargo.lock'); do
  (cd "$(dirname "$lock")" && cargo metadata --locked --offline >/dev/null) \
    || echo "STALE $lock"
done
```

Fix a stale one with `cargo metadata --offline` in its directory — offline, so
nothing resolves off the network and no third-party version can move. Confirm
the diff is purely additive before committing it.

## Publishing the deployment manifest's activation-cache hint

`DEVNET_DEPLOYMENT_V1.activationCache` is a bootstrap hint, not an answer. A
cohort activates a new release set, which mints a new cache at a new PDA, so
the shipped address ages by itself. `openReleaseBoundSessionV1` already
survives that — it follows the chain past a hint whose pinned deployment slots
no longer match the live programs — which is precisely why nobody noticed the
shipped hint going four cohorts stale on 2026-08-29: a superseded cache keeps
its Registry owner, its `DCLTACT1` magic and its exact width forever, so every
cheap health check on it passes and only its content ages.

The value is therefore generated, never typed:

```sh
cd packages/dclutch-sdk
node scripts/derive-activation-hint.mjs            # report; exit 1 on drift
node scripts/derive-activation-hint.mjs --write    # rewrite both manifest twins
```

It imports `discoverCurrentActivationCacheV1` from the SDK rather than
restating its rule, so the publish and the client answer "which release is
live?" with the same code. Two RPC rounds: every 1,288-byte account the
Registry owns, then the five live ProgramData deployment slots; the current
cache is the single one whose five pinned slots equal those five.

Wire it into a publish as an INFORMATIONAL step, not a gate. A stale hint costs
a reader accuracy and costs a session nothing, and a check that can block a
real deploy over a cosmetic field is a check someone will delete. The wrapper
publish script runs it before the archive-sync and prints the remedy on drift.

Two things it deliberately does not do:

- it never joins `final-generated-convergence.py`. That batch is offline and
  refuses generated changes outside its owner list; this reads a live cluster,
  so its output legitimately differs between two runs of the same commit;
- it never stamps the slot the reading happened at. That advances constantly,
  so stamping it would make the generator report drift against its own last
  output and produce a diff on every publish. The block records facts about the
  ANSWER — cache address, release set, pinned Core deployment slot — which move
  only when a cohort does.

After `--write`, the literal pin in `apps/dclutch-web/lib/deployments.test.ts`
still wants a human, and a new cohort may want a new ABI table in
`packages/dclutch-sdk/lib/releaseIdentity.ts`. The script says both on exit.
