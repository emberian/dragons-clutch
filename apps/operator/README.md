# Operator Bench

The first frontend, in two modes.

**Watch** is a window onto the general-clearing committed walk — the same
forty-four signed transactions `run_general_committed.sh` drives, watched
instead of tailed.

**Trade** founds the Friday clutch — eight hats on a $100–$240 knot grid,
`basis_degree` 1 — on a fresh local ledger and hands it to you. You endow, you
split, you rest orders, you paint a belief, you freeze, and the epoch clears
against a fixed-belief automaton.

```sh
# watch mode: the sealed lane's plan, step by step
cargo run --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- serve
# trade mode: found the Friday clutch and trade it
cargo run --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- \
  serve --mode trade
# then open 127.0.0.1:9130 in a browser
```

## The rule that makes it honest

**The browser never builds a transaction.** This directory contains no RPC
client, no wallet, no transaction serializer, and no key material. The page
subscribes to one server-sent event stream from the local daemon and renders
it. What it can POST is an *intent* — a knot, a side, a quantity, a limit, or a
pacing verb — never a transaction; the daemon decides which accounts in which
roles that needs and hands the result to
`clutch_sbf_harness::general_transaction`.

Every byte the bank sees is built by `clutch_sbf_harness`, the same library the
sealed lane's plan generator is.
`programs/clutch-sbf/scripts/run_operator_replay.sh` rebuilds every plan
transaction through those builders, byte-diffs all 294 emitted files against
the harness CLI's own output, and requires a single corrupted byte to go red.

## What is on screen

### Watch mode

| screen | what it reads |
| --- | --- |
| **Bench** | validator health, the ELF identity block (path, bytes, sha256 hashed here from the loaded file), the signing roster, the genesis-assistance disclosure, and the forty-four-step lifecycle rail |
| **Walk** | the step log in the committed-harness grammar, per-step compute-unit bars against the 1 400 000-unit ceiling, refusals rendered as first-class results with their expected `Custom(0x….)` codes, a live slot countdown for the two real-clock waits, and the conservation strip |
| **Funding** | pooled custody, locked backing, and per-owner cash / reserved / free / eggs |
| **Ticket** | one card per reservation, joined to its live book slot by order id |
| **Book** | the epoch and its window, the order page slot by slot, and the candidate record |

### Trade mode

| screen | what it reads, and what it can do |
| --- | --- |
| **Clutch** | the founded market's identity — terms digest, basis degree, the eight knots, the frozen limit ladder — the two actors and their positions, the epoch phase, and the Freeze control |
| **Ticket** | three tabs. *Single hat*: the hat row, a side, a size and a ladder limit. *Belief*: eight sliders, quantized by the daemon, previewing the orders that belief implies against the automaton's resting quotes, with one button to place them all. *Portfolio*: a coefficient vector, lots, and a per-lot collateral bound. Below them, your resting orders with their reservations and a retire button |
| **Book** | the automaton's own disclosure — what it believes, what it can quote, and the two rules it quotes by — the two beliefs and the cleared vector drawn over the eight hats, and the order page slot by slot |
| **Settlement** | positions across all eight outcomes, the value plane's identities re-derived from observed bytes, and every reservation's entitled/consumed counters |
| **Steps** | one row per transaction the session actually submitted, with its family, its confirmed slot, its compute units against the 1 400 000-unit ceiling, and its signature. Not a rail with pending rows: a trade session has no plan, so a row exists only because something was built, signed and confirmed. A refusal is a first-class row carrying the bank's own `Custom(0x….)` |
| **Bench** | validator health, the ELF identity block, the roster, and the genesis-assistance disclosure — the same cards as watch mode, minus the lifecycle rail, which a session with no plan simply does not have |

Every cell is the latest account image the daemon reloaded from the bank,
decoded through the frozen `clutch_solana_layout` codecs. A role that has not
been observed yet says `NOT YET OBSERVED` rather than showing a zero, and a
number nobody has submitted carries a `MODEL-ONLY` chip.

## The opponent

A **fixed-belief automaton**. Not a model, not a strategy, not an AI, and the
bench says so in those words wherever it appears. It holds one integer vector
that never changes — the disagreement exhibit's Model E,
`[0, 127, 2662, 5945, 1266, 0, 0, 0]` — and quotes by two published rules:

- **Opening.** One quote per knot where its belief differs from the flat prior:
  buy at its own value where it is higher, sell at its own value where it is
  lower. This is the exhibit's book-former with the flat prior standing in for
  the counterparty belief that does not exist yet at session open.
- **Response.** An order that crosses its value is answered on the other side,
  at that same value, when it is not already resting there.

Both rules, the belief, and the belief put on the limit ladder are on the Book
screen. Every order it will ever place can be worked out before it places one.

## The cleared price

The auto-crank is **not a solver** and claims no optimality. It tries four
*stated* coordinates in a fixed published order and reports which the relation
admitted and exactly how it refused the ones before: the midpoint of the two
published beliefs, the automaton's belief, your painted belief, and the flat
prior. Each attempt is published to the stream with its refusal.

The midpoint is first because it has the property the frozen allocation policy
needs: at a knot where the two beliefs disagree it sits strictly between the
two limits, and at a knot only one side quoted it sits on the far side of that
quote's limit — which makes an unanswered quote *ineligible* rather than a
strict order nobody can fill. `PricePriorityMarginalProRata` fills every strict
order in full, so an eligible order with no counterparty refuses the whole
candidate. `session.rs`'s tests pin both halves of that.

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
kind. The gate lives in `scripts/run_operator_trade.sh` and greps every file
the daemon is allowed to serve — `.html`, `.css`, `.js`, `.svg`, `.json` — for
an off-machine address. This README is prose about that gate and is
deliberately not one of the served extensions.

Even the SVG namespace is read off a node in the document (`namespaceURI`)
rather than written as a URL literal, so the grep needs no exception.

## Files

| file | role |
| --- | --- |
| `index.html` | the shell, the non-dismissible honesty strip, and the empty node the SVG namespace is read from |
| `styles.css` | `site/style.css`'s token block, plus this bench's layout |
| `app.js` | the router, the banner, and the mode switch |
| `stream.js` | the event-stream client and the store |
| `dom.js` | six DOM helpers, which is the whole rendering library |
| `evidence.js` | the frozen claim vocabulary, copied verbatim |
| `action.js` | the one write: a JSON intent |
| `bench.js`, `walk.js`, `market.js` | the watch-mode screens; `bench.js` is shared |
| `trade.js` | the trade-mode screens: Clutch, Ticket, Book, Settlement, Steps |

## Not built

Multi-page epochs — a trade session opens one order page, so the book holds
`MAX_ORDERS_PER_PAGE` orders and the ticket says so rather than letting the
bank refuse the seventeenth. Resolution and redemption: the Friday clutch never
resolves, so the Ticket's weight preview is a MODEL-ONLY reading of what a
terminal statistic would carry, not a payout the bank has made.
