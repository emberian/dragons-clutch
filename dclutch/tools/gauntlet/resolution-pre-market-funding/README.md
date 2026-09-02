# resolution-pre-market-funding — the pre-Market funding pair, as census evidence

A ProgramTest fast lane over
`crates/dclutch-svm-harness/tests/pre_market_resolution_funding.rs`, against the
real compiled Core, Registry, Rent and Resolution ELFs plus a real SBF
Trading-shaped caller. It drives the two routes that let a Market's Resolution
funding exist **before the Market does**:

- `pre_market_funding_v1::process_pre_market_funding_v2` (`DCLRPMF2`) — project
  the future Market through Core's `ProjectFound36` and initialize its
  Resolution-owned funding ledger, all three rows Pending.
- `pre_market_funding_abort_v1::process_pre_market_funding_abort_v1`
  (`DCLRPMA1`) — the prepared checkpoint expired, so unwind it: every native
  principal lamport back to the funding source, the rent to the Market's
  persisted RentCredit beneficiary, and a canonical abort receipt.

```sh
tools/gauntlet/resolution-pre-market-funding/run-resolution-pre-market-funding.sh
```

## What this is NOT

1. **Not validator evidence.** Nothing deploys through Loader-v3 and ProgramTest
   has no finalized commitment.
2. **Not a Trading claim.** The caller is a test program that makes the same CPI
   Trading would; it is not Trading.

## The fast-lane bar, answered one at a time

- **Loader-v3 / ProgramData / `SetAuthority`.** Not depended on; immutable
  ProgramData bodies, no authority transition.
- **Packet serialisation.** Depended on, so **measured**, and this is where the
  tier's sharpest finding is — see below.
- **Compute and heap.** ProgramTest's compute maximum is exactly Solana's
  1,400,000 and is never raised. The largest observed consumption is **488,773
  CU** on the initializer: 35% of the whole ceiling for one pre-Market
  bootstrap, and the largest single figure C-09 measured anywhere.
- **Real Agave account shapes.** Core state, the Registry records, the RentCredit
  account and the funding ledger are the real encoders' output.
- **Frame diagnostics.** The runner counts SBF stack-frame-overwrite diagnostics
  per artifact and refuses to run at all if the count is nonzero.

## !! THE INITIALIZER OVERRUNS A LEGACY PACKET BY 565 BYTES !!

Measured 2026-09-01, first measurement of this family. Legacy maximum 1,232
bytes (`PACKET_DATA_BYTES`).

| transaction | bytes | over |
|---|---:|---:|
| initialize the Pending ledger | 1,797 | **+565** |
| the two initializer hostiles | 1,765 | **+533** |
| abort (both) | 1,002 | fits, by 230 |

This is not the marginal Found31 overrun. **The pre-Market initializer cannot be
submitted on a legacy message at all** and needs a v0 message over an Address
Lookup Table. The asymmetry is worth stating plainly: *unwinding* a pre-Market
funding is submittable by anyone on a plain legacy packet; *creating* one is not.

## Three hostiles that had no word for what they found

Until 2026-09-01 all three of this campaign's hostiles asserted only
`is_err()` — which per `AGENTS.md` is a test of nothing, since it passes on
whatever the transaction refuses first. Recording them is what showed that **two
of the three refuse in different programs**:

| hostile | raiser | code | refusal |
|---|---|---|---|
| funding source aliased into `ProjectFound36` | Resolution, depth 2 | `0x8000` | `ResolutionError::AccountFrame` |
| internal `ProjectFound36` alias | **Core, depth 3** | `0x3001` | `CoreSbfError::AccountFrame` |
| surplus ledger dust on abort | Resolution, depth 2 | `0x800e` | `ResolutionError::Funding` |

The first two are the pair worth staring at: the same accusation — an alias —
caught by two different programs at two different CPI depths, spending 64,470
and 90,024 CU respectively, and utterly indistinguishable under the shared
`is_err()` they used to share. The campaign's own assertions now name what the
census names, and each was proved red individually by weakening its expected
discriminant before it was trusted green.

One asymmetry, stated because it is a real limitation rather than a choice: this
harness workspace takes no dependency on the Core program crate, so the Core code
is asserted **in the binding** (where the census derives it from the inventory
and checks that the chain says Core raised it) rather than as a literal in the
test. What the test itself can derive without that dependency — that the refusing
band is *not* Resolution's — it does assert, and that is the half that
discriminates.
