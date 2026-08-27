# The gauntlet: a standing outside-in functional suite

The gauntlet exists because dClutch has proven, repeatedly and expensively, that
a large unit/fixture suite can be green while the protocol cannot execute at
all.

## The evidence this was built from

All of the following were true simultaneously on 2026-08-26, with roughly 2,300
unit and fixture tests passing:

| Defect | Why the suite could not see it |
|---|---|
| `GENERAL_CANDIDATE_PAGE_PDA_DOMAIN_V1` was 33 bytes over Solana's 32-byte seed cap | Nothing ever derived the address. The constant was only ever compared to itself. |
| `ProjectedCustodyCallerSeedsV1`'s domain was 35 bytes — **no projected-Custody transition could ever have signed** | Same: the seed set was never handed to `create_program_address` on a validator. |
| `plan_lifecycle_with_protected_outputs_atomic` refused the real all-zero System Program | Every fixture in `lifecycle_v3.rs` substituted a made-up non-zero `SYSTEM` constant. The identity guard and the fixtures agreed with each other and disagreed with Solana. |
| Found31 serialised to 1,242 bytes against the 1,232-byte legacy limit | The transaction was never submitted; it was only ever constructed and inspected. |
| `cc228cd` silently broke *every* Profile14 emission for days | The producers were adapted to satisfy the validator; the validator and the producers were both authored by the same lane, so they re-agreed. |
| Registry activation with the real seven artifacts exceeded 1.4M CU outright | Every campaign substituted four roles with the small Registry ELF. |
| A whole Profile14 refusal chain (six consecutive refusals) sat behind a 32KB heap wall | The wall aborted at phase 4 of 10. Phases 5-10 had never run, so the six refusals behind it were invisible. |

Every one of these is the same failure: **a mirror test**. A fixture authored by
the code under test, verifying a modeled world, agreeing with itself.

The gauntlet is the anti-mirror. It is not a replacement for the unit suite; it
is the thing that makes the unit suite's greenness mean something.

## Principle 1 — Outside-in only

Every interaction is a real transaction, built by the chain-derived operators
from hostilely decoded RPC state, exactly as a user's client would build it, and
submitted to a real validator running real ELFs at real limits.

Concretely, and non-negotiably:

- **Real packets.** 1,232-byte legacy / 1,224-byte continuation limits are
  actually hit by actually serialising and actually submitting. A frame that
  does not fit is a failure, not a diagnostic.
- **Real compute.** `COMPUTE_LIMIT` is 1,400,000 because that is Solana's
  per-transaction maximum. A run that raises it to measure something is a
  *measurement* and is labeled as such; it never satisfies a gate.
- **Real heap.** 32,768 bytes. A diagnostic heap frame is a measurement, never
  a gate.
- **Real rent, real Agave account shapes.** The System Program is the all-zero
  address with 14 bytes of NativeLoader metadata. Token-2022 mints carry
  extensions. ProgramData carries the 45-byte Loader-v3 span with the
  authority-tag byte at offset 12 and inactive retained bytes after a revoke.
- **No genesis-injected protocol state** beyond what the transaction-only
  bootstrap legitimately deploys. Loader accounts and the infrastructure release
  records needed to *start* are genesis; the infrastructure profile, the
  activation cache, every record, every Market is created by a submitted
  transaction.
- **No native processors, no mock programs.** The ELFs under test are the ELFs
  `cargo build-sbf` produced from the pinned commit, and their SHA-256 digests
  are recorded in the run evidence.

`solana-program-test` is admissible only as a labeled fast lane, only for tiers
whose semantics it reproduces identically, and never as the sole evidence for a
route. See `TIERS.md`.

## Principle 2 — Witnesses, not mirrors

An expected value is a **witness** only if its provenance is independent of the
code under test. Admissible sources, in descending preference:

1. **A Lean-emitted vector.** The generator is a `formal/dclutch-semantics`
   emitter; the value is checked byte-for-byte by the crate's own
   `check-generated.sh`.
2. **A hand-stated constant with a provenance comment** naming where the number
   comes from: a Solana runtime limit, an RFC, an SPL layout, a measured
   validator observation with the date and validator version.
3. **A cross-check against a second implementation** — e.g. the browser decoder
   and the on-chain contract refusing the same byte string.

Inadmissible, always:

- Reading a value back out of the code under test and asserting it equals
  itself.
- A fixture constructed by the emitter whose output is being verified.
- `assert!(result.is_ok())` with no statement of what the result *is*.
- A vacuous implication (`P → P`) dressed as a refinement.

The bar is the DP lane's asserted-witness pattern from commit `52f14fa`: the
test states the exact expected bytes, and the test fails if the emitter changes
its mind.

`tier1/witnesses.json` is where tier-1's witnesses live, each with a
`provenance` field. A witness without provenance is rejected by the harness.

## Principle 3 — Every refusal is a submitted transaction

A documented hostile case is not a hostile case until it has been submitted.

Each one executes as a real failing transaction and asserts three things:

1. **The named error.** Not "it failed" — the exact program error, matched
   against the census's enumerated refusal code for that program.
2. **Byte-exact account rollback.** Fetch the pre-state, submit, fetch the
   post-state, compare bytes. Where the transaction carries a deliberate
   earlier instruction (a transfer, a create), that instruction's effect must be
   gone and the only delta on the fee payer must be exactly the transaction fee.
3. **That the refusal came from the program we think it came from.** The
   finalized transaction's log messages must show that program invoked. A
   transaction that refused in the runtime before reaching the program proves
   nothing about the program's refusal.

"It refused" without (2) is a partial result and is recorded as such.

## Principle 4 — The execution census

This is the novel piece, and it is the reason the gauntlet is not just another
integration test.

**The problem it solves:** a route that is never executed produces *silence*.
Silence is indistinguishable from success in every test report ever written.
Found31 was never submitted for months and no report anywhere said so.

**The mechanism:**

- `census inventory` statically enumerates every public entry of every program
  in `programs/`: each entrypoint, each dispatch branch and the wire
  discriminant that selects it (instruction magic bytes, exact-length
  discriminators, action tags), and each program's full refusal-code enum with
  variant names and numeric values. Every entry carries `file:line` provenance.
  The enumeration is derived from the Rust AST (`syn`), not from a hand-kept
  list, so it cannot drift silently from the source.
- `census observe` folds a campaign's **chain evidence** — the finalized
  transaction records the campaign emitted, including their log messages — into
  an append-only ledger of what was actually driven on a validator.
- `census report` renders EXECUTED / NEVER-EXECUTED per route and per refusal
  code.

**The honesty rules, which matter more than the mechanism:**

- A route counts as EXECUTED only when a **finalized transaction** in the
  evidence names it *and* the chain's own log messages show that program
  invoked. The harness's belief about what it submitted is cross-checked
  against what the chain says ran.
- A campaign transaction label with no binding to a census route is a **hard
  error**, not a skip. Unbound labels are how coverage silently rots.
- A route that cannot be driven today — anything behind the not-yet-open Market,
  family actions behind an in-flight lane — is listed **NEVER-EXECUTED with the
  blocking reason and the owning lane**, in `blocked.json`. It is never listed
  as anything else, never excluded from the denominator, and never suppressed.
- There is no "expected coverage" threshold and no percentage to game. The
  report prints the routes.

The honest number will be ugly. That is the point. An ugly number that is true
is worth more than 2,300 green tests that agreed with themselves.

## Principle 5 — One command, arbitrarily resumable

`tools/gauntlet/run.sh` drives build → deploy (transaction-only, local
validator) → campaign → census report.

- **Resumable at any stage.** Each stage writes a stamp under the work root
  keyed by its exact inputs (source revision, ELF digests, spec digest). A
  re-run skips a stage whose stamp matches and re-runs everything downstream of
  the first stage that does not. `--from <stage>` forces a restart at a stage.
- **Nothing lands in the repo.** The work root defaults to
  `/private/tmp/dclutch-gauntlet`; `CARGO_TARGET_DIR` lives there too. The
  shared checkout's `target/` is never used, because parallel lanes share this
  working tree.
- **Builds from a pinned commit**, via `git archive`, following
  `tools/release/checked-release-candidate.sh`. A dirty shared tree cannot
  silently change what was tested.
- **hbox-safe.** Every build runs through `swarm-build` when it is on PATH.
  hbox is co-tenant with codex; the memory cap is structural, not polite.
- **Never an unfiltered `-p <crate>` suite.** The gauntlet runs the campaign and
  the census. It does not run the unit suite.

## What the gauntlet does not claim

- Local-validator execution is **local-validator evidence**. It is not devnet
  evidence and it is emphatically not mainnet evidence. `AGENTS.md` names these
  as distinct levels and the gauntlet keeps the boundary machine-readable.
- A green gauntlet is not verification. Nothing here discharges a theorem. It
  establishes that the named routes executed on a real validator at real limits
  and that the named refusals refused.
- The census's denominator is the *statically enumerable* public surface. A
  route reachable only through a dispatch shape the enumerator does not
  recognise is reported as `UNCLASSIFIED` — visibly, in the report — rather than
  dropped. An enumerator that silently under-counts would be the same mirror
  failure one level up.

## Ownership and boundaries

The gauntlet owns `tools/gauntlet/**` and nothing else. It is **read-only**
toward every protocol source, toward `tools/local-validator/**` (the
transaction-only bootstrap it drives as a subprocess), and toward
`apps/dclutch-web` and `formal/`.

That is deliberate: a suite that can edit the thing it tests is one refactor
away from being a mirror again.

## Known compromises, stated rather than hidden

There is exactly one, and it is recorded in the run directory every time it is
taken.

**The Pyth fixture pin gate.** At the revision this lane built,
`tools/local-validator/fixture-sha256.txt` does not verify against the committed
fixtures: `30bfc71` rewrote `PROVENANCE.md` and added
`guardian-set-0.account.hex` without regenerating the pin list, and the verifier
additionally hardcodes an expected artifact count of ten. Because
`dclutch-successor-validator start` runs that gate as its very first statement,
**the entire local-validator campaign has been unreachable by its own
one-command path since that commit** — which is itself a finding of exactly the
kind this suite exists to produce.

`tools/gauntlet/tier1/launcher.sh` prefers the committed launcher and `exec`s it
unchanged whenever the pins verify. Only under an explicit, off-by-default
`--allow-stale-fixture-pins` does it take a second path: it copies both launcher
scripts verbatim, regenerates the pin list from the `git archive` snapshot,
relaxes the hardcoded count, and writes `FIXTURE_PIN_OVERRIDE.md` beside the
ledger naming the exact drift, the causing commit, and the owning lane. Every
other check — attestations, plan, account directory, validator version, the
exact validator argument vector — runs unmodified from the committed code.

What the override gives up is a hand-maintained hash list over vendored Pyth
artifacts. What replaces it is stronger: the gauntlet builds and runs from
`git archive <revision>`, and every artifact attestation records
`archive_sha256`, the SHA-256 of the complete `git ls-tree -r --full-tree`
listing at that revision. The fixtures are inside that digest.

The override path is meant to die. It goes dead the moment the pin file is
regenerated at its owner, and it should be deleted then.

## What the first run found

Before the tier-1 campaign completed a single Market, the harness had already
surfaced three things no test in the tree reported:

1. The fixture-pin drift above — the campaign's own launcher could not start.
2. Two SBF stack-frame-overwrite diagnostics in
   `dclutch_trading_sbf::projected_custody_bootstrap_v1::authenticate_and_project`,
   which `cargo build-sbf` reports and then exits zero on, so nothing downstream
   sees them. The six other role artifacts emit none.
3. A route census in which the honest EXECUTED count started at zero out of two
   hundred and forty.

None of these required a clever test. They required running the thing.
