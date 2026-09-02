# resolution-sponsored — the sponsored-push family, as census evidence

A ProgramTest fast lane over
`crates/dclutch-svm-harness/tests/sponsored_push_lifecycle.rs`, against the real
compiled Core and Resolution ELFs. It drives **all five** actions of the closed
`SponsoredPushActionV1` set — Capture, Settle, CloseCandidate, CloseHead,
CommitFailure — and six routes render EXECUTED from it, every one of which had
no campaign of any kind before 2026-09-01.

```sh
tools/gauntlet/resolution-sponsored/run-resolution-sponsored.sh
```

## What this is NOT

1. **Not validator evidence.** Nothing here deploys through Loader-v3 and
   ProgramTest has no finalized commitment. `slot` orders a campaign and proves
   nothing about finality.
2. **Not provider evidence.** The Pyth receiver and sponsored-push programs are
   synthetic bootstrap artifacts and every price body is a fixture written by
   the test. The honest sentence about the strongest row is *"the bank accepted
   a sponsored capture,"* never *"the market observed a price."*
3. **Not full coverage of the campaign.** Two fixture warm-up transactions are
   deliberately unrecorded: they drive no protocol route, and a campaign that
   labelled every transaction it happens to send would be claiming coverage no
   binding was written for.

## The fast-lane bar, answered one at a time

- **Loader-v3 / ProgramData / `SetAuthority`.** Not depended on. The tier
  installs ProgramData bodies for Core, Resolution and the two provider programs
  and never exercises an authority transition.
- **Packet serialisation.** Depended on, so **measured** rather than asserted.
  Every recorded transaction carries `wire_bytes` the campaign serialised
  itself, and three witnesses read them back. **Eight of the sixteen do not
  fit** — see below.
- **Compute and heap.** The campaign sets ProgramTest's compute maximum to
  exactly Solana's 1,400,000 and never raises it;
  `sponsored-fits-the-compute-maximum` checks the largest observed consumption
  (310,957 CU, `CommitFailure/head-vacant`) against that limit. No budget here
  is a gate; the numbers are measurements.
- **Real Agave account shapes.** Core state, the activation cache, the Source
  graph records and the capability funding ledger are the real encoders' output;
  the price updates are real `PriceUpdateV2` bodies at the real
  `FULL_PRICE_UPDATE_V2_LEN` under the receiver's own discriminator.
- **Frame diagnostics.** `cargo build-sbf` exits ZERO when the SBF backend
  reports that a call overwrites its own stack frame. The runner counts them per
  artifact and refuses to run the campaign at all if the count is nonzero.

## !! CAPTURE AND SETTLE DO NOT FIT A LEGACY PACKET !!

Measured 2026-09-01, the first time this family's wire extents were measured at
all. Solana's legacy maximum is 1,232 bytes (`PACKET_DATA_BYTES`).

| action | bytes | over |
|---|---:|---:|
| `Capture` (×4) | 1,255 | **+23** |
| `Settle` (×4) | 1,321 | **+89** |
| `CommitFailure` (×4) | 1,222 | fits, by **10** |
| `CloseCandidate` / `CloseHead` (×4) | 333 | fits |

Two consequences, and they are not the same.

**Capture and Settle need a v0 message over an Address Lookup Table.** A real
sponsor cannot submit either on a legacy message. This is the Found31 defect
class exactly: ProgramTest submits no packet, so the frame survives every
fixture test in the tree until someone serialises it and looks.

**`CommitFailure` fits with ten bytes of headroom** — and no room at all for a
priority-fee instruction. `sponsored-commit-failure-fits-by-ten-bytes` records
that margin so a later account added to the failure frame is a red witness
rather than a silent crossing.

## Nine hostiles, five distinct codes

The bindings name a refusal code per label, read off the fold. They are not the
same code, which is the whole point:

| code | refusal | hostiles |
|---|---|---:|
| `0x8009` | `ResolutionError::ProviderRelease` | 1 |
| `0x800c` | `ResolutionError::Transition` | 3 |
| `0x800e` | `ResolutionError::Funding` | 1 |
| `0x8012` | `ResolutionError::ProviderFreshness` | 2 |
| `0x8015` | `ResolutionError::SponsoredPush` | 2 |

`sponsored-hostiles-raise-five-distinct-codes` asserts the whole set. It exists
because the census lane's own first draft failed exactly here: six hostiles
sharing one label had the first one's code written for all six, and five of them
raise something else.
