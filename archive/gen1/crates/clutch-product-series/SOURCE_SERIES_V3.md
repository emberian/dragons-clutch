# Source-bound Series occurrence slice

`source_series` is a pure successor seam between the V5 Product/Series model
and the existing `clutch-source-plane-v3` semantic owners. It does not mint a
second Window or statistic identity. Instead it derives the canonical
`WindowSpecV3` and `StatisticKeyV3` identities after an adapter authenticates:

- the exact SourcePlane V3 contract, whose codec and capability set refuse both
  legacy and unknown future versions;
- the existing SourceSpec and source-neutral SummaryProgram owners;
- the central registry release and capability profile; and
- the registry mappings for the Template's statistic and coverage selectors.

`AuthenticatedSourceSeriesAuthorityV3` is default-deny. There is no compiler
overload accepting a raw source/registry DTO or an `is_authenticated` boolean.
An adapter implementation is nevertheless still a trust boundary: Rust cannot
prove that an implementation actually checked account owner, PDA, release, or
registry bytes.

`CompiledSourceOccurrenceV3` is a 184-byte immutable provenance record. The
economic identity remains `MarketInstanceV2Id`; Series and ordinal appear only
in the provenance record. The source-plane crate remains the semantic owner of
the referenced WindowKey and StatisticKey.

This slice deliberately has no cursor or lifecycle phase. `SeriesFundingStateV1`
is the single mutable owner of `next_ordinal`, lapse count, and the five
segregated principal compartments; created count and phase derive from that
state. Persisting those facts here would create a second truth. The occurrence
compiler moves no value and does not
register or activate a Series, authenticate Clock, create an account, debit a
quote component, or prove an occurrence is present. A live adapter still needs
owner/PDA/rent checks, authenticated Clock-to-bucket mapping, exact component
receipts, atomic rollback, terminal refund/lapse ownership, and SVM evidence.
