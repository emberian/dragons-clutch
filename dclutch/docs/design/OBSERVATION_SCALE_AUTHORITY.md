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
