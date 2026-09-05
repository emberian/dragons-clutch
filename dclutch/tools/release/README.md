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

## Successor campaign release pack

A strict checked-candidate run now also emits
`SUCCESSOR_CAMPAIGN_PACK.json`. This is not another release format and does not
rebuild an ELF. It is the campaign-facing join over the same all-shipped-link
Upgrade gate:

- the exact source revision/tree and immutable Cargo lock set;
- pinned host, SBF, platform-tools, Solana and target toolchains;
- the preserved official Node v26.4.0 distribution plus the exact Node/npm
  member digests used to build the public CLI;
- all ten shipped ELF/checked-manifest identities and all twelve frame
  reports;
- the execution release set, the exact 144-byte predecessor profile input,
  and the successor infrastructure profile IDs and bytes;
- the source-owned compute, frame and packet ceilings;
- the source-bound SBOM, notices and licence-verifier pointers; and
- the seven exact role bindings needed by the existing successor runner; and
- an executed, key-free public CLI → Rust spline compiler → SDK inspection
  handoff over the canonical degree-2 fixture and all five emitted records.

The directory is the pack and must remain together. Verify it after a copy or
before a campaign:

```sh
/absolute/candidate/source/tools/release/successor_campaign_pack.py verify \
  --pack /absolute/candidate/SUCCESSOR_CAMPAIGN_PACK.json
```

Turn one authenticated Market input into the existing runner's exact
`dclutch-local-successor-run-spec-v2` shape with:

```sh
tools/release/successor_campaign_pack.py materialize-spec \
  --pack /absolute/candidate/SUCCESSOR_CAMPAIGN_PACK.json \
  --market /absolute/market.json \
  --run-root /absolute/new-campaign-run \
  --record-publication transaction \
  --rpc-port 31890

cargo run --release --locked --offline \
  --manifest-path /absolute/candidate/source/tools/local-validator/bootstrap/successor/Cargo.toml \
  -- run --spec /absolute/new-campaign-run/spec.json \
  --keypair-seed <64-lowercase-hex>
```

`materialize-spec` reauthenticates the full gate first, then writes
root-relocatable ELF selections into canonical absolute campaign paths and
generates the launcher's existing attestation shape from those selections. It
derives Resolution's semantic identity from the protocol-owned V4 release
preimage; it never copies the checked candidate's deliberately `unowned`
Resolution semantic placeholder into a campaign. A substituted ELF, release,
program identity, frame report, budget authority, SBOM, or attestation refuses
before a validator is launched.

After the current-source private lifecycle has executed infrastructure
succession, bind its Rust-authored finalized-chain lineage back to the exact
pack that supplied the campaign. This does not reinterpret that document: the
private-lifecycle supervisor remains its semantic verifier. The pack tool
rehashes it and requires its source, gate, seven checked artifacts, execution
release set and V1→V2 profile join to be the ones this pack selected:

```sh
/absolute/candidate/source/tools/release/successor_campaign_pack.py bind-lineage \
  --pack /absolute/candidate/SUCCESSOR_CAMPAIGN_PACK.json \
  --lineage /absolute/campaign-run/infrastructure-lineage.json \
  --output /absolute/campaign-run/release-pack-lineage.json

/absolute/candidate/source/tools/release/successor_campaign_pack.py \
  verify-lineage-binding \
  --pack /absolute/candidate/SUCCESSOR_CAMPAIGN_PACK.json \
  --binding /absolute/campaign-run/release-pack-lineage.json
```

After independently building the same frozen source on any two supported
builders, compare the two fully verified packs and preserve a re-verifiable
reproduction report:

```sh
/absolute/left/source/tools/release/successor_campaign_pack.py compare-packs \
  --left /absolute/left/SUCCESSOR_CAMPAIGN_PACK.json \
  --right /absolute/right/SUCCESSOR_CAMPAIGN_PACK.json \
  --output /absolute/new-reproduction.json

/absolute/left/source/tools/release/successor_campaign_pack.py \
  verify-reproduction \
  --report /absolute/new-reproduction.json
```

The comparison requires exact source and lock digests, pinned SBF and Node
toolchain strings,
shipped ELF and checked-manifest bytes, release/profile identities, frame and
budget metadata, compliance inputs, and their source-pinned verifiers to
match. Absolute work paths, fresh run identifiers, per-run provenance/gate
hashes, the declared builder label, and the recorded host OS/kernel/C
toolchain/libc identity are deliberately excluded because they identify the
independent executions rather than their deterministic outputs. Each pack
still records and rehashes its source-built host successor binary. The
cross-builder projection compares that producer's exact source and its
deterministic Product records, not path- and libc-bearing host executable
bytes.
The report retains and rehashes both full pack manifests; verification reruns
each pack's complete gate before recomputing the comparison.

The final devnet campaign must also traverse the public CLI wrappers, not call
their Rust child commands by hand. From the exact checked candidate, run:

```sh
/absolute/candidate/source/tools/release/public_route_campaign.py run \
  --pack /absolute/candidate/SUCCESSOR_CAMPAIGN_PACK.json \
  --rpc-url https://api.devnet.solana.com \
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
  --plan /absolute/devnet/plan.json \
  --session /absolute/devnet/direct-trade-session.json \
  --direct-journal /absolute/devnet/direct-trade-journal \
  --producer-journal /absolute/devnet/direct-trade-producer.json \
  --output-root /absolute/new-public-route-campaign

/absolute/candidate/source/tools/release/public_route_campaign.py verify \
  --evidence /absolute/new-public-route-campaign/PUBLIC_ROUTE_CAMPAIGN.json
```

This runner is read-only with respect to devnet and opens no key. It builds the
Rust producer and TypeScript CLI from the pack's exact archived source, invokes
`dclutch-terminal route release-set` and then `dclutch-terminal route direct`, preserves their
actual JSON stdout reports, and rehashes both outputs. The release-set output
must be byte-identical to the pack's checked multiprogram, and the Direct route
must name the pack's checked infrastructure digest. The plan, session, frozen
lookup journal, source inputs, built callers, command vectors, reports and
outputs remain joined by `PUBLIC_ROUTE_CAMPAIGN.json`. The finalized producer
journal additionally binds the exact founding campaign, admitted participant,
Market input, ticket pair, checked release, private session and public manifest;
a still-live same-release session from another Market therefore refuses rather
than masquerading as the intended campaign.

After those read-only public routes are bound to the finalized producer
journal, the exact archived mutating suffix resumes the remaining authorized
devnet life from one source-owned plan:

```sh
/absolute/candidate/source/tools/release/devnet_direct_lifecycle.py run \
  --plan /absolute/devnet/direct-complete-life-plan.json \
  --output-root /absolute/new-direct-complete-life

/absolute/candidate/source/tools/release/devnet_direct_lifecycle.py verify \
  --report /absolute/new-direct-complete-life/DIRECT_COMPLETE_LIFE.json
```

The suffix hard-refuses an adjacent runner copy, mainnet, a plan or public
campaign from another checked source/Market/session, non-independent actors,
or a fee-settlement payer that is also a Market participant. Its durable
journal is written before each child dispatch and authenticated on resume. The
Python runner never opens keypair files; the plan names the explicit payer,
seller and buyer paths passed to the source-pinned Rust children. This is
devnet evidence only.

Run the focused parser/mutation tests with:

```sh
python3 -m unittest tools.release.test_successor_campaign_pack
```

## Fresh-build rule

Use a new absolute `--work` root for every candidate you intend to admit. The
runner enumerates `programs/*/Cargo.toml`; that directory is the sole owner of
the frame-gated link set. Ten current packages produce release artifacts and
two are frame-gate-only, so the present summary reports TWELVE links.
**Do not restate that number anywhere.** `e6b7bf1a` deleted `dclutch-dealer-sbf`
on 2026-09-02 and took the set from thirteen to twelve; the runner derives its
count from `programs/` and stayed green, and four consumers that had written the
literal 13 went silently red. `artifact_provenance.SHIPPED_LINKS` is the one
owner; count it from there.

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
fresh measurement build for each of the same twelve links with
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

The static plan enumerates exactly the shipped link set, whose size it derives
from its own role tables. It computes each SBF
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

## A cold machine

Replayed 2026-09-03 on hbox from an empty `CARGO_HOME`/`RUSTUP_HOME` and a fresh
clone; every line below is one that campaign actually ran, and the whole setup
took 51 seconds. Nothing in this tree said any of it before that replay, so a
cold machine had to reconstruct the toolchain from `docs/board-archive-2026-08-27.md`.

```sh
# Isolate everything a cold run creates, so the host's own caches are not inputs.
export COLD=/absolute/scratch/root
export HOME="$COLD/home" CARGO_HOME="$COLD/toolchain/cargo" RUSTUP_HOME="$COLD/toolchain/rustup"

# 1. Rust, at the channel rust-toolchain.toml pins (1.97.1 as of this writing).
curl -sSfL https://sh.rustup.rs -o "$COLD/rustup-init.sh"
sh "$COLD/rustup-init.sh" -y --no-modify-path --profile minimal --default-toolchain none
export PATH="$CARGO_HOME/bin:$PATH"
rustup toolchain install 1.97.1 --profile minimal --component clippy --component rustfmt

# 2. Agave, at the EXACT tag, never the `stable` channel. This is also what
#    installs cargo-build-sbf, and cargo-build-sbf is what downloads
#    platform-tools v1.53 into $HOME/.cache/solana on its first invocation.
sh -c "$(curl -sSfL https://release.anza.xyz/v4.0.2/install)" -- v4.0.2 --no-modify-path
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

# 3. The pinned Node distribution for THIS platform. Both --node and
#    --node-archive are required on every builder, `local` included.
#      linux-x64     node-v26.4.0-linux-x64.tar.xz
#                    5c4286dcd5bbd5acb1ccc7eb0e088bd5eb1e3affad671ee9364004f8f6a4a431
#      darwin-arm64  node-v26.4.0-darwin-arm64.tar.xz
#                    bef4c7e75087c029835f519a7ba640eba52fa617fadb3a9049828ff3b45b57dd
curl -sSfL -O "https://nodejs.org/dist/v26.4.0/node-v26.4.0-$PLATFORM.tar.xz"

# 4. WARM THE CARGO CACHE FOR THE SUCCESSOR WORKSPACE. See below.
cargo fetch --locked --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml
```

**Step 4 is not optional and its absence is invisible.** The runner builds the
successor host producer with `cargo build --release --locked --offline`, from a
SEPARATE workspace whose lock resolves crates the SBF builds do not necessarily
fetch. On a cold cache that build fails, and the failure is silent: the runner
prints nothing after `SBF build freshness PASS`, exits **101**, and the only
copy of the reason lives in `<work>/product-handoff/build.log`
(`error: failed to download serde v1.0.228 ... --offline was specified`). Four
crates and 1.1 seconds of `cargo fetch` is the whole fix, and 427 seconds of
build is what skipping it costs. **When a candidate exits 101 with no message,
read `product-handoff/build.log` first.**

A cold machine can only build a **GENESIS** candidate, because a succession is
not a function of the successor alone and its `--predecessor-profile` has to be
read off the chain being succeeded. The genesis invocation, which nothing else
in this file showed:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
  "$COLD/src/tools/release/checked-release-candidate.sh" \
    --repo "$COLD/src" \
    --work "$COLD/work/candidate-1" \
    --commit <exact commit> \
    --genesis-cohort \
    --node "$COLD/toolchain/node-v26.4.0-linux-x64/bin/node" \
    --node-archive "$COLD/toolchain/node-v26.4.0-linux-x64.tar.xz" \
    --builder hbox
```

Measured cost of one clean genesis candidate: **447-455 s on hbox** (24 cores,
co-tenant, `CARGO_BUILD_JOBS=4`), **526-619 s on the laptop** (macOS arm64).

## One builder artifact

**The release is the bytes ONE builder artifact produces**, and
`supported_builders` names the hosts that run *that artifact*. It is not a set
of hosts asserted to agree; the previous list -- `local`, `persvati`,
`hbox-through-swarm-build` -- was policy that had never been measured, and when
it was measured it was false.

The named artifact is **platform-tools v1.53 on Linux/x86_64**, with the host
Rust channel `rust-toolchain.toml` pins. `checked-release-candidate.sh` refuses
on any other host in its first second, and every pack built from a candidate
that says otherwise refuses.

### The measurement

Measured 2026-09-03 (hbox against the laptop, ten roles) and 2026-09-04
(the causes, on `registry.so`, and persvati against hbox, ten roles), all at
commit `fe70f076`, `cargo-build-sbf 4.0.0` / `platform-tools v1.53` /
`rustc 1.89.0` on every host:

| pair | result |
| --- | --- |
| one host, two absolute `--work` roots | **all ten identical** -- the build path is not an input |
| hbox (Linux x86-64) vs laptop (macOS arm64) | **nine of ten differ**; only `series-shadow` reproduces |
| **persvati (Linux x86-64) vs hbox (Linux x86-64)** | **all ten identical** |
| **the laptop in a `linux/amd64` container vs hbox** | **all ten identical** |

The third row is the one C-14 needed and never had, and it is what makes the
first two mean something: two independent machines, different kernels,
different `$HOME`, different absolute build paths, a fresh per-package target
directory on one side and the release runner's on the other, and the ten
shipped ELFs agree byte for byte. The whole pack projection agrees too --
`toolchains`, `artifacts`, `release` and `ceilings`, `projection_sha256
0e50ca5658ec2d07...` on both hosts -- so `execution_release_set_id`,
`checked_infrastructure_id` and `infrastructure_profile_sha256`, the three §3
reported as diverging, are equal.

The fourth row is the same laptop that produces row two's right-hand side. Run
the named artifact instead of its own and it produces row two's left-hand side
instead, all ten.

### Why a macOS build can never join, and why it is two reasons

**Cause 1 -- the prebuilt standard library carries Anza's CI checkout path.**
`platform-tools` ships `core` and `alloc` already compiled, and their
`core::panic::Location` file strings are the paths of the machine Anza built
them on. In `registry.so` there are exactly two, and they are the whole of
`.rodata`'s divergence:

```
hbox    /home/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/slice.rs        (83 bytes)
laptop  /Users/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/slice.rs       (84 bytes)
hbox    /home/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/raw_vec/mod.rs  (89 bytes)
laptop  /Users/runner/work/platform-tools/platform-tools/out/rust/library/alloc/src/raw_vec/mod.rs (90 bytes)
```

`/Users/` is one byte longer than `/home/`, `.rodata` rounds up by eight, and
every address behind it moves -- which is why the difference reads as codegen
and is not. Three such strings in claims, core, custody, dealer-accelerator,
resolution and trading; two in registry and rent; one in general-accelerator;
**zero in series-shadow**, which is exactly why series-shadow was the one role
that reproduced. `--remap-path-prefix` cannot reach any of them: they are data
in a prebuilt `.rlib`, not a path the local compiler is given.

**Cause 2 -- cargo's per-unit metadata hash carries the builder's host triple,
and nothing about the ELF is downstream of a string.** Installing the linux-x64
`sbpf-solana-solana` sysroot into the macOS platform-tools closes cause 1
exactly: `.rodata`, `.dynamic`, `.dynsym`, `.dynstr`, `.shstrtab` and the file
length all become identical to hbox's. **654 bytes of `.text` still differ.**
Cargo computes each unit's `-C metadata` from its dependency units' hashes, and
build-script and proc-macro units are HOST units, so every crate that
transitively uses one inherits the host triple: at one commit, one source tree,
one sysroot and one rustc version, **61 of 76 units took a different
`-C metadata`** on macOS than on Linux. And the shipped ELF is a function of
that hash -- changing only `__CARGO_DEFAULT_LIB_METADATA`, with no source, flag
or toolchain change at all, moves the bytes.

So the three remedies that suggest themselves all fail, and they fail for
reasons worth writing down once:

- **Post-link rewrite of the panic strings.** The strings differ in *length*,
  so canonicalizing them shifts `.rodata` and every address behind it: that is
  a relink, not a rewrite. And it cannot touch cause 2 at all.
- **`-Z build-std` with `--remap-path-prefix`, or `panic_immediate_abort`.**
  Both rebuild the standard library, so both move the frame manifest, and
  neither touches cause 2.
- **Pinning the platform-tools tarball.** Necessary, not sufficient -- it is
  exactly the experiment above, and it leaves 654 bytes.

Only pinning the *host triple* reaches byte identity, which is why the policy
is one named artifact rather than a set of agreeing hosts.

### How a host joins the supported set

A host is a supported builder when it **runs the named artifact**:

- `hbox-through-swarm-build` and `persvati` run it natively.
- `linux-x86_64-container` is any other machine running it in a linux/amd64
  container -- the laptop's route. Build with `--builder container` inside the
  container; the cold-toolchain recipe under "A cold machine" is the whole
  setup.

A native macOS build is a **diagnostic** build: real sources, real compilation,
useful for finding a defect, and not a release. `--diagnostic-builder` is how
to make one deliberately; it stamps `release_builder=false` into the summary
and every pack built from that candidate refuses.

## hbox

hbox is shared build infrastructure. Wrap the entire runner once so one cgroup
contains the host-tool and all sequential SBF child builds:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=4 swarm-build \
  /tank/dregg-build/dclutch-source-<commit>/tools/release/checked-release-candidate.sh \
    --repo /tank/dregg-build/dclutch-source-<commit> \
    --work /tank/dregg-build/dclutch-checked-<commit>-<unique-run> \
    --commit <commit> \
    --predecessor-profile /tank/dregg-build/predecessor-profile.bin \
    --node /tank/dregg-build/node-v26.4.0-linux-x64/bin/node \
    --node-archive /tank/dregg-build/node-v26.4.0-linux-x64.tar.xz \
    --builder hbox
```

The runner deliberately does not discover or invoke `swarm-build` itself. That
keeps the scheduling boundary visible and prevents recursive wrapping. Refuse a
run on hbox if the outer wrapper is absent; do not silently fall back. The
resulting summary and campaign pack record `builder=hbox` and
`builder_scheduler=swarm-build`. Use `--builder persvati` on persvati; the
default `local` preserves ordinary local invocations. The predecessor profile
is the exact public 144-byte profile-account value being succeeded, not a key
or secret. The runner admits only a canonical regular input, copies it into
`infrastructure/predecessor-profile.bin`, and derives only from that preserved
copy. Changing it changes the successor profile and therefore cannot count as
a reproduction.

For the hbox/persvati release pair, both Node inputs must come from
`https://nodejs.org/dist/v26.4.0/node-v26.4.0-linux-x64.tar.xz`, whose pinned
SHA-256 is
`5c4286dcd5bbd5acb1ccc7eb0e088bd5eb1e3affad671ee9364004f8f6a4a431`.
The runner requires absolute `--node` and `--node-archive` paths, refuses the
hbox system Node and persvati Bun shim, and uses a source-pinned bounded member
lister to prove that the archive contains exactly one regular Node and sibling
npm at their canonical paths. It copies the archive into `toolchain/`, records
the executing host Rust/C/linker/libc substrate, builds the SDK, CLI and Rust
producer from the archived source locks, and executes
`spline-product-handoff-smoke.sh`. The pack verifier independently reopens the
preserved archive and rehashes the Product build and five output records. The
supported-builder comparison includes the Node/npm/archive identities, public
CLI, canonical compiler report, semantic basis, Found coordinates and record
bytes. The host successor binary remains exact evidence inside each pack, but
is excluded from the cross-builder projection because absolute build paths and
the Linux distribution linker/libc are embedded in those helper bytes.

The admitted summary must say `sbf_build_freshness=passed`,
`sbf_build_freshness_links=12`, `sbf_build_diagnostics_total=0`, and
`sbf_build_diagnostics_accepted=false`. It must also say
`cargo_lock_immutability=passed`. Preserve `build-links.tsv`,
`build-run.txt`, `source-tree.txt`, every `build-*.log`, every
`frame-build-*.log`, the `frame/` reports, `build-diagnostics.txt`, and both
Cargo lock manifests with the candidate evidence. Preserve every `provenance/`
descriptor and its referenced frame object as well.
Preserve `RELEASE_GATE.json`, `gate.sha256` and `run/RUN_RECORD.json` too --
and note the asymmetry those three make explicit: the first two can be
regenerated by rebuilding the commit, and nothing else on this list can.
The same summary must say `node_version=v26.4.0` and
`spline_product_handoff=passed`; preserve `toolchain/` and `product-handoff/`
with the rest of the pack.

## Upgrade gate

A clean run also emits `CHECKED_UPGRADE_GATE.json` and prints its SHA-256. The
gate is generated only after the fresh all-link build, static frame gate, and
checked release manifests complete. It binds the source commit/tree, exact
shipped-link identities, run stamps and compile markers, build and frame logs,
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

It refuses a noncanonical gate path, anything other than the exact shipped
set, a wrong role/package map, a stale source/run/log/object/report/ELF, and an
adjacent or renamed file. Its JSON result names the only ELF path that a
consumer may open.

## The reproducible gate, and why the Upgrade gate is not one

`CHECKED_UPGRADE_GATE.json` binds one run's whole evidence envelope, and that
envelope does not reproduce. Measured over two full candidate runs at one
commit on one host: **the shipped ELFs and the checked release manifests come
back byte-identical, and so do `source-tree.txt`, `build-links.tsv` and
`build-diagnostics.txt`. Every `provenance/*.json`, every `build-*.log`, every
`frame-build-*.log`, every `frame/*.txt` and `build-run.txt` do not** — the
last of those is a per-run nonce whose whole content is
`dclutch-sbf-build-run-v1=<random>`, and it alone moves the Upgrade gate's
digest.

The consequence is not academic. Cohort-15 lost its candidate directory and
could no longer satisfy the produce command's provenance check *even though a
rebuild reproduced every byte it had deployed*, because the admission it had
recorded named a nonce. A cohort in that state keeps the ability to settle its
market and loses the ability to trade it.

So a clean run now emits three more files beside the Upgrade gate:

| file | what it is |
| --- | --- |
| `RELEASE_GATE.json` | the **reproducible gate** — the admission authority |
| `gate.sha256` | its digest in `shasum -a 256 -c` form |
| `run/RUN_RECORD.json` | that run's per-run evidence index |

`RELEASE_GATE.json` carries the source revision and tree digest, the Solana CLI
version, the three reproducible manifests, and one row per shipped link holding
its diagnostics count, its frame measurements **as numbers**, its ELF and its
checked manifest. It names no log, no report file, no provenance descriptor and
no build run id. Two candidate runs of one commit in two different work roots
produce it byte-for-byte, so `shasum -a 256 -c gate.sha256` passes in either
root and a cohort that records that digest can re-admit from a fresh build.

`run/RUN_RECORD.json` carries exactly what was removed — the build run id,
`build-run.txt`, and per link the build log, frame build log, frame report,
compile markers and provenance descriptor — plus the digests of both gates it
accompanies. It is kept for the reader and is deliberately outside the gate
digest: a gate from one run plus provenance from another is inconsistent
evidence even when every check passes, and the run record is the thing that
says which run an envelope belongs to.

The frame *numbers* stay in the gate while the frame *report file* does not,
because the numbers are measurements of a reproducible object and the file
stamps the run that measured it. That is the whole partition rule: a value that
the source determines is gate content; a value that the run determines is run
content.

The two bounded selectors:

```sh
tools/release/artifact_provenance.py verify-reproducible-gate \
  --root /absolute/candidate \
  --gate-sha256 <separately-recorded-reproducible-gate-sha256>

tools/release/artifact_provenance.py select-reproducible-role \
  --root /absolute/candidate \
  --gate-sha256 <same-digest> \
  --role trading
```

Both rehash every byte the gate names and nothing else, so they succeed in a
work root that holds only `RELEASE_GATE.json`, the three manifests, `elf/` and
`evidence/*/checked.bin`. They refuse a link set that is not the exact shipped
order, a nonzero diagnostics count, a frame report that admitted a frame at or
over the 4096-byte bound, an ELF or checked manifest at a noncanonical path, a
symlink, and any named file whose bytes or SHA-256 moved.

**A candidate built from a different commit refuses on the gate digest**, which
is the point: the gate stopped being sensitive to which run built the bytes and
did not stop being sensitive to which bytes were built.

`checked-release-candidate.sh` prints all three digests, and the summary keeps
recording `checked_upgrade_gate_sha256` beside them.

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

## The cohort cut window

The cut is manual. Nothing here updates itself, and the window is the only
time the whole-tree generators may run: `tools/genref/generate.sh` swept
eighteen lanes' refusal codes into one reference when it was run casually, so
the standing rule is that a lane runs it only inside an announced window on a
quiet tree. This section is that window's checklist, written down because the
lane that executes it will not be the lane that discovered the items.

**One command does the regeneration half**, and it is the one above:
`final-generated-convergence.py --write --expected-head <sha>` runs
`tools/genref/generate.sh` inside a fixed batch that refuses a dirty or moving
tree and confines its writes to named owners. `--check` at the same head is the
verifier. Do not hand-run the individual generators.

### Regenerate (quiet tree, announced first)

`tools/genref/generate.sh --check` reports the whole backlog in one read; as of
2026-08-30 it is **seven stale files, and they are one set, not three items**:

```
docs/reference/README.md          docs/reference/refusals.md
docs/reference/decisions.md       docs/reference/routes.md
docs/reference/programs.md        docs/reference/abi/refusalRegistryV1.md
                                  docs/reference/abi/routeCensus.md
```

- `decisions.md` stops at 0013, so **all four of 2026-08-30's ADRs are missing
  from the index that ships to the public site**. It is stale for the same
  reason the other six are, and one regeneration closes all seven.
- **The regeneration alone re-stamps a known overclaim.** `docs/reference/README.md`
  is generated from `tools/genref/generate.mjs:406`, which still says *"there is
  no open market or value at risk today"*. Regenerating without correcting the
  generator first reproduces the sentence and makes it look handled. Correct the
  source in the same window — see below.

### The open-market posture, and the two assertions that pin it

The site is written to a posture that was true when it was written. Correcting
it is not a prose edit: `tools/genref/render-site.mjs` **asserts the untrue
strings are present**, so changing the prose without the assertions fails the
render, and changing the assertions without the prose fails it too. They move
together or not at all.

| # | site | what it says |
|---|---|---|
| 1 | `tools/genref/generate.mjs:406` | → generates `docs/reference/README.md:35` |
| 2 | `tools/genref/render-site.mjs:453` | the footer, on **every** page |
| 3 | `tools/genref/render-site.mjs:470` | the documentation landing |
| 4 | `tools/genref/render-site.mjs:690` | **assertion**: `guides/README.html` must contain "There is no open market" |
| 5 | `tools/genref/render-site.mjs:691` | **assertion**: `guides/trader.html` must contain "no devnet market is open for trading" |

Assertion 5 is a *second* pinned string in a *different* file, and a sweep that
counts "one link-check assertion" will miss it. Each assertion has a
hand-written target that must change in the same commit, and there are more
targets than assertions:

- `docs/guides/README.md:23` (target of 4), `docs/guides/trader.md:7` (target of 5),
- `docs/guides/reader.md:23`, and `docs/guides/trencher.md:11,14,15,93` — four
  more instances that no assertion pins and no sweep has listed.

Today the claim is false in letter and true in spirit: three markets are
`Phase::Open` and none of them is tradeable (decision 0015 §8.1). **When
market19 opens with an activated capability root it becomes flatly false**, and
that is the moment all of the above must land together.

> **EXECUTED 2026-08-30 (PUBLISH-2), at market19's opening.** All five sites
> above moved in one commit, with the six hand-written instances. The posture
> the site is now written to: *seven programs on devnet, one market open for
> trading, its collateral a devnet test token, nothing bought with money, no
> trade made yet.* Two things changed about the pins themselves, and the next
> lane to read this table should know both. **The needles were reworded so the
> FOOTER cannot satisfy them** — the old needle for site 4 ("There is no open
> market") appeared verbatim in the footer that renders on every page, so
> `guides/README.html` would have passed that assertion with its body deleted;
> the new needles are body-only, and each was proved red by deleting the
> sentence before the commit. And **`docs/reference/README.md` is not edited by
> hand** — site 1 is the generator, and the regeneration in the same window is
> what moves the generated file. The current needles are in
> `render-site.mjs`; read them there rather than from this table, which
> records the window, not the live state.

### The public-cut fixture

`apps/dclutch-web/fixtures/public-cut.devnet.json` headlines
`7Mcu1ZT9…` with `trade`/`resolve`/`redeem` all `null`. At the cut it is
replaced by market19, and the five sites above are checked against it.

**Read decision 0015 §8 before deciding to wait.** That market is not merely
"not traded yet": it can never trade *and* can never be retired, because its
whole claim supply sits in a Position whose owner key no longer exists. The
fixture is the site's front door, and it currently points at the deadest object
on the cluster. If market19 slips, re-pointing it at market18
(`9JwhTHyx…` — `Phase::Open`, `outstanding_capabilities = 1`, holding the only
live capability root on any dClutch deployment) is available immediately and is
not gated on the protocol cut.

### Two different cuts

They are routinely conflated and they have different blockers:

- the **cohort cut** deploys ELFs and founds market19 (blocked on the release
  verdict and the orchestrator's go);
- the **publication cut** is a single-parent content-sync of the
  `dragons-clutch` `dclutch/` subtree plus a `pages.yml` dispatch, and it is
  what actually moves the live site.

A fixture or prose correction reaches readers at a *publication* cut. Anything
in this section that does not need market19 to exist should not be queued
behind the cohort cut.

### Whichever cut comes first

The shared-window rule above is not "at the cohort cut". A generated file and
the source it is generated from must move together at **whichever cut reaches
readers first** — publication or cohort. `docs/reference/` is regenerated
output; correcting `tools/genref/generate.mjs` in one window and regenerating in
another ships a reference that disagrees with its own generator in between.

### What the cohort upgrade does to the markets already on devnet

State it plainly in the cut announcement; ember's standing Q1 ruling accepts
devnet stranding, so this is disclosure, not a blocker. Verified rather than
inferred:

- **Every existing devnet market becomes permanently inert.** All three are 360
  bytes on chain; the cohort's `CoreState::decode` refuses
  `input.len() != STATE_BYTES` at 368. **There is no migration path and none can
  be added after the fact**: the only `resize` calls in Core are `resize(0)`,
  which is account *closure* (`retire_v1.rs`, `generic_founding_v1.rs`). A
  360-byte market can never become a 368-byte one, so after the upgrade nothing
  can trade, resolve, retire or close any of them.
- **Market18 is included, and it is the expensive one.** `9JwhTHyx…` holds the
  only live capability root on any dClutch deployment — the account the whole
  first-trade effort produced. Note for honesty about cause: its collateral was
  *already* unrecoverable before the cut, for the unrelated reason in decision
  0015 §8. The upgrade does not create that stranding; it adds a second,
  independent one, so no single fix restores it.
- **The clients are already there.** `CORE_STATE_BYTES = 368` is generated into
  both `packages/dclutch-sdk/lib/generated/coreFound.ts` and the web twin
  *today*. The reader half of this is not a future consequence of the cut — the
  current tree already cannot decode any live devnet market.
- **The site degrades honestly rather than breaking**, which is CORESTATE's
  work: `marketDiscovery.ts` classifies a market by **(magic, version, width)**
  and files `(3, 360)` as a historical generation with a `refused` provenance,
  instead of throwing. Worth confirming on the staged fixture before announcing.
- **The honest-degradation path is implemented but never exercised for the
  generation that matters.** `marketDiscovery.test.ts` (SDK and web twins) has
  exactly one historical-account case and it is `DCLTCOR2 at 352 bytes`. The
  `(3, 360)` generation — which describes **every live devnet market**, and which
  the cut makes the universal case — has no test. The code path exists; nothing
  proves it fires for the accounts it was written for. Add the 360-byte case
  before announcing, or the "the site degrades honestly" line in the
  announcement is an unverified claim.
- **One stale doc contradicts that**, and it will mislead the next reader:
  `marketCoreV2.ts` still documents `DCLTCOR3` as "360 fixed bytes" (SDK and web
  twins both), one file away from the discovery reader that lists `(3, 360)` as
  historical. Not this lane's file; route it with the cut notes.

### The funding-readiness suffix, and why the devnet founding does not stall

The post-Open funding-readiness suffix refuses offline in its builders. The
founding pipeline **does** share them — `execute_generic_market_founding` calls
`execute_funding_readiness_suffix_v1` immediately after
`authenticate_open_market_poststate_v1`, inside the founding, not after it — so
coordinate with that lane before a devnet sequence rather than assuming
independence.

But the devnet path takes an early return. The suffix's first act is to
classify against chain state, and the atomic DCLTGMF3 founding
(Lock+Found+Realize+Claims+Open in one transaction) consumes the staged
readiness, so the plan comes back `FundingReadinessPlanV1::ConsumedByFounding`
and the function returns `Ok(())` before touching a builder. The refusing arms
are `Create`/`Activate`/`Accept`/`Complete`, which belong to the non-atomic
walk.

**The evidence is three for three**: every market this pipeline has founded on
devnet reads `Readiness::Consumed`, which is exactly the state that selects the
early return. **What that does not prove**: the arm is selected by a chain read,
so it is only as good as the founding staying atomic, and the FOUND-5182 green
proof is a *local* multi-step founding, not the devnet atomic one. If market19's
founding stops being atomic — a frame that no longer fits, say; Found31 already
needed an address lookup table to clear the legacy packet limit by ten bytes —
the refusing arms become live and the stall is real.

### Executing the upgrade: three things the plan does not say

Learned on the 2026-08-30 attempt, which held at role 1 of 5 with devnet
semantically unchanged (`/Users/ember/jobs/dclutch-cohort7-20260830/HELD_STATE.md`).

**Core is last, and that is a safety property.** The role order is
custody → resolution → claims → trading → **core**. The CoreState widening that
strands every existing devnet market lands at the very end, so a partial cut is
a holdable state and everything before core is reversible in effect.

**`devnet-upgrade-extend-v1` invalidates its own role's baseline.** Its docs say
it outright — *"After it completes, capture a new baseline: the Upgrade refuses
the old one"* — so claims, trading and core each need extend → **fresh
baseline** → journal row update → upgrade. Skipping the recapture refuses on a
stale baseline. Extension rent for the three is **0.780856 SOL, non-refundable**,
measured from the baselines rather than estimated.

**The one-attempt Buffer writer can refuse on liveness, and it is not a
semantic failure.** The exact code is:

```
Error("one-attempt Buffer writer returned before exact payload finalized;
recovery is poll-only until expiry")
```

The receipt's `buffer_write_attempts[0].exit_disposition` was `returned_success`
— the upload *finished* and the buffer held the exact payload — but the tool's
finalization confirmation did not land inside a 512-block window. **Recovery:
wait for the window to expire, then re-run the identical command with
`--adopt-existing-buffer`.** Cost of the miss is one parked buffer (rent
recoverable) plus ~0.0028 SOL of upload fees.

Worth sizing before a long run: custody is the **smallest** payload at 563 KB
and it is what hit this. Trading is 2,030,592 bytes and claims 1,263,616.

**Run the execute on the KEYED endpoint, not the public one.** This is the
mistake that stalled the 2026-08-30 attempt and it is one flag. The read-only
captures genuinely demand the canonical public origin — `devnet-carry-forward-capture-v1`
refuses anything else outright — and it is easy to carry that over to
`devnet-upgrade-v1 --execute`, where it is wrong. Resuming a role
re-authenticates **every** buffer-upload transaction with `getTransaction`, which
for a 563 KB payload is hundreds of calls, and `api.devnet.solana.com`
rate-limits it: `getTransaction returned HTTP 429 Too Many Requests`.

That is a trap and not merely a slowdown, because `validate_receipt_binding` is
called with `require_exact_rpc_origin = true` at both execute call sites. **A
receipt written against the public endpoint can only ever be resumed against the
public endpoint — the one that cannot serve its own resume.** The house already
knew: *"execution campaigns run on a keyed endpoint"* (2026-08-29).

**Use the CLI for `program extend`, not the driver.** `devnet-upgrade-extend-v1`
builds the **checked** loader instruction (`upgrade.rs` imports
`extend_program_checked`), and devnet's Loader-v3 rejects it outright:

```
InstructionError[0, InvalidInstructionData]
Program BPFLoaderUpgradeab1e11111111111111111111111 failed: invalid instruction data
```

It fails in simulation, so it costs nothing and changes nothing. The documented
house path is the CLI, and it is proven on this cluster: 2026-08-29 extended
resolution, trading and core with `solana program extend` at the same pinned CLI
version. Capture a **fresh baseline** afterwards either way.

**And an armed receipt cannot be rescued by adopt.** `validate_receipt_binding`
compares `receipt.buffer_adopted != args.adopt_existing_buffer`, so adding
`--adopt-existing-buffer` to an existing non-adopt receipt refuses the binding,
while keeping `--buffer-keypair` refuses because *"adopt refuses a Buffer keypair;
only the retained Buffer authority is used"*. `--adopt-existing-buffer` is for a
buffer some **other** run uploaded — pair it with a **fresh receipt path**.

### Cut artifacts do not live in `/private/tmp`

Three separate artifacts of the 2026-08-29 cohort were lost to scratch reaping,
and each cost something different:

1. the **founder keypair** — decision 0015 §8; three markets can never be
   redeemed or retired, ~0.26 SOL and 1.5 billion collateral atoms stranded;
2. the `founder-ids/` directory that held it;
3. the **`SuccessorPlan`** the administration campaign runs on, recorded in
   `campaign-open.json` at `…/scratchpad/spine-market/plan.json`.

The third is recoverable — `prepare` regenerates a plan from the seven roles'
ids, ELFs, digests and semantic release ids — but it is exacting work that has to
be redone from scratch, and a wrong semantic release id refuses at activation
*after* publication rent is spent.

**So: keys, release ELFs, the checked gate, baselines, receipts, dumps, the
deployment-set journal and the plan all belong in a durable job directory**
(`/Users/ember/jobs/<job>/`), not a session scratchpad. The 2026-08-30 cohort
keeps them at `dclutch-cohort7-20260830/` with a README that says, in as many
words, *this is not a scratch directory*.
