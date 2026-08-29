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

The committed public Direct and terminal-sequence callers are progressive:
each invocation advances exactly one durable action and a later invocation
reauthenticates the child journal before continuing. An adapter using either
caller must carry an exact `progressive` contract. The harness writes one
fsynced journal before every child invocation, polls the terminal completion,
and invokes the same authenticated child again when an earlier invocation
exited or its RPC answer was lost. Every step binds the accepted manifest,
scenario, Market, caller-source, private-session, binary, argv, and completion
spec SHA-256. `maxSteps` is a hard bound in `1..256`; exhaustion refuses instead
of turning an unfinished lifecycle into success. `resume` remains poll-only and
never invokes a child.

## Finite ongoing run envelope

`ongoing_v1.py` adds a separate finite supervisor rung without changing the
V1--V3 manifest, authorization, or `supervisor-cycle-v1` bytes. In particular,
the old child supervisor still refuses `maxCycles != 1`. The ongoing manifest
schema is `dclutch-devnet-activity-ongoing-manifest-v1` and has exact fields:

```text
schema, runId, workBase, maxCycles, economicAuthority,
acceptedHarness, binaries, cycles
```

`maxCycles` is one explicit integer in `1..72` and must equal the exact cycle
list length. Every cycle binds one distinct Activity-v3 manifest and one
distinct `dclutch-devnet-activity-cycle-rent-envelope-v1`. Child manifest paths
and bytes, Rent-envelope paths, progressive private-session paths and bytes,
derived work paths, wallet key-slot ids, and session-slot ids must be globally
unique. Reusing the economic scenario, checked Market, or accepted caller
source is permitted; these are shared authorities, not disposable state.

The Rent envelope is deliberately separate from the economic fixture. It binds
the child manifest/scenario/devnet, an observed slot, exact Rent-sysvar digest,
ordered account/lamport rows, and their exact sum. The planner never calls the
fixture's guaranteed residual “rent.” Instead it derives each maximum payer
debit as:

```text
maxSpendLamports                 (post-init principal + post-init fee cap)
+ maxFeeLamports                (activity transaction fee cap)
+ exact cycle Rent envelope
```

That total must fit inside the fixture-owned `360000000` payer funding. All
payer funding, post-init principal, both fee ceilings, Rent, maximum debit, and
minimum residual are then checked-added across the finite cycle count. The
planner imports the economic ledger's existing authenticator and accepts only
the repository `activity-v3-canonical.json`; it does not own a second economic
quantity table.

Live preparation uses schema
`dclutch-devnet-activity-live-authorization-v4`. Its canonical signed body binds
the ongoing manifest and derived plan SHA-256, a fresh 32-byte run nonce,
deterministic run-envelope id/work root, exact finite count, aggregate monetary
envelope, devnet genesis, economic authority, harness source/bytes, four
binary paths/digests, accepted authorization signer, accepted verifier digest,
and an ordered at-most-six-hour window. The signature is detached Ed25519 over
the canonical sorted compact JSON body.

`dclutch-verify-ed25519` is a verifier-only binary exposing the relayer's
existing `keys::verify_detached` / `ed25519-dalek::verify_strict` owner. It has
no signing or key-file command. `verify-and-prepare` hashes the installed
verifier, invokes it with the exact canonical body bytes, requires its exact
success document, and only then creates the deterministic run root and
write-ahead journal. A different nonce requires a new signature and produces a
different root; replaying the same authorization resumes its exact journal.

```sh
python3 tools/devnet-activity/ongoing_v1.py \
  --manifest /absolute/ongoing.json --manifest-sha256 HEX64 \
  plan --output /absolute/ongoing-plan.json

python3 tools/devnet-activity/ongoing_v1.py \
  --manifest /absolute/ongoing.json --manifest-sha256 HEX64 \
  verify-and-prepare \
  --live-authorization /absolute/signed-v4.json \
  --verifier /absolute/bin/dclutch-verify-ed25519 \
  --accepted-signer-public-key BASE58
```

The run journal permits at most one active lifecycle. `begin_or_resume_cycle`
writes `active` before materializing that cycle's work marker; after a crash it
returns the same cycle and work path, never the next one. Immediately after
wallet preparation and before funding/mutation, `admit_active_wallet_ledger`
joins the public wallet ids/addresses to the derived key slots and refuses any
address seen in a completed cycle. Only a child V3 supervisor status with exact
reconciled completion plus the same admitted wallet ledger and reconciliation
can close the cycle and expose its successor.

Before a V4 authorization is signed, every Direct progressive adapter in every
cycle must already have its ordinary manifest-bound private-session file and
one distinct successor
`dclutch-devnet-direct-trade-session-producer-journal-v1` reference in that
cycle's `directSessionProducers` list.  Each entry is exactly
`{adapterId,journal:{path,sha256}}`, in Direct-adapter order.  The finite-plan
parser accepts only the producer's `finalized` phase and rechecks its state
digest, public-manifest/source SHA, checked-release plan SHA, Market SHA, and
the exact manifest session path/SHA.  It records the producer journal SHA,
terminal producer state SHA, and session path/SHA in both the signed plan and
the run journal.  A prepared journal, a different session file, or a reused
producer journal refuses before a V4 body can be formed.  The V3 child then
continues to consume its normal immutable `{{input.*}}` session; the finite
adapter never introduces a second session authority or a runtime override.

The preceding offline preparation manifest is separately versioned as
`dclutch-devnet-activity-direct-session-preparation-manifest-v1`.  It names an
accepted successor binary and finite `cycles[]`; each producer entry carries
the five accepted input file/digest pairs (public manifest, plan, Market,
seller participant, buyer participant), the runtime payer-keypair *path*, and
three unique output paths.  Run it before generating the V3 manifests that
bind those produced sessions:

```sh
python3 tools/devnet-activity/prepare_direct_sessions_v1.py \
  --manifest /absolute/direct-session-preparation.json \
  --manifest-sha256 HEX64
```

It invokes only `devnet-direct-trade-session-produce-v1`; that successor
command is offline and does not read its payer key bytes, call RPC, sign, or
submit. Repeating the preparer is a journal-authenticated producer resume, not
a newly synthesized authority.

This rung does not itself sign, generate keys, call RPC, fund, or invoke the
child supervisor. Direct-session production is a separately accepted,
key-free successor preparation boundary; the finite plan consumes only its
Finalized journal. A one-command live ongoing campaign still requires the
accepted launcher to connect these journal transitions to the existing
single-cycle child ABI. Until that handoff exists, the correct live-readiness
statement is “finite signed plan and recovery contract accepted; live ongoing
dispatch not yet connected,” not “ongoing devnet activity ready.”

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
- Direct and terminal completions can expose their whole child-owned mutation
  journal as a transaction list. The completion spec names relative label and
  signature pointers and must set `requireAllTransactionsSuccessful: true`;
  reconciliation then captures every listed signature rather than only the
  final Hot or aggregate-retirement transaction.
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
  `{id, covers, caller, argv, dependsOn, wallets, mutation, completion}` or that
  exact object plus `progressive` for the two stepwise successor callers.
  `caller` is `dclutch-cli` or `successor`; `argv` omits the executable.
  Completion is `{path, schema, signaturePointers, transactionListPointer,
  requiredTransactionLabels, requiredValues}`. Simple receipts set the two
  transaction-list fields to `null` and `[]`. A successor campaign sets
  `signaturePointers: []`, `transactionListPointer:
  /execution/transactions`, and lists every stage whose finalized transaction
  is required for semantic completion. Its exact `checked-release` and
  `market` manifest inputs must also be the campaign's `--plan` and `--market`,
  and its completion path must be the campaign's `--evidence` path.
- `progressive` is exactly `{maxSteps, sourceInput, sessionInput, marketInput}`.
  All three ids name immutable manifest inputs. Direct completion must join its
  public-manifest and private-session SHA-256; terminal completion must join its
  session SHA-256 and exact plan/session/Market-input paths.
- A non-campaign transaction-list completion additionally names
  `transactionLabelPointer`, `transactionSignaturePointer`, and
  `requireAllTransactionsSuccessful`. Direct uses `/mutations` with `/kind`
  and `/signature`; terminal uses `/journals` with `/mutation/kind` and
  `/signature`.

Templates are limited to `{{rpc}}`, `{{devnetGenesis}}`, `{{work}}`,
`{{input.ID}}`, `{{wallet.ID.address}}`, and `{{wallet.ID.keypair}}`.

### Canonical Activity-v3 producer

`produce_v3.py` is the only scenario/manifest producer in this directory. It
requires accepted SHA-256 values for both
`tools/economic-lifecycle-ledger/fixtures/activity-v3-canonical.json` and the
flagship economic operation ensemble. It derives, rather than accepts through
bindings, the ten-wallet partition, the exact `360000000` payer bankroll, four
post-init transfers of `50000000`, and all 25 truthful mutating expectations.
It authenticates the result with the economic ledger before publishing.

The bindings document has schema
`dclutch-devnet-activity-v3-producer-bindings-v1` and exact fields
`schema,target,inputs,addressBindings,adapters,campaignIdentities,permanentAuthorityRef,foundingAdapter`.
It supplies checked deployment/caller/session artifacts and no monetary field.
The producer refuses existing output paths and validates the emitted manifest
with this harness. Production still requires real, externally produced Direct
private sessions; this producer never synthesizes operator authority or reads a
key.

```sh
python3 tools/devnet-activity/produce_v3.py \
  --economic-fixture /absolute/activity-v3-canonical.json \
  --economic-fixture-sha256 HEX64 \
  --base-scenario /absolute/flagship.json \
  --base-scenario-sha256 HEX64 \
  --bindings /absolute/activity-v3-bindings.json \
  --scenario-out /absolute/activity-v3-scenario.json \
  --manifest-out /absolute/activity-v3-manifest.json
```

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
python3 -m unittest -v \
  tools/devnet-activity/test_activity.py \
  tools/devnet-activity/test_ongoing_v1.py

cargo test --manifest-path tools/relayer/Cargo.toml \
  --bin dclutch-verify-ed25519
```
