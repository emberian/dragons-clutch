# Devnet activity harness

This harness creates disposable wallets, funds them with exact lamport
arithmetic, invokes accepted public dClutch callers, and reconciles finalized
history. It is orchestration, not a protocol authority. Canonical economic
scenarios own expected integer deltas, caller completion documents own their
schemas, and finalized RPC transactions own observed results.

The harness never uses `projectedAcceptedDelta` as execution evidence. It
compares only `expectedObservedDelta` and explicitly records
`untrustedProjectionUsed: false`.

No live devnet run is part of this directory's test evidence. The focused suite
uses synthetic scenario fixtures, fake executables, and a private loopback RPC
server. The canonical devnet fixtures at `tools/devnet-scenarios/fixtures/`
currently mark Direct, payout planning, and retirement as nonmutating caller
gaps. They cannot produce a complete live dossier until accepted mutating
callers and exact completion receipts exist. A real PRIVATE proof also needs a
separately generated canonical owned-loopback scenario; the harness does not
rewrite a devnet fixture to create one.

## Safety properties

- `solana-keygen` creates mode-`0600` disposable key files. The harness never
  reads or prints their bytes. Public ledgers contain addresses, roles, and
  decimal-string amounts only.
- Every funding transfer is serial, has a unique Memo, and moves through a
  write-ahead `planned -> dispatching -> finalized` journal. Finalized history
  must prove the exact System transfer, wallet credit, funder debit, and fee.
- Caller arguments are arrays, never shell strings. Public journals contain
  only argument/path digests; caller output goes to mode-`0600` private logs.
- A mutating adapter must name at least one exact completion JSON pointer to a
  finalized signature. The transaction must contain that same signature and a
  successful finalized result.
- The scheduler preserves the scenario dependency graph, enforces the exact
  involved wallet set, never runs two adapters sharing a wallet concurrently,
  honors `maxConcurrency`, and spaces dispatches by
  `minDispatchIntervalMs`.
- A STOP control prevents each new dispatch. It does not interrupt a child that
  already owns an ambiguous transaction boundary.
- Public devnet requires the full genesis hash, at most two concurrent callers,
  at least one second between dispatches, and a separate at-most-six-hour live
  authorization bound to the exact manifest, scenario, and Market.
- An ordinary run requires that authorization to be current. Poll-only recovery
  accepts it after expiry only when its exact file SHA-256 matches every
  pre-existing funding/activity journal. Recovery never creates a journal or
  dispatches a caller.

## Canonical scenario input

The activity manifest consumes the exact envelope
`dclutch-devnet-economic-scenario-v1`, version `1`, including its compact-body
digest. The authoritative generator and field contract are in
`tools/devnet-scenarios/README.md`.

The activity manifest schema is `dclutch-devnet-activity-manifest-v1` with
exact top-level fields:

```text
schema, scenario, target, inputs, addressBindings, adapters
```

- `scenario` is `{path, sha256}` using absolute immutable bytes.
- `target` is `{kind, rpcUrl, devnetGenesisHash}`. Ordinary manifests require
  an explicit URL port. Public devnet uses
  `https://api.devnet.solana.com:443/` in the frozen supervisor join.
- `inputs[]` is `{id, path, sha256}`. Template expansion accepts only
  `{{input.ID}}` for these bound files.
- `addressBindings[]` is `{ref, source}`. A source is exactly one of
  `{kind: wallet, walletRef}`, `{kind: literal, address}`, or
  `{kind: input-json, inputId, pointer}`. These bindings join logical token
  accounts/Mints to physical addresses for reconciliation.
- `adapters[]` is
  `{id, covers, caller, argv, dependsOn, wallets, mutation, completion}`.
  `caller` is `dclutch-cli` or `successor`; `argv` omits the executable.
  Completion is `{path, schema, signaturePointers, requiredValues}`.

Templates are limited to `{{rpc}}`, `{{devnetGenesis}}`, `{{work}}`,
`{{input.ID}}`, `{{wallet.ID.address}}`, and `{{wallet.ID.keypair}}`.

## Lifecycle commands

All paths are explicit. There is no default wallet or RPC fallback.

```sh
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run validate

python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run prepare-wallets \
  --solana-keygen /absolute/bin/solana-keygen

python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run fund \
  --solana /absolute/bin/solana \
  --solana-keygen /absolute/bin/solana-keygen \
  --funder-keypair /private/funder.json \
  --live-authorization /private/authorization.json

python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run run \
  --dclutch-bin /absolute/bin/dclutch \
  --successor-bin /absolute/bin/dclutch-local-successor-bootstrap \
  --solana-keygen /absolute/bin/solana-keygen \
  --live-authorization /private/authorization.json
```

`resume` is poll-only. It does not probe or invoke a caller. Its terminal states
are:

```text
no-pending-submissions  no dispatching funding/activity journal exists
pending-funding         ambiguous funding still has no finalized exact Memo tx
funding-finalized       funding recovered; no activity was submitted
partial-recovery        submitted activity recovered; other adapters untouched
complete                every adapter is finalized
```

Only `complete` admits full `reconcile`. Reconciliation refreshes every
signature from finalized RPC, refuses duplicate/foreign wallet history,
rechecks funding plus fee arithmetic, sums each disposable wallet's native
delta to its final balance, and compares bound token deltas only with canonical
`expectedObservedDelta`.

```sh
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run resume \
  --dclutch-bin /absolute/bin/dclutch \
  --successor-bin /absolute/bin/dclutch-local-successor-bootstrap \
  --solana-keygen /absolute/bin/solana-keygen \
  --live-authorization /private/original-authorization.json

python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run reconcile \
  --dclutch-bin /absolute/bin/dclutch \
  --successor-bin /absolute/bin/dclutch-local-successor-bootstrap \
  --solana-keygen /absolute/bin/solana-keygen \
  --live-authorization /private/original-authorization.json
```

Owned loopback refuses a live-authorization file. The keygen argument on
`resume`/`reconcile` is retained in the ordinary CLI ABI, but those read-only
paths use only the authenticated private index's paths and public addresses;
they do not open key files or invoke keygen.

## Supervisor cycle ABI

The hbox child ABI is deliberately incapable of sending. Canonical argv order:

```text
dclutch-wallet-harness supervisor-cycle-v1
  --manifest ABS --manifest-sha256 HEX64 --scenario-id ID
  --work /tank/dclutch-activity/runs/<manifest-sha256>
  --rpc-url https://api.devnet.solana.com/
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
  --journal /tank/dclutch-activity/.../request.json
  --evidence-dir /tank/dclutch-activity/.../evidence
  --accepted-harness-sha256 HEX64
  --accepted-harness-source-commit HEX40
  --cycle-id ID
  --dclutch-bin ABS --successor-bin ABS
  [--live-authorization ABS --live-authorization-sha256 HEX64]
  --no-send [--poll-only]
```

The supervisor-facing spelling intentionally omits default port 443, while the
manifest spelling includes `:443`; the harness accepts only that exact pair and
no other normalization.

Infra owns and atomically creates one immutable request journal per cycle. Its
schema is `dclutch-devnet-activity-supervisor-request-v1` with exact fields:

```text
schema, manifestSha256, scenarioId, workPath,
supervisorRpcUrl, manifestRpcUrl, devnetGenesisHash,
acceptedHarnessSha256, acceptedHarnessSourceCommit,
cycleId, requestedAt, mode, evidenceDirectory,
liveAuthorizationSha256, stateSha256
```

`mode` is `no-send` or `poll-only`. `no-send` requires a current exact live
authorization and performs only manifest/input parsing, caller `--help`
capability probes, and an RPC genesis read. `poll-only` with no submitted
journals requires no authorization and returns `no-pending-submissions`.
Recovery of an existing dispatching journal requires its original authorization
but accepts that capability after expiry. It never accepts a current different
capability as a retry.

The harness never edits the request journal. It appends one authenticated status
file per immutable request at:

```text
<evidence-dir>/<manifest>.<cycle-id>.<request-sha256>.supervisor-status.json
```

Status schema `dclutch-devnet-activity-supervisor-status-v1` records the request,
scenario, accepted source commit and byte hash, mode, terminal status, optional
full-reconciliation digest, and `newDispatches: "0"`. Repeating the exact same
request is idempotent. A new mode uses a new request/cycle/status path, so a
poll-only startup can transition to a later no-send readiness cycle without
overwriting evidence. Success exits `0`; every refusal exits `2` and does not
write a success status.

## Stop and cleanup

```sh
python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run stop \
  --reason 'operator convergence window'

python3 tools/devnet-activity/activity.py \
  --manifest /work/activity.json --work /work/run cleanup-keys \
  --solana-keygen /absolute/bin/solana-keygen \
  --confirm-scenario exact-scenario-id
```

Cleanup is irreversible. It refuses until funding and every activity adapter are
finalized, removes only the disposable key files/private index, and preserves
the public wallet ledger and cleanup receipt.

## Offline test

```sh
python3 -m unittest -v tools/devnet-activity/test_activity.py
```
