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
immutable Loader-v3 Program accounts and canonical fixed-45-byte ProgramData
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
accepted immutable release observation. These remain executable transactions:

1. Core initialization of the sole Registry/Rent infrastructure profile.
2. Loader-v3 revocation of Core's ephemeral authority to `None`, followed by
   Registry activation of the five-role immutable release set.
3. RentCredit creation through the selected Rent program.
4. Canonical 31-account Found.
5. Core-owned Source creation/funding and Resolution consumption of the real
   locally posted Pyth update.

Core commit `d6d5f2d` exposes verifier-clean infrastructure-init and Found31
instructions. The remaining executable seam is process ownership: one
ephemeral Core upgrade-authority key must remain only in memory across validator
start, init, Loader revocation, immutable release activation, and Found. The
standalone `run` command does not yet own that process lifetime, so it validates
the full plan and provider evidence, then fails before opening an RPC connection
or signing anything. This is an intentional gate, not lifecycle evidence.

## Safety boundary

- RPC input must be an exact numeric loopback HTTP origin.
- No Solana CLI config, wallet, browser session, or public RPC is read.
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
observation supplied by the future same-process supervisor; this utility never
persists its corresponding private key.

## Launch substrate only

`tools/local-validator/dclutch-successor-validator` verifies all artifact
attestations, the plan, and every account JSON hash. Its foreground mode is the
process boundary intended for a supervisor that retains the ephemeral Core
authority in memory; starting this substrate is not a claim that init,
revocation, Found, or Source resolution ran.

The later joined command shape is reserved as:

```text
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline -- run \
  --rpc-url http://127.0.0.1:20890/ \
  --plan /absolute/plan.json \
  --provider-evidence /absolute/provider-evidence.json \
  --output /absolute/new/evidence.json
```

Until the same-process supervisor is wired, this exits with the machine plan's
exact blocker and creates no output evidence file.

## Local checks

```text
cargo fmt --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml -- --check
cargo test --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline
cargo clippy --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --all-targets --offline -- -D warnings
bash -n tools/local-validator/dclutch-successor-validator
```
