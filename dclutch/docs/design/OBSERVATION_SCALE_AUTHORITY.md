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
