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

## Current finality boundary

This transport repeatedly polls finalized `getProgramAccounts`. It does not
pretend polling supplies a processed fork graph. Requests with
`commitment=processed` return a conflict until a transport owns and admits the
complete program, block, slot-update, and root WebSocket subscription set.
Finalized account rows retain their observed scan slots; `/v1/forks` remains
empty rather than fabricating block identities.

The RPC plan accepts HTTPS endpoints or canonical `http://127.0.0.1:PORT` for a
local validator. Public reads remain subject to the configured response,
account, timeout, index, and polling bounds. The worker invokes `curl` only with
read-only JSON-RPC bodies, no redirects, an explicit protocol allowlist, proxy
variables disabled, and response limits.
