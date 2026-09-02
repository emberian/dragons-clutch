# resolution-relayed — the `RelayedMainnetStateV1` family, as census evidence

A ProgramTest fast lane over `crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs`,
against the real compiled Core and Resolution ELFs. It drives all six actions of
the closed `RelayActionV1` set — create, append, seal, retire, consume, and the
funded liveness walk of `MAINNET_STATE_RELAY.md` §4.8/§12.7 — and eight routes
render EXECUTED from it.

```sh
tools/gauntlet/resolution-relayed/run-resolution-relayed.sh
```

## What this is NOT

Three separate disclaimers, because they are three separate things and running
them together is how a reader ends up believing the strongest one.

1. **Not validator evidence.** Nothing here deploys through Loader-v3 and
   ProgramTest has no finalized commitment. `slot` orders a campaign and proves
   nothing about finality.
2. **Not provider evidence.** The Ed25519 signatures are cryptographically real
   and the runtime's own precompile verifies them before the program runs.
   Everything they attest is synthetic: the account bytes are fixtures, the
   "mainnet" slot is a number, and the relayer key is generated in the test. The
   honest sentence about the strongest row is *"the bank accepted an attestation
   asserting mainnet state,"* never *"the market observed mainnet."*
3. **Not full coverage of the campaign.** The campaign's hostile corpora run,
   and are deliberately not recorded. A campaign that labelled every transaction
   it happens to send would be claiming coverage no binding was written for.

## The fast-lane bar, answered one at a time

`TIERS.md` says a fast-lane claim asserted in aggregate is unfalsifiable. So:

- **Loader-v3 / ProgramData / `SetAuthority`.** The tier does not depend on
  them. It installs immutable ProgramData bodies for Core and Resolution and
  never exercises an authority transition. The one deployment fact it *does*
  test — P-B, the venue's pinned `ArtifactReleaseV1` — is authenticated against
  a deployment reconstructed from *attested* Loader V3 bodies, which is a
  decode-time comparison rather than a loader behaviour.
- **Packet serialisation.** The tier does depend on it, so it measures rather
  than asserting. Every recorded transaction carries `wire_bytes` the campaign
  serialised itself, and two witnesses read them back:
  `the-deadline-walk-fits-a-legacy-packet` (true — the walk is 991 bytes and
  stays legacy on purpose) and
  `the-two-over-limit-relayed-routes-ride-v0-lookup-tables`, which pins what the
  conversion bought. See below.
- **Compute and heap.** The campaign sets ProgramTest's compute maximum to
  exactly Solana's 1,400,000 and never raises it;
  `relayed-fits-the-compute-maximum` checks the largest observed consumption
  against that limit. No budget here is a gate; the numbers are measurements.
- **Real Agave account shapes.** Core, Registry and Rent state are the real
  encoders' output; the activation cache is a real
  `ActivatedExecutionReleaseSetV1`; the funding compartments are built by the
  same two production calls `core_effect::new_funding` makes
  (`FundingStateV1::new` then `activate`) and land wherever
  `CapabilityFundingDerivationV1` puts them.
- **Frame diagnostics.** `cargo build-sbf` exits ZERO when the SBF backend
  reports that a call overwrites its own stack frame. That is not hypothetical
  in this family: the deadline walk arrived with **nine** such diagnostics
  against `process_commit_deadline_failure` and built green. The runner counts
  them per artifact and refuses to run the campaign at all if the count is
  nonzero.

## Two transactions did not fit a legacy packet, and the fix is not the same fix

Measured 2026-08-27, the first time this family's wire extents were measured at
all; converted 2026-09-02. Packet maximum 1,232 bytes.

| transaction | legacy | over | v0 over its frozen table | static / looked up |
|---|---:|---:|---:|---:|
| `relayed consumption` (both markets) | 1,534 | +302 | **733** | 3 / 27 |
| `relayed transport: append observation 2` | 1,377 | +145 | **1,196** | 4 / 7 |
| `append observation 0 / 1 / 3` | 989 / 998 / 993 | fit | 808 / 817 / 812 | 4 / 7 |

Neither could be submitted by a real relayer on a legacy message. Both now
execute as v0 messages over a table frozen for the route, whose addresses are
derived from the route's own metas by the message compiler rather than by a
filter this campaign wrote.

**The two are not the same kind of overrun, and the figures are what shows it.**
Consumption is key-heavy: 27 of its 30 addresses become one-byte indexes and it
lands at 733, comfortably under half the maximum. The append is **data-bound**.
Only seven of its addresses are eligible for a table — the relayer and the
worker sign, and the ed25519 precompile and Resolution are invoked — so the
table buys a constant **181 bytes** on all four appends, and observation 2 lands
at 1,196 with **36 bytes of headroom**. That is not a margin to plan on. If the
chunk grows, the lever is the relay's own commit-don't-inline seam — the append
*is* the chunk, and a smaller one keeps the sealed digest exactly as it is — not
a wider table, because there is no wider table to be had.

All four appends ride the table, not just the one that was over. The route is
one route, and a client that picks its envelope by payload size is a client with
two code paths and one name for them.

**The walk is deliberately not on that list.** It is the one route in this
family that has to work when nobody is cooperating, so it must not depend on an
Address Lookup Table a silent operator might never have published. Its
twenty-two-account frame is 991 bytes and rides a bare legacy message.


## Files

- `bindings.json` — ten labelled transactions, the routes each credits, and the
  refusal each hostile case must reach.
- `witnesses.json` — seven witnesses, none of them a number read back out of the
  code under test.
- `programs.json` — the campaign's pinned fixture program addresses.
- `run-resolution-relayed.sh` — build (with the frame-diagnostic gate), run,
  fold, check witnesses, observe.

## What happens after the walk

The walk ends with a market nobody resolved sitting Terminal on its own terms.
Two things now execute past that point, and until 2026-08-29 neither did.

**Core terminalizes it.** `crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs`,
`a_market_walked_to_failure_ends_terminal_on_its_pre_disclosed_terms`: Core admits
the `ResolutionFailure` certificate a walk produced, so `terminal_winner` becomes
the Product's own pre-disclosed failure region and the certificate lands at a
different PDA from the one a provider-resolved terminal would occupy.

**The holder exits.** Two Claims campaigns now redeem collateral against that
certificate, which no route in the tree had ever done — every executed terminal
settlement, in every campaign, had settled a certificate a provider stood behind.

| Route | Campaign | Test |
|---|---|---|
| Wallet, role `Claims`, 36-account frame over an ALT, holder signs for itself | `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs` | `a_wallet_held_position_exits_at_failure_terms_when_nobody_resolved_the_market` |
| Fractional, role `Trading`, 44-account enclosing frame | `programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/fractional_atomic.rs` | `a_holder_exits_at_failure_terms_when_nobody_resolved_the_market` |

Both assert conservation rather than success: the Hoard falls by exactly what the
recipient gains, the pair sums to its opening total, the settled coordinate is
burned out of the holder and out of the aggregate's outstanding supply, every
other coordinate stays byte-identical, and the Custody replay cursor advances
exactly once.

Both are pinned by hostiles that are ONE FIELD from a case that commits, because
`ResolutionCertificateV2::validate_terminal_product` reserves the Product's final
coordinate for explicit failure and admits an ordinary success strictly below it:
a provider success may not occupy the failure region, and failure terms may not be
claimed for an ordinary coordinate. The second differs from an already-committing
redemption by exactly one byte — the certificate kind — and both refuse `0x5002`
(`ClaimsSbfError::Identity`) inside the Claims ELF after the Custody composition,
the Realm and the certificate account have all authenticated.

The bounty is the link between the two halves: a `ResolutionFailure` certificate
whose `work_paid` is zero is refused by `validate_shape`, so the same fact that
lets a holder exit is the fact that records the walker being paid.

**Still owed.** `TerminalScenarioV3::Failure` — the arm where a `GradedExactComplement`
basis pays out its own pre-disclosed `failure_payouts` partition — has never
executed against a real ELF. Both Claims campaigns carry `CategoricalQ1` bases,
for which a `ResolutionFailure` certificate maps to `Categorical(terminal_winner)`
instead (`programs/dclutch-claims-sbf/src/terminal_certificate_v3.rs:86-105`).
Reaching it needs a Claims campaign with a graded basis and a failure-payout
partition, which is a fixture, not a parameter.
