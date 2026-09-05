# tools/gate — the instruments that decide what is true

One entry point, one tier table, one verdict vocabulary, one clean-revision
export. `tools/gate --list` prints every gate with its measured cost, what it
needs, and what it refuses; that table is the only copy, and this file does not
restate it.

```sh
tools/gate cheap                     # the push tiers, ~2 min
tools/gate all --require             # the cut: an unrun gate is a failed one
tools/gate frames --commit HEAD      # any gate, on a clean export of a revision
tools/gate emission --run --all      # an instrument on its own terms
tools/gate <instrument> --help
```

Exit 0 passed, 1 the tree has the defect, 2 a prerequisite was missing and
nothing was proven, 64 usage. `--require` turns 2 into 1. `--commit REV`
measures a clean `git archive` of REV wherever a gate compiles, because on a
shared checkout a working-tree build measures a revision nobody committed.
`--dry-run` prints each gate's commands.

## What each instrument refuses

**census** — `dclutch-route-census inventory --check-unique` over one parse of
every first-party source: a refusal code outside its registered band or claimed
twice; an eight-byte magic claimed by two names without an adjudicated entry in
`tools/gauntlet/magic-collisions.json`; a schema identity that is not the
SHA-256 of the label it documents; a program directory absent from the census
target list. `observe` folds a campaign's chain evidence into the shared ledger
under its lock, refusing any claimed route the finalized logs do not show
invoked. Writes `<work>/out/{inventory.json,CENSUS.md}` (default
`/private/tmp/dclutch-gauntlet`, the gauntlet's own).

**emission** — a generated file (first line names a Lean emitter) with no guard
that re-runs that emitter and compares (`emission-coverage.md` is the committed
census; `--verify` byte-gates it, `--write` regenerates it); a raw-comparing
guard over a Rust emission that rustfmt would reflow, one `lane.sh fmt` from
red (`fixpoint-debt.tsv` is that ratchet); a two-sided wire vector whose sha256
differs from the pin a human typed in `wire-vector-pins.tsv`. `--run --all` (the
`guards` tier) runs every guard for real and needs `lake`.

**frames** — a function in any of the twelve SBF links whose exact frame
multiset differs from `frames-baseline.json` in either direction (shrinkage is
red until the ratchet is lowered); a link that did not freshly compile; any
`overwrites values in the frame` diagnostic; a capture from a dirty tree with no
`--at`; two captures naming different commits. Red prints `owed`: the commits
since the baseline's commit that changed sources in a link's path-dependency
closure and carried no rows, each with its `Lane:` trailer.

**reference** — `docs/reference/` and the client mirrors emitted from it not at
their fixpoint at the measured commit; a generator pair still moving a file on
the third pass; an emitter reading `docs/reference` that is not declared
reference-coupled; a dirty tree, for a regeneration. `tools/genref/generate.mjs`
renders; this drives it.

**witness** — a devnet route-witness document (`docs/evidence/witnesses/*.json`)
whose signature is not finalized, whose outer magic is not the declared one sent
to the declared program, or whose claimed route's program the chain's own
`invoke` lines do not show. Reads devnet through `~/.helius-key`; bounded to
the documents' signatures.

**budgets** — `tools/gauntlet/CU_BUDGETS.json`: an enforced budget that is not
`measured + tolerance`, above the 1,400,000 ceiling, of a scope that is neither
`transaction` nor `stage`, or naming a campaign no bindings file or
`substrates.json` row knows; an unenforced row with no reason; a duplicated id.
Evaluation against evidence stays in `tools/gauntlet/tier1/check-witnesses.sh`,
the one evaluator the campaign runners call.

**commands** — a command a runbook (`README.md`, `docs/guides`,
`docs/operators`) publishes whose program is absent, whose subcommand or long
flags its own `--help` does not name, or which omits an argument its usage line
marks required. Runs `--help` and nothing else, only on programs whose source
handles it; an unprobed command is 2, never a pass.

**twins** — a web/SDK twin pair diverging from the class
`tools/twins/classification.mjs` gives it: a TWIN that differs, an exemption that
became identical, a REEXPORT that grew a body, a SHIM that adds nothing, a
WEB-ONLY file the package has a copy of.

**selftest** — the gates' own refusal tests (this directory's `tests/`,
`tools/lane/test.sh`, `tools/gauntlet/test-run-cli.sh`,
`tools/seam-audit/test-seam-audit.py`), so a gate that cannot fail is found
before it is trusted.

**lane** — `tools/lane.sh` (commit, commit-patch, fmt, board), reachable here.
**archive** — `tools/gate archive REV DIR`, the one clean-export helper for
runners outside this package.

## The tiers that are not instruments

`fmt`, `locks`, `seam`, `release`, `clippy`, `sbom`, `sbfcontracts`, `web`,
`abi`, `journey`, `root-targets`, `programs`, `suites`, `workspaces` each run a
tool another directory owns (`tools/seam-audit`, `tools/sbom`, `tools/release`,
the program-test runners) or a cargo/npm invocation, against a register kept
here: `fmt-baseline.txt` (files still owed a format, and by whom),
`clippy-debt.tsv` (packages red today that must stay red until their row is
deleted), `root-targets.tsv` (every cheap root-workspace integration test with
its measured seconds and whether it runs, is quarantined red, or is excluded as
slow). Every register is a ratchet in both directions: a new entry and a stale
entry both fail.

## The wrapper

The public repository's workflows call `tools/ci/run.sh <tier>`, which is a shim
onto this entry point with the old tier names mapped (`census`→`emission`,
`emission`→`guards`, `frameguard`→`frames`, `genref`→`reference`,
`runbooks`→`commands`). Delete the shim once the workflows call `tools/gate`.
