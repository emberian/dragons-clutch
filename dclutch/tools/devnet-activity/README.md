# Devnet activity harness

This harness prepares disposable wallets and drives a separately authored
economic scenario through accepted dClutch callers. It is orchestration, not a
protocol authority: the scenario owns amounts and expected deltas, the public
CLI/SDK or successor owns each transaction, and finalized Solana history owns
the ledger result.

The harness currently lands in two milestones:

1. `validate`, `prepare-wallets`, and `fund` provide strict input binding,
   secret-safe key creation, exact funding arithmetic, and poll-only crash
   recovery.
2. The lifecycle scheduler binds `found`, participant admission, Direct,
   resolution, redemption, and retirement adapters to the same journals and
   reconciles every named wallet. This milestone stays execution-closed until
   a real Direct caller exists.

The tracked public CLI does not currently submit Direct trades. `intent` only
signs an off-chain intent, and `buy`/`sell` refuse before reading a key or RPC.
The harness refuses to count that path as activity. Owned-loopback proof waits
for the accepted `local-private-validator-lifecycle-v1` caller; devnet execution
waits for that caller's public counterpart and an explicitly authorized live
market.

## Safety model

- The harness never reads keypair bytes. It asks `solana-keygen` to create a
  mode-`0600` file and derive its public address.
- A public ledger contains addresses, roles, and decimal-string lamports only.
  Key paths remain in a mode-`0600` private index.
- Funding is serial even when later activity is concurrent. Each transfer has
  a unique public Memo, a write-ahead journal, and exact finalized arithmetic:
  the wallet gains the requested lamports and the funder loses that amount plus
  the transaction's recorded fee.
- If funding stops after dispatch, reruns only scan finalized history for the
  exact Memo/System-transfer pair. They never send again.
- Owned loopback accepts only a credential-free explicit-port loopback HTTP
  origin. Devnet refuses loopback, checks the full genesis hash, caps concurrency
  at two, and requires at least one second between dispatches.
- Devnet mutation requires a separate at-most-six-hour authorization document
  bound to the exact scenario and activity-manifest SHA-256. Validation and
  wallet creation need no authorization because they cannot reach RPC or sign.

## Inputs

The activity manifest uses schema `dclutch-devnet-activity-manifest-v1`. It
binds one canonical scenario by absolute path and exact SHA-256, one target,
immutable input files, and caller adapters. The scenario is owned outside this
directory and carries:

```text
schema, scenarioId, clusterTarget, marketRef, wallets, operations, limits
```

Wallet funding and every economic delta are decimal strings. Each operation is
one of `found`, `participant`, `direct`, `resolve`, `redeem`, or `retire`, names
its wallet set and dependencies, and declares whether mutation is expected.
Adapters must cover every operation exactly once. Only a private full-lifecycle
caller may cover several operations with one command.

## Prepare and fund

All paths are explicit; there is no default wallet fallback.

```sh
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity-manifest.json \
  --work /work/activity-run \
  validate

python3 tools/devnet-activity/activity.py \
  --manifest /work/activity-manifest.json \
  --work /work/activity-run \
  prepare-wallets \
  --solana-keygen /absolute/bin/solana-keygen

# Owned loopback uses a disposable validator-owned funder key.
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity-manifest.json \
  --work /work/activity-run \
  fund \
  --solana /absolute/bin/solana \
  --solana-keygen /absolute/bin/solana-keygen \
  --funder-keypair /work/private-validator-funder.json

# If dispatch becomes ambiguous, this command can only poll/reconcile.
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity-manifest.json \
  --work /work/activity-run \
  fund \
  --solana /absolute/bin/solana \
  --solana-keygen /absolute/bin/solana-keygen \
  --funder-keypair /work/private-validator-funder.json \
  --poll-only
```

For devnet funding, also pass `--live-authorization` naming a document with
schema `dclutch-devnet-activity-live-authorization-v1`, the exact manifest and
scenario digests, the full devnet genesis hash, the scenario's `marketRef`, an
at-most-six-hour `notBefore`/`expiresAt` window, and the exact phrase
`authorize-one-devnet-activity-run`.

## Stop and cleanup

`stop` writes an authenticated control file. The lifecycle scheduler will not
dispatch another action after observing it.

```sh
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity-manifest.json \
  --work /work/activity-run \
  stop --reason 'operator convergence window'
```

After every funding journal is finalized, `cleanup-keys` irreversibly removes
only the disposable keypair files and the private path index. It preserves the
public wallet ledger and writes a cleanup receipt. You must repeat the exact
scenario id as an explicit confirmation.

```sh
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity-manifest.json \
  --work /work/activity-run \
  cleanup-keys \
  --solana-keygen /absolute/bin/solana-keygen \
  --confirm-scenario flagship-activity
```

## Test

The focused suite uses only temporary files, fake executables, and a loopback
JSON-RPC server. It never reads a real key or reaches a cluster.

```sh
python3 -m unittest -v tools/devnet-activity/test_activity.py
```
