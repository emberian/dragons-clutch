# genref — the generated protocol reference

`docs/reference/` is emitted by this tool from the authorities that run the
protocol. Nothing under `docs/reference/` is edited by hand; hand-written
prose exists only as the connective narrative inside `generate.mjs`'s
templates.

```sh
tools/gate reference                 # regenerate docs/reference/
tools/gate reference --check         # byte-compare, write nothing, exit 1 on drift
tools/gate reference --converge      # regenerate the reference AND the client mirrors
                                     #   that read it, to a fixpoint. THE ONE TO USE.
tools/gate reference --check --converge
                                     # is the COMMITTED revision already that fixpoint?
tools/gate selftest                  # the loop's own refusal tests, seconds, no build
```

`tools/genref/generate.sh` is a shim to `tools/gate reference` (the generated
banners and `tools/release/final-generated-convergence.py` spell it).

It refuses a dirty tree. `--allow-dirty` and `GENREF_ALLOW_DIRTY=1` are the same
escape spelled two ways; the driver consumes both and `generate.mjs` sees neither.

## Sources

| page | authority |
|---|---|
| `programs.md`, `routes.md`, `refusals.md` (code tables) | `dclutch-route-census inventory` (tools/gauntlet/census), run fresh from the tree with `--check-unique` |
| `routes.md` (execution status) | `tools/gauntlet/*/bindings.json` (+ `*-bindings.json`) and `tools/gauntlet/blocked.json` |
| `route-witnesses.md` | the same bindings, plus `tools/gauntlet/substrates.json` (which substrate each campaign ran on, CHECKED here against its runner) and `docs/evidence/witnesses/*.json` (devnet transactions, whose chain-derived fields are written by `tools/gauntlet/devnet-witness/corroborate.py`) |
| `refusals.md` (band allocation) | the census inventory's `bands`, read from `crates/dclutch-refusal-registry/src/generated_bands.rs` |
| `budgets.md` | `tools/gauntlet/CU_BUDGETS.json` |
| `decisions.md` | `docs/decisions/*.md` |
| `abi/*.md` | `apps/dclutch-web/lib/generated/*.ts` — themselves emitted, each byte-gated by its own `abi:*:verify` |

## Convergence: use `--converge`, not two passes by hand

The reference and its client mirrors close a **cycle**:

```
docs/reference/refusals.md  ->  {web,sdk}/lib/generated/refusalRegistryV1.ts
                            ->  docs/reference/abi/refusalRegistryV1.md
```

`generate.mjs` on its own is a pure function of sources it never writes, so
running it twice is byte-identical and its fixpoint is trivial. The cycle is
one layer out: `generate-refusal-registry.mjs` takes its per-code names and
meanings **from** `refusals.md`, and this generator mirrors the module that
script emits back **into** the reference. So a newly landed refusal code reaches
`refusals.md` on pass one and `abi/refusalRegistryV1.md` only on pass two. The
same is true of any other reference-coupled emitter -- `generate-route-census.mjs`
emits a module this page mirrors, so moving it also costs a second pass.

That rule was learned by hand, written into a commit message, and then had to be
re-learned by whoever read the reference next. `--converge` is the rule made
mechanical: it runs `generate.mjs` and then every reference-coupled emitter,
repeating until a pass moves nothing, **bounded at three** -- two productive
passes and one that proves the fixpoint. A third pass that still moves a file is
**refused** (exit 4), never accepted as close enough.

| exit | meaning |
|---|---|
| 0 | a fixpoint, and nothing moved -- the tree was already converged |
| 1 | a fixpoint, reached by writing -- commit what it wrote |
| 3 | the working tree is dirty (as for every other mode) |
| 4 | no fixpoint within three passes -- refused, moving files named |

The list of reference-coupled emitters is **declared** in `tools/gates/reference.py` and then
**checked against discovery** on every run: a script under
`apps/dclutch-web/scripts` or `packages/dclutch-sdk/scripts` that reads
`docs/reference` and is not in the list fails the run by name. A hardcoded list
that silently missed a new emitter would reproduce the exact defect `--converge`
removes.

`--converge --check` answers the same question about a **committed** revision:
it archives it (`git archive`, so it touches no repository state and cannot
contend on `.git` locks) and settles it there, writing nothing into this
checkout. That is the form the `reference` tier runs, because convergence is
a property of what a commit holds and this checkout is a dozen lanes'
half-written files.

## Discipline

- **This generator runs BEFORE the `abi:*` TypeScript ones, not after.**
  `apps/dclutch-web/scripts/generate-refusal-registry.mjs` reads its band table
  from the registry crate and its codes from `refusals.md`, so a band removed
  from the crate while this page still lists its codes makes that script throw
  "sits in no registered band" (found deleting band 0x7 on 2026-09-02); and
  because `abi/*.md` here mirrors those emitted `.ts` modules, correcting one of
  them takes two genref passes. `--converge` does both in the right order; run
  it rather than sequencing them by hand.
- The generator owns the whole directory: `--check` fails on stale, missing,
  **or stray** files; regeneration removes strays. The directory cannot drift
  or grow by hand.
- Writes are temp-file + atomic rename in the destination directory
  (AGENTS.md: a failed generator leaves the last accepted output
  byte-for-byte intact).
- Deterministic by construction: no timestamps, no absolute paths, every
  collection sorted. Two consecutive runs are byte-identical; the check gate
  is the proof.
- Anything the ABI renderer does not recognize is carried verbatim into an
  "unrendered exports" section, never dropped — the reference must not
  silently narrow its source.
- Route status is coarse and honest: "witnessed" means an in-tree campaign
  binding names the route (campaign coverage, not a proof about all inputs);
  "blocked" quotes `blocked.json`'s rule; a route with neither is printed as
  NEVER-EXECUTED with no stated reason, which is the row that should make
  someone uncomfortable. Binding refs that match no census route id get their
  own table instead of being dropped.
