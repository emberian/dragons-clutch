# The observation scale's one author

**Head current at `887d6c04a` (2026-09-04), tree root `/Users/ember/dev/dclutch`; devnet evidence, not mainnet evidence.** The three sections below `## History` — the finding, the factor as landed, the three debts closed — are verbatim and in order.

## The defect, and the two markets that are its instances

A market's cuts and its observation could be authored on **different scales** with no record declaring a factor between them, so nothing — program, operator or
reader — could notice. Cohort-14's markets **B** (`DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A`) and **C** (`BL8zsFokbz7aEdo3wjtcNffd5P1D8a9wVxwKq3mcMsMN`)
each compared a raw Pyth mantissa at exponent −8 against cuts in US cents, and each was paid the cell the deployed program chose rather than the cell its
reading falls in: the chain paid cell 2, the reading at the feed's own exponent falls in cell 1, which pays zero. Market C is the one that reached a stranger —
participant-2 held 200 claims at index 1. Both are asserted from chain, per market, by `apps/dclutch-web/lib/ordinarySelector.live.test.ts:48-49`.

## The authority

`StatisticSpecV1.source_scale_exponent: i32` is the source-to-result decimal shift: the observation times ten to this power is the
reading in the result unit. It occupies **bytes 12..16** (`crates/dclutch-source-contract/src/generated_statistic_spec_v1.rs:23`,
width at `:9`) — four bytes `decode` previously required canonically zero — and `STATISTIC_SPEC_BYTES` stays **176** (`:5`).

Lean has owned the layout since **`485f5cb9f`**: `DClutchSemantics.SourceStatisticSpecV1Abi` declares the fields and
`formal/dclutch-semantics/EmitSourceStatisticSpecV1Rust.lean` prints them. The theorems worth naming:

- `the_factor_fills_the_span_that_was_reserved` (`SourceStatisticSpecV1Abi.lean:170`) — the shift begins where the rounding tag ends
  and ends where the first unit identity begins, so it occupies exactly the four bytes that were reserved.
- `ResultDomain.scaled_selection_in_one_cell` (`DClutchSemantics/ProductRuntimeV2.lean:253`) — once the declared factor has put the observation
  and the cuts on one scale, the observation falls in exactly one ordinary cell. `maxScaleExponent = 18` (`:126`) is emitted to
  `MAX_SOURCE_SCALE_EXPONENT` (`crates/dclutch-product-runtime-v2/src/generated.rs:27`).
- `ResultDomain.selectOrdinaryScaled_identity` (`:184`) is **the migration statement**: a record declaring no factor selects exactly what the
  unscaled selector selected. Every statistic founded before the field decodes with the factor at zero, re-encodes byte-for-byte and keeps its
  content digest, so no account is rewritten and a pre-factor market's reading is now a *declared* identity scale, not an undefined one.

## One selection site, one adapter rule, three routes

Every on-chain route reaches the selector through `SourceResolutionStateV2::resolve_primary_from_authenticated_domain` (`crates/dclutch-source-contract/src/source_resolution_v2.rs:393`),
which takes the shift as an argument and hands it to `ResultDomainV2::select_ordinary` (`:416`); an unsupported scale refuses `NonCanonicalSourceScale` rather than being flattened into
`InvalidResultMap`. The shift is never defaulted — a caller that omits it has not chosen the identity, it has failed to state a choice.

`StatisticSpecV1::require_admitted_scale` (`crates/dclutch-source-contract/src/lib.rs:1617`) is the single author of which shift an adapter admits, called
by both provider families: `PythAdapterConfigV1::validate_update` (`:703`) and `relay_v1.rs:339`. `StatisticSpecV1::validate_scale` (`:1639`) refuses a
statistic claiming a conversion between one unit and itself. A shift the publication does not carry refuses `ResolutionError::ProviderScale = 0x801C`
(`programs/dclutch-resolution-proof-sbf/src/lib.rs:204`), checked **after** the publication is admitted, so that code can only mean the market's own two
records disagree — a fault no resubmission can fix. The three routes read the number off the record, never off an adapter account: `provider_v3.rs:258`
(`obligation.source_scale_exponent()`), `sponsored_push_v1.rs:1238` and `relay_v1.rs:376` (`records.statistic.source_scale_exponent()`).

## The relayed route's slot, and its second fault

The relayed route had no statistic in its frame and passed the identity. `0b5e862ea` grew `CONSUME_RECORD_FRAME_V1` from 28 positions to **30**
(`crates/dclutch-relay-contract/src/frame.rs:317`) — the raw `StatisticSpecV1` and its staging vacancy, both read-only, authenticated against
`SourceMaterialV3::statistic_spec` by content identity. **The second fault it fixed was the larger one:** the route had compared the *Source spec's* unit against
the Product's result unit, the wrong end of the map, so a market whose own statistic said `A` maps to `B` by a factor could satisfy it and be selected at the
identity with nothing red. Still not executable is a relayed founding whose declared conversion MOVES a cell: both decoding-rules rows publish `raw_exponent = 0`.

## The browser, and cohort-15

`inspectMarketDeclaredScaleV1` (`packages/dclutch-sdk/lib/marketResolution.ts`) walks `SourceMaterialV3 → StatisticSpecV1` in two
account reads; `MarketDetailWorkspace.tsx` passes `resolution.scale.sourceScaleExponent` where it passed the literal `0`, and
withholds the join entirely when the record did not read. **`unread` is a status, never a zero** (`marketResolution.ts:123`).

**Cohort-15 is the first cohort founded carrying the factor.** Its devnet founding writes the shift from the observed publication's own exponent,
and a market founded after `4cd2b9cb5` that declares a conversion and omits the number cannot resolve at all. The witness is
`docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md` §8, in flight with the COHORT-15 lane as this head was written.

## History

*Everything below is this note as written on 2026-09-03, unchanged and in order: the finding, the factor as landed, and
the three debts closed. Where it contradicts the head above, the head is the current truth.*

# The observation scale nothing publishes — 2026-09-03

**Devnet evidence. Not mainnet evidence.** Written by the WEB lane while
mirroring the Resolution program's selector derivation into the browser
(`packages/dclutch-sdk/lib/ordinarySelectorV1.ts`).

## The finding in one line

A market's cuts and its observation can be authored on **different scales**, and
no record on chain declares a factor between them, so nothing anywhere — not the
program, not the operator, not a reader — can notice. Cohort-14b settled that
way, and the mis-scaling **inverted its outcome and moved 500,000,000 atoms**.

## What the browser CAN now do, so the gap is not confused with it

The join the browser called undecidable is decidable, and the previous refusal
in `MarketDetailWorkspace.tsx` was wrong about why.

`ResultDomainV2::select_ordinary`
(`crates/dclutch-product-runtime-v2/src/lib.rs:222`) compares the observation
ratio against each cut ratio **directly**. There is no exponent multiplication
anywhere on the live resolution path, and every producer of a certificate pins
`result_denominator` to the literal `1` —
`crates/dclutch-resolution-core-v3-operator/src/provider_finalized_projection_v3.rs:632`
refuses a receipt carrying anything else. So the selector is a function of
exactly the numbers a browser already holds, and cohort-14b's committed
selector `2` is reproduced from its own finalized records by
`apps/dclutch-web/lib/ordinarySelector.live.test.ts`.

That settles **which cell the protocol chose**. It settles nothing about
whether that cell is right about the world, and this document is about the
second question.

## The numbers

Cohort-14b, market `DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A`, settled
2026-09-03 at slot 492,412,657.

| | |
| --- | --- |
| cuts / denominator | `9900, 10300` over `100` — authored in **US cents** |
| observation | `10062091764` over `1` — raw Pyth SOL/USD mantissa at **exponent −8** |
| committed selector | **2** — reproduced exactly by the mirrored derivation |
| coefficient vector | `[1,0,1,0]` — a `CentredRangeProtection`, paying **outside** the $99–$103 band |
| the same observation on the cuts' scale | `10062091764 / 100000000` = **$100.62**, which is **inside** the band |
| the selector that scale would have chosen | **1**, which pays `0` |

The market paid cell 2. The price was in cell 1. Both readings are the honest
output of the arithmetic each was handed; the arithmetic was handed two
different units.

## Where the exponent actually lives, and where it stops

It exists on chain and it is reachable. It simply never reaches the comparison.

```
certificate.sourceMaterial            (certificate byte 80)
  → SourceMaterialV3   DCLTSMV3, 240 B, offset 48  → primary_source_spec
  → SourceSpecV1       DCLTSRC1, 192 B, offset 112 → adapter_config_id
  → PythAdapterConfigV1 DCLTPAC1, 64 B, offset 12  → expected_exponent : i32 LE   ← the −8
```

`PythAdapterConfigV1::validate_update`
(`crates/dclutch-source-contract/src/lib.rs:625-646`) **checks the exponent for
equality and then discards it**, returning the raw mantissa:

```rust
        if provider_feed_id != self.provider_feed_id
            || exponent != self.expected_exponent
            || confidence_scaled > admitted_confidence
        {
            return Err(Error::InvalidPythObservation);
        }
        Ok(i128::from(price))
```

That value flows through `normalize_authenticated_update`
(`crates/dclutch-source-contract/src/provider_join_v2.rs:230-270`) into
`result_numerator` unchanged, and the certificate carries no exponent byte at
all (`packages/dclutch-sdk/lib/generated/resolutionCertificateV2.ts`: 312 bytes,
`RESULT_NUMERATOR_OFFSET = 280`, `RESULT_DENOMINATOR_OFFSET = 296`, and nothing
else numeric about the scale).

## The record that should carry the factor and does not

**`StatisticSpecV1`** (`crates/dclutch-source-contract/src/lib.rs:1344-1353`)
is the record whose entire job is to declare the conversion, and it declares it
as two opaque identities with no number between them:

```rust
pub struct StatisticSpecV1 {
    source_unit_id: ContentId,
    result_unit_id: ContentId,
    kind: StatisticKind,
    rounding: RoundingBoundary,
    required_samples: u16,
    threshold_atoms: i128,
    capacity_profile_id: ContentId,
    evaluator_release_id: ContentId,
}
```

The chain checks only that each side matches its own peer —
`provider_join_v2.rs:280` (`statistic.source_unit_id() != source.unit_id()`) and
`programs/dclutch-resolution-proof-sbf/src/provider_v3.rs:192-197`
(`obligation.result_unit_id() != observation.result_domain.result_unit_id()`).
Both checks pass while the two units mean different things, because the market
publisher gives them genuinely different meanings:

* `tools/local-validator/bootstrap/successor/src/market.rs:13355` —
  `result_unit = demo_id("result-unit/usd-cents", …)`
* `tools/local-validator/bootstrap/successor/src/market.rs:13443` —
  `source_unit = demo_id("source-unit/pyth-scaled-price", …)`
* `tools/local-validator/bootstrap/successor/src/market.rs:13486` — the
  `StatisticSpecV1` joining them, with `RoundingBoundary::ExactRational`

`ExactRational` means "pass an exact numerator and a positive denominator to the
mapping release", and the denominator passed is `1`. `mapping_release_id` in the
ResultDomain (offset 192) is a NAME for the mapping semantics and carries no
number either.

Two conventions coexist in one tree, which is why nobody caught it: the
operator's own reference authoring uses raw atoms rather than cents —
`crates/dclutch-product-runtime-v2-operator/src/authoring.rs:303-305`,
`SOL_USD_ANCHOR: i128 = 100_000_000`, cuts `[99_666_667, 100_333_333]` over
denominator `1`.

## Why no test caught it

**No integration test in this tree has ever run a market with
`cut_denominator != 1`.** Every SVM-harness end-to-end resolution uses `1`:
`resolution_successor.rs:883`, `sponsored_push_lifecycle.rs:298`,
`pre_market_resolution_funding.rs:346`, `resolution_core_v3_lifecycle.rs:858`,
`relayed_mainnet_state.rs:811`. With a cut denominator of `1` and an observation
denominator of `1`, a missing scale factor is the identity and every case
passes.

## What would have refused market B at founding

Either is one number in one record, and either is enough:

1. **`StatisticSpecV1` gains a declared source-to-result factor** — an exact
   rational, or a signed decimal shift — that `normalize_authenticated_update`
   applies before returning `atoms`. This is the shape the record already
   implies: it names a source unit and a result unit and is the only place
   entitled to say how they relate.
2. **`PythAdapterConfigV1.expected_exponent` is joined to
   `ResultDomainV2.cut_denominator`** by an admission that refuses a market
   whose cuts are not on the feed's scale. Narrower, cheaper, and specific to
   Pyth-backed markets, so it does not generalise to a second adapter family.

**(1) is the right one** and (2) is a stopgap: the defect is that a declared
unit conversion carries no factor, not that one particular adapter forgot one.

A third repair is owed regardless of which lands: an SVM end-to-end resolution
with `cut_denominator != 1`, since the absence of that case is what let a wrong
outcome reach a real wallet.

## What the browser publishes in the meantime

`terminalWinnerNameV1` (`apps/dclutch-web/components/MarketDetailWorkspace.tsx`)
names an ordinary cell **only when the mirrored derivation reproduces the
selector the chain committed**, and says on the page which authority the name
rests on. Beside the name it states that the observation and the cuts are
declared as two unit identities with no published factor, so a reader is told
what the check covers and what it does not. The page never rescales anything:
the rescaled reading appears only in
`packages/dclutch-sdk/lib/ordinarySelectorV1.test.ts`, as a measurement of what
the gap costs, kept out of any surface a reader could mistake for the chain's
own answer.

---

# The factor, as landed — 2026-09-03

**Devnet evidence. Not mainnet evidence.** Written by the RESOLUTION-SCALE lane
at `4cd2b9cb5`, closing repair (1) above and the third repair it said was owed
regardless.

## Where the number lives

`StatisticSpecV1.source_scale_exponent: i32`
(`crates/dclutch-source-contract/src/lib.rs`, accessor
`StatisticSpecV1::source_scale_exponent`), at **bytes 12..16 — four bytes that
were already reserved and enforced zero**. `STATISTIC_SPEC_BYTES` stays 176.
The observation times ten to this power is the reading in the result unit.

That placement is what makes the migration statable rather than a silence:
every statistic founded before today decodes with the factor at zero and
re-encodes byte-for-byte, so **a pre-factor market's reading is now a
*declared* identity scale rather than an undefined one**. Its content digest
does not move, no account is rewritten, and cohort-14's records keep the
meaning the deployed program acted on.

## Why not `result_denominator`, which was the other candidate

Folding the factor into the certificate's `result_denominator` would have been
smaller — the browser reads that field already — and it is wrong. That
denominator is the **statistic's own rational**: `RoundingBoundary::ExactRational`
means "pass an exact numerator and a positive denominator to the mapping
release", and `ExactScheduledAverage` and `OddScheduledMedian` legitimately
produce denominators above one. A denominator carrying both a sample count and
a unit conversion makes neither recoverable from the other. The scale is also
founding-immutable while the observation is per-event; putting an immutable
market property into every certificate duplicates it into a second place it
can be wrong.

So the cuts stay authored in the market's own quote unit — the founding writes
`$99`, not `9900000000` atoms — and the shift is applied where the observation
becomes a cell.

## Which shift an adapter admits, and why it is not simply equality

The tree founds markets **both ways**, and the two unit identities are what say
which. This is the rule, and it is the first thing those two identities have
ever been checked against:

* **One identity on both sides declares no conversion.** The result domain's
  cuts are on the feed's own atom scale and the only admissible shift is zero.
  `tools/local-validator/bootstrap/successor/src/relayed.rs` and
  `crates/dclutch-product-runtime-v2-operator/src/authoring.rs`
  (`SOL_USD_ANCHOR = 100_000_000`, denominator 1) are this shape.
* **Two identities declare a conversion**, and the only conversion this release
  performs is the feed's published decimal exponent — so that is the only
  admissible shift, and the cuts are in the feed's quote unit.
  `tools/local-validator/bootstrap/successor/src/market.rs`
  (`source-unit/pyth-scaled-price` into `result-unit/usd-cents`) is this shape,
  and until `4cd2b9cb5` it declared the conversion and left the number out.

A statistic cannot even be constructed claiming a conversion between one unit
and itself (`StatisticSpecV1::validate_scale`), and
`PythAdapterConfigV1::validate_update` refuses a shift its config does not
admit with `Error::SourceScaleMismatch`, surfaced on chain as
`ResolutionError::ProviderScale = 0x801C`.

**That refusal is checked after the publication is admitted, deliberately.**
Reaching it means the feed published exactly what this market pinned, so
`ProviderScale` in a validator log can only mean the market's own two records
disagree — a fault no resubmission can fix. Reported as
`ProviderConfiguration`, an operator would resubmit forever.

## One author, every consumer derives

`ResultDomainV2::select_ordinary(numerator, denominator, source_scale_exponent)`
takes the shift as an argument rather than defaulting, because an observation
alone does not name a cell: the same numerator names different cells under
different declared factors, and a route that omits the number has not chosen
the identity — it has failed to state a choice. The shift multiplies one side's
denominator, chosen by its sign, and never divides, so the comparison stays
exact and stays inside the integer widths the unscaled one already used.

Consumers, none of which reads an adapter account for it:

| route | where the factor comes from |
| --- | --- |
| `programs/dclutch-resolution-proof-sbf/src/provider_v3.rs` | `obligation.source_scale_exponent()` |
| `programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs` | `records.statistic.source_scale_exponent()` |
| `crates/.../provider_finalized_projection_v3.rs` | a statistic account authenticated against `SourceMaterialV3::statistic_spec` by digest |
| `programs/dclutch-resolution-proof-sbf/src/relay_v1.rs` | the identity — **see the debt below** |

## Lean

`formal/dclutch-semantics/DClutchSemantics/ProductRuntimeV2.lean` owns the
semantics. `ResultDomain.scaled_selection_in_one_cell` is the law: once the
declared factor has put the observation and the cuts on one scale, the
observation falls in exactly one ordinary cell — the selector is below the
region count, every cut beneath it is at or below the reading, and the next cut
is strictly above. `ResultDomain.selectOrdinaryScaled_identity` is the
migration statement as a theorem. `maxScaleExponent` is Lean-owned and emitted
to `MAX_SOURCE_SCALE_EXPONENT` in
`crates/dclutch-product-runtime-v2/src/generated.rs`. Market B is two
kernel-checked twins in that module: the identity gives `2`, `-8` gives `1`.

## The test the absence of which was the whole defect

`crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs` now founds
its market with cuts `50, 150` over `100` against the captured Pyth mantissa
`100_000_000` at exponent `-8` — one dollar, inside the $0.50–$1.50 band, cell
1. Rebuilding the Resolution ELF with the route passing the identity, **the
program commits selector 2, the top cell, and the assertion fails 2 against
1**; with the factor it commits 1. The identity-scale selector is asserted
beside it as a control, so the fixture cannot quietly stop distinguishing the
two scales.

## What this document said that reading disproved

* *"the browser's `/create` through the source-provider wasm"* founds no
  statistic. **Nothing outside Rust authors those 176 bytes** — no wasm crate,
  no TypeScript, no generated module; `lib/founding/ladder.ts` declares the
  record-graph rung `tooling-only` and says the honest browser path is an
  emitter per encoder. The only 176-byte author in the repository is
  `StatisticSpecV1::to_bytes`. There were exactly **two** non-test
  `StatisticSpecV1::new` call sites, both in the successor bootstrap.
* The certificate's layout is Lean-emitted
  (`EmitSourceResolutionTerminalV2AbiRust.lean`); **`StatisticSpecV1`'s is
  not**. There is no `StatisticSpecAbi.lean` and no
  `generated_statistic_spec_v1.rs`, though `generated_window_spec_v1.rs` exists
  for its sibling. The field landed in a hand-written layout.

## Owed

1. **The relayed route cannot see the factor.** `relay_v1.rs` has no statistic
   slot by design ("A terminal window over a terminal sample has one
   observation and one atom"), so it passes the identity. The relayed founding
   now *declares* the identity — one unit identity on both sides, which
   `validate_scale` refuses to pair with any nonzero shift — but **nothing on
   the route checks it**, because the record is not in the frame. A relayed
   market founded with a declared conversion would be selected at the identity
   and mis-paid exactly as market B was. Closing it is an account-frame change,
   not a line change.
2. **`StatisticSpecV1` has no Lean-owned layout.** The field sits at an offset
   nothing emits. A `StatisticSpecAbi.lean` plus `generated_statistic_spec_v1.rs`
   would put it under the same authority as `WindowSpecV1`.
3. **The browser's mirrored derivation still applies no factor.**
   `packages/dclutch-sdk/lib/ordinarySelectorV1.ts` reproduces the chain's
   committed selector from `(numerator, denominator, cuts, cutDenominator)`,
   which is correct for every cohort-14 market and wrong for the first scaled
   one it meets. It needs the market's `StatisticSpecV1` in its join and the
   factor in its comparison. Owner: the WEB lane, whose file it is.

---

# The three debts, closed — 2026-09-03

**Devnet evidence. Not mainnet evidence.** Written by the STATISTIC lane.

## 2, the layout

`DClutchSemantics.SourceStatisticSpecV1Abi` owns the twelve fields;
`EmitSourceStatisticSpecV1Rust.lean` prints them to
`crates/dclutch-source-contract/src/generated_statistic_spec_v1.rs`, and
`decode`/`to_bytes` read the emitted names. `check-generated.sh` pins the width,
the shift's coordinate and width, and the two identity offsets before the byte
compare. The emission census moves 95 → 96, still 96 guarded.

The theorem worth naming is `the_factor_fills_the_span_that_was_reserved`: the
shift begins where the rounding tag ends and ends where the first unit identity
begins, so the four bytes it occupies are exactly the four `decode` used to
require canonically zero. That is the migration statement, as a placement rather
than as a comment beside a `zero` call.

## 1, the relayed route

`CONSUME_RECORD_FRAME_V1` grows from 28 positions to 30: the raw
`StatisticSpecV1` and its staging vacancy, both read-only, beside the window's
pair. `consume_source_records` authenticates it against
`SourceMaterialV3::statistic_spec` by content identity like every other link.

The route then does two things it could not do before, and the FIRST is the one
that was mis-paying:

* **It compares the statistic's own two identities, one to each end.** The route
  used to compare the *Source spec's* unit against the Product's result unit,
  and said so in a comment: "there is no statistic record to interpose because a
  terminal sample is the identity map." That is the wrong end of the map. A
  market could satisfy it — Source unit `A`, result domain unit `A` — while its
  own statistic said `A` maps to `B` by a factor, and be selected at the
  identity with nothing red.
* **It applies the declared shift**, admitted against the selected
  decoding-rules row's published `raw_exponent` through
  `StatisticSpecV1::require_admitted_scale` — which is now the single author of
  that rule for both provider families, called by `PythAdapterConfigV1::validate_update`
  as well. A shift the row does not publish refuses `ResolutionError::ProviderScale`
  `0x801C`, the same code the Direct route publishes for the same accusation,
  and checked after the publication is admitted for the same reason.

**PROVEN RED**, against a Resolution ELF rebuilt with the frame in place and the
two authorities removed:

| founding | pre-fix | with the fix |
| --- | --- | --- |
| statistic `{A, B, −8}`, spec unit `A`, domain unit `A` | **consumes** — record `Consumed`, Source `Resolved`, a `ResolutionSuccess` certificate written on an unconverted observation | refuses `ProductDomain`; record still `Sealed`, Source not `Resolved` |
| statistic `{B, A, −8}`, spec unit `B`, domain unit `A` | refuses `ProductDomain` `0x8008` — the right answer for the wrong reason | refuses `ProviderScale` `0x801C` |

The whole 26-case `relayed_mainnet_state` campaign is green, including both
identity-shape consumptions, whose certificates are unchanged: the atom is still
the venue's own discriminant and the selector is still `0`.

**What is NOT executable, and why.** A relayed founding with a declared
conversion that *moves a cell* cannot exist on this release. Both rows of the
decoding-rules table publish `raw_exponent = 0` — a `MigrationProgress`
discriminant and a renunciation flag are not quantities anything scales — so the
only admissible shift is the identity and the arithmetic cannot move. The
positive scaled case becomes reachable when a row publishes a nonzero exponent,
and inventing one to make a test green would be inventing a venue. What the two
cases above prove is that the record is read, joined and admitted; what they
cannot prove is a cell moving.

## 3, the browser

`inspectMarketDeclaredScaleV1` walks `SourceMaterialV3 -> StatisticSpecV1` in
two account reads and returns the declared shift.
`inspectMarketResolutionV1` calls it and carries a `scale` on every
authenticated resolution; `MarketDetailWorkspace` passes
`resolution.scale.sourceScaleExponent` where it passed the literal `0`, and
**withholds the join entirely** when the record did not read, because there is
no number a reader may substitute for the one the founding wrote.

Every coordinate is Lean-owned. `generate-core-found.mjs` gained
`generated_statistic_spec_v1.rs` as a source and emits
`STATISTIC_SPEC_BYTES_V1`, the magic and its coordinate, the shift's offset and
the two identity offsets, plus `SOURCE_MATERIAL_STATISTIC_SPEC_OFFSET_V3` and
`STATISTIC_SPEC_SCHEMA_ID_V1`. `abi:coverage` counts **no new hand-mirror**:
the one literal that did appear -- a `0` for the magic's offset -- was replaced
by the emitted coordinate rather than admitted to the baseline.

**`unread` is a status, never a zero.** The reader reports five distinct
reasons it has no scale -- a failure certificate, a material the Market did not
name, no Registry supplied, a record that did not read, a record not at its own
content-derived address -- and none of them offers a number. That is the same
finding one level up: a caller that omits a scale has not chosen the identity.

Two checks came along with it, because reading a value out of a graph is what
made them matter. The certificate's `source_material` must equal the Market's
own `resolutionPolicyId` -- the terminal join had taken that identity on the
certificate's word, harmlessly, while nothing was read out of the graph -- and
the statistic must live at the Registry PDA its own digest derives.

### The live case, both markets

`ordinarySelector.live.test.ts` runs cohort-14 market B
(`DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A`) **and market C**
(`BL8zsFokbz7aEdo3wjtcNffd5P1D8a9wVxwKq3mcMsMN`), and asserts on each that the
scale came off the chain (`declared`, exponent `0`), that the join at that scale
reproduces the chain's committed selector, and that the two cells are the pair
the evidence names: **the chain paid cell 2 and the reading at the feed's own
exponent falls in cell 1**, which pays zero. Market C is the one that reached a
stranger -- participant-2 bought 200 claims at index 1 -- so running both is
what makes this an assertion about the defect rather than about one market.

The unit tests hold the coordinate itself: a **negative** shift read back as
`-8` and not as 4,294,967,288, a statistic substituted at its own address
refused, and a reader with no Registry saying so in words.

## Wire cost

The consumption's two new read-only keys: legacy `1,534 → 1,600`, v0 over its
frozen table `733 → 737`. Sixty-six bytes legacy, four over the table, which is
the ratio that route's ALT exists for. `resolution-relayed`'s packet witness,
its README table, `PACKET_LIMIT_2026_09_01.md`'s two rows and the harness's own
`CONSUME_EXTENT` all carry the new pair.
