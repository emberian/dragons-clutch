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
- A simple mutating adapter must name at least one exact completion JSON
  pointer to a successful finalized signature. A successor campaign instead
  binds its full ordered `execution.transactions` list and names the labels
  that must have succeeded. Every listed signature, including a fee-paying
  failed hostile probe, is reconciled and included in both spend caps.
- The scheduler preserves the scenario dependency graph, enforces the exact
  involved wallet set, never runs two adapters sharing a wallet concurrently,
  honors `maxConcurrency`, and spaces dispatches by
  `minDispatchIntervalMs`.
- A STOP control prevents each new dispatch. It does not interrupt a child that
  already owns an ambiguous transaction boundary.
- Public devnet requires the full genesis hash, at most two concurrent callers,
  at least one second between dispatches, and a separate at-most-six-hour live
  authorization bound to the exact manifest, scenario, and Market.
- The service-capable authorization additionally binds a finalized funding
  closure, checked release, Market artifact, harness source/bytes, all four
  executable digests, one lifecycle, a total wallet-debit ceiling, and a
  separate activity-transaction-fee ceiling.
- An ordinary run requires that authorization to be current. Poll-only recovery
  accepts it after expiry only when its exact file SHA-256 matches every
  pre-existing funding/activity journal. Recovery never creates a journal or
  dispatches a caller.

## Canonical scenario input

The activity manifest consumes the exact envelope
`dclutch-devnet-economic-scenario-v1`, version `1`, including its compact-body
digest. The authoritative generator and field contract are in
`tools/devnet-scenarios/README.md`.

The public activity manifest schema is `dclutch-devnet-activity-manifest-v3` with
exact top-level fields:

```text
schema, scenario, target, inputs, addressBindings, adapters, campaign
```

Version 2 is evidence-only and refuses public devnet execution. Version 3 owns
the exact eight-role founding identity partition, payer-only initial funding,
five fresh zero-lamport campaign signers, and named post-founding wallet
funding. A prefunded campaign-created role is unsupported and refused.

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
  Completion is `{path, schema, signaturePointers, transactionListPointer,
  requiredTransactionLabels, requiredValues}`. Simple receipts set the two
  transaction-list fields to `null` and `[]`. A successor campaign sets
  `signaturePointers: []`, `transactionListPointer:
  /execution/transactions`, and lists every stage whose finalized transaction
  is required for semantic completion. Its exact `checked-release` and
  `market` manifest inputs must also be the campaign's `--plan` and `--market`,
  and its completion path must be the campaign's `--evidence` path.

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
  --solana-bin /absolute/bin/solana \
  --live-authorization /private/authorization.json
```

## Root-owned artifact bundle

Before you enable the service, validate one bundle with schema
`dclutch-devnet-activity-artifact-bundle-v1`. It replaces a loose collection
of release, Market, authorization, scenario, and executable arguments with one
state-digested join. It has the exact top-level fields:

```text
schema, stage, cluster, artifacts, binaries, ensemble, bindings, stateSha256
```

- `stage` is exactly `template`, `ready`, or `reconciled`.
- `artifacts` binds the Activity-v3 manifest, scenario, checked release,
  Market, installed harness bytes/source commit, live authorization, public
  wallet ledger, and terminal reconciliation. The last three are absent in a
  template; reconciliation is absent until the run finishes.
- `binaries` has exactly four ordered rows: `dclutch`, `successor`,
  `solana-keygen`, and `solana`. Every row binds an absolute executable path
  and SHA-256. The validator also requires the v3 authorization to carry the
  same four digests.
- `ensemble` is not free-form. The validator derives it from the exact
  scenario and manifest: wallet roles and funding phases, every economic
  action in predecessor order, the six reader-facing event kinds, every
  completion source, the scenario's exact fee fraction and integer-floor
  rounding rule, and the reconciliation invariants.
- `bindings.walletAddresses` is null in a template, then must match the
  authenticated public wallet ledger exactly. `bindings.activitySignatures`
  is empty until reconciliation, then must match the ordered adapter/signature
  rows in the authenticated reconciliation exactly. These are injected
  values, not another semantic owner.

Validation is offline and key-free. It does not construct an RPC client, open
a wallet file, or start the service:

```sh
python3 tools/devnet-activity/activity.py activity-artifact-bundle-v1 \
  --bundle /absolute/activity-artifact-bundle.json \
  --require-stage ready
```

The deterministic reconciliation contract requires finalized evidence for
every listed transaction, successful required completion transactions, global
funding/activity signature uniqueness, nonregressing slots, exact wallet and
token/Position continuity, declared changed accounts, scenario-owned observed
deltas and fee rounding, separate hoard-principal classification, closed
post-init transfer-plus-fee arithmetic, and terminal raw account closure.

Funding is a separate authority boundary. `fund` is run once outside the
supervisor with the authorized development funder. After every wallet transfer
is finalized, it creates `public/funding-closure.json` with schema
`dclutch-devnet-activity-funding-closure-v1`. That closure authenticates the
public wallet ledger and every ordered funding journal, signature, slot,
transfer, fee, funder, and journal SHA-256. A later bounded live-send
authorization binds the closure SHA-256. The service receives disposable
participant key paths only; it never receives the funder or deployer key.

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

The hbox child ABI has two explicit dispatch modes. Omitting the mode is a
refusal; an environment value independently has to agree. Canonical argv order:

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
  --scenario-sha256 HEX64
  --checked-release ABS --checked-release-sha256 HEX64
  --market ABS --market-sha256 HEX64
  --cycle-id ID
  --dclutch-bin ABS --accepted-dclutch-sha256 HEX64
  --successor-bin ABS --accepted-successor-sha256 HEX64
  --solana-keygen-bin ABS --accepted-solana-keygen-sha256 HEX64
  [--live-authorization ABS --live-authorization-sha256 HEX64]
  (--no-send | --live-send) [--poll-only]
```

The supervisor-facing spelling intentionally omits default port 443, while the
manifest spelling includes `:443`; the harness accepts only that exact pair and
no other normalization.

Infra owns and atomically creates one immutable request journal per cycle. Its
schema is `dclutch-devnet-activity-supervisor-request-v2` with exact fields:

```text
schema, manifestSha256, scenarioId, workPath,
supervisorRpcUrl, manifestRpcUrl, devnetGenesisHash,
acceptedHarnessSha256, acceptedHarnessSourceCommit,
scenarioSha256, checkedReleaseSha256, marketSha256,
dclutchSha256, successorSha256, solanaKeygenSha256,
cycleId, requestedAt, mode, dispatchMode, evidenceDirectory,
liveAuthorizationSha256, authorizationMaxCycles,
authorizationMaxSpendLamports, authorizationMaxFeeLamports,
prefundedWalletClosureSha256, stateSha256
```

`dispatchMode` is `no-send` or `live-send`; `mode` is that value or
`poll-only`. `no-send` requires a current exact live
authorization and performs only manifest/input parsing, caller `--help`
capability probes, and an RPC genesis read. `poll-only` with no submitted
journals requires no authorization and returns `no-pending-submissions`.
Recovery of an existing dispatching journal requires its original authorization
but accepts that capability after expiry. It never accepts a current different
capability as a retry.

`live-send` accepts only
`dclutch-devnet-activity-live-authorization-v2`, the exact phrase
`authorize-bounded-devnet-activity-live-send`, `maxCycles: 1`, canonical
positive `maxSpendLamports` and `maxFeeLamports`, the prefunded closure digest,
and exact release/Market/harness/binary pins. It authenticates every pin and the
closure before wallet verification or dispatch. STOP refuses each new
dispatch. Activity journals become `dispatching` durably before a caller is
started. After completion, finalized reconciliation must prove total disposable
wallet debit no greater than `maxSpendLamports` and the sum of activity
transaction fees no greater than `maxFeeLamports`. Funding fees are separately
recorded in the outside-service funding closure.

The harness never edits the request journal. It appends one authenticated status
file per immutable request at:

```text
<evidence-dir>/<manifest>.<cycle-id>.<request-sha256>.supervisor-status.json
```

Status schema `dclutch-devnet-activity-supervisor-status-v2` records the request,
scenario, accepted source commit and byte hash, mode, terminal status, optional
full-reconciliation digest, authorization caps, prefunded closure, reconciled
wallet debit, reconciled activity fees, and the exact new-dispatch count.
Fresh live success is `complete-reconciled-live-send`; all later handling is
poll/reconcile-only under the original capability. Repeating the exact same
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
