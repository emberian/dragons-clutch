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
  projection, predecessor chain, exact per-transaction compute units and total,
  and final-state continuity;
- exact transaction lamport conservation: all observed changes sum to the
  negative finalized transaction fee, including account creation rent;
- exact retirement closure rent and refund equality, with each closed account
  observed moving from a positive balance to zero; the terminal receipt must
  pay the creation-fixed refund beneficiary from classified historical account
  lamports, with one distinct fee payer and no future-revenue or Hoard-principal
  capitalization;
- exact per-mint scaled-integer asset conservation in every finalized
  non-founding transaction and across the whole post-founding history;
- Direct gross arithmetic, the one named divisibility boundary, independent
  50-bps side floors, seller-net-only Hot token movements, and seller/buyer
  Position fill conservation; a distinct later transaction must settle the
  combined seller and buyer fee from the buyer's actual collateral account to
  the fee recipient, under the exact standing allowance and custody revision,
  with a finalized stranger fee payer;
- payout Hoard-principal classification, disjointness from every Direct
  trading-token role, token conservation, claim burns, and Position revision
  continuity; the terminal recipient token must belong to the named holder,
  while the finalized transaction fee payer must be someone else.

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

The canonical JSON report on stdout uses
`dclutch-activity-lifecycle-property-report-v2`. Its `livenessEconomics`
section binds the observed permissionless fee-completion payer, fixed retirement
beneficiary, retirement payer, and the accepted non-capitalization classes.
It is an unsigned property verdict binding the input dossier digests and is
local-validator execution evidence, not a proof of the Solana runtime, RPC
capture, protocol adapter, or mainnet behavior.
