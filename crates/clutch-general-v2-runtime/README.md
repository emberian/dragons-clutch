# General V2 pure runtime join

This `no_std`, allocation-free, safe-Rust crate is the executable direct-
candidate join that was missing between Product V2, the General V2 feed
contract, the quantized price-measure checker, owner-blind RelationV2, and
ScoreV2-Q.

It admits smooth degree-two and degree-three markets only. The caller supplies
authenticated immutable Product bodies, the canonical PriceGrid, the revealed
AdmissionNode, the EconomicDomain artifact, a sealed CandidateFeed account,
and the ownerless RelationV2 book. The crate then:

1. decodes and validates the complete sealed active-width feed;
2. derives the canonical owner-blind RelationV2 and ScoreV2-Q policy IDs and
   joins full Product, Genesis, market, grid, basis, policy, and domain IDs;
3. freezes and recomputes the canonical V3 quantized-witness body digest;
4. proves every candidate price is an exact grid member;
5. verifies exact finite-atom coherence against the production quantized
   B-spline evaluator;
6. verifies the owner-blind coefficient-vector relation and recomputes its
   final candidate digest;
7. recomputes ScoreV2-Q; and
8. emits the canonical descending rank with the authenticated Node's
   Window-assigned first-admitted ordinal tie.

The sole payout rounding boundary is Product's immutable largest-remainder,
lowest-outcome-index quantizer. Prices are exact integer grid members before
entry; neither certificate reconstruction nor RelationV2 rounds.

The price witness is intentionally representation-nonunique. Its canonical
body digest authenticates the retained sidecar but is absent from the
RelationV2 candidate identity and ScoreV2-Q rank. Selection is therefore over
the best valid submitted candidate under the frozen rank, never an assertion
of optimal clearing.

This crate does not authenticate Solana owners or PDAs, project an order-set
account into `EconomicBookV2`, check the settlement-slice decomposition,
persist a verdict, authorize settlement, implement fees, or activate an SBF
capability. Those are explicit adapter/integration dependencies rather than
implicit success claims.

The successful wrapper has private fields and no public constructor. Safe
downstream code can inspect its checked economics and rank through getters but
cannot fabricate the wrapper as a substitute for executing the full join.

The default host build forwards the existing full layout profile. A future
General adapter must disable defaults and select `layout-profile-general`, so
Cargo feature unification never combines the mutually exclusive full and
General layout profiles.
