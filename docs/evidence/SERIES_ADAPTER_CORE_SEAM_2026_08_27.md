# The Series adapter's Core seam does not exist — 2026-08-27

## Evidence boundary

This is a **static reachability finding**, not a campaign. Nothing here was
measured on a validator or in ProgramTest. It is a claim about what the code
in this tree can and cannot do, and every step below names the exact file and
line so it can be rechecked or refuted. Where it says a route "cannot succeed",
that is an argument over the dispatch structure, not an observation of a
failing transaction — no transaction has ever reached these routes at all,
which is precisely the problem.

## Headline

**Every route of `programs/dclutch-series-sbf` is unreachable-to-success
against the `programs/dclutch-core-sbf` in this tree.** All three routes end in
a Core CPI that Core's dispatch refuses by construction, so the best outcome
any of them can reach is `SeriesSbfError::RoleCpi`. This is not a missing test
and not a missing tier. It is a seam that was never joined, on a Series
representation the rest of the tree has since moved off.

The census records four `series/*` routes as NEVER-EXECUTED with the reason
"no tier deploys it". That reason is true but it understates the situation: a
tier that deployed it would still not execute a single route.

## The argument

### 1. The adapter always calls Core, on every route

`programs/dclutch-series-sbf/src/lib.rs:110-137` dispatches on instruction
length to exactly three routes: `bootstrap` (`BOOTSTRAP_BYTES_V1`),
`fund_ticket` (`TICKET_BYTES`), and `execute_transition` (`REQUEST_BYTES`).

- `bootstrap` ends at `invoke_core(..., SeriesCoreActionV1::Prepare, ...)`
  (`lib.rs:217-226`).
- `fund_ticket` ends at `invoke_core(..., SeriesCoreActionV1::Prepare, ...)`
  (`lib.rs:266-275`).
- `execute_transition` calls `stage_prepared` (`lib.rs:288`), which selects
  `Consume`, `Expire`, or `Close` and calls `invoke_core` (`lib.rs:355-386`).

There is no route that returns `Ok` without `invoke_core` returning `Ok`.

### 2. The CPI carries a bare request with no receipt tail

`invoke_core` builds the child instruction with
`data: prepared.request_bytes.to_vec()` (`lib.rs:844`), where `request_bytes`
is `[u8; SERIES_CORE_REQUEST_BYTES_V1]` produced by `request.encode()`
(`lib.rs:774`). `SERIES_CORE_REQUEST_BYTES_V1 = 336`
(`crates/dclutch-market-core-codec/src/generated_physical.rs:5`). The
instruction data is therefore **exactly 336 bytes**, always.

### 3. Core's dispatch requires a tail that is not there

`programs/dclutch-core-sbf/src/lib.rs:176-232` handles
`SERIES_CORE_REQUEST_MAGIC_V1` in two arms, each gated on a trailing receipt:

| arm | required tail | constant | source |
|---|---|---|---|
| `series_open::process` | Claims FoundingV5 receipt | `CLAIMS_FOUNDING_RECEIPT_BYTES_V5` = **1008** | `crates/dclutch-claims-svm/src/founding_v5.rs:17` |
| `series_consume::process` | projected-Custody Lock receipt | `PROJECTED_CUSTODY_LOCK_RECEIPT_BYTES_V1` = **320** | `crates/dclutch-custody-contract/src/projected.rs:34` |

Each arm computes `instruction_data.len().checked_sub(TAIL)` and then
`.filter(|start| *start >= SERIES_CORE_REQUEST_BYTES_V1)`. At `len == 336`:

- `336.checked_sub(1008)` is `None` — the Claims arm is skipped.
- `336.checked_sub(320)` is `Some(16)`, and `16 >= 336` is false — the Custody
  arm is skipped.

Control reaches `return Err(CoreSbfError::Instruction.into())` at
`core-sbf/src/lib.rs:232`. The adapter maps any CPI failure to
`SeriesSbfError::RoleCpi` (`series-sbf/src/lib.rs:855`).

### 4. Even with a tail, three of the four actions have no handler

`SeriesCoreActionV1` has four variants — `Prepare = 0`, `Consume = 1`,
`Expire = 2`, `Close = 3`
(`crates/dclutch-market-core-codec/src/physical.rs:591-600`). Both Core arms
refuse anything but `Consume`:

- `programs/dclutch-core-sbf/src/series_open.rs:253` —
  `if request.action() != SeriesCoreActionV1::Consume { ... }`
- `programs/dclutch-core-sbf/src/series_consume.rs:296` — identical.

Grepping `programs/dclutch-core-sbf/src/` for `SeriesCoreActionV1::Prepare`,
`::Expire`, or `::Close` returns nothing. So the two routes that send `Prepare`
(`bootstrap`, `fund_ticket`) and the two transition actions `Expire` and
`Close` have no counterpart in Core at all, tail or no tail.

## Why this happened: two Series representations, one seam

`dclutch-series-sbf` is the only consumer of `crates/dclutch-series-codec` in
the tree. Everything else that speaks Series speaks
`crates/dclutch-series-v3-kernel`:

| consumer | Series crate |
|---|---|
| `programs/dclutch-series-sbf` | `dclutch-series-codec` |
| `programs/dclutch-core-sbf` | `dclutch-series-v3-kernel` |
| `programs/dclutch-trading-sbf` (`src/series/`) | `dclutch-series-v3-kernel` |
| `programs/dclutch-series-shadow-sbf` | `dclutch-series-v3-kernel` |
| `crates/dclutch-operator` | `dclutch-series-v3-kernel` |

Core's live Series seam is the v3 one, reached through
`dclutch_series_v3_kernel` composition (`crates/dclutch-series-v3-kernel/src/
composition.rs:50`, `src/lib.rs:941`) and driven today by Trading's
`src/series/` and by `programs/dclutch-core-sbf/tests/found_program_test.rs`,
which stands up a real-ELF `series_consume` campaign with a caller program at
`programs/dclutch-core-sbf/test-programs/series-consume-caller/`.

`dclutch-series-sbf` is a parallel authority path for the same concept, on the
older representation, with a Core seam that was never built on the Core side.
Its 12 unit tests all pass; they exercise the adapter's own frame,
identity, and codec checks, none of which is the seam.

## What this needs: a decision, not a test

AGENTS.md: *"Do not preserve parallel legacy/current authority paths. When a
successor is accepted, delete the superseded path in the same convergence
cycle."* The successor was accepted — Core, Trading, the Shadow accelerator and
the operator are all on v3. Two admissible outcomes, and this lane cannot pick
between them alone because the second is a protocol decision:

1. **Delete** `programs/dclutch-series-sbf` and `crates/dclutch-series-codec`,
   remove `dclutch-series-sbf` from the census `TARGETS`, and drop the four
   `series/*` blocking entries. This is what the "delete the superseded path"
   rule says on its face, and nothing in the tree depends on either crate.
2. **Re-seam it onto v3**: give Core handlers for `Prepare`, `Expire`, and
   `Close`, and give the adapter the receipt tail for `Consume`. This is a real
   protocol expansion — `Prepare` in particular is a prepayment step Core does
   not currently have a concept of — and would need its own ADR.

Doing neither is the only outcome that is definitely wrong, because today the
census counts four routes and twelve refusal codes as merely un-driven, when
in fact no campaign anyone writes can drive them.

## What is NOT claimed here

- Not claimed: that `dclutch-series-sbf` is wrong, or that its unit tests are
  wrong. They test what they test and they pass.
- Not claimed: that the v3 Series path is complete. `core/series_open` and
  `core/series_permit_expiry` are still NEVER-EXECUTED, and the joined founding
  composition they sit inside is another lane's.
- Not claimed: any measurement. No transaction in this document was executed.
