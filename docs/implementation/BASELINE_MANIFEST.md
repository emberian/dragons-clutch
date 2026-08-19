# Baseline evidence manifest

`scripts/baseline_manifest.py` derives `MANIFEST.baseline.json`, a single
machine-readable record of what the reviewed offline baseline *is* and what its
named checks *did*. Its purpose is narrow and worth stating twice: it turns
"the baseline is intact" into a claim a machine can re-derive and contradict.

It is **not** a release manifest. It publishes nothing, signs nothing, tags
nothing, and attests no proof content. `CODEX_HANDOFF.md` §7 P0-1 remains open
after this file lands; this is groundwork under that blocker, not its closure.

Status: **IMPLEMENTED** (source exists locally, the named offline checks record
their own outcomes). This includes the current low-cost host-model, research,
and Glass gates. It does not promote any of those results into runtime,
deployment, or proof evidence. Everything the manifest describes about *proofs*
remains **BLOCKER** per §7 P0-2, and everything it describes about *deployment*
remains **BLOCKER** per §7 P0-6.

## Usage

```sh
# Derive digests, toolchain pins, and gate declarations only (fast).
scripts/baseline_manifest.py emit

# The full record: also execute every declared gate and store exit codes plus
# normalized key output lines. This is intentionally not a presubmit: a
# cache-cold run can take tens of minutes because it includes the low-cost
# research/model/frontend inventory, two fresh SBF ELF builds, the loopback
# lifecycle, and the isolated Token-2022 bank suite.
scripts/baseline_manifest.py emit --run-gates

# A labelled mid-flight snapshot from a dirty tree. Never a baseline.
scripts/baseline_manifest.py emit --allow-dirty --run-gates

# Re-derive everything and report drift against the stored manifest.
scripts/baseline_manifest.py check
scripts/baseline_manifest.py check --run-gates
```

Defaults: `--out`/`--manifest` is `MANIFEST.baseline.json` at the repository
root; `--gate-timeout` is 1800 seconds per gate. Standard library only, no
network, no writes outside the manifest path (gates themselves write to their
own `target/` directories and to `mktemp` scratch, exactly as they do when run
by hand).

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | success |
| 1 | `check` found drift, or `emit --run-gates` saw a gate contradict its declaration |
| 2 | refusal: dirty working tree under `--strict` (the default) |
| 3 | environment or usage error |

## The refusal rule

`emit` is `--strict` by default and **refuses** when the NUL-delimited
`git status --porcelain=v1` contains anything except the manifest output itself.
It prints every dirty path and exits 2 without writing anything. Rename and copy
records are parsed as two-path records; an operation involving the manifest and
another path is not hidden by the manifest exemption.

`--allow-dirty` is the escape hatch for mid-flight snapshots. It emits, but the
manifest then carries:

- `"dirty": true`;
- `dirty_warning`, prose stating that `content_identity` covers tracked
  working-tree bytes but the historical provenance commit/tree do **not**
  describe the tree the gates ran against and untracked bytes are not bound;
- `dirty_porcelain`, the verbatim porcelain listing;
- `claims.manifest_label: "PROPOSED"` instead of `"IMPLEMENTED"`.

A dirty manifest may be used to see the machinery work or to capture a wave in
progress. It may never be cited as a baseline. `check` re-states this in its
success output so that a passing check on a dirty snapshot cannot be quoted as a
clean-tree result.

Exactly one path is excluded from both the dirtiness decision and the content
identity: the manifest's own output path. It is this tool's output, not an input,
and its bytes cannot truthfully attest themselves. The exclusion is recorded in
both `dirty_check_excludes` and `baseline.content_identity`, including whether
the excluded path was tracked/modified. Nothing else is excluded.

One consequence deserves saying plainly: gates always execute against the
**working tree**, never against the recorded commit. On a clean tree those are
the same thing, which is the entire reason `--strict` is the default. On a dirty
tree they are not, and a gate that fails there may be reporting another lane's
in-progress edit rather than a defect in the baseline — a `Cargo.toml` whose
matching `Cargo.lock` has not been regenerated will fail every `--locked` gate
for that crate, for instance. Read a dirty snapshot's failures as observations
about *now*, and re-emit on a clean tree before drawing conclusions.

## Schema `dragons-clutch/baseline-manifest/v2`

| Key | Contents |
| --- | --- |
| `schema` | the literal `dragons-clutch/baseline-manifest/v2` |
| `generator` | path and sha256 of the script that produced the manifest |
| `dirty`, `dirty_warning`, `dirty_porcelain` | worktree state; the latter two only when dirty |
| `dirty_check_excludes` | the one path excluded from the dirtiness decision, stated explicitly |
| `baseline.content_identity` | canonical SHA-256 over every tracked working-tree entry except the manifest itself |
| `baseline.provenance` | historical emission commit/tree/subject plus remotes and tags; informative, not the drift identity |
| `baseline.self_reference_policy` | the explicit reason the generated artifact is its only exclusion |
| `claims` | `verified`/`deployed`/`release` (all `false`), the §1 label vocabulary, and `not_attested` |
| `gates` | every declared check: id, handoff section, verbatim command, expected disposition, key-line patterns, note |
| `gate_runs` | with `--run-gates`: per gate exit code, expectation match, normalized key lines, key-line digest, and any `volatile_lines` |
| `gate_summary` | totals and the list of gates that contradicted their declaration |
| `digests` | the artifact ledger: per-entry path, kind, sha256, and the handoff-declared value where one exists |
| `toolchain` | parsed `versions.env` pins, digests of both pin records, their cross-agreement, and the explicit unpinned list |
| `unavailable_or_failing_gates` | the honest list, declared and observed |
| `handoff_digest_disagreements` | ids where the tree disagrees with `CODEX_HANDOFF.md` §6 |
| `run` | wall-clock timestamps; the **only** nondeterministic fields, ignored by `check` |

### `baseline`: content identity, not a self-defeating HEAD equality

Schema v1 required the manifest's recorded `commit` and `tree_hash` to equal
current `HEAD`. That condition could not survive the commit that checked in the
manifest: adding the manifest changes both object ids. The first clean check
after the required commit was therefore guaranteed to report drift. That was a
self-reference bug, not evidence of source drift.

Schema v2 binds `baseline.content_identity.sha256` instead. Its input is every
stage-0 path returned by `git ls-files --stage -z`, sorted by raw path bytes,
except the manifest output. Each length-delimited record binds:

- the raw path bytes;
- the observed Git mode/type (`100644`, `100755`, `120000`, or gitlink);
- a kind tag and payload length; and
- SHA-256 of the working-tree bytes, symlink-target bytes, or gitlink object id.

The schema tag prefixes the complete stream. Consequently, committing only the
generated manifest leaves the identity unchanged, while changing, adding,
deleting, renaming, or chmodding any other tracked entry changes it. Strict
cleanliness rejects untracked paths and index/worktree drift, so a clean baseline
also binds the complete repository input to every gate. The generator has its
own digest as a redundant, legible check.

`baseline.provenance.emitted_from_commit` and `emitted_from_tree_hash` preserve
the historical context in which the record was produced, but `check` does not
require them to equal the later manifest-only commit. Remotes and tags are also
historical context, not signed provenance. A configured remote means only that a
destination is configured; no push, tag, signature, or release is attested.

### `gates`

Commands are the verbatim `CODEX_HANDOFF.md` §5 forms, with the per-manifest
loop expanded so each crate's `cargo test`, `cargo clippy`, and `cargo doc`
carries its own exit code. Each gate is run through `/bin/sh -c` from the
repository root with `CARGO_NET_OFFLINE=true`, `NO_COLOR=1`, and `LC_ALL=C`.

`section` records where a gate comes from:

- `5` — verbatim from `CODEX_HANDOFF.md` §5;
- `5-extended` — a check the repository pins elsewhere that §5 omits. Currently
  one: the coupled golden trace, pinned by
  `docs/implementation/VERTICAL_MODEL.md`;
- `post-5` — benchmark harness and ABI checks added after the handoff list;
- `post-5-research` — documented, low-cost local research/model/frontend checks
  added after the handoff list. Their notes retain each surface's model or
  host-only boundary; they are not substitute SBF or proof gates;
- `post-5-runtime` — the full loopback SBF differential/lifecycle gate and the
  isolated Agave/Token-2022 program-test gate;
- `5-expected-unavailable` — the two proof gates §5 lists as expected failures.

`expected.mode` is the reviewed disposition, not a wish:

- `zero` — the gate must exit 0;
- `nonzero` — the gate is **expected to fail**, with the reason recorded inline.
  `toolchain/scripts/run_verus.sh` is the only one: the pinned Verus release
  rejects the pinned probe source (`verus_builtin` crate not imported). Making it
  pass requires editing the probe, which changes the pinned source digest, so a
  recorded failure is the correct state and a green gate here would be the
  defect. The separately isolated Verus batch shadow is still in flight and is
  deliberately neither a declared gate nor proof evidence until its source and
  reproduction command are committed;
- `either` — `rocq/check.sh`, accepting exit 0 (the `.v` file elaborates) or
  exit 2 (no `rocq`/`coqc` on `PATH`). Both carry `proof_content: "none"`:
  `ClutchKernel.v` contains zero theorems, only `Definition ... : Prop`
  obligations, one of which has a machine-checked vacuous conjunct. A `PASS`
  here means "the definitions typecheck" and nothing more.

`key_patterns` are the regexes whose matching output lines are stored. Lines are
normalized to strip elapsed times (`; finished in 0.03s`, `Ran 45 tests in
0.12s`) so that a re-run on the same tree produces the same `key_lines_sha256`.
For lint gates the clean state is *no* matched lines. Raw output is never
stored: it contains absolute temporary paths and timings, and storing it would
make the manifest neither deterministic nor reviewable.

A gate may additionally declare `volatile_patterns`: output worth recording as
evidence that is genuinely not stable across runs. Those lines land in
`volatile_lines`, stay out of `key_lines_sha256`, and are never compared by
`check`, with the reason stored inline. There is currently one, and it is worth
knowing about: `run_lab.sh` prints `host_rlib_sha256`, which changes every run
because the host probe is built into a fresh `mktemp` target directory whose
path is embedded in the artifact. `run_lab.sh` never rebuilds the host side, so
it measures no host reproducibility and `TOOLCHAIN_SPIKE.md` claims none. Only
`sbf_rlib_sha256` is measured reproducible, and only by a single same-machine
rebuild.

The two runtime gates have intentionally different coverage:

- `sbf.runtime_bringup` builds the deployable ELF twice into fresh targets,
  requires byte identity, launches a new loopback `solana-test-validator`, waits
  for a transaction-level program readiness probe, runs every implemented
  instruction-family differential and refusal, proves the differential can be
  made red, then executes the ordered lifecycle and its terminal accounting
  identity. Its stable evidence lines include measured compute units and the ELF
  digest.
- `sbf.token2022_program_test` executes that program form and the real
  Token-2022 binary in an in-process Agave bank. It adds extension-policy cases,
  mandatory token/collateral plane tests, out-of-band reconciliation failure,
  and the E5 post-CPI atomic-rollback case. These are not redundant with the
  loopback differential.

The present runtime inventory also needs careful reading. The sealed R1 profile
now admits the measured local `ResolutionWork` Begin/Fold/Finalize/Abort routes:
the current-profile gate hashes the exact ELF and 23 committed audit/build/bank
files, rederives the rent/reward/account projection, and rejects drift from its
frozen source closure. That is a bounded subsystem result, not global liveness,
future inclusion, a production provider, or terminal closure. The default
bringup ELF has an empty production source registry and must refuse `Endow`;
only the explicitly named mock-source ELF can exercise the local successful
path. Similarly, the Direct V2 measurement is a negative result: a
three-Candidate selection reaches the 1,400,000-CU limit and rolls back. The V3
checks in `batch-policy-identity` are bounded host-model evidence, not a live
ABI or runtime promotion. ResolutionWork's measured payer/rent return also does
not close existing storage, mint, donation, bearer-burn, or fractional-residue
terminal STOPs.

`cargo-build-sbf` may diagnose excessive frames in public dependency functions
while compiling rlibs. The bringup gate extracts those symbol names and refuses
unless every one is absent from the final linked, unstripped ELF after LTO. The
manifest therefore classifies the observed messages as backend build
diagnostics for eliminated symbols, not reachable undefined behavior. Symbol
absence is also not a general stack-safety proof; a surviving diagnostic is an
immediate gate failure.

### `digests`

Three kinds:

- `file-sha256` — the sha256 of a file's bytes. Covers the static client bundle,
  the E0 probe source and its lock, both vertical-model golden traces, the
  collateral-profile vector files, the benchmark goldens and their checksum
  file, the economics fixtures, every `Cargo.lock` named by §5, both toolchain
  pin records, and the Rocq/Verus shadow sources.
- `derived-sha256` — a declared canonicalization rather than raw bytes.
  Currently one: `static_client.canonical_terms`, the sha256 of the
  `canonicalTerms` object serialized as compact UTF-8 JSON with every object's
  keys sorted recursively. The rule is stored in the manifest so an independent
  implementation can reproduce it without reading the script;
  `apps/static-client/test/smoke.mjs` enforces the same rule from the other side.
- `declared-build-output` — a named identity that is *not* a repository file.
  There are two: the E0 SBF `rlib` produced in `run_lab.sh` scratch, and the
  deployable `clutch_sbf.so` built twice in `run_bringup.sh` scratch. With
  `--run-gates`, each `observed_sha256` is lifted from the producing gate's
  stable output and compared with the documented value. Both are same-machine
  comparisons, not independent reproducible-build closure.

Where §6 (or a named implementation note) declares a digest, the entry carries
`handoff_declared_sha256`, `matches_handoff`, and `handoff_reference`. Ids that
disagree are collected into the top-level `handoff_digest_disagreements`. This
is deliberately a *finding*, not an error: the correct response is to fix
whichever of the tree or the prose is stale, and the manifest's job is to make
sure nobody has to notice by hand.

### `toolchain`

`versions.env` is parsed as `KEY=VALUE` with quote stripping. The manifest stores
the parsed host, Verus, and Rocq pin groups, plus the sha256 of both
`toolchain/versions.env` and `toolchain/PINNED_PROOF_TOOLS.md` — those digests,
not the parsed convenience fields, are the authoritative binding.

`pin_agreement` checks that every sha256 in `versions.env` also appears literally
in `PINNED_PROOF_TOOLS.md`, so the machine-readable and human-readable pin
records cannot silently diverge.

`unpinned` restates, in the manifest itself, what `PINNED_PROOF_TOOLS.md`
declares is *not* pinned: the `vstd` revision (transitive via `VERUS_COMMIT`
only), Homebrew formula provenance, the Rocq dependency closure, the ambient
`librustc_driver` dylib, and — the important one — any correspondence between
the Verus/Rocq shadows and `crates/clutch-*`.

## What this manifest does NOT attest

Verbatim from the manifest's own `claims.not_attested`:

- **No release.** Nothing here publishes, tags, pushes, or authorizes a release.
  `claims.release` is `false` and is not a field the tool can be argued into
  setting.
- **No signature chain.** No signed tag, no signed artifact, no key material, no
  transparency log entry. A sha256 in this file proves only that the emitting
  machine saw those bytes.
- **No independent reproducible-build closure.** `run_lab.sh` rebuilds one narrow
  SBF `rlib` twice, and `run_bringup.sh` builds the deployable program ELF twice
  into fresh target directories. Both comparisons occur on one machine with the
  installed toolchain. There is no independent rebuilder, toolchain bootstrap,
  or rebuild from pinned dependency sources.
- **No formal proof content.** The Rocq gate typechecks `Definition`s (zero
  theorems), while the root Verus probe is an expected failure. The isolated
  Verus batch shadow is still in flight, is deliberately absent from this
  manifest, and is not a claim until its source and reproduction gate are
  committed. `check` will flag a silent root-gate disposition change as drift.
- **No non-local runtime evidence.** The two SBF gates record a loopback validator
  differential/lifecycle and an in-process Agave/Token-2022 bank suite. They do
  not establish public-cluster behavior, deployment, validator diversity, an
  independently rebuilt ELF, or cross-runtime vector closure.
- **Measured ResolutionWork is not global liveness.** The sealed local R1
  artifact and its committed logs admit the bounded ResolutionWork route under
  its frozen policy. They do not prove transaction inclusion or emit a complete
  protocol-wide liveness policy.
- **No production-provider closure.** The default runtime ELF refuses `Endow`
  without a registered source release. The successful mock-source path is local
  test evidence only, not an oracle/source-release or deployment claim.
- **No direct-selection promotion.** The V2 three-Candidate selection measurement
  reaches the 1,400,000-CU transaction limit and rolls back. The V3 authority
  work is a bounded host model with live ABI/runtime and terminal-cleanup STOPs.
- **No terminal closure.** ResolutionWork's measured Work/Reserve payer and rent
  return does not close separate legacy storage, mint, donation, bearer-burn, or
  fractional-residue STOPs.
- **No SBOM**, license closure, fixture provenance chain, or source offer.
- **No published provenance.** The identities are git object ids. A configured
  remote or a pushed branch is neither a signed tag nor a release artifact.
- **No security review and no regulatory closure.** Gate L0 remains open.

A green `check` means exactly: *the bytes and the named check outcomes are what
the manifest recorded*. It does not mean correct, verified, safe, or deployable.

## Promotion path to a real release manifest

Every step below is future work and every step is user-gated. None may be taken
by inference from this document.

1. **Clean-tree baseline.** Commit every source/gate change, emit with
   `--strict --run-gates`, then commit only the generated manifest. Because the
   v2 content identity excludes that artifact, `check` remains green after the
   manifest-only commit. A `--allow-dirty` record remains a labelled snapshot,
   never a baseline.
2. **CI-independent re-derivation.** A second machine runs `check --run-gates`
   against the committed manifest. Until that happens, "deterministic" is a
   property of one machine.
3. **Remote and signed tag.** Requires explicit user direction (§7 P0-1). A
   detached signature is a sibling artifact, never a field the generator writes
   about itself. Historical remote/tag observations live under
   `baseline.provenance`; a real release record must separately bind the tag and
   signature.
4. **SBOM and license closure.** Dependency graph, licenses, fixture provenance,
   and a source offer, each digest-bound into the manifest.
5. **Reproducible-build closure.** An independent rebuilder reproducing the SBF
   ELF, not one machine rebuilding one `rlib` twice. Only then may
   `not_attested` lose that line.
6. **Proof-result records.** When a pinned tool checks a named non-vacuous
   theorem, its record joins the manifest in the
   [`docs/EVIDENCE_MATRIX.md`](../EVIDENCE_MATRIX.md) §3 artifact-ledger shape
   (property id, statement digest, tool version and commit, assumption manifest,
   reproduction command, unclosed boundaries). The current `digests` block is
   the source/lock half of that ledger; the proof half is empty and must stay
   visibly empty.
7. **Deployment identity.** Program id, program-data account, upgrade authority,
   and ELF digest — only under a separately authorized deployment, and never
   before Gate L0 closes.

`claims.verified`, `claims.deployed`, and `claims.release` flip only when the
corresponding evidence exists and is named in the manifest. A convenience commit
may not flip them to obtain a green result.

## Runtime-gate boundary

The gate inventory currently has 70 declarations: the `clutch-sbf` cargo gates,
documented low-cost research/model/frontend checks, post-handoff benchmark/ABI
checks, and both real SBF runtime gates. A full `--run-gates` baseline is the
evidence path, not a fast presubmit; on a cache-cold host it can take tens of
minutes. The added research gates cover current committed surfaces such as
direct-selection authority, the shape compiler, source profile, ResolutionWork
model, sealed liveness-profile checks, and the 32-check offline Glass client.
The liveness current-profile gate rehashes committed evidence and recompiles an
archived host probe; it adds no fresh SBF build. A
declaration-only `emit` remains useful for inspecting structure but sets
`claims.reviewed_offline_checks_recorded` to `false` and records no run outcomes.
