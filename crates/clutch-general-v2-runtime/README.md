# General V2 pure runtime join

This `no_std`, allocation-free, safe-Rust crate is the executable direct-
candidate join that was missing between Product V2, the General V2 feed
contract, the quantized price-measure checker, owner-blind RelationV2, and
ScoreV2-Q.

The same crate now contains the first honest construction path. It streams a
frozen OrderPage set into the single owner-blind `EconomicBookV2` truth,
enumerates a caller-bounded deterministic family of exact singleton and
primitive two-atom measures plus supplied wider measures, derives integer
simplex prices without floats or a new rounding boundary, enumerates
zero/minimum/full fill coordinates plus maximal exact buy/sell ratio pairs, and
retains the best candidate that the authoritative V3 and RelationV2 checkers
actually accept. Its result reports whether all members of that named bounded
heuristic family were visited. It never describes the result as an optimal
clearing.

It admits the complete Product-selected V3 quantized family: mapped finite
degree-zero markets and smooth degree-one-through-three markets. The caller
supplies authenticated immutable Product bodies, the canonical PriceGrid, the
revealed AdmissionNode, the EconomicDomain artifact, a sealed CandidateFeed
account, and the ownerless RelationV2 book. The crate then:

1. decodes and validates the complete sealed active-width feed;
2. derives the canonical owner-blind RelationV2 and ScoreV2-Q policy IDs and
   joins full Product, Genesis, market, grid, basis, policy, and domain IDs;
3. derives the canonical V3 quantized-witness body digest through the General
   contract's single active-width transcript owner;
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

The page projection validates frozen page commitments, exact market/epoch/
order-set bindings, grid membership, widths, expiry, and RelationV2 admission.
Owner and replay-generation labels do not enter the economic projection;
single-Egg and portfolio records map to one coefficient-vector order type.
The projection retains its exact MarketBinding, EconomicDomain digest, and
PriceGrid/Realm identities behind private fields, so the solver cannot relabel
the resulting book before feed construction.
CandidateFeedV2 serialization then takes all economic fields only from the
checked builder result and all rank/policy/lifecycle fields from authenticated
General accounts. No caller-supplied score or rank is representable.

The settlement constructor privately rejoins owner/replay membership retained
from the complete frozen page projection, decodes and recomputes one active
reservation envelope per live order, and constructs a deterministic
owner-aware pairing. It classifies every exact tail row as direct,
split-to-buy, or sell-to-merge; derives real receipt ends on demand; commits a
canonical owner/order-set digest; derives the contract-owned settlement-witness
digest over the exact active slice tail; and serializes that tail through the
one CandidateFeedV2 codec. The complete bundle identity is then recomputed by
the contract from typed header fields and exact active tails, not from a second
runtime transcript.

Before owner rows can be built, the constructor also requires exactly one
canonical `PositionAccountV3` body for every distinct frozen owner. It consumes
the checked Realm-selected `BoundCollateralProfileV2`, binds full
MarketInstanceV2, Realm, collateral-policy, and adapter-release identities,
requires General-purpose open Positions, derives each exact Position data ID,
and rejoins every reservation generation. Controller, replay, purpose binding,
cash, and native-Egg state remain committed by that data ID even though frozen
orders distinguish only their semantic Position owner.

Terminal fee inputs come from the private authenticated fee-runtime projection,
including explicit zero rows for sellers. The bridge recomputes
`CandidateSettlementTotalsV1`, canonical owner rows, receipt accounting inputs,
and the candidate cash-pot expectation without accepting caller summaries.
Receipt/Position PDA and account-owner authentication plus complete Egg/
reservation state transitions remain explicit SBF-adapter obligations.
The owner-settlement cash expectation keeps terminal split principal and
opening merge proceeds as distinct typed directions. Its current receipt API
cannot represent a zero-price real end; the bridge refuses that shape rather
than inventing an extra rounding event.

This crate does not authenticate Solana account owners or PDAs, persist a
verdict, authorize value movement, or activate an SBF capability.

The successful verifier wrapper and builder result have private fields and no
public constructors. Safe downstream code can inspect checked economics,
search coverage, and the verifier-derived rank through getters but cannot
fabricate either wrapper as a substitute for executing the owned checkers.

The default host build forwards the existing full layout profile. A future
General adapter must disable defaults and select `layout-profile-general`, so
Cargo feature unification never combines the mutually exclusive full and
General layout profiles.
