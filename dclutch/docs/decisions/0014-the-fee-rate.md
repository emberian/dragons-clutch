# Decision 0014: the fee rate — the shape is already built, the band and the default are not

Status: **RULED 2026-08-30, all three; none of the three is built yet.** Ledger
M-26, open since 2026-08-17, the oldest question in the project. This record
does not freeze a rate; see §6 for why that matters.

- **D1 — DEFERRED-AS-BUILT (ember).** *"While it is on devnet it doesn't
  matter. Mainnet is a loooong way away."* The as-built shape — per-venue
  `fee_recipient`, no protocol take — stands by default rather than by
  ratification, and is revisited pre-mainnet (`WAVE.md`, "Rulings — ember,
  2026-08-30 (evening, on the decision packet)", E1). §6's downstream line
  for D1 (a paragraph in `README.md` and `docs/guides/trader.md` naming who
  receives fees) is **unexecuted**.
- **D2 — ADOPTED AND BUILT: `MAX_FEE_BPS = 500`, no lower bound**
  (`DECISION_PACKET_2026_08_30.md` §1, orchestrator ruling, ember veto window).
  **Enforced in the protocol as of the FEE-CORE lane.** The const is
  `DIRECT_MAX_FEE_BASIS_POINTS_V1`
  (`crates/dclutch-direct-codec/src/successor.rs`), refused at config
  construction with its own discriminant `SuccessorError::FeeBandExceeded` —
  which every founding path reaches, because the immutable record is built from
  that type — and refused again as a relation of the authored transition, whose
  prelude compares the config rate against a program-owned `maxFeeBps` register
  (`DClutchSemantics.DirectOrdinaryV3`). The shell guard in
  `tools/release/stage-devnet-sponsored-market-open.sh` stays as an
  operator-console refusal, not as the only one. Cross-finding
  (`docs/design/FEE_SECOND_TRANSACTION_V1.md:683-688`), also executed: the band
  retires the `FeeSole` Custody route, which is reachable only at exactly
  10,000 bps. The retirement is proved rather than asserted —
  `DClutch.Direct.banded_fee_leaves_a_positive_seller_net` — and the state that
  would want that route refuses by name at
  `DirectInlineCandidateErrorV2::FeeSoleRetired`.
- **D3 — ADOPTED IN PRINCIPLE, BLOCKED ON MEASUREMENT.** The packet sequenced
  rate diversity behind FEEWALL; FEEWALL then measured a fee-bearing Direct
  trade at **1,515,003 CU all-first-try, over the 1,400,000 ceiling by 115,003
  before any key is drawn, with no tail**
  (`docs/evidence/DIRECT_HOT_FEE_BEARING_CU_2026_08_30.md`; `24b2b7f2`,
  `3d5dda0e`). So every market founded today is zero-fee **by necessity, now
  measured rather than inferred**, and D3 unblocks only when the
  second-transaction fee leg ships
  (`docs/design/FEE_SECOND_TRANSACTION_V1.md`). The release const D3 would
  unpin is still `DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1: u16 = 50`
  (`crates/dclutch-direct-codec/src/token_setup_v1.rs:25`).

  **The measurement D3 was blocked on is taken, 2026-08-31, and it clears.** The
  fee leg ships as a second transaction: the fee-bearing fill executes 32/32
  seeds with tens of thousands of CU of margin under the ceiling, and the
  settlement that follows costs about 170,000 in its own transaction. The fill's
  key-independent floor is indistinguishable from the zero-fee route's, so a
  fee-bearing market is no longer a more expensive market to trade in. Evidence:
  `docs/evidence/FEE_SECOND_TRANSACTION_PAIR_2026_08_31.md`. **D3's blocker is
  discharged; D3 itself is still a decision nobody has taken**, and the release
  const above is still pinned. What changed is that unpinning it is a choice now
  rather than a wish.

## 1. The question

> *"i think it would be fair to capture a modest percentage but **i don't know
> how to model the tradeoff space to figure out *what* percentage. 5%? 0.5%?
> 0.035%?**"* — ember, `01a00a3d`, 2026-08-17T07:25Z (`docs/INTENT.md` §3)

Two questions live inside it: **what number**, and **who chooses it**. The
second is the one that binds, and the tree has already answered it in a
direction no decision record states.

## 2. What the code does today

**The rate is not a protocol constant. It is a per-market, immutable,
maker-signed config field.**

- `DirectExecutionConfigV1 { price_scale: u64, fee_basis_points: u16,
  fee_recipient: [u8; 32] }` —
  `crates/dclutch-direct-codec/src/successor.rs:329-333`, doc-commented
  *"Immutable content-selected Direct price and fee policy."* Construction
  refuses `price_scale == 0 || fee_basis_points > DIRECT_FEE_DENOMINATOR_V1`
  as `InvalidExecutionConfig`, and `require_nonzero(fee_recipient)` refuses
  the zero key (`:336-350`).
- **The founder already sets it per market, and refusing to state it is a hard
  error.** `--direct-fee-basis-points is required; the first-market planner
  has no fee default` /
  `--direct-fee-recipient is required; the first-market planner has no
  recipient default`
  (`tools/local-validator/bootstrap/successor/src/direct_market.rs:99-108`).
  There is no default rate anywhere in the founding path.
- **Immutable by address, not by a missing instruction.** The config lives in a
  Registry-owned content-addressed PDA seeded by the SHA-256 of its own body,
  with the staging cursor required vacant
  (`programs/dclutch-trading-sbf/src/direct_token_setup_v1.rs:614-640`;
  *"the content-addressed raw PDA and absent staging cursor make the body
  immutable"*,
  `crates/dclutch-registry-contract/src/immutable_registry.rs:115-120`).
  `set_fee`, `update_fee`, `UpdateConfig` and `recalibrat*` return **zero hits**
  in crate and program code. **Changing a rate is not an update — it is
  founding a new market**, because different bytes give a different digest, a
  different PDA, and a different Market.
- **Both makers sign the exact rate.** The transition refuses `sellerFeeBps ≠
  policyFeeBps` and `buyerFeeBps ≠ policyFeeBps`
  (`crates/dclutch-direct-aot-v3-contract/src/lib.rs:164-165`;
  `DirectOrdinaryV3.lean:507-508`), with hostile witnesses
  (`registered_tests.rs:317-322`).
- **Charged per side, floored toward the makers.** `fee =
  mul_div_floor(gross, POLICY_FEE_BPS, FEE_DENOMINATOR)`, then `seller_net =
  gross − fee`, `buyer_debit = gross + fee`, `combined_fee = fee + fee`
  (`crates/dclutch-direct-aot-v3-contract/src/lib.rs:168-183`). So **a rate of
  50 bps per side takes 100 bps of gross on a single fill.** The rounding
  direction is a stated value choice: *"it rounds toward the makers, never
  toward the venue"* (`tools/gauntlet/direct/expectations.json:6`).
- **Registered orders charge differences of floors of cumulative gross**
  (`registered.rs:334-357`), so matcher fragmentation cannot change a resting
  order's final fee — proved for an *arbitrary* monotone fee function
  (`cumulative_fee_telescopes`, `DirectProofs.lean:143-153`).
- **The ceiling is 100%, and it is deliberate.** `DIRECT_FEE_DENOMINATOR_V1 =
  10_000` (`successor.rs:31`), and the boundary is admitted by a proved
  theorem with its rationale in the docstring
  (`DirectOrdinaryV3.lean:677-696`): *"A venue rate exactly at the denominator
  is admitted: the fee equals the gross and the seller nets nothing, which is
  a policy the makers may sign."* One basis point above refuses. **There is no
  lower bound and no below-100% ceiling anywhere** — no `MIN_FEE` const exists.
- **Zero is legal and exercised.** `zero_fee_bps` sits in the *admitted*
  `BOUNDARY_CORPUS`, not the hostile one (`registered_tests.rs:576-580`), an
  explicit zero-fee policy is asserted to succeed
  (`direct_market.rs:2144-2152`), and the doctrine is written out
  (`DirectRegisteredFillV4.lean:808-821`): a zero combined fee is a
  no-transfer path, not a refusal, because refusing it would refuse ordinary
  small fills at realistic rates.
- **Dealer has its own rational geometry** — `fee_numerator` / `fee_denominator`
  with `feeDue = ceilDiv(base·num, den)`, fragmentation-independence proved
  (`crates/dclutch-dealer-codec/src/lib.rs:115-119`;
  `DealerLiquidity.lean:192-206`).
- **General — the batch-auction family — charges nothing at all.** No fee field
  exists in its config or codec; the only mention is a disclaimer
  (`crates/dclutch-general-config-contract/src/v3.rs:89`).
- **Claims, custody and redemption charge nothing** — gen-1's founding
  objective still holding. Read as already-decided.
- **There is no protocol treasury.** `fee_recipient` is a required pubkey and
  nothing more; gen-1's `REVENUE-TREASURY-UNSET-SENTINEL1` mechanism was not
  carried into this generation (zero code hits for `REVENUE_TREASURY` /
  `RevenueTreasuryUnset`; both survive only as citations of the compost repo).

**Where 50 bps actually lives — four consts, and one of them is a refusal.**
There is no file matching `staging hook` (zero hits). The devnet rate is
pinned at `tools/devnet-scenarios/src/model.rs:12`,
`tools/local-validator/bootstrap/successor/src/direct_trade_producer.rs:103`,
`programs/dclutch-trading-sbf/program-test/direct-hot/src/fixture.rs:128`, and
— decisively —
`crates/dclutch-direct-codec/src/token_setup_v1.rs:25`:

```rust
/// The one Direct fee rate admitted by this setup release.
pub const DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1: u16 = 50;
```

with its own named refusal, *"The selected fee rate was not the release-pinned
50 basis points"* → `InvalidFee` (`token_setup_v1.rs:81-82, 278-281`;
program-side twin at `direct_token_setup_v1.rs:477-479`). **So on the live
Token-2022 setup path the admissible set is exactly `{50}`.** The core
transition accepts any value ≤ 10,000; the *release* narrows it to one. That
const is the single strongest evidence that 50 bps is currently a release
decision rather than a market decision — and it is the one thing standing
between the tree and a zero-fee or diverse-rate demo.

**The shape question is already recorded; this is the rate question.**
`docs/design/FEE_GEOMETRY.md` §4.1 confirmed flat `fee_basis_points` as the
deliberate V1 placeholder and closed ledger N-1, ratified at `WAVE.md:940-947`
— *"rates still open, still ember's."*

**One correction to how this question is usually framed.** L7 is not a
fee-conservation law. It is the lamport-ledger delta law in the journey
harness — `payer_delta + fees + watched_growth == 0`,
`tools/gauntlet/journey/src/ledger.rs:48-61` — about **SOL transaction fees**,
asserted at runtime, and it *"has never once been evaluated over a founding"*
by its shape (`tools/lamport-ledger/README.md:18-35`). The real fee
conservation law is `admitted_collateral_conserved`
(`DirectProofs.lean:128-133`): buyer, seller and **venue** collateral is
invariant across a fill, proved. Neither it nor `cumulative_fee_telescopes`
constrains the rate — they govern *accounting*, not *magnitude*. The only
rate-restricting statements in the tree are the `≤ feeDenominator` clauses.

## 3. The reframe: three of the four decision shapes are already foreclosed

The question is usually posed as a four-way choice — fixed forever,
founder-set per market, protocol-governed, or zero-for-the-demo. Against the
tree, three of those are not open choices but reversals:

| shape | status against the tree |
|---|---|
| **founder-set per market, immutable at founding** | **This is what is built and CLI-exposed with no default.** Ruling for it costs zero code. |
| fixed forever, one protocol rate | A reversal: it deletes a config field both makers already sign, and re-freezes what gen-1 called *"an experiment, not a natural constant."* Also the opposite of ember's *"chose the 'weakest' choice — the one most general, with the least constraining over resulting dynamics"* (`docs/INTENT.md` §3). |
| protocol-governed | Foreclosed structurally, not just on values. Because the config is content-addressed, governance could not turn a dial — it could only found successor markets, which is what a founder already does. And the values objection stands independently: governance needs a committee or a token vote, the Isometric machinery ember refused by name (`docs/INTENT.md` §4), against *"No committee, no vote, no discretion."* |
| zero-for-the-demo | Legal in the protocol at zero cost (§2), but **not free on the live path**: `token_setup_v1.rs:25` would refuse a 0-bps market with `InvalidFee`. And gen-1 already ruled how zero must be held if chosen: **declared, never defaulted into** (`FEE_GEOMETRY.md` §2.2). |

So the live decisions are three, and none is "pick the rate":

**D1 — is there a protocol cut at all?** Fees route to a per-market
`fee_recipient` bound at founding; there is no protocol take and no treasury.
That dissolves the treasury question cleanly and serves ember's
strongest-sourced value (*"i don't want to have to operate any of this
infrastructure"*, `docs/INTENT.md` §3) — and it means **the protocol earns
nothing; the founder of each market does.** Against the motive INTENT records
for the demo (*"hoping to earn some keep so that the game can go on"*; *"turn
this pile into a *stream*"*), revenue exists only for markets ember founds.
That is a coherent answer. It should be a chosen one rather than a discovered
one.

**D2 — what bounds a founder's choice?** Today, nothing below 100%. A market
that takes the entire gross from the seller and charges the buyer the same
again is admissible — and, unusually, that is not an oversight: it is a proved
theorem with a written rationale (§2). Ruling here means either endorsing that
rationale or overriding it.

**D3 — what do the demo markets charge?** Currently one value, `{50}`, forced
by the token-setup release rather than chosen per market.

## 4. Options for D2

| option | cost | consequence |
|---|---|---|
| **A. Keep the 100% ceiling** | zero | Endorses the existing theorem: consent is the whole bound, and two signers may agree to anything. Defensible; hard to screenshot. |
| **B. Freeze a band as consts, checked at founding** | one const pair, one refusal discriminant, the Lean bound beside `DirectOrdinaryV3.lean:509`, its two `native_decide` boundary theorems restated, census regeneration — the shape decision 0012 already ran | Forecloses the take-everything market. Reverses a proved-admissible boundary, so the theorem and its docstring change with it. |
| **C. B, plus a recommended default in the founding tooling** | B, plus a default the CLI currently refuses to have | Contradicts the deliberate no-default design in `direct_market.rs:99-108`. Not recommended for that reason. |

## 5. Recommendation

**D1: keep the per-venue `fee_recipient`; take no protocol cut.** It is built,
it is the strongest available form of "nothing requires a service we operate,"
and it makes revenue a property of founding markets rather than of running
infrastructure. Say the consequence out loud in the docs — the protocol has no
income; market founders do — so nobody discovers it later as a surprise.

**D2: option B, at `MAX_FEE_BPS = 500` (5%).** Five percent is the top of
ember's own stated range, so every rate he named stays admissible while the
take-everything market stops being foundable. Keep no lower bound: zero must
remain expressible because gen-1 required zero-fee to be a declaration.
Stated honestly, this **overrides a deliberate decision** — the current
boundary is proved and reasoned, and its reasoning ("a policy the makers may
sign") is sound. The counter-argument is that this venue's product claim is
that a market is a readable, permanent public object; a market whose config
takes everything is signable, publishable, and indistinguishable at a glance
from an honest one. Consent is a good bound on fraud and a poor bound on what
a venue should be able to host.

**D3: unpin the release const and ship rate diversity.** Replace the
`{50}`-only admission at `token_setup_v1.rs:25` with the same band check, then
found the demo markets at distinct rates as `FEE_GEOMETRY.md` §4.2 already
recommends, with the conservation ledger showing venue take as the sum of
floor-differences over each market's life. This is the smallest change that
makes the fee a *demonstrated mechanism* rather than a number.

**On ember's actual question — 5%, 0.5%, or 0.035% — the field says 0.5% per
side, and here is the only production comparable.** Kalshi charges a taker fee
of `ceil(0.07 · C · P · (1−P))` per contract, maker side free
(`FEE_GEOMETRY.md` §2.2). At a price of 0.50 that is **3.5% of notional
traded**. dClutch at 50 bps per side takes both sides, so **1% of notional per
fill — under a third of Kalshi's single-sided charge.** 5% would be well above
the only event venue with real volume; 0.035% is below what a fill costs to
place. **Ember's middle guess was the right one**, and the rate already
running is in the right place.

**The same arithmetic exposes flat's one surviving defect, and it is worth
seeing as a number.** Kalshi's fee is shaped by `P(1−P)`, so it charges the
same on a claim and its complement. Flat does not. At a price of 0.10, buying
the cheap claim costs 1% of a 0.10 notional; acquiring the identical economic
position by buying the 0.90 complement costs 1% of a 0.90 notional — **nine
times as much fee for the same risk, decided entirely by which label you
bought.** That is complement asymmetry, the one gen-1 defect that survives on
the bilateral plane (`FEE_GEOMETRY.md` §1.3.3), and the composite geometry
`FEE_GEOMETRY.md` §4.3 specifies is its fix. It is a reason to keep the rate
revisable, not a reason to delay this ruling.

## 6. What changes downstream once ruled

- **D1 ruled** → one paragraph in `README.md` and `docs/guides/trader.md`
  stating who receives fees. Ledger M-26 closes with a decision record instead
  of a fourth silent generation (`ASPIRATION_LEDGER.md` GITSCAN-2 §D.1 item 5:
  selected in gen-1, built in gen-2, discarded in gen-3, no record).
- **D2 ruled** → one lane, protocol-tier: one const, one refusal discriminant,
  the Lean bound and its two boundary theorems, a refusal-census regeneration,
  and a positive/negative pair. Small, and it touches a proved statement, so
  it is protocol-tier review rather than mechanical.
- **D3 ruled** → the token-setup release const, then the demo foundings. The
  product surface needs no work: the fee is already displayed per side with
  its immutability narrated (*"{feeBasisPoints} bps each side / immutable,
  founded with the Market"*,
  `apps/dclutch-web/components/MarketTradePanel.tsx:684`), so a rate change is
  user-visible on day one.
- **N-15 is not tripped by any of this**, deliberately. The standing
  precondition is that *"the composite fee base's characterization is
  formalized before any rate freezes"* (`NEXT_WAVE_ROADMAP_2026-08-20.md:98`,
  ledger N-15 — unfired and unowned). A band, a devnet rate, and an unpinned
  release const are none of them a freeze. Whoever eventually freezes a
  production rate inherits that gate; this record does not discharge it and
  does not let it be forgotten.

## 7. What stays refused

Carried forward from `FEE_GEOMETRY.md` §4.3 item 6, unchanged by any option
above: redemption-side fees (a tax on trusting the venue's own resolution),
source-adaptive fees (the Mango lens — a fee reading the observable that also
resolves the market), geometry-as-code in the Uniswap-v4-hook sense, floating
point anywhere, and mutation of any fee record — which this tree enforces
structurally, since calibration can only be registered succession.
