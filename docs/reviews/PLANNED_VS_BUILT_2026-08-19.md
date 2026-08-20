# Planned versus built — scorecard, 2026-08-19

Status: **ASSESSMENT.** Scores the original planning corpus (commit
`7149114`, 127 files, 22,019 lines — everything in `docs/implementation/`
and `docs/design/` postdates it and is implementation record, not plan)
against the tree at `2d530d2`. Verdicts are cited to code. It promotes
nothing.

## Headline

Roughly **14% of originally planned commitments are in the sealed runtime**.
`docs/V1_BACKLOG.md`'s own checkboxes say 68/128 (53%), and that number is
inflated: gates 0 and 1 are bookkeeping (18 boxes), and several boxes close
on *a model existing* rather than a runtime transition.

The semantic core is real, well-evidenced, and in places machine-checked.
Everything that would make it a product — a compiler, a book, fees, a client
that can read a chain, a source that exists — is a Python model or absent.

## Five facts that dominate the rest

1. **The merged venue is a two-order crossing engine, not an auction.**
   `direct_selection_v3/common.rs:510` requires exactly two orders, one
   page, both slots `Single`, opposite sides, different owners, the same
   outcome, equal quantity, `minimum_fill == 0`, and `max_fee_atoms == 0`
   forced. It genuinely settles — real Position balances move and seven
   transients close — which retires "nothing has ever matched." One trade
   costs roughly twelve transactions and about 7M CU.
2. **Coefficient-portfolio orders can be placed and can never clear.**
   `orders_batch.rs:796` validates `OrderSlot::Portfolio` on the legacy
   path; `:888` refuses it on the V4 branch, the only branch with a working
   clearing lifecycle. The exact-rational basis — the project's genuinely
   novel contribution — is inert *as a shape*.
3. **The sealed artifact does not contain the venue.** The attested
   manifest was emitted from `d2e1cd5`, 47 commits before the merge; zero
   of its 100 gates are Direct-V3 gates. (The reseal to `af6bb79c` is what
   addresses this.)
4. **Nothing has been deployed anywhere.** The devnet harness is built and
   dry-run green, keys are provisioned, the faucet was dry. No public
   transaction has ever been sent.
5. **The default artifact cannot accept value.** Endow refuses `0x79`
   because the compiled source registry is empty, and the only working
   provider is a mock requiring an executable account with chosen bytes —
   impossible on any public cluster.

## Per-document fractions

| Document | Fraction |
| --- | --- |
| `ARCHITECTURE.md` | ~80% (discipline honored throughout; aged best) |
| `PROTOCOL.md` | ~65% (transitions shipped; object vocabulary silently replaced) |
| `V1_BACKLOG.md` | 53% by its boxes, ~35% real |
| `PROJECT.md` §10 success walk | ~5.5 / 11 |
| `PARTITION_ALGEBRA.md` | 3/15 |
| `SIMPLEX_AUCTION.md` | 5/18 (every shipped row scoped to two orders, zero fee) |
| `ENGINEERING_PLAN.md` | ~2 of 9 stages |
| `VERIFICATION.md` | ~1.5 / 25 named properties |
| `EVIDENCE_MATRIX.md` | ~1.5 / 17 property IDs |
| `COST_MODEL.md` | 5/19 |
| `BENCHMARK_PLAN.md` | ~28% |
| `STATIC_CLIENT.md` | ~25% (offline half done, chain-facing half at zero) |
| `ECONOMICS.md` | 2/28 |
| `COMPETITIVE_POSITION.md` | 1/14 |
| `PRODUCT_THESIS.md` | 1/16 |
| `FEE_GEOMETRY.md` | 0/18 (its subject has no Rust implementation) |
| `JOSHI_EXECUTION_THESIS.md` | 0/12 (zero source occurrences) |

## Quietly superseded, never retired — ranked

1. **ADR-0003's architecture is inverted with no superseding record.** It
   designated Verus the executable gate and Rocq the independent
   mathematical shadow, and explicitly warned against Lean "becoming
   mandatory by inertia." Reality: `rocq/ClutchKernel.v` contains **zero
   theorems** (only `Definition … : Prop` obligations, one with a
   machine-checked vacuous conjunct — the manifest's own gate note says
   so), Verus covers roughly 1.5 of 11 named properties, and Lean carries
   the proof story with 184 theorems and zero `sorry`. The Lean work is
   excellent; the governance record is simply false.
2. `Template` / `Instance` / `Series` — zero code, no retirement note.
   `Series` was the only permissionless repeated-creation mechanism in the
   design.
3. The compiler pipeline vocabulary — five named stages, zero hits.
4. `ENGINEERING_PLAN.md`'s crate layout — 9 of 12 crates never existed, and
   all four planned scripts never existed.
5. `RevenuePolicy` — a named deliverable in three documents, zero hits.
6. The solver bond, replaced by sponsor-funded keeper rewards: different
   actor, different funding direction, unrecorded.
7. Venue adapters, `KeeperCredit`, plain-SPL collateral, the per-Egg fee
   control arm (the specific baseline the dispersion fee was designed to
   beat — so it has never been compared against its own benchmark).

**Introduced today and unnoticed:** the V3 merge added six persistent
account families (`DirectEpochV4` 672B, `DirectCandidateV3`,
`DirectWindowV3`, `DirectWorkBudgetV1`, `DirectReservationV2`,
`DirectBatchPolicyV3`), and none appears in the 37-row terminal inventory.
V3 closes its seven transients correctly, but the Epoch and BatchPolicy
persist with no close path and no classification. **The terminal ledger
regressed the moment the venue landed.**

## Built but never planned

The exact-rational degree-0–3 B-spline stack with Lean-proved partition of
unity (no planning document contemplates it); Direct V3's whole staged
shape; resumable `ResolutionWork`; the two-ELF mock discipline; 20 research
crates totalling ~50,000 lines of which exactly one has a consumer outside
`research/`; `lp-mapping-probe`, whose falsifiers include counterexamples to
its own score's optimality claims; the schema-v2 manifest, portable
attestation, and cross-host rebuild; the dependency/license closure.

Manifest composition worth knowing: of 100 gates, 61 are
`cargo test`/`clippy`/`doc` hygiene, 50 are research-crate gates, and only
**4 are SBF runtime gates**.

## Two corrections to record

- Outcome mints cannot close for a **structural** reason, not a policy one:
  `MINT_ACCOUNT_LEN = BASE_MINT_LEN = 82` allocates no TLV region, so
  `MintCloseAuthority` is unrepresentable. Same conclusion as the earlier
  "actively refuses" reading, more permanent cause.
- `benchmarks/constants.json` is pinned at `0e4bd51` while the merge added
  3,109 layout lines; codec-digest drift emits a soft note rather than a
  refusal. The tool built to prevent a cost conclusion being attributed to
  a layout the codec no longer has is currently in exactly that state.

There is also no CI: `.github/` does not exist, so there is no automated
regression defense. (This sentence originally said the project operates "one
order of magnitude from the compute ceiling"; the syscall-hash correction —
docs/reviews/COMPUTE_CEILING_REATTRIBUTION_2026-08-19.md — moved every
measured route to 3–27% of the ceiling, which weakens the urgency framing
and none of the CI point.)

## What is genuinely excellent

The B-spline stack; the honesty apparatus (two-ELF discipline, in-tree
injection inventories, a machine-refutable
`claims_universal_no_stranded_value = False`); reproducible builds across
two hosts with divergence exhaustively classified; a clean tree with three
TODOs and 1,008 tests; and a claim vocabulary more rigorous than most
shipped protocols'. The gap is not rigor. The rigor is concentrated in the
semantic plane while every join to the outside world — source, book, client,
deployment — is missing or mocked.
