# The CLI drives the loop: `dclutch` against a live local validator

**Date** 2026-08-27 · **Lane** SDKCLI · **Tree** at this doc's commit
· **Binary** `packages/dclutch-cli` bundled at the same tree
· **Chain** a fresh successor validator on `127.0.0.1:19790`, founded by the
run-spec producer from a spec assembled off the relayed-vertical ELF set
(seven immutable slot-zero roles), seeded keys (`--keypair-seed`, loopback
only), ~90 campaign transactions to two markets.
Artifacts: `/private/tmp/dclutch-sdkcli/run2/` (spec, evidence, ledger,
founding transcript) and `docs/evidence/cli-trader-loop-2026-08-27.transcript.txt`
(the session below, unabridged).

## What ran, in order

Every command is the shipped `dclutch` binary talking to the running
validator through `@dclutch/sdk`. Nothing below is a fixture or a mock.

### 1. `dclutch found` — founding from the terminal

The CLI wrapped `dclutch-local-successor-bootstrap run` end to end: derived
the producer's RPC-origin pin from the spec's own `rpc_url`, streamed the
campaign (infrastructure activation, record graph, DCLTPCB1 staging at
754,119 CU, the DCLTGMF1 atomic five-stage founding at 1,193,247 CU), and
wrote a session file the other commands read:

```
markets   38s7BVhduYrgNJnWUizQFQMQ7khqDdtNyJifG3G6YrPM, 8DmxJvwsQVYz6ZURfNoxssKK2YrQMwFPnsYM6t2xrzG9
session written to /private/tmp/dclutch-sdkcli/run2/session.json
```

### 2. `dclutch markets ls` / `show` — live enumeration and full decode

```
markets under Core 2rJG…hd8o at finalized slot 4207 (getProgramAccounts
returned 3 finalized Core accounts at slot 4207; 2 carry the DCLTCOR2 header.)
  38s7…YrPM  gen 2  Open      CHAIN · finalized slot 4207
  8Dmx…rzG9  gen 1  Founding  CHAIN · finalized slot 4207
```

`show` decoded the Open market completely: collateral bound (Token-2022
mint `EJtm…KY6c`, mint and freeze authority required absent), liability
bound (4 claims, supply 500,000,000 atoms each, backing basis
maximum-claim-supply), Hoard derived with exactly the founding principal
(500,000,000 atoms), and all three PDA bindings verified.

### 3. `dclutch spine` — the trade wall, named before anything is signed

```
spine refused: this Market's authenticated capability manifest lists 3
entries and none is the Direct successor kind — Direct trading was never
part of this Market's founding, which is the Market's own choice, not an
outage
```

This is the measured answer to "why no `buy` in this transcript": the
demo-market recipe founds without the Direct capability entry, so the chain
itself would refuse a trade, and the spine says so from one read. The
`buy`/`sell` construction path (intent signing, crossing, compile, submit)
is the same code the web trade workspace ships and is pinned by the SDK
suite; executing it live needs a market founded with the Direct capability,
which is the frontend journey lane's running campaign, not this one.

### 4. `dclutch portfolio` — the founder's position through derived keys

The run used the seeded-key affordance, so the founder's key is a pure
function of the seed. Deriving `founding-founder[0]` client-side
(`SHA-256(domain‖0‖seed‖0‖role‖0‖index)`) reproduced the campaign's key
byte-for-byte — the derived `collateral-wallet[0]` matches the evidence
document's account exactly — and the portfolio read straight off the chain:

```
portfolio of EV744eic… at finalized slot 4216 across 2 market(s)
  market 38s7…YrPM
  position  FpgjPYktL8cQ3W…  balances 500000000 ×4  claim mergeable
  market 8Dmx…rzG9
  position  absent — No Claims Position exists at kefbc1hd…, the address
  this Market and owner derive under the selected Claims program. That is
  the chain state at this finalized floor, not a lookup failure.
```

The founder holds a complete set in the Open market (one atom of every one
of four claims, 500,000,000 each) and no position in the still-Founding
market — both read directly, no indexer.

### 5. `dclutch redeem --dry-run` — the payout gap, stated not spun

```
claim     mergeable — … the count that can be merged is the smallest owned
          claim balance. This is arithmetic on these balances, not an offer.
the position is not redeemable at this floor; nothing was signed
```

A held-but-not-redeemable position stops the command with a reason, before
any transaction. A redeemable one would first create the Claims-role
Custody replay (the one wallet-constructible step) and then state the
payout gap in the SDK's own words — the payout route admits caller role
Core or Trading only (ADR-0008 §7.6). No flag pretends otherwise.

### 6. `dclutch refusal` — any code, named from the band registry

```
$ dclutch refusal 0x5002
  claims refused: ClaimsSbfError::Identity (0x5002) — Market or Position
  semantic identities did not join the packet.
```

### 7. `dclutch walk` — a live refusal, by name, on a deliberately wrong frame

This demo market's resolution deadline has not passed, and its founding
writes no Source resolution state or certificate, so a funded failure walk
cannot succeed here — which makes it the exact case for proving the
fail-closed path end to end. A walk book was assembled from the evidence
document with the missing slots filled by the nearest real records (a
knowingly invalid frame), and the walk was **submitted to the running
validator**, not simulated in a fixture:

```
walking market 38s7…YrPM to its explicit failure outcome (703 byte legacy packet)
refused: sendTransaction refused: Transaction simulation failed: Error
processing Instruction 0: custom program error: 0x8000
  resolution refused: ResolutionError::AccountFrame (0x8000) — Account
  count, order, privilege, executable state, or aliasing was invalid.
```

The chain refused the frame; the CLI turned the bare `0x8000` into the
program that raised it and the reason, from the same band registry the SDK
generates from the Rust authority. The 22-slot frame the `--dry-run` above
printed is built entirely from `@dclutch/sdk`'s generated relay-transport
module. This is the whole thesis of the lane in one line of output: a
refusal is the protocol working, and the terminal reads it as such.

## What this proves

- `@dclutch/sdk` is a real client surface: every one of these reads and the
  one write went through it, against a live chain, with no fixture in the
  path.
- Refusals render by name on every error path, including a **live** custom
  code the chain produced (`0x8000`), not only the offline `refusal`
  lookup.
- The honest-gap discipline holds under execution: `spine` and `redeem`
  each stopped with a measured reason instead of a spun success, and the
  seeded-key derivation reproduced a campaign key byte-for-byte, so a
  terminal user can hold the founder's own position.

What is *not* here, and why: a live `buy`. The demo-market recipe founds
without the Direct capability entry, so the chain would refuse a trade and
`spine` says so from one read (§3). The construction path is the same code
the web trade workspace ships, pinned by the SDK suite; exercising it live
needs a Direct-capable market, which is a different campaign than this one.

Raw session: `docs/evidence/cli-trader-loop-2026-08-27.transcript.txt`,
committed alongside this doc.
