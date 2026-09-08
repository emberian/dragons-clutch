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

## Remaining checked-release wall

The checked-release pipeline still does not select a Series generated include:
`tools/release/checked-release-candidate.sh` leaves
`DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE` absent, so current checked releases
remain intentionally fail-closed. The next selected accelerator build must:

1. Authenticate the checked ArtifactRelease and take its semantic release
   identity plus exact translation-validation identity into the source
   operator.
2. Finalize the returned Certificate bytes, require its record content identity
   to equal `build_inputs.certificate`, and hand that value to the Series
   Prepare driver.
3. Atomically stage the returned include, set
   `DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE` for the actual accelerator build,
   and refuse a selected build without it.
4. Register the new ArtifactRelease only after its exact ELF digest and live
   deployment checks pass.

No sizing, budget, or runtime claim is made for a selected Series accelerator
until that checked build and registration evidence exists.
