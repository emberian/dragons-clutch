# Verification and evidence matrix

Status: planning inventory. A row is a promised artifact shape, not evidence that
the theorem, implementation, or build presently exists.

## 1. Evidence policy

V1 is Verus-first for the executable kernel. Rocq is the leading independent
mathematical shadow. Lean is a compatible secondary shadow seam for small finite
relations, generated vectors, or independent reproduction; it does not create a
requirement to maintain three executable semantic implementations.

No artifact earns the phrase “formally verified” by itself. A release statement
must name the property ID, source/model digest, toolchain digest, result, trusted
assumptions, and every unclosed adapter/refinement boundary.

## 2. Property inventory

| ID | Property | Verus executable | Rocq shadow | Lean seam | Host/SBF falsifier |
|---|---|---|---|---|---|
| `P-PART-01` | accepted numeric partition is ordered, exhaustive, disjoint | required | required | optional reproduce | boundary corpus |
| `P-PART-02` | exact value has one cell; interval has canonical compatible set | required | required | optional | exhaustive small domains |
| `P-PAY-01` | bounded dot product cannot overflow and rounds once | required | required | vector checker | differential maxima |
| `P-SOLV-01` | successful transition preserves maximum-liability solvency | required local | required reachable-state | optional | randomized traces |
| `P-SUP-01` | materialize/dematerialize preserves total categorical supply | required | required | optional | direct-burn fixtures |
| `P-POOL-01` | Hoard, order, fee, liveness, rent, treasury cannot alias | required transition | required noninterference | optional | hostile account tests |
| `P-FEE-01` | fee collection/allocation/carry conserves atoms | required | required | vector checker | fragmentation corpus |
| `P-ACC-01` | summary combine is associative over admitted adjacent ranges | required | required | preferred reproduce | all page splits |
| `P-ACC-02` | accepted coverage and cursor are monotone | required | required | optional | repair/replay corpus |
| `P-BATCH-01` | accepted candidate satisfies every filled order limit | required | required | finite relation | exhaustive tiny books |
| `P-BATCH-02` | accepted candidate conserves collateral and every Egg | required | required | finite relation | single-atom mutations |
| `P-BATCH-03` | resumed/page-sharded verification equals one ordered fold | required | required | optional | shard permutations |
| `P-BATCH-04` | final pots and reservations are disjoint ownership phases | required | required | optional | replay/race tests |
| `P-BATCH-05` | candidate comparison is deterministic total ordering | required | required | finite relation | ties and digest edges |
| `P-SET-01` | settlement is idempotent and order independent | required | required | optional | permutation tests |
| `P-CODEC-01` | canonical codec round-trips and rejects aliases | required | model bytes | vector checker | malformed-byte fuzzing |
| `P-LIVE-01` | admission preserves worst-case unfinished-work booking | required | required | optional | zero-volume traces |

## 3. Artifact ledger schema

Every completed property produces a machine-readable record with at least:

```text
property_id
statement_digest
model_or_source_digest
tool_name
tool_version_and_commit
configuration_digest
dependency_lock_digest
result
assumption_manifest_digest
proof_or_log_digest
vector_manifest_digest
generated_artifact_digests
timestamp
reproduction_command
known_unclosed_boundaries
```

Logs alone are not theorem inventories. A theorem name without the exact source
and assumptions is not release evidence.

## 4. Verus gate

The first implementation packet must prove tiny kernels before building the full
adapter. Release verification runs without focus/filter modes and mechanically
rejects first-party `unsafe`, FFI, `assume`, `admit`, axioms, `external_body`,
`assume_specification`, executable `cfg(verus_only)`, unchecked casts, and public
proof-only preconditions.

The gate archives both annotated and erased source digests. The same executable
semantics must compile under the pinned host and Anza SBF toolchains. A successful
Verus result does not prove the SBF compiler or emitted ELF.

## 5. Rocq shadow gate

The Rocq model is Rust-independent and transition-oriented. It should prove
reachability and global invariants that are awkward to express as only local Rust
contracts. Extraction provides one independent semantic oracle.

The model may not use admitted obligations, project axioms, or unreviewed
parameters in the release theorem closure. Its correspondence to Eggcrate remains
manual and named until an independently checked refinement is complete.

## 6. Lean shadow seam

Lean is optional for V1 but intentionally accommodated through:

- language-neutral canonical semantic vectors;
- closed finite relation definitions rather than Rust ABI types;
- theorem/property IDs shared by meaning, not proof-assistant syntax;
- exact integer/rational reference inputs and outputs;
- no proof-assistant-specific fields in consensus bytes.

A useful first Lean experiment is the finite `BatchRelationV1` conservation and
deterministic-score model over tiny bounds. If it merely duplicates Rocq at high
maintenance cost, retain the seam and do not make it a release blocker.

## 7. Cross-runtime differential gate

For every semantic vector:

```text
ordinary Rust reference
Verus-checked Eggcrate host execution
Rocq-extracted evaluator
optional Lean evaluator/checker
SBF program-test adapter
```

must return the same canonical success value or mapped error class. The manifest
records deliberate differences such as adapter-level account errors that have no
kernel analogue. Random inputs are reproducible from a fixed generator version
and seed; minimized failures become permanent named fixtures.

## 8. Mutation gate

Mutation testing is property-directed. At minimum mutate:

- one partition comparator and one boundary canonicalization check;
- one payoff coefficient width/range check and the rounding placement;
- one split/merge/materialization supply update;
- one Hoard debit condition;
- one fee numerator, carry, and allocation remainder;
- one accumulator boundary/generation check;
- one order omission/duplication check;
- one virtual complete-set outcome delta;
- one settlement replay bit;
- one codec tag/length/padding check.

Each mutation must be killed by a named theorem, vector, adversarial test, or
multiple layers. A surviving mutation is a gap, not a flaky test to suppress.

## 9. Adapter and release evidence

The unverified adapter requires hostile-byte fuzzing, account-alias tests, CPI
substitution tests, atomic rollback checks, local-validator walks, resource
measurements, and reproducible SBF builds. A release evidence bundle binds:

- source and dependency locks;
- theorem and assumption inventories;
- canonical and randomized vectors;
- mutation results;
- SBF ELF and static bundle hashes;
- SBOM, licenses, fixture provenance, and source offer;
- program-data/upgrade-authority identity only for a separately authorized
  deployment.

Passing these gates does not close the regulatory mainnet gate.
