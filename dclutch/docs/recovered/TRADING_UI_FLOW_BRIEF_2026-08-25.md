# Recovered artifact: trading_ui_flow_brief

Recovered 2026-08-27 (ledger M-8, Tier-4 batch) via `cv`/raw session inspection,
for an Opus (or whoever owns `apps/dclutch-web`'s remaining routes) to place.
**This file adds nothing to the payload below and changes nothing in it.**
Everything from `BEGIN VERBATIM PAYLOAD` to `END VERBATIM PAYLOAD` is copied
byte-for-byte from the recovered message.

## Why this exists

`docs/ASPIRATION_LEDGER.md` M-8 calls this *"the highest-value unlanded
artifact"* in the project's history: a complete, implementation-ready
product-flow brief that `WAVE.md` cites as cycle-1 input (*"the recovered
product-flow brief"*) while the brief itself was in no repository — it lived
only in a sub-agent's final message, never committed by the lane that
produced it.

## Provenance (exact chain of custody)

- **Root session**: codex, id `01a02ad0-653f-7a63-aeeb-6151f7d65ca7`, raw file
  `~/.codex/sessions/2026/08/22/rollout-2026-08-22T14-51-35-01a02ad0-653f-7a63-aeeb-6151f7d65ca7.jsonl`
  (the session started 2026-08-22; the spawn below happened partway through
  its run on 2026-08-25).
- **Spawn**: `function_call` `spawn_agent` at `2026-08-25T00:04:38.928Z`,
  `task_name: "trading_ui_flow_brief"`, `fork_turns: "none"` (an independent
  sub-agent, not a context fork of the root).
- **Sub-agent thread**: `01a0363b-b9af-7b31-8493-2f9aca50cf8b`, path
  `/root/trading_ui_flow_brief` — this is the *"lane session 01a0363b under
  root 01a02ad0"* `docs/ASPIRATION_LEDGER.md` M-8 names.
- **The payload below**: a `response_item` of type `agent_message`,
  `author: "/root/trading_ui_flow_brief"`, `recipient: "/root"`,
  `id: "amsg_01a03640-8609-7852-bd03-b2a263753600"`, timestamped
  `2026-08-25T00:09:53.418Z` (ordinal 75222 in the root's raw JSONL), reading
  `Message Type: FINAL_ANSWER`. The root orchestrator never acted on it — the
  next several hundred lines of the same session move on to other lanes
  without a single read-back of this result, which is exactly the failure
  mode the ledger describes (*"and committed nothing"*).
- **Recovery method**: `spawn_agent`'s own `message` argument is Fernet-
  encrypted (`gAAAAAB...`) and irrecoverable; the AGENT'S OWN RETURNED
  `agent_message`, in contrast, is plaintext in the same raw session file —
  consistent with this project's standing note that "codex lane prompts are
  encrypted, but task_names, commits, goal text and exec output are
  recoverable." Recovered by grepping
  `~/.codex/sessions/**/*.jsonl` for `trading_ui_flow_brief`, matching the
  three hits against `docs/ASPIRATION_LEDGER.md`'s cited ids/timestamp, then
  extracting the `content[0].text` field of the matching `agent_message` by
  its `ordinal`.

## What has already landed since (as of 2026-08-27, per the ledger)

`/markets`, `/markets/:address`, and `/portfolio` were built independently
after this brief was lost, so some of the fields/structure below may already
be superseded by what actually shipped — cross-check before treating any
specific field list as still-open work. The `/create` wizard and `/activity`
were **not** built; the wizard is `WAVE.md`'s one remaining cycle-3
pull-forward, and this brief's six-step wizard breakdown (Product, Collateral
Realm, Resolution, Capabilities, Review, Found→readiness→open) is the most
complete design surface for it that exists anywhere. The two preview datasets
and the "controls that must remain disabled" inventory are also unlanded in
full — worth a diff against whatever `apps/dclutch-web` currently disables.

## Placement judgment call, left to the reader

This is priced as *not* small/self-evident to place: two days of further
implementation separate this brief from the current tree, several of its
named routes now have real implementations with their own field lists, and
reconciling "what this brief wanted" against "what got built instead" is a
product judgment call, not a mechanical port. Hence: recovered verbatim here
rather than silently merged into `apps/dclutch-web` or `docs/design/`.

---

## BEGIN VERBATIM PAYLOAD

Message Type: FINAL_ANSWER
Task name: /root
Sender: /root/trading_ui_flow_brief
Payload:
Implementation-ready product-flow brief for dClutch’s screenshot frontend.

Global honesty contract

- Persistent banner: `PREVIEW INTERFACE · No deployed program, official frontend, live market, wallet integration, production source profile, or release.`
- Every data surface gets one provenance chip:
  - `CHAIN · finalized slot {slot}` for hostile-decoded accounts.
  - `PREVIEW · non-authoritative demo data` for the datasets below.
  - `REFUSED` when owner, schema, width, PDA, content link, or canonical bytes fail.
- Human titles, outcome names, symbols, charts, volume, probability, P&L, and cost basis are not Market-account truth. Treat them as preview/index metadata unless a schema-specific finalized artifact validator is added.
- Show raw atoms as primary values. A formatted collateral amount is secondary and only valid after decoding the actual Mint’s display precision.
- Never label Hoard principal as TVL, fees, treasury, reserve, or available liquidity.
- Direct prices are exact integers over `PRICE_SCALE = 1_000_000`; any percentage is a derived display.
- Existing browser functionality is finalized, read-only scanning of Market, Realm, Position, and RentCredit plus structural raw-record finality. It explicitly has no transaction construction.

Suggested routes

- `/markets` discovery
- `/markets/:market` detail with `Overview | Direct | Dealer | Resolution`
- `/create` wizard
- `/portfolio`
- `/activity` optional read-only decoded records/receipts

Market discovery and detail

Discovery card fields:

- Preview title/outcome labels, visibly marked non-authoritative.
- Market address or `No chain address · preview`.
- Phase: `Founding | Open | Resolved | Retiring | Retired`.
- Generation, finalized observed slot, outcome count `2..16`.
- Collateral mint short address and Realm content ID.
- Hoard atoms, never “available balance.”
- Settlement: `Empty` or `Resolved · Occurrence/Failure/Recovery`.
- Capability badges derived from the authenticated manifest; do not infer them from child accounts.
- Refusal reason when decoding/bindings fail.
- Omit protocol “volume,” “open interest,” “APR,” and “odds.” An order index may later show `Indexed, untrusted`.

Detail overview fields:

- Market address, schema/profile, phase, generation, outstanding direct children, rent-refund authority, observed slot/finality.
- Immutable identity IDs: Realm, Product Instance, Claim Basis, Resolution Policy, Capability Manifest.
- Economics: Hoard atoms; ordered aggregate supply vector; exact required backing (`max(supply)` while unresolved, winning supply after resolution).
- Settlement when present: resolution kind, winning zero-based outcome, terminal sequence, evidence content ID.
- Realm: token program, collateral Mint, adapter release ID, mint-authority policy, freeze-authority policy.
- Product/source drawer: target Unix time, grace seconds, inclusive resolution window, maximum crossing lag, maximum age, future skew, confidence multiplier/BPS/atom ceiling, normalized decimals, ordered upper edges, Pyth release/feed/base/quote semantic IDs, failure outcome index.
- Manifest drawer per entry: kind/release/config/capacity/schema/derivation IDs, activation policy, deadline slot, dependency indices, typed funding compartments, FundingState `Pending | Active` and activation slot.

Phase meanings for UI copy:

- `Founding`: identity exists; obligations/readiness still being assembled; no liabilities/trading.
- `Open`: liability transitions admitted; terminal settlement empty.
- `Resolved`: winner fixed; redemption admitted.
- `Retiring`: no new direct children; resolved redemption still admitted. An unresolved retiring market must already be economically empty.
- `Retired`: terminal replay authority only; zero children and empty economics.

Create-market wizard

1. `Product`
   - Preview-only title/description/outcome labels.
   - Finalized record selectors: Terms, Occurrence, Product Instance, Claim Basis, Result Domain, Capacity Profile.
   - Exact checks: exhaustive/disjoint/ordered/canonical partition requirement; outcome count; artifact bytes/pages; shared content links.

2. `Collateral Realm`
   - Token program and collateral Mint.
   - Observed mint and freeze authorities.
   - Explicit risk choices: `Require absent` or `Admit issuer control`; strict default requires both absent.
   - Preview report: adapter release, Realm content ID/PDA, signer, Realm rent, exact sponsor debit.

3. `Resolution`
   - Resolution material and policy/feed fields listed above.
   - Exact result-domain cuts/denominator and explicit failure outcome.
   - Resolution fund quote: Fund rent, provider reimbursement, success bounty. Keep these separate from Hoard collateral.

4. `Capabilities`
   - Ordered immutable manifest entries, dependencies, activation policy:
     `Required at founding` or `Prepaid lazy`.
   - Show activation deadline and typed Rent/Creation/Work/Provider/Bounty/Liquidity/Service funding.
   - Readiness progress is derived as `next_entry_index / entry_count`; “Ready” only when equal.

5. `Review exact plan`
   - Generation is fixed to `0` for the implemented foundation flow.
   - Derived Market/Fund PDAs, outcome count, sponsor signer, market/fund/provider/bounty debit breakdown, total debit, finalized observation slot/time.

6. `Found → readiness → open`
   - Timeline: `Create RentCredit → Create/reuse Realm → Found Market + Fund → Begin readiness → advance each manifest entry → create collateral custody/Vault and Open`.
   - Opening report: generation, prior child count, custody PDA, vault PDA, exact sponsor rent debit.

Direct order ticket

Editable preview controls:

- `Buy | Sell`
- Ordered outcome index plus preview label
- Lifecycle: `Fill or kill | Immediate or cancel | Registered resting`
- Maximum fill in raw claim atoms
- Limit price in exact `0..1,000,000` scale
- Inclusive valid-from and valid-through slots
- Collateral account and native Position account

Locked/derived fields:

- Market address, generation, maker Ed25519 key, next gap-free nonce.
- Manifest-selected fee-config digest, exact fee BPS, fee recipient.
- Replay-root PDA and registration state `Open | Closed`.
- Registered Buy maximum reserve:
  `floor(max_fill × limit_price / 1_000_000) + maximum cumulative fee`.
- Registered Sell reserve: exactly `max_fill` selected claims.
- Required token delegate/allowance warning for Buy.
- System creation payer and RentCredit beneficiary.

Live intent card:

- Nonce, side, lifecycle, outcome, slot interval, limit, max fill, filled, remaining.
- Reserved claims/collateral, fee-bearing gross, cumulative fee.
- Replay root: next nonce, minimum-live nonce, live count, registration status.
- State labels: `scheduled`, `fillable`, `expired`, `invalidated`, `partially filled`; these are client-derived, not persisted status bytes.

Do not render a canonical order book. Resting records may be indexed only as an explicitly untrusted projection.

Dealer liquidity

Pool header:

- Pool address, Market generation, capability release/config IDs.
- Status `Active | Retiring | Retired`.
- Liquidity owner/service-refund beneficiary.
- Reset number, next replay sequence, next reset slot.
- Config price scale, fee BPS, maximum trade quantity, reset interval.
- Per-outcome bid/ask ladders, best first: exact prices, configured capacities, filled counters, derived remaining capacity.
- Segregated values: principal collateral, realized fee collateral, claim-reserve vector, service funding.
- Total shares and live LP-position count.

Immediate Dealer ticket:

- Side: `Buy claim from pool | Sell claim to pool`.
- Claim index, quantity.
- Collateral limit:
  - Buy: maximum gross collateral debit.
  - Sell: minimum principal collateral credit; fee is a separate collateral debit.
- Locked reset number and expected sequence.
- Exact quote output: notional, fee, collateral debit/credit, claim debit/credit, selected bin before/after.

LP panel:

- LP Position address, parent pool/generation, owner, status `Empty | Active | Closed`.
- Shares, next position sequence, RentCredit beneficiary/principal.
- Add request: shares to mint plus maximum deposit vector.
- Remove request: shares to burn plus minimum withdrawal vector.
- Keep principal, realized fees, claims, and service funding visibly separate.

Portfolio and positions

- Group native Position accounts by Market and generation.
- Fields: Position PDA, owner, generation, ordered raw outcome balances, phase, settlement.
- Open-market derived `complete sets mergeable = min(outcome balances)`.
- Resolved-market row per outcome:
  - Winner pays exactly one collateral atom per redeemed claim atom.
  - Losers pay zero; redemption still burns the losing claims.
- Show `exact redeemable payout` only from the canonical settlement and current Position balance.
- Separate sections for:
  - Native Position balances.
  - Direct reserved intent custody/escrow.
  - Dealer LP shares/value.
  - Bearer representation, if the manifest enables it.
- Do not synthesize portfolio value, P&L, or cost basis as onchain facts.
- Empty Position may be shown as close-eligible only after child-count and RentCredit checks.

Resolve and redeem

Derived resolution state:

- Open, before `target + grace`: `Waiting for price window`.
- Open, inclusive `[target + grace, target + grace + window]`: `Price resolution window open`.
- Open, strictly after window end: `Permissionless failure resolution eligible`.
- Resolved/Retiring: show winner, kind, evidence ID, sequence and redemption rows.
- Retired: no redemption/economics remain.

Price-resolution review:

- Finalized observation slot and Unix time.
- Required signer roles: resolver and temporary update account.
- Exact Pyth post-update body/encoded VAA and selected release.
- Fund movements: Fund rent refund, provider reimbursement, bounty, unclassified excess to immutable RentCredit.
- Price path is valid only inside the inclusive window.

Failure-resolution review:

- Enabled semantically only strictly after window end.
- Explicit failure winner, bounty recipient, same funding classification.
- Bounty recipient is plumbing, not Product truth.

Redeem review:

- Outcome, raw quantity, canonical winner, exact payout.
- Allowed only in `Resolved | Retiring`.
- Losing redemption payout is exactly zero.
- Show Market Hoard and winning supply before/after in confirmation.

Controls that must remain disabled

The screenshot may allow tabs, filters, wizard navigation, and editing preview fields. Every state-mutating CTA must remain disabled.

- Global: `Connect wallet`, `Sign`, `Submit`, `Deploy`, `Create market`, `Place order`, `Match`, `Provide liquidity`, `Resolve`, `Redeem`.
- Foundation builders exist for RentCredit, Realm, Found Market/Fund, and Open Vault, but there is no frontend/wallet bridge or checked deployed release.
- There is no operator builder for readiness Begin/Advance, so the wizard cannot honestly progress from Founding to Open.
- Direct SBF/contract code exists, but the repo states trading is not end to end; operator only covers cancel-through and retirement-close paths, not order registration/matching. Disable all Direct actions.
- Dealer has semantic/SBF lifecycle code, but local-validator, unsigned operator, routing, and measurement integration are incomplete; operator currently exposes reset only. Disable activate, quote execution, trade, LP create/add/remove/close, reset, and retire.
- Native Position split/merge/transfer/redeem SBF paths exist, but no corresponding operator builders/frontend integration. Disable all Position mutations, including Redeem.
- Price/failure resolution and terminal compaction have operator builders, but remain disabled until a checked deployment/release manifest, same-finalized snapshot plumbing, exact account acquisition, wallet/signature path, and local-validator evidence are wired.
- Preview datasets always keep CTAs disabled, regardless of apparent phase.

Two non-authoritative demo datasets

Both must display a prominent `PREVIEW · non-authoritative · no chain address` badge.

```ts
const previewEth = {
  provenance: "preview",
  id: "preview-eth-2026-09-30",
  title: "ETH/USD at 16:00 UTC on Sep 30, 2026",
  marketAddress: null,
  phase: "Open",
  generation: 0,
  observedSlot: null,
  collateralLabel: "USD stable collateral · 6 decimals (preview metadata)",
  outcomeCount: 4,
  outcomes: [
    { index: 0, label: "ETH/USD < 3,500.00" },
    { index: 1, label: "3,500.00 ≤ ETH/USD < 4,000.00" },
    { index: 2, label: "ETH/USD ≥ 4,000.00" },
    { index: 3, label: "Resolution failure" }
  ],
  targetTime: 1790784000,
  graceSeconds: 60,
  windowSeconds: 900,
  resolutionWindowInclusive: [1790784060, 1790784960],
  normalizedDecimals: 8,
  upperEdges: ["350000000000", "400000000000"],
  failureOutcomeIndex: 3,
  hoardAtoms: "125000000000",
  aggregateSupply: ["125000000000", "125000000000", "125000000000", "125000000000"],
  indicativePreviewPrices: [240000, 450000, 280000, 30000],
  capabilities: ["Direct preview", "Dealer preview", "Pyth categorical preview"],
  settlement: null
};
```

```ts
const previewSolResolved = {
  provenance: "preview",
  id: "preview-sol-2026-08-21",
  title: "SOL/USD at 16:00 UTC on Aug 21, 2026",
  marketAddress: null,
  phase: "Resolved",
  generation: 0,
  observedSlot: null,
  collateralLabel: "USD stable collateral · 6 decimals (preview metadata)",
  outcomeCount: 4,
  outcomes: [
    { index: 0, label: "SOL/USD < 160.00" },
    { index: 1, label: "160.00 ≤ SOL/USD < 220.00" },
    { index: 2, label: "SOL/USD ≥ 220.00" },
    { index: 3, label: "Resolution failure" }
  ],
  targetTime: 1787328000,
  normalizedDecimals: 8,
  upperEdges: ["16000000000", "22000000000"],
  failureOutcomeIndex: 3,
  hoardAtoms: "70000000000",
  aggregateSupply: ["81000000000", "70000000000", "82250000000", "82000000000"],
  settlement: {
    kind: "Occurrence",
    winner: 1,
    terminalSequence: "402144321",
    evidenceId: "preview:evidence:sol-2026-08-21"
  },
  previewPosition: {
    balances: ["0", "250000000", "25000000", "0"],
    exactWinningPayoutAtoms: "250000000",
    exactLosingPayoutAtoms: "0"
  }
};
```

Primary source anchors: `README.md`, `apps/dclutch-web/lib/decoders.ts`, `crates/dclutch-operator/src/{lib.rs,foundation.rs,verticals.rs}`, `dclutch-core-contract`, `dclutch-market-contract`, `dclutch-realm-contract`, `dclutch-direct-contract/DESIGN.md`, and `dclutch-dealer-contract/DESIGN.md`.

## END VERBATIM PAYLOAD
