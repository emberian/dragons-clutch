# Author a ticket, take a trade

A Direct trade settles two independently signed intents. You sign one, the
other side signs the other, and a third party — the route payer — builds the
transaction that spends both. This walkthrough covers the half you can do
alone: signing an offer, checking it, and publishing it where a taker can
find it. It ends where that half genuinely ends, at the handoff.

Everything below runs on your own machine. No chain is read, no key leaves
your disk, and nothing is submitted.

## Build the tool, and make a key

```sh
cd tools/dclutch-cli && cargo build --release   # Finished in 1m 46s
./target/release/dclutch --version
solana-keygen new --no-bip39-passphrase --silent -o ~/keys/maker.json
solana-keygen pubkey ~/keys/maker.json
```

```
dclutch 0.1.0-devnet.3
8LkuQ2uvpxzEFPLnhGwTYEP27zaBX4qtzFmWgCgFdTKo
```

The binary is not on your `PATH`; either add it or call it by path. A
prebuilt copy may already sit in `target/` from an older build — it will
happily run and report an older version, so read the version back rather
than trusting the file's presence.

## Sign the offer

Every field of the offer is an argument, and none of them has a default.

```sh
export DCLUTCH_MAKER_KEY=~/keys/maker.json

dclutch ticket author \
  --keypair-env DCLUTCH_MAKER_KEY \
  --maker 8LkuQ2uvpxzEFPLnhGwTYEP27zaBX4qtzFmWgCgFdTKo \
  --market 48kcHbUzgzg8e1AZCACpkWZcwrxoGhupxSFt5So1QK8g \
  --collateral-account B2sSzvcf9jPQ5Pv9bsw9pNt9UDGr18hqTgyoiXbDqaK5 \
  --side sell --lifecycle ioc --outcome 3 \
  --generation 7 --nonce 9 \
  --valid-from 11 --valid-through 4294967295 \
  --maximum-fill 100000000 --limit-price 500000 --fee-basis-points 50 \
  --out ~/tickets/seller-ticket.json
```

```json
{
  "schema": "dclutch-direct-intent-ticket-author-receipt-v1",
  "ticket": "~/tickets/seller-ticket.json",
  "ticketSha256": "aa073eba5ce735b83afbdc6252d9afa4e98044996b307aa34a27192b1f910bd1",
  "ticketBytes": 643,
  "maker": "8LkuQ2uvpxzEFPLnhGwTYEP27zaBX4qtzFmWgCgFdTKo",
  "market": "48kcHbUzgzg8e1AZCACpkWZcwrxoGhupxSFt5So1QK8g",

  … every field you passed, echoed back …

  "signedPreimageBytes": 172,
  "signatureDomain": "dclutch/signature/direct-compact-intent-v2"
}
```

Keep `ticketSha256`. The operator who settles this trade is asked for it by
name, and a ticket whose digest does not match the one you were given is
refused rather than read.

### If you pass the key as a path

**Remedy: name an environment variable that holds the path, not the path.**

```
dclutch: REFUSED: --keypair is refused: pass --keypair-env NAME so the path
never reaches the command line or the process table
```

A path on the command line is a path in the process table and in your shell
history. The tool refuses `--keypair`, `--keypair-path` and `--secret-key`
by name, at parse, before it opens anything.

## Read it back

Verification opens no key and touches no network.

```sh
dclutch ticket verify ~/tickets/seller-ticket.json
```

```
ticket           ~/tickets/seller-ticket.json
sha256           aa073eba5ce735b83afbdc6252d9afa4e98044996b307aa34a27192b1f910bd1
bytes            643
signature        VERIFIED against the maker below
maker            8LkuQ2uvpxzEFPLnhGwTYEP27zaBX4qtzFmWgCgFdTKo
market           48kcHbUzgzg8e1AZCACpkWZcwrxoGhupxSFt5So1QK8g
side             sell
lifecycle        ioc (immediate or cancel)
outcome          3
generation       7
nonce            9
valid slots      11..=4294967295
maximum fill     100000000 atoms
limit price      500000 scaled
fee              50 bps
collateral       B2sSzvcf9jPQ5Pv9bsw9pNt9UDGr18hqTgyoiXbDqaK5
```

Run this on any ticket somebody hands you, before you act on it.

## Start a board

A board is a relay: it collects offers so a taker can find them. Run your
own on loopback.

```sh
bash tools/ticket-board/run-local.sh
```

```
dclutch-ticket-board
  listening        http://127.0.0.1:8787
  snapshot         ~/work/ticket-board-snapshot.json
  market           every Market
  restored         0 offers (0 snapshot rows refused)

  This board holds no keys, takes no custody, and has no authority. It
  checks a ticket's shape and its signature; it reads no chain, so an offer
  listed here is WELL-FORMED, never verified. Only the chain verifies.
```

It binds to `127.0.0.1` and has no authentication, so do not move it off
loopback without putting something in front of it. Pin it to one market with
`--market PUBKEY`; every other market is then refused with
`MARKET_NOT_SERVED`.

## Publish

```sh
dclutch ticket post --board http://127.0.0.1:8787 ~/tickets/seller-ticket.json
```

```
ticket           ~/tickets/seller-ticket.json
sha256           aa073eba5ce735b83afbdc6252d9afa4e98044996b307aa34a27192b1f910bd1
board            http://127.0.0.1:8787
posted           accepted
digest           aa073eba5ce735b83afbdc6252d9afa4e98044996b307aa34a27192b1f910bd1
maker            8LkuQ2uvpxzEFPLnhGwTYEP27zaBX4qtzFmWgCgFdTKo
market           48kcHbUzgzg8e1AZCACpkWZcwrxoGhupxSFt5So1QK8g
outcome          3
valid slots      11..=4294967295
```

There is no default board. Give `--board`, or set
`DCLUTCH_TICKET_BOARD_URL` — guessing one would publish your offer somewhere
you did not choose.

Posting the same file twice is not an error; it answers `"duplicate":true`
and stores one copy.

### If the ticket is malformed

**Remedy: fix the file — the shape is named in the refusal.**

`post` re-checks the ticket locally before it opens the socket, so you hear
it from your own tool:

```
dclutch: this Direct ticket shape: unknown field `schema`, expected one of
`kind`, `maker`, `signature`, `intent`
```

A board reached directly answers with the same sentence under a code:

```json
{"accepted":false,"refusal":"TICKET_MALFORMED","reason":"posted Direct ticket shape: unknown field `schema`, expected one of `kind`, `maker`, `signature`, `intent`"}
```

### If the offer has already expired

**Remedy: author a new ticket with a later `--valid-through`.**

Tell the board the current slot and it will not accept an offer that the
chain would reject anyway:

```json
{"accepted":false,"refusal":"EXPIRED","reason":"the ticket's last valid slot is 100 and the posted current slot is 500; it is already expired and would be refused at execution"}
```

The board has no clock of its own. It judges expiry only against the slot
the caller supplies, and that judgment filters one response without
mutating stored state — so no caller can expire everyone else's offers.

## Find offers

```sh
curl -s "http://127.0.0.1:8787/tickets?market=48kcHbUzgzg8e1AZCACpkWZcwrxoGhupxSFt5So1QK8g&slot=500" | jq
```

```json
{
  "offers": [ { "digest": "aa073eba…", "text": "…", "postedAtSlot": null } ],
  "slotBasis": "500",
  "droppedExpired": 1,
  "notice": "Offers are collected by a relay, not by the chain. The chain checks every signature when the trade executes — a relay can hide an offer from you, but it cannot change one."
}
```

Newest first, not best price first. `droppedExpired` counts offers held back
from this response because the slot you passed is past their window.

Nothing here consulted a chain. The market address above is one this
walkthrough generated locally, and the board took the offer anyway, because
a board checks shape and signature and nothing else. That is the whole
security story: **a relay can hide an offer from you, but it cannot change
one.** The signature covers every field, and the chain re-derives the
signing message and checks it natively when someone crosses the ticket.

## The taking side

Crossing an offer needs the other half and a transaction, and `dclutch`
sends none.

**In the browser.** The panel on a market page (`/markets/<address>`) does
the taking side with a wallet and no key file: connect, pick an outcome and
a size, paste the maker's ticket into the box marked *the other half*,
preview the exact debit, credit and fee, then sign. The board is not wired
into that box yet — it is reached today by pasting.

Signing is where you stop, unless you are also the route payer. The
authenticated route names one payer, and when that is not your wallet the
panel says so and hands you back your own signed taker ticket:

> Your intent is signed. Nothing has executed.

That is the seam. Two signed tickets now exist, and neither side can spend
them.

**As the operator.** The third party settles both with the operator binary
`dclutch-local-successor-bootstrap`. Its producer,
`devnet-direct-trade-produce-v1`, takes both tickets *and both digests* —
`--seller-ticket` with `--expected-seller-ticket-sha256`, and the buyer pair
beside it — then re-checks every signed field against finalized chain state
and refuses on any mismatch. This is where the digest from `ticket author`
is spent. Producing opens no key and submits nothing; sending is a separate
command, `devnet-direct-trade-v1`, whose default is still preflight. Without
`--execute` its transport is opened read-only, so "opens no key and sends
nothing" is a property of the transport rather than a promise the driver
makes about itself; with it, each invocation advances exactly one durable
mutation and never blind-resubmits.

Those two commands are not run here — settling needs the other half, a
funded chain, and a payer, none of which this walkthrough has. Their
loopback equivalents are `local-private-validator-direct-trade-produce-v1`
and `local-private-validator-direct-trade-v1`, which take no
`--i-mean-devnet`, because a loopback origin is admitted without ceremony.

## Stop the board

Stop the process. There is no `DELETE` route — a relay with a delete route
is a relay that can be made to censor. The offers are in the snapshot file
and come back when you start it again.
