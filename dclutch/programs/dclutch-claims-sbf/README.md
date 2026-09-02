# dclutch-claims-sbf

Standalone SBF trust boundary for the canonical runtime-width Claims owner.
It authenticates the sparse logical Core Market, the canonical Claims
aggregate and Position PDAs derived from that logical Market, a Market-selected
Registry, current Core/caller/Claims Loader deployments, the shared release-set
caller PDA authority, and exact optimistic revisions before executing one
`ClaimsPlanV1` basket. Immutable Realm/Product/release content remains owned by
Core and the finalized records; Claims consumes only Core-owned identity,
lifecycle, and manifest references.

The generic frame preserves its original first ten accounts and appends the
cross-owner Core join:

| Index | Account |
| ---: | --- |
| 0 | release-pinned caller authority signer |
| 1 | writable canonical Claims aggregate PDA |
| 2 | writable source Position, or current Claims executable sentinel |
| 3 | writable destination Position, or current Claims executable sentinel |
| 4 | Registry activation cache |
| 5–6 | current caller program and ProgramData |
| 7–8 | current Claims program and ProgramData |
| 9 | immutable Market-selected Registry program |
| 10 | canonical logical Core Market state (`ClaimsPlanV1.market`) |
| 11–12 | current Core program and ProgramData |

The one foundational exception is a named, non-aliasing
`InitializeClaims`/`InitializeCompleteSet` route. It uses the same first 13
accounts plus Rent at index 13 and the System program at index 14. The logical
Core Market must still be `Founding`; the canonical Claims aggregate and
founder Position must be System-owned, zero-data PDAs that already hold at
least their exact runtime-width rent floors. Claims signs only for those two
PDAs, allocates and assigns them, initializes the aggregate directly into
`Open`, mints the equal positive founding complete set, and returns the one
Core effect acknowledgement. Ordinary `SplitClaims` remains exact13 and
`Open`-only. Existing accounts, underfunding, caller signer authority, a
substituted PDA, or an aliased discriminator refuse; excess lamports remain a
donation in the created account rather than becoming economic principal.
The Claims child route and generated Core effect tag are implemented, but the
first isolated Core SBF slice at `c4b8baab` does not yet dispatch
`InitializeClaims`; founding is therefore not claimed as end-to-end physical
evidence from that Core ELF.

Generic Core and Trading callers atomically compose Claims with the canonical
Custody child. Claims returns the exact 256-byte `ClaimsReceiptV1`; the outer
caller must authenticate its producer, request digest, payout, revisions, and
post-resource digest before committing its own state.

`ClaimsPlanV1` and the EconomicSlice state it addresses are migration-only.
The exact remaining producers are
`crates/dclutch-general-adapter-contract/src/child_packets.rs`,
`programs/dclutch-trading-sbf/src/dealer/physical.rs`, and
the deleted `dclutch-dealer-sbf` prototype. No new controller may use that route;
removing those three producers permits deletion of the generic branch and its
EconomicSlice dependency.

Rational Representation V2 instead consumes the canonical runtime-width
LiabilityBasisV2 aggregate and ProtocolPositionV2 accounts. Its operator and
onchain adapter share the SDK-free state layout from
`dclutch-claims-svm::liability_basis_state_v2`; Core alone owns lifecycle and
winner. The obsolete `ActionV1` representation wire, its parallel state
adapter, and its terminal caller harness are not dispatched or built.

## Running the Rational Representation V2 ProgramTest

`run-rational-representation-v2-program-test.sh` builds seven SBF programs plus
the audited Token-2022 v11 fixture and runs
`tests/rational_representation_v2_program_test.rs` against them.

**The claims row printing `DID NOT RUN` in public CI does not mean it cannot
run. It means it did not run *on a CI runner*, and those are different
sentences.** The row needs two things a developer machine usually has and a
GitHub runner never does, so a lane reading the CI output alone will conclude
the row is unreachable and be wrong.

### The two prerequisites, and why CI lacks them

1. **`cargo-build-sbf-<version>.crate` in the cargo registry cache.**
   `fixtures/prepare-token-2022-v11.sh` authenticates the builder itself by
   digest (`cargo_build_sbf_crate_sha256` in
   `fixtures/token-2022-v11.provenance`), looking in
   `${CARGO_HOME:-$HOME/.cargo}/registry/cache` and honouring an explicit
   `CARGO_BUILD_SBF_CRATE`. A CI runner installs Agave from the anza release
   tarball, which is not a cargo download, so nothing there ever populates that
   cache entry. The script exits **2**, not 1, for exactly this: the archive
   being absent is a fact about the host, not a defect in the fixture or in
   Claims, and `tools/ci/run.sh` counts 2 as "did not run" rather than as a
   failing suite. A digest *mismatch* still exits 1, and should.
   Also checked and equally host-local: the platform-tools version manifest at
   `$HOME/.cache/solana/v<platform_tools_version>/platform-tools/version.md`,
   overridable with `SBF_PLATFORM_TOOLS_VERSION_MANIFEST`.

2. **The canonical Token-2022 ELF.** It is host-bound (see
   `fixtures/README.md`): Linux x86_64 reproduces `canonical_elf_sha256`, macOS
   arm64 reproduces a different digest recorded as
   `macos_arm64_audit_elf_sha256`, and only the former is accepted. The outer
   runner therefore exits 2 on any non-Linux-x86_64 host **unless
   `TOKEN_2022_V11_ELF` names a canonical artifact.**

### Running it off the canonical host

`TOKEN_2022_V11_ELF` is the supported escape hatch and it is safe to feed from
another machine, because the fixture builder re-checks the artifact's SHA-256
against `canonical_elf_sha256` and its length against `canonical_elf_bytes`
locally before the test loads it. Copying the file transports no trust.

```sh
# Once: take the canonical artifact from a Linux x86_64 host that has built it.
scp <linux-host>:/path/to/out/spl_token_2022.so /tmp/spl_token_2022_canonical.so
shasum -a 256 /tmp/spl_token_2022_canonical.so   # must equal canonical_elf_sha256

# Then, on any host with the builder archive in its registry cache:
TOKEN_2022_V11_ELF=/tmp/spl_token_2022_canonical.so \
  programs/dclutch-claims-sbf/run-rational-representation-v2-program-test.sh
```

Measured on macOS arm64 on 2026-09-01 with exactly this recipe: the suite
reproduced the canonical Linux corpus row for row. Use it before recording any
Rational representation claim as unverifiable — the difference between "no
evidence" and "no evidence *here*" is the difference between an open question
and a wrong one.
