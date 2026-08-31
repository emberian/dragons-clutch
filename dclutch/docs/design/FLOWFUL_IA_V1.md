# FLOWFUL_IA_V1 — an information architecture you can move through

**Status:** design spec, ready to implement. **Scope:** `apps/dclutch-web`.
**Written for:** one implementing lane that should need no further research.
Every signature, string, file path and line number below was read at HEAD on
2026-08-31; where a claim is a measurement it says so, and where it is a
judgement it says that too.

This document does not re-derive the diagnosis. It builds on the one the
orchestrator took from ember's trade-panel screenshot:

> everything at the same depth (byte budgets and capability roots at equal
> weight with the trader's two real decisions); the trade is a FLOW rendered as
> a flat console; the two-signed-halves model explained in prose instead of
> taught by the interface; raw units (`500000000` "issued atoms", price scale
> `1000000` unexplained, "claim atoms" as an input label); the ticket is an
> empty JSON textarea instead of rendering AS A TICKET on paste; sign-vs-send is
> a paragraph instead of two visible states.

And on ember's charter addendum, which sharpens the fifth item into a
requirement rather than a polish note:

> what can we be offering (onchain or as a service in the dregg infra, or some
> js peer-to-peer thing) so they don't need to be doing manual stuff

That addendum is answered in **§4**, and it changes the shape of the trade flow:
step 3 is not "paste better", it is *"do not make them source the other half by
hand at all."*

---

## 0. How to read this, and the one rule that governs it

The site is not badly built. It is **honestly built and flatly presented.** Almost
every defect below is a *depth* defect, not a truth defect: the right fact is on
screen, at the wrong weight, in the wrong unit, at the wrong moment. That
distinction decides every call in this spec, and it produces one governing rule:

> **The governing rule.** A surface may only ask a person for something it has
> already given them the means to decide. Everything else moves down a level —
> and *down a level is never gone.*

Two corollaries used throughout:

- **Nothing is deleted to make room.** Depth is the tool, not subtraction. The
  precedent is GRICE's market card (10 raw rows → 5 reader rows + a collapsed
  "in the protocol's own words" drawer, nothing removed). This spec extends that
  pattern; it does not invent one.
- **Refusals stay named.** A refusal is the protocol working. It may be
  *re-ordered* (remedy first, cause second) and *relocated* (to the step that
  owns it). It may never be softened, merged into a generic error, or hidden.

### 0.1 Measured facts this spec stands on

These were measured, not assumed. They are load-bearing for §7 especially.

| Fact | Measurement |
| --- | --- |
| Tailwind v4 is already installed and imported | `tailwindcss@4.2.1` + `@tailwindcss/postcss@4.2.1` in devDependencies; `@import 'tailwindcss';` is line 1 of `app/globals.css` |
| …but **zero** Tailwind utilities are in use | Exhaustive scan of every `className` in `components/*.tsx`, `components/charts/*.tsx`, `app/*.tsx`: no standalone utility token (`flex`, `p-4`, `text-sm`, `gap-2`, `w-full`, …) appears. Every class is a hand-authored semantic name (`trade-v3-card`, `direct-status`, `market-refusal`). No `@theme`, `@apply`, `@layer`, `@config` or `@plugin` anywhere |
| Hand-written CSS surface | `app/globals.css` 1433 lines + `app/charts.css` 115 lines = 1548 lines |
| Tests pin **strings**, never DOM structure | 98 × `renderToStaticMarkup`; 656 × `toContain`; 224 × `not.toContain`; 11 × `toMatch`; **0** × `querySelector` / `getByRole` / `getByTestId` / `.closest(` / `.tagName` across all 51 component test files |
| The humanizer already exists, tested, unused by the trade panel | `formatAtomsV1(atoms, decimals)` at `packages/dclutch-sdk/lib/marketDiscovery.ts:1238`. Pinned: `formatAtomsV1('500000000', 6) === '500'` |
| Collateral display decimals are chain-read | `MarketHoardV1.mintDisplayDecimals` (`marketDiscovery.ts:199`), the mint's own `decimals` byte, `number \| null` |
| The collateral token has **no** name anywhere | No symbol, no Metaplex metadata read, nothing in `fixtures/market-registry.devnet.json`. It is identified only by mint address + decimals |
| Price is a fraction, provably | `previewDirectInlineV3` refuses `executionPrice > route.priceScale` (`lib/directInlineV3.ts:743`), and `gross = fill × price / priceScale`. So `price / priceScale ∈ (0, 1]` always |
| The browser is the only ticket author in the repo | `SESSION_STATE.md` (TRADE-3): "the only Direct-ticket AUTHOR in the repo is the browser trade panel"; a tools-side ticket author is queued and does not exist |
| The on-chain resting-order record set already exists | `crates/dclutch-direct-codec/src/registered_*.rs` (7 modules) + `generated_registered_controller.rs` + `generated_registered_fill_v4.rs`; decoded and labelled in `lib/explorer/accountRecords.ts:1208-1290` |

---

## 1. The three journeys

Three people use this site. Today the site is organised by *protocol family*
(Direct, General, Dealer, Registry, Claims), which is the right organisation for
the ninth visit and the wrong one for the first.

| | **Trader** | **Observer** | **Operator** |
| --- | --- | --- | --- |
| Wants | to take a position | to believe the thing works | to run the protocol |
| Asks | "what can I buy, and is it worth it?" | "is this real, and is it alive?" | "what does this account/packet/release actually say?" |
| Tolerates | almost no jargon | some, if it is glossed | all of it, and *wants* precision |
| Fails when | a raw integer asks for a decision | a number appears with no provenance | a value is rounded, renamed, or hidden |
| Post-GRICE state | **worst served** — this spec is mostly about them | **mostly fine** — an inventory of small changes | **fine** — flow helps, precision must survive |

**The single most important consequence:** operator precision and trader clarity
are *not* in tension, because they are not the same depth. The trader gets
`500 · 35¢` at decision depth; the operator gets `500000000 atoms @ limitPrice
350000 / scale 1000000` one click down, in mono, exact. Both are true; only the
ordering changes.

### 1.1 The complete public route map

Every route in `app/`, assigned. **Reference** means "not a journey step — a
place you go to look something up."

#### Trader

| Route | Component | Becomes |
| --- | --- | --- |
| `/markets` | `MarketDiscoveryWorkspace` | **Step 1 · Find.** Stays. Its §01/§02 split ("Markets you can trade" / "Everything else here") is already correct journey design — the only such split on the site. Keep verbatim |
| `/markets/[address]` | `MarketDetailWorkspace` | **Step 2 · Understand** and **Step 3 · Trade.** The whole of §3 and §6 of this spec |
| `/market?address=…` | `MarketAddressWorkspace` | **Merges.** Query-param twin of `/markets/[address]` (it exists for the static export). Keep the route as a redirect shell; it must never render a second, differently-shaped market page |
| `/portfolio` | `PortfolioWorkspace` | **Step 4 · Hold.** Stays |
| `/redeem` | `PortfolioWorkspace` (`mode="redemption"`) | **Step 5 · Cash out.** Stays. Currently listed as an operator console — **demote out of the console list**: it is the trader's last step, it takes no operator input, and it is already honest that payout is not open ("Payout is not open yet") |
| `/activity` | `ActivityWorkspace` | **Reference** (trader-facing). Stays in nav |

#### Observer

| Route | Component | Becomes |
| --- | --- | --- |
| `/` | `SiteLanding` | **The front door.** Stays. §1 `What is out there right now` is the live proof; keep it first |
| `/live` | `LaunchStory` | **The pitch.** Stays. **One defect to fix:** its `aside.launch-scoreboard` renders `7` / `64` / `0.50%` as **hard-coded literals** while every neighbouring surface reads live. Either read them or label them as the release's fixed parameters — do not let a static number sit in a live-looking tile |
| `/pulse` | `PulseWorkspace` | **The heartbeat.** Stays, unchanged. Post-DESIGN this is the best page on the site: seven numbered sections, every empty state distinct and honest, `—` for unread vs `0` for read-zero. **Use it as the reference implementation** for §6's disclosure grammar |
| `/smoke` | `SmokeStory` | **The three tests.** Stays. Flag-gated from `/` |
| `/campaign` | `CampaignWorkspace` | **Splits out of the consoles.** Zero inputs, zero buttons, zero forms — it is a *published transcript of one market's whole life*, which is the single best observer artifact in the repo. It is currently filed under "Operator consoles" where no observer will find it. **Move to observer**, link from `/live` |
| `/population` | `PopulationWorkspace` | **Splits out of the consoles.** Same reasoning: zero inputs, pure narrative record of a simulated world. **Move to observer** |
| `/bounty` | `BountyWalk` | **Adopt or retire.** Currently an **orphan**: in neither `PRODUCT_ITEMS` nor `CONSOLE_PATHS`, reachable only from a flag-gated card on `/workbench`. It is a *good page* (pure read-out, 4 clear sections, honest cost table). Give it a home under observer, linked from `/smoke` §03, which already tells its story |
| `/explorer` | `ChainExplorer` | **Reference** — the deep end, for all three journeys. Stays in nav. Out of scope for migration (§7.5) |

#### Operator

The 13 consoles `ConsoleDirectory` lists are a **flat array with no grouping**
(`type ConsoleEntry = { href; name; blurb }` — no `kind`, no headers). The order
is *roughly* lifecycle-shaped but nothing in the code says so.

**What changes:** give the directory the grouping its order already implies, in
four named bands. This is the cheapest high-value operator change on the site —
it is one component, no page touched.

| Band | Consoles |
| --- | --- |
| **Build a market** | `/workbench` (readiness), `/found` (legacy founding), `/product-v2` (payoff studio), `/create` — see note |
| **Run a market** | `/trade` (Direct route check), `/liquidity` (dealer equity), `/general` (clearing), `/resolution` |
| **Run the deployment** | `/release` (activation), `/operate` (constructors) |
| **Read the record** | `/local` (checkpoint diff) |

…with `/campaign`, `/population` and `/redeem` **leaving** the console list per
the rows above.

**Three IA defects found in the console tier, all worth fixing in the same pass:**

1. **`/direct` is a byte-for-byte duplicate of `/trade`.** Both render
   `DirectTradeWorkspace`; the component hardcodes `<ConsoleHeader path="/trade">`,
   so a reader at `/direct` is told they are at `/trade`. `CONSOLE_PATHS` lists
   both; `ConsoleDirectory` links only `/trade`. **Make `/direct` a redirect.**
2. **`/resolution` lies about where you are.** It renders `MarketWorkbench` with a
   hardcoded `<ConsoleHeader path="/workbench" title="Lifecycle readiness">`, so
   the console strip and the Nav active state both claim `/workbench`. Its
   `ConsoleDirectory` blurb describes content the page has no copy of. **Either
   give it its own header or fold it into `/workbench` as a deep link.**
3. **`RationalRepresentationWorkspace` is dead.** It declares
   `<ConsoleHeader path="/redeem">` but is routed nowhere; its only importer is
   its own test. `/redeem` is served by `PortfolioWorkspace`. **Retire it or route
   it**, but do not leave a console surface that no URL reaches.

**`/create` (nav label "Design")** is a product page, not a console — it is the
market *designer's* journey, a fourth persona thinly served. Leave it where it
is; note it as out of scope.

### 1.2 What merges, what splits, what demotes — the summary

- **Merges:** `JoinPanel` (§05 of the market page) → **into trade-flow step 1**;
  `/direct` → `/trade`; `/market` → `/markets/[address]`.
- **Splits:** `ConsoleDirectory` flat list → four named bands; `/campaign`,
  `/population`, `/redeem` → out of the operator tier.
- **Demotes to a drawer:** on the market page, sections **01** (`What this market
  is` — 10 identity facts + 6 content IDs + the bindings checklist), **03**
  (`What it pays out in` — the Realm), **04** (`What it is allowed to do` — the
  capability manifest and its seven funding compartments). All three become
  drawer depth. See §6.
- **Adopts:** `/bounty` gets a home.

---

## 2. The market page, rebuilt (Journey step 2: understand it in 10 seconds)

Today `MarketDetailWorkspace` renders, in order: hero, **00** Connection, **01**
What this market is, **02** The money, **03** What it pays out in, **04** What it
is allowed to do, **05** Join this market, **06** Trade this market, then
aggregate retirement status. Nine peers, numbered like a checklist, at one depth.

**The trader reads a market page to answer four questions.** Everything else is
evidence for those answers and belongs underneath them.

### 2.1 The stat-card grammar

> **A fact earns the top row if, and only if, changing it would change whether a
> reader enters this market in the next ten seconds.**

That rule admits exactly four, and it is worth stating what it *excludes*:
account width in bytes, schema version, generation, ledger revision, outstanding
capabilities, manifest fingerprint, rent beneficiary, registry program, custody
namespace. Each is true, each is somebody's decision input — none is *this*
reader's, *now*.

| # | Card | Value | Sub-line | Source |
| --- | --- | --- | --- | --- |
| 1 | **Status** | `Open` / `Resolved — <winner name>` / `Never traded` / `Closed` | the phase's own meaning sentence | `decoded.phase` + `marketActivationOutlookV1(card)`, fused. The fusion matters: `Open` with an elapsed activation window is **not** tradeable, and the chain has no phase for that — the page already knows this (`activation.status === 'never'`) and says it in a footnote. Promote it into the card |
| 2 | **Collateral held** | humanized vault principal + unit | `<raw> atoms` in mono, and the mint short address | `decoded.hoard.principalAtoms` @ `decoded.hoard.mintDisplayDecimals` |
| 3 | **Leading outcome** | editorial outcome name + share % | `<n> of <total> claims issued` | `decoded.liability.supplyAtoms` + `editorial.outcomes`. Falls back to `claim <i>` when unregistered |
| 4 | **Settles** | wall-clock deadline phrase, or `Resolved <when>` | the resolution sentence from the registry | `deadlineMomentPhraseV1(clock, deadline, nowMs)` — already built, already used for capability deadlines |

Cards 2 and 3 render `—`, never `0`, when unread. That distinction is already
enforced on `/pulse` and in `NumberStrip`; it is site law and this spec keeps it.

### 2.2 The new section order

| Depth | Section | Contents |
| --- | --- | --- |
| Hero | Title, question, the four stat cards | editorial `title` + `question`, then §2.1 |
| Decision | **The odds** | `SupplyShareStrip` + `CellStrip` + the per-outcome list, outcome names from the registry. Unchanged charts — see §7.5 |
| Decision | **Trade this market** | The flow. All of §3 |
| Decision | **Your position here** | `PortfolioWorkspace`'s per-market row, when a wallet is connected. (New; it currently exists only on `/portfolio`) |
| Drawer | *In the protocol's own words* | old §01 + §03 + §04, verbatim, nothing dropped: identity facts, content IDs, bindings checklist, the Realm, the capability manifest, the funding compartments table |
| Drawer | *Connection & provenance* | old §00: endpoint, finalized floor, core/registry program, the live-watch sentence, the slot-clock caveat, every `SectionProvenance` chip |
| Reference | `See everything it is connected to →` | `/explorer?view=market&q=…`. Already present in the hero aside. Keep |
| Reference | Aggregate retirement status | Unchanged, last |

The `<details>` **stays a `<details>`** — see §7.4 for why that is a correctness
requirement and not a preference.

---

## 3. The trade flow, fully specified

This is the heart of the spec. The panel today (`components/MarketTradePanel.tsx`,
800 lines) is a `<section>` containing eleven sibling blocks: two action buttons,
three status paragraphs, two evidence grids, an outcome list, a ticket textarea,
a size input, a wallet directory, a walls list, and one `<details>` that contains
the *entire* second half of the trade — preparation, both signatures, submission,
and the finalized read-back.

**The logic in that file is good and must not be rewritten.** It does durable
intent before key access, refuses to send twice, re-acquires chain context
between every signature, and re-checks `sameChain` five times. That discipline is
the product. What changes is *presentation*: the same state machine, rendered as
a flow.

### 3.1 The gate, before the stepper

Two of the four named walls from `inspectDirectTradeSpineV1` are **market-level**
and must be resolved before a stepper is meaningful. Rendering six greyed steps
under "this market can never trade" is the flat-console failure in a new costume.

| Wall | Detail (verbatim, `lib/directTradeSpine.ts`) | Renders as |
| --- | --- | --- |
| `phase` | `this Market is {phase} — trading is only open while a Market is Open` | **No stepper.** One card: the phase, its meaning, and a link to `/markets` |
| `activation` | `this Market founded a Direct trading capability but never switched it on — no activation root exists at {root}. Activation is the operator's move, not yours.` | **No stepper.** One card, and the final clause is the remedy — keep it exactly |

The other two walls belong to steps and appear there: `prestate` → step 1,
`packet` → step 6.

### 3.2 The stepper

Seven steps, always all seven visible, each in one of five states:
`done` · `current` · `available` · `blocked` (with the reason inline) ·
`upcoming`. The reader can always see the whole shape of what they are about to
do — that is the entire point.

```
①Connect ─ ②Outcome ─ ③The other half ─ ④Size ─ ⑤Preview ─ ⑥Sign ─ ⑦Send
```

Steps ① and ② are independent and may be done in either order. ③ requires ②
(the board filters by outcome). ④–⑦ are strictly ordered. Any edit to ①–④
invalidates ⑤–⑦ — the existing `invalidatePreview()` / `invalidateWalletState()`
functions already implement exactly this and are reused unchanged.

### 3.3 Step-by-step

---

#### ① Connect — *"can you trade here at all?"*

**Absorbs `JoinPanel` entirely.** Joining and trading are one continuous need
("I want in") that the site currently presents as sections 05 and 06.

**Content.** `WalletDirectory` connect control. On connect, the readiness
read-out from `inspectDirectParticipantReadinessV1`: does a Position exist, does
the collateral account exist, spendable collateral (humanized, §5), Position
revision.

**Empty state.** `Connect a wallet to see where you stand.` (verbatim from
`JoinPanel`.)

**Error states.**

| Condition | Message | Remedy shown |
| --- | --- | --- |
| `prestate` wall | `Your wallet does not have a Claims Position on this Market yet.` | The rest of the existing sentence *is* the remedy and must survive: a devnet admission command exists, this page does not create or sign one. **Show the exact command**, as `JoinPanel` already does. This is the site's most important honest refusal — a button here would lie |
| Deployment incomplete | `This deployment does not name every program needed to authenticate your participant accounts.` | Cluster picker |
| Wallet changed mid-flow | `Your wallet changed. Ask the chain again before previewing a crossing.` | Re-read button |
| Market terminal/retiring | `joiningClosedForPhaseV1()` is already true here | Do not offer joining |

**Known constraint, stated plainly:** in-browser admission does not exist. Step ①
can *diagnose* completely and *complete* only when the wallet already has a
Position. The spec does not paper over this; the step shows a two-item checklist
with the CLI remedy against the unmet one.

---

#### ② Outcome — *"which claim?"*

**Content.** One selectable card per outcome. Each card carries: the **editorial
outcome name**, issued supply (humanized), that outcome's share of all claims
issued, and — once resolved — `won` / `lost · pays nothing`.

**The defect this fixes.** `MarketDetailWorkspace` already resolves
`editorial.outcomes` and uses the names in `CellStrip`, `SupplyShareStrip` and
the per-outcome list — and then renders `MarketTradePanel` **without passing
them**, so the one place a person *chooses* an outcome is the one place it is
called `claim 0`. Pass `editorial` into the flow. One prop.

**Empty state.** None — outcomes exist whenever the market decoded.

**Error state.** `This Market does not expose the Trading program and Product
width needed for an exact crossing.` (when `inspected.outcomeCount === null`).

---

#### ③ The other half — *"who is on the other side?"*

This is the step ember's addendum rewrites, and it gets its own section: **§4**.

In one line: **a ticket board is primary, paste is the fallback**, and the step's
contract is identical whichever transport supplies the ticket.

---

#### ④ Size — *"how much?"*

**Content.** One quantity input, labelled in the **collateral's display
denomination** — never `claim atoms` (§5). Beside it: a live "you can take up to
N" derived from the ticket's `maximumFill`, and the running cost.

**Fill-or-kill is a different control, not a validation error.** A ticket with
`lifecycle === 0` admits *exactly* `maximumFill` and nothing else
(`planDirectCrossingV1`). Rendering a free input that always refuses is the
current behaviour and it is a trap. **When the ticket is FOK, render the size as
a fixed, non-editable value with the label `All or nothing — this offer is for
exactly N.`** Only `lifecycle === 1` (IOC) gets an editable input.

**Empty state.** Blank means "take the ticket in full" — already true, currently
explained in a parenthetical inside the label. Make it the input's placeholder.

**Error states.**

| Condition | Message (verbatim, from `MarketTradePanel` / `directTicket.ts`) |
| --- | --- |
| Non-integer | `your size must be one positive whole number of claim atoms` — **restate in the display unit** (§5.4) |
| Over u64 | `your size exceeds the protocol's u64 amount width` |
| Not representable | `no admissible fill exists at or below the requested size at this exact price scale` — remedy first: show the nearest admissible size, which `largestAdmissibleFillV1` already computes |
| FOK mismatch | `the ticket is fill-or-kill for exactly {n}; a smaller fill is not admissible` — unreachable once the control above is fixed; keep the guard |

---

#### ⑤ Preview — *"what exactly happens?"*

**Content.** A receipt, in sentence order, not a grid of four equal tiles:

> **You buy 500 · Above $180** — at **35¢** per claim
> You pay **175.00 <unit>** — 175.00 principal + 0.00 fee (0 bps)
> You will hold **500** claims. If this outcome wins, they pay **500 <unit>**.
> *Checked against your assets: 175000000 required / 240000000 available,
> finalized through slot 490,712,003.*

The last line stays mono and exact — it is `execution.admission` and it is
evidence. Everything above it is the decision.

The existing four tiles (`You {side}`, `Gross collateral`, `Your fee`, `Asset
check`) move to the drawer as the exact twin, in raw atoms. **Nothing is lost.**

**Standing note, kept verbatim:** `Unsigned preview. Nothing is signed until you
continue below.`

**Error states.**

| Condition | Message |
| --- | --- |
| Outcome ≠ ticket outcome | `You picked claim {a}, but this ticket is signed for claim {b}.` — with names substituted (§3.3②) |
| Participant not ready | `Ask the chain to authenticate your participant accounts before previewing a crossing.` → jumps to ① |
| Seller cannot cover | `the ticket seller's finalized Position does not cover this fill` → belongs to ③; remove the ticket from the board |
| Self-cross | `the connected wallet is the ticket maker; a Direct fill needs two distinct makers` → ③ |
| Ticket expired | `ticket expired at slot {n}` → ③ |
| Ticket not yet valid | `ticket becomes valid at slot {n}, after the current finalized slot` → ③ |

---

#### ⑥ Sign — *"two signatures, and neither one sends"*

**Sign is one step with two ordered signatures, and the spec refuses to collapse
them,** because they are genuinely different acts:

| | **Signature A — your intent** | **Signature B — the transaction** |
| --- | --- | --- |
| What it signs | a detached Ed25519 message: your half of the trade | the exact v0 packet carrying both halves |
| Wallet prompt | "sign message" | "sign transaction" |
| Produces | a ticket — portable, yours, tradeable | a signed packet, saved locally |
| If you stop here | you have a valid signed offer, and nothing executed | the packet exists and is *still not sent* |

Render as a two-row mini-progress inside step ⑥. Row A's success state must say
what the user now *has*: `Your intent is signed. Nothing has executed.`
(verbatim, already in the `operator-required` branch — promote it to every path).

**Where the route manifest lives.** Between A and B, the flow needs an
operator-published route manifest. Today it is a 7-row textarea inside the
`<details>`, pre-filled from `publishedDirectRouteManifestV1(marketAddress)` when
one is published. **Correct behaviour:** when a published route exists, the flow
uses it and says so in one line (`Using the operator's published route for this
market.` + a `change` affordance). Only when none is published does the textarea
surface, and then it is the step's own empty state, not a drawer's.

**Error states.**

| Condition | Message | Owns |
| --- | --- | --- |
| No route | `Paste the operator-published Direct Hot route manifest before asking your wallet to sign.` | ⑥ |
| Wrong route | `route manifest authenticates another Market or Trading program` | ⑥ |
| Buy ticket | `Wallet preparation V1 accepts a portable sell ticket and your connected wallet as buyer. This buy ticket remains a valid read-only preview, but this caller will not silently reverse its participant roles.` | ③ — **filter these out of the board** so a trader never reaches ⑥ with one |
| `packet` wall | `Your measured Direct transaction is {n} bytes, above the network's 1,232-byte limit. Reduce its account or instruction geometry before signing.` | ⑥ |
| Either party not ready | `both participants must be ready before signing: seller is {s}; you are {t}` | ⑥ |
| Blockhash expired | `prepared Direct blockhash expired at block height {n}` | ⑥, with a re-prepare action |
| Chain moved | `RPC endpoint or genesis changed while the Direct route was being authenticated` | ⑥ |
| Wallet rewrote bytes | `wallet did not complete the sole required payer signature` | ⑥ |

---

#### ⑦ Send — *"once, and only once"*

**Two buttons, never one.** This is the requirement the brief names, and it is
already structurally true in the code — `signPreparedTransaction()` and
`submitDirectPacket()` are separate functions with separate buttons. What is
missing is that they *look* like one continuous ceremony because they sit in the
same undifferentiated `<details>`.

| State | Primary control | Standing line |
| --- | --- | --- |
| `wallet-preparable` | **Sign this packet** | `This request still does not submit.` |
| `wallet-signed` | **Send it** (+ secondary: copy the signed packet) | `Wallet signed · saved locally, not yet submitted` |
| `submitted` | *(none — no control exists that could help)* | the live `confirmation` string, `aria-live` |
| `executed` | **See it in the explorer** | `Executed · finalized` + the balance changes |
| `operator-required` | **Copy your signed ticket** | `Your intent is signed. Nothing has executed.` + the payer address and the exact handoff instruction |

The `operator-required` branch is a **first-class outcome, not an error.** The
trader did everything right; the route's payer is somebody else. It gets the same
visual weight as `executed`.

**The resumption promise, kept verbatim and moved up:**
`Signing sends nothing. Sending is a separate step you take, and it happens once
— reload part-way through and this page picks up the transaction you already sent
rather than sending a second one.` Today this sits as the second of three
undifferentiated status paragraphs at the top of the panel, before the reader
knows what signing or sending are. **Move it to step ⑥'s header**, where it is
about to be true.

### 3.4 The parsed ticket card

The single highest-leverage component in this spec. A ticket is 12 signed fields;
the JSON is the transport, not the artifact. On a valid parse the textarea (or
the board row) resolves into:

```
┌──────────────────────────────────────────────────────────┐
│ 7Mcu1ZT9…8WAC  offers to SELL          [signature valid] │
│                                                          │
│ 500 claims · Above $180                                  │
│ at 35¢ each  ·  you would pay 175.00 <unit>              │
│                                                          │
│ All or nothing        Valid for ~4 minutes (to slot …)   │
│ Fee 0 bps each side                                      │
│                                                          │
│ ▸ the exact signed fields                                │
└──────────────────────────────────────────────────────────┘
```

| Card element | Ticket field(s) | Rendering |
| --- | --- | --- |
| Maker | `maker` | `shortAddressV1(maker, 6)`, `title` = full |
| Direction | `intent.side` | `0` → `SELL` (they sell, you buy) · `1` → `BUY`. **State it from the reader's side too**: "you would buy" |
| Quantity | `intent.maximumFill` | humanized (§5) |
| Outcome | `intent.outcome` | editorial name, falling back to `claim {i}` |
| Price | `intent.limitPrice` ÷ `priceScale` | as a fraction of one payout — see §5.3 |
| Your cost | derived | `fill × price / priceScale`, humanized |
| Lifecycle | `intent.lifecycle` | `0` → `All or nothing` · `1` → `Partial fills allowed` |
| Validity | `validFrom`, `validThrough` | wall-clock via `deadlineMomentPhraseV1`, raw slots in the drawer |
| Fee | `intent.feeBasisPoints` | `{n} bps each side` |
| Signature | `signature` | a chip. **Says "well-formed", never "verified"** — the browser checks shape (128 lowercase hex, nonzero); only the chain verifies the signature, at the Ed25519 program. Do not let a chip claim an authority it does not have |
| Drawer | `market`, `generation`, `nonce`, `collateralAccount`, full signature, raw `maximumFill`/`limitPrice` | mono, exact, `<details>` |

**Invalid JSON.** `decodeDirectIntentTicketV1` already throws precise,
remedy-shaped errors. Render the thrown message verbatim in an inline alert
attached to the input — plus, for a parse failure specifically, a collapsed
`▸ what a ticket looks like` showing the shape from
`fixtures/direct-intent-ticket.json`. The eleven distinct refusals
(`ticket is not valid JSON`, `ticket kind is not dclutch/direct-intent-ticket/v1`,
`ticket signature must be one nonzero 64-byte lowercase-hex Ed25519 signature`,
`ticket text is empty or above its explicit 4096-byte bound`, …) all survive
unchanged.

### 3.5 Where "Advanced: full route workbench" lives

It stays, and it stops being a peer of the primary action. Today it is a
`secondary-action` anchor sitting immediately beside `Ask the chain about trading
here` in the same `.direct-actions` row — two links of near-equal weight before
the reader knows what either does.

**New home:** the flow's footer, as a single line —
`Advanced · full route workbench →` (`/trade`) — alongside
`See this market in the explorer →`. Reference depth, one click, never gone.

---

## 4. Step ③'s real answer: the ticket board

> ember: *"what can we be offering … so they don't need to be doing manual
> stuff"*

### 4.1 The paste box is the defect, not its styling

Step ③ today asks a person to obtain, out of band, a 4096-byte JSON blob signed
by a stranger, and paste it into a textarea. Every other step in this flow is
something the site can help with. This one is homework.

And the homework is currently **impossible for almost everyone**, which the
inventory makes explicit: `SESSION_STATE.md` records that *the only Direct-ticket
author in the entire repo is this browser panel*, and a tools-side ticket author
is queued but does not exist. So the paste box asks the reader to obtain an
artifact that essentially nobody can produce.

### 4.2 Why a board is trustless-safe — the invariant that permits all of this

A Direct ticket is **bearer-signed, self-authenticating data.** Every field the
transaction depends on is covered by the maker's detached Ed25519 signature, and
the chain re-derives the signing message and verifies natively. The codec's own
header says it:

> *"Anyone can carry it (a maker service, a chat message, a file); nothing about
> it is trusted until the builder re-derives the signing message and the chain
> verifies the signature. … A tampered field changes the signing message and dies
> at the Ed25519 program, so the honest failure mode is a refused transaction,
> never a different trade."*
> — `lib/directTicket.ts:15-31`

Therefore, for **any** transport whatsoever:

- **A relay can withhold. A relay can never forge.** Its worst case is censorship
  and staleness — never a wrong trade, never a stolen one.
- It is squarely inside the protocol's hard invariants. **O-016**: *"Callers may
  supply hints, witnesses, candidates, and physical accounts; release-selected
  state verifies every authoritative identity and derived effect"* — a board
  supplies candidates, exactly the permitted category. **O-007**: *"clients may
  submit untrusted witnesses."* And the product brief's own constraint —
  *"resting orders may only ever be an untrusted projection"* (`directTicket.ts:20`)
  — **is a description of a board.** A board is the permitted thing, not a
  concession.

**The design consequence, and it is the load-bearing one:** step ③'s contract is
*"produce one `SignedDirectIntentV3`, by any means."* It is transport-blind. The
board, the paste box, a URL fragment, a QR code, a WebRTC peer — all feed the
identical `decodeDirectIntentTicketV1` → `TicketCard` → step ④ path. **Build step
③ against that contract and every option in §4.4 becomes a source plugged into a
finished UI, not a redesign.**

### 4.3 The board UI

Step ③ renders, in priority order:

1. **Offers for this market and outcome** — a list of `TicketCard`s (§3.4) in
   compact form, sorted by price (best for the reader first), each with a
   **Take this offer** button that populates the flow and advances to ④.
2. **Client-side filtering, always applied before render.** The board must never
   show an offer the flow would refuse at ⑤ or ⑥. Drop: expired (`validThrough` <
   finalized slot), not-yet-valid, wrong `generation`, wrong `feeBasisPoints`,
   outcome ≥ width, **self-authored** (`maker === connected wallet`), and — until
   wallet preparation V1 accepts them — **buy-side tickets** (`side !== 0`). Each
   drop is silent in the list and countable in the drawer (`3 offers hidden — why?`).
3. **A "make an offer" affordance** — see §4.5.
4. **`▸ Paste a ticket instead`** — the fallback, collapsed. Same parse, same
   card, same everything. Never removed: it is the only path that works with no
   relay, and it is the escape hatch that keeps the whole design honest.

**Empty states, and they matter more than the populated one:**

| Condition | Message |
| --- | --- |
| No relay configured | `No offer board is configured for this deployment. You can still take an offer someone sends you directly.` (paste box expands by default) |
| Relay unreachable | `The offer board did not answer. Nothing is wrong with this market — you can still paste a ticket.` |
| Board empty | `No one is offering {outcome name} right now. You could make the first offer.` |
| All filtered out | `{n} offers here, none you can take right now.` + the drawer explaining each |

**A standing honesty line, on every board state:**
`Offers are collected by a relay, not by the chain. The chain checks every
signature when the trade executes — a relay can hide an offer from you, but it
cannot change one.`

### 4.4 The options ladder — what we can offer, and what each costs

Four rungs. They are **not alternatives**; they are a sequence, and step ③'s
contract (§4.2) makes each one a drop-in source.

---

**(a) The invariant — already true, costs nothing.** *Tickets are bearer-signed
self-authenticating data, so any transport is trustless-safe.* This is not
something to build; it is the property that licenses (b), (c) and (d). It is
already implemented, already tested, already documented in the codec.
**Status: shipped.** **Cost: zero.**

---

**(b) A ticket-board relay in the dregg infra — the short-term answer.**

A small service: `POST /tickets` (accept, validate, store), `GET
/tickets?market=&outcome=` (list). Validation is *exactly*
`decodeDirectIntentTicketV1` plus a chain check that the maker's Position covers
the offer — i.e. **the code already exists in `lib/`** and can be lifted directly.

- It holds **no keys**, takes **no custody**, and has **no authority**. Losing it
  loses availability and nothing else.
- Sweep for expiry (`validThrough` < finalized slot) and for consumed nonces.
- The web client needs one module: `lib/ticketBoard.ts`, one fetch, one parse
  loop through the existing decoder.
- **Cost:** small. The service is a few hundred lines around an existing
  validator; the client side is one module and one loading state. Its real cost
  is **operational, not engineering** — someone must run it, and the site must
  degrade correctly when it is down (§4.3's empty states are that degradation).
- **Recommended as phase 2.**

---

**(c) On-chain resting orders — already in the protocol's plan, and further along
than expected.**

`docs/OMISSION_INDEX.md`, row **U-002**:

> | U-002 | Direct inline and registered/reserved order lifecycle | unfinished successor convergence | Chain-derived operator packets, claim/custody effects, cancellation/retirement, hostile rollback, packet and CU evidence |

This is resting orders, rent-priced, fully permissionless — **the board on
chain**, needing no relay and no trust at all. **Measured status at HEAD, which
is the useful part:** the record and instruction set is not a sketch. It is
defined, decoded, and labelled.

| Artifact | Where |
| --- | --- |
| `RegisteredIntentState` — phase, controller, **maker**, embedded signed intent, **remaining**, sequence | `crates/dclutch-direct-codec/src/registered_state_artifacts_v4.rs`; decoded at `lib/explorer/accountRecords.ts:1211` as *"An order waiting on chain to be filled: who placed it, what it asks, and how much is left."* |
| `Registered create` — *"Puts a signed order on chain to rest until something fills it."* | `registered_creation_artifacts_v4.rs` |
| `Registered fill` — *"Fills two resting orders against each other."* | `generated_registered_fill_v4.rs`, `registered_fill_artifacts_v4.rs` |
| `Registered terminal` — Cancel / Expire, at an expected sequence | `registered_requests_v4.rs` |
| `Registered retire` — *"Closes a finished order and returns the rent it held."* | `registered_bundle_v4.rs` |

So the **codec, account layout, instruction set, rent model and cancellation
semantics all exist**, and the explorer can already decode a resting order if one
existed. What U-002 says is unfinished is the **successor convergence**:
chain-derived operator packets, claim/custody effects wired through, hostile
rollback, and packet/CU evidence — the same gauntlet the Direct *inline* path
went through (whose analogue is ~4,000 lines of fixture support in
`programs/dclutch-trading-sbf/program-test/direct-hot`).

- **Cost:** large, and it is a **protocol lane, not a web lane** — the same shape
  of work as the inline Hot path, with its own program-test bundle, ALT frame,
  capability seal, and CU census.
- **What it buys:** no relay, no censorship surface, rent-priced spam resistance,
  and cancellation that is a chain fact rather than a service promise.
- **The web cost is near zero**, and this is the point of §4.2: a resting order
  read from chain decodes to the same `SignedDirectIntentV3` the board renders.
  The UI built in phase 1 does not change.

---

**(d) Browser peer-to-peer — noted, not scheduled.**

WebRTC/libp2p ticket gossip between browsers. Removes the relay without waiting
for (c). Real costs: signalling still needs a server, browser peers are offline
most of the time (so the board is empty most of the time), and it adds a
substantial dependency to an app whose entire dependency list is currently seven
packages. **Note it; do not build it.** (c) is the better trust story and (b) is
the better availability story.

---

### 4.5 Making an offer — the half that does not exist

A board with no makers is an empty board, and **there is currently no UI anywhere
that authors a resting offer.** The panel authors a *taker* ticket only, as a
by-product of crossing (`encodeDirectIntentTicketV1(signedTaker)` in the
`operator-required` branch).

The maker flow is the mirror of steps ②–⑥, and it is *simpler* — it stops at
signature A:

> ② outcome → ④ size → **price** (new: the one field a taker never sets) → ⑤
> preview *"you would receive…"* → ⑥ **sign your intent** → **⑦ publish** (to the
> board) **or copy** (hand it to someone directly)

It never signs a transaction and never submits. It ends with a ticket. Everything
it needs — `encodeCompactIntentSigningMessageV2`,
`requestWalletMessageSignatureV1`, `encodeDirectIntentTicketV1`,
`inspectDirectMakerNonceV1` — **already exists and is already used by the take
flow.**

**This is the single highest-leverage addition in the whole spec**: it is the
cheapest new flow (it reuses everything and terminates early), and without it
options (b), (c) and (d) all deliver an empty board. **Schedule it with (b), not
after.**

---

## 5. The units policy

### 5.1 The rule

> **Every quantity a human decides on is shown in the collateral's display
> denomination, with thousands separators. The raw atoms are one hover or one
> drawer away, always, and are never rounded, never scaled, and never lost.**

Ember's screenshot showed `500000000` labelled `issued atoms`. At the devnet
collateral's 6 decimals that is **500**. The site has *had* the function to say
so since before this lane started — `formatAtomsV1` is pinned by a test asserting
exactly `formatAtomsV1('500000000', 6) === '500'` — and the trade panel simply
never received the decimals to call it with.

### 5.2 The formatting contract

Add `apps/dclutch-web/lib/quantity.ts`. It **wraps** `formatAtomsV1`; it does not
replace it. `formatAtomsV1` stays the exact, float-free atom→decimal-string
converter it already is.

```ts
/** A resolved display denomination for one Market's collateral. */
export type DenominationV1 = Readonly<{
  /** The mint's own decimals byte, chain-read. Null when unread or unauthenticated. */
  decimals: number | null;
  /** Editorial unit label, or null. NEVER invented — see §5.5. */
  unit: string | null;
  /** The collateral mint, for provenance and the drawer. */
  mint: string;
}>;

/**
 * Humanize an exact atom count for a reader who is about to decide with it.
 *
 * Exact and float-free: delegates the atom→decimal split to formatAtomsV1, then
 * groups the INTEGER part only. The fractional part is never grouped, never
 * padded, and never rounded.
 *
 * Fails OPEN to the truth: when `decimals` is null the atoms have no known
 * display scale, so the raw integer is returned, grouped, and the caller MUST
 * render it with the `atoms` suffix. A null decimals is never treated as 0.
 */
export function formatQuantityV1(
  atoms: bigint | string,
  denomination: DenominationV1,
): Readonly<{
  /** What a person reads. e.g. "1,250.5" or, decimals-unknown, "500,000,000". */
  display: string;
  /** The exact integer, always. e.g. "500000000". Never omitted. */
  atoms: string;
  /** True when `display` is scaled atoms; false when decimals were unknown. */
  humanized: boolean;
  /** One title/tooltip string carrying the whole truth. */
  title: string;
}>;
```

**Behaviour, pinned:**

| `atoms` | `decimals` | `display` | `humanized` | `title` |
| --- | --- | --- | --- | --- |
| `500000000` | `6` | `500` | `true` | `500000000 atoms at 6 decimals` |
| `1250500000` | `6` | `1,250.5` | `true` | `1250500000 atoms at 6 decimals` |
| `1` | `6` | `0.000001` | `true` | `1 atom at 6 decimals` |
| `0` | `6` | `0` | `true` | `0 atoms at 6 decimals` |
| `500000000` | `null` | `500,000,000` | `false` | `500000000 atoms — this mint's display precision was not read` |
| `12345678901234567890` | `6` | `12,345,678,901,234.56789` | `true` | (exact; u64-safe, BigInt throughout) |

**Non-negotiables.** No `Number`, no `parseFloat`, no `toFixed` — `bigint` and
string manipulation only, all the way down. Grouping is applied to the integer
part **after** the split, so it can never perturb the value. There is no
"compact" mode: `1.2M` is a rounded number wearing a decision's clothes, and this
site does not do that.

### 5.3 The price scale, explained exactly once

`limitPrice` is a numerator over the market's immutable `priceScale`, and
`previewDirectInlineV3` refuses any `executionPrice > priceScale`. So:

> **`price ÷ priceScale` is always a fraction between 0 and 1** — the share of one
> full payout that one claim costs. At `priceScale = 1000000`, a `limitPrice` of
> `350000` is `0.35`: **35¢ on the unit**, and equivalently the market's implied
> 35% for that outcome.

**Render it as a percentage-of-payout wherever a person decides** (`35¢` on the
ticket card and in the preview receipt), with **one** inline gloss the first time
it appears in the flow:

> *Each claim pays 1 <unit> if this outcome wins, nothing if it does not. So a
> price of 35¢ is the market saying "about 35% likely".*

`priceScale` itself (`1000000`) is **evidence, not a decision input.** It moves to
the drawer, where it currently sits in the evidence grid as
`Immutable price scale / 1000000 / from the Direct config record` — that tile is
correct and stays, one level down.

### 5.4 Every label that changes

| Today | Becomes |
| --- | --- |
| `My size · claim atoms (blank = take the ticket in full)` | label `How much`, placeholder `all of it`, suffix `claims` |
| `issued atoms` (outcome list) | `claims issued` |
| `Your claim balance` → raw | humanized + `claims` |
| `Your collateral` → raw atoms | humanized + unit; delegated amount to the drawer |
| `Gross collateral` → raw | `You pay` / `You receive`, humanized |
| `Collateral it must hold` → raw | humanized, with raw in `title` |
| `Collateral held (raw)` (the vault) | `Collateral held`, humanized; the raw keeps its own drawer row |
| `your size must be one positive whole number of claim atoms` | `your size must be one positive whole number of claims` |
| `{n} claim atoms` in `planDirectCrossingV1`'s `note` | humanized in the receipt; **the `note` string itself stays byte-identical** — it is SDK-side and pinned |

**Operator surfaces keep raw atoms as the primary rendering.** `/trade`,
`/liquidity`, `/general`, `/release`, `/local`, `/explorer` are unchanged by this
section. Their readers are checking arithmetic against a chain, and a thousands
separator in a byte offset is a hazard. The rule is scoped to *decision* surfaces.

### 5.5 The unit label — and the honest gap

**There is no name for the collateral token anywhere.** No symbol on chain, no
metadata read, nothing in the registry. So the spec **does not invent one**.

- **Registry extension.** Add an optional `collateral` block to
  `MarketEditorialEntryV1` — `{ unit: string, note: string | null }` — validated
  exactly like the existing `title`/`question`/`story` fields (trimmed, non-empty,
  unknown keys refused). It is editorial and it is labelled as editorial by the
  `MARKET_EDITORIAL_NOTE_V1` sentence the page already renders.
- **When absent:** the unit is the word **`collateral`**. Never a guessed ticker.
- The mint address stays one click away in every case, as it is today.

### 5.6 Wiring — the one real plumbing task

`MarketTradePanel` receives `liability` but **not** `hoard`, and
`mintDisplayDecimals` lives on `MarketHoardV1`, not `MarketCollateralV1`. So:

1. `MarketDetailWorkspace` resolves a `DenominationV1` once, from
   `decoded.hoard` (decimals + mint) and `marketEditorialV1(address)` (unit).
2. It passes `denomination` and `editorial` into the flow — two new props.
3. `formatQuantityV1` is called at render sites only. **Atoms remain `bigint`
   everywhere in logic.** No formatted string ever re-enters arithmetic.

---

## 6. Progressive disclosure

### 6.1 Three depths, and the test that assigns them

| Depth | Contains | Test |
| --- | --- | --- |
| **Decision** | what changes whether or how you act, in display units | *Would a different value here change what this reader does in the next ten seconds?* |
| **Drawer** (`<details>`, one click, never gone) | the evidence for the decision: raw atoms, addresses, slots, revisions, provenance chips, the protocol's own words | *Is this the proof of something above it?* |
| **Explorer** (a link away) | account bytes, field offsets, PDA derivations, instruction frames | *Is this the layout rather than the value?* |

Applied to the market page, this is exactly the GRICE market-card move (10 rows →
5 + a drawer) generalised to the whole surface.

### 6.2 Drawer law

- **One click, never two.** A drawer inside a drawer is a hiding place. The
  capability manifest currently nests `capability-drawers` inside a section that
  itself becomes a drawer — flatten it to one level, or promote the manifest to
  its own drawer sibling.
- **Every drawer is labelled with what is inside it,** not with "details" or
  "more". Existing good examples to follow: `More fields`, `Exact numbers`,
  `Each check, and its latest result`, `Developer note · the routes no leading
  magic selects`.
- **A drawer never hides a refusal.** Named refusals render at decision depth, at
  the step that owns them, with the remedy first. Their *causes*, codes and
  observed slots may live in the drawer.
- **Every drawer keeps its exact twin.** The charts already implement this
  (`<details className="viz-table">` beside every figure). Humanized value above,
  exact value inside. This is the mechanism by which §5 loses nothing.

### 6.3 The stat-card grammar, generalised

Four cards. Never five, never three.

1. **Is it live?** (status/phase, fused with anything that contradicts it)
2. **How big is it?** (the money, humanized)
3. **What does it say?** (the leading outcome, or the headline measurement)
4. **When does it end?** (wall-clock, not slots)

`/pulse` §01 already satisfies this shape with three, and `/markets` satisfies it
with four of which two (`Endpoint`, `Core program`) are *connection* facts, not
*market* facts — those two belong in the connection drawer, and the two freed
slots go to collateral-locked and markets-resolved, which the landing already
computes via `LandingPulse`.

---

## 7. The shadcn adoption plan, honestly sized

### 7.1 Static export: fine, and here is exactly why

- **shadcn/ui is not a dependency.** It is a generator that copies component
  *source* into `components/ui/`. There is no runtime, no bundle contract, and
  nothing to be incompatible with. What it *does* add as real dependencies are
  Radix primitives, `class-variance-authority`, `clsx`, `tailwind-merge`, and
  (optionally) `lucide-react`.
- **The build is `vinext@1.0.0-beta.8`**, a Vite-based Next 16 runtime, with
  `output: 'export'` under `DCLUTCH_PAGES_EXPORT=1`. Radix is client-side; every
  surface this spec touches is **already `'use client'`**, and `next.config.ts`
  states outright that *"every route is client-rendered against the selected
  chain."* Nothing here needs a server.
- **One caution, not a blocker:** the export prerenderer is the component of this
  stack with known rough edges (`basePath` is broken; see the measured comment in
  `next.config.ts`). Add one shadcn component early, run
  `DCLUTCH_PAGES_EXPORT=1` build, confirm the artifact. **Do this in the first
  hour of phase 1, before any migration work** — it is a ten-minute check that
  de-risks the whole plan.

### 7.2 The theme mapping — no token is renamed

Tailwind v4 has no `tailwind.config.js`; the theme is CSS. And shadcn's v4 idiom
is `@theme inline`, aliasing `:root` variables into Tailwind's colour namespace.
That idiom lets the **existing `:root` block in `globals.css` stay exactly as it
is** and become the theme by aliasing. This is the cheapest correct path and the
only one that keeps DESIGN's token work as the single source of truth.

```css
@import 'tailwindcss';

/* :root stays EXACTLY as written today — --ink, --acid, --fs-*, --sp-*, --r-*.
   Nothing below redefines a value; it only gives Tailwind names to what exists. */

@theme inline {
  --color-background:         var(--ground);
  --color-foreground:         var(--ink);
  --color-card:               var(--panel);
  --color-card-foreground:    var(--ink);
  --color-popover:            var(--panel);
  --color-popover-foreground: var(--ink);
  --color-primary:            var(--acid);
  --color-primary-foreground: #17200f;   /* the skip-link's proven on-acid ink */
  --color-secondary:          var(--signal);
  --color-accent:             var(--signal);
  --color-muted:              var(--panel);
  --color-muted-foreground:   var(--muted);
  --color-border:             var(--line);
  --color-input:              var(--line);
  --color-ring:               var(--acid);
  --color-destructive:        #d5c985;   /* the established refusal amber */

  --radius-sm: var(--r-sm);
  --radius-md: var(--r-md);
  --radius-lg: var(--r-lg);
  --radius-xl: var(--r-xl);

  /* The type scale is the rule, not a suggestion. Binding Tailwind's size
     names to the ten tokens means a shadcn component that ships `text-sm`
     lands ON the scale instead of beside it. */
  --text-xs:   var(--fs-micro);
  --text-sm:   var(--fs-small);
  --text-base: var(--fs-base);
  --text-lg:   var(--fs-lead);
  --text-xl:   var(--fs-h3);
  --text-2xl:  var(--fs-h2);
  --text-3xl:  var(--fs-h1);
}
```

**Dark ground and lime accent are preserved by construction** — they are the same
two custom properties, referenced rather than restated. The site is dark-only; no
light palette is defined and none is needed.

**One measured caveat.** `@import 'tailwindcss'` already ships Preflight today,
and the site's 1548 lines of CSS were authored against it, so no reset change is
introduced by this work. But **zero Tailwind utilities are currently in use** —
so this plan introduces the *first* real utility usage. For a period the app has
two styling systems. That is the honest cost of §7.4's migration order, and it is
why the order is "one flow at a time, completely" rather than "a bit everywhere."

### 7.3 Component inventory

| Need | shadcn component | Note |
| --- | --- | --- |
| The stepper (§3.2) | **none — shadcn ships no stepper** | The one component this spec needs most and shadcn does not have. Build `components/ui/stepper.tsx` over `Separator` + the five states. ~120 lines. **Budget for it explicitly** |
| Ticket card, stat cards | `card` | |
| Outcome picker (②) | `radio-group` + `card` | Selectable cards, keyboard-navigable for free |
| Size, ticket paste, route manifest | `input`, `textarea`, `label`, `form` | |
| All buttons | `button` | Maps onto the existing `.secondary-action` / primary pair |
| Raw-atoms reveal (§5) | `tooltip` | **Must not be the only path to the raw value** — touch devices have no hover. The drawer is the guaranteed path; the tooltip is the convenience |
| Drawers | `collapsible` — **conditionally**; see §7.4 | |
| Route workbench, "protocol's own words" | `sheet` | |
| Named refusals | `alert` | |
| Provenance / phase / signature chips | `badge` | |
| Send lifecycle | `sonner` (toast) | For transitions only. The authoritative state stays on the page — a trade's status must never live solely in something that disappears |
| Funding compartments | `table` | |
| Board filters | `select` | |

**Not adopted:** `lucide-react`. The site currently uses text glyphs (`✓`, `×`,
`→`, `↓`) and has no icon dependency. Adding one is a visual-language decision for
ember, not a side effect of a component migration.

### 7.4 The migration risk that decides the drawer question

This is the most important finding in §7, and it inverts the obvious plan.

**Measured:** the test suite renders with `renderToStaticMarkup` (98 call sites)
and asserts on the resulting **HTML string** (656 `toContain`, 224
`not.toContain`). There are **zero** structural DOM queries.

Two consequences, pointing opposite ways:

**(1) Restructuring markup is cheap — with one sharp edge.** No test asserts a
tag, a class, or a hierarchy, so swapping a `<div>` for a shadcn `<Card>` breaks
nothing *provided* every pinned sentence stays **one contiguous text node.**
Wrapping half a sentence in a `<span>` splits the string and fails the assertion.

> **Rule for the implementing lane:** a string that appears in a test may be
> *moved*, but never *split*. Before touching a component, grep its test file for
> `toContain` and treat every hit as a contiguity contract.

**(2) `<details>` must stay `<details>` wherever a test asserts drawer content.**
A native `<details>` renders its children into static markup **even when
closed** — so today's tests can assert on drawer content directly. Radix
`Collapsible`, `Sheet`, `Dialog`, `Tooltip` and `Toast` render **nothing** (or a
portal stub) in the closed state under `renderToStaticMarkup`. Migrating a drawer
to Radix would silently empty the markup and fail every `toContain` inside it —
and, worse, would make the 224 `not.toContain` honesty guards **vacuously pass**,
which is the dangerous direction.

**The resolution, and the repo already knows how:** there is an established
`*.opened.test.tsx` pattern (`SiteLanding.opened.test.tsx`,
`ActivityWorkspace.opened.test.tsx`, `LaunchStory.opened.test.tsx`) for rendering
a surface in a state it does not reach by default. So:

- **Default: keep `<details>`** for drawers. It is accessible, zero-dependency,
  works with no JS, and is already test-compatible. There is no reader-visible
  problem it has.
- **Use Radix `Sheet`/`Dialog` only for genuinely overlaid surfaces** (the route
  workbench), and give each one an `.opened.test.tsx` companion.
- **Never migrate a `<details>` that a `not.toContain` guard depends on.** Those
  guards are the honesty ratchet; a vacuous pass is worse than a failure.

### 7.5 What does not migrate

- **The charts.** `components/charts/` — 9 components + 2 helpers, every mark
  hand-authored JSX SVG, no external library in the dependency list at all. Their
  token layer is deliberately declared on `.viz-figure` rather than `:root` so a
  figure carries its palette wherever it mounts; their scaling problem is solved
  by `useFigureScale.ts` against a documented failure (3.2px axis labels on
  `/population`); their status trio is dataviz-validated and its intentional
  contrast exceptions are recorded in `charts.css`. **`charts.css` (115 lines) is
  not touched. No chart is rewritten.** The one permitted change is that charts
  *receive* humanized labels via §5 where they render a decision quantity.
- **`/explorer`.** 1,033 lines, ~60 readouts, five tabbed views, an audience that
  wants density. Out of scope.
- **The copy.** GRICE's plain-language pass is the site's voice. **Components
  change; strings do not, without cause.** Where this spec changes a string it
  says so explicitly and gives the reason (§5.4 is the complete list).
- **The type scale.** Ten sizes, named for the job. A sentence is sans at
  `--fs-base` or larger; a value is mono; `--fs-micro`/`--fs-tiny` never carry a
  sentence. §7.2 binds Tailwind's names to these so a shadcn default cannot open
  an eleventh size.

### 7.6 Order and cost

| Phase | Scope | Size | Gate |
| --- | --- | --- | --- |
| **0** | Install shadcn; write the `@theme inline` block; add **one** component; run `DCLUTCH_PAGES_EXPORT=1` build | **≈0.5 lane-day** | The export artifact renders and links |
| **1** | **The trade flow** (§3) + units (§5) + ticket card + board *contract* with paste as the only source | **≈6–8 lane-days** — sized in §9 | The flow completes a devnet trade end to end; every §3 error state reachable in a test |
| **2** | **The market page** (§2): stat cards, section reorder, drawers | **≈2–3 lane-days** | Nothing deleted — every old field reachable in ≤1 click |
| **2b** | **The maker flow + relay** (§4.5, §4.4b) — parallel, different lane | **≈3–5 lane-days** | An offer authored in one browser is takeable in another |
| **3** | `/markets`, `/portfolio`, `/redeem`; console directory grouping; the three console IA defects (§1.1) | **≈3–4 lane-days** | Console feature parity, exactly |
| **—** | On-chain resting orders (§4.4c) | **protocol lane, weeks** | Out of this spec's scope; U-002's own evidence bar |

These are **judgement estimates**, not measurements, and they assume the
preservation discipline in §9.1 — a rewrite of the trade logic would be several
times phase 1's number and would throw away audited code.

---

## 8. What NOT to do

1. **No rebrand.** Dark ground (`--ground: #07100c`), lime accent
   (`--acid: #b9ff64`), the mono/sans value/sentence split, the ten-step type
   scale. §7.2 preserves all of it by reference. shadcn adopts *dClutch's* theme;
   dClutch does not adopt shadcn's.
2. **No chart rewrite.** See §7.5. The hand-drawn SVG system is better than what a
   library would give and it has documented validator runs behind its palette.
3. **No operator-console feature loss.** Every input, every readout, every
   downloadable artifact survives. The consoles get *grouping* and *flow*; they do
   not get simplified. `/release` keeps all 35 readouts. `/local` keeps all four
   provenance hashes. Precision is the feature.
4. **Do not bury the honesty.** Named refusals stay named and move *up*, to the
   step that owns them. Drawers are one click and never two. The
   `not.toContain` guards are a ratchet — if a redesign makes one pass vacuously
   (§7.4), the redesign is wrong, not the test.
5. **Do not let a humanized number become the only number.** Every scaled value
   has an exact twin within one click. `formatQuantityV1` returns `atoms`
   alongside `display` for exactly this reason, and no caller may drop it.
6. **Do not invent a token symbol.** §5.5. Absent an editorial entry, the unit is
   the word `collateral`.
7. **Do not remove the paste box.** §4.3. It is the no-relay path and the proof
   that the board is a convenience rather than an authority.
8. **Do not let the board claim authority it lacks.** The signature chip says
   *well-formed*, not *verified*. The standing line says a relay can hide an
   offer but not change one. Only the chain verifies.
9. **Do not collapse sign into send.** Two acts, two buttons, two states, always
   — even when it costs a click. Especially then.

---

## 9. Phase 1 sizing — the trade flow

### 9.1 The sizing insight

**Phase 1 is a presentation refactor over preserved logic.** The eight async
orchestration functions in `MarketTradePanel.tsx` — `inspect`, `previewIntent`,
`prepareWalletIntent`, `signPreparedTransaction`, `pollDirectJournal`,
`submitDirectPacket`, plus `participantReadRequest` and the `ticketState` memo —
implement journal-before-key-access, signature-match-on-resume, never-send-twice,
and five separate `sameChain` re-checks. **That code is the product's integrity
and it must move unchanged.**

The refactor is: lift those functions into `lib/tradeFlowMachine.ts` **verbatim**,
with the `useState` unions becoming the machine's states, and render the same
states as steps. If a diff of that extraction shows a changed condition, the
extraction is wrong.

### 9.2 Files

**New (9):**

| Path | Purpose | Est. lines |
| --- | --- | --- |
| `lib/tradeFlowMachine.ts` | The three state unions + eight functions, lifted unchanged | ~450 |
| `lib/quantity.ts` | `formatQuantityV1`, `DenominationV1` (§5.2) | ~90 |
| `lib/ticketBoard.ts` | Source contract + the filter set (§4.3); paste-only impl in phase 1 | ~120 |
| `components/ui/stepper.tsx` | The component shadcn does not ship (§7.3) | ~120 |
| `components/trade/TradeFlow.tsx` | Stepper shell, step routing, invalidation | ~200 |
| `components/trade/steps/*.tsx` | Seven step bodies | ~600 total |
| `components/trade/TicketCard.tsx` | §3.4 | ~150 |
| `components/trade/TicketBoard.tsx` | §4.3, paste-fallback prominent | ~180 |
| `components/trade/PreviewReceipt.tsx` | §3.3⑤ | ~120 |

**Modified (4):** `MarketTradePanel.tsx` (becomes a thin host, or is replaced by
`TradeFlow`); `MarketDetailWorkspace.tsx` (+2 props: `denomination`, `editorial`);
`JoinPanel.tsx` (its `JoinStanding` export is already a pure props→markup
component — reuse it *inside* step ①, do not rewrite it); `globals.css` (the
`@theme inline` block; the `.trade-v3-*` rules stay until phase 3 retires them).

**Tests:** `MarketTradePanel.test.tsx` renegotiates. Its pinned strings
(`Pick an outcome, choose how much`, `Signing sends nothing.`,
`Advanced: full route workbench`, and the `not.toContain` forbidden list) must all
still pass — they are about *copy and honesty*, which this phase does not change.
New: one test per step's empty and error states, plus `quantity.test.ts` pinning
the §5.2 table, plus `TicketCard.test.tsx` including every
`decodeDirectIntentTicketV1` refusal.

### 9.3 Estimate

| Work | Lane-days |
| --- | --- |
| Phase 0 export check + theme block | 0.5 |
| Machine extraction, behaviour-identical | 1.5 |
| Stepper component + shell | 1.0 |
| Seven step bodies | 2.0 |
| Ticket card + preview receipt | 1.0 |
| Units module + decimals threading + label pass | 1.0 |
| Test renegotiation + new step tests | 1.0 |
| **Total** | **≈8 lane-days** |

Of which **≈1.5 is the risky part** (the machine extraction), and the rest is
additive presentation work that can be reviewed against a still-running old
panel. The board's *contract* is built in phase 1; the board's *content* waits on
phase 2b.

### 9.4 Definition of done

1. A devnet trade completes end to end through the stepper.
2. Every error state in §3.3 is reachable, and each renders at the step that owns
   it, remedy first.
3. No raw atom count appears at decision depth; every humanized value has its
   exact twin within one click.
4. Sign and send are two buttons with distinct states, and the resumption promise
   renders at step ⑥.
5. A ticket renders as a card on paste; invalid JSON shows the decoder's own
   verbatim refusal plus the shape example.
6. All 224 `not.toContain` guards still pass **non-vacuously** — verified by
   confirming the surrounding content is present in the rendered markup.
7. `DCLUTCH_PAGES_EXPORT=1` produces a working artifact.

---

## Appendix A — the four named walls, and who owns each

| Wall | Source | Owner |
| --- | --- | --- |
| `phase` | `directTradeSpine.ts:200` | The gate (§3.1) — no stepper |
| `activation` | `directTradeSpine.ts:236` | The gate (§3.1) — no stepper |
| `prestate` | `DIRECT_PRESTATE_WALL_V1`, `:148` | Step ① |
| `packet` | `directPacketWallV1`, `:142` | Step ⑥ |

## Appendix B — the ticket's twelve signed fields

From `decodeDirectIntentTicketV1` (`lib/directTicket.ts:88`). Every one is covered
by the maker's signature; tampering with any changes the signing message and dies
at the Ed25519 program.

`side` · `lifecycle` · `outcome` · `market` · `generation` · `nonce` ·
`validFrom` · `validThrough` · `maximumFill` · `limitPrice` · `feeBasisPoints` ·
`collateralAccount` — plus `maker` and `signature` outside `intent`.

Bounds the decoder enforces: text ≤ 4096 bytes; `kind` exactly
`dclutch/direct-intent-ticket/v1`; signature 128 lowercase hex, nonzero;
addresses canonical base58; u64 fields canonical unsigned decimal strings;
`feeBasisPoints` ≤ 10000; `side` and `lifecycle` ∈ {0,1}.
