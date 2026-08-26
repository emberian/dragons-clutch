# Successor immutable-infrastructure bootstrap

This standalone localhost utility prepares the exact immutable substrate for
the current multi-program successor and then drives the first market lifecycle
through a real local validator. It does not seed mutable protocol state or call
a legacy direct Resolution ABI.

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

Only Loader accounts and the infrastructure release records required to start
the successor are prepared as genesis fixtures. Core's genesis ProgramData is
explicitly pre-init and not an accepted immutable release observation. The
supervisor executes the remaining infrastructure and market boundary as real
localhost transactions:

1. Core initialization of the sole Registry/Rent infrastructure profile.
2. Loader-v3 revocation of Core's ephemeral authority to `None`, followed by
   Registry activation of the five-role immutable release set.

Loader-v3 owns authority presence with the tag at byte 12. Its real
`Some -> None` serialization leaves bytes 13..45 as inactive storage rather
than clearing the former key; the ELF still begins at byte 45. The runner pins
and verifies that exact retained-byte poststate, while Registry never exposes
inactive bytes as an authority.

3. Creation of a real Token-2022 collateral Mint and wallet, preserving raw
   `u64` atoms and treating the full `u8` decimals field as display metadata.
4. Bounded Registry `Begin -> Append -> Finalize` publication of the Realm,
   Runtime-V2 Product graph, Source material, recovery policy, and capability
   manifest. The Product root, result domain, and portfolio are compiled and
   published through one chain-derived graph state machine.
5. One same-slot pre-credit projection of the canonical Market and
   `Market+generation` lifecycle-rent PDA, followed by RentCreditV2 creation
   and finalized reacquisition.
6. Publication of a finalized address lookup table covering the Found frame,
   and submission of Found31 as a packet-safe v0 transaction. With its keys
   inline the canonical 31-account frame serialises to 1,242 raw bytes against
   Solana's 1,232-byte legacy limit — it misses by ten. Routing is table data,
   never protocol authority: only
   non-signer coordinates and the invoked Program are routed, the fee payer and
   every signer stay in the message's static key list, and the table is
   authority-owned rather than frozen so its rent stays recoverable.
7. Canonical Core Found31 creation from the post-credit snapshot.

It emits finalized transaction metadata, exact poststate account hashes, and
hostile observations for wrong infrastructure authority, pre-revocation
activation, late atomic rollback, substituted Registry refund wallet, a
substituted lifecycle credit in Found31, and a substituted Market coordinate
under attacker-chosen routing whose whole multi-instruction transaction must
roll back to a fee-only debit.

The runner creates every signing keypair in process memory, gives `prepare`
only the Core authority public key, and retains no private key on disk. The run
spec contains semantic market inputs—not account addresses or caller-authored
digests. The Rust compiler and chain-derived operators own every record digest,
PDA, instruction frame, and next publication action.

## The current stopping point

Measured on a real localhost validator on 2026-08-26, the campaign reaches
Found31 and **Found31 does not execute**: it exhausts Solana's per-transaction
maximum of 1,400,000 compute units. Worse, when the real
Claims/Trading/Resolution/Custody artifacts are supplied through
`DCLUTCH_SUCCESSOR_*_ELF`, the five-role Registry activation exhausts the same
maximum before the campaign reaches any Market work at all; the run therefore
binds those four roles to distinct immutable deployments of the much smaller
Registry ELF. Release authentication hashes whole
ProgramData ELFs on chain -- the ~1.0 MB Core ELF twice in the same
transaction, once inside Core and once inside the Registry role CPI -- and that
hashing alone is roughly 1.19M CU. The compute limit is already the protocol
maximum, so there is no headroom to buy. No Market is created and the Market is
not Open. The full decomposition, and the two defects that had to be fixed
before the campaign could get this far, are in
`docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md`.

Beyond that ceiling, the atomic generic founding outer (`DCLTGMF1`,
`programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs`) -- which has
the correct Lock -> Found/permit -> Realize -> Claims FoundingV5 -> Core
Open-last order in one rollback domain -- remains unreachable for two
structural reasons recorded with file and line in the same document:

- a Trading capability root can only be activated against a Core Market that
  already exists, and generic founding needs one for the Market it is about to
  create; and
- no live Trading route emits the projected-Custody `Initialize`/`OpenHoard`
  requests whose resulting state the outer's Lock stage consumes.

A third defect on that path -- the capability-root selection was a SHA-256
fixed point and so unsatisfiable for every well-formed artifact -- was fixed at
its semantic owner in `386f254`.

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

## Run the infrastructure and Found31 campaign

`tools/local-validator/dclutch-successor-validator` verifies all artifact
attestations, the generated plan, and every account JSON hash. Write a run spec
whose seven program objects each contain `program_id`, absolute `elf_path`,
`elf_sha256`, `semantic_release_id`, and absolute `attestation`:

```json
{
  "schema": "dclutch-local-successor-run-spec-v2",
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
  "rent_credit": { "program_id": "...", "elf_path": "/absolute/rent.so", "elf_sha256": "...", "semantic_release_id": "...", "attestation": "/absolute/rent-attestation.json" },
  "market": {
    "generation": 1,
    "collateral_display_decimals": 9,
    "initial_collateral_atoms": 1000000000,
    "product_id": "32-byte-lowercase-hex",
    "coordinate_domain_id": "32-byte-lowercase-hex",
    "result_unit_id": "32-byte-lowercase-hex",
    "claim_basis_id": "32-byte-lowercase-hex",
    "liability_basis_id": "32-byte-lowercase-hex",
    "representation_release_id": "32-byte-lowercase-hex",
    "mapping_release_id": "32-byte-lowercase-hex",
    "cut_denominator": 1,
    "cuts": ["0"],
    "portfolio_denominator": 1,
    "coefficients": [0, 1, 0],
    "primary_source_spec_id": "32-byte-lowercase-hex",
    "window_spec_id": "32-byte-lowercase-hex",
    "statistic_spec_id": "32-byte-lowercase-hex",
    "failure_policy_release_id": "32-byte-lowercase-hex",
    "recovery_policy_hex": "canonical-RecoveryPolicyV2-bytes-as-lowercase-hex",
    "capability_manifest_hex": "canonical-CapabilityManifestV1-bytes-as-lowercase-hex"
  }
}
```

Then execute:

```text
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline -- run \
  --spec /absolute/successor-run-spec.json
```

### Generating the `market` object

Do not hand-write the `market` object; the example above is explanatory and
will not parse. Generate the canonical local demo Market -- a small categorical
SOL/USD range-protection Product over the captured synthetic-local Pyth release
in `fixtures/pyth/local-upgraded-2026-08-22` -- with:

```text
cargo run --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline -- \
  demo-market --registry-program-id REGISTRY_ID
```

Its coordinate domain is USD cents per SOL with `cut_denominator` 100 and cuts
at `12000` and `18000`, so the three ordinary regions are below $120.00, inside
$120.00-$180.00, and at or above $180.00, followed by the explicit failure
outcome. The portfolio coefficients `[1, 0, 1, 0]` pay one unit of the
liability basis in either tail and nothing inside the band or on failure. Every
semantic identifier is a domain-separated digest naming the spec it stands for,
and the resolution identifiers bind the captured release's adapter identity.
That release is a **local lab projection** documented in
`docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md`, never a production provider
release, and this Market is a lab demo, not a product offering.

The command stops its child before returning but does not delete the ledger.
The evidence file contains the public ephemeral authority, never its private
bytes, every finalized Registry/Rent/Core poststate, and the exact remaining
atomic-open seam. Placeholder text in the example is explanatory and will not
parse; produce the semantic IDs, RecoveryPolicyV2, and CapabilityManifestV1
through the canonical Rust tooling used by the intended release.

## Local checks

```text
cargo fmt --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml -- --check
cargo test --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --offline
cargo clippy --manifest-path tools/local-validator/bootstrap/successor/Cargo.toml --all-targets --offline -- -D warnings
bash -n tools/local-validator/dclutch-successor-validator
```
