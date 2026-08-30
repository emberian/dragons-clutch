# Decision 0014: the fee rate — the shape is already built, the band and the default are not

Status: **OPEN — ember's ruling required.** Ledger M-26, open since
2026-08-17, the oldest question in the project. This record does not freeze a
rate; see §7 for why that matters.

## 1. The question

> *"i think it would be fair to capture a modest percentage but **i don't know
> how to model the tradeoff space to figure out *what* percentage. 5%? 0.5%?
> 0.035%?**"* — ember, `01a00a3d`, 2026-08-17T07:25Z (`docs/INTENT.md` §3)

Two questions live inside it: **what number**, and **who gets to choose it**.
The second is the one that binds, and the tree has already answered it in a
direction nobody wrote down.

## 2. What the code does today

**The rate is not a protocol constant. It is a per-market, immutable,
maker-signed config field.**

- `DirectExecutionConfigV1 { price_scale: u64, fee_basis_points: u16,
  fee_recipient: [u8; 32] }` —
  `crates/dclutch-direct-codec/src/successor.rs:327-352`. Content-selected by
  digest, hostile-decoded only after descriptor-to-record selection, and
  validated at construction: `price_scale == 0 || fee_basis_points >
  DIRECT_FEE_DENOMINATOR_V1` is refused as `InvalidExecutionConfig`, and
  `require_nonzero(fee_recipient)` refuses the zero key.
- **The ceiling is 100%.** `DIRECT_FEE_DENOMINATOR_V1` is 10,000, so
  `fee_basis_points = 10_000` constructs. The Lean side pins the same bound
  (`.scalarLe (s .policyFeeBps) (s .feeDenominator)`,
  `formal/dclutch-semantics/DClutchSemantics/DirectOrdinaryV3.lean:509`, with
  `feeDenominator = 10000` at `Direct.lean:20`). That is a type range, not an
  economic bound — nothing refuses a market that takes the entire trade.
- **Zero is legal.** Only `> 10_000` is refused, and a zero combined fee is a
  no-transfer path rather than a refusal, with its reason recorded
  (`DirectRegisteredFillV4.lean:808-821`): per-fill deltas of zero are routine
  under cumulative floors, so refusing them would refuse ordinary small fills.
- **Both makers sign the exact rate.** The transition refuses `sellerFeeBps ≠
  policyFeeBps` and `buyerFeeBps ≠ policyFeeBps`
  (`DirectOrdinaryV3.lean:507-509`); both refusals are exercised
  (`docs/evidence/DIRECT_FAMILY_CAMPAIGN_2026_08_27.md:112-113`).
- **Charged as differences of floors of cumulative gross** —
  `floor(cum_after·bps/10^4) − floor(cum_before·bps/10^4)`
  (`crates/dclutch-direct-aot-v3-contract/src/registered.rs:334-352`;
  `DirectRegisteredFillV4.lean:653-706`), so matcher fragmentation cannot
  change a registered order's final fee. Proved for an *arbitrary* monotone
  fee function (`cumulative_fee_telescopes`, `DirectProofs.lean:145-153`).
- **Dealer has its own leg**: `feeDue = ceilDiv(base·num, den)`, charged as
  incremental differences of ceilings, fragmentation-independence proved
  (`DealerLiquidity.lean:192-206`).
- **General — the batch-auction family — charges nothing at all.** No fee field
  exists in its config or codec; the only mention is a disclaimer
  (`crates/dclutch-general-config-contract/src/v3.rs:89`).
- **Claims, custody and redemption charge nothing.** Zero fee terms in
  `ClaimsRepresentation.lean` and `EconomicKernel.lean` — complete-set
  split/merge and redemption of a correct claim are free. This is gen-1's
  founding objective still holding, and it should be read as already-decided.

The full three-generation recovery, the field survey, and the six candidate
geometries are in `docs/design/FEE_GEOMETRY.md` (554 lines). That study
**confirmed flat as the deliberate V1 placeholder** (§4.1, closing ledger N-1)
and explicitly left the rate to ember: *"The rate pair remains strictly-after
and ember's alone."* This record is the layer above it.

**One correction to the framing this question is usually posed in.** L7 is not
a fee-conservation law. It is the lamport-ledger delta law in the journey
harness — `payer_delta + fees + watched_growth == 0`
(`tools/gauntlet/journey/src/ledger.rs:640`) — and it *"has never once been
evaluated over a founding"* by its shape, because it differences balances by
label and a label has no predecessor the first time it appears
(`tools/lamport-ledger/README.md:18-35`). L7 constrains no rate. It is the
instrument that would **observe** one, in seven post-Open journey stages.

## 3. The reframe: three of the four decision shapes are already foreclosed

The question is usually posed as a four-way choice — fixed forever,
founder-set per market, protocol-governed, or zero-for-the-demo. Against the
tree, three of those are not open choices but reversals:

| shape | status against the tree |
|---|---|
| **founder-set per market, immutable at founding** | **This is what is built.** Ruling for it costs zero code. |
| fixed forever, one protocol rate | A reversal: it deletes a config field both makers already sign, and re-freezes what gen-1 called *"an experiment, not a natural constant."* Also the opposite of ember's *"chose the 'weakest' choice — the one most general, with the least constraining over resulting dynamics"* (`docs/INTENT.md` §3). |
| protocol-governed | A reversal on values, not cost. Governance needs a committee or a token vote — the Isometric machinery ember refused by name (*"ditching the things we think are actually extraneous (staking bullshit etc)"*, `docs/INTENT.md` §4) — and it contradicts *"No committee, no vote, no discretion."* |
| zero-for-the-demo | Available at zero code (§2), but gen-1 already ruled how it must be held if chosen: **declared, never defaulted into** (`FEE_GEOMETRY.md` §2.2, the Polymarket row). |

So the live decisions are three, and none of them is "pick the rate":

**D1 — is there a protocol cut at all?** Fees route to a per-market
`fee_recipient` bound at founding. There is no protocol-level take. That
dissolves the treasury question cleanly and serves ember's strongest value
(*"i don't want to have to operate any of this infrastructure"*,
`docs/INTENT.md` §3) — and it means **the protocol earns nothing; the founder
of each market does.** Against the motive INTENT records for the demo (*"hoping
to earn some keep so that the game can go on"*; *"turn this pile into a
*stream*"*), revenue exists only for markets ember founds. That is a coherent
answer, but it should be a chosen one.

**D2 — what bounds a founder's choice?** Today: 100%. A market can be founded
that takes the entire trade, and both makers would have to sign it, and it
would be admitted. Consent makes it non-fraudulent; it does not make it
something this venue should be able to host.

**D3 — what do the demo markets charge?** A product and narrative choice,
reversible per market, independent of D1 and D2.

## 4. Options for D2 (the only one with code cost)

| option | cost | consequence |
|---|---|---|
| **A. Leave the 100% ceiling** | zero | The venue can host a market that takes everything. Defensible on consent grounds; indefensible in a screenshot. |
| **B. Freeze a band as consts, checked at founding** | one const pair, one refusal discriminant, one Lean bound, census regeneration — the shape ADR 0012 already ran (209 codes, banded discriminants) | Forecloses the 100% market. Rate stays the founder's within the band. |
| **C. Band plus a recommended default in the founding tooling** | B, plus a default in `dclutch-release-tool` / the web founding path | Founders who do not care get a sane rate; the field stays free. |

## 5. Recommendation

**D1: keep the per-venue `fee_recipient`; take no protocol cut.** It is
built, it is the strongest form of "nothing requires a service we operate,"
and it makes revenue a property of founding markets rather than of running
infrastructure. Name the consequence out loud in the docs — the protocol has
no income; market founders do — so that nobody later discovers it as a
surprise.

**D2: option C.** Freeze `MIN_FEE_BPS = 0` and `MAX_FEE_BPS = 500` (5%) as
protocol consts checked at construction beside the existing denominator check,
with a new refusal discriminant in the Direct band. Zero stays legal because
gen-1 required zero-fee to be expressible as a declaration; 5% is the top of
ember's own stated range (*"5%? 0.5%? 0.035%?"*) and leaves every rate ember
named admissible while foreclosing the market that takes everything.

**D3: hold the devnet default at its current rate, labeled an experiment**,
and ship the per-market diversity `FEE_GEOMETRY.md` §4.2 already recommends —
distinct rates across the demo markets, with the conservation ledger showing
venue take as the sum of floor-differences over each market's life.

**On ember's actual question — 5%, 0.5%, or 0.035% — the field says 0.5% per
side is the right order, and here is the only production comparable.** Kalshi
charges a taker fee of `ceil(0.07 · C · P · (1−P))` per contract, maker side
free (`FEE_GEOMETRY.md` §2.2). At a price of 0.50 that is 1.75% of maximum
payout on one side, or **3.5% of notional traded**. A 50 bp per-side flat fee
is 0.5% of notional, **1% round trip — under a third of Kalshi's one-sided
charge at the same price.** 5% would be well above the only event venue with
real volume; 0.035% is below the noise of what a fill costs to place. Ember's
middle guess was the right one.

The same comparison exposes the one real defect flat carries on this plane.
Kalshi's charge falls as the price approaches either boundary because it is
priced on uncertainty; flat does not. At a price of 0.10, Kalshi takes 0.63%
of maximum payout while flat takes 0.5% of a notional that is now only a tenth
as large — so **identical economic risk pays a different fee depending on which
label carries it**, up to a `(S−p)/p` ratio (`FEE_GEOMETRY.md` §1.3.3). That is
complement asymmetry, it is the one gen-1 defect that survives on the bilateral
plane, and the composite geometry `FEE_GEOMETRY.md` §4.3 specifies is the fix.
It is a reason to keep the rate revisable, not a reason to delay this ruling.

## 6. What changes downstream once ruled

- **D1 ruled** → one paragraph in `README.md` and `docs/guides/trader.md`
  stating who receives fees, and ledger M-26 closes with a decision record
  instead of a third silent generation (`ASPIRATION_LEDGER.md` GITSCAN-2 §D.1
  item 5: selected in gen-1, built in gen-2, discarded in gen-3, no record).
- **D2 option B or C ruled** → one lane, protocol-tier: two consts, one
  refusal discriminant, the Lean bound beside `DirectOrdinaryV3.lean:509`, a
  refusal-census regeneration, and a positive/negative test pair. Small.
- **D3 ruled** → the founding configs for the demo markets, and the fee
  becomes sayable in the product surface, which it currently is not.
- **N-15 is not tripped by any of this**, and that is deliberate. The standing
  precondition is that *"the composite fee base's characterization is
  formalized before any rate freezes"*
  (`NEXT_WAVE_ROADMAP_2026-08-20.md:98`, ledger N-15 — unfired and unowned).
  A band, a default, and a devnet rate are none of them a freeze. Whoever
  eventually freezes a production rate inherits that gate; this record does
  not discharge it and does not let it be forgotten.

## 7. What stays refused

Carried forward from `FEE_GEOMETRY.md` §4.3 item 6, unchanged by any option
above: redemption-side fees (a tax on trusting the venue's own resolution),
source-adaptive fees (the Mango lens — a fee reading the observable that also
resolves the market), geometry-as-code in the Uniswap-v4-hook sense, floating
point anywhere, and mutation of any fee record — calibration is registered
succession, never an edit.
