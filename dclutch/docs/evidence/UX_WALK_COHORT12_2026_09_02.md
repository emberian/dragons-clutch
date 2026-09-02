# The browser, walked as a person meets it — cohort-12, 2026-09-02

**Devnet evidence. Not mainnet evidence.** A reading lane: no source was changed.

Tree root `/Users/ember/dev/dclutch`, read at HEAD `0f69918cd3ffc0f56b2b14c810b21ad991736ff6`
(the web and SDK files cited below are unchanged since `dfab77e17`, where the walk began).
Every page was rendered from HEAD by `vinext dev --port 3111` in a real Chromium
(Playwright 1.62.1, chromium-1234) against the **public devnet endpoint**
(`https://api.devnet.solana.com`, the app's default), 1280×900 and 390×844, 44 captures
plus 8 targeted follow-ups. A read-only Wallet Standard identity was injected for
participant-1 `Frvzdn6QupyCGRQEbXo7kCgkuxYLWYFfwiJzbLSdtF9Q` (cohort-12's first admitted
stranger); it connects and refuses to sign, so every signed act was read, not run.
Screenshots, inner text and control inventories live in the session scratchpad
(`scratchpad/ux/out`, `out2`); they are not committed.

## 0. The five deployment facts every row below is measured against

1. **The open market is `EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1`** (Core
   `G4Wz4fj4…`, 368 bytes, phase Open, generation 2). The browser's editorial registry
   (`apps/dclutch-web/fixtures/market-registry.devnet.json`) names **six** markets and
   **not this one**; all six are owned by `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N`,
   a Core program of a closed cohort (read live, finalized slot 491,897,522). The public
   cut (`fixtures/public-cut.devnet.json:4`) names `8Xky2yx3…`, one of those six.
   So every editorial word the site can say is about a dead market, and the live one
   renders as `Unnamed · EQnY…mGs1`.
2. **Direct trading on the open market is founded and not switched on.** The trade
   spine reads `no activation root exists at 88jJTMmUGr4tB92SwAVpNnQ5CYnWYsg19cu3ULgrZmd4`;
   capability entry 0 (the only one with a deadline) **must be activated by slot
   492,091,890 — about nine hours after capture** (`market-open.open.txt`). Past that
   slot the browser will file the market under "can never trade", which is what
   happened to the four earlier SOL/USD markets. The browser reports this correctly;
   it is not a browser defect, and it is the most consequential fact on the site.
3. Two strangers are admitted; participant-1's Position is revision 0 with `0 · 0 · 0 · 0`
   claims; no fill has executed (COHORT12_GENESIS_POPULATED §7–8).
4. No market on cohort-12 has resolved, so **nothing is redeemable anywhere**. That is a
   fact, not a UX bug; the rows about `/redeem` are about what the page *says*.
5. `public/simulator-status.json` and `simulator-series.json` were written
   2026-08-30 (git `ad2b89c0`) for market `9JwhTHyxGh…` on a closed cohort. Cohort-12's
   simulator ran (§7 of the genesis record) and published nothing the site reads.

Timing, for the record: `/markets` shows content 1.3 s after navigation and the market
page 0.8 s (public endpoint, dev server); the explorer's market lens took one `429` from
the public endpoint and still rendered 18 nodes, 0 gaps. Speed is not a finding.

## 1. What each person hits

Severity: **blocks** (the act cannot be done or the page cannot be understood),
**misleads** (a true-looking statement that is false today), **friction**, **polish**.
"Defect in": **browser** (code), **deployment** (a fixture, artifact or record the
redeploy did not move), **protocol** (the chain or tooling has no venue).

### The stranger with a wallet and no documents

| # | Page | What they see | What they needed | Severity | Where | Defect in |
|---|---|---|---|---|---|---|
| S1 | `/` aside | "the first market is open — SOL/USD — which side of the week" → `/markets/8Xky2yx3…` → **"refused: account owner differs from the selected Core program"**. The front door's only market link is dead. | A link to `EQnY…` | blocks | `components/SiteLanding.tsx:64-70` reading `fixtures/public-cut.devnet.json:4` | deployment |
| S2 | `/live` | "YES · MARKET OPEN", "Enter the live market", "Check your standing and join →" (`/markets/8Xky…#join`), "Open found transaction →" — all the closed cohort. `#join` is an anchor on `JoinPanel`'s default export (`JoinPanel.tsx:358`), which no route renders any more (only `JoinStanding` is imported, `MarketTradePanel.tsx:7`). | The open market, and an anchor that exists | blocks | `components/LaunchStory.tsx:44-47, 81` | deployment + browser |
| S3 | `/markets` | The one tradeable card: **"Unnamed · EQnY…mGs1"**, no question, "Outcomes 4", "Claims bought, per outcome 500000000 · 500000000 · 500000000 · 500000000", "Paid in J7jS…9Jfk". | What the market asks, its outcome names, when it settles | blocks (understanding) | `fixtures/market-registry.devnet.json` (no entry); `lib/marketRegistry.ts:116-120` | deployment (registry) — and see R1 |
| S4 | `/markets` | Heading **"Markets you can trade"** over a market whose trading is not switched on; the only hint is "4 capability entries · one with a deadline · ≈ in 9 h". | "Trading not yet switched on — the operator has until slot 492,091,890 (≈ 9 h)" | misleads | `components/MarketDiscoveryWorkspace.tsx:420` (heading), `:84-86` (badge line); grouping in `packages/dclutch-sdk/lib/marketDiscovery.ts` `curateMarketListingV1` | browser |
| S5 | market page § 06 | Only a button, "Ask the chain about trading here"; the wall ("founded, but never switched on") appears after the click. Everything else on the page reads on load (`MarketDetailWorkspace.tsx:330-340`). | The gate on load | friction | `components/MarketTradePanel.tsx:209-212` | browser |
| S6 | market page, wallet connected | **No wallet control anywhere.** Step ① (connect · standing · *Join this market*) is inside the stepper, and the stepper is not rendered while the market gate is closed. So on cohort-12 a stranger cannot connect, see their standing, or **join** from the market page — while the chain admits participants today (two on 2026-09-02) and `/console` advertises "Admit another participant · This browser · one wallet signature, sent from here". | Step ① outside the gate | blocks (join) | `components/MarketTradePanel.tsx:216-218` (gate) vs `:221-266` (step ①); `components/trade/MarketGateCard.tsx` | browser |
| S7 | market page, Status card | "Open — **Trading.** Put collateral in and you get one claim on every outcome…" while Direct trading is not switched on. The card fuses phase with activation only when the window has already closed. | "Open · trading not yet switched on" | misleads | `components/MarketDetailWorkspace.tsx:220-226`; phase text `lib/marketDetail.ts:29` (SDK twin `packages/dclutch-sdk/lib/marketDetail.ts:29`) | browser |
| S8 | `/portfolio`, wallet | The Position (`0 · 0 · 0 · 0`, "collateral parked rather than a stance") but **not the market collateral account**. The only surface that shows spendable collateral is trade step ① (hidden per S6), so a stranger who deposited sees no balance anywhere. | "Your collateral on this market: N" beside the Position | friction | `components/PortfolioWorkspace.tsx:57-128`; `lib/portfolio.ts` | browser |
| S9 | `/portfolio`, `/redeem` § 02 | Five paragraphs of caveats for a zero bundle: "This Position pays exactly 0 atoms whatever happens…", "Netting is a question about two positions or more…", "What this page does not compute, plainly…". | One line for a zero bundle; the caveats in a drawer | polish | `components/BundleExposurePanel.tsx` (whole); mounted `PortfolioWorkspace.tsx:210-215` | browser |
| S10 | `/redeem` hero | "**Payout is not open yet.** … Paying winning claims out is not available yet" — false since `c0bd9f53`/`eb2c6e99`/`d8b1f30f`: redemption is built and needs no file. What is true is that no market has resolved. | "No market has resolved yet. When one does and you hold the winning side, you redeem here." | misleads | `components/PortfolioWorkspace.tsx:182-184` | browser |
| S11 | `/console` § 04 | "Redeem a terminal Claims Position — Before you start · **a file this browser cannot produce**"; same on "Create the replay account". The prerequisite is derived from "the module renders a file input", and `RedeemFlow` keeps an *optional* one. | No prerequisite, or "optional" | misleads | `lib/capabilitySurface.ts:325-332`; `scripts/generate-capability-surface.mjs:249-258` | browser |
| S12 | `/activity` | A product page whose form asks for "RPC endpoint", "Claims program · required to derive Positions", "Core program · label only", "Trading program · label only"; the Market box is prefilled with dead `8Xky…`. Reading P1 + `EQnY…` works: "3 finalized transactions across 2 watched addresses". | Owner + a market picker; programs in a drawer | friction | `components/ActivityWorkspace.tsx:95-100, 176-181` | browser + deployment |
| S13 | every wallet panel | "No wallet extension found. Install a Solana wallet to connect." — no name, no link. | One link to a devnet-capable wallet and the faucet | polish | `components/WalletDirectory.tsx:187`; `lib/walletStandard.ts` | browser |
| S14 | market page at 390 px | Page `scrollWidth` 432 > viewport 390: the hero `<aside>` with the full address (`MarketDetailWorkspace.tsx:393`) reaches 406 px, so the page scrolls sideways. The nav scrolls to 667 px with its scrollbar hidden (`app/globals.css:1161`): Portfolio · Explorer · Docs · Console are off-screen with no affordance. `/markets` fits. | `overflow-wrap:anywhere` on the code; a fade or chevron on the nav | friction | `components/MarketDetailWorkspace.tsx:393`; `app/globals.css:1161-1163` | browser |
| S15 | market page | Sections numbered 00, 01, 06, 07 — the drawers took 02–05's numbers with them. | 00, 01, 02, 03 | polish | `MarketDetailWorkspace.tsx:404, 491`; `MarketTradePanel.tsx:207`; `AggregateRetirementStatus.tsx:77` | browser |
| S16 | market page § 07 | "Retirement checkpoint": "packet-bounded retirement waist", "Rust-authored four-step campaign with one durable crash journal per mutation", a **disabled** button "Retirement unavailable in this browser" — at the foot of the stranger's page. | A drawer, or the operator's page | friction | `components/AggregateRetirementStatus.tsx:77-116`, mounted `MarketDetailWorkspace.tsx:608-616` | browser |
| S17 | `/markets` card | "**Claims bought**, per outcome 500000000 · …" — nothing was bought; these are the founder's complete sets. Raw atoms on the list where the detail page already prints "500 collateral". | "Claims issued" and the humanizer | misleads / friction | `components/MarketDiscoveryWorkspace.tsx:125-128`; `lib/quantity.ts` `formatQuantityV1` unused here | browser |
| S18 | `/` pulse tile | "COLLATERAL LOCKED UP **500000000**" large, "500 at 6 decimals" small — the units policy inverted. | The humanized figure large, atoms small | polish | `components/charts/LandingPulse.tsx:137-160` | browser |
| S19 | `/markets`, `/`, `/operate` | "CORE PROGRAM · **DEPLOY-1 permanent address**"; the downloadable evidence says "The addresses are permanent" and links `docs/evidence/DEPLOY_1.md`; `/operate` says "upgraded in place at permanent addresses"; the SDK preset's provenance says "checked DEPLOY-1 record". The deployment record itself says "These ids are not permanent and nothing here should say they are" (`packages/dclutch-sdk/lib/deployments.ts:96-98`). | Cohort-12 wording, the cohort-12 record | misleads | `MarketDiscoveryWorkspace.tsx:431`; `components/PublicDeploymentEvidence.tsx:31, 49`; `components/OperatorSurface.tsx:204`; `packages/dclutch-sdk/lib/operatorSurface.ts:180` | browser |
| S20 | `/markets` (dormant) | Historical-accounts copy "352 bytes where this build expects 360" — the current width is 368. | Read the width from the constant | polish | `MarketDiscoveryWorkspace.tsx:207` | browser |

### The reader trying to learn what a market IS, from the page alone

| # | Page | What they see | What they needed | Severity | Where | Defect in |
|---|---|---|---|---|---|---|
| R1 | market page `EQnY…`, every drawer open | 1,173 words, **23 sixty-four-hex identities, 18 base58 addresses, zero "$", zero "SOL/USD"** (measured over the rendered text). Title is the address; "Settles — No settlement time is published." The page cannot say the question, the outcome names or the deadline. The chain has all three: cuts 9800/10200 over denominator 100 in the Product/ResultDomain records and the window in the source specs (genesis §4), records the explorer already derives (`lib/explorer/marketLens.ts`), and `lib/founding/rangeProtection.ts` already formats "SOL/USD < 120"-style labels. | "Where does SOL/USD finish — below $98, $98–$102, above $102? Settles ≈ <time>." derived from the records when the registry is silent | blocks (understanding) | `components/MarketDetailWorkspace.tsx:385-388` (title from editorial only), `:261-263` (Settles) | browser (protocol has the facts) |
| R2 | market page stat | "Leading outcome · claim 0 · 25.00%" when all four are 25.00%. | "No leader — nothing traded" | polish | `MarketDetailWorkspace.tsx:250-258` | browser |
| R3 | capability drawer | "Capability entry 0 · switches on by slot 492091890 · What kind it is 2f9cf505bd6a…" — the entry that decides whether this market trades has no name; none of the four are `recognized`. | "Direct trading" | friction | `lib/capabilityManifest.ts:306` (`RECOGNIZED_CAPABILITY_KINDS_V1`) | browser (kind ids are content ids; the table lacks cohort-12's) |
| R4 | `/pulse` | Nav pill "**simulator publishing**" beside the strip "**Gone quiet** — overdue for its next write"; numbers from 2026-08-30 on market `9Jwh…` (closed cohort) with the dead registry's outcome names. "Is anybody home?" — the robot last spoke three days ago about a market that no longer decodes. | Cohort-12's artifact; a pill that follows the beat | misleads | `public/simulator-status.json` (git `ad2b89c0`); `components/PulseWorkspace.tsx:387` vs `:414` | deployment (artifact) + browser (pill) |
| R5 | `/live` | "resolve and redeem come later" / "Found, join and trade fit devnet now" — `/redeem` exists; "the three steps that work today" link to the closed cohort. | Copy from the cut, cut at cohort-12 | misleads | `components/LaunchStory.tsx:24-28, 44` | deployment + browser |
| R6 | `/markets/8Xky…` (the front door's link) | Title "SOL/USD — which side of the week · refused", story "carries the first capability seal written on chain", then "account owner differs from the selected Core program, or it is executable program data". | "This market was founded by cohort-9, whose programs were closed on 2026-09-01; it stays readable in the explorer." | misleads | `components/RefusedMarketStory.tsx:20-33`; registry `story` fields | deployment + browser |
| R7 | `/smoke`, `/bounty` (from `/live`'s finale) | "Not live yet — none of these three smoke markets exists yet", "No market is open, so there is none to close" — beside `/live`'s "YES · MARKET OPEN". | One source for "is a market open" | polish | `components/SmokeStory.tsx:35`; `components/BountyWalk.tsx:56, 60` | browser |

### The operator founding and running a market

| # | Page | What they see | What they needed | Severity | Where | Defect in |
|---|---|---|---|---|---|---|
| O1 | `/workbench` with `EQnY…` observed | Every Author & fund act reads **READY TO PREFLIGHT** — "Activate a checked multiprogram release", "Found a Market and admit its first participant" — for a market that is already Open; Market: "Core-owned / unclassified"; Release: "unrecognized until route preflight". The page promises "a read-only map of where a market has got to" (`:362`); the verdict is "was a chain read, was a market named". | Verdicts from the decoded phase, activation and settlement | misleads | `components/MarketWorkbench.tsx:386-403`; `packages/dclutch-sdk/lib/capabilityModel.ts:397-417` | browser/SDK |
| O2 | `/create` step 01 | Defaults band **12000–18000** with founding observation **15000** — SOL at $150, the stale numbers cohort-11 founded on; spot is ≈$100 (genesis §4, three venues and the sponsored PriceUpdateV2 `7UVimffx…`). The observation is typed, never read. The gate admits the default because the belief fields are tuned to it. | Read the sponsored feed for the observation; centre the default cuts on it | misleads (would re-found an unfillable market) | `components/CreateMarketWizard.tsx:123-127, 381` | browser |
| O3 | `/create` step 05 | 14 address inputs, 12 empty and `required`, "Payer (the connected wallet)" with no wallet control on the page; the same 14 as `/found`. | The derive gap OPERATOR_FORMS_V1 §1.3 names | friction | `CreateMarketWizard.tsx:75-90, 602-607` | browser |
| O4 | `/resolution`, `/workbench`, `/operate`, `/activity` | "Core Market" / "Market · optional" must be **pasted**; the enumeration `/markets` performs is not offered. `/console` sends the operator to `/markets` to find one and then to a console to paste it. | One market picker with the registry title beside each address | friction | `components/ResolutionWorkspace.tsx:910`; `MarketWorkbench.tsx:384`; `OperatorSurface.tsx:208`; `ActivityWorkspace.tsx:181` | browser |
| O5 | market page gate | "Activation is the operator's move, not yours" — names no console and no command. `/console` lists no act that activates a market capability; the catalogue has `release.activate` only. The operator has until slot 492,091,890. | The runbook or console that activates the Direct capability, named in the gate | blocks (the operator cannot find the act from the browser) | `lib/tradeFlowSteps.ts:938-941`; `components/trade/MarketGateCard.tsx`; `packages/dclutch-sdk/lib/capabilityModel.ts` catalogue | protocol/tooling (no browser venue) |
| O6 | `/trade` § 03 | "Execution remains closed — Direct execution reopens here once the caller persists the exact signed bytes…" under a header that says "WHERE TRADING HAPPENS · Markets". | Delete § 03 or say "execution lives on the market page" | polish | `components/DirectTradeWorkspace.tsx:227-228` | browser |
| O7 | `/operate` § 03 | 27 acts × (venue, guarantee, prerequisite, wall) in a four-column grid ≈ 6,100 px tall; the walls are the content, the rest repeats `/console`. The preset + inspect flow itself is good: "Checked devnet preset matched finalized chain state at slot 491,901,064 · 6 executable roles". | Walls first; the rest collapsed | friction | `components/OperatorSurface.tsx:215-222` | browser |
| O8 | `/local` | "Refused: Failed to fetch" — the browser's words for "no validator at 127.0.0.1:20890". Expected here; the refusal is unauthored. | "No validator answered at 127.0.0.1:20890" | polish | `components/LocalSuccessorWorkspace.tsx:63-65` | browser |
| O9 | `/release`, `/found`, `/product-v2`, `/general`, `/liquidity` | Honest and gated: artifact inputs name their producer and report what they received; program and cache fields are filled from the deployment and say so; signing gates are closed with the reason. Nothing beyond OPERATOR_FORMS_V1's inventory. | — | — | — | — |

## 2. The ten changes, ranked

Hours are for one lane at HEAD, tests included.

| Rank | Change | Rows | Hours | Lives in |
|---|---|---|---|---|
| **1** | **Move the site's editorial to cohort-12.** Public cut → `EQnY…` and its founding signature; a registry entry for `EQnY…` (title, the $98–$102 question, four outcome names, resolution, story) and a `story` on the six dead entries saying which cohort closed them; drop `#join`; re-run `og-cards.sh`. Every fixture the redeploy did not move. | S1 S2 S3 R5 R6 S12 | 1.5 | `fixtures/public-cut.devnet.json`, `fixtures/market-registry.devnet.json`, `components/LaunchStory.tsx:81`, `scripts/og-cards.sh` |
| **2** | **Render step ① outside the activation gate.** The gate closes ②–⑦; connect, standing and *Join this market* stay, because the chain admits participants before Direct is switched on. Show spendable collateral there and on `/portfolio`. | S6 S8 | 3 | `components/MarketTradePanel.tsx:216-266`, `components/PortfolioWorkspace.tsx:57-128` |
| **3** | **Derive the question from the records when the registry is silent.** Decode the ResultDomain cuts and denominator, the Product coordinate label, and the window end; render "Where does SOL/USD finish — below $98, $98–$102, above $102?" with named outcomes and a Settles time. The registry becomes an override, not the only author; it has lagged every redeploy. | R1 S3 R2 | 6–8 | `components/MarketDetailWorkspace.tsx:385-388, 261-263`, a new `lib/marketQuestion.ts` over `lib/explorer/marketLens.ts` and `lib/founding/rangeProtection.ts` |
| 4 | Fuse activation into Status, the list, and the gate on load: "Open · trading not switched on until slot N (≈ time)"; list heading "Markets" with the tradeable count as a sub-line; inspect the spine on mount. | S4 S5 S7 | 3 | `MarketDiscoveryWorkspace.tsx:84-86, 420`, `MarketDetailWorkspace.tsx:220-226`, `MarketTradePanel.tsx:209-218` |
| 5 | Name the operator's remedy in the gate: the CLI runbook (or console) that activates the Direct capability, with the deadline slot and its wall-clock phrase. | O5 | 2 | `lib/tradeFlowSteps.ts:938-941`, `components/trade/MarketGateCard.tsx` |
| 6 | Publish cohort-12's simulator status and series; the pulse pill from `beat.state`. | R4 | 2 (+ the simulator run) | `public/simulator-status.json`, `public/simulator-series.json`, `components/PulseWorkspace.tsx:387` |
| 7 | Retire "permanent" and DEPLOY-1 in five places; read the market width from its constant. | S19 S20 | 1 | `MarketDiscoveryWorkspace.tsx:207, 431`, `PublicDeploymentEvidence.tsx:31, 49`, `OperatorSurface.tsx:204`, `packages/dclutch-sdk/lib/operatorSurface.ts:180` |
| 8 | `/redeem` hero says what is true (no resolved market yet); the console's file prerequisite learns "optional". | S10 S11 | 1.5 | `PortfolioWorkspace.tsx:182-184`, `lib/capabilitySurface.ts:325-332`, `scripts/generate-capability-surface.mjs:249-258` |
| 9 | Workbench verdicts from decoded state (phase, activation, settlement — the snapshot already holds the bytes). | O1 | 4 | `packages/dclutch-sdk/lib/capabilityModel.ts:397-417`, `MarketWorkbench.tsx:386-403` |
| 10 | One `MarketPicker` (enumeration + registry title) for `/resolution`, `/workbench`, `/operate`, `/activity`; wizard defaults read from the sponsored feed; the two mobile overflows. | O4 S12 O2 S14 | 3 + 2 + 1 | new `components/MarketPicker.tsx`; `CreateMarketWizard.tsx:123-127`; `MarketDetailWorkspace.tsx:393`, `app/globals.css:1161` |

**First three: 1, 2, 3.** One is an hour and repairs every link a stranger can click today.
Two is the only row where the browser refuses an act the chain accepts. Three is the
one that survives the next redeploy, because the registry has now been stale for every
cohort since it was written.

And before any of them: **fact 2** — the open market's Direct capability expires at
slot 492,091,890. If nobody activates it, rows S4–S7 stop mattering and the site gains a
fifth "never trades" market.

## 3. What could not be rendered or reached, and why

- **The static Pages artifact.** Nothing was built with `tools/genref/render-site.mjs`
  or checked with `pages-nav-check.mjs`; the served build was walked instead. The
  `404.html` shell for `/markets/<address>` (`app/not-found.tsx`) was therefore not
  exercised — on the served build `/markets/EQnY…` renders the detail page directly
  (follow-up `permalink-markets-slash`: renders, decoded at finalized floor).
- **Anything past a wallet prompt.** The injected wallet is a read-only identity, so
  join (`AdmitInThisBrowser`), sign A/B, send, replay creation and payout were read in
  `JoinPanel.tsx`, `trade/steps/*.tsx`, `RedeemFlow.tsx`, `lib/tradeFlowMachine.ts`, not
  run. On cohort-12 the whole trade flow past the gate is unreachable anyway (fact 2),
  and nothing is redeemable (fact 4).
- **The ticket board and the maker composer.** No `NEXT_PUBLIC_DCLUTCH_TICKET_BOARD`
  is configured (paste path only), and both sit behind the closed gate.
- **`/local`** needs a validator at `127.0.0.1:20890` (refused, expected).
- **The Custom cluster dialog** and the docs links (which point at the repository in
  a dev build and at `/docs` only in the Pages artifact) were not followed.
- One `429` from the public endpoint during the explorer's market lens; the page
  completed. No 4xx/5xx from the app itself on any route; the only console noise is
  Vite's `buffer` externalisation warning on every page.
