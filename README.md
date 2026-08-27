# Dragon's Clutch

Dragon's Clutch is a Solana protocol project for fully collateralized,
liquidation-free claims over bounded objective states. A market partitions an
objective outcome domain into an exhaustive, disjoint, canonical set of cells;
claims over those cells are minted only against collateral already segregated
for that market; resolution consumes authenticated, release-bound source
evidence, never a discretionary resolver. There is no leverage to unwind, so
there is no liquidation and no path where a claim outgrows the collateral
standing behind it.

Active development is **dClutch**, the successor protocol, vendored here as
the [`dclutch/`](dclutch/) subtree and updated in waves from its live working
tree. Start at [`dclutch/README.md`](dclutch/README.md) for the current
protocol, its seven programs, its route census, and its execution evidence.
The successor keeps this project's mathematics — canonical state partitions,
exact claim bases, complete-set accounting, checked clearing, funded
resolution — on a small Market Core with immutable capability children, in
place of the first generation's universal account graph.

## Status

Nothing in this repository is released. There is no deployed program on any
cluster, no official frontend, no production source identity, no live market,
and no value-bearing anything. The successor's evidence is local execution —
in-process SVM banks and local Agave validators — and is labeled at exactly
that level; the first generation's evidence below is likewise local. Evidence
levels are deliberately nontransitive: a local campaign is not devnet
evidence, and neither is mainnet evidence.

The microsite under [`site/`](site/) is published to GitHub Pages by manual
dispatch only; a push to `main` is not publication authority.

## Layout

- [`dclutch/`](dclutch/) — the successor protocol (the live work; everything
  else here is context for it).
- [`programs/`](programs/), [`crates/`](crates/), [`apps/`](apps/),
  [`lean/`](lean/), [`verus/`](verus/), [`rocq/`](rocq/),
  [`research/`](research/) — the retained first-generation implementation and
  its formal lanes.
- [`docs/`](docs/) — first-generation architecture, protocol, verification,
  and review documents, including the retained campaign evidence.
- [`site/`](site/) — the hand-authored static microsite.

## Generation 1: the retained implementation

The first-generation implementation ("Clutch") remains in this repository as a
working archive: it is compost for the successor — studied for requirements,
invariants, counterexamples, and measurements — and its retained evidence is
still the project's deepest end-to-end lifecycle record. It is not the active
codebase, and defects found in it are fixed in the successor, not here.

What it demonstrated, all local and unpromoted:

- **A joined source-to-redemption lifecycle.** The retained 2026-08-23
  joined-v4 campaign records 52 signed transactions on a loopback Agave
  validator: the captured deployed Pyth router and receiver verify a
  deterministic locally signed 13-of-19 observation, hostile append attempts
  roll back, the admitted update seals, two funded owners trade one direct
  outcome pair, and the same market resolves, redeems, and returns all 128
  collateral atoms. Evidence:
  [`docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-23/`](docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-23/).
- **Reproducible artifacts under audit.** Multiple independent builds of the
  frozen source closures produced byte-identical SBF ELFs with
  dependency/syscall and final-LTO stack checks green; the accepted Cycle-G
  manifest carries 101 declared gates. Reproducibility is not a seal, release,
  or deployment.
- **Exact settlement mathematics.** Categorical and degree-1–3 B-spline claim
  bases, coupled portfolio clearing at exact integer simplex prices,
  streamed candidate verification, and conservation identities closed over
  complete local campaigns.

Its honest limits are recorded where they were found:
[`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) for implementation and evidence status,
[`docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md`](docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md)
for the source-backed findings that motivated the restart, and
[`docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md`](docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md)
for the recovered requirements. The successor's
[`dclutch/COMPOST.md`](dclutch/COMPOST.md) governs what may be transplanted
and how.

To explore it locally: `python3 -m http.server 9129 --bind 127.0.0.1
--directory site` serves the microsite; the retained campaign renders with the
Operator's explicitly historical `non-production-retained-source-v2` mode; the
local campaign runners and their exact invocations are documented in
[`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) and under
[`docs/reviews/`](docs/reviews/). Everything is local and offline except the
explicitly marked Pyth devnet clone, which performs bounded read-only RPC.

Vocabulary that survives into the successor: a **Realm** is an immutable
collateral and admission namespace; a **Hoard** is segregated market
collateral whose principal pays claimants only. The first generation's
egg-themed terms (Egg, Clutch, Hatch) name its own claim objects and remain in
its documents.

## Safety and release posture

There is no signed release manifest, tag, deployment, official URL, live
market, or financial authority in this repository. Static clients and indexes
are untrusted projections of canonical program state. Security policy and
threat model: [`SECURITY.md`](SECURITY.md).

## License and provenance

First-party source and documentation are licensed under
[`AGPL-3.0-or-later`](LICENSE). The project is greenfield: it must not import,
copy, or depend on JOSHI, joshibot, leanuweave, minidregg, breadstuffs, Oracle
Pit, or historical DREGG prototypes without an explicit provenance and license
review. See [`docs/PROVENANCE.md`](docs/PROVENANCE.md).
