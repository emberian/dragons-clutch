# Project-intent archaeology — 2026-08-22

This review reconstructs Dragon's Clutch from the human messages that created
and directed it. It is a requirements source, not a promotion ledger. Current
runtime facts still come from code, measured artifacts, and named tests.
Regulatory-filing work is deliberately out of scope.

## Corpus and method

The local `cv` index was refreshed before searching. The useful commands were:

```sh
cv index
cv ls --cwd dragons-clutch --json
cv search 'degg-research' --json --limit 200
cv search 'Dragon Clutch' --json --limit 200
cv show <session-id> --json
cv pack 'dragons-clutch degg-research architecture optimization generalize protocol frontend'
```

The apparent corpus is much larger than the human corpus. Codex swarm children
inherit or repeat a parent prompt, Claude task notifications are stored as user
messages, and `/goal` continuations repeat prior text. Those were treated as
copies or machine envelopes, not new human requirements. Across the four main
Claude sessions, `cv show --json` exposes 733 user-role records and 357
exact-unique texts, but most are task notifications, local-command envelopes,
expanded goal narratives, or pasted handoffs. No precise “human messages”
count is asserted here because that classification is not encoded by `cv`; the
requirements below were recovered by reading the de-duplicated content, not by
treating storage records as votes.

### Primary sessions

| Harness | Session | Recorded working directory | Role in the project |
| --- | --- | --- | --- |
| Codex | `01a00a3d-5612-7253-b858-7b244522a16e` | `~/dev/joshibot` | Canonical prehistory and origin. It contains the Aug 17 concept, the Aug 18 split into `degg-research` and `dragons-clutch`, and the first implementation swarms. |
| Claude | `c37f7ac1-cd60-402f-af26-a45d5b50204a` | `~/dev/dragons-clutch` | Initial repository build and long-running mixed research/implementation coordinator. |
| Claude | `65da8d1f-7994-4d66-b7d4-45c984839d9f` | `~/dev/dragons-clutch` | Architecture, SBF, proof, and test waves. |
| Claude | `1ed3129c-5b6c-4f83-a649-d11dae76cded` | `~/dev/dragons-clutch` | Promotion, source, settlement, and decision waves. |
| Claude | `c198d7f7-ee2b-484a-a70b-10f103e0ed17` | `~/dev/dragons-clutch` | Most recent Cycle-G, operator, seal, branch-triage, and optimization handoff. |
| Claude | `a6fbfac9-8e7b-4064-a47b-e1a109e4eda8` | `~/` | Peripheral status survey that explicitly joined the live Joshi, `degg-research`, and Dragon's Clutch sessions. It adds no independent product requirement. |
| Codex | `01a02aca-fd99-7e82-9823-3443e6f76de8`, `01a02ad0-653f-7a63-aeeb-6151f7d65ca7` | `~/dev/dragons-clutch` | Restart and current architectural review. |

The many `01a00e9c-*`, `01a00e9d-*`, `01a00eb2-*`, `01a00eb3-*`,
`01a00f44-*`, `01a012a7-*`, `01a013*`, `01a0167*`, `01a017*`, `01a018*`,
and `01a019*` Codex hits are parallel children or continuations of the
canonical `01a00a3d...` parent. An audit of 133 matching Codex sessions found
that every other user text was an exact member of that parent. Their project
prompts are inherited copies; substantive landings survive in the repository
and commit history. They should not be counted as separate occasions on which
the user chose the same requirement.

No top-level session is recorded with `degg-research` as its working directory.
That project was created and directed from the `joshibot`-cwd Codex parent and
from the first Claude coordinator. Searching only by cwd therefore misses the
origin of both repositories.

## Recovered product requirements

The following requirements recur in independent human messages and are the
best reconstruction of the intended product, ordered by semantic importance
rather than date.

1. Build a public, fully onchain Solana protocol that does not require a
   Dragon-operated service. Static GitHub Pages/IPFS clients are replaceable
   projections of onchain state.
2. Make liabilities fully collateralized, liquidation-free, and exact in
   integer units. Hoard principal pays claims, never fees, bounties, rent, or
   operating costs.
3. Make the system genuinely general. DREGG is a dogfood Realm, not required
   collateral or a hard-coded branch. Realms immutably select collateral.
4. Support distributions over bounded outcomes rather than a toy handful of
   fixed bands. The native family should include categorical claims and a
   compact smooth basis with exact partition-of-unity semantics.
5. Treat portfolios as native programs: coupled clearing, complete-set
   algebra, shared path accumulators, partial fills, and reusable payoff
   compilation should be one system rather than disconnected demos.
6. Keep price formation pluggable and permissionless. Say “best valid
   submitted candidate” unless a checked optimality certificate exists.
7. Freeze objective resolution procedures and consume proof-carrying evidence;
   refuse ambiguity rather than introduce a discretionary resolver.
8. Design Clear, Shielded, and Dark modes as modalities of one relation, while
   keeping the public Solana system independently useful. Do not turn privacy
   research into an oppression tool.
9. Use Verus/Lean and adversarial testing to improve correctness, but do not let
   an evidence bureaucracy substitute for product capability or honest runtime
   integration.
10. Design fee geometry and protocol income so liveness can be self-sustaining,
    while never treating future fees or Hoard principal as current liveness
    capitalization. Optimize execution cost, rent, and keeper economics without
    weakening exactness or refusal semantics.
11. Make the static GitHub Pages/IPFS frontend excellent in visual design,
    mental model, and information architecture for degens, novice programmers,
    builders, academics, and increasingly capable machine traders.
12. Build the pillars and layers as one advanced product. The user repeatedly
    rejected “minimal demo” or isolated-slice completion framing; a working
    bounded transition family is substrate, not the end state.
13. Local execution may be the completion gate when devnet SOL is unavailable,
    provided the artifact and procedure become testnet-deployable. This is a
    workflow requirement, not standing authorization for a faucet, deployment,
    key read, signature, or transaction submission.
14. Prefer the least constraining and most general sound choice when several
    designs are viable. Fixed bounds should be explicit deployable capacity
    profiles, not quietly confused with the product's conceptual limits.

The most compact original thesis, written by the user in the Claude corpus, was
to compile objective state and path predicates into fully collateralized payoff
bases, clear bounded portfolio programs through interchangeable checked venues,
and settle proof-carrying evidence without an operator. That is broader than a
working bounded venue lifecycle.

## What “finished” can honestly mean

The last Claude session closed a real and substantial implementation wave:
general bounded clearing, partial-fill settlement, pot realization, degree-2/3
price-plane admission, local pull-source custody, a keeper, and a local operator
bench. In that scope, `SETTLEMENT_BLOCKERS` is empty.

It did not finish the complete recovered product thesis. In particular, the
payoff compiler remains a research crate rather than a market-creation path;
Clear/Shielded/Dark do not share a deployed relation; capacity is one fixed
profile; the current adapter is not actually collateral-program-generic; source
identity is not production-pinned; and nothing is deployed. “Cycle-G
capability-complete” must therefore be read as “the bounded Cycle-G capability
matrix has no unimplemented transition,” not “Dragon's Clutch is complete.”

## Requirements that should not be recovered

- No code or dependency should be imported from JOSHI, joshibot, leanuweave,
  minidregg, breadstuffs, Oracle Pit, or historical DREGG prototypes without a
  new explicit decision and provenance/license review. A scan of first-party
  manifests and source imports found no such code dependency in the current
  tree; historical comparison documents are not implementation authority.
- Regulatory packets and filing deadlines are a separate workstream and are
  not part of this review or its execution queue.
- Historical statements authorizing a particular swarm, push, or remote action
  do not enlarge current authority.
