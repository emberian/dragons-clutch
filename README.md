# dClutch

dClutch is a Solana protocol for fully collateralized, liquidation-free claims
over bounded objective states. A market partitions an objective outcome domain
into an exhaustive, disjoint, canonical set of cells; claims over those cells
are minted only against collateral already segregated in the market's Hoard;
resolution consumes an authenticated, release-bound source observation, never a
discretionary resolver. Digitals, ranges, and tail protection compile exactly
onto the categorical basis. There is no leverage to unwind, so there is no
liquidation, no margin call, and no path where a claim is worth more than the
collateral standing behind it.

It is an architectural restart informed by Dragon's Clutch (the neighboring
`dragons-clutch` repository is compost: studied for requirements, invariants,
and counterexamples, never copied wholesale). The successor keeps the
mathematics — canonical state partitions, exact claim bases, complete-set
accounting, checked clearing, funded resolution — on a small Market Core with
immutable capability children, in place of the predecessor's universal account
graph.

## Status, 2026-08-27

Every claim below is **local execution evidence**: in-process ProgramTest banks
and a local Agave validator. There is no deployment on any cluster, no release,
no official frontend, no live market, and nothing value-bearing. Devnet
deployment is deferred by explicit decision and requires named authorization
before any deploy. Evidence levels are deliberately nontransitive — a local
campaign is not devnet evidence, and neither is mainnet evidence.

**The market is open.** `DCLTGMF1`, the atomic generic founding — Custody
Lock, Core Found, Custody Realize, Claims founding, Core Open **last**, five
stages in one rollback domain — executes end-to-end on a local validator:
1,189,823 CU against Solana's 1,400,000 ceiling, reproduced across runs, with
the whole-chain rollback hostile case green and the tier-1 gauntlet's
witnesses checked. Eight founding blockers were found and killed by execution
across six campaign runs. Checked-in compute budgets
([`tools/gauntlet/CU_BUDGETS.json`](tools/gauntlet/CU_BUDGETS.json)) now watch
every golden transaction, because this transaction's cost moved 84.6% → 91.3%
of the ceiling in one evening while nothing was watching.

**Trading executes.** The canonical Direct continuation runs to completion on
the shipped ELFs at the real 32,768-byte heap: the trading gate is 15/15
across three consecutive runs. The honest wall that remains is compute — the
shipped path spends 1,336,865–1,386,359 CU depending on PDA bump-search depth,
and one fixture seed in twenty exceeds the ceiling outright. That is a
protocol cost with a recorded fix direction (store each canonical bump in the
record it belongs to), not measurement noise.

**The census counts everything.** Thirteen on-chain programs expose 98
instruction routes and declare 198 protocol refusal codes (248 across the
whole tree counting test-only caller programs, in 24 registered bands).
[`dclutch-route-census`](tools/gauntlet/census) enumerates routes and refusal
enums from source; every custom error code is namespaced per program (decision
0007; `band = code >> 12`, band 0 never allocated, so a code below `0x1000` is
not ours); [`tools/gauntlet/blocked.json`](tools/gauntlet/blocked.json)
carries every route deliberately not yet executable, with reasons. The
standing doctrine: a route ships with the campaign row that executes it, or
ships marked never-executed.

**The frontend reads the real chain.** [`apps/dclutch-web`](apps/dclutch-web)
has Wallet Standard discovery, `/markets`, `/markets/:address`, and an
indexer-free `/portfolio`, with a 200+-case test suite, and has read the first
live open Market on a resumed campaign chain
([evidence](docs/evidence/FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md)). Its
decoders are generated, not hand-mirrored: eleven ABI surfaces each carry a
byte-compare `abi:*:verify` gate against the emitting authority, and the
shared decoders enforce the same grammars the chain enforces — the browser
refuses what the chain refuses.

**Lean authors the semantics.** [`formal/dclutch-semantics`](formal/dclutch-semantics)
authors record layouts, wire ABIs, and the V3 transition programs; its
emitters produce the generated Rust and TypeScript modules, each gated on byte
identity with the checked-in output. Today's assurance evidence is per-case
corpora and emitter checks — they prove the cases they contain and nothing
else. Universal refinement theorems are real debt, parked by explicit
decision, and are not claimed.

Not yet true, and said plainly: Structured V2 is in implementation; the
General and Dealer families have artifacts and activation paths but not yet
their first hot executions; the market discovery index does not exist; and
roughly half the census's routes still await their executing campaign row.

## The seven programs

Five fixed execution roles (decision 0003) plus two infrastructure programs.
A Market names its capability program set immutably at founding.

| Role | Program | Owns |
|---|---|---|
| Registry | [`programs/dclutch-registry-sbf`](programs/dclutch-registry-sbf) | release-set activation cache; deployment reauthentication; sole writer of activation state |
| Core | [`programs/dclutch-core-sbf`](programs/dclutch-core-sbf) | canonical Market truth: founding, permits, phase, Open |
| Claims | [`programs/dclutch-claims-sbf`](programs/dclutch-claims-sbf) | the one claims economic owner: liabilities, complete sets, settlement |
| Trading | [`programs/dclutch-trading-sbf`](programs/dclutch-trading-sbf) | the fixed execution role: data-driven Direct / General / Dealer paths |
| Resolution | [`programs/dclutch-resolution-proof-sbf`](programs/dclutch-resolution-proof-sbf) | source resolution controller: terminal windows, funded failure walk |
| Custody | [`programs/dclutch-custody-sbf`](programs/dclutch-custody-sbf) | collateral custody: Hoard vault, Token-2022 boundary |
| Rent | [`programs/dclutch-rent-sbf`](programs/dclutch-rent-sbf) | lifecycle RentCredit accounts |

The remaining programs under [`programs/`](programs) are accelerators and
test shadows, registered in the same refusal-band and census regime.

## Repository map

- [`crates/`](crates) — SDK-free contracts (byte layouts, PDA seeds, exact
  arithmetic), `no_std` kernels, codecs, operators. One semantic owner per
  persisted fact; adapters authenticate boundaries and may not recreate kernel
  economics.
- [`programs/`](programs) — the SBF adapters listed above.
- [`formal/`](formal) — the Lean semantics, emitters, and qedsvm proof lanes.
- [`tools/gauntlet/`](tools/gauntlet) — build → deploy (local validator) →
  campaign → census; the route census; CU budgets; family campaign tiers.
- [`apps/dclutch-web`](apps/dclutch-web) — the browser frontend.
- [`docs/decisions/`](docs/decisions) — architecture decision records.
- [`docs/reference/`](docs/reference) — the generated protocol reference:
  programs, routes and their execution status, refusal codes with meanings,
  compute budgets, ADR index, ABI tables. Regenerate with
  `tools/genref/generate.sh`; `--check` byte-compares.
- [`docs/guides/`](docs/guides) — thin hand-written guides (trader,
  operator, reader) that link into the generated reference.
- [`docs/evidence/`](docs/evidence) — dated execution evidence.
- [`docs/OMISSION_INDEX.md`](docs/OMISSION_INDEX.md) — the challenge ledger:
  what the successor deliberately does not do yet, and what would reopen each
  row.

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# static route census + report, seconds, no chain:
tools/gauntlet/run.sh --mode census

# the full tier-1 campaign: builds the seven ELFs, boots a local validator,
# founds and opens a Market, checks witnesses and CU budgets (~13 minutes):
tools/gauntlet/run.sh --mode full

# frontend suite, including every generated-ABI byte-compare gate:
cd apps/dclutch-web && npm test
```

## Reading order

[`WAVE.md`](WAVE.md) (living swarm state — current cycle, active lanes,
doctrine), then [`AGENTS.md`](AGENTS.md) (authority, safety, and correctness
vocabulary), then [`ARCHITECTURE.md`](ARCHITECTURE.md) (the architectural
baseline; its narrative predates the current one-Market-truth ruling in
places), [`PROJECT_METHOD.md`](PROJECT_METHOD.md), and
[`COMPOST.md`](COMPOST.md) before adding a subsystem.

Direction, so the shape of the work is legible: the near-term goal is the
completed protocol live on devnet, resolving markets about the state of
Solana mainnet — with a disclosed proof-of-authority relayer attesting raw
observed bytes as the honest v1 cost of cross-cluster truth, and Pyth's
devnet feeds needing no relayer at all. Nothing about that goal changes the
status section above until it happens, and it will be labeled at the evidence
level it actually reaches.
