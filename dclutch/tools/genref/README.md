# genref — the generated protocol reference

`docs/reference/` is emitted by this tool from the authorities that run the
protocol. Nothing under `docs/reference/` is edited by hand; hand-written
prose exists only as the connective narrative inside `generate.mjs`'s
templates.

```sh
tools/genref/generate.sh            # regenerate docs/reference/
tools/genref/generate.sh --check    # byte-compare, write nothing, exit 1 on drift
tools/genref/test.sh                # self-test the dirty-tree gate, seconds, no build
```

It refuses a dirty tree. `--allow-dirty` and `GENREF_ALLOW_DIRTY=1` are the same
escape spelled two ways, `generate.sh` is its only author, and neither spelling
reaches `generate.mjs` -- which is what `test.sh` pins.

## Sources

| page | authority |
|---|---|
| `programs.md`, `routes.md`, `refusals.md` (code tables) | `dclutch-route-census inventory` (tools/gauntlet/census), run fresh from the tree with `--check-unique` |
| `routes.md` (execution status) | `tools/gauntlet/*/bindings.json` (+ `*-bindings.json`) and `tools/gauntlet/blocked.json` |
| `route-witnesses.md` | the same bindings, plus `tools/gauntlet/substrates.json` (which substrate each campaign ran on, CHECKED here against its runner) and `docs/evidence/witnesses/*.json` (devnet transactions, whose chain-derived fields are written by `tools/gauntlet/devnet-witness/corroborate.py`) |
| `refusals.md` (band allocation) | `crates/dclutch-refusal-registry/src/lib.rs` |
| `budgets.md` | `tools/gauntlet/CU_BUDGETS.json` |
| `decisions.md` | `docs/decisions/*.md` |
| `abi/*.md` | `apps/dclutch-web/lib/generated/*.ts` — themselves emitted, each byte-gated by its own `abi:*:verify` |

## Discipline

- **This generator runs BEFORE the `abi:*` TypeScript ones, not after.**
  `apps/dclutch-web/scripts/generate-refusal-registry.mjs` reads its band table
  from the registry crate and its codes from `refusals.md`, so a band removed
  from the crate while this page still lists its codes makes that script throw
  "sits in no registered band" (found deleting band 0x7 on 2026-09-02); and
  because `abi/*.md` here mirrors those emitted `.ts` modules, correcting one of
  them takes two genref passes with a commit between.
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
