# dclutch-series-shadow-sbf

Stateless Shadow-AOT accelerator boundary for recurring Series.

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
manifest contains the exact LifecycleV5, 157 fixed account-width rules, five
occurrence-specific child requests, and every generated artifact byte. It also
binds the reviewed semantic source, generator-source manifest, pinned toolchain
manifest, translation certificate, and a domain-separated complete-bundle
digest. Its decoder hostile-revalidates the artifact tuple; its rebuild gate
requires byte-for-byte identity.

The generator's unit manifests are deliberately labeled ephemeral and are not
release evidence. This package remains fail-closed: it has no production
entrypoint or accepted embedded manifest until a separately reviewed source
manifest and checked ELF release are selected.
