# Local successor bootstrap

This standalone host package composes the small Registry and Resolution SBF
successors with the real local Pyth router/receiver bootstrap. It never reads a
Solana CLI configuration, wallet, browser session, or public RPC endpoint.

The `prepare` command emits a fresh `--account-dir` and a machine-readable plan.
Those files are explicitly **genesis fixtures**, not transactions: the current
Registry can activate finalized records but cannot publish them, and the current
Core/Resolution split does not yet create Market, Source-state, or funding
accounts. The runner refuses any plan that hides that boundary.

The `run` command accepts only an explicit loopback HTTP RPC URL and the evidence
file produced by the real provider bootstrap. It observes Loader V3 facts from
chain, activates and reauthenticates the Registry release set, consumes the real
posted 134-byte Pyth `PriceUpdate`, and submits sequential funded Source actions.
It records transaction logs, compute units, exact account hashes, and a hostile
rollback snapshot. It does not claim a checked production release, captured
deployment identity, or on-chain creation for genesis-prepared state.

The fixture semantics were reconstructed from the public contract constructors
and the compiled-SBF successor test. This package does not import the harness and
does not substitute native processors. Local validator Loader headers, slots,
and clock remain a separately named runtime boundary.

## Reproducible sequence

Build this package in its standalone workspace. Its lock and target directory do
not mutate the root workspace.

```sh
cargo build \
  --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml \
  --locked --offline
```

Choose explicit local Registry and Resolution program IDs. They are inputs to a
local campaign, not canonical deployment authority. Build both ELFs from one
exact `git archive`, retain the build logs, and create the launcher's two
attestation JSON files. Each attestation binds the full source commit, source
archive SHA-256, ELF path and SHA-256, selected program ID, exact SBF tool
versions and build command, build-log SHA-256, and
`verifier: {"status":"clean","diagnostic_count":0}`.

Prepare the immutable Loader V3 accounts and protocol genesis inputs:

```sh
cargo run \
  --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml \
  --locked --offline -- prepare \
  --account-dir /absolute/new/account-dir \
  --output /absolute/new/plan.json \
  --registry-program-id REGISTRY_PROGRAM_ID \
  --registry-elf /absolute/path/dclutch_registry_sbf.so \
  --registry-sha256 REGISTRY_ELF_SHA256 \
  --resolution-program-id RESOLUTION_PROGRAM_ID \
  --resolution-elf /absolute/path/dclutch_resolution_proof_sbf.so \
  --resolution-sha256 RESOLUTION_ELF_SHA256
```

Start a fresh successor profile. The launcher verifies the plan, every genesis
JSON hash, both attestations, both ELFs, and all committed Pyth fixture pins
before starting the validator.

```sh
tools/local-validator/dclutch-successor-validator start \
  --ledger /absolute/new/ledger \
  --account-dir /absolute/new/account-dir \
  --plan /absolute/new/plan.json \
  --registry-program-id REGISTRY_PROGRAM_ID \
  --registry-elf /absolute/path/dclutch_registry_sbf.so \
  --registry-sha256 REGISTRY_ELF_SHA256 \
  --registry-attestation /absolute/path/registry-attestation.json \
  --resolution-program-id RESOLUTION_PROGRAM_ID \
  --resolution-elf /absolute/path/dclutch_resolution_proof_sbf.so \
  --resolution-sha256 RESOLUTION_ELF_SHA256 \
  --resolution-attestation /absolute/path/resolution-attestation.json
```

The profile is fixed at `http://127.0.0.1:20890`. First run the existing real
provider bootstrap without `--reclaim`, then compose the successor lifecycle:

```sh
cargo run --manifest-path tools/local-validator/bootstrap/Cargo.toml \
  --locked --offline -- \
  --rpc-url http://127.0.0.1:20890 \
  --evidence /absolute/new/provider-evidence.json

cargo run \
  --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml \
  --locked --offline -- run \
  --rpc-url http://127.0.0.1:20890 \
  --plan /absolute/new/plan.json \
  --provider-evidence /absolute/new/provider-evidence.json \
  --output /absolute/new/successor-evidence.json
```

All payer, worker, VAA, and PriceUpdate signing identities are generated in
process memory. The clients reject non-loopback RPC origins, proxies, redirects,
reclaimed provider evidence, mismatched provider accounts, and output paths that
already exist.

## Exact semantic boundaries

Registry and Resolution are installed as complete immutable Loader V3 genesis
accounts. Their ProgramData layout is variant `3`, slot `0`, authority `None`,
canonical zero padding through byte `44`, and ELF beginning at the fixed byte
offset `45`. The runtime re-reads the Program and ProgramData accounts from RPC
and verifies their PDA linkage, owners, headers, authority, ELF hashes, and full
account hashes before submitting protocol instructions.

The signed Pyth fixture has a historical publish time. The primary Source
material therefore uses a checked local policy whose maximum age is the observed
clock delta plus 900 seconds. The funded recovery cases use a distinct Source
material with maximum age one second, so their primary and recovery deadlines
are genuinely expired. The plan and runner reject conflating these two immutable
materials. No oracle, clock, loader, feed, exponent, confidence, funding, or
certificate check is bypassed.

An Active funding account holds exactly its account rent floor plus the bounty:
activation has already moved the quote's Rent compartment from `remaining` to
`released`. The larger two-rent-plus-bounty observation belongs only to the
pre-activation Pending boundary.

The campaign executes Registry activation and reauthentication, primary Pyth
resolution, then recovery, exhaustion, and explicit failure. A final attempt
preoccupies the failure-certificate PDA and must refuse atomically; evidence
requires exact Source, certificate, funding, and worker hashes before and after
that failed transaction.

## Validated immutable checkpoint

The first immutable campaign used exact `git archive` commit
`30dc6cbb2929de00ffd41cd1a720e9390f3a94fe` with source-archive SHA-256
`21e5660e3250b7d29026a35cd53ef71ce8a883c7a67093a1cfd31a973f0194f9`.
Its verifier-clean Registry ELF SHA-256 is
`b7d6634a23de84cb1b1f0a3368493b9008d88278c460f90e26b522af5e9a6e39`
(89,760 bytes); its Resolution ELF SHA-256 is
`a1b75d4093d688cea61456f5d2124cd7e9e1f95b010259d0ac61509811dde6d8`
(210,528 bytes). Both were built with `cargo-build-sbf 4.0.0`, platform-tools
v1.53, and SBF rustc 1.89.0, with zero verifier diagnostics.

That localhost run consumed 619,265 CU for Registry activation, 127,044 for
reauthentication, 224,287 for primary Pyth resolution, 283,229 for recovery,
285,940 for exhaustion, and 294,399 for explicit failure. The rollback lineage
used 286,229 and 284,440 CU; its deliberately occupied final certificate refused
with custom error `2` after 278,204 CU and preserved every pinned account hash.
These figures describe that exact local-validator campaign only; they are not a
mainnet benchmark or a checked production release.
