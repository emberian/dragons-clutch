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
3. The cohort's own post-deploy/release gate has been checked by its owner. The
   aquarium records the gate's checked timestamp and refuses a manifest whose
   bytes, commit, or program set disagree. It does not substitute its own
   shallow file check for that gate.
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
    "prior_manifest": "/private/job/cohort-17.json"
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
`false`: the current admission driver requires an operator-supplied owner
keypair path, so it is not a stranger-facing wallet handoff. The browser must
describe this feed as observed, untrusted real-chain activity by configured
synthetic actors. It must not call it an official market source or claim that
an ordinary visitor can join.

Delivering a public join requires a separate noncustodial frontend/adapter that
accepts a visitor's wallet signature, builds the current admission route without
the supervisor learning their key, reports the finalized admission evidence,
and then allocates a fresh pinned Direct ticket pair. That capability is not
present in the current drivers or this supervisor. Until it lands, a visible
active market is a watchable devnet market, not an open public entrance.
