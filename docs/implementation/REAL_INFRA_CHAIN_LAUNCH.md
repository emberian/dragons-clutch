# Real-infrastructure chain launch

`operatord` now has two deliberately separate release-coordinate paths:

- `launch-local-chain` creates a fresh marked local-validator session, writes
  the v6 public session seal through `SessionLayout`, starts only the exact
  validator binary and ELF files named by the checked launch input, observes
  that validator's loopback genesis hash, composes the v2 chain configuration,
  and serves the chain-attached static client.
- `compose-devnet-chain-config` consumes a canonical, independently recorded
  devnet deployment manifest and emits a chain configuration. It never creates
  or reuses a local session and cannot sign, submit, deploy, request a faucet,
  or mutate devnet.

Neither path reads Solana CLI configuration, a default wallet, browser state,
or any key-like path. The local path does not create a signer. A public mint
address in the launch input is a validator-genesis coordinate, not a key path.

## Local validator

Build the deployable ELF and capability manifest first. The capability checker
must report a completely linked deployable profile. Then create a strict JSON
input with this shape (all paths must be absolute; example values below are
placeholders, not a checked release):

```json
{
  "schema": "dragons-clutch/local-validator-launch-config/v1",
  "session_root": "/absolute/fresh/session-directory",
  "validator": {
    "binary": "/absolute/pinned/solana-test-validator",
    "sha256": "64-lowercase-hex"
  },
  "cluster": {
    "name": "local-validator",
    "expected_genesis_hash": null,
    "rpc_port": "9137",
    "rpc_websocket_port": "9138",
    "faucet_port": "9139",
    "gossip_port": "9200",
    "dynamic_port_start": "9201",
    "dynamic_port_end": "9250"
  },
  "release": {
    "program_id": "BASE58_PROGRAM",
    "program_data": "BASE58_PROGRAM_DATA",
    "deployment_slot": "0",
    "elf_path": "/absolute/build/clutch_sbf.so",
    "compiler_release_sha256": "64-lowercase-hex",
    "source_neutral_sink": "BASE58_ADDRESS"
  },
  "external_programs": [
    {
      "program_id": "BASE58_PROGRAM",
      "program_data": "BASE58_PROGRAM_DATA",
      "deployment_slot": "0",
      "elf_sha256": "64-lowercase-hex",
      "elf_path": "/absolute/capture/program.so"
    }
  ],
  "source": {
    "receiver_program": "BASE58_ADDRESS",
    "receiver_program_data": "BASE58_ADDRESS",
    "receiver_deployment_slot": "0",
    "receiver_config": "BASE58_ADDRESS",
    "receiver_release_sha256": "64-lowercase-hex",
    "parser_program": "BASE58_ADDRESS",
    "parser_program_data": "BASE58_ADDRESS",
    "parser_deployment_slot": "0",
    "parser_config": "BASE58_ADDRESS",
    "parser_release_sha256": "64-lowercase-hex",
    "feed_account": "BASE58_ADDRESS",
    "feed_id": "64-lowercase-hex",
    "transport_program": "BASE58_ADDRESS",
    "transport_program_data": "BASE58_ADDRESS",
    "transport_deployment_slot": "0",
    "transport_release_sha256": "64-lowercase-hex",
    "source_spec_id": "64-lowercase-hex",
    "acquisition": {
      "mode": "pinned-local-capture",
      "capture_manifest_sha256": "64-lowercase-hex",
      "https_rpc_url": null,
      "maximum_account_reads": null
    }
  },
  "mint_authority": "BASE58_PUBLIC_ADDRESS",
  "warp_slot": "1",
  "genesis_accounts": [
    {
      "role": "reviewed source Config",
      "address": "BASE58_ADDRESS",
      "account_json": "/absolute/capture/account.json",
      "body_sha256": "64-lowercase-hex"
    }
  ]
}
```

External programs must appear in increasing program-ID order. Their exact
Program/ProgramData/slot/digest coordinates must close the Source receiver,
parser, and transport tuple. Genesis-account JSON and every executable are
hashed before the session directory is created; every genesis account must be
non-executable state owned by one of those loaded external releases, so this
path cannot inject Clutch-owned protocol state as mock authority. The local
slot fields are exactly the string `"0"`: pinned Agave's `--bpf-program`
genesis path synthesizes each ProgramData body with slot zero. Historical
devnet deployment slots are therefore invalid local coordinates and are never
reported by the local seal or served chain configuration. RPC WebSocket is
exactly `rpc_port + 1`, and every fixed/dynamic port is disjoint.

The launcher accepts at most 16 external programs and 256 genesis accounts.
External ELF bytes are capped at 80 MiB in aggregate, genesis JSON bytes at
64 MiB in aggregate, and all staged inputs at 384 MiB in aggregate, in addition
to the per-file limits. It refuses wallet-, keypair-, seed-, mnemonic-,
secret-, and recovery-material-like paths and non-normal path components.
Every existing input is resolved before any content read and the same refusal
is reapplied to the resolved path, so a benign alias cannot conceal a key-like
target. Symlink file leaves are refused. Resolved validator, ELF, and genesis
paths remain the exact staging/provenance sources; a later resolution change
refuses instead of silently selecting another file. The fresh session root is
lexically normal and every existing ancestor must be a real directory rather
than a symlink, so use its canonical parent path (for example `/private/tmp`
rather than `/tmp` on systems where the latter is an alias).

Preparation starts no validator and performs no RPC operation; it does invoke
the repository's offline capability-profile checker:

```text
operatord prepare-local-chain \
  --config /absolute/local-launch.json \
  --capability-manifest /absolute/checked-profile.json
```

It creates the fresh session, `public-session.txt`, and
`local-launch-plan.json`. The chain configuration remains absent until an
actually started validator returns its loopback genesis hash.

The joined launcher is:

```text
operatord launch-local-chain \
  --config /absolute/local-launch.json \
  --capability-manifest /absolute/checked-profile.json \
  --static /absolute/repository/apps/static-client
```

The launcher starts the exact digest-checked validator, observes only its
configured loopback RPC, and refuses before serving unless the repository's
pinned-runtime verifier and live RPC/WebSocket/faucet/non-loopback listener
probe both pass for the exact child. The staged validator and every staged ELF
are rehashed immediately before process spawn. It then creates
`operatord-chain.json` through the existing capability/profile composer and
enters `chain-serve`. An optional non-null `expected_genesis_hash` is checked
against the observation. Validator logs and the ledger remain under the fresh
mode-0700 session root.

## Devnet deployment manifest

The devnet owner accepts exactly compact JSON in the following field order,
followed by one newline:

```json
{"schema":"dragons-clutch/devnet-deployment-manifest/v1","network":"solana-devnet","genesis_hash":"EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG","rpc_http_url":"https://api.devnet.solana.com","rpc_websocket_url":"wss://api.devnet.solana.com/","release_coordinates":"observed-finalized","program_id":"BASE58_PROGRAM","program_data":"BASE58_PROGRAM_DATA","deployment_slot":"POSITIVE_DECIMAL","elf_sha256":"64-lowercase-hex","capability_manifest_sha256":"64-lowercase-hex","capability_profile_identity":"64-lowercase-hex","source_commit":"40-or-64-lowercase-hex","compiler_release_sha256":"64-lowercase-hex","source_neutral_sink":"BASE58_ADDRESS","signing":"not-exposed","submission":"not-exposed","deployment":"not-exposed"}
```

This manifest is a post-deployment coordinate record. It is not produced by
the local launcher and must never name `local-validator` or the v6 local
session schema. Its deployment slot is a canonical positive value observed
from the finalized public-cluster ProgramData account; zero is refused.
Composition independently checks the deployable capability
manifest and the built ELF, binds the canonical deployment-manifest digest into
the workflow identity, and emits only read-only `chain-serve` configuration:

```text
operatord compose-devnet-chain-config \
  --deployment-manifest /absolute/devnet-deployment.json \
  --capability-manifest /absolute/checked-profile.json \
  --built-elf /absolute/build/clutch_sbf.so > /absolute/devnet-chain.json

operatord chain-serve --config /absolute/devnet-chain.json
```

The second command performs bounded read-only release checks when explicitly
run. Neither command exposes deployment authority, wallet material, signing,
submission, or faucet access.
