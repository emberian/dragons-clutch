# Direct market geometry — 2026-08-27

Evidence level: **solana-program-test against real role ELFs** (harness
evidence). Not a local validator campaign, not devnet, not mainnet. Nothing
here claims any program is formally verified; the Lean theorems named below are
theorems about an authored transition program, and the unverified runtime
boundary is everything the SVM does with it.

This document records what the ordinary Direct artifact family does as a
market's geometry changes, because a wall was named there and the measurement
says it is not one.

## 1. A geometry is one number

Product Runtime V2 enforces `region_count = cut_count + 1` on decode
(`crates/dclutch-product-runtime-v2/src/lib.rs:134-141`) and defines
`outcome_count = region_count + 1` — the ordinary regions plus the explicit
failure outcome (`:202-211`). So

```
outcome_count = cut_count + 2
```

exactly, and a market's geometry is a single free number rather than a
`(claims, cuts)` pair. A pair that does not satisfy it is not a market the
protocol can found: the AccountProfile refuses it with `Geometry`.

The canonical demo geometry is **three outcomes, one cut** — `cuts = [0]`,
`coefficients = [1, 1, 1]` in the executable fixture
(`programs/dclutch-trading-sbf/program-test/direct-hot/src/fixture.rs`). Two
descriptions in circulation before this lane, "3-claim/3-cut" and
"3-claim/2-cut", are both inadmissible geometries and neither is the canonical
one.

`DirectOrdinaryGeometryV3`
(`crates/dclutch-direct-codec/src/ordinary_geometry_v3.rs`) is the one owner of
that arithmetic. It constructs from either count, answers the four
runtime-width records a market of a given width must present, and reads a
geometry back out of one set of observed widths — refusing a set that states
two rather than resolving it to whichever record is read first.

## 2. The artifacts do not move with the geometry

Every runtime-width account the AccountProfile pins is stated as an affine
`(data_length, data_item_stride)` rule against the transaction's own Product
tail count, never as a resolved width, and the family declares
`item_account_stride = 0`, so no coordinate appears or disappears with the
geometry. The LifecycleV5 policy's created records carry `data_stride = 0`;
the EffectV4 program declares five `Once` routes and no per-item route, and its
one geometry-derived value is a register READ of `SCALAR_OUTCOME_COUNT_V3`. The
Lean transition program folds its item body once per tail coordinate at
runtime; its 1,712 encoded bytes are a constant.

Measured, at two layers:

| control | range | result |
|---|---|---|
| emitted Profile14 bytes | 2–16 outcomes | byte-identical to the canonical emission |
| artifact bundle, ProgramSet, and all pinned identities | 2–16 outcomes | byte-identical; `DIRECT_HOT_FIXTURE_DESCRIPTOR_ID_V5` and `DIRECT_HOT_FIXTURE_PROGRAM_SET_ID_V5` unchanged |

(`the_emitted_profile_is_byte_identical_at_every_admissible_geometry`,
`the_artifacts_are_the_same_bytes_at_every_geometry`.)

Six Lean theorems state the same thing about the authored program where a Rust
translation can be checked against it: the scalar bank is affine in the tail
count and the identity bank does not move at all; the two-, four- and
five-outcome geometries admit the canonical frame with the same derived quote;
and both refusals hold off the canonical geometry — a traded outcome outside a
four-outcome tail, and a market authenticated at four outcomes executed against
a tail of three, where the fold writes nothing and the epilogue refuses a fill
no item coordinate accounts for
(`formal/dclutch-semantics/DClutchSemantics/DirectOrdinaryV3.lean`).

## 3. The campaign

`programs/dclutch-trading-sbf/program-test/tests/registry_hot_continuation.rs`,
through the Registry continuation, at the 1,400,000 CU protocol ceiling and the
default 32,768-byte BPF heap, with a v0 message and a lookup table.

Role ELFs, staged in `target/deploy`, **built before this lane** — no program
source was changed:

| program | bytes | SHA-256 |
|---|---:|---|
| `dclutch_registry_sbf.so` | 207,072 | `e1f4a20f0fefb60ad8f809f153c4403363d298d5eb11b88e29abe404048ac6e1` |
| `dclutch_trading_sbf.so` | 1,325,848 | `7facb8e58e45843f46b9d3d572ced5e45507bfcbfb2250e865b5427baa1b9d3c` |
| `dclutch_core_sbf.so` | 934,088 | `e0cc7109da7a7b2b94cfa5a0f00a63c40ce44519f7d0186b6c1fbfe39b68f0ee` |
| `dclutch_claims_sbf.so` | 1,010,496 | `51967830f17ab6ebad074fbaf178482c027910bc9d14a8ade070e17004b84b8a` |
| `dclutch_custody_sbf.so` | 360,328 | `d171cf742391dcc6ff152171657187d6a62538f38cedc9ce048af457b16746f1` |
| `dclutch_rent_sbf.so` | 137,608 | `3b857b2236522c29e17b7d73cf27df6e6028fd8298a52df386753638f915ff79` |

**Provenance caveat, stated rather than glossed:** these ELFs were staged by an
earlier build of the tree, not produced by `git archive` of this document's
commit, so they are not bit-reproducible from it. What that costs is precision
about which source produced them; what it does not cost is the finding, because
this lane changed no program source at all and the claim is about artifacts and
markets rather than about program bytes. `cargo build-sbf` of
`dclutch-trading-sbf` at HEAD reports **zero** frame diagnostics.

### 3.1 The named wall: a four-outcome market

`a_four_outcome_market_trades_on_the_canonical_artifacts`. Two cuts, three
ordinary regions, four outcomes; a wider result domain (272 bytes), portfolio
(240), Claims aggregate (288) and both Positions (160); a Product tail of four.
It selects the same descriptor, the same validated-artifact seal, the same
one-entry ProgramSet and the same six artifacts as the canonical market.

Executed at **1,363,637 CU**, moving the identical collateral as the canonical
geometry — a fill of ten at fifty against a scale of one hundred, source 95 and
destination 35 — because the geometry must not move the economics: the traded
outcome is one coordinate of the tail either way, and the epilogue requires the
other coordinates' Claims quantities to sum away.

### 3.2 The sweep

`the_widest_geometry_the_shipped_hot_path_can_trade` (`#[ignore]`d; about a
minute of real-ELF execution). Every geometry from the protocol floor upward,
on one artifact set:

| outcomes | cuts | CU | | outcomes | cuts | CU |
|---:|---:|---:|---|---:|---:|---:|
| 2 | 0 | 1,352,967 | | 17 | 15 | 1,343,033 |
| 3 | 1 | 1,341,795 | | 18 | 16 | 1,366,371 |
| 4 | 2 | 1,363,637 | | 19 | 17 | 1,377,759 |
| 5 | 3 | 1,367,471 | | 20 | 18 | 1,353,049 |
| 6 | 4 | 1,365,311 | | 21 | 19 | 1,353,887 |
| 7 | 5 | 1,358,649 | | 22 | 20 | 1,377,225 |
| 8 | 6 | 1,333,997 | | 23 | 21 | 1,366,063 |
| 9 | 7 | 1,390,325 | | 24 | 22 | 1,369,901 |
| 10 | 8 | 1,371,661 | | 25 | 23 | 1,358,739 |
| 11 | 9 | 1,369,501 | | 26 | 24 | 1,367,077 |
| 12 | 10 | 1,359,839 | | 27 | 25 | 1,358,915 |
| 13 | 11 | 1,366,677 | | 28 | 26 | 1,382,253 |
| 14 | 12 | 1,352,515 | | 29 | 27 | 1,351,591 |
| 15 | 13 | 1,353,353 | | 30 | 28 | 1,385,429 |
| 16 | 14 | 1,366,195 | | **31** | **29** | **did not fit** |

**MEASURED-PROFILE BOUND: thirty outcomes, twenty-eight cuts.** Thirty-one
exhausts the ceiling, and the sweep asserts that it exhausts COMPUTE rather
than being refused its shape — a geometry refusal at any width would mean a
market's dimensions had reached an artifact after all.

The striking part is that compute is nearly **flat** in the geometry across
that whole range, and non-monotone throughout: 1,333,997 CU at eight outcomes
and 1,390,325 at nine. The per-outcome cost — three folded TransitionVM
instructions, two projected scalar registers, one row in each runtime-width
record — is smaller than the run-to-run variation in PDA bump-seed searching,
which moves with the market's content-addressed identities and therefore with
the geometry. Thirty is where the accumulated tail finally overruns a hot path
already sitting at roughly 96% of the ceiling for its own reasons. It is a
property of that hot path, not a Direct-family width limit, and it will move —
in either direction — with every hot-path compute change.

The routine gate is `the_family_trades_every_geometry_it_is_given`, which
trades nine consecutive geometries (two through ten outcomes) on the one
artifact set and costs about eight seconds. `registry_hot_continuation` reads
18 passed, 1 ignored.

## 4. What this does not establish

- **No Direct entry has been activated on a public cluster.** Everything above
  is harness evidence. Devnet activation and its transcript are not here.
- **Prestate is untouched.** A market still needs its Claims aggregate and both
  Positions to exist before anything can trade, and Claims admission remains
  behind the Hot gate. This lane moved nothing there.
- **The bound is a measurement, not a guarantee.** Thirty outcomes is what the
  staged ELFs did on one machine on one day. Re-measure rather than quote it
  after any hot-path change.
