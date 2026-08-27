# Decision 0007: a custom program error code names the program that raised it

Status: accepted on 2026-08-27. This is an allocation decision with a
machine-checked authority: `crates/dclutch-refusal-registry` is the registry,
this record documents it, and the gauntlet census enforces it. It is not
release evidence and it makes no claim about which refusals are reachable.

## Context

A Solana `ProgramError::Custom` is a bare `u32`. The runtime reports it as
`custom program error: 0xN` and says nothing about which program produced it.
One transaction can carry several programs' refusals through a CPI chain, and
only the last one survives into the transaction error.

Until this record, every first-party dClutch program numbered its refusals from
zero. `Custom(0)`, `Custom(1)` and `Custom(2)` were each claimed by every
program in the tree simultaneously. Nothing was wrong inside any one program —
each taxonomy was careful, documented and stable — and the defect was entirely
in the space between them: **any reader of a code had to already know which
program it came from, and every reader resolved that by assuming.**

The derpage hunt (2026-08-27, finding 2) named the consequence. The gauntlet
census reads the chain's own logs precisely so it does not have to believe the
harness, then folds them through `reported_custom_code`, which scanned for the
last `custom program error: 0x` in the log and returned the number. It could
not attribute it. A campaign binding that expected `claims/ClaimsSbfError::Release`
(code 3) and got a *test caller's* `DeliberateLateFailure` (also code 3) from a
completely different program was credited as coverage of the Claims refusal.

That specific collision was known and already had an escape hatch: `Binding`
carries an `unnamed_refusal` field, and
`tools/gauntlet/claims-affine-batch/bindings.json` says in its own note that the
code "collides numerically with claims/ClaimsSbfError::Release". Seven binding
files carry twelve such annotations. Every one of them is a place where a human
noticed a collision and wrote it down instead of being able to fix it. The
annotation is honest and it is also the wrong layer: it documents an ambiguity
rather than removing one.

Pre-release is the only free moment to remove it. No wire carries a
compatibility entitlement to these numbers.

## Decision

### 1. The `u32` is partitioned into bands, and a band identifies a program

```text
band = code >> 12          (each band is 0x1000 = 4096 codes wide)
```

- **Band 0 (`0x0000..=0x0FFF`) is never allocated.** A custom code below
  `0x1000` is by construction *not* a first-party dClutch refusal. This is the
  load-bearing half: SPL Token, the Loaders and every other foreign program
  number from zero, and after this decision nothing of ours does, so "is this
  ours?" stops being a judgement call.
- **Bands `0x001..=0x0FF`** belong to on-chain protocol programs.
- **Bands `0x100` and up** belong to test-only caller programs, which exist to
  drive hostile CPI cases under `program-test` and never deploy to a real
  cluster. They are registered rather than ignored: a deliberate late failure
  inside a test caller must never be legible as a protocol refusal, and that is
  exactly the collision the census had been annotating around.

Reading a band off a log line is dropping the last three hex digits.
`custom program error: 0x5100` is band 5, Claims, offset `0x100`.

### 2. The registry crate is the authority

`crates/dclutch-refusal-registry` is `no_std`, zero-dependency and const-only;
it compiles to nothing but constants and links into every program. It holds
`BANDS`, the complete table, and proves its own invariants at compile time
rather than in a test a stale build could skip: the bands are ascending,
disjoint, occupy exactly one band index each, and sit in the tier their
`BandTier` claims.

This table, not this document, is the authority. If they disagree, the table is
right and this document is stale.

### 3. Discriminants stay literal, and are pinned to the band by assertion

A variant is written `Instruction = 0x5100`, not `CLAIMS_REFUSAL_BASE + 0x100`.
The literal is what a person greps for after reading a code out of a validator
log, and an arithmetic expression hides exactly that. What stops the literal
drifting is a pair of compile-time assertions beside the declaration:

```rust
const _: () = assert!(
    ClaimsSbfError::Instruction as u32 == dclutch_refusal_registry::CLAIMS_REFUSAL_BASE,
    "ClaimsSbfError must start at its registered refusal band base"
);
const _: () = assert!(
    (ClaimsSbfError::Token as u32)
        < dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + dclutch_refusal_registry::BAND_SPAN,
    "ClaimsSbfError must not run past its registered refusal band"
);
```

The pattern is not new: `programs/dclutch-trading-sbf/src/execution_strategy_v2.rs`
already bound the published Shadow boundary's codes to `TradingSbfError` this
way. This decision generalises it.

### 4. `#[repr]` on an `*Error` enum declares "these codes are protocol-visible"

The census admits a program's refusal taxonomy only from `#[repr]`-annotated
enums whose name contains `Error` (`census/src/enumerate.rs::collect_refusals`).
That rule is now a contract in both directions: a refusal enum that can reach
the chain as a custom code carries `#[repr(u32)]` and explicit discriminants, so
it is enumerable, and an internal error type that only ever maps into another
enum does not.

`SeriesAccountErrorV3` was the counterexample that motivated this clause. It
carried no `repr` and no discriminants at all, and shifted by `+ 80` inside its
`From` impl:

```rust
ProgramError::Custom(80_u32.saturating_add(value as u32))   // before
```

So it reported `80 + n` on chain while reporting `0 + n` to anything reading the
source, and the census skipped it entirely — an entire account and persistence
boundary of the Series family with no refusal taxonomy in the inventory. It is
now `#[repr(u32)]`, explicit, and inside Trading's `0x4100` sub-band.

### 5. Sub-bands inside a program are the program's own business

Claims established the convention and it generalises: a program with several
independently versioned request families gives each family a round offset inside
its band. Claims' historical *decimal* offsets survive verbatim as *hexadecimal*
ones, so the family structure now reads straight off the code.

| Claims family | was | now |
|---|---|---|
| `ClaimsSbfError` | 0–9 | `0x5000`–`0x5009` |
| `LiabilityBasisSbfErrorV2` | 100–110 | `0x5100`–`0x510A` |
| `ProtocolPositionSbfErrorV2` | 140–150 | `0x5140`–`0x514A` |
| `AffineBatchSbfErrorV2` | 160–167 | `0x5160`–`0x5167` |
| `ClaimsFoundingSbfErrorV5` | 180–189 | `0x5180`–`0x5189` |
| `SignedDeltaSbfErrorV3` | 200–207 | `0x5200`–`0x5207` |
| `RationalLifecycleSbfErrorV2` | 210–218 | `0x5210`–`0x5218` |
| `SparseNativeTransferSbfErrorV1` | 260–267 | `0x5260`–`0x5267` |
| `ClaimsMarketClosureSbfErrorV1` | 500–505 | `0x5500`–`0x5505` |

Trading has one sub-band, `0x4100`, for `SeriesAccountErrorV3`. Sub-bands are
documented here and enforced only by the enclosing band check: interleaving
inside a band is a legibility question, not a correctness one.

## The allocation

| band | base | label | package | tier |
|---|---|---|---|---|
| `0x001` | `0x0000_1000` | `registry` | `dclutch-registry-sbf` | Program |
| `0x002` | `0x0000_2000` | `rent` | `dclutch-rent-sbf` | Program |
| `0x003` | `0x0000_3000` | `core` | `dclutch-core-sbf` | Program |
| `0x004` | `0x0000_4000` | `trading` | `dclutch-trading-sbf` | Program |
| `0x005` | `0x0000_5000` | `claims` | `dclutch-claims-sbf` | Program |
| `0x006` | `0x0000_6000` | `custody` | `dclutch-custody-sbf` | Program |
| `0x007` | `0x0000_7000` | `dealer` | `dclutch-dealer-sbf` | Program |
| `0x008` | `0x0000_8000` | `resolution` | `dclutch-resolution-proof-sbf` | Program |
| `0x009` | `0x0000_9000` | `product-runtime-v2` | `dclutch-product-runtime-v2-sbf` | Program |
| `0x00a` | `0x0000_A000` | `direct-aot` | `dclutch-direct-aot-sbf` | Program |
| `0x00b` | `0x0000_B000` | `series-shadow` | `dclutch-series-shadow-sbf` | Program |
| `0x00c` | `0x0000_C000` | `general-accelerator` | `dclutch-general-accelerator-sbf` | Program |
| `0x00d` | `0x0000_D000` | `dealer-accelerator` | `dclutch-dealer-accelerator-sbf` | Program |
| `0x100` | `0x0010_0000` | `test/claims-affine-batch-caller` | `dclutch-claims-affine-batch-test-caller-sbf` | TestCaller |
| `0x101` | `0x0010_1000` | `test/claims-fractional-signed-delta-caller` | `dclutch-fractional-signed-delta-test-caller-sbf` | TestCaller |
| `0x102` | `0x0010_2000` | `test/claims-liability-basis-caller` | `dclutch-claims-liability-basis-test-caller-sbf` | TestCaller |
| `0x103` | `0x0010_3000` | `test/claims-rational-lifecycle-caller` | `dclutch-rational-lifecycle-test-caller-sbf` | TestCaller |
| `0x104` | `0x0010_4000` | `test/claims-rational-v2-caller` | `dclutch-rational-v2-test-caller-sbf` | TestCaller |
| `0x105` | `0x0010_5000` | `test/claims-sparse-chain-caller` | `dclutch-claims-sparse-chain-test-caller-sbf` | TestCaller |
| `0x106` | `0x0010_6000` | `test/claims-terminal-settlement-caller` | `dclutch-terminal-settlement-test-caller-sbf` | TestCaller |
| `0x107` | `0x0010_7000` | `test/custody-caller` | `dclutch-custody-test-caller-sbf` | TestCaller |
| `0x108` | `0x0010_8000` | `test/dealer-accelerator-caller` | `dclutch-dealer-accelerator-test-caller-sbf` | TestCaller |
| `0x109` | `0x0010_9000` | `test/general-accelerator-caller` | `dclutch-general-accelerator-test-caller-sbf` | TestCaller |
| `0x10a` | `0x0010_A000` | `test/resolution-receipt-caller` | `dclutch-resolution-receipt-test-caller-sbf` | TestCaller |

Bands `0x00e`, `0x00f` and `0x010` were drafted for `dclutch-controller-proof-sbf`,
`dclutch-custody-proof-sbf` and `dclutch-claims-proof-sbf` and are **withdrawn,
not retired**: `11ca28b` banished all three DCLTCAT1 proof programs while this
record was being written, before any wire carried the numbers. A band entry for
a program that does not exist reads exactly like a live one, which is the
failure the census's own `TARGETS` comment records paying for once already.

Five other test programs — `dclutch-series-consume-caller-sbf`,
`dclutch-trading-core-caller-test-program`, `dclutch-trading-registry-test-program`,
`dclutch-trading-outer-test-program` and `dclutch-trading-dealer-wave-fixture` —
hold no band because they raise no custom code at all. One that grows a refusal
enum takes the next free test band.

## Deliberate aliases

`ShadowAcceleratorAuthErrorV4` (in `crates/dclutch-shadow-accelerator-auth-v4`)
raises Trading's codes on purpose: the crate is Trading's *published boundary*,
not a program, and its refusals must be indistinguishable from the ones Trading
would have raised in its place. It is therefore an alias rather than a band, is
listed in the registry's `ALIASES`, and stays bound to `TradingSbfError` by the
assertions already in `execution_strategy_v2.rs`. Any uniqueness check has to
know about it deliberately, or the alias reads as the exact collision this
decision forbids.

## Enforcement

Four layers, each catching what the one above cannot:

1. **The registry's own const assertions** — the table is well-formed.
2. **Per-enum const assertions** — each enum starts at its base and stays inside
   its band. Catches a variant appended past the end, or a base typo.
3. **`dclutch-route-census inventory --check-unique`**, a `run.sh` stage that
   reds on any duplicated code, any code outside every registered band, and any
   band whose codes are declared by a package that does not own it. It sweeps
   wider than the route inventory does — test-program directories included —
   because that is where the collisions actually were.
4. **Program-attributed crediting** in `census/src/ledger.rs` — the fold now
   parses the `Program <id> failed: custom program error: 0xN` line and carries
   the raising program's address with the code, so a refusal is credited only
   when the chain says *that* program raised it. This closes the mirror one
   level up: even a correct band table would not help a reader who assumed which
   program a number came from, and the census no longer has to.

## Consequences

- Renumbering an allocated band is now a wire-compatibility break and needs its
  own decision record. Bands are append-only; a deleted program's band is
  retired, never reused.
- `unnamed_refusal` remains in the binding schema and remains correct — a
  refusal genuinely raised outside the enumerated programs is still not
  first-party coverage. What changes is that it is no longer needed to work
  around a numeric coincidence, and the twelve existing annotations become
  statements about *which program* rather than *which number*.
- A test that asserts a refusal by matching the substring `Custom(3)` was always
  a bad test — it also matches `Custom(30)` — and is now a broken one. Those
  sites derive their expected code from the enum instead.
- Codes are four to six hex digits rather than one. That is the cost, and it
  buys the reader the program identity in the same glance.
