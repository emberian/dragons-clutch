# A Market founded from the browser (2026-08-27, FE-CREATE)

**Claim.** A real Chromium, driving the `/create` wizard through a Wallet Standard
wallet, originated three finalized transactions against a local validator and left
a **Core Market at phase Founding** behind. Not a rendered claim about chain state —
chain state.

**Not claimed.** This is not a Market at Open. `DCLTGMF1` — the atomic
Lock→Found→Realize→Claims→Open route — is not driven here, and
`apps/dclutch-web/lib/founding/ladder.ts` is the inventory of exactly which rungs
a browser can and cannot carry today, with the reason on each.

## What landed

| Stage | Signature | Result |
|---|---|---|
| Lifecycle RentCredit Create | `obB56zMsXU7CKqYMxBDnoaEmQPDJEUSjocZhxeedXKmntdUcZYaYeFn1RgrHvmRDfTQuSWqpx8HnEgGJPtBUjBV` | finalized slot 15,339 |
| Routing table (create + 2 extend pages) | `5FEDaBqYw8k2NdG9GEfPar85tEYPPXJ5kinacPf1N5ybDa6of1HxQ57EodbMdCaFioJbFKnHDRkzYsXq7Y3YjeQZ` | `9JQu29kV7fjJqdnNZQykeYVjNQVh823XmpzQ6tqfyCSH`, 30 addresses, contents re-read and compared |
| Found31 | `3VKMGVaABK1wCJWS5iwiLfT4g4oGygi4RBSje5XiCFWjH7Y7EmS5cSY5HitiXxCrjNd49qFWgNVjRML11Gv3jA1o` | finalized slot 15,485 |

Market `8evRoQwEksaxtBHTDideCwbQmHpdyBZsNJEzUcGp2Zw4`, owner
`2rJGzuF2AduNJCc2td1y87ApUk8NhiCUGhsKCNRqhd8o` (Core), 352 bytes, magic `DCLTCOR2`,
schema 2, phase byte 0 = `Founding`, data SHA-256
`1802f77fee89bba41b8c499a9549bc2cdf2ed3b4b717d80b706207fce0b2e039`.

The readback is independent of the page: raw JSON-RPC `getAccountInfo`, decoded
against offsets cited to `crates/dclutch-market-core-codec/src/generated.rs`.
Nothing in it came from the browser.

## Chain profile

The journey campaign's own ledger, resumed —
`/private/tmp/dclutch-journey/runs/20260827T102346Z-8aa62277a756-h16/ledger` on
`127.0.0.1:22890`, via `tools/gauntlet/frontend/resume-validator.sh`. The record
graph, collateral Mint and infrastructure were already published there by the Rust
campaign; the browser reauthenticated all of it and supplied only the two Core
rungs and their routing table, at generation 11 (1 through 3 are the campaign's own).

Reproduce with `tools/gauntlet/frontend/drive-founding.mjs`. Playwright is
deliberately not a repository dependency; pass `--playwright`.

**Reproduced.** A second run at generation 13 founded
`HZv6xojgR5m6gNRCEXD2gkg7eDMWwmFBQNV73x7LMFKB` — same three rungs, same
`DCLTCOR2` / 352 bytes / phase `Founding`, a different routing table
(`4rTLQFP6BiR5GqawzRVZhJH6htu6PsgVu39Hz71uepGn`). One founding is a run; two at
different generations is a path.

## Three defects this found, none of which any test could have

Every one of them is the same shape: **a builder whose output was never submitted
is not tested, it is formatted.** `/found` emitted unsigned base64 and stopped, so
three separate errors sat in a shipped path and every assertion about them stayed
green.

1. **The lifecycle RentCredit stated the wrong action.**
   `lifecycleRentCreateRequest` wrote `0` at offset 10.
   `INSTRUCTION_ACTION_OFFSET` is 10 and `LifecycleRentActionV2::Create` is **1**
   (`crates/dclutch-rent-contract/src/lifecycle_v2.rs:51,352`), and the Rust
   encoder writes it through `instruction_header(...Create)` at `:400`. So every
   RentCredit the browser had ever built was refused at
   `LifecycleRentInstructionV2::decode`. Measured: 1,041 CU, `custom program
   error: 0x0` (that ELF predates ADR 0007's refusal band, so `Instruction` is
   still 0 there). Fixed by emitting the discriminant from the Rust enum in
   `scripts/generate-core-found.mjs` — the defect was the second hand-written
   copy of a wire constant, not the value.

2. **Found31 carried no ComputeBudget declaration.** It spends over a million CU
   authenticating whole program ELFs, against a 200,000 default, so the runtime
   killed it with `Program failed to complete` — which reads like a program bug
   and is not one. Fixed with `lib/founding/computeBudget.ts`, whose limit and
   discriminants are emitted from the reference client's own
   `bounded_instructions`.

3. **And then Found31 no longer fit a packet.** With that declaration the inline
   v0 message is **1,242 bytes against 1,232** — the same ten-byte overflow
   `4e1c4db` recorded when the Rust client moved the route onto a table. So the
   browser needed a lookup-table builder, which is `lib/founding/lookupTable.ts`.
   It reads the table's contents back off the chain before anything routes
   through it, because the vertical lane paid three validator runs for a client
   that compiled indexes against the list it built the plan from.

A fourth, found and fixed on the way: `prepareCoreFoundV2` required the RentCredit
destination to be **vacant**, which is the postcondition of the credit's own
transaction and the *inverse* of Found31's precondition. Holding both made a
two-stage flow impossible — the moment the credit landed, re-preparing Found31
refused. The observation now branches on what is actually on chain.

## What stays out of the browser, and why

`lib/founding/ladder.ts` carries the full inventory. The short version: the
collateral Mint, the thirteen-record semantic graph, the `DCLTPCB1` projected-Custody
prestate, the Lock/Realize/Claims request bodies, and the five-account prefunding.
The binding reason for most of them is not effort — it is that their bytes are
produced by first-party Rust encoders and Custody kernel transitions that are *the
authority for what those bytes mean*. A browser re-implementation would be a second
authority, not a client. The honest path is an emitter per encoder.
