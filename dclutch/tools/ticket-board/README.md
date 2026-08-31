# `dclutch-ticket-board`

A relay that holds bearer-signed Direct intent tickets so a taker can find one.

**Devnet-grade infrastructure.** It is deliberately small, it keeps its state in
memory with a JSON snapshot beside it, and it has no rate limiting. Read
[Honest limits](#honest-limits) before running one anywhere that matters.

## Why a relay is allowed to exist at all

A Direct ticket is **bearer-signed, self-authenticating data**. Every field the
transaction depends on is covered by the maker's detached Ed25519 signature, and
the chain re-derives the signing message and verifies it natively at execution.

So, for any transport whatsoever:

> **A relay can withhold. A relay can never forge.** Its worst case is
> censorship and staleness — never a wrong trade, never a stolen one.

That is why this service can hold no keys, take no custody, and carry no
authority. Losing it entirely loses *availability* and nothing else: every offer
it held is an artifact its maker still has and can post again. It is rung **(b)**
of the transport ladder in `docs/design/FLOWFUL_IA_V1.md` §4.4, and it supplies
*candidates* — which is exactly the category O-016 permits a caller to supply.

## What it validates, and what it does not

It runs **one** check, and it is not this crate's:
`dclutch_direct_ticket::parse_portable_direct_ticket_v1` — the same reader
`dclutch ticket verify` runs and the same one the operator's producer runs before
it opens a socket. That covers the 4096-byte bound, duplicate JSON keys, unknown
fields, canonical base58 and canonical decimal text, enum and fee widths, a
codec roundtrip, and **the detached Ed25519 signature against the preimage the
reader rebuilds**.

This crate re-derives no signing message and parses no wire format of its own. A
second implementation of a signing preimage is a signature that verifies
nowhere, discovered at the refused trade.

**It does not read chain state.** It cannot tell whether the maker's Position
covers the offer, whether the `generation` is current, whether the
`feeBasisPoints` match the Market's immutable Direct config, or whether the
`outcome` is inside the Product width. Those are decided against finalized state
by the code that builds the transaction, and finally by the chain.

> An offer on this board is **well-formed and correctly signed**. It is not
> "valid", and this service never says it is. Only the chain verifies.

It also **cannot sign**: `dclutch-direct-ticket` is taken with
`default-features = false`, so the `author` feature is off and no signer crate is
linked into the binary. That is a property of the dependency graph, not a promise
in a comment.

## Run one

```
bash tools/ticket-board/run-local.sh
```

Loopback on `127.0.0.1:8787`, snapshot beside the crate. Flags pass through:

| Flag | Default | |
| --- | --- | --- |
| `--bind ADDR` | `127.0.0.1:8787` | Loopback by default: no auth, no rate limiting, so exposing it is an explicit act. |
| `--snapshot PATH` | `ticket-board-snapshot.json` | Written after every accepted post; every row re-validated on load. |
| `--market PUBKEY` | every Market | Serve one Market and refuse the rest with `MARKET_NOT_SERVED`. |

## The API

Every response on every path is JSON, and carries the standing honesty line.

### `POST /tickets[?slot=SLOT]`

Body is the ticket JSON, verbatim, at most 4096 bytes. `slot` is the poster's own
current slot; it is used for exactly one thing — refusing a ticket that has
*already* expired — so a poster who lies about it can only harm their own post.

```
201 {"accepted":true,"digest":"<sha256 hex>","duplicate":false}
```

A re-post of the same bytes is `duplicate: true` and still `201`: offers are
content-addressed, so the second copy *is* the first one.

### `GET /tickets?market=PUBKEY[&outcome=U32][&slot=SLOT]`

```
200 {
  "offers": [{ "digest": "...", "text": "<the ticket, verbatim>", "postedAtSlot": "1200" }],
  "slotBasis": "1200",
  "droppedExpired": 3,
  "notice": "Offers are collected by a relay, not by the chain. ..."
}
```

Newest first — **not** price order. Price ordering needs the route's price scale
and the reader's side, neither of which a transport has; a board that ranked
offers "best first" would be asserting something it cannot compute. The flow
sorts for presentation.

`text` is the maker's exact bytes. The client decodes them with its own decoder,
so the board's opinion of a ticket reaches no further than its own admission.

### `GET /health`

```
200 {"status":"ok","offers":12,"capacity":4096,"servedMarket":null,"observedSlot":null,"notice":"..."}
```

`observedSlot` is always `null`, and it is present rather than absent so that
"this board has no clock" is an answer rather than an omission.

### Refusals, by name

Every refusal carries a stable machine name and a sentence, because a caller told
only that something "failed" cannot act.

| Name | Status | |
| --- | --- | --- |
| `TICKET_MALFORMED` | 400 | The shared reader refused it; its sentence is passed through verbatim. **A tampered field or signature lands here.** |
| `MARKET_NOT_SERVED` | 400 | This board serves one Market and the ticket names another. |
| `EXPIRED` | 400 | `validThrough` was already behind the slot the poster supplied. |
| `BODY_TOO_LARGE` | 413 | Above the 4096-byte bound a ticket has by its own codec. |
| `BOARD_FULL` | 503 | At capacity. See below — this is a refusal, never an eviction. |
| `QUERY_INVALID` | 400 | A parameter was missing or not canonical. |
| `ROUTE_NOT_FOUND` / `METHOD_NOT_ALLOWED` | 404 / 405 | There is no `DELETE`: a relay with a delete route is a relay that can be made to censor. |

## Two design decisions worth the paragraph

**The board has no clock, and refuses to invent one.** Expiry is judged against
the `slot` the *caller* supplies, and that judgment filters one response and
never touches stored state. The tempting alternative — remembering the highest
slot anyone has mentioned and sweeping against it — would let any caller expire
everyone else's offers by asserting a large number. That is precisely the
censorship a relay is otherwise structurally incapable of, and it is not worth a
garbage collector. Supply `slot` on every read; the trade flow already has one,
because step ⑤ needs the same finalized slot.

**A full board refuses rather than evicting.** Evicting the oldest offer to make
room for the newest lets a flood push every honest offer off the board — the same
censorship by another door. Refusing keeps the failure loud and existing offers
safe.

## Honest limits

Named as debt, because they are debt.

- **Durability is a snapshot, not a database.** The write is atomic against a
  crash mid-write (staged sibling file, `sync_all`, then rename), and a failed
  write leaves the last accepted snapshot byte-for-byte intact. But posts
  accepted between a write and a power loss are gone, there is no replication,
  and two boards pointed at one file will overwrite each other. Acceptable
  because the loss is bounded and harmless: offers are bearer artifacts their
  makers still hold.
- **No rate limiting.** None. The only backpressure is `MAXIMUM_OFFERS_V1`
  (4096) and the 4096-byte body bound, so anyone who can reach the port can fill
  the board with well-signed junk from throwaway keys and reach `BOARD_FULL`.
  This is the single biggest reason the default bind is loopback. Fixing it
  properly is per-maker accounting, not a token bucket on an IP.
- **No expiry sweep**, per the clock decision above. Storage is reclaimed by
  restarting, or by raising the cap. The right fix is an operator-controlled
  clock — the board polling `getSlot` on an RPC endpoint *it* was configured
  with — which is chain-derived rather than caller-asserted. Queued, not built.
- **No chain checks.** Position coverage, current generation, fee-rate match and
  outcome width are all unchecked here. A board full of correctly-signed offers
  that no longer clear is a real state, and the client filters them at render
  (§4.3) with the route it has and the board does not.
- **No cancellation.** A maker cannot withdraw an offer; it merely expires. A
  delete route would be a censorship lever handed to whoever can reach it, and
  authenticating one would mean this service holding a secret. Cancellation that
  is a chain fact rather than a service promise is rung **(c)**, on-chain resting
  orders (`OMISSION_INDEX.md` U-002).
- **No HTTPS and no CORS restriction.** It serves `access-control-allow-origin: *`
  because it holds only public bearer artifacts and no cookie or credential, so
  there is nothing for an origin check to protect. Terminate TLS in front of it.
- **One process, one lock.** State is a `Mutex`; throughput is fine for a devnet
  board and is not the design point.

## The client half

`packages/dclutch-sdk/lib/ticketBoard.ts` — `listBoardOffersV1`,
`postBoardOfferV1`, `boardHealthV1`, taking the board URL as an argument.
`apps/dclutch-web/lib/ticketBoard.ts` is the deployment half, reading
`NEXT_PUBLIC_DCLUTCH_TICKET_BOARD`; **an unset variable is the supported default**
and hides every board affordance, because the paste box needs no relay and
carries the flow by itself.

The client **re-decodes every listed offer locally** through
`decodeDirectIntentTicketV1` — the same call the paste box makes, on the same
bytes. An offer whose text that decoder refuses is reported as refused and never
rendered.

One asymmetry to know about: the Rust reader verifies the Ed25519 signature and
the TypeScript decoder does **not** (it checks shape and canonical form only). So
the board rejects a forged ticket at admission, but the browser cannot confirm
that and must not claim it did. The signature chip says *well-formed*. This is
consistent, not a gap being papered over: the board's word is not evidence, and
the only verifier that counts is the chain.

## Publishing an offer

```
dclutch ticket post --board http://127.0.0.1:8787 /abs/path/offer.json
dclutch ticket post --board http://127.0.0.1:8787 --keypair-env VAR --market ... --out /abs/path/offer.json
```

The second form authors and publishes in one breath. `DCLUTCH_TICKET_BOARD_URL`
sets the board instead of `--board`; there is no default, because a board is one
deployment's relay and guessing one would publish an offer somewhere the maker
did not choose. **Posting is not submitting**: it builds no transaction and
reaches no cluster.

## Tests

```
cd tools/ticket-board && cargo test
```

Twenty, and the two that carry the file are
`a_tampered_signed_field_dies_on_the_lifted_signature_check` and
`a_tampered_signature_is_refused_by_name`. They mutate a signed field and a
signature byte after authoring and require the refusal to **name the signature**.
A board that had merely re-checked JSON shape passes every other test here and
fails those two — which was confirmed by removing the tamper and watching the
board accept the ticket.
