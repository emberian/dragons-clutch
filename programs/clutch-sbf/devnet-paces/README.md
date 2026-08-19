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

## What each profile proves on devnet

Both profiles run the same real, value-neutral public prefix as confirmed
transactions: Token-2022 collateral mint + token accounts, sealed
policy/grid/Terms artifacts (192-byte chunked uploads), `InitRealm`,
`InitProfile`, and `CreateMarket` allocating the full market plane, all
reloaded and checked.  They then assert the refusal boundaries:

| step | `default` ELF | `mock` ELF |
|---|---|---|
| public `InitSourceSpec` | `Custom(0x0079)` `SourceReleaseUnavailable` (empty registry, before authentication) | `Custom(0x007a)` `SourceAdmissionFailed` (registered spec reaches the deployment authenticator, which refuses the unconstructible provider trio) |
| public `InitSourceArchive` | `Custom(0x007a)` (absent spec fails spec verification) | same |
| `Endow` | `Custom(0x0004)` `WrongProgramOwner` (absent spec fails the state-role gate) | same |

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

# boundary campaign against the deployed NON-PRODUCTION mock-source ELF
target/release/devnet-paces \
  --url https://api.devnet.solana.com \
  --program-id EbWhsDm4BC46zt1iFuAMfh2hgpPQ35nckapP7NAtdrFX \
  --payer ~/jobs/dragons-clutch-devnet-20260819/keys/deployer.json \
  --profile mock \
  --out ~/jobs/dragons-clutch-devnet-20260819/paces-mock
```

The payer needs at least 0.7 SOL (checked at preflight; ~0.6 SOL of that is
headroom).  The actor is funded by faucet airdrop with an automatic
payer-transfer fallback.  Default throttle is 400 ms between RPC calls; raise
it with `--throttle-ms` if the public endpoint rate-limits anyway.  Fresh
secondary keypairs are generated per run and persisted (0600) under
`<out>/keys/`; the transcript is `<out>/transcript.json` and is written on
red runs too.

## Acceptance dry run (local, no funds)

```sh
scripts/run_devnet_paces_dryrun.sh
```

Builds both ELFs, loads them at fresh ids on one blank
`solana-test-validator` (no genesis-injected Clutch or provider accounts —
the same shape devnet presents), runs both profiles green, then requires a
negative control (default expectations against the mock ELF) to go red on the
0x0079/0x007a mismatch.
