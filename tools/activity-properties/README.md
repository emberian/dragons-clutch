# Activity lifecycle properties

`activity_properties.py` is the executable cross-operation property layer for
one or more already reconciled Dragon's Clutch lifecycles. Its first admitted
input is the existing `dclutch-owned-loopback-activity-dossier-v1` projection
from `tools/devnet-reconcile/reconcile.py`; it does not introduce a second DTO
for protocol state or transaction facts. The public dossier remains refused
because its Direct projection does not yet carry both Position transitions,
so it cannot support the same exact asset-conservation claim.

For every dossier it checks:

- the canonical dossier self-digest, exact six-phase order, source-digest
  projection, predecessor chain, and final-state continuity;
- exact transaction lamport conservation: all observed changes sum to the
  negative finalized transaction fee, including account creation rent;
- exact retirement closure rent and refund equality, with each closed account
  observed moving from a positive balance to zero;
- exact per-mint scaled-integer asset conservation after founding;
- Direct gross arithmetic, the one named divisibility boundary, independent
  50-bps side floors, token movements, and seller/buyer Position fill
  conservation;
- payout Hoard-principal classification, token conservation, claim burns, and
  Position revision continuity.

With multiple dossiers it additionally requires one cluster/genesis, distinct
activity IDs, dossier digests, disposable fee-payer wallets, transaction
signatures, and Direct semantic-owner journals. Shared lamport, token, and
Position histories must form exact chains; duplicate revisions, crossed nonces,
same-slot shared-account ambiguity, and missing transitions refuse.

Run the focused suite:

```sh
python3 -m unittest tools/activity-properties/test_activity_properties.py -v
```

Check one lifecycle, or several concurrent lifecycles, without writing a new
artifact:

```sh
python3 tools/activity-properties/activity_properties.py \
  --dossier /absolute/path/activity-a.json \
  --dossier /absolute/path/activity-b.json
```

The canonical JSON report on stdout is an unsigned property verdict binding the
input dossier digests. It is local/devnet execution evidence, not a proof of the
Solana runtime, RPC capture, protocol adapter, or mainnet behavior.
