# Devnet aquarium v1

`tools/load-simulator/aquarium.py` keeps a bounded inventory of **already
founded, still-active Direct markets** visible while the existing devnet Direct
drivers make finite, journaled activity epochs. It is an operator supervisor,
not a market constructor, wallet service, or alternative transaction client.

The distinction is deliberate. A market is founded by the fresh cohort's
checked runbook, and a Direct session is made only by the shipped
`devnet-direct-trade-produce-v1` and `devnet-direct-trade-v1` commands through
`simulator.py`. The aquarium never constructs a packet and never opens or
discovers a keypair. It invokes only:

```
python3 tools/load-simulator/simulator.py run --config E --cycles N [--execute]
```

`--execute` is required at both layers before any child can sign. Without it,
the exact same child command is a preflight and private key files remain
unopened.

## Admission gate

Do not make an aquarium config until all of these are true:

1. The fresh cohort manifest has a nonempty 40-hex `deploy_commit`, all current
   program ids, and the launched Direct market addresses.
2. The manifest's sha256, commit, and program ids are copied into the aquarium
   config and match byte-for-byte. Its prior manifest is also named. Every role
   must have a new program identity; reuse is a refusal.
3. The cohort's reproducible `RELEASE_GATE.json` is present and its digest is
   recorded separately. The aquarium rehashes it, requires schema
   `dclutch-reproducible-release-gate-v1`, and requires that its
   `source_revision` equals the manifest's `deploy_commit`. A timestamp alone
   is not a release check. The separately deployed General accelerator is also
   pinned by program id, deployment slot, ELF digest, and semantic release id.
4. Every simulator epoch has a credential-free HTTPS devnet configuration,
   devnet genesis acknowledgement, the exact inventory market address, a
   positive `budget.max_lamports_spent`, and distinct sha-pinned Direct ticket
   pairs.

The supervisor itself makes no RPC call during `check`; it is safe to use
before a deploy. A blank cohort-18 placeholder consequently refuses instead of
being mistaken for a launch authorization.

## Bounded epochs

Direct tickets authorize one exact pair. The simulator's legacy `--sustain`
mode cycles its configured pairs and is therefore not an aquarium command.
Each aquarium epoch has a distinct simulator config and work directory; its
cycle count may not exceed its number of unique pinned pairs. A ticket digest
may not occur in any other aquarium epoch.

The global `limits.max_lamports_spent` bounds the sum of the child budgets.
The child ledger measures cumulative payer outflow, including fees, account
rent, and any other debits; it does not call a refill a negative spend. This is
the meaningful spend ceiling available to the current driver, rather than a
made-up fee estimate. Market count is capped at 32 and wallet count at 512.

The caps of 32 active markets, 256 cycles per epoch, and 512 synthetic wallets
are **provisional operational bounds**, not protocol limits. Lift one only with
a measured profile of child process count, work-directory storage, ticket
authoring time, and the browser parser's bounded-list behavior; then change the
named hard cap, its hostile bound test, and this runbook together. The 32-market
lift needs a bounded-feed browser measurement, the 256-cycle lift needs a
single-child journal/storage profile, and the 512-wallet lift needs ticket
preparation time and actor-population process measurements.

When all precommitted epochs finish, the supervisor writes `stopped`. It leaves
every market in the active inventory. More activity requires a reviewed config
with fresh ticket pairs and a new simulator work directory. This is the
replenishment boundary: it prevents a daemon from silently reusing a ticket or
spending outside its reviewed envelope.

## Config shape

All filesystem locations are absolute and private; none are copied into the
public status feed.

```json
{
  "schema": "dclutch-aquarium-config-v1",
  "work_dir": "/private/job/aquarium",
  "public_status": "/private/public/aquarium-status-v1.json",
  "limits": {
    "max_active_markets": 2,
    "min_active_markets": 2,
    "max_wallets": 16,
    "max_lamports_spent": 500000000,
    "heartbeat_seconds": 90
  },
  "cohort": {
    "manifest": "/private/job/cohort-18.json",
    "manifest_sha256": "<64 lowercase hex>",
    "deploy_commit": "<40 lowercase hex>",
    "checked_at": "2026-09-07T00:00:00+00:00",
    "program_ids": {"registry": "<pubkey>", "rent": "<pubkey>", "custody": "<pubkey>", "resolution": "<pubkey>", "claims": "<pubkey>", "trading": "<pubkey>", "core": "<pubkey>"},
    "prior_manifest": "/private/job/cohort-17.json",
    "general_accelerator": {"program_id": "<pubkey>", "deployment_slot": 0, "elf_sha256": "<64 lowercase hex>", "semantic_release_id": "<64 lowercase hex>"},
    "release_gate": {"path": "/private/release/RELEASE_GATE.json", "sha256": "<64 lowercase hex>"}
  },
  "synthetic_actors": ["<explicit actor pubkey>"],
  "markets": [{
    "market_id": "direct-1",
    "address": "<manifest market-1 pubkey>",
    "source_market_label": "1",
    "join_open": false,
    "epochs": [{
      "epoch_id": "opening-01",
      "cycles": 3,
      "simulator_config": "/private/job/direct-1-opening-01.simulator.json"
    }]
  }]
}
```

## Preparing a replenishment epoch

`prepare-epoch` is the bounded, offline half of replenishment. It takes a
separate `dclutch-aquarium-epoch-preparation-v1` specification naming a checked
aquarium config, an existing inventory market, a fresh epoch id and simulator
work directory, a credential-free simulator template, and exactly one explicit
seller/buyer ticket pair per requested cycle. Each author input states every
ticket term and names a `keypair_env`; it never gives a key path on the command
line. Both sides must agree on market, outcome, generation, validity interval,
fill, price, and fee, and must be distinct makers.

```
python3 tools/load-simulator/aquarium.py prepare-epoch --spec /private/job/replenish.json
python3 tools/load-simulator/aquarium.py prepare-epoch --spec /private/job/replenish.json --author
```

The first command only validates and prints the exact signing plan. `--author`
explicitly permits the existing `direct-intent-ticket-author-v1` to sign local
portable ticket files; it makes no RPC request and submits no transaction. It
produces a new simulator config with its ticket digests and a private
`prepared-epoch.json`. Its plan records the prior, candidate, and cumulative
reserved lamports. It refuses a candidate that would exceed the existing
configuration's global spend limit, a simulator work directory that already
exists, and a generated ticket digest that is already pinned by the checked
plan. Review its epoch stanza, add it to the aquarium config, and run
`aquarium.py check` again; full-inventory validation repeats the cumulative
spend and global ticket-unique checks. It never edits an accepted aquarium
config itself.

Run the nonmutating admission check, then a preflight, then the reviewed
execution. The latter is an authorized devnet action only after the fresh
cohort has been fully deployed from a commit and its release owner has supplied
the completed manifest.

```
python3 tools/load-simulator/aquarium.py check --config /private/job/aquarium.json
python3 tools/load-simulator/aquarium.py run --config /private/job/aquarium.json
python3 tools/load-simulator/aquarium.py run --config /private/job/aquarium.json --execute
```

The supervisor records its exact config digest and phase in
`epochs/<market>/<epoch>/journal.json`; changing the simulator config underneath
an existing journal is a refusal. The child retains its own signed journals and
is never resent by the supervisor. SIGTERM and SIGINT terminate the child,
which seals its journal, then publish `stopped`. A nonzero child result publishes
`halted` with a redacted reason and retains the private driver transcript.

## Public status

The only public file is `aquarium-status-v1.json` with schema
`dclutch-aquarium-status-v1`. It carries the checked cohort identity, liveness
deadline, bounded counts and spend, aggregate activity outcomes, and a unique
list of at most `limits.max_active_markets` market ids and addresses. It never
carries a local path, RPC credential, signature transcript, or keypair path.

`activity.synthetic_actors` is always `true` in v1. `join_open` is always
`false` while the checked release material has no canonical, published
first-admission linked-basis binding for that market. The browser must describe
this feed as observed, untrusted real-chain activity by configured synthetic
actors. It must not call it an official market source or claim that an ordinary
visitor can join.

A public join implementation can accept a visitor's wallet signature, build the
admission route without the supervisor learning their key, report the finalized
evidence, and allocate a fresh pinned Direct ticket pair. The current launch
gate is its canonical published first-admission linked-basis binding in checked
market/release material, plus its matching chain read. Until that evidence is
accepted by the aquarium schema, a visible active market is watchable devnet
activity rather than an open public entrance.
