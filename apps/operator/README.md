# Operator Bench

The first frontend, in three explicit modes.

Run its dependency-free source/mechanical checks with:

```sh
cd apps/operator
npm test
npm run check
```

These checks cover exact integer formatting and persistent accessibility/
interaction invariants. They are not browser execution or visual QA.

**Watch** is a window onto the general-clearing committed walk — the same
forty-four signed transactions `run_general_committed.sh` drives, watched
instead of tailed.

**Trade** founds the Friday clutch — eight hats on a $100–$240 knot grid,
`basis_degree` 1 — on a fresh local ledger and hands it to you. You endow, you
split, you rest orders, you paint a belief, you freeze, and the epoch clears
against a fixed-belief automaton. The market is created by a signed
`CreateMarket`, the cash arrives by a signed `Endow`, the Eggs by a signed
`Split`: nothing about the trading plane is injected bank state.

**Pyth local** is a read-only presentation of a retained, public-safe
local-real Pyth campaign transcript. It does not start a validator, contact
RPC or a provider API, load a wallet or key file, or build a browser
transaction. Its permanent boundary is **NON-PRODUCTION / SYNTHETIC
OBSERVATION / LOCAL VALIDATOR ONLY / NO VALUE**. The screen also says
**READ-ONLY RETAINED TRANSCRIPT**: it cannot trade, extend, replay, refresh, or
re-read the chain behind the recorded campaign.

```sh
# watch mode: the sealed lane's plan, step by step
CARGO_NET_OFFLINE=true cargo run --offline \
  --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- serve
# trade mode: found the Friday clutch and trade it
CARGO_NET_OFFLINE=true cargo run --offline \
  --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- \
  serve --mode trade
# retained campaign mode: display three truth-labelled public transcript files
CARGO_NET_OFFLINE=true cargo run --offline \
  --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- \
  serve --mode pyth-local \
  --transcript docs/reviews/evidence/local-real-pyth-signed-rpc-2026-08-22
# retained historical joined-v2 lifecycle: exact 21-step blocker history
CARGO_NET_OFFLINE=true cargo run --offline \
  --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- \
  serve --mode pyth-local \
  --transcript docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-22
# current joined-v4 settled trading lifecycle
CARGO_NET_OFFLINE=true cargo run --offline \
  --manifest-path programs/clutch-sbf/operatord/Cargo.toml -- \
  serve --mode pyth-local \
  --transcript docs/reviews/evidence/local-real-pyth-joined-lifecycle-2026-08-23
# then open 127.0.0.1:9130 in a browser
```

To retain a new source-only campaign, choose a new empty directory with
`CLUTCH_LOCAL_REAL_PYTH_TRANSCRIPT_DIR` when running
`programs/clutch-sbf/scripts/run_local_real_pyth.sh`. For the joined signed-user
lifecycle, use `programs/clutch-sbf/scripts/run_local_joined_pyth_lifecycle.sh`
with the same variable. Then pass that directory to `--transcript`. The display
reads only `campaign.json`, `result.json`, and `probe-evidence.json`; it does not
replay or extend the campaign.

Watch/trade runtime prerequisites are Rust/Cargo, `cargo-build-sbf`,
`solana-keygen`, and `solana-test-validator`; the scripted gate also uses
`curl` and Python. Retained campaign mode starts no validator. Node
and npm are required only for the source/mechanical browser checks. Override
the Trade wrapper's listeners with `CLUTCH_OPERATOR_TRADE_PORT`,
`CLUTCH_OPERATOR_TRADE_RPC_PORT`, `CLUTCH_OPERATOR_TRADE_FAUCET_PORT`,
`CLUTCH_OPERATOR_TRADE_GOSSIP_PORT`, and
`CLUTCH_OPERATOR_TRADE_DYNAMIC_PORT_RANGE`. Watch mode uses the corresponding
`CLUTCH_OPERATOR_*` names. Both wrappers pass explicit, disjoint gossip and
dynamic port ranges so the validator never falls back to Solana's broad
implicit local range.
The wrapper stops its daemon/validator and the daemon removes orderly-run
ephemeral signer files; a force-killed standalone daemon may leave its printed
temporary work directory for manual inspection and cleanup.

Trade mode follows the program's real local clock. The default session waits
for its short 260-slot freeze window and then the protocol's fixed 1,000-slot
candidate window, so freeze-to-settled normally takes several minutes. The
Epoch card shows the current slot, target, reason, and remaining slots; the
daemon log now names the selection wait separately from the completed relation
walk.

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

The daemon binds only to IPv4 loopback. It also rejects DNS-rebound `Host`
values, requires same-origin JSON from browsers, and gives each process a fresh
HttpOnly, SameSite capability cookie before either `/api` or `/api/events` is
usable. The scripted local gate acquires that session cookie from the index in
the same way a browser does.

## Exact integer transport

Watch and Trade identity events advertise `canonical-decimal-v1`. Every
onchain `u64` quantity, slot, cursor, generation, scaled price, and balance —
and every `u128` market knot exposed by the Operator — crosses the
daemon/browser boundary as a canonical decimal string. The browser uses
`BigInt` for exact formatting, subtraction, comparison, and ratios; only the
final bounded 0–100 display percentage becomes a JavaScript `Number`.

Trade intents use the same decimal strings. The daemon checks the complete
field or vector and refuses signs, whitespace, exponents, fractions, leading
zeroes, overflow, and malformed vector members without defaulting them to zero
or deleting an entry. Legacy JSON-number intents are accepted only through
`Number.MAX_SAFE_INTEGER`; a numeric literal at or above 2^53 is refused and
must be resent as a string. The browser itself refuses to send unsafe numbers
and stops on an SSE event containing one.

This protects the presentation from silent IEEE-754 rounding. It does not make
the static client authoritative: every screen remains an untrusted projection
of daemon-validated or daemon-decoded state, and the onchain accounts remain
the semantic owners of persisted facts.

## What is on screen

### Pyth local mode

| screen | what it reads |
| --- | --- |
| **Campaign** | exact captured receiver/router Program and ProgramData identities; campaign, validator, ELF, source-profile, VAA, and update hashes; loopback listener evidence; synthetic source value and conservative interval; both atomic rollback negatives; seal and categorical resolution; all signed transactions in retained order with signature, signed-wire hash, slot, compute units, fee, top-level program order, and exact error. Historical source-v1 remains readable. Current source-v2 requires one registered SourceSpec/Archive plane plus a distinct router-verified wrong-feed VAA account; it does not create a second wrong-feed source plane. |
| **Historical signed user lifecycle (joined-v2)** | preserves the exact retained 21-step history: explicit non-genesis market creation; one ephemeral user and collateral identity; exact `CreateMarket`, `Endow`, `Split`, four `RedeemInternal`, and `WithdrawCash` signatures; final zero position/supply/Hoard obligations and the exact 64 atoms returned; trading remains **BLOCKED / NOT SUBSTITUTED** at `missing-sealed-price-grid-and-epoch-plane` |
| **Current signed trade lifecycle (joined-v4)** | requires exactly 52 signed steps and two distinct owner/token identities; signed PriceGrid and allocation-policy uploads; non-genesis market, epoch, orders, and candidate; an exact funded buy/sell book; one-witness candidate verification, selection, entitlement, and settlement; one registered source plane, a separately router-verified wrong-feed VAA negative, resolution, five owner-bound redemptions, two withdrawals, and terminal zero liabilities with all 128 collateral atoms returned. The screen says **TRADE SETTLED / NOT SUBSTITUTED** and “best valid submitted candidate,” never “optimal clearing.” |

All integers cross the daemon/browser boundary as decimal strings. The daemon
refuses the presentation unless the three inputs carry the exact boundary,
provider role set, mode-specific thirteen-, twenty-one-, or fifty-two-step order,
instruction-2 `SourceAdmissionFailed` rollback errors, matching terminal
signatures, closed rollback checks, seal, and payout cell 1. Source-v2 and
joined-v4 additionally require matching producer schema tags, steps 7 and 8 as
the wrong-feed router VAA allocation/write and verification transactions, no
wrong-feed SourceSpec/Archive or genesis plane, and a distinct
`wrong_feed_verified_vaa_account`. They also retain two exact Clock readings:
the append-time Clock must put the observation inside its 60–300 second source
window, while the final authenticated Clock may be later after the protocol's
fixed 1,000-slot selection window. Final age is reported, not incorrectly
re-applied as an append-freshness condition. Joined-v2 also
requires exact four-outcome redemption arithmetic, terminal conservation, and
the named un-substituted trading blocker. Joined-v4 instead requires the exact
two-owner identities, funding, signed artifact sequences, orders, candidate
prices/fills, settlement state, five owner-bound redemptions, two withdrawals,
and terminal conservation emitted by the current campaign. Unknown or missing
joined-v4 lifecycle, trade, funding, order, settlement, terminal, and redemption
fields are refused. The static page remains an untrusted projection of that
retained evidence; it neither discovers nor attests onchain state independently.
The unretained joined-v3 transitional 52-step shape is explicitly refused; its
step count is never silently interpreted as joined-v4.

The wrong-feed VAA address is a checked producer-attested field. The retained
step rows bind each signed transaction by `signed_wire_sha256` but do not retain
the signed wire or account-meta list, so this reader cannot independently
re-derive that step 10 consumed that address. A stronger claim requires a later
transcript schema that retains the signed wire or an authenticated account-meta
projection.

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
| **Ticket** | four tabs. *Single hat*: the hat row, a side, a size and a ladder limit. *Belief*: eight sliders, quantized by the daemon, previewing the orders that belief implies against the automaton's resting quotes, with one button to place them all. *Portfolio*: a coefficient vector, lots, and a per-lot collateral bound. *Funding*: Endow more collateral into pooled custody, Split more cash into complete sets. Below them, your resting orders with their reservations and a retire button |
| **Book** | the automaton's fixture disclosure, the two daemon-held beliefs, the daemon's pre-submit candidate-trial vector drawn over the eight hats with a MODEL-ONLY boundary, and the role-decoded order page slot by slot |
| **Settlement** | positions and reservation counters from sequential role-decoded RPC data, plus a value-plane invariant that mixes those reads with daemon-held endowed and split totals |
| **Steps** | one row per transaction the session actually submitted, with its family, its confirmed slot, its compute units against the 1 400 000-unit ceiling, and its signature. Not a rail with pending rows: a trade session has no plan, so a row exists only because something was built, signed and confirmed. A refusal is a first-class row carrying the bank's own `Custom(0x….)` |
| **Bench** | validator health, the ELF identity block, the roster, and the genesis-assistance disclosure — the same cards as watch mode, minus the lifecycle rail, which a session with no plan simply does not have |

Trade mode deliberately has more than one field source. Account fields are
decoded through the frozen `clutch_solana_layout` codecs from sequential RPC
data responses, but the current daemon discards account owner, executable bit,
response context, and same-slot consistency, and it selects codecs by its own
role labels. They therefore say **ROLE-DECODED RPC DATA**, not
`chain-derived`. Market configuration, actor/address declarations, order
roster, and session phase come from fixture or daemon memory and say so.
Beliefs and candidate-trial coordinates are MODEL-ONLY. Transaction rows are
daemon-reported RPC receipts. A role that has not been observed says
`NOT YET OBSERVED` rather than showing a zero.

In particular, the daemon publishes its candidate coordinates before it
submits the candidate. The Book never calls those coordinates bank-stamped,
verified, selected, or cleared. Later transaction rows and account projections
remain separate sources until a future authenticated client performs the exact
candidate/epoch/release join.

## The verbs

`POST /api` in a trade session takes intents, one per verb:

| verb | what it means |
| --- | --- |
| `endow` | move `amount` collateral atoms from your ordinary token account into pooled custody |
| `split` | lock `quantity` complete sets: cash becomes one Egg on every active outcome |
| `place` | rest one single-Egg order: `outcome`, `side`, `quantity`, `limit` |
| `place-portfolio` | rest one portfolio order: `coefficients`, `side`, `lots`, `limit_per_lot` |
| `cancel` | retire your order at `rank` |
| `propose` | quantize a `belief` and return the orders it implies — MODEL-ONLY, submits nothing |
| `paint` | do that, then place the list |
| `weights` | the resolution weight vector a terminal statistic at `cents` would carry — MODEL-ONLY |
| `freeze` | close at the deadline, then drive the epoch to settled |
| `status`, `bot` | the session's phase and book; the automaton's disclosure |

None of them is a transaction. Each names *what*, and `builders.rs` decides
*which accounts in which roles*.

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

## Candidate submission and selection

The auto-crank is **not a solver** and claims no optimality. It tries four
*stated* coordinates in a fixed published order and reports which the relation
admitted and exactly how it refused the ones before: the midpoint of the two
published beliefs, the automaton's belief, your painted belief, and the flat
prior. Each attempt is published to the stream with its refusal.

The browser's candidate-trial drawing is the daemon's pre-submit model output,
not the admitted result. Selection evidence lives in later transaction receipts
and role-decoded account records, which this client does not authenticate or
join into a same-slot snapshot.

The midpoint is first because it has the property the frozen allocation policy
needs: at a knot where the two beliefs disagree it sits strictly between the
two limits, and at a knot only one side quoted it sits on the far side of that
quote's limit — which makes an unanswered quote *ineligible* rather than a
strict order nobody can fill. `PricePriorityMarginalProRata` fills every strict
order in full, so an eligible order with no counterparty refuses the whole
candidate. `session.rs`'s tests pin both halves of that.

Before it submits that candidate, the daemon passes the projected page, book,
candidate, and witness through `crates/clutch-client-contract`. The shared gate
currently admits only an exhaustive, exact-conversion, one-page direct
single-Egg plan. Multi-page or churned books, virtual legs, pot-required
conversion, duplicate pair receipts, and every portfolio shape refuse before
submission; the shared crate owns the adversarial matrix. That client result is
only a statement that Operator can construct the complete account shape. It is
not a candidate-validity verdict or evidence that settlement executed.

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
second. A candidate may be labeled `verified` only when that exact status was
decoded from the bank-written record; it is never presented as verification of
the client, adapter, runtime, or protocol as a whole.

The shared client contract separately freezes provenance labels
(`chain-derived`, `chain-history-derived`, `transaction-derived`,
`producer-attested`, `model-only`, `unavailable`). Those labels are not the
claim-strength chips above. In particular, a fresh chain snapshot cannot be
promoted into retained historical evidence or a completion claim.

## No dependencies

Hand-authored ES modules, loaded directly by the browser. No build step,
bundler, transpiler, runtime dependency, or external reference of any kind.
The tiny package manifest names only dependency-free test/check commands. The
gate lives in `scripts/run_operator_trade.sh` and greps every file
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
| `pyth.js` | the read-only retained local-real Pyth campaign screen |

## Not built

In **Trade mode**, multi-page epochs are not built — a trade session opens one order page, so the book holds
`MAX_ORDERS_PER_PAGE` orders and the ticket says so rather than letting the
bank refuse the seventeenth. Resolution and redemption are also not built into
the interactive Friday clutch: it never
resolves, so the Ticket's weight preview is a MODEL-ONLY reading of what a
terminal statistic would carry, not a payout the bank has made.
