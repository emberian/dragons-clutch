# The Series arm of dclutch-accelerator-sbf

Stateless Shadow-AOT evaluation for recurring Series, folded into the one
accelerator on 2026-09-04 as `src/series/` (formerly `dclutch-series-shadow-sbf`).

The accelerator owns no Series root, Ticket replay, Market, token account, or
child-CPI authority. One release embeds one generator-produced artifact bundle,
reexecutes its AccountProfile, RequestProfile, TransitionVM, Effect program,
and Series semantic kernel over read-only observations, and emits only a typed
`ShadowAckV3`. Trading remains the sole interpreter authority, child caller,
and commit-last state writer.

`evaluator` is the SDK-free comparison core. The physical SBF account adapter
and checked-in generated bundle are intentionally separate so a release cannot
silently fall back to caller-supplied artifact bytes.

`generator` is a host-only nested workspace. It calls the canonical Series
artifact constructors and emits a bounded `SeriesShadowSourceManifestV1`. The
manifest contains the exact LifecycleV5, 161 fixed account-width rules, five
occurrence-specific child requests, and every generated artifact byte. It also
binds the reviewed semantic source, generator-source manifest, pinned toolchain
manifest, translation certificate, and a domain-separated complete-bundle
digest. Its decoder hostile-revalidates the artifact tuple; its rebuild gate
requires byte-for-byte identity.

The generator's unit manifests are deliberately labeled ephemeral and are not
release evidence. `DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE` is the sole build
input for an explicitly selected generator include; its source, compiler,
toolchain, certificate, manifest, and complete-bundle identities are embedded
in that ELF. With no such build input the crate contains no selected bundle and
remains fail-closed. A physical entrypoint is enabled only after the separately
authenticated Shadow callback and checked ELF release are selected.

## The certificate cannot currently be authored (2026-08-29)

Read this before trying to select a release. The ShadowAot certificate this
crate's strategy names is not a fact nobody has gathered; under the shipped
typing it is unconstructible, and the loop is enforced in real code:

1. `certificate_id = sha256(certificate record bytes)` and the strategy must
   name it — `dclutch-operator/src/series_hot_v3.rs:545`,
   `dclutch-execution-strategy-contract/src/v2.rs:524`.
2. The generated bundle's strategy embeds that identity
   (`generator/src/lib.rs:229`), and `src/evaluator.rs:451` refuses unless the
   embedded strategy's `certificate_program` equals the embedded
   `SERIES_SHADOW_CERTIFICATE_ID_V1`. So it is in the ELF's bytes.
3. `ArtifactReleaseV1` carries `elf_digest`
   (`dclutch-registry-contract/src/artifact.rs:78`), and
   `artifact_release = sha256(ArtifactReleaseV1 bytes)`
   (`series_hot_v3.rs:573`).
4. `ExecutionStrategyCertificateV2::validate_artifact` requires the certificate
   to contain exactly that (`v2.rs:583`, called at `series_hot_v3.rs:568`).

Measured rather than argued: two ELF builds whose includes differed only in the
certificate constant produced different digests
(`07064ab1…` vs `cc6b326b…`, both 377,080 bytes), each rebuild was
byte-identical, and the 32 certificate bytes appear verbatim **twice** in each
ELF. The constant is not optimised away, so the loop is closed.
`generator/src/manifest/tests.rs` pins the dependency.

The General accelerator has no such problem and is **not** a precedent: it
embeds no certificate, strategy, or descriptor, so nothing about it names
itself.

Two things follow. First, do not mint a certificate from an earlier deployment
at the same address — the program, programdata and loader keys are unchanged
across redeploys, nothing on chain compares `elf_digest` to the live
programdata, and such a certificate would pass every check while being false.
Second, the fix likely belongs in the typing: `ArtifactReleaseV1` already
carries `semantic_release_id`, derived from source rather than from the built
ELF, and a certificate binding that would leave deployment bound by the checks
already at `series_hot_v3.rs:580-585` without closing a loop. That is a change
to a struct shared with AdmittedAot, so it needs a decision rather than a
patch.

Note also that `tools/release/checked-release-candidate.sh` never sets
`DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE`, so every checked release to date has
built and shipped this crate in its fail-closed form (55,928 bytes here,
against 377,080 for a selected build). Sizing or budget figures taken from that
artifact describe a program that cannot run.
