# GOAL — the index

This file is an index, not a store. It says what the project is, what the
standing goal is, what the tree is trying to become, and where each dated
delta is recorded. Nothing below is a fact's only home: a decision lives in
its record, a cohort fact in its evidence document, a design fact in its
note's head, a rule in `AGENTS.md`, and a number in the generated reference.
History lives in `docs/ledger/` verbatim and dated.

## What the project is

dClutch is a Solana protocol for fully collateralized claims on bounded-state
questions — where a price will be at a stated time, split into cells, every
claim backed by collateral locked before it exists. `README.md` says it for a
stranger; `docs/INTENT.md` says why, in ember's own words with provenance.
Completion is defined by `docs/MASTER_COMPLETION_CONTRACT.md` (rows
C-00..C-17); what is deliberately not built is `docs/OMISSION_INDEX.md`.

## The standing goal — ember, 2026-09-01

> *"Make dclutch the best version it can be, eliminating all protocol defects
> and making the operator console & UX excellent."*

Standing authority (devnet deploy, publication cuts, what is not authorized)
is in `AGENTS.md`. Rulings are decision records: `docs/decisions/` (the
generated index is `docs/reference/decisions.md`, with each record's status).
The one question still genuinely ember's is decision 0029's tenth item, the
flagship conditional market's feature gate, slot and metric.

## The attractor

A tree where every fact has exactly one author and every claim is either
generated from the tree or dated and owned. Concretely:

1. this file is an index of dated deltas; the stores are `docs/decisions/`
   (records, generated index), `docs/evidence/<cohort>` (facts from the job
   directory's machine-readable witnesses, prose only for findings) and
   `docs/reference/` (generated, `--converge`);
2. a design note's head states the current truth; addenda live below a
   `## History` fold;
3. one cohort runbook parameterized by a cohort manifest (`tools/cohort/`);
4. no builder without a campaign that executes it — dead ones deleted by
   reading, producer-missing ones named in their crate doc;
5. `tools/lane.sh commit` is the only commit path, and every rule a lane
   needs is in `AGENTS.md` as a rule, with its history in the ledger.

The parsimony closeouts that named it are in the ledger
(`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2477`, `#L4147`).

## Dated deltas

One line each. The link is the entry in the ledger; the store is where the
fact now lives. The ledger is
[`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md`](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md)
(4,992 lines, verbatim, newest 2026-09-04 material at the head and again at
the tail; the pre-2026-09-01 ledger starts at line 289), and before it
[`docs/ledger/WAVE_2026-08-26_to_2026-09-02.md`](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md).

| when | delta | ledger | store |
| --- | --- | --- | --- |
| 2026-08-26 | cycle 1 launched: local-first, assurance parked, frontend first-class | [WAVE L1](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md#L1) | `PROJECT_METHOD.md` |
| 2026-08-27 | the market is open (run 6); DEPLOY-1 lands the substrate on devnet | [WAVE L509](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md#L509), [L1016](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md#L1016) | `docs/evidence/DEPLOY_1.md`, decision 0012 |
| 2026-08-27 | the aspiration audit: what was intended and never mapped | — | `docs/evidence/ASPIRATION_LEDGER_2026_08_27.md` |
| 2026-08-29 | cycle 3 charter: "fold it all in" | [WAVE L1164](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md#L1164) | decisions 0013–0017 |
| 2026-08-30 | ember's rulings on the decision packet | [WAVE L1238](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md#L1238) | `docs/decisions/DECISION_PACKET_2026_08_30.md`, 0014–0017 |
| 2026-08-31 | full-autonomy directive; the completion contract starts | [WAVE L1282](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md#L1282) | `docs/MASTER_COMPLETION_CONTRACT.md` |
| 2026-08-31 | the codex handoff and its queue | — | `docs/ledger/HANDOFF_CODEX_2026_08_31.md`, `LETTER_TO_CODEX_2026_08_31.md` |
| 2026-09-01 | the codex swarm's letter back: five walls, lanes S0–S11 | — | `docs/ledger/LETTER_TO_CLAUDE_2026_09_01.md`, `START_HERE_2026_09_01.md` |
| 2026-09-01 | the class: declarations never executed against reality (three instances) | [WAVE L1609](docs/ledger/WAVE_2026-08-26_to_2026-09-02.md#L1609) | `docs/evidence/C16_ENTRY_LIST_2026_09_01.md` |
| 2026-09-01 | cohort-9 opens; devnet deploy authorized, standing | [GOAL L437](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L437), [L1520](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L1520) | `AGENTS.md`, `docs/evidence/COHORT9_DEVNET_DEPLOY_2026_09_01.md` |
| 2026-09-01 | C-15 ruled out: privacy/FHE is a later Clutch | [GOAL L2067](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2067) | decision 0018 |
| 2026-09-01 | ruling 9 withdrawn (ceremony); devnet is disposable | [GOAL L2097](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2097) | decision 0012 (amended), 0019 |
| 2026-09-01 | the architect-scholar overturns six of eight coordinator calls | [GOAL L2181](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2181) | `docs/evidence/ARCHITECT_SCHOLAR_2026_09_01.md` |
| 2026-09-01 | non-price resolution sources ruled; already built, gate broken and repaired | [GOAL L2234](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2234) | `docs/evidence/NON_PRICE_RESOLUTION_DESIGN_2026_09_01.md` |
| 2026-09-01 | the standing goal | [GOAL L2392](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2392) | this file |
| 2026-09-01 | parsimony closeout: the attractor first named | [GOAL L2477](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L2477) | this file |
| 2026-09-02 | the first devnet fill (cohort-13) | [GOAL L3360](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3360) | `docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md` |
| 2026-09-02 | the first devnet resolution, by the failure walk | [GOAL L3456](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3456) | same |
| 2026-09-02 | the full lifecycle on devnet: founded → paid | [GOAL L3506](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3506) | same |
| 2026-09-03 | cohort-14 deployed, sealed, filled | [GOAL L3695](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3695) | `docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md` |
| 2026-09-03 | the partial equity Remove commits; C-06 closes at 31 of 31 | [GOAL L3723](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3723), [L3767](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3767) | `docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md` |
| 2026-09-03 | the first honest resolution and the first ATA payout (market B) | [GOAL L3786](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3786) | cohort-14 evidence |
| 2026-09-03 | market C relayed end to end; the two scales disagree over a stranger | [GOAL L3961](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3961) | `docs/design/OBSERVATION_SCALE_AUTHORITY.md` |
| 2026-09-03 | the docket for ember | [GOAL L3946](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L3946) | decisions 0019–0030 |
| 2026-09-03 | tier 1 completes; cohort-15 starts; parsimony closeout two | [GOAL L4110](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4110), [L4147](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4147) | `tools/cohort/` |
| 2026-09-03 | cohort-15 deployed, sealed, founded, captured | [GOAL L4198](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4198) | `docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md` |
| 2026-09-04 | the two selectors agree; a third market filled; market 3 settled | [GOAL L4234](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4234), [L4306](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4306), [L4415](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4415) | cohort-15 evidence |
| 2026-09-04 | rent is a rate; a ruling on rent | [GOAL L4460](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4460), [L4505](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4505) | decision 0030 |
| 2026-09-04 | the first stranger paid on an honest selector | [GOAL L4541](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4541) | cohort-15 evidence |
| 2026-09-04 | a resolution fund closed on chain | [GOAL L4600](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4600) | cohort-15 evidence |
| 2026-09-04 | ember's rulings, as amended (D1–D8) | [GOAL L4644](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4644) | decisions 0024–0030 |
| 2026-09-04 | the mechanism agenda: six directions, design first | [GOAL L4670](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4670) | decision 0031; `docs/design/MECHANISM_*_2026_09_04.md` |
| 2026-09-04 | wave one of the agenda and the ruling spokes | [GOAL L4714](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4714) | decisions 0032–0034 |
| 2026-09-04 | ember confirms the provisional records | [GOAL L4913](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4913) | decisions 0019–0034 (status CONFIRMED) |
| 2026-09-04 | the simplification map: dClutch at a third the size | — | `docs/design/SIMPLIFICATION_MAP_2026_09_04.md` |
| 2026-09-04 | after the limit: General derives and projects; the Lean log bounds; recovery's ladder; Claims founding v6; Series' eighteen phases | [GOAL L4925](docs/ledger/GOAL_2026-08-31_to_2026-09-04.md#L4925) | the commits named there |
| 2026-09-04 | the simplification swarm: eleven makers, one branch each, rip and tear and converge later | [2026-09-04 L4](docs/ledger/2026-09-04.md#L4) | `docs/design/SIMPLIFICATION_MAP_2026_09_04.md`; each maker's `SIMPLIFY_<DOMAIN>.md` on its branch |
| 2026-09-04 | the second wall and the resume; every branch reports; crates 94 → 46; all eleven final | [2026-09-04 L44](docs/ledger/2026-09-04.md#L44) | the same reports |
| 2026-09-04 | the convergence: eleven branches merged in the map's order, one build-and-gate pass, the ELF table | [2026-09-04 L165](docs/ledger/2026-09-04.md#L165) | `docs/evidence/SIMPLIFICATION_CONVERGENCE_2026_09_04.md` |
| 2026-09-05 | the convergence closed: the gates re-measured, the gauntlet run with controls, the frames function by function, twelve repairs | [2026-09-04 L175](docs/ledger/2026-09-04.md#L175) | `docs/evidence/SIMPLIFICATION_CONVERGENCE_2026_09_04.md` §7–8 |
| 2026-09-05 | the repair wave after the convergence: the suite reds by column, the four web tests, one Cargo workspace for the 55 | [2026-09-05](docs/ledger/2026-09-05.md) | `docs/evidence/SIMPLIFICATION_CONVERGENCE_2026_09_04.md` §8 |
| 2026-09-05 | a handoff for the next orchestrator, human or model: the state, the rules, the path, the pitfalls | [handoff](docs/HANDOFF_2026_09_05.md) | `docs/HANDOFF_2026_09_05.md` |

A new delta is one new row here and one entry in the store it names. The
GOAL ledger file is closed; the entries main appended during the swarm are
`docs/ledger/2026-09-04.md`, verbatim, and later narrative goes in a new dated
file under `docs/ledger/`, which this table links to.
