# Baseline evidence manifest

`scripts/baseline_manifest.py` derives `MANIFEST.baseline.json`, a single
machine-readable record of what the reviewed offline baseline *is* and what its
named checks *did*. Its purpose is narrow and worth stating twice: it turns
"the baseline is intact" into a claim a machine can re-derive and contradict.

It is **not** a release manifest. It publishes nothing, signs nothing, tags
nothing, and does not turn any named check into a release, deployment, security,
legal, or whole-system verification conclusion. This is evidence bookkeeping
under the current truth boundary, not closure of any release blocker.

Status: **IMPLEMENTED** (source exists locally, the named offline checks record
their own outcomes). This includes the current low-cost host-model, research,
and Glass gates. It does not promote any of those results into runtime,
deployment, or system-proof evidence. Individual proof/model lanes retain their
own narrow labels and explicit boundaries; deployment remains a blocker.

## Usage

```sh
# Derive digests, toolchain pins, and gate declarations only (fast).
scripts/baseline_manifest.py emit

# The full record: also execute every declared gate and store exit codes plus
# normalized key output lines. This is intentionally not a presubmit: a
# cache-cold run can take tens of minutes because it includes host research,
# Lean/Verus lanes, two profiles rebuilt twice by bringup, and four local SBF
# bank/validator lanes (default SVM, explicit mock SVM, loopback lifecycle, and
# the signed committed walk).
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

The checked-in `MANIFEST.baseline.json` is now a clean-tree **schema-v2**
emission, committed at `6743b9d`. It records 94/94 declared gate outcomes, and
`check --run-gates` passes after that manifest-only commit. This is a checked
local evidence baseline, not a release manifest.

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
| `claims` | `verified`/`deployed`/`release` (all `false`), label vocabulary, and `not_attested` |
| `gates` | every declared check: id, current inventory class, command, expected disposition, key-line patterns, note |
| `gate_runs` | with `--run-gates`: per gate exit code, expectation match, normalized key lines, and key-line digest |
| `gate_summary` | totals and the list of gates that contradicted their declaration |
| `digests` | the artifact ledger: per-entry path, kind, sha256, and the handoff-declared value where one exists |
| `toolchain` | parsed `versions.env` pins, digests of both pin records, their cross-agreement, and the explicit unpinned list |
| `unavailable_or_failing_gates` | every reviewed non-ordinary disposition plus every unexpected mismatch, with declared and observed status; a successful exact-output gate is never described as a contradiction |
| `handoff_digest_disagreements` | ids where a tree digest disagrees with its named declared authority |
| `run` | wall-clock timestamps; ignored by `check` |

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

Commands are the current repository's documented local forms, with the
per-manifest loops expanded so each crate's `cargo test`, `cargo clippy`, and
`cargo doc` carries its own exit code. Each gate is run through `/bin/sh -c`
from the repository root with `CARGO_NET_OFFLINE=true`, `NO_COLOR=1`, and
`LC_ALL=C`.

Each command starts in its own process group. A per-gate timeout sends `SIGKILL`
to that group and waits for it to exit, so a timed-out compiler, validator, or
helper cannot leak descendants into a later gate.

`section` records where a gate comes from:

- `current-baseline` — maintained core offline checks;
- `documented-extension` — a separately documented current surface, presently
  the coupled vertical-model golden trace;
- `current-benchmark` — checked benchmark-harness and ABI checks;
- `current-research` — documented bounded host research/model/frontend checks;
- `current-runtime` — loopback bringup, default and explicitly non-production
  mock local SVM suites, and the signed committed local walk; and
- `current-proof-boundary` — Lean/Verus/Rocq commands whose proof content and
  boundaries are recorded explicitly rather than being inflated into a generic
  “verified” label.

`expected.mode` is the reviewed disposition, not a wish:

- `zero` — the gate must exit 0 and, when declared, print every required
  stable output pattern. The source-profile, failure-payout, and
  terminal-economics model gates respectively require their 32-, 18-, and
  16-test summaries. The scalar batch proof additionally requires its 28
  verified obligations and all five named expected-red mutants;
- `exact` — one non-zero tool disposition is the only accepted result. The root
  `toolchain/scripts/run_verus.sh` probe must return **exit 1**, the pinned
  Verus proof-status failure for the digest-pinned source, and print its
  reviewed `verus_builtin crate was not imported` diagnostic. Its setup
  refusals are deliberately rejected: exit 2 means missing tool, 3 means an
  off-pin tool/frontend, and 4 means source-digest drift;
- `either` — `rocq/check.sh`, accepting exit 0 (the `.v` file elaborates) or
  exit 2 (no `rocq`/`coqc` on `PATH`). Both carry `proof_content: "none"`:
  `ClutchKernel.v` contains zero theorems, only `Definition ... : Prop`
  obligations, one of which has a machine-checked vacuous conjunct. A `PASS`
  here means "the definitions typecheck" and nothing more.

`key_patterns` are the regexes whose matching output lines are stored. Lines are
normalized to strip elapsed times (`; finished in 0.03s`, `Ran 45 tests in
0.12s`) so that a re-run on the same tree produces the same `key_lines_sha256`.
For lint and doc gates the clean state is *no* matched lines. Cargo's
`Documenting ...` progress is intentionally excluded because target paths and
cold/warm cache progress are nonsemantic. Raw output, byte counts, and failure
tails are never stored: they contain temporary paths, timing, and cache noise.

A gate may additionally declare `volatile_patterns` and a reason. Those lines
are deliberately excluded from the run record, not merely from its digest. The
current example is `run_lab.sh`'s `host_rlib_sha256`: it changes because the host
probe is built in a fresh `mktemp` target directory whose path is embedded in
the artifact. `run_lab.sh` measures no host reproducibility; only
`sbf_rlib_sha256` is a same-machine two-build comparison. Excluding the volatile
host hash keeps cold/warm and cross-path manifest records byte-stable.

The four runtime gates have intentionally different coverage:

- `sbf.runtime_bringup` builds both the default empty-production-source-registry
  ELF and the distinct explicitly non-production mock-source ELF twice into
  fresh targets, requiring per-profile byte identity. It then launches a new
  loopback `solana-test-validator`, waits for a transaction-level program
  readiness probe, and runs a profile-bound differential/refusal matrix. The
  default plan declares Endow only as `Custom(0x0079)` and carries no lifecycle;
  the explicitly different mock plan is the only one that declares successful
  Endow and runs the ordered lifecycle. Its stable evidence records
  `default_sbf_elf_sha256`,
  `non_production_mock_sbf_elf_sha256`, `default_reproducibility`, and
  `mock_reproducibility`; these are observed fresh-run identities, not a release
  pin or independent build result.
- `sbf.token2022_program_test` executes the default program form and the real
  Token-2022 binary in an in-process Agave bank. It adds extension-policy cases,
  mandatory token/collateral plane tests, out-of-band reconciliation failure,
  and the E5 post-CPI atomic-rollback case. These are not redundant with the
  loopback differential.
- `sbf.token2022_program_test_non_production_mock` separately builds and runs
  the mock-source ELF in that same local bank. It exists so a successful mock
  source route cannot be confused with the default ELF's fail-closed production
  provider behavior.
- `sbf.committed_signed_walk` builds the explicitly different
  `non-production-mock-source` ELF, then submits 22 signed, confirmed
  transactions against one fresh loopback validator, including one expected
  semantic refusal and one measured two-instruction compute-ceiling STOP; both
  leave all watched state unchanged. It reloads 18 watched accounts and reruns after a
  terminal-byte corruption that must go red. Its successful Endow is evidence
  only for that compiled laboratory release; the default production-inert ELF
  still refuses this unregistered V1 spec with `0x79`. Its 12 program-owned prerequisites are genesis-injected;
  it is not blank-bank source ingestion, production-provider, deployment,
  devnet, or mainnet evidence.

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
  file, the economics fixtures, selected core and newly added gate `Cargo.lock`
  files, both toolchain
  pin records, and the Rocq/Verus shadow sources.
- `derived-sha256` — a declared canonicalization rather than raw bytes. The
  current inventory is empty: the obsolete static-client fixture terms were
  removed rather than retained as a second protocol truth.
- `declared-build-output` — a named identity that is *not* a repository file.
  There are three kinds of entry: the E0 SBF `rlib` with its reviewed literal
  pin, plus separately named default and explicitly non-production mock
  `clutch_sbf.so` profiles. With `--run-gates`, each `observed_sha256` is lifted
  from stable output. The two current SBF profile identities are intentionally
  observed rather than compared to an invented external pin; the sealed R1
  default artifact/log identity is checked separately by
  `python.liveness_policy_profile_current_seal`. All are same-machine evidence,
  not independent reproducible-build closure.

Where a named current source or implementation note declares a digest, the entry carries
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
`librustc_driver` dylib, and — the important one — any whole-system
correspondence between proof/model lanes and `crates/clutch-*` outside the
explicitly pinned transfer helper.

## What this manifest does NOT attest

Verbatim from the manifest's own `claims.not_attested`:

- **No release.** Nothing here publishes, tags, pushes, or authorizes a release.
  `claims.release` is `false` and is not a field the tool can be argued into
  setting.
- **No signature chain.** No signed tag, no signed artifact, no key material, no
  transparency log entry. A sha256 in this file proves only that the emitting
  machine saw those bytes.
- **No independent reproducible-build closure.** `run_lab.sh` rebuilds one narrow
  SBF `rlib` twice, and `run_bringup.sh` builds each default and non-production
  mock ELF twice into fresh target directories. All comparisons occur on one
  machine with the installed toolchain. There is no independent rebuilder,
  toolchain bootstrap, or rebuild from pinned dependency sources.
- **No whole-system formal proof.** The Rocq gate typechecks `Definition`s
  (zero theorems). The root Verus probe accepts only its exact pinned proof-tool
  exit 1; missing, off-pin, and source-drift exits are rejected. The committed
  batch lane checks a scalar mathematical shadow; the transfer lane checks a
  narrow production arithmetic helper; and the B-spline lane checks finite
  Lean/Rust rows. None proves a whole kernel, accounts, CPI, SBF, or runtime
  refinement. `check` flags any disposition or source-pin drift.
- **No non-local runtime evidence.** The four local SBF gates record a loopback
  validator differential/lifecycle, a signed genesis-assisted committed walk,
  and default/explicit-mock in-process Agave/Token-2022 bank suites. They do
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
3. **Remote and signed tag.** Requires explicit user direction. A
   detached signature is a sibling artifact, never a field the generator writes
   about itself. Historical remote/tag observations live under
   `baseline.provenance`; a real release record must separately bind the tag and
   signature.
4. **SBOM and license closure.** Dependency graph, licenses, fixture provenance,
   and a source offer, each digest-bound into the manifest.
5. **Reproducible-build closure.** An independent rebuilder reproducing the SBF
   ELF, not one machine rebuilding one `rlib` twice. Only then may
   `not_attested` lose that line.
6. **Proof-result records.** The current manifest records bounded gate
   dispositions for the scalar batch shadow, narrow transfer helper, and finite
   B-spline bridge. A future wider theorem or refinement record must join the
   manifest in the
   [`docs/EVIDENCE_MATRIX.md`](../EVIDENCE_MATRIX.md)'s artifact-ledger shape
   (property id, statement digest, tool version and commit, assumption manifest,
   reproduction command, unclosed boundaries). A gate result is still not an
   account/SBF/runtime or whole-system proof claim.
7. **Deployment identity.** Program id, program-data account, upgrade authority,
   and ELF digest — only under a separately authorized deployment, and never
   before Gate L0 closes.

`claims.verified`, `claims.deployed`, and `claims.release` flip only when the
corresponding evidence exists and is named in the manifest. A convenience commit
may not flip them to obtain a green result.

## Runtime-gate boundary

The inventory has 98 declarations; the generator records the same count in
`gate_summary.total` when it runs. It covers core crates including the
B-spline, occupation accumulator, and liveness kernel; documented model and
frontend checks; the vector executor and invariant campaign; Lean and bounded
refinement lanes; the 32-test source-profile, 18-test failure-payout, 16-test
terminal-economics, and 16-test terminal-lifecycle host models; and the four
local runtime lanes. The batch scalar proof is likewise bounded: 28 verified
mathematical obligations and five expected-red source mutants, not an
executable-body or runtime refinement. A full `--run-gates` baseline is evidence
collection, not a fast presubmit. A cache-cold host can take tens of minutes and
invokes nine bounded SBF compiler builds: two E0 `rlib` builds, four bringup
builds (two default, two mock), default and explicit-mock SVM builds, and one
committed-walk build. No runtime command contacts a public RPC, deploys, or
releases anything. The signed-walk gate creates fresh ephemeral local keypairs
and test-only validator funds to sign loopback transactions; it never reads a
real/user wallet or key, uses user funds, or submits to a public cluster. The
liveness current-profile gate rehashes sealed artifact/log evidence and
recompiles an archived host probe; it adds no fresh SBF build. A declaration-only
`emit` remains useful for inspecting structure but sets
`claims.reviewed_offline_checks_recorded` to `false` and records no run outcomes.
