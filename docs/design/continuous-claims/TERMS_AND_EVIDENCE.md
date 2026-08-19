# Terms and evidence closure

Status: **PARTIAL LIVE JOIN / EXPLICIT ADMISSION STOPS** (2026-08-19). Current
Terms, SourceSpec/archive, native point-v3, and occupation-v4 layouts bind a
substantial settlement chain. The permissionless SourceSpec/Feed/archive
construction ABI is live and has real-SBF lifecycle evidence under a deliberately
non-production mock registry. The shipped default registry is empty, so no
production source/provider release is admitted; `Endow` now rechecks that exact
registry and refuses before collateral custody. Bounded resumable occupation is
routed, while Direct V2 selection is a measured compute STOP. Shape
certificates, canonical shaped-claim artifacts, liquidity policies, and
portfolio entitlement receipts are also not current onchain identities.

> **Supersession notice.** This file originally described one proposed generic
> `WindowResult -> weight vector` chain. The live smooth paths are now two
> distinct semantics. Point statistics persist Resolution v3; statistics 6 and
> 7 fold quantized native basis occupation directly from a verified sealed
> archive and persist Resolution v4. Smooth TWAP remains refused.

## What immutable Terms bind now

The self-certifying Terms v3 body currently binds:

- Realm, collateral Profile, market collateral cap, feed, and PriceGrid
  identities;
- outcome count, denominator anchor, payout-set bytes, and degree-zero payout
  map (the map must be entirely unused for smooth degrees);
- basis degree `0..=3`, active knot count and values, uniform-spacing
  declaration, edge and ambiguity policies, and evaluator version;
- observation grid, bucket duration, exact start/end range, maturity horizon,
  coverage and repair policies, repair generation, and failure policy;
- statistic identity, source-adapter identity/version, and feed identity; and
- canonical padding and arithmetic bounds, including `MAX_OUTCOMES = 16`.

KernelAccount v2 independently persists the immutable projection
`FinitePreset` for degree zero or `DerivedBasis` for degrees one through three.
Market creation and every Terms-bearing lifecycle seam cross-check that mode.
Changing any digest-bound field creates different Terms; it does not mutate a
live market.

## What current Terms do not bind

The original target list included several identities that have not landed in
the Terms/account plane:

- no onchain `ClaimArtifactV1`, rational-to-integer coefficient scale, or Terms
  commitment to the canonical host `NativeShapeCertificateV1` digest; its
  analytic source, compiler version, approximation norm, and error fields remain
  offline evidence;
- no `LiquidityPolicyV1`, tranche, quote schedule, reserve, share, withdrawal,
  or fee-carry identity;
- no Terms commitment to the live 64-byte BatchPolicy artifact and no
  program-created atomic coefficient-portfolio entitlement receipt;
- no program ELF/build digest or deployment manifest; and
- no production provider/parser registry entry; the live construction route in
  the default artifact therefore refuses before its first CPI or state write.

The host compiler may explain how coefficients were constructed, but a valid
Terms digest does not authenticate that explanation. Likewise, a valid model
policy digest is not authority to quote, reserve, settle, or withdraw onchain.
The routed direct-selection source is narrower: one page, two single-Egg
orders, full fills, and zero fees. It reexecutes its full-width candidate and
drives Reservations `ACTIVE -> ENTITLED -> CONSUMED`, but does not create a
coefficient-vector entitlement or authorize an LP policy. Its real-bank/ELF/CU
evidence proves Init/Freeze/Submit, while maximum top-three Select consumes
exactly 1,400,000 CU and rolls back every watched byte and lamport. Direct V3 is
model-only, so no live settlement or lapse claim follows.

## Current evidence chains

### Native point-v3

```text
canonical SourceSpec + sealed SourceArchive
  -> owner/key/lineage/seal/commitment verification
  -> byte-exact legacy point/window projection
  -> admitted point statistic under frozen Terms
  -> WEIGHT-ROUND-01 native basis evaluation
  -> 319-byte point Resolution v3
  -> ephemeral DerivedBasis kernel reconstruction
  -> exact-lot internal or bearer redemption
```

The caller projection is redundant but still required by the point route. It
must equal the archive exactly and is not an independent authority. Degree
two/three non-point evidence refuses; the program never substitutes a midpoint
or silently chooses an endpoint. Smooth TWAP statistics 4/5 refuse.

### Native quantized-basis occupation-v4

```text
canonical SourceSpec + sealed SourceArchive
  -> once-verified borrowed archive view
  -> each exact bucket evaluated by WEIGHT-ROUND-01
  -> checked u128 occupation masses
  -> Terms-selected ExactOnly or LargestRemainderV1 finalizer
  -> 383-byte occupation Resolution v4 with archive provenance
  -> ephemeral DerivedBasis kernel reconstruction
  -> exact-lot internal or bearer redemption
```

Occupation-v4 uses no redundant caller projection. Statistic 6 accepts only an
exactly divisible average. Statistic 7 applies one final largest-remainder step
with lowest-index exact ties. Both are occupation of the per-bucket canonical
quantized basis; neither is evaluate-at-TWAP or the research-only exact-rational
basis occupation arm.

SourceArchive V1 records only conservative `(low, high)` observations and has no
authenticated gap record. Occupation-v4 currently requires `low == high`, full
coverage, and zero gaps. A positive-width observation, missing bucket, wrong
archive address, altered commitment, conflicting retry, or mismatched
statistic/finalizer refuses before a settlement fact can replace another.

## Evidence ownership and replay

Every accepted runtime edge binds the predecessor identities available in its
version: exact account key and owner, Terms/feed/window identity, source
generation, range/cursor, archive commitment where applicable, and canonical
Resolution PDA. Resolution is market-global and idempotent: an exact retry
rederives the same fact without changing bytes; a conflicting retry refuses.

The v3/v4 Resolution account is the sole persisted owner of a smooth payout
vector. Kernel reconstruction is ephemeral. `RedeemInternal` now has one
record-only 16-account prefix plus the complete mint vector for categorical v2,
point v3, and occupation v4; it accepts neither Feed nor caller evidence. Its
focused real-SBF campaign is green on the named provisional joined artifact,
while final clean artifact attribution remains open. A source brand,
caller-supplied number, syntactically valid digest, host certificate, or
static-client display is not evidence closure.

The source construction instructions create the canonical SourceSpec, Feed,
and SourceArchive, append parser-admitted conservative observations, and seal
the archive with the unique next-bucket maturity witness. This full lifecycle
is proven in a bank only for the non-production mock-source ELF. The default ELF
registers no provider/parser release and is intentionally inert; source
availability and transaction inclusion remain external liveness dependencies.
`Endow` is the protocol-recognized inbound collateral boundary. It now
reauthenticates canonical Terms/SourceSpec and the same compiled registry before
allocating an owner plane or calling Token-2022. The default ELF therefore
refuses with `SourceReleaseUnavailable` (`0x79`); mock-source success is evidence
about a different non-production ELF, not an exception to that refusal.

For future path predicates, the accumulator must retain enough authenticated
summary state for the frozen predicate family. A constant-size generic
accumulator cannot answer arbitrary post-hoc path questions. Shared feeds may
amortize observations, but do not remove that information requirement.

## Gap, ambiguity, and liveness semantics

Terms choose the supported behavior for missing, corrected, stale, conflicting,
or unavailable evidence. No fallback may be invented after activation. The
present occupation route is narrower: current SourceArchive V1 requires exact
complete point coverage and both occupation finalizers refuse gaps. General
smooth interval certificates, authenticated gaps, and smooth TWAP are not live.

Unused liveness endowment must not reward a party capable of causing failure.
Hoard principal, LP reserve, expected fees, and insurance promises are not
liveness capital. A production profile still needs measured prepaid admission
for every mandatory source, resolution, redemption, and cleanup job. In
particular, exact initial occupation measurements for every span `1..=3` and
degree `1..=3` fail the selected 25% operating-headroom gate: the best measured
case is 1,236,364 CU against a 1,120,000-CU threshold. Span-three retries range
from 1,086,756 to 1,108,857 CU, but a retry cannot make the initial resolution
reachable. Spans `4..=32` remain unmeasured and unadmitted; the nonmonotonic
measurements do not justify extrapolation.

The 1,296-byte `ResolutionWork` route now allocates account tag 22 and intents
32--35. Four real-bank tests execute Begin, Fold widths one through four, late
Finalize byte-equivalent to monolithic v4, and expired zero-progress Abort.
Measured maxima are 810,992 / 815,573 / 1,094,832 / 587,197 CU. Terminal paths
close Work and Reserve, return exact principal/unused budget to the frozen
payer, pay callers only from prepaid rewards, and transfer unsolicited excess
to the canonical neutral sink. This is route-level, exact-shape,
zero-charge-policy evidence: the candidate ELF still needs integrated release
identity, unmeasured shapes do not inherit admission, and current archives
still have no authenticated gap record.

These improvements do not establish universal terminal or no-stranding
semantics. A frozen Direct V2 epoch with no candidate has no Window/lapse route,
and the top-three Select compute STOP likewise leaves Reservations without a
terminal release. Except for artifact stages, program accounts have no general
close instruction; most do not persist a defensible rent payer separately from
third-party prefunds. Existing outcome mints have no `MintCloseAuthority`.
Unsolicited Hoard-token donations, externally burned winning claims, and native
sub-lot fragments can leave value with no frozen terminal recipient. They are
not fees, keeper capital, insurance, or LP reserve by inference.

## Coefficient portfolio and liquidity-policy binding

The landed liquidity-policy crate models the following identities, non-netted
reserve/share arithmetic, and explicitly bounded terminal fee apportionment for
at most eight quotes, but derives no canonical digest and owns no live account
or authority. A future atomic coefficient claim, quote, or LP tranche must
additionally bind:

- exact Market Terms and native basis identity;
- canonical integer coefficients, scale, claim identity, and primitive units;
- the tranche's immutable policy and single beneficial owner,
  reserve/inventory generation, and nontransferable accounting-share supply;
- valid batch interval, expiration, all-in limit/quantity schedule, and full
  frozen reservation set;
- candidate-selection and vector-entitlement receipt authority;
- one-terminal-allocation fee grid, owner aggregation, tie, direct-credit,
  rounding, and carry policy; and
- cancellation, lapse, withdrawal, and settlement priority.

Resolution evidence never authorizes a quote, and a quote never authorizes
resolution. Hoard backing, sell-side Egg ownership, tranche reserve, venue cash,
fees, and liveness remain distinct ownership domains. No collateral atom may be
counted twice across them. The model aggregates risk weight by immutable owner,
uses owner-ID ties, records an owner's output on its smallest tranche identity,
and pays whole credits directly rather than increasing reserve. It permits only
one allocation after `batch_end + 1`, with zero old carry/allocation, and bounds
the deviation from each owner's raw quota to less than one atom. A live fee
authority must prove the complete tranche/owner set, consume the pot once, own
retained carry escrow, and pay/apply outputs atomically. Funded terminal carry
still locks the last share; the model assumes neither a second allocation nor
future volume.

## Static-client trust boundary

A static client is an untrusted projection. It must derive canonical addresses,
display program/version/upgrade authority and exact Terms, construct
transactions, and verify returned account bytes locally. Consensus does not
depend on the origin, availability, or honesty of any hosted UI, RPC endpoint,
or indexer. Current host compiler output does not become a valid claim artifact
merely because a client displays or serializes it.

## Release evidence

Before activation, publish exact Terms bytes, program and toolchain identities,
reproducible-build record, account-layout digest, source-adapter vectors,
accumulator/window goldens, claim-compiler goldens, adversarial failures, exact
formal theorem statements and assumptions, and a deployment manifest.
“Verified” must name the theorem, tool, source revision, finite-vs-universal
scope, and every unverified adapter/runtime boundary.

The current native SVM campaigns are local runtime evidence, not deployment or
production-source evidence. The current Lean/Rust bridge is finite executable
agreement, not universal Rust, compiler, SBF, Solana-runtime, or economic
refinement.
