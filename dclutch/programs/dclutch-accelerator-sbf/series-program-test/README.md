# Series Shadow real-SBF campaign

This nested harness accepts only real SBF ELFs from `SBF_OUT_DIR`. It has no
native or mock processor fallback. The selected Series Shadow crate and the
harness must both be built with the exact generator-emitted include named by
`DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE`; the source manifest, include digest,
embedded bundle, and real ELF digest are checked before campaign evidence is
recorded.

The production founding order is fixed as:

1. projected Custody `LockHoardAndCloseSource`;
2. Core `Found`;
3. projected Custody `RealizeAndClose`;
4. Claims `FoundingV5`;
5. Core `Open`.

The eventual integration test invokes the production projected Trading outer,
which continues through ordinary live-Market Hot after Core Found. It then
retires the terminal Ticket, closes a zero-outstanding Series root, and closes
the lifecycle Rent V2 sink through the generic retirement authority. A late
Claims/Open refusal snapshot covers the Series root, Ticket, Market, permit,
Claims, Custody, ordered FundingStates, and LifecycleRentCreditV2 byte-for-byte.

Until the common authenticated Shadow callback is committed, this crate exposes
only the real-ELF loader, selected-build gate, route-order contract, and rollback
snapshot support. It does not install a provisional entrypoint or pass artifacts
at runtime.
