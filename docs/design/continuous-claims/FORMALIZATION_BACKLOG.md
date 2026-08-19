# Continuous-claims formalization and promotion map

Status: **MIXED LANDED EVIDENCE / OPEN UNIVERSAL REFINEMENT AND LIVE
AUTHORITY** (2026-08-19). Passing an earlier stage does not imply a later stage.
No mainnet, audit, deployment, or end-to-end formal-verification claim follows
from this document.

> **Supersession notice.** The original C0 item “select the degree-1 basis” is
> complete and obsolete as a future decision. The selected native semantics are
> the open-clamped degree-0--3 basis and `WEIGHT-ROUND-01`. The stage labels
> below now distinguish landed evidence from still-open promotion work; they
> must not be read as a claim that a whole stage is complete.

## C0 — Freeze semantics: partial

Landed:

- native degree `0..=3`, open-clamped knot/count/edge rules, denominator anchor,
  evaluator version, and largest-remainder/lowest-index-tie semantics;
- Terms v3 basis, source/window/statistic, and
  coverage/repair/failure/ambiguity/edge policy identities;
- immutable Kernel v2 `FinitePreset` versus `DerivedBasis` mode; and
- distinct point-v3 and quantized-basis occupation-v4 Resolution meanings.

Open:

- a consensus `ClaimArtifactV1`, integer coefficient scale, and onchain
  commitment to the landed host `NativeShapeCertificateV1`;
- canonical `LiquidityPolicyV1`, schedule, tranche, fee-carry, and share bytes;
- the complete live batch-policy/selection/entitlement preimage; and
- broader gap, smooth interval, and smooth TWAP semantics. Current occupation
  instead requires exact complete point records and smooth TWAP refuses.

Exit remains open until two independent hostile-byte readers agree on every new
claim/policy identity and digest, not merely on model structs.

## C1 — Pure bounded models: substantial, not promoted

Landed host/model evidence:

- the dependency-light shape compiler constructs degree-0--3 rational
  coefficients for hard ranges/tails, triangles, capped call/put spreads,
  affine restrictions, and Gaussian proximity kernels;
- it distinguishes exact span membership from certified approximation, reports
  rational sup/L1 and consensus-quantization bounds, and has a canonical host
  certificate whose Rust decoder recompiles the source description;
- the pure native coefficient-portfolio seam canonicalizes identity, checks
  exact funding and full-simplex worst-case payout, and models exact paired
  settlement while deliberately refusing live authority;
- a bounded two-order direct-selection model streams the best three verified
  candidates under a frozen score/window, with full-width candidate/window
  account-body codecs and now underpins one deliberately narrow source-routed
  authority; a separate resumable occupation model has safe-resume accumulator
  semantics and an isolated unexported 1,296-byte layout codec, but is not a
  routed SBF authority; and
- the proof-constrained liquidity model compiles a maximum-eight-rung schedule,
  maintains `E = B + max_i(q_i+s_i)` without netting, reserves sell-proceeds
  numeric headroom, and models single-owner deposits, partial fills,
  cancel/lapse, buy-back, withdrawal, settlement, bounded risk weight, and one
  owner-aggregated fixed-grid terminal fee allocation with physical carry
  escrow.

Open:

- an independently implemented full shape compiler/certificate reader rather
  than shared production code plus goldens;
- a live rational-to-integer coefficient admission rule;
- broader randomized/differential campaigns across compiler, portfolio, and LP
  seams; and
- machine-checked proofs of compiler enclosures and the LP state machine.

The liquidity successor's 20-test debug/release suite, strict Clippy/rustdoc,
and independent six-case hostile harness are scoped model evidence. V1 has one
immutable beneficial owner per tranche and nontransferable accounting shares;
it does not claim multi-holder exit-order invariance. `MAX_QUOTES = 8` is a
bounded witness, not equivalence to a continuous maker.

## C2 — Kernel and batch refinement: partial

Landed:

- degree-1--3 native vectors join immutable Terms/evidence to
  `resolve_with_vector`; degree zero retains its separate categorical seam;
- signed Portfolio placement and exact buy-cash/sell-Egg Reservation and
  cancellation are live;
- direct-selection intents `27..=31` route a one-page, two-order, single-Egg,
  full-fill, zero-fee authority chain with full-width candidate reexecution and
  Reservation `ACTIVE -> ENTITLED -> CONSUMED`; this source route has no
  real-bank/ELF/CU evidence yet;
- a pure canonical portfolio pair checks Terms, basis, claim, simplex value,
  full funding, exact divisibility, and one-time consumption; and
- standalone full-width batch-policy identity rejects policy/digest truncation.

Open, in dependency order:

1. general relation candidates, partial fills, portfolio selection, fees,
   lapse, and full terminal epoch closure beyond the narrow direct route;
2. complete frozen order-to-Reservation-set commitment;
3. stable vector receipt codec and program-only entitlement creation;
4. exact frozen-page provenance and partial portfolio allocation;
5. live decoded policy preimages and terminal fee-pot/owner/carry authority;
6. terminal Reservation/receipt closure; and
7. policy/tranche translation into those existing semantic owners without
   accepting caller-constructed model structs as authority.

Until these close, atomic coefficient settlement and passive liquidity remain
pure/model seams even though primitive Egg placement and Reservations are live.

## C3 — Executable proof/refinement: narrow landed results

Landed, with exact scope:

- Lean proves named mathematical-model exact-basis, open-clamped endpoint,
  uniform-knot linkage, largest-remainder priority/admissibility,
  maximum-liability, and complete-set results;
- eight Lean-computed fixture rows agree byte-for-byte with the digest-pinned
  complete production Rust evaluator source, and five executable source mutants
  go red; the released source digest is
  `220de128366a8311de6579c0ce334a64c97620159eaf9570f61fa10fabb6de92`;
- the production smooth evaluator uses a validated private capability and one
  fixed-common-denominator path; the reduced-`Fraction` implementation is now
  test-only differential evidence rather than an alternate production arm; and
- a separate Verus result proves the named production transfer-arithmetic helper
  under its executable caller gate.

These results do not compose into a proof of every evaluator input, parser,
compiler certificate, occupation fold, portfolio transition, LP transition,
SBF binary, source adapter, Token CPI, or runtime behavior. At release commit
`87d2dbd60fa13d50e4f8b9e1c3697cd680697ce3`, the B-spline runner, evidence,
and assumptions digests are respectively
`1778824030783f0209d0217cfe158f4f98a3f68ea53e4cb964fc186f0fd9eb67`,
`b3b32b8bdd617229670e8be3844bd7d2cc88774abe6c3bccc7af76246b6deeed`,
and `6e463b0c24223f953163cde4a44d78a371d325000c1b36791726ca074da806ea`.
The evidence-manifest digest is
`d50579898a58f449c0e28a9a77eac44975ae5a855ce560b191b42acc157f11a8`.
The runner intentionally blocks on production-source digest drift; every
evaluator change must be re-pinned and re-exercised before the finite agreement
claim follows it.

Still-open executable proof targets include:

- universal refinement of pane selection, expanded knots, the production
  fixed-common-denominator recurrence, `WEIGHT-ROUND-01`, and every refusal for
  degrees `0..=3`;
- occupation mass/finalizer conservation and v4 codec refinement;
- coefficient compiler bounds and integer admission;
- portfolio/tranche reserve and ownership preservation;
- fixed-grid owner aggregation, direct fee-pot payout, physical carry-escrow
  conservation, and terminal carry; and
- batch vector conservation at frozen array bounds.

Keep account parsing, CPI, runtime ownership, source authentication, compiler
code generation, and deployment outside any proved kernel unless a separately
named refinement closes them.

## C4 — Independent economic state machine: open

The handwritten Rocq shadow remains finite-preset/index oriented, has no checked
native degree-0--3/occupation/portfolio/LP refinement, and its theorem
obligations are not a current proof claim. A complete independent state machine
must prove:

- native partition-of-unity and complete-set identities;
- global solvency across every reachable transition;
- no principal flow to fees, liveness, LP reserve, carry escrow, or insurance;
- exact portfolio Reservation and single-owner tranche share/withdrawal bounds;
- one-shot resolution/redemption, candidate receipt, fee allocation, and retry
  idempotence; and
- refinement from atomic coefficient claims to primitive native Eggs.

Any translation experiment must enumerate unsupported constructs,
`Admitted`/axiom counts, fixed-width assumptions, and the Solana/runtime trust
boundary as release artifacts, not footnotes.

## C5 — Solana composition: native settlement landed, operatorless path open

Landed local SBF evidence includes:

- immutable basis-mode account binding and degree-1--3 blank-bank point-v3
  lifecycle walks through real Token-2022 mints;
- the exact permissionless SourceSpec/Feed/SourceArchive construction ABI,
  exercised end-to-end only by a deliberately non-production mock-source ELF;
  the default artifact has an empty provider/parser registry and refuses before
  its first CPI or state write;
- exact-lot bearer redemption and record-only v2/v3/v4 internal redemption on
  the exact `16+n` account plane, with hostile rollback cases; and
- occupation-v4 statistics 6/7 with direct sealed-archive folding, exact retry,
  provenance, and point/gap/substitution/mode refusals.

Open:

- review and registration of a production provider/deployment authenticator
  and immutable/finalized parser release in the default artifact;
- an occupation execution architecture with adequate CU headroom: the measured
  initial span-1--3/degree-1--3 matrix admits no case under the exact 25%
  headroom gate, while spans 4--32 remain unmeasured and unadmitted;
- shared export/registry/router/account-plane integration for prepaid
  `ResolutionWork`, followed by real-SBF Begin/Fold/Finalize/Abort evidence;
- a clean build-SBF and real-bank account/CU/rent/rollback campaign for the
  newly routed narrow direct-selection chain;
- live atomic portfolio/LP account construction and authority;
- final-LTO stack, rent, account-lock, transaction-size, and reproducible
  artifact evidence after all source changes; and
- deployment, upgrade-authority, cluster, and transaction-inclusion evidence.

Local SVM execution is runtime evidence, not deployment evidence. Any network
exercise remains a separate explicit human gate. The record-only redemption
campaign used a named provisional joined ELF; final clean artifact,
supply-chain, and liveness attribution remains open.

## C6 — Optional cost-function maker: open and ordered later

Only after schedule liquidity has live authority:

- select one regularizer over the actual payoff polytope;
- prove convexity, coherence, cash invariance, and bounded loss;
- compile canonical integer endpoint charges with persistent carry;
- capitalize the loss budget before activation; and
- prove immutable or value-balanced parameter transitions.

The landed liquidity model has no endogenous potential, dynamic depth, or
continuous-availability guarantee. No schedule result can be relabeled as a
cost-function AMM.

## C7 — Static client and release evidence: partial host tooling

Landed host tooling now has canonical native `BasisSpec` and shape-certificate
bytes, exact typed Terms artifact upload/CreateMarket intents, a Rust-generated
fixture, and a static JavaScript implementation which checks structural and
digest equality. The certificate still remains offline evidence because Terms
do not commit it and the onchain program does not parse it.

Open:

- onchain claim/policy/tranche admission and returned-account verification;
- local display of every exact risk, approximation, rounding, source, window,
  and upgrade boundary;
- multiple RPC endpoint support without treating RPC as consensus;
- accessible transaction previews and hostile fixture coverage for all live
  account versions; and
- a source/tool/font/build/artifact manifest for every published document and
  binary.

Deployment, filing, regulator contact, key use, and public release remain
separate explicit human gates.
