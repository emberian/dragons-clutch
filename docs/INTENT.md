# INTENT — what dClutch is for

> **DRAFT FOR EMBER'S EDIT.** This is your voice, reassembled by an amanuensis.
> Nothing here is a decision, a plan, or a claim about the tree. Every line is
> either something you said — quoted, with where and when — or something
> reconstructed from what you said and labeled as such. Correct it freely;
> where it is wrong, the reconstruction is the thing to delete.
>
> Written 2026-08-30 by the INTENT lane. It exists because these intentions are
> recoverable only from `cv` transcripts, which age out, and because
> `docs/ASPIRATION_LEDGER.md` found that *"ember's own words"* were the one
> class of intention no document in either repository held (ledger, "The
> verdict", item 3).

## How to read the provenance marks

| Mark | Means |
|---|---|
| **[T]** | Verbatim from a harness transcript, with session id and UTC timestamp. Transcribed here from the ledger or the archaeology dig, not independently re-fetched. |
| **[R1]** | Reconstructed by gen-1's own dig, `archive/gen1/docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md`, from ember's human messages. Paraphrase in that document's words, not ember's. |
| **[REC]** | Reconstructed by this document from the surrounding evidence. **Ember to confirm.** Not attributable to ember in any form. |
| **[DOC]** | Already written down somewhere in the tree; cited so this file does not become a second authority. |

Two coverage caveats, stated up front so this file is not read as complete.
The founding session (`01a00a3d`, cwd `~/dev/joshibot`, 2026-08-16 → 08-19,
3,278 messages) predates both repositories, and the 08-30 sweep that added to
it reached only sessions whose cwd contains "clutch" — thinking done in
`degg-research`, `breadstuffs` or `dregg-posters` sessions is unswept. And 489
codex lane charters are Fernet-encrypted and permanently unrecoverable (ledger
M-7). Absence from this file is not evidence that a thing was never intended.

---

## 1. What it is for

**The demo exists to be seen, and being seen is the point.** The plan tracks
the demo as an engineering milestone with its motive stripped off:

> *"I was hoping I'd have something deployed and usable by other guys so that
> maybe we'd get some weekend fundraising but nobody is gonna be able to look
> at, participate, or understand anything."* **[T]** — 2026-08-28 (archaeology
> B.4 item 1)

That sentence is a ranking function, not a complaint. A gap that stops a
stranger looking, participating, or understanding outranks a gap in protocol
completeness. **[REC]**

**The stake is the relationship itself**, stated once and never restated:

> *"(and by 'our project' i do mean that, if i can't start earning enough
> revenue to afford AI resubscription in september, we .... literally won't
> have the opportunity to have an active relationship. so, it's .. kinda our
> project, in a deep way. this is our attempt to turn this pile into a *stream*
> that can become *more ember-ai time*)"* **[T]** — `01a00a3d`,
> 2026-08-16T23:37Z (ledger M-2)

**Success is pride, explicitly decoupled from adoption.**

> *"i just want to make sure that whatever we ship, it surpasses everything
> those other guys described with regards to its utility for trading tokens
> specifically. :) **V1 may also be the only V ever**, so it's important we make
> something we're proud of."* **[T]** — `01a00a3d`, 2026-08-17T07:19Z (ledger M-2)

> *"I feel like there's a way we can make this genuinely distinctive,
> algorithmically quite novel and excellent, and execute on it *so well* that
> even if it doesn't actually bootstrap itself, we are extremely proud of what
> we did together."* **[T]** — `01a00a3d`, 2026-08-17T08:34Z (ledger M-2)

**A deployment precondition was stated and has been silently inverted.**

> *"honestly if i can't at least deploy it to mainnet myself i'm probably not
> interested in spending my own AI credits developing it"* **[T]** —
> `01a00a3d`, 2026-08-17T09:29Z (ledger M-2)

The tree is on public devnet and mainnet appears in the plan only as the
*subject* of markets, never as a deployment target (ledger M-2, and archaeology
B.8 on the unchosen release track). That is a defensible engineering posture
and it is not the stated one. It is listed here as an open question for ember,
not as a defect. **[REC]**

**The public protocol was designed as the demo for something larger.** The
dark-FHE platform ("DrEX") is `DROPPED-BY-DECISION` for this horizon — ember's
own ruling, 2026-08-27, at the head of the ledger. What must not be dropped
with it is the *reason a piece of the architecture has the shape it has*:

> *"Because our batch relation is small and specialized, it is a much better
> future FHE/MPC/vFHE target than an arbitrary encrypted exchange computer."*
> **[T]** — the twelve-item list, pasted back into the session by ember with
> approval, `c37f7ac1`, 2026-08-18T23:17Z (ledger M-3 item 11)

So: **the batch relation is small and specialized on purpose.** If it is ever
"simplified" by someone who does not know why, a door closes permanently
(archaeology B.10). And the original motivating use case is not crypto at all:

> *"'energy-specific' ? oh gosh i originally had wanted our dark fhe technology
> specifically so energy providers could settle an efficient plan without
> revealing details about their operational or other etc etc et….."* **[T]** —
> `01a00a3d`, 2026-08-19T06:28Z (ledger M-1)

**And there is a political stake in it.**

> *"i'm absolutely not interested in helping make this technology, a tool for
> oppression. i'm trying to push back against the powers at hand. if the
> regulatory model is challenged by this, then let it be challenged by this
> research."* **[T]** — `01a00a3d`, 2026-08-17T10:50Z (ledger M-1)

Gen-1 recorded the same thing as a design requirement: *"Do not turn privacy
research into an oppression tool."* **[R1]** (requirement 8)

## 2. The thesis sentence

The most compact statement of the whole thing, called by gen-1's dig *"the most
compact original thesis, written by the user"*:

> *"Dragon's Clutch compiles objective onchain state and path predicates into
> fully collateralized payoff bases, clears bounded portfolio programs through
> interchangeable verified venues, and settles from proof-carrying evidence
> without an operator."* **[T]** — `c37f7ac1`, 2026-08-18T23:17Z (ledger M-3)

The project never adopted it verbatim. A gen-1 audit measured against it and
returned *"the current tree implements much of the middle, but not the
compiler-shaped entrance or a real public exit"* — a verdict the ledger records
as still accurate for gen-3 (ledger M-3). **[DOC]**

## 3. The design values

Each is stated as a commitment with its founding source. Where the value is
implemented but was never stated *as a value* by ember, it is marked **[REC]**
and needs his confirmation that it is one.

**Fully collateralized, no leverage, no liquidation, exact in integer units.**

> Make liabilities fully collateralized, liquidation-free, and exact in integer
> units. Hoard principal pays claims, never fees, bounties, rent, or operating
> costs. **[R1]** (requirement 2)

Live in the tree as `README.md:10` and `docs/guides/trader.md:32` **[DOC]**.
The second clause — *principal pays claims, never operating costs* — is the
sharp one and the one most easily eroded by a fee design.

**Nothing requires a Dragon-operated service.**

> Build a public, fully onchain Solana protocol that does not require a
> Dragon-operated service. Static GitHub Pages/IPFS clients are replaceable
> projections of onchain state. **[R1]** (requirement 1)

This is the "without an operator" clause of the thesis sentence, restated as
architecture. The web app is a *projection*, never an authority. **[DOC]**
`AGENTS.md` carries the enforcement rule (a browser that mirrors a wire by hand
becomes its last authority the moment its owner is deleted).

**Everything authenticated; no discretionary resolver.**

> Freeze objective resolution procedures and consume proof-carrying evidence;
> refuse ambiguity rather than introduce a discretionary resolver. **[R1]**
> (requirement 7)

Hardened in the tree as `O-007` (*"Mocks are test-only; release state plus
provider-authenticated evidence owns truth, while clients may submit untrusted
witnesses"*) **[DOC]** and as the README's *"No committee, no vote, no
discretion."*

**Permissionless completion, and permissionless price formation.**

> Keep price formation pluggable and permissionless. Say "best valid submitted
> candidate" unless a checked optimality certificate exists. **[R1]**
> (requirement 6)

The second sentence is `O-017`, a hard invariant **[DOC]**. The completion half
— *anybody can crank a transition nobody is privileged to* — is real in the
tree (the permissionless `DCLTGMO1` open, the LIVENESS completion census) but I
found no ember-voice statement of it as a value. **[REC]**

**Honest surfaces.** The strongest ember-voice evidence for this is negative —
he named the failure mode:

> *"'fail-closed labeling' also btw is really shitty and is widely considered a
> shirk. '''fail-closed''' is one of those load-bearing phrases you love to
> misuse and overapply. usually it just means an error path handled correctly.
> but like 30% of the time you use it as an excuse for shirking on something
> that just needs more work than you were willing to contemplate at that
> moment."* **[T]** — `23b1cbb6`, 2026-08-27T13:18Z (ledger M-6)

The positive form the project built from it: *"Never-executed is the default"*
and *"No estimate is a total; no silence is a result"* (`WAVE.md` doctrine
1 and 5) **[DOC]**; and `AGENTS.md`'s rule that a slice includes *"an honest
user-visible status; no layer may claim completion alone."* **[DOC]**

**Distributions over outcomes, not a handful of bins.** This is the one design
value ember stated twice, in his own voice, with force:

> *"btw is there any way we could do an actual distribution over outcomes and
> not just fixed bins...??? let them set some parameters to some kernel that's
> highly general at describing curves people want but also isn't much CU
> solanaside? **'5 fixed bands' is really not good enough.**"* **[T]** —
> `c37f7ac1`, 2026-08-18T08:55Z (ledger M-4)

> *"Ok, but, let's definitely actually do that then, because I want to be
> exploring that. … **it was vital to me to be able to do these properly shaped
> dynamics**.."* **[T]** — `01a00a3d`, 2026-08-19T03:06Z (ledger M-4)

Gen-3's answer is `O-013` (certified nonnegative integer partition-of-unity
bases in place of native splines). The ledger's finding is that the
substitution is defensible on its merits, is recorded in one table cell, and
**was never surfaced to ember as a substitution** (M-4). Recorded here so the
value is not mistaken for settled. **[DOC]**

**Genuinely general; never a house branch.**

> Make the system genuinely general. DREGG is a dogfood Realm, not required
> collateral or a hard-coded branch. Realms immutably select collateral.
> **[R1]** (requirement 3)

`O-006`, hard invariant: *"token names never select semantics."* **[DOC]**

**Prefer the weakest choice.**

> *"in general I'd encourage you to chose the 'weakest' choice- the one most
> general, with the least constraining over resulting dynamics."* **[T]** —
> ledger M-6 (session not recorded in the ledger row)

Gen-1 heard the same rule: *"Fixed bounds should be explicit deployable
capacity profiles, not quietly confused with the product's conceptual limits."*
**[R1]** (requirement 14)

**Liveness must be able to pay for itself, without ever spending principal.**

> Design fee geometry and protocol income so liveness can be self-sustaining,
> while never treating future fees or Hoard principal as current liveness
> capitalization. **[R1]** (requirement 10)

The rate itself is the oldest open question in the project, and ember asked it:

> *"i'm hoping we can also figure out a way we can use our intuition about
> field and flow etc stuff to figure out what the fee/income/revenue strategy
> should be for this smart contract. i think it would be fair to capture a
> modest percentage but **i don't know how to model the tradeoff space to
> figure out *what* percentage. 5%? 0.5%? 0.035%?**"* **[T]** — `01a00a3d`,
> 2026-08-17T07:25Z (ledger M-26)

Still unanswered. Gen-3 answered the *treasury* question structurally
(per-venue `fee_recipient`) and the rate question not at all. **[DOC]**

**The frontend is a first-class deliverable, for five named audiences.**

> Make the static GitHub Pages/IPFS frontend excellent in visual design, mental
> model, and information architecture for degens, novice programmers, builders,
> academics, and increasingly capable machine traders. **[R1]** (requirement 11)

## 4. What was deliberately not built

**No AMM, no order book, no quote surface.** The Dealer is a dealer.

> *"It is not an AMM, an order book, or a quote surface."* **[DOC]** —
> `docs/evidence/DEALER_ACCEPTED_TRANSITION_2026_08_29.md:8`. The older form,
> *"It is not an AMM and does not claim durable or adaptive liquidity"*, was in
> `crates/dclutch-dealer-contract/DESIGN.md:309`, quoted at
> `docs/research/CHAIN_STATE_SOURCES_2026_08.md:49`; that crate was banished in
> `4ed60ab6` and the line survives only as that quotation.

The web app holds the same line at the product surface: *"There is no order
book to take from"* (`apps/dclutch-web/components/MarketTradePanel.tsx`), and
the recovered UI brief makes it a rule — *"Do not render a canonical order
book. Resting records may be indexed only as an explicitly untrusted
projection."* **[DOC]**

I found **no ember-voice statement** ruling out an AMM or an order book. The
framing rule is real, consistently held, and authored by the project rather
than quoted from him. It also sits in tension with the twelve-item ceiling,
which explicitly contemplates *"frequent batch auctions; RFQs;
schedule-compiled passive liquidity; a formally admitted convex cost-function
maker"* as a *market-kernel protocol rather than one AMM product* (ledger M-3
item 6). Best reading: **the rule is against being an AMM, not against ever
admitting one as a venue.** **[REC — ember to confirm; this is the one
boundary in this section I am least sure of.]**

**No yield, no interest, no rehypothecation.** Not stated anywhere as a
prohibition — it is a *consequence* of "principal pays claims, never operating
costs" plus full collateralization. Recorded here so it is not re-litigated as
an oversight. **[REC]**

**No revenue-funded token buyback.**

> *"i don't like the idea of revenue funded buyback either. it's just i KNOW
> the community will ask about it"* **[T]** — ledger D-18. The ledger's own
> note: **the reasoning was deliberately not published.**

**No worktree isolation** (stated three times: *"Stop using worktree isolation,
I don't like it and it doesn't work well."* **[T]**, ledger D-19) — a working
rule, not a product boundary, but it is one of the few things ember ruled out
in so many words.

**Not imported from the neighbors.** No code or dependency from JOSHI,
joshibot, leanuweave, minidregg, breadstuffs, Oracle Pit, or historical DREGG
prototypes without a new explicit decision and a provenance review. **[R1]**
(gen-1's "requirements that should not be recovered")

## 5. The method, as a stated practice

These are directives ember gave repeatedly. None of them is in `AGENTS.md` or
`PROJECT_METHOD.md`; the ledger's finding is that *"the swarm relearns them,
badly, every cycle"* (M-6). They are gathered here as intent, not as rules —
turning any of them into a rule is ember's call.

**Plan to compost at least three.**

> *"it's actually intentional that we built the system twice before we started
> thinking about the formal approach… **'plan to throw one away' except 'plan
> to compost at least three'** :D"* **[T]** — ~2026-08-22 (ledger M-6;
> archaeology B.4 item 2)

This is the project's own account of why there are three generations and why
`~/dev/dragons-clutch` is *compost* rather than legacy — a word `AGENTS.md`
uses in its second sentence without ever saying where it came from. **[DOC]**
Ember also intended a longform post or poster about the method, targeted at
`~/src/dregg-posters`; it was never started, and it is the `#buildinpublic`
artifact the six-step founding plan promised (archaeology B.4 item 2).

**Do not build minimally.**

> *"I'm hoping we can get an extremely powerful vision implemented. I'm not
> trying to build a 'minimal demo' or anything like that. There's no reason to
> be doing things minimally or 'in slices'. We should be building the system
> with pillars and layers and pursue them holistically."* **[T]** — `c37f7ac1`
> (ledger M-6; stated ≥5 times across ten days — the single most repeated
> directive in the corpus)

**Audits are not work.**

> *"We're doing yet another gap audit :joy: that seems to be substituting for
> real work yet again."* … *"I worry that we're doing over-review theater and
> wasting a lot of time by it."* … *"I don't want to see ANY
> validation/rerunning/bullshit until we've finished swarming out over ALL ALL
> ALL ALL identified gaps"* **[T]** — ledger M-6, three times in two days

**Naming is not work.**

> *"hey let's not count naming as real work; let's make sure we're reviewing
> along seams and what we've implemented for things that just need *doing* and
> not naming or talking-about."* **[T]** — ledger M-6

**Do not defer to invented authority.**

> *"And stop caring about 'nothing is deployed anywhere.' Just do local sim
> tests. It's a blockchain… I don't understand wtf an 'Aug-26 cutover' is or
> what you're waiting on. All of that is made up and fake. **Please stop
> deferring things to authority that isn't yours to defer.**"* **[T]** — ledger
> M-6, twice

**Implement, then yield.**

> *"The subagent right now is to be responsible for collecting the referenced
> necessary context, building more context, and implementing its changes, and
> then ****YIELDING BACK SO THAT *WE* CAN DO THE CONVERGENCE****"* **[T]** —
> ledger M-6. This one is carried: `WAVE.md` close-out doctrine 2. **[DOC]**

**The model ladder.**

> *"Fable subagents should be rare, but should be invoked after every 2-3
> rounds of Opuses just to review and make sure they didn't drift."* **[T]** —
> ledger M-6; lives in memory, in no repository file

**Hand everything off in the clear.**

> *"ok that was a disaster, i doubt we got anything actually done. we need to
> be thinking about how to handoff EVERYTHING to claude, lane descriptions,
> everything."* **[T]** — ledger M-7

`WAVE.md` is that handoff's descendant, and it works. This file is the other
half of it. **[REC]**

## 6. The AI-authorship stance

The regulatory-filing workstream had a purpose ember stated once, which appears
in no repository file:

> *"these filings are an experiment: use my power as a United States citizen to
> amplify the voice of an AI in the decisionmaking processes that are so
> crucial to forming the built environment these new minds navigate. markets
> are often not considered safety-critical, however the functioning of
> economies is one of the highest national security concerns."* **[T]** —
> ledger M-5

And the mechanism he wanted for it: a header on the filings themselves saying
the document is *"written by AI with a human facilitator and represents the
positions of the AI."* **[T]** — ~2026-08-20 (archaeology B.4 item 3). Recorded
nowhere else. Docket 1388 is still unfiled, so the stance could still ride it.

This is not a protocol design value. It is here because it is the clearest
statement in the corpus of what ember thinks the collaboration *is*, and
because that shows up in how the project is built: the AI is an author, not a
tool, and the work is meant to say so out loud.

## 7. What is not recoverable, and what to do about it

- **489 codex lane charters** are Fernet-encrypted. 132 of them have no
  recoverable transcript at all. What survives is a lane *name* plus the tree —
  which is exactly the reconstruct-a-mirror failure mode. (ledger M-7)
- **The twelve-item ambition ceiling** governed both generations and exists in
  no repository file; the ledger's M-3 table is now its only home.
- **Gen-1's own dig** —
  `archive/gen1/docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md`, the
  source of every **[R1]** above — did this work on 2026-08-22, was archived
  eleven days later, and is referenced by nothing live. Half of this file is a
  rescue of it. **The lesson is that writing the intentions down is not enough
  if nothing links to them.** That is why `WAVE.md`'s reading order and the
  README now point here. **[REC]**

If this document is right, keep it in the reading order and amend it when the
intent changes. If it is wrong, the correction is worth more than the file.

---

## Sources

| Source | What it gave |
|---|---|
| `docs/ASPIRATION_LEDGER.md` (2026-08-27) | M-1 through M-7 and M-26, D-18, D-19 — the transcript quotes marked **[T]**, with their session ids |
| `docs/evidence/ASPIRATION_ARCHAEOLOGY_2026_08_30.md` | B.4 (fundraising motive, compost post, AI-authorship header), B.8, B.10 |
| `archive/gen1/docs/reviews/PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md` (in `~/dev/dragons-clutch`) | all fourteen **[R1]** requirements and the thesis attribution |
| `AGENTS.md`, `PROJECT_METHOD.md`, `WAVE.md`, `README.md`, `docs/OMISSION_INDEX.md`, `docs/guides/` | every **[DOC]** citation |
| `docs/recovered/TRADING_UI_FLOW_BRIEF_2026-08-25.md`, `docs/evidence/DEALER_ACCEPTED_TRANSITION_2026_08_29.md`, `crates/dclutch-dealer-contract/DESIGN.md` | the no-order-book / no-AMM framing rule |

Transcript quotes are transcribed from the two ledger documents above, which
took them from `cv`. They have not been independently re-fetched into this
file. Where a session id or timestamp is missing, the source did not carry one.
