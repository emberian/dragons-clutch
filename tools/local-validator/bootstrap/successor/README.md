# Successor immutable-infrastructure bootstrap

This standalone localhost utility prepares the exact immutable substrate for
the current multi-program successor. It does not manufacture a partial Market
or call a legacy direct Resolution ABI.

The prepared plan binds seven pairwise-distinct, real SBF artifacts:

- Registry
- Core
- Claims
- Trading
- Resolution
- Custody
- RentCredit

Registry, Claims, Trading, Resolution, Custody, and Rent are represented by
immutable Loader-v3 Program accounts and exact fixed-offset ProgramData
headers followed by the exact ELFs. Core begins with the same exact ELF and a
single ephemeral upgrade authority, then must reach that immutable header by
Loader revocation before release recognition. The plan also creates distinct
`ArtifactReleaseV1` bodies, the five-role
`ExecutionReleaseSetV1`, the captured local-Pyth release body, and the expected
144-byte `ProtocolInfrastructureProfileV1` body selecting Registry and Rent.
The profile itself is not genesis-injected: its sole PDA is derived under Core
and must be created by the canonical initialization transaction.

## Evidence boundary

Only Loader accounts and finalized Registry record bodies are prepared as
genesis fixtures. Core's genesis ProgramData is explicitly pre-init and not an
accepted immutable release observation. The supervisor now executes the
infrastructure boundary as real localhost transactions:

1. Core initialization of the sole Registry/Rent infrastructure profile.
2. Loader-v3 revocation of Core's ephemeral authority to `None`, followed by
   Registry activation of the five-role immutable release set.

Loader-v3 owns authority presence with the tag at byte 12. Its real
`Some -> None` serialization leaves bytes 13..45 as inactive storage rather
than clearing the former key; the ELF still begins at byte 45. The runner pins
and verifies that exact retained-byte poststate, while Registry never exposes
inactive bytes as an authority.

It emits finalized transaction metadata, exact poststate account hashes, and
three hostile observations: a wrong authority cannot initialize, activation
cannot recognize Core before revocation, and a late substituted ProgramData
rolls back an earlier instruction in the same transaction.

The following remain market-specific transactions:

3. RentCredit creation through the selected Rent program.
4. Canonical 31-account Found.
5. Core-owned Source creation/funding and Resolution consumption of the real
   locally posted Pyth update.

The run spec intentionally contains no authority key and no market DTO. The
runner creates one Core keypair in memory, gives `prepare` only its public key,
retains it across validator start/init/revocation, then drops it when the
guarded child is stopped. RentCreditV2 and Found31 require finalized Realm,
ProductV3 basis/result-domain, portfolio, resolution, execution-manifest, and
lifecycle-policy record pairs plus exact generation, refund wallet, Hoard
principal, and lifecycle-rent funding. The evidence names this seam instead of
inventing a partial Market.

## Safety boundary

- RPC input must be an exact numeric loopback HTTP origin.
- No Solana CLI config, wallet, browser session, or public RPC is read.
- Run-spec objects refuse unknown fields, including any private-key field.
- Output/account/plan/ledger paths must be fresh, absolute, and canonical.
- The foreground validator is killed and reaped on success or failure; its
  ledger and separate supervisor log are retained.
- Semantic release IDs are mandatory checked inputs; the tool does not invent
  a release identity from an ELF hash.
- All seven program IDs, all seven artifact IDs, and Registry/Rent bindings
  must be pairwise distinct where the protocol requires distinctness.
- The historical Pyth fixture is a local lab projection, not a checked
  production release.

## Prepare

Build the seven actual SBF programs from one exact committed source archive and
retain clean verifier attestations. Then run:

```text
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline -- prepare \
  --account-dir /absolute/new/accounts \
  --output /absolute/new/plan.json \
  --registry-program-id REGISTRY_ID \
  --registry-elf /absolute/dclutch_registry_sbf.so \
  --registry-sha256 REGISTRY_ELF_SHA256 \
  --registry-semantic-release-id REGISTRY_SEMANTIC_RELEASE_ID \
  --core-program-id CORE_ID \
  --core-elf /absolute/dclutch_core_sbf.so \
  --core-sha256 CORE_ELF_SHA256 \
  --core-semantic-release-id CORE_SEMANTIC_RELEASE_ID \
  --core-bootstrap-upgrade-authority IN_MEMORY_AUTHORITY_PUBKEY \
  --claims-program-id CLAIMS_ID \
  --claims-elf /absolute/dclutch_claims_sbf.so \
  --claims-sha256 CLAIMS_ELF_SHA256 \
  --claims-semantic-release-id CLAIMS_SEMANTIC_RELEASE_ID \
  --trading-program-id TRADING_ID \
  --trading-elf /absolute/dclutch_trading_sbf.so \
  --trading-sha256 TRADING_ELF_SHA256 \
  --trading-semantic-release-id TRADING_SEMANTIC_RELEASE_ID \
  --resolution-program-id RESOLUTION_ID \
  --resolution-elf /absolute/dclutch_resolution_proof_sbf.so \
  --resolution-sha256 RESOLUTION_ELF_SHA256 \
  --resolution-semantic-release-id RESOLUTION_SEMANTIC_RELEASE_ID \
  --custody-program-id CUSTODY_ID \
  --custody-elf /absolute/dclutch_custody_sbf.so \
  --custody-sha256 CUSTODY_ELF_SHA256 \
  --custody-semantic-release-id CUSTODY_SEMANTIC_RELEASE_ID \
  --rent-credit-program-id RENT_ID \
  --rent-credit-elf /absolute/dclutch_rent_sbf.so \
  --rent-credit-sha256 RENT_ELF_SHA256 \
  --rent-credit-semantic-release-id RENT_SEMANTIC_RELEASE_ID
```

The command refuses an existing output or account directory, invalid or
duplicate program IDs, non-ELFs, digest mismatches, non-lowercase IDs, a
Resolution semantic ID that does not match the executable contract, and an
aliased Registry/Rent profile. The authority argument is only a public
observation supplied by the same-process `run` supervisor; this utility never
persists its corresponding private key.

## Run the infrastructure campaign

`tools/local-validator/dclutch-successor-validator` verifies all artifact
attestations, the generated plan, and every account JSON hash. Write a run spec
whose seven program objects each contain `program_id`, absolute `elf_path`,
`elf_sha256`, `semantic_release_id`, and absolute `attestation`:

```json
{
  "schema": "dclutch-local-successor-run-spec-v1",
  "rpc_url": "http://127.0.0.1:20890/",
  "launcher": "/absolute/dclutch/tools/local-validator/dclutch-successor-validator",
  "ledger": "/absolute/new/successor-ledger",
  "account_dir": "/absolute/new/successor-accounts",
  "plan": "/absolute/new/successor-plan.json",
  "output": "/absolute/new/successor-evidence.json",
  "registry": { "program_id": "...", "elf_path": "/absolute/registry.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/registry-attestation.json" },
  "core": { "program_id": "...", "elf_path": "/absolute/core.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/core-attestation.json" },
  "claims": { "program_id": "...", "elf_path": "/absolute/claims.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/claims-attestation.json" },
  "trading": { "program_id": "...", "elf_path": "/absolute/trading.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/trading-attestation.json" },
  "resolution": { "program_id": "...", "elf_path": "/absolute/resolution.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/resolution-attestation.json" },
  "custody": { "program_id": "...", "elf_path": "/absolute/custody.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/custody-attestation.json" },
  "rent_credit": { "program_id": "...", "elf_path": "/absolute/rent.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/rent-attestation.json" }
}
```

Then execute:

```text
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline -- run \
  --spec /absolute/successor-run-spec.json
```

The command stops its child before returning but does not delete the ledger.
The evidence file contains the public ephemeral authority, never its private
bytes, and the exact market-specific seam for the next lifecycle campaign.

## Local checks

```text
cargo fmt --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml -- --check
cargo test --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline
cargo clippy --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --all-targets --offline -- -D warnings
bash -n tools/local-validator/dclutch-successor-validator
```
