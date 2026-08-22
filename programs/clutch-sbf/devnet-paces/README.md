# devnet-paces

Public-devnet paces driver for the Dragon's Clutch market lifecycle.  Runs the
full campaign against a **deployed** program the moment one exists, with fresh
throwaway keys, confirmed commitment, reloaded accounts, and a machine-readable
transcript.

## Claim vocabulary

A green run is **PUBLIC-TESTNET (devnet) execution evidence** for the deployed
ELF at `--program-id`.  It is distinct from local **SBF-EXECUTED** evidence
(`svm-tests`, `scripts/run_committed.sh`) and from **mainnet anything** — this
driver refuses mainnet twice, by URL admission and by genesis hash.
The public URL must be exactly `https://api.devnet.solana.com`, and that
endpoint must return the pinned devnet genesis hash before any account or
balance preflight continues. Loopback rehearsals retain their separate
non-mainnet genesis guard.

## Payer identity

The original key-generation record for the prepared devnet job printed this
binding:

```text
~/jobs/dragons-clutch-devnet-20260819/keys/deployer.json
  -> 4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP
```

Do not fund `4zrxtw5cQbGXnEMWJUJcCJvTuP6GTHM76KTsRRcVW4jn`.  That address
appeared only in later session commands and has no recorded key-generation or
signer-path binding.  Before any public operation, the session owner must
independently derive the public key from the named payer file and compare it
with the address above.  Agents do not read wallet or private-key files.

## What the default profile can prove on devnet

The default profile can run a real, value-neutral public prefix as confirmed
transactions: Token-2022 collateral mint and token accounts, sealed
policy/grid/Terms artifacts (192-byte chunked uploads), `InitRealm`,
`InitProfile`, and `CreateMarket` allocating the full market plane, all
reloaded and checked. It then asserts the honest production-source refusal
boundaries:

| step | expected default-profile result |
|---|---|
| public `InitSourceSpec` | `Custom(0x0079)` `SourceReleaseUnavailable` (empty registry, before authentication) |
| public `InitSourceArchive` | `Custom(0x007a)` (absent spec fails spec verification) |
| `Endow` | `Custom(0x0004)` `WrongProgramOwner` (absent spec fails the state-role gate) |

Every refusal is asserted with instruction-index and code exactness and with
every watched account byte-identical before and after.

## Why the funded mock walk cannot run on devnet

The local mock walk genesis-injects a provider trio at fixed addresses:
`[0xb2;32]` (executable, owner `[0xb3;32]`, body `MOCK-PROVIDER-V1`),
`[0xd4;32]` (owner `[0xd5;32]`, body `DEP1`+generation), `[0xc3;32]`
(owner `[0xb2;32]`, 77-byte record, host-rewritten between appends).  On a
public cluster these are structurally impossible: no private key exists for a
fixed byte-pattern address, executable accounts are created only by BPF-loader
deployment (loader ownership, ELF bytes), and account data can be written only
by its owner program, which is not deployable at those addresses.  Everything
funded (endow → split → materialize → transfer → resolve → redemptions →
withdraw) hangs off the SourceSpec that therefore can never exist.  The full
enumeration, step by step with reasons, is `steps::devnet_impossible` and is
embedded in every transcript under `boundaries`.  A shorter honest walk beats
a faked one.

## Devnet invocation (session owner)

```sh
cd programs/clutch-sbf/devnet-paces && cargo build --release

# fail-closed campaign against the deployed default ELF
target/release/devnet-paces \
  --url https://api.devnet.solana.com \
  --program-id 3SLhMAFm2fXZsqwtTDoDQCQBALBqAEu79N11AySHY2jG \
  --payer ~/jobs/dragons-clutch-devnet-20260819/keys/deployer.json \
  --profile default \
  --out ~/jobs/dragons-clutch-devnet-20260819/paces-default
```

The mock-source ELF is an offline negative-control fixture. It is not a public
deployment target. A real-source public campaign requires authenticated,
deployable provider identities and a production source adapter; the current
default profile refuses honestly until those exist.

The payer needs at least 0.7 SOL (checked at preflight; ~0.6 SOL of that is
headroom).  The actor is funded by faucet airdrop with an automatic
payer-transfer fallback.  Default throttle is 400 ms between RPC calls; raise
it with `--throttle-ms` if the public endpoint rate-limits anyway.  Fresh
secondary keypairs are generated per run and persisted (0600) under
`<out>/keys/`; the transcript is `<out>/transcript.json` and is written on
red runs too.

The local dry-run wrapper removes its payer/program keys and every
paces-generated secondary key on exit while retaining transcripts and logs.
Direct public invocations deliberately retain their output keys for inspection;
their lifecycle must be part of any separately authorized campaign plan.

## Acceptance dry run (local, no funds)

```sh
scripts/run_devnet_paces_dryrun.sh
```

Builds both ELFs, loads them at fresh ids on one blank
`solana-test-validator` (no genesis-injected Clutch or provider accounts —
the same shape devnet presents), runs both profiles green, then requires a
negative control (default expectations against the mock ELF) to go red on the
0x0079/0x007a mismatch.
