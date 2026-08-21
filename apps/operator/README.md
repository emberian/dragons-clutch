# Operator Bench

The first frontend. A window onto the general-clearing committed walk —
the same forty-four signed transactions `run_general_committed.sh` drives,
watched instead of tailed.

```sh
cargo run --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- serve
# then open 127.0.0.1:9130 in a browser
```

## The rule that makes it honest

**The browser never builds a transaction.** This directory contains no RPC
client, no wallet, no transaction serializer, and no key material. The page
subscribes to one server-sent event stream from the local daemon and renders
it. The single write it can perform is a *pacing* request — pause, step once,
resume — and the daemon has no verb behind that endpoint which composes,
reorders, or skips a transaction.

Every byte the bank sees is built by `clutch_sbf_harness`, the same library
the sealed lane's plan generator is.
`programs/clutch-sbf/scripts/run_operator_replay.sh` rebuilds every plan
transaction through those builders, byte-diffs all 294 emitted files against
the harness CLI's own output, and requires a single corrupted byte to go red.

## What is on screen

| screen | what it reads |
| --- | --- |
| **Bench** | validator health, the ELF identity block (path, bytes, sha256 hashed here from the loaded file), the signing roster, the genesis-assistance disclosure, and the forty-four-step lifecycle rail |
| **Walk** | the step log in the committed-harness grammar, per-step compute-unit bars against the 1 400 000-unit ceiling, refusals rendered as first-class results with their expected `Custom(0x….)` codes, a live slot countdown for the two real-clock waits, and the conservation strip |
| **Funding** | pooled custody, locked backing, and per-owner cash / reserved / free / eggs |
| **Ticket** | one card per reservation, joined to its live book slot by order id |
| **Book** | the epoch and its window, the order page slot by slot, and the candidate record |

Every cell is the latest account image the daemon reloaded from the bank,
decoded through the frozen `clutch_solana_layout` codecs. A role that has not
been observed yet says `NOT YET OBSERVED` rather than showing a zero.

## Evidence scope

Signed, confirmed, committed sequential execution on a local
`solana-test-validator` from a genesis-assisted prestate, against an ELF built
with `--features non-production-mock-source`. **Unpromoted.** Not a
deployment, not devnet evidence, not mainnet evidence, not a wallet, not an
operatorless venue, not a blank-bank lifecycle.

The header strip carrying that scope is markup in `index.html`, present before
any script runs, and no code path on this page removes it. The claim
vocabulary in `evidence.js` is a verbatim copy of the frozen `EVIDENCE` map in
`../static-client/app.js`; a new claim is added there first and copied here
second. The word "verified" does not appear in this bench's prose, because
nothing it shows is.

## No dependencies

Hand-authored ES modules, loaded directly by the browser. No build step, no
bundler, no transpile, no package manifest, and no external reference of any
kind — the gate is `grep`:

```sh
grep -rniE 'https?:|cdn|unpkg|jsdelivr|googleapis' apps/operator   # must find nothing
```

## Files

| file | role |
| --- | --- |
| `index.html` | the shell and the non-dismissible honesty strip |
| `styles.css` | `site/style.css`'s token block, plus this bench's layout |
| `app.js` | the router and the banner |
| `stream.js` | the event-stream client and the store |
| `dom.js` | six DOM helpers, which is the whole rendering library |
| `evidence.js` | the frozen claim vocabulary, copied verbatim |
| `action.js` | the one write: a pacing request |
| `bench.js`, `walk.js`, `market.js` | the five screens |

## Not built

Interactive Endow / Split / PlaceOrder builders, the Friday-clutch
eight-outcome degree-1 fixture, and the density painter. The screens above are
read-only views of the committed walk; nothing here pretends to be a control
it is not.
