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

`every-submitted-frame-is-over-the-legacy-packet-maximum` is green at **12**,
and that is not a tolerance. Every transaction this campaign submits is a
LEGACY message of 2,005–2,157 bytes against Solana's 1,232-byte maximum. The
campaign's own packet claim is about a different shape: `packet_census`
compiles each of the four instructions as a **v0 message over a dedicated
Address Lookup Table** and measures 1,135 / 1,191 / 1,191 / 1,071 bytes, all
under the limit, and asserts that.

So: the chain is packet-bounded **through an ALT**, and is not packet-bounded
without one. A validator tier owes the ALT frame, not this one.

## Running it

    tools/gauntlet/run.sh --mode census          # once, for the inventory
    tools/gauntlet/retirement-checkpoint/run-retirement-checkpoint.sh

It builds seven SBF programs, refuses on any stack-frame-overwrite diagnostic,
refuses if the fold is not exactly twelve transactions (a missing file is a
duplicated signature, not a skipped act), evaluates six witnesses, and folds.
It is a **ProgramTest fast lane**: nothing deploys through Loader V3, the
ProgramData accounts are constructed by the campaign, and ProgramTest has no
finalized commitment. `TIERS.md` states the bar.
