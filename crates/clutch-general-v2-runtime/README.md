# General V2 pure runtime join

This `no_std`, allocation-free, safe-Rust crate is the executable direct-
candidate join that was missing between Product V2, the General V2 feed
contract, the quantized price-measure checker, owner-blind RelationV2, and
ScoreV2-Q.

The same crate now contains the first honest construction path. It streams a
frozen OrderPage set into the single owner-blind `EconomicBookV2` truth,
enumerates a caller-bounded deterministic family of exact singleton and
primitive two-atom measures plus supplied wider measures, derives integer
simplex prices without floats or a new rounding boundary, requires an exact
finite production atom-mixture certificate, enumerates
zero/minimum/full fill coordinates plus maximal exact buy/sell ratio pairs, and
retains the best candidate that the authoritative V3 and RelationV2 checkers
actually accept. Its result reports whether all members of that named bounded
heuristic family were visited. It never describes the result as an optimal
clearing.

The actual successor ranking path deliberately admits only degree-two and
degree-three Product bases, with respectively at least three and four outcomes
and at most sixteen. Degree zero and one remain available only through the
legacy V3 verifier; they cannot enter the successor policy until an equally
exact finite certificate is selected for those profiles. The caller supplies
authenticated immutable Product bodies, the canonical PriceGrid, the revealed
AdmissionNode, the EconomicDomain artifact, a sealed CandidateFeed account,
and the ownerless RelationV2 book. The crate then:

1. decodes and validates the complete sealed active-width feed;
2. derives the canonical owner-blind RelationV2 and ScoreV2-Q policy IDs and
   joins full Product, Genesis, market, grid, basis, policy, and domain IDs;
3. derives the canonical V3 quantized-witness body digest through the General
   contract's single active-width transcript owner;
4. proves every candidate price is an exact grid member;
5. verifies exact finite-atom coherence against the production quantized
   B-spline evaluator and reprojects the same feed atoms into the required
   payout-denominator-scale positive-mixture certificate;
6. mints a private price authority only from that certificate, then verifies
   the owner-blind coefficient-vector relation and recomputes its
   final candidate digest;
7. recomputes ScoreV2-Q; and
8. emits the canonical descending rank with the authenticated Node's
   Window-assigned first-admitted ordinal tie.

The sole payout rounding boundary is Product's immutable largest-remainder,
lowest-outcome-index quantizer. Prices are exact integer grid members before
entry; neither certificate reconstruction nor RelationV2 rounds.

General admission requires the MarketBinding/Grid price scale to equal the authenticated
NativeClaimBasis payout denominator. The exact Genesis V2 identity supplies
the complete coordinate-domain Terms binding, the NativeClaimBasis identity
supplies the evaluator binding, and RelationV2's canonical candidate-price
digest supplies the price binding. The verifier constructs the 544-byte
certificate ephemerally from the already-retained feed atoms; there is no
second atom account, persisted certificate, or caller-selected identity. The
builder and public sealed-feed verifier both converge on the same private
certificate-bearing price authority before any successor RelationV2 call.

The price witness is intentionally representation-nonunique. Its canonical
body digest authenticates the retained sidecar but is absent from the
RelationV2 candidate identity and ScoreV2-Q rank. The successor Relation policy
digest instead commits the exact certificate schema, production evaluator
semantics, admitted degrees, denominator-scale rule, and proof-independent
identity rule. Because that policy digest and the exact semantic price digest
already enter RelationV2's candidate transcript, a caller cannot substitute a
raw simplex while alternate valid witnesses cannot grind the rank. Selection
is over the best valid submitted candidate under the frozen rank, never an
assertion of optimal clearing.

The isolated General SBF source now checks this exact admission before creating
even a resumable nonempty ClearWork account, and repeats it on the empty-book
completion path before projecting ScoreV2-Q. Work creation's successor tuple
also authenticates the canonical PriceGrid PDA and every active tick plus full
Template/Basis/Policy/Genesis/MarketInstance coordinate ownership. ClearWork
binds the authenticated feed and successor policy thereafter. The 17-account
handler is staged pending the shared account-meta/capability join. This is
source composition only: the non-production profile remains disabled by
default and no build, local-bank, compute, or deployment claim follows from it.

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

The preselection `CandidateCostCertificateV1` successor rejoins that private
owner membership only after RelationV2 accepts the exact fills. It aggregates
signed contingent payoff by semantic Position owner, subtracts each owner's
minimum outcome coordinate (the risk-free complete-set quotient), then derives
the exact quotient range and state-price value. The ranking coordinate is the
sum of terminal-owner ceilings of those quotient values. Adding a constant
complete set and its exact simplex cash equivalent therefore cannot improve the
coordinate. The certificate also commits exact gross buy/sell consideration,
terminal buyer-ceil/seller-floor residue, and virtual split/merge work, but does
not reinterpret them as fee revenue, volume quality, identity/personhood,
collateral funding, or evidence of optimal clearing.

The certificate binds a canonical batch-policy preimage and its full content
ID without accepting an authentication boolean. `MarketBindingV2` now owns that
immutable `batch_policy_id` under the existing MarketBinding account tag, and
the cost-aware wrapper exact-joins the preimage, Market ID, breaking score
policy, owner projection, RelationV2 candidate, and certificate. The existing
ScoreV2-Q rank encoder and live action 14/15 ABI remain unchanged. Same-tag
`CandidateWindowV5AccountV1` and `AdmissionNodeV4AccountV1` pure contracts own
the 96-byte rank and checked certificate ID for the future action-14 seam; all
SBF capabilities remain disabled pending Work V3 composition and the counted
settlement root that will own action-15 output.

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

The authoritative account-local action-25 composer accepts only OrderPage V5,
the canonical ordinary-General Position purpose profile, and the exact
`0x81/3` OwnerSettlement row. It consumes the V5 slot's authenticated Position
generation, rederives the Receipt V3 prestate through the typed owner-V3 hash
domain, and advances the receipt latch, row, Reservation, and GEN1 Replay as
one write set while leaving Position unchanged. Only the canonical terminal
buy end carries `Reservation.remaining_cash_atoms` into the mutable row; the
completed-order mask and Reservation's zeroed successor make that ownership
handoff once-only. The older compact V3 composer remains a withdrawn reference
for the V2 row and pre-V5 page shape.

The action-26 successor derives its sole delivery identity from the
authenticated Receipt V3 PDA and stages both real endpoints as one bundle.
Each endpoint rechecks its V5 page generation against the canonical
Reservation and ordinary-General Position V3, requires a finalized `0x81/3`
row, advances cumulative Reservation delivery, returns a seller portfolio
remainder only at cumulative completion, and emits a role-distinct GEN1
Replay V3 successor. The receipt becomes exhausted only alongside both exact
Position and Reservation postimages; this action creates or closes no account
and has zero rent movement.

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
