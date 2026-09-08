# The Series arm of dclutch-accelerator-sbf

Stateless Shadow-AOT evaluation for recurring Series is folded into the
accelerator at `src/series/`. The accelerator owns no Series root, Ticket
replay, Market, token account, or child-CPI authority. It reexecutes the
selected AccountProfile, RequestProfile, TransitionVM, Effect program, and
Series semantic kernel over read-only observations, then emits a typed
`ShadowAckV3`. Trading remains the sole interpreter authority, child caller,
and commit-last state writer.

The host-only `generator` builds one bounded `SeriesShadowSourceManifestV1`
from finalized records and one checked observation. Its decoder
hostile-revalidates every generated artifact and its rebuild gate requires the
same source bytes to reproduce the manifest byte for byte.

## Certificate production and the acyclic release graph

`ExecutionStrategyCertificateV2` already has two binding profiles. The
`Release(ArtifactReleaseIdV1)` profile remains required for `AdmittedAot` and
is validated by `validate_admitted_aot_v2/v4`; it continues to require the
exact ArtifactRelease identity, ELF digest, and deployment authentication.
Series Shadow uses the existing `Semantic(ContentId)` profile only.

The canonical Series generator now authors the Certificate record rather than
accepting a certificate identity as an input. It derives these fields before
building the strategy or descriptor:

1. Profile13, RequestProfile, TransitionVM, and Effect content identities.
2. The source-derived semantic release identity of the checked accelerator
   ArtifactRelease.
3. The compiler-source, toolchain, and translation-validation identities.

It returns both exact `ExecutionStrategyCertificateV2` bytes and their content
identity. The generated Shadow strategy names that identity; the descriptor
then names the strategy. Neither strategy nor descriptor identity is a
Certificate input, so this is not the old certificate → ELF → ArtifactRelease
→ certificate fixed point. An ArtifactRelease subsequently binds the exact ELF
and deployment as before. The Series runtime validates the semantic identity
against the checked ArtifactRelease separately from the exact program and
programdata checks.

The first-selection producer is
`build_series_shadow_preselection_v1(SeriesShadowBundleSourceV4)`. It accepts
the canonical prepublication descriptor semantics, Lifecycle, observed widths,
child bank, and checked evidence; it does not require ProgramSet or descriptor
records that only exist after the selected release is compiled. The full
`build_series_shadow_source_v1` repeats that work after publication and proves
those finalized records equal the generated bundle.

Both paths expose this pair as:

```text
BuiltSeriesShadowSourceV1 {
  certificate: [u8; EXECUTION_STRATEGY_CERTIFICATE_BYTES_V2],
  build_inputs.certificate: ContentId,
}
```

The raw Certificate bytes must be finalized as the Registry Certificate record;
the resulting content identity must equal `build_inputs.certificate` and is the
only value eligible for `consume_shadow_certificate_program`. No manual hex
value or zero/default certificate is a selected release.

Tests cover reproducible certificate/include generation and hostile refusal of
a substituted semantic release or generated contract artifact tuple.

## Checked selected-build boundary and remaining wall

`tools/release/checked-release-candidate.sh` now accepts one absolute,
canonical `--series-shadow-generated-include` path. It stages the exact
generator output beneath its work root, applies it only to the accelerator's
ordinary and frame builds through `DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE`,
and records the staged SHA-256 in `SUMMARY.txt`. It takes no shell command and
no textual certificate identity.

The Series driver must still perform the complete vertical slice:

1. Authenticate the shared checked accelerator ArtifactRelease, then derive
   the Certificate from its semantic release and checked compiler, toolchain,
   and translation evidence.
2. Reauthenticate the generated Certificate, manifest, and include; atomically
   write them in its new output directory; finalize the Certificate record and
   require its content identity to equal `build_inputs.certificate`.
3. Invoke the checked candidate with that staged include. Reauthenticate the
   selected checked manifest and ELF against the same source revision and
   semantic release, then derive the selected ArtifactRelease from that exact
   manifest.
4. Finalize the selected ArtifactRelease record and check its raw/staging pair,
   Loader deployment, and exact ProgramData ELF before claiming selected
   runtime evidence.

No selected SBF build, Registry publication, deployment, budget, or runtime
claim exists until this driver produces that evidence.
