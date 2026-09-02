# retirement-checkpoint — the aggregate-retirement chain, on real ELFs

This tier witnesses seven routes that row **C-10** owned and no campaign drove:
the whole checkpointed terminal lifecycle of a market, from the Claims handoff
to the last rent lamport reaching the immutable refund wallet.

Nothing about it is new coverage. `crates/dclutch-svm-harness/tests/market_retirement_v1_lifecycle.rs`
has driven this chain against real Core, Claims, Custody, Registry, Resolution,
Trading and Rent ELFs since it landed. What was missing was **evidence
emission**: the campaign called `record()` for nothing, so `docs/reference/routes.md`
read `NEVER-EXECUTED, no stated reason` for all seven while the suite was green.
That is the register reporting an absence that was really an instrument gap —
the same gap `claims-fractional-atomic` closed for the fractional claim-check
life, and the thing the unwitnessed-routes list warned about when it said an
unwitnessed route "is a statement about coverage and not about correctness".

The campaign is unchanged apart from twelve call sites that now go through the
harness's shared `submit_recorded`. The other test in the same file — the
LEGACY atomic retirement — is deliberately left unlabelled, and the runner
filters to the checkpointed test so the fold contains exactly the chain.

## The chain

Four accepted acts and eight hostiles, one transaction each:

| act | CU | what it does |
|---|---|---|
| prepare | 225,798 | hands the Claims aggregate to Core; Claims + Core in the frame |
| close-vault | 229,762 | closes the HoardPrincipal vault; Core + Custody + SPL Token |
| close-replay | 220,472 | closes the Custody replay; Core + Custody, **no token frame** |
| finish | 166,219 | closes the checkpoint, the Market and the RentCredit; Core + Rent |

The eight hostiles carry four distinct refusal codes —
`claims/ClaimsMarketClosureSbfErrorV1::Identity`, `core/CoreSbfError::Market`
(four of them), `core/CoreSbfError::ChildAck`, `core/CoreSbfError::AccountFrame`
— and each has its own label, because a shared label would let the census read
all four `Market` refusals off whichever ran first.

## The packet position, stated once so it cannot drift

Until 2026-09-02 the campaign had two packet claims and only one instrument.
Every transaction it submitted was a **legacy** message of 2,005–2,157 bytes
against Solana's 1,232-byte maximum, recorded by
`every-submitted-frame-is-over-the-legacy-packet-maximum` as a defect at 12; and
beside it `packet_census` computed a *model* of a different shape — the same
instructions over a synthetic lookup table — and asserted the model fit. The
model was right and it was never submitted.

The campaign now submits what it claims.

| route | legacy | over | v0 over the chain's frozen table |
|---|---:|---:|---:|
| prepare | 2,101 | +869 | **1,083** |
| close-vault | 2,157 | +925 | **1,139** |
| close-replay | 2,157 | +925 | **1,139** |
| finish | 2,037 | +805 | **1,019** |
| finish, substituted-wallet hostile | 2,005 | +773 | **1,018** |

**One table for the chain, not one per route.** The four 35-meta frames share
their coordinates, so a table per route would be a rent per route for a single
market's retirement, and it is built before the chain runs — which is what a
real controller must do, since the addresses have to be finalized before the
first submission can resolve them. 34 addresses become one-byte indexes; two
stay static, the payer and the invoked program, which no table can move. The two
`finish` figures differ by one byte because the hostile collapses two
coordinates onto one address and so resolves 33 rather than 34.

`packet_census` is gone, replaced by `unique_account_locks`. Bytes are now
measured where they are submitted, so there is no second byte model to drift
from the first; what the function still answers is the wall a packet fix cannot
move — **36 unique locks per frame** against the runtime's 64, which a table
does not change because a looked-up address is locked exactly like an inline
one.

These figures exclude the ComputeBudget instructions. The design record
(`docs/evidence/AGGREGATE_RETIREMENT_CHECKPOINT_SPLIT_2026_08_28.md`) states
1,135 / 1,191 / 1,191 / 1,071, which is these numbers plus exactly 52 — a
compute-unit limit at 40 bytes and a priority fee at 12. The two agree.

**The chain is now DATA-bound.** The aggregate retirement had already been split
into four transactions to get this far, and the split alone does not reach the
packet; the table is what carries it. The requests are 744–864 bytes and no
table touches those, so if a request grows there is no third lever.

## Running it

    tools/gauntlet/run.sh --mode census          # once, for the inventory
    tools/gauntlet/retirement-checkpoint/run-retirement-checkpoint.sh

It builds seven SBF programs, refuses on any stack-frame-overwrite diagnostic,
refuses if the fold is not exactly twelve transactions (a missing file is a
duplicated signature, not a skipped act), evaluates seven witnesses, and folds.
It is a **ProgramTest fast lane**: nothing deploys through Loader V3, the
ProgramData accounts are constructed by the campaign, and ProgramTest has no
finalized commitment. `TIERS.md` states the bar.
