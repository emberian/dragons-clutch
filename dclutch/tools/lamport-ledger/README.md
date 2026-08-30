# The lamport statement

One question with one answer: **where did every lamport come from, and where did
it go.** Not per family, not per stage — for a whole market, at one slot, with
every number carrying the evidence it came from.

```sh
# 1. resume a validator on the run's own preserved ledger (read-only; work on a
#    COPY if the run belongs to another lane)
tools/gauntlet/frontend/resume-validator.sh /abs/path/runs/seed-01/ledger 39100

# 2. state the market
python3 tools/lamport-ledger/lamport_ledger.py \
  --run-root /abs/path/runs/seed-01 \
  --rpc-url http://127.0.0.1:39100
```

## Why this is not an eighth law

The protocol already has seven (`tools/gauntlet/journey/README.md`). The lamport
one is L7, `payer_delta + fees + watched_growth == 0`, and it is honest, careful,
and **has never once been evaluated over a founding**. That is not an oversight;
it is forced by its shape. L7 is a *delta law over a watched set identified by
label*, and:

| where | what it says | file |
|---|---|---|
| any boundary that admits a new label | `inapplicable`, and names the labels | `tools/gauntlet/journey/src/ledger.rs:655-670` |
| "founding through Open" | `inapplicable` — the founding's placements are tier 1's | `tools/gauntlet/journey/src/journey.rs:124` |
| the successor's `ledger-census` — the only external verifier of a real founded market | `inapplicable`, hardcoded: it "refuses to guess their fees" | `tools/local-validator/bootstrap/successor/src/main.rs:804` |
| the relayed vertical, all five claims | `inapplicable` | `tools/gauntlet/relayed-vertical/src/vertical.rs:329,559,968,1027,1179` |

A founding is *nothing but* account admission, so the first row alone would
silence L7 there. It genuinely evaluates in seven places, all post-Open journey
stages.

**The fix is the shape, not the arithmetic.** A *label* has no predecessor
balance the first time it appears. An *address* always does: a nonexistent
account holds zero, and that is a fact rather than a guess. Stated over
addresses in a closure, the identity is total and nothing has to abstain.

## The two identities

**1. Every funder's own movement.** A run has more than one funder — the
administration stage is paid by the deployer, the founding by the campaign payer
the bankroll transfer funded. For each:

```
opening − closing − fees_it_paid  =  rent it placed
```

**2. The test.** Identity 1 is an arrangement of a definition; on its own it
cannot fail. The test compares it against what the chain actually holds:

```
Σ rent implied by every funder   ==   Σ lamports in campaign-created accounts
```

A difference is a finding with a direction:

- **positive** — accounts hold more than the known funders spent, so some
  *other* key funded them (`rent-from-an-unnamed-funder`);
- **negative** — the funders are poorer than their accounts explain. Lamports
  left and arrived nowhere in the closure, and on a chain the only such
  destination is a transaction fee, so **the campaign's own fee record is
  incomplete** (`spend-exceeds-observed-holdings`).

**3. Optionally, the whole cluster.** With `--universe` and `--capitalization`
the statement also checks that its address set is complete:

```
Σ every account in the ledger  +  fees burned since  ==  capitalization
```

On an idle resumed validator the only traffic is one vote per slot, and half of
each 10,000-lamport vote fee burns, so the gap must be an exact multiple of
5,000. A gap that is *not* means the dump missed an address. Measured on
`selseam-hold-01`: 17,025,000 = exactly 3,405 slots. The address set is complete.

## What it reads, and what it never does

| | |
|---|---|
| journals | a `runs/seed-01` root from `tools/release/private-validator-lifecycle/run.py` — per-transaction fees, the role→program map, named accounts, the bankroll transfer |
| chain | finalized account state over loopback JSON-RPC |
| universe | optionally `agave-ledger-tool accounts --no-account-data --include-sysvars` — **addresses only**, because that dump prints balances as float SOL and a genesis-scale account does not survive an f64 |

It **derives**; it never keeps its own copy of a fact. Every number carries a
journal path plus JSON pointer, or `chain:<address>@<slot>`. Where a journal and
the chain both state a balance, it cross-checks and reports the disagreement
rather than choosing a favourite (`journal-vs-chain`).

It **never invents a class to make a total balance.** An account it cannot
attribute becomes an `unclassified` row carrying its address, owner and balance,
because "an account exists that no flow class claims" is a finding and a
silently-absorbed residual is not.

It refuses a non-loopback endpoint without `--allow-remote-rpc`, refuses mainnet
outright, and refuses a chain whose genesis hash is not the one the run was
driven against — a statement joining two chains would be fiction.

## Fee evidence, its known limit, and the verdict that carries it

Fees come from three places, and the statement cites which for every event:

1. `execution.transactions[].fee_lamports` when the run populated it;
2. the stage's own stderr when `execution` is empty (`campaign transaction:
   slot=N fee=N compute_units=N LABEL`) — a *superset*, so a fallback, never a
   preference;
3. **the founding submission journals**. The funding-readiness ops are driven
   over a durable-packet path that never prints the stage line, so a finalized
   journal's `feeLamports` appears nowhere else. On `selseam-hold-01` that was
   310,000 lamports recorded in `founding.json` and counted by nothing — most
   of the original −385,000 residual. When `execution.transactions` is
   populated the journals join it by signature (run.py's founding gate asserts
   exactly that join) and the fee counts once; a journal whose slot+fee
   coincide with an unsigned stage-log line is flagged as a possible double
   count rather than silently blended.

What remains after all three is the true limit: a journal in phase `submitted`
whose transaction the driver never observed. Its fee is **two-point** — 0 if
the send never landed, or exactly the journal's own `exactFeeLamports` if it
did (the driver computes fees at message-build time; run.py asserts that
equality on every finalized journal). The tool asks the chain
(`getSignatureStatuses` with history search, then `getTransaction`), and every
outcome is stated: `chain-served` promotes the bound to a read fee;
`chain-status-only` proves it landed and the deterministic fee stands as fact;
`chain-unserved` (purged history) leaves a named bound.

The **conservation verdict** then closes the statement one of three ways:

- `exact` — every lamport is in a named account or a named fee;
- `bounded` — the entire miss lies within the unresolved submissions'
  deterministic fee bound, each suspect named with its signature. When the
  residual equals the bound *exactly*, the funders' own balances have selected
  the landed point of every two-point fee, and the verdict says so;
- `divergent` / `unbounded-unknown` — lamports moved that nothing in the
  record explains, or a bound the record cannot state. These fail `--strict`;
  the first two do not, because a closure with a named bound IS a closure.

On `selseam-hold-01` the verdict is `bounded` at −75,000 against the 75,000
bound of `resolution-funding-activate-v1` (signature chain-unserved: the
blockstore purges below slot 15361, and every founding transaction sits below
it) — the original −385,000 resolved to 310,000 of named journal fees plus
this one exactly-selected bound.

## Feeding the existing oracle

`--trace` emits `dclutch-exact-lamport-trace-v1`, the shape
`tools/economic-lifecycle-ledger/ledger.py check-lamports` already validates.
That oracle is **predictive** and never opens RPC, so until now its trace could
only be written by hand. This is the same protocol's **observed** history in the
oracle's own vocabulary — which is what makes the two one system instead of two.

## The flow classes

Holdings are partitioned into classes read off the routes, never off prose.
`market.rent.*` is per role program, and the role→program map comes from the
run's own `founding.json.roles`, so a redeployed cohort cannot be quietly
classified against stale ids.

Two classes are marked terminal: **no route can ever return them.** The Registry
dispatcher (`programs/dclutch-registry-sbf/src/lib.rs:183-188`) routes only
`ActivateRole` and `Reauthenticate`, and Core's infrastructure profile
(`programs/dclutch-core-sbf/src/infrastructure.rs:524-566`) has no close route at
all. Any claim that "all protocol rent is recoverable" is false, and the
statement separates terminal holdings from refundable ones so a reader sees it.

## Tests

```sh
python3 tools/lamport-ledger/test_lamport_ledger.py
```

Every test that matters is a **negative control** planting a defect the tool was
built to find — and each of these is one it actually hit on real evidence while
being written:

- the wrapped-SOL native mint counted as a campaign collateral mint (it is
  Token-owned and 82 bytes, exactly the right shape; it overstated a founding by
  a round 1,000,000,000);
- administration fees booked against the campaign payer when the deployer paid
  them (2,475,000 misattributed);
- the validator identity counted as campaign rent in a whole-cluster closure
  (six orders of magnitude);
- a funder poorer than its accounts explain, which must surface as an incomplete
  fee record and never be absorbed.

## Reading the devnet flagship

The same statement, against a real cluster, is one flag different:

```sh
python3 tools/lamport-ledger/lamport_ledger.py \
  --run-root /abs/path/to/devnet/runs/seed-01 \
  --rpc-url "$DEVNET_RPC_URL" \
  --allow-remote-rpc \
  --json docs/evidence/flagship-lamport-statement.json
```

Mainnet is refused whatever the flags say.
