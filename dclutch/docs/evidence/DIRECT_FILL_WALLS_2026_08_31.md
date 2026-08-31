# The walls in front of the first local Direct fill

**2026-08-31, lane FINALIZATION.** The fill did not land. This document exists
because four of the walls in front of it did come down, on a validator that no
longer exists, and the sequence reached the second-to-last stage of the route
before the machine was lost. Everything here is either committed code or a
number read off a chain; where a fix is unverified against a chain, this
document says so in the same sentence as the fix.

The mission's own success document, `FIRST_LOCAL_DIRECT_FILL_2026_08_31.md`, is
deliberately **not** written. It belongs to the lane that lands the fill.

## How far it got

On loopback validator `43080`, the six-cell market
`6wLYToyGCRNa39Hjph9L4zgCPE4mjr2wKiHLQSyBDKaK`, generation 10, with the Direct
execution root `9SpRy2Yx…` FILLWIDTH activated:

| stage | signature | slot | CU |
| --- | --- | --- | --- |
| admission | `3d6U5Gp7VAW9niJ9gdVKcmrgFcqqFFGKyiGd4stDXbAYusaADu2vw2mWXn8DoTrgEmRtUgRAL3gtR9NDfyPAfwF3` | 45876 | — |
| replay setup | `26ZoqXrqjiV2Ez9Pd7SsSzKrLZuGb4NzStLnARzPTcB2zLgZj9BPdv7mEmy8edTmaLoa9kYG2MkZ2TgAmu1B9V1z` | 46022 | 178,435 |
| token setup | `37QKH8UCCHkz9Zd4qNYM1nUVdicYRynmBAg7oA1WCD3yeCBrEaQUNrPXqCGuSjQhHv6em6mtStCt87SHpPUHcv25` | 46060 | 113,170 |
| lookup create | `53gFcYcMk1UaW4qiiDV7UQKWDNyTnEGscPjpmNPtxqnN2bHuRmG7pYiLWhyV64BBgt7DU3TWYJLuQA76KxKQAV6a` | 46102 | 10,505 |
| lookup extend ×3 | `28Tm2CiY…`, `4AWun5e1…`, `jc9WaEn4…` | 46141–46215 | 11,657 / 11,660 / 10,780 |
| lookup freeze | `5o85s1yv3eVYNHfYsrC5aKpWKesJ82jnPQU6rK4jTjggMH5xMQKUDKYt3b9cpqH4fk8vEAXzP73hp8zR1Jwt3kLi` | 46251 | 1,517 |
| capability seal | **refused** `0x4008` | — | 24,033 of 1,399,850 |
| Hot execution | never reached | — | — |

Every stage before the seal is the first time this repository has landed it on a
Direct route. The seal is wall 8, and its fix is committed and **unverified
against a chain** — the validator was gone before it could be re-run.

## The eight walls, and who owned each

FILLWIDTH handed this lane one wall and predicted the method: split the refusal
until it names itself. That method found seven more. Four had to be fixed in
code; three were owned by an earlier gate and are recorded so nobody suspects
them again; one is the fixture's.

### Wall 4 — `Direct Hot finalization: Finalization` (fixed, `9c386c57`)

`DirectInlineRouteErrorV3::Finalization` was one unit variant shared by **27**
refusing sites across `prepare_direct_inline_hot_finalization_v3` and its three
helpers. It is now `Finalization(DirectInlineFinalizationRefusalV3)` with 23
named variants covering all 27, each rendering a sentence with observed against
expected, and every multi-clause conjunction reporting **all** failing clauses
rather than the first — PAIRFIX's `refusing_ticket_half_clauses_v1` rule, on the
grounds that an operator who fixes one clause and re-runs has learned almost
nothing.

Three red-proofs are in-crate on the shipped fixture
(`distinct_finalization_clauses_refuse_under_their_own_names`): a substituted
report instruction names `SealedReportProjection(Instruction)`; a buyer who
cannot pay and a buyer whose delegate may spend nothing both name
`Finalizer { error: Candidate, candidate: Some(Binding) }`.

A second test, `earlier_gates_own_the_clauses_the_finalizer_never_reaches`,
records as a **test rather than a comment** which clauses are unreachable
through this entry point, because a clause that never fires is a clause an
operator should stop suspecting:

- an undelegated buyer token and a wrong-width maker replay are `ChildFrame`;
- an **absent** root — the zero-length placeholder a finalized snapshot renders
  for a MISSING account, the exact disguise that cost the population run
  twenty-one refused fills — is `Profile`;
- grown strategy or descriptor bytes are `Seal`.

### The `Candidate` collapse underneath it (fixed, same commit)

The named refusal immediately produced `Candidate`, which is the next collapse
down: six sites in `dclutch-direct-codec` share it, and the largest discards a
whole nine-variant `DirectInlineCandidateErrorV2` through a `map_err`. That
crate compiles into the Trading SBF program, where widening a refusal is on-chain
cost for a diagnosis nobody on chain can read, so the discard is defensible
**there and only there**.

`rederive_direct_inline_candidate_refusal_v3` therefore re-runs the same public
candidate partition with the same five arguments, on the refusal path only. This
is not a second reader of the accounts — there is no reimplemented rule to
drift — it is the one reader, asked again with its own answer kept. It said
`Binding`.

### Wall 5 — the allowance is an equality (fixed, `14da01d5`)

`validate_collateral` in `dclutch-direct-codec`, which is what the Trading
program runs, tests two different things about the buyer's finalized token
account:

- `balance < buyer_collateral_debit` refuses — a floor, as expected;
- `delegated_amount != buyer_collateral_debit` refuses — an **equality**.

The delegation is a single-use authorization spent to zero by the Custody
effect, so an allowance **larger** than the debit is not generous; it is a
different trade's authorization.

`direct_trade_producer.rs` tested one `<` against the admission **report's**
quantity and never read the allowance at all. Measured on chain: the buyer
collateral `GiQdwQaJJSZEuCC22xTk3R9oFbrwm1Lup6g2dnLyRA65` held 100,000,000
atoms delegated to the Custody authority `2kj3vmKPTkzdjKYq5e5toF4tCjLS5eFm3s27Nd1LmXZR`
for 100,000,000, against a debit of 50,250,000
(`100,000,000 × 500,000 / 1,000,000 = 50,000,000` gross, plus a 50-bps floor of
250,000). The producer accepted it, the chain refused it, and nothing anywhere
named the number.

The producer now puts the collateral account in its finalized snapshot and
mirrors both chain clauses, naming what the account holds against what the trade
requires. **The admission report's quantity is deliberately not a clause** — the
chain reads this token account and nothing else at trade time, and adding a rule
the chain does not have is the same drift as missing one, failing in the safe
direction, which is how a producer ends up refusing trades the validator would
accept. It is carried only to be named beside the allowance when they disagree.

Verified: a fresh admission at exactly 50,250,000 atoms produced clean and
walked past this wall.

### Wall 6 — the setup transactions had no compute budget (fixed, `e6d078f8`)

`direct_setup_message_v1` compiled **both** setup transactions as a bare legacy
message holding one instruction, so they ran on the runtime's default 200,000
compute units. `direct_replay_setup_v1`'s Custody CPI alone consumes 131,391 and
the outer frame hit `200,000 of 200,000` and died `ProgramFailedToComplete` — a
runtime abort with no protocol refusal anywhere, which is the least legible
failure this driver can produce.

It was invisible for as long as it was because nothing had ever reached this
stage: every Direct trade before this one refused at wall 4 in front of it.

Fixed through the house `bounded_instructions` helper, and the embedded verifier
now pins the two-declaration prefix by program **and** discriminant (2 is
`SetComputeUnitLimit`, 3 is `SetComputeUnitPrice`) instead of asserting one
instruction — because "ignore the instructions before the interesting one" is
how a substituted prefix gets past a verifier that reads the interesting one
correctly. Verified: replay setup then landed at 178,435 CU, token setup at
113,170.

### Wall 7 — the producer admits only a vacant seller/fee token (NOT fixed)

Once token setup has run on a market, `direct_trade_producer.rs` refuses every
later trade:

```
REFUSED: seller Token-2022 destination was not a System-owned data-empty PDA prestate
```

The check (`direct_trade_producer.rs`, the loop over `seller_token` and
`fee_token`) admits only vacancy. This is **the same defect WALL4 dissolved in
the TypeScript panel on the same day**, and the Rust producer never got the
pass: the chain admits two prestates, not one — vacant System-owned-and-empty,
which token setup requires to CREATE the account, and the initialized base
Token-2022 account for this Realm Mint and this owner, which the trade route
requires to EXECUTE against. The producer should admit both and refuse every
third thing, and the setup stage machine must then skip a token setup whose
accounts already exist.

Left unfixed deliberately: the stage-machine half is not a change worth making
blind, and this lane had no validator left to verify it on. It is why the
four-cell control market could not be retried after its partial run, and it is
the first thing the next lane will hit on any market that has traded once.

### Wall 8 — the seal asks for allocation and sends compute units (fixed, `7623e436`, UNVERIFIED)

The capability seal stage refused with custom program error `0x4008`, which is
`TradingSbfError::HeapFrame`, having consumed 24,033 CU of the 1,399,850 it had
asked for and none of the heap it had not. That error's own documentation names
the remedy: add `ComputeBudgetInstruction::request_heap_frame` and keep the
instructions sysvar in the account frame.

`compile_direct_inline_capability_seal_routed_v0_v3` carried this comment:

> The seal walks every role's record … so like Hot it does not fit Solana's
> default allocation and must declare its own.

and then declared `set_compute_unit_limit` and nothing else. The sentence says
**allocation**, which is the heap; the code granted compute units. The comment
is the witness to its own gap.

Trading's adapter is correct and was recently made so: a CLOSESEAL lane
declared the seal outer's extended heap profile on 2026-08-31 precisely so a
grant would be admissible, and wrote that "a seal transaction that sends no
`RequestHeapFrame` still keeps the default ceiling and still refuses by name —
which is the right shape for a caller who forgot, rather than an unnamed abort."
This caller forgot. The instructions sysvar the adapter reads is already in the
frame: it is `HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3` of the Hot fixed prefix, which
the seal reuses whole.

**Unverified.** The fix compiles; it was never sent to a chain.

## The fixture cannot land a fill, and that is recorded not fixed

`tools/release/private-validator-lifecycle/run.py` sets
`PARTICIPANT_FIXTURE_LIQUIDITY_ATOMS = 100_000_000` and passes it straight to
`--collateral-quantity-atoms`. The Direct producer's own constants are
`FILL_ATOMS_V1 = 100_000_000` at `EXECUTION_PRICE_V1 = 500_000` against
`EXPECTED_PRICE_SCALE_V1 = 1_000_000` with `FEE_BASIS_POINTS_V1 = 50`, so the
buyer debit is 50,250,000. By wall 5, the probe therefore admits **exactly twice
what its own trade debits**, and no Direct fill can ever land from it.

`docs/VALIDATION_BACKLOG.md` still asks the next convergence gate to "require the
explicit partition of 1,000,000,000 founding atoms plus 100,000,000 participant
atoms" — the same number, written down as a requirement.

The fix is to admit the derived debit rather than a round number, and to assert
the produced public manifest's `fill`, `executionPrice` and `feeBasisPoints`
against whatever the probe predicts so a Rust-side drift breaks the probe loudly
instead of silently. It is **not made here**: it changes a 4,000-line probe's
assertion web, and this lane had no validator left to run it on. Making it blind
is the debt hole, not the fix.

FILLWIDTH's separately reported `run.py` defect — the admission argv built
without `--routing-table` — is already closed on main; the flag and its comment
are at the admission call site.

## What the machine took

The substrate did not survive. `/private/tmp` was cleared: validator `43080`,
its ledger, `/private/tmp/dclutch-fillwidth-{probe,run,release}`, this lane's
`/private/tmp/dclutch-finalization-run`, and the cross-lane board file are all
gone. Two activated Direct execution roots at two widths, both markets, both
admissions and every landed signature above exist now only as the numbers in
this document.

One thing worth carrying forward for whoever restages: **`solana-test-validator`
purges transaction history in multi-thousand-slot chunks**, and these drivers
re-verify every earlier stage's transaction from history on every invocation. A
purge mid-sequence strands the journal permanently — the driver refuses
`finalized Direct signature omitted transaction history` and cannot be resumed.
Launch it with a large `--limit-ledger-size`. A restart with the same ledger and
that flag preserves all account state; that was verified here, before the crash.

## Not verified

- The seal heap-frame grant (`7623e436`) against any chain.
- The Hot execution itself, at any width. No Direct fill has landed.
- The four-cell control market end to end; it was left with a partial setup and
  then blocked by wall 7.
- The full `dclutch-operator` test suite: a concurrent lane's in-progress edit to
  `direct_inline_v3.rs` was red in the shared working tree, so only the
  `direct_inline_route_v3::tests` module was run (12 passed). The crate's own
  library compiles clean.
- No devnet read or write of any kind. market19 and TRADE-4's territory were not
  touched.
