# License and provenance policy

Status: applies now to research, documentation, fixtures, generated artifacts,
and future source.

## 1. First-party license

First-party Dragon's Clutch source and documentation are intended to be licensed
under `AGPL-3.0-or-later`, as recorded in the repository [LICENSE](../LICENSE).
Every public release must include the license, corresponding source, build and
installation material required by the license, and appropriate notices.

This file is a project process, not legal advice and not a substitute for a
release-time license review.

## 2. Greenfield boundary

Dragon's Clutch is a separate implementation. Do not copy, import, translate, or
depend on source from JOSHI, joshibot, leanuweave, minidregg, breadstuffs, Oracle
Pit, historical DREGG prototypes, or another local repository without an explicit
current user decision and a recorded provenance/license review.

Prior ideas may inform a human-authored clean-room specification. The record must
distinguish a general concept or public interface from copied expressive source.
Commit history or a local path is not license permission.

### 2.1 Remediation record — 2026-08-22

A repository-wide audit found two violations of that boundary:

- `site/artifacts/manipulation-cost-v1.txt` was byte-identical to
  `degg-research/experiments/manipulation-cost/vectors/v1.txt` at source commit
  `e0af5fe16b64324b1f4c401e8e206d92335206bd` (SHA-256
  `079d121ea18ed254fe4a0d9ce0be9d44785d80f3b36719c4fefcda7e38939c5d`).
- `site/artifacts/manipulation-cost-v2-offhours.txt` was byte-identical to the
  corresponding `degg-research` vector at source commit
  `2afe802f2a983606795d14c158dc45aa4821ad2b` (SHA-256
  `da93104878829f3063eb41089cd0b090b4ad3713655db9810957c4c119766257`).

Both files were copied without the generator, derivation manifest, or an
explicit user decision. They were removed from the current tree; their former
presence remains visible in Git history and must not be mistaken for reusable
Dragon's Clutch material. The public evidence page no longer cites them.

The same audit found exact Breadstuffs Lean declarations in two explanatory
documents despite their “no code moves” boundary. Those declarations were
removed from `docs/design/SUCCINCT_CLEARING_FEASIBILITY.md` and
`docs/implementation/OPTIMALITY_CERTIFICATE_MAPPING.md`; the remaining text
states only source-independent mathematics and high-level comparison.

No prohibited runtime import, Cargo/npm dependency, path dependency, or copied
first-party source file was found. Release remains blocked on a complete
third-party notice bundle, pinned generator environments, full-commit pins for
the Pages actions, and an advisory audit. The Pages workflow is manual-only in
the in-flight review, so these mutable action tags cannot run merely because
someone pushes to `main`.

## 3. Dependency admission

Before adding a dependency, record:

```text
name and purpose
upstream repository/package
exact version and commit/content digest
license and notice obligations
source availability
maintainer/release authenticity
features enabled
transitive dependency lock digest
runtime/proof/build/dev classification
security and reproducibility notes
reviewer and date
```

Proof dependencies need the same review as runtime dependencies. A trusted
specification, axiom, code generator, compiler plugin, or binary tool expands the
evidence boundary even when it is not linked into the SBF ELF.

### 3.1 `sha2` 0.10.9 admission for `clutch-product-series`

Status: **admitted to the offline pure-core dependency lock; not a release
approval or SBF promotion** (reviewed 2026-08-23).

| Required fact | Admission record |
| --- | --- |
| Name and purpose | `sha2` 0.10.9 supplies the SHA-256 implementation used to derive the typed, domain-separated product, basis, recovery-policy, MarketInstance, Series, attachment, and funding identities in `crates/clutch-product-series`. These identities are semantic/consensus inputs, so the hash implementation is a production pure-core dependency rather than a test convenience. |
| Upstream package/repository | crates.io package `sha2`; package metadata names `https://github.com/RustCrypto/hashes`, with `path_in_vcs` equal to `sha2`. Authors are recorded as “RustCrypto Developers.” |
| Exact version and content | Exact requirement `=0.10.9`, registry source `registry+https://github.com/rust-lang/crates.io-index`. Cargo checksum and independently recomputed SHA-256 of the locally cached `sha2-0.10.9.crate` archive are both `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283`. |
| Upstream commit | The archive's Cargo-generated `.cargo_vcs_info.json` records RustCrypto/hashes commit `82c36a428f8d6f05f3bfccdedb243e9d1f85359d` and `path_in_vcs: sha2`. This is package-carried provenance, not an independently signed Git attestation. |
| License and notices | Package metadata declares `MIT OR Apache-2.0` and the archive contains `LICENSE-MIT` and `LICENSE-APACHE`. The MIT file records copyright notices for Graydon Hoare, Mozilla Foundation, and Artyom Pavlov. No separate `NOTICE` file is present in the package. A distribution must retain the selected license text and applicable copyright/notice material; the repository-wide third-party notice bundle and human release-time license review remain outstanding. |
| Source availability | Preferred source is identified by the repository/commit/path above. The exact crates.io source archive and its unpacked Rust source, manifest, licenses, README, changelog, tests, and benches were available in the local Cargo cache during this review. No source was copied into first-party code. |
| Maintainer/release authenticity | Integrity was checked from the cached archive through the crates.io/Cargo checksum recorded in the lock. No maintainer signature, Sigstore record, independent Git checkout, or other release attestation was available or evaluated. The checksum authenticates bytes relative to registry metadata; it does not independently authenticate the publisher. |
| Features enabled | `default-features = false`; no `sha2` feature is enabled. In particular `std`, `asm`, `asm-aarch64`, `loongarch64_asm`, `oid`, `compress`, `force-soft`, and `force-soft-compact` are not enabled. |
| Classification | Production pure-core/runtime dependency for typed SHA-256 identities. It is not a proof, build, or dev dependency and is not presently linked into an admitted SBF ELF. Any later SBF use requires a target-specific closure and final-ELF review. |
| Security and reproducibility | The crate assumes SHA-256 collision and preimage resistance and the correctness of this Rust implementation and its transitives; Dragon's Clutch has not formally verified that boundary. Exact versions/checksums are frozen by the crate-local lock. Offline `--locked` release tests, clippy, and rustdoc passed for the introducing core. Repository-wide advisory/SBOM review and a public third-party notice bundle remain release blockers. |
| Reviewer and date | Codex dependency/provenance audit, 2026-08-23. This is a technical admission record, not human legal advice or final release approval. |

The exact introducing lock is
`crates/clutch-product-series/Cargo.lock`, SHA-256
`ec92ea7f8b9119f36bb15cae13775ba3e3a5d12f8181da2919f9a157c825d897`.
Its exact locked resolution covers the following normal and build dependencies;
target-conditional packages remain lock entries:

| Locked package | Cargo package checksum | Dependency role |
| --- | --- | --- |
| `sha2 0.10.9` | `a7507d819769d01a365ab707794a4084392c824f54a7a6a7862f8c3d0892b283` | Direct SHA-256 implementation |
| `cfg-if 1.0.4` | `9330f8b2ff13b34540b44e946ef35111825727b38d33286ef986142615121801` | Direct `sha2` configuration dependency |
| `cpufeatures 0.2.17` | `59ed5838eebb26a2bb2e58f6d5b5316989ae9d08bab10e0e6d103e656d1b0280` | Target-conditional CPU-feature selection on aarch64/x86/x86_64 |
| `libc 0.2.189` | `3eaf3ede3fee6db1a4c2ee091bf8a8b4dccdc6d17f656fb07896ee72867612f2` | Transitive of target-conditional `cpufeatures` |
| `digest 0.10.7` | `9ed9a281f7bc9b7576e61468ba615a66a5c8cfdff42420a70aa82701a3b1e292` | Hash trait and block-processing boundary |
| `block-buffer 0.10.4` | `3078c7629b62d3f0439517fa394996acacc5cbc91c5a20d8c658e77abd503a71` | Digest block buffering |
| `crypto-common 0.1.7` | `78c8292055d1c1df0cce5d180393dc8cce0abec0a7102adb6c7b1eef6016d60a` | Shared cryptographic types |
| `generic-array 0.14.7` | `85649ca51fd72272d7821adaf274ad91c288277713d9c18820d8499a7ff69e9a` | Fixed-size generic buffers |
| `typenum 1.20.1` | `b6f5e870be6c3b371b77fe0ee0bafb859fa4964b4404c27de1d380043c4dda20` | Type-level lengths used by `generic-array`/`crypto-common` |
| `version_check 0.9.5` | `0b928f33d975fc6ad9f86c8f283853ad26bdd5b10b7f1542aa2fa15e2289105a` | Build-time transitive of `generic-array`; locked even though it is not runtime code |

Changing any direct version, feature, registry source, archive checksum, upstream
commit claim, transitive version/checksum, or lock digest invalidates this record
and requires a new admission review.

## 4. Fixtures and research inputs

Every nontrivial fixture or dataset records:

- source and acquisition date;
- applicable license/terms and redistribution permission;
- exact digest of the acquired input;
- derivation command/tool version and random seed;
- normalization, filtering, and unit conversions;
- synthetic versus observed status;
- whether it may enter the public repository.

Synthetic fixtures must not silently contain copied wallet, provider, or trading
data. No fixture may contain secrets, private keys, seed phrases, RPC credentials,
browser sessions, personal data without an explicit lawful basis, or unexplained
real account activity.

## 5. Generated artifacts

Generated wire types, proof output, vectors, SBF ELFs, static bundles, diagrams,
and benchmark reports must identify:

- the generator and exact version;
- all source/config/input digests;
- a deterministic reproduction command where possible;
- whether the artifact is reviewed source, build output, or evidence only;
- license/notice handling for embedded third-party material.

Generated files never become a second semantic owner. The checked schema or model
from which they derive remains authoritative.

## 6. Contribution checklist

Every future contribution should answer:

- [ ] Is the work original or is every incorporated source identified?
- [ ] Is it compatible with AGPL-3.0-or-later distribution?
- [ ] Are third-party notices and source-offer duties captured?
- [ ] Are fixtures/data redistributable and reproducible?
- [ ] Are generated artifacts clearly marked and rebuildable?
- [ ] Does it avoid the prohibited local-repository dependency boundary?
- [ ] Does it add any trusted proof, runtime, build, source, or hosted service?
- [ ] Are the SBOM and assumption manifests updated?

Unknown provenance blocks public release. Reimplementing an unknown-origin file
with superficial edits does not cure the problem.

## 7. Release ledger

An offline or public release candidate includes:

```text
LICENSE
NOTICE or third-party notices, when required
dependency and toolchain locks
SBOM
source-offer/reproduction instructions
fixture provenance manifests
generated-artifact manifest
proof assumption/trust inventory
source, vector, ELF, and static-bundle checksums
known provenance exceptions (normally empty)
```

No deployment, client mirror, or binary may be called official merely because it
was built from a repository with the correct license. Official identity requires
a checked release manifest and, for any separately authorized deployment, the
exact program-data manifest described elsewhere in this repository.
