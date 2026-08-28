# Owned-loopback lifecycle chaos gate

This directory is the failure-injection shell around the accepted private
validator lifecycle. It does not build a Market, construct an instruction,
read a key, decide a retry, or interpret a protocol receipt. Those facts remain
owned by the successor commands and their journals.

The gate runs one clean baseline plus sixteen hostile cases:

- process death immediately after durable `Submitted` at founding,
  participant admission, ALT creation, capability seal, Direct Hot execution,
  Resolution, wallet payout, and retirement;
- a lost RPC response, an exact injected duplicate `sendTransaction`, and a
  blockhash-expiry refusal at Hot;
- corrupted and atomically replaced evidence, wallet underfund and surplus,
  and a late child refusal.

Every case starts from the same owned-loopback fixture. Successful recovery
must end with the exact baseline account bytes and lamports. A refused case
must preserve its independently observed pre-fault snapshot byte-for-byte.
The RPC trace rejects a second client `sendTransaction` for one frozen journal
intent; the deliberately duplicated request is marked as supervisor-injected,
so it cannot disguise a caller retry.

## Driver projection

`lifecycle_chaos.py` consumes a strict `dclutch-lifecycle-chaos-spec-v1` JSON
document. The spec must sit beside a non-symlink `fixture/` directory and name:

- one opaque session command and one read-only observer command;
- literal `owned-loopback`, the exact source commit, and an optional literal
  `http://127.0.0.1:PORT` upstream;
- relative session, journal, canonical evidence, and replacement-evidence
  paths;
- the exact ordered boundaries `founding`, `participant`, `alt`, `seal`,
  `hot`, `resolution`, `payout`, `retire`.

The commands may use `{case}`, `{caseWork}`, and `{rpcUrl}` placeholders. The
supervisor supplies only three environment values:

```text
DCLUTCH_LIFECYCLE_CHAOS_CONTROL=/absolute/case/control
DCLUTCH_LIFECYCLE_CHAOS_CASE=<named-case>
DCLUTCH_LIFECYCLE_CHAOS_RPC_URL=http://127.0.0.1:PORT
```

The opaque driver creates `control/PREPARED.json` with exactly:

```json
{"schema":"dclutch-lifecycle-chaos-control-v1","state":"prepared"}
```

and waits for `control/GO.json`. This lets the supervisor take a canonical
pre-fault observation and corrupt only copied evidence before any transaction.
Local wallet and late-child hostiles are requested through an fsynced
`control/FAULT.json`; the accepted driver owns their construction and never
exports a private key or packet.

Each durable boundary is projected as `<journalDir>/<stage>.json`:

```json
{
  "schema": "dclutch-lifecycle-chaos-stage-projection-v1",
  "stage": "hot",
  "phase": "submitted",
  "intentSha256": "64 lowercase hex digits"
}
```

The projection is not a second journal. It is a four-field view of the
semantic owner's already-fsynced journal. The driver must never advance it to
`submitted` before its owner has durably persisted that phase.

The observer prints sorted exact account rows with raw data, its SHA-256,
owner, executable flag, and lamports under
`dclutch-lifecycle-chaos-snapshot-v1`. This gate recomputes every digest and
the account-count/lamport totals before comparison.

## Run

After the private-validator full-session command and projections are frozen:

```sh
python3 tools/release/lifecycle-chaos/lifecycle_chaos.py \
  --spec /absolute/accepted/spec.json \
  --work /absolute/fresh/chaos-run
```

`SUMMARY.json` is terminal only when all seventeen named cases pass. This is
local-validator evidence, never devnet or mainnet evidence.

The isolated supervisor tests use `fake_session.py`; that file is a process and
RPC fault target, not a protocol simulator or release artifact:

```sh
python3 -m unittest tools/release/lifecycle-chaos/test_lifecycle_chaos.py
```
