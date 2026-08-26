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
