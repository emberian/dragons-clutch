# Baseline evidence manifest

`scripts/baseline_manifest.py` derives `MANIFEST.baseline.json`, a single
machine-readable record of what the reviewed offline baseline *is* and what its
named checks *did*. Its purpose is narrow and worth stating twice: it turns
"the baseline is intact" into a claim a machine can re-derive and contradict.

It is **not** a release manifest. It publishes nothing, signs nothing, tags
nothing, and attests no proof content. `CODEX_HANDOFF.md` §7 P0-1 remains open
after this file lands; this is groundwork under that blocker, not its closure.

Status: **IMPLEMENTED** (source exists locally, the named offline checks record
their own outcomes). Everything the manifest describes about *proofs* remains
**BLOCKER** per §7 P0-2, and everything it describes about *deployment* remains
**BLOCKER** per §7 P0-6.

## Usage

```sh
# Derive digests, toolchain pins, and gate declarations only (fast).
scripts/baseline_manifest.py emit

# The full record: also execute every declared gate and store exit codes plus
# normalized key output lines. Several minutes; builds two SBF rlibs.
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

`emit` is `--strict` by default and **refuses** when `git status --porcelain` is
non-empty. It prints every dirty path and exits 2 without writing anything. A
manifest emitted from a dirty tree would pair a clean commit id and tree hash
with bytes that are not in that tree — a false claim of provenance in the exact
field the manifest exists to make trustworthy.

`--allow-dirty` is the escape hatch for mid-flight snapshots. It emits, but the
manifest then carries:

- `"dirty": true`;
- `dirty_warning`, prose stating that the recorded `commit`/`tree_hash` do **not**
  describe the hashed bytes;
- `dirty_porcelain`, the verbatim porcelain listing;
- `claims.manifest_label: "PROPOSED"` instead of `"IMPLEMENTED"`.

A dirty manifest may be used to see the machinery work or to capture a wave in
progress. It may never be cited as a baseline. `check` re-states this in its
success output so that a passing check on a dirty snapshot cannot be quoted as a
clean-tree result.

Exactly one path is excluded from the dirtiness decision: the manifest's own
output path. It is this tool's output, not an input, and letting its git status
gate its own regeneration would mean the first emit permanently blocks every
later one. The exclusion is recorded in the manifest as `dirty_check_excludes`,
including whatever status the excluded path actually had, so it is stated rather
than hidden. Nothing else is ever excluded.

One consequence deserves saying plainly: gates always execute against the
**working tree**, never against the recorded commit. On a clean tree those are
the same thing, which is the entire reason `--strict` is the default. On a dirty
tree they are not, and a gate that fails there may be reporting another lane's
in-progress edit rather than a defect in the baseline — a `Cargo.toml` whose
matching `Cargo.lock` has not been regenerated will fail every `--locked` gate
for that crate, for instance. Read a dirty snapshot's failures as observations
about *now*, and re-emit on a clean tree before drawing conclusions.

## Schema `dragons-clutch/baseline-manifest/v1`

| Key | Contents |
| --- | --- |
| `schema` | the literal `dragons-clutch/baseline-manifest/v1` |
| `generator` | path and sha256 of the script that produced the manifest |
| `dirty`, `dirty_warning`, `dirty_porcelain` | worktree state; the latter two only when dirty |
| `dirty_check_excludes` | the one path excluded from the dirtiness decision, stated explicitly |
| `baseline` | `commit`, `tree_hash`, `commit_subject`, `remotes`, `tags_at_head` |
| `claims` | `verified`/`deployed`/`release` (all `false`), the §1 label vocabulary, and `not_attested` |
| `gates` | every declared check: id, handoff section, verbatim command, expected disposition, key-line patterns, note |
| `gate_runs` | with `--run-gates`: per gate exit code, expectation match, normalized key lines, key-line digest, and any `volatile_lines` |
| `gate_summary` | totals and the list of gates that contradicted their declaration |
| `digests` | the artifact ledger: per-entry path, kind, sha256, and the handoff-declared value where one exists |
| `toolchain` | parsed `versions.env` pins, digests of both pin records, their cross-agreement, and the explicit unpinned list |
| `unavailable_or_failing_gates` | the honest list, declared and observed |
| `handoff_digest_disagreements` | ids where the tree disagrees with `CODEX_HANDOFF.md` §6 |
| `run` | wall-clock timestamps; the **only** nondeterministic fields, ignored by `check` |

### `baseline`

`commit` and `tree_hash` are git object ids. `remotes` records configured remote
names and fetch URLs; `tags_at_head` records tags pointing at `HEAD`.

Neither is provenance, and the manifest says so inline rather than letting a
populated `remotes` list imply something it does not. A configured remote means
a push destination is configured. A pushed branch means bytes were copied to a
host. Neither is signed, neither is tagged, and neither is attested here. An
empty `tags_at_head` means no release tag exists — the expected state while
`CODEX_HANDOFF.md` §7 P0-1 is open.

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
- `5-expected-unavailable` — the two proof gates §5 lists as expected failures.

`expected.mode` is the reviewed disposition, not a wish:

- `zero` — the gate must exit 0;
- `nonzero` — the gate is **expected to fail**, with the reason recorded inline.
  `toolchain/scripts/run_verus.sh` is the only one: the pinned Verus release
  rejects the pinned probe source (`verus_builtin` crate not imported). Making it
  pass requires editing the probe, which changes the pinned source digest, so a
  recorded failure is the correct state and a green gate here would be the
  defect;
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
- `declared-build-output` — an identity `CODEX_HANDOFF.md` §6 states that is
  *not* a repository file. Currently one: the reproducible E0 SBF `rlib`, which
  only exists inside the temporary directory `toolchain/scripts/run_lab.sh`
  creates. With `--run-gates` its `observed_sha256` is lifted from that gate's
  `sbf_rlib_sha256` line and compared to the declared value.

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
- **No reproducible-build closure.** The only reproducibility measured anywhere
  in this repository is `run_lab.sh` rebuilding one narrow SBF `rlib` twice on
  the same machine and comparing. No ELF, no toolchain bootstrap, no rebuild
  from dependency sources, no independent rebuilder.
- **No proof content.** The Rocq gate typechecks `Definition`s (zero theorems);
  the Verus gate fails on the pinned probe. Both are recorded as-is, and
  `check` will flag it as drift if either silently changes disposition.
- **No SBF runtime evidence.** No entrypoint, program-test lifecycle, Token-2022
  CPI, CU/stack/heap measurement, or cross-runtime vector closure.
- **No SBOM**, license closure, fixture provenance chain, or source offer.
- **No published provenance.** The identities are git object ids. A configured
  remote or a pushed branch is neither a signed tag nor a release artifact.
- **No security review and no regulatory closure.** Gate L0 remains open.

A green `check` means exactly: *the bytes and the named check outcomes are what
the manifest recorded*. It does not mean correct, verified, safe, or deployable.

## Promotion path to a real release manifest

Every step below is future work and every step is user-gated. None may be taken
by inference from this document.

1. **Clean-tree baseline.** Emit with `--strict --run-gates` on a clean tree and
   commit the result. That is the first manifest that may be called a baseline;
   the first one generated is a `--allow-dirty` snapshot taken to prove the
   machinery, and is labelled as such.
2. **CI-independent re-derivation.** A second machine runs `check --run-gates`
   against the committed manifest. Until that happens, "deterministic" is a
   property of one machine.
3. **Remote and signed tag.** Requires explicit user direction (§7 P0-1). Adds
   `baseline.remotes` and `baseline.tags_at_head` content, and a detached
   signature over the manifest. The manifest schema is stable across this: the
   signature is a sibling artifact, never a field the generator writes about
   itself.
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

## Addendum 2026-08-19: SBF lane and post-section-5 gates

The gate inventory now includes the `clutch-sbf` cargo gates and two
post-section-5 benchmark gates (`benchmarks.unittest`,
`benchmarks.abi_audit`). Deliberately NOT a manifest gate: the SBF
reproducible-ELF build and the local-validator differential — both are
multi-minute, environment-heavy procedures whose evidence is recorded in
`docs/implementation/SBF_BRINGUP.md`; the manifest attests neither.
