# Live read-only chain server

`operatord chain-serve` is the chain-attached owner of Glass static assets, the
fork-aware untrusted index API, and the pure payoff compiler. It has no wallet,
signer, blockhash, transaction submission, faucet, deployment, account
creation, or persistence path.

```text
operatord chain-serve --config chain.json [--port 9130] [--static apps/static-client]
```

The config is explicit and closed. Every integer is a canonical decimal string;
identities are either Solana base58 addresses/hashes or lowercase 32-byte hex as
named. No field has an inferred network or release default.

```json
{
  "schema": "dragons-clutch/operatord-chain-config/v1",
  "cluster": {
    "name": "localnet-or-devnet-label",
    "genesisHash": "<exact Solana genesis hash>",
    "rpcHttpUrl": "http://127.0.0.1:8899",
    "rpcWebsocketUrl": "ws://127.0.0.1:8900"
  },
  "releases": [
    {
      "programId": "<program address>",
      "programData": "<linked ProgramData address>",
      "elfSha256": "<lowercase SHA-256 of ProgramData bytes after the 45-byte loader metadata>",
      "deploymentSlot": "1",
      "families": ["general", "source", "series", "fees", "liveness", "position-v3", "replay-v3", "structured-claim", "dealer", "failure"]
    }
  ],
  "sourceNeutralSink": "<explicit Source runtime neutral-sink address>",
  "workflowId": "<nonzero lowercase 32-byte workflow identity>",
  "maximumKeeperActions": "256",
  "bounds": {
    "maximumAccountsPerScan": "4096",
    "maximumAccountDataBytes": "1048576",
    "maximumTotalResponseBytes": "16777216",
    "maximumSubscriptions": "64",
    "maximumAddresses": "4096",
    "maximumVersionsPerAddress": "8",
    "maximumForkNodes": "4096"
  },
  "pollingIntervalMilliseconds": "5000",
  "rpcTimeoutSeconds": "30",
  "websocketReconnectInitialMilliseconds": "250",
  "websocketReconnectMaximumMilliseconds": "30000",
  "compilerReleaseSha256": "<configured pure compiler build/release SHA-256>"
}
```

The values above illustrate widths and shape, not a shipped network, program,
release, wallet, or source fixture. Replace every placeholder and review every
bound. Decoder family names are explicit operator configuration assertions about
the selected release;
the hostile decoder refuses unknown and ambiguous account bodies.

## What is checked before serving

The process makes bounded read-only calls to the explicitly configured,
untrusted RPC and refuses to bind Glass unless that one endpoint reports:

1. `getGenesisHash` equals `cluster.genesisHash`;
2. each Program and ProgramData account exists at finalized commitment;
3. the Program is executable, both accounts are owned by the pinned
   Upgradeable Loader, and the Program body links the exact ProgramData address;
4. ProgramData decodes canonically and names the configured deployment slot;
5. SHA-256 of the exact ELF suffix equals `elfSha256`.

It checks the same coordinates again after each complete scan and only then
exposes `SharedIndexApi`, so an observation that changes across the scan keeps
the projection withdrawn. These checks are consistency observations, not
cryptographic chain authentication or an RPC quorum. The configured decoder-family
list is not derived from the ELF and must not be treated as release-manifest
proof. Every account response is still an untrusted projection:
onchain execution must reload complete authoritative accounts.

## Processed finality and rollback boundary

Finalized state comes from repeated `getProgramAccounts` scans. One ordered
WebSocket reader separately owns exactly one `programSubscribe` per configured
release plus `blockSubscribe`, `slotsUpdatesSubscribe`, and `rootSubscribe`.
Before a processed generation becomes readable it:

1. opens the exact configured WebSocket URL with no redirect or proxy fallback;
2. binds every server-assigned subscription ID to its planned request;
3. buffers notifications under the configured count and aggregate-byte bounds;
4. runs a serialized HTTP release-check → finalized scan → release-check cycle;
5. replays the buffered notifications in wire order; and
6. marks the generation live only after the complete replay succeeds.

Program rows remain buffered until their slot has a unique blockhash and is
frozen. A dead slot withdraws buffered rows and already-indexed descendants.
Each new root must be non-regressing, uniquely block-identified, free of known
dead ancestry, and descend from the previous observed root. Root arrival drains
same-slot pending rows. Capacity exhaustion, ambiguous topology, closure/owner
change, malformed input, release mismatch, idle timeout, or transport loss
withdraws the entire generation—including processed versions, fork nodes,
pending rows/root, and connection-scoped subscription IDs—and reconnects with
an explicitly configured exponentially increasing, capped backoff.

Processed output is always labeled non-final and rollbackable. It has
`authorityEligibility: false`; the processed keeper endpoint returns no actions,
and Glass refuses workflow construction until the user selects and reacquires a
finalized projection. The common read gate is withdrawn during each release-
bracketed finalized scan; otherwise the last complete finalized scan remains
available while processed transport is in backoff, registration, or replay.

`/v1/acquisition` echoes the exact HTTP URL, WebSocket URL, genesis binding,
and every configured Program/ProgramData/deployment-slot/ELF coordinate. Glass
requires exact equality with its immutable selected configuration before it
accepts any account response. For processed reads it brackets its bounded GET
sequence with acquisition-state reads and rejects a changed connection
generation or rollback epoch. The echoed verification disposition means only
that the read gate follows the last complete untrusted HTTP release bracket; it
is explicitly not authority eligibility or cryptographic chain authentication.

The HTTP genesis/release checks do not cryptographically prove that a distinct
WebSocket service is operated by the same backend. The exact URL join prevents
silent target substitution inside Glass; all WebSocket observations remain
untrusted and onchain execution must independently reload authority.

The RPC plan accepts HTTPS/WSS endpoints or canonical loopback
`http://127.0.0.1:PORT` and `ws://127.0.0.1:PORT` pairs. Public reads remain
subject to configured response, account, timeout, index, notification-buffer,
polling, and reconnect bounds. The HTTP worker invokes `curl` only with read-only
JSON-RPC bodies, no redirects, an explicit protocol allowlist, proxy variables
disabled, and response limits. The WebSocket owner refuses binary/batch JSON,
caps raw messages and fragmented aggregates before JSON admission, and applies
connect, read-idle, and write deadlines. Hostname resolution occurs through the
host OS resolver before bounded TCP address attempts; `std::net` does not expose
a per-lookup DNS deadline, so resolver configuration remains a named tooling
boundary. An IP-literal loopback endpoint avoids that boundary for local use.
RPC operators must also enable `blockSubscribe` and `slotsUpdatesSubscribe`;
an absent or refused required subscription withdraws the generation and enters
the configured backoff rather than silently degrading topology.
