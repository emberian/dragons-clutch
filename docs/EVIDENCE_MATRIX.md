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
| `P-SUP-01` | materialize/dematerialize preserves per-native-basis-Egg total supply | required | required | optional | direct-burn fixtures |
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

### Addendum, 2026-08-18 — the seam is now the primary home of model theorems

Status of this addendum: **MODEL for what it says is proved; PROPOSED for
everything else.** Nothing in §6 above is amended, retracted, or rewritten; the
paragraph above stands as the record of what was planned. This addendum records
a decision that supersedes its posture, and the artifact that now exists.

**The decision (the user's).** Properties of the semantic plane are proven of a
**mathematical model in Lean**, independently of the Rust implementation. Two
implementations of the semantic plane is the accepted cost. The correspondence
between model and implementation is bounded **empirically, by the canonical
semantic vectors both evaluate**, and is never claimed as proven. Lean is
therefore no longer "optional for V1"; it is where semantic-plane theorems live.
Verus is unchanged and remains the only tool that says anything about the Rust
source. The Rocq shadow is unchanged and undecided (see the plan, §9.4).

**The claim shape, which every statement about this work must use verbatim:**

> Lean 4.33.0 checked theorem `T` about the model `M` in `lean/` at source
> digest `d`, under hypotheses `H`. `M` is a hand-written mathematical model of
> the kernel's semantic plane. Its correspondence to `crates/clutch-kernel` is
> manual, unproved, and bounded only by the semantic vectors both evaluate. No
> theorem in `M` is a statement about the Rust program, the compiled SBF ELF, or
> any deployed program.

**What exists as of this date.** `lean/` is a dependency-free Lake package
(Lean 4.33.0, commit `d8b18978322de05a8f3dba51ef03cf5461676c17`, no Mathlib, no
registry dependencies) that builds with zero errors and zero warnings and
contains 86 theorems, none using `sorry`, project axioms, `native_decide`, or
`unsafe`, and all closing over only Lean's three standard axioms (`propext`,
`Classical.choice`, `Quot.sound`). It models the kernel state, the ten
transitions, and the payout basis as a partition-of-unity hypothesis on a total
weight map. Architecture, toolchain pin, findings, and the ranked next theorems
are in
[`implementation/LEAN_MODEL_PLAN.md`](implementation/LEAN_MODEL_PLAN.md).

**Property rows this changes.** The §2 table is not edited here; these are the
Lean-column dispositions the work supports, for a reviewer to apply:

| ID | Lean column, as written | Supported by this work |
|---|---|---|
| `P-SOLV-01` | optional | **primary** — the maximum-liability bound, its exact-supremum form over the frozen simplex lattice, and per-transition preservation are proved of the model |
| `P-PAY-01` | vector checker | **primary for the arithmetic-width half** — the liability numerator provably fits `u128`; the rounding-placement half stays with Verus and the vectors |
| `P-SUP-01` | optional | **primary** — materialize/dematerialize are market-neutral and transfer is claim-conserving, proved |
| `P-PART-01`, `P-PART-02` | optional reproduce | unchanged; not yet modelled (ranked next in the plan, §5.2) |
| `P-BATCH-01`, `-02`, `-05` | finite relation | unchanged; the plan names the Mathlib decision these force (§3.1) |
| all others | unchanged | unchanged |

**One proposed new row.** The property that a **complete set redeems for exactly
`q` at every admissible resolved value** — the unconditional exit from the
fractional-payout trap — has no ID in §2. This work proves it of the model and
proposes:

| ID | Property | Verus executable | Rocq shadow | Lean seam | Host/SBF falsifier |
|---|---|---|---|---|---|
| `P-PAY-02` | a complete set redeems for exactly `q` at every admissible payout vector | required | optional | **primary (proved)** | fractional-weight corpus |

Adding the row is a reviewer's edit, not this lane's.

**Executor-column consequence for §7.** The `lean-checker` column of the
cross-runtime differential gate stays `pending` today, and every vector still
carries it (22 pending, 3 not-applicable). The plan's §7 designs the reader that
fills it, including the rule that until the `Error -> taxonomy code` table is
reviewed, a Lean run reports **refusal-only** rather than inventing codes — the
same hazard as finding A.4.2 of `implementation/VECTOR_SPINE_PROPOSAL.md`. The
model already evaluates two canonical kernel vectors at build time
(`lean/DragonsClutch/Vectors.lean`, `#guard`), which is an existence proof for
the column, not the column.

**What this addendum did not claim when written.** At the time of the Lean wave,
no implementation refinement existed. The later narrow Verus result below
supersedes only that sentence. Vector agreement, when the column exists, will
still be agreement on the facts a vector names and nothing else. The Lean
theorems bound the mathematics; the vectors bound their broader correspondence;
nothing here bounds the adapter or runtime.

### Addendum, 2026-08-18 — first production-bound Verus subset

Status: **CHECKED for the named arithmetic contract; REVIEWED for its
digest-bound call seam; OPEN everywhere else.**

`verus/kernel/run_transfer_refinement.sh` mechanically instruments the exact
production source of `prepare_internal_transfer`, rather than checking a
hand-written semantic shadow. Under `quantity <= from`, its postcondition proves
equal-and-opposite sender/receiver deltas, mathematical-sum conservation, and
the precise receiver-overflow alternative. Underflow and the defensive
conservation refusal are unreachable. Changing receiver addition to subtraction
or inverting the conservation guard makes the same postcondition fail.

This partially discharges only the local arithmetic slice of `P-SUP-01`. The
`MarketState::transfer_internal` call/error-map/delayed-write region is pinned by
digest and manually reviewed; it is not itself a checked Verus contract. No
claim is made about semantic owner identity, phase, whole-state refinement,
canonical vectors, accounts, SBF, or Solana. Consequently the vector spine's
`verus-host` dispositions stay `pending`; this helper is not a vector executor.

The evidence record is `verus/kernel/TRANSFER_REFINEMENT.json`, the assumptions
are `verus/kernel/TRANSFER_ASSUMPTIONS.md`, and the reproduction command is:

```sh
sh verus/kernel/run_transfer_refinement.sh
```

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
