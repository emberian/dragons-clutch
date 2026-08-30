# Runtime-width and CU architecture change matrix

Date: 2026-08-28
Accepted source reviewed: `3d11642387c637bc168538d754b5bc90fe677799`
Scope: architecture audit of the externally reachable lifecycle and the SBF
links that can carry it. This is not a checked release or deployment record.

## Result

There is no single protocol-wide "CU issue." Four independent runtime ceilings
have to be diagnosed separately:

| Ceiling | Exact runtime rule | What changes it | What does not change it |
| --- | --- | --- | --- |
| Account locks | at most 64 complete unique transaction keys on current devnet | split the account topology or remove accounts from the transaction | an address lookup table |
| Signed packet | at most 1,232 serialized bytes | reduce instruction data, signatures, or inline addresses; an ALT can compress addresses | a higher compute limit |
| Transaction compute | at most 1,400,000 CU shared by the outer program and every CPI | remove repeated work, precompute non-authoritative work, or split at a safe state boundary | an ALT or a smaller packet |
| SBF stack frame | each function frame must remain below 4,096 bytes | shorten live locals, avoid large by-value objects, or move buffers to bounded heap/account storage | splitting a transaction by itself |

The current architecture response is therefore deliberately mixed:

- keep Direct Hot's value/liability mutation atomic and remove duplicated
  authentication and PDA-search work inside it;
- keep the accepted controller-funding and Resolution V7 transaction splits;
- finish AggregateRetirement's already-implemented checkpoint exterior;
- finish Dealer's page/checkpoint/final-commit path because its unsplit frame
  has 121 instruction locks;
- use the existing SHA-256 syscall adapter in Series and in Fractional's future
  live SBF integration; and
- add another provider/relay checkpoint only if a final source-bound
  measurement shows that the already incremental route needs it.

## Artifact identity: what was actually measured

Three source generations must not be blended:

1. The public devnet deployment in `DEPLOY_1.md` contains the seven permanent
   programs. Its activation observations are deployment diagnostics, not a
   current CU margin.
2. The last complete checked-build gate was source
   `2b0e6c29b9adea55b979585e20cfc024ea07816c`. Its Trading ELF
   `675c9c45bde6089ef4b57daf770ece7d2bd33870a0043e42e5d0e2119c229d2a`
   is the newest exact M-61 baseline: **20/20 passed, 20-seed arithmetic mean
   1,359,277 CU**. It predates the accepted splits and is not the current ELF.
3. Accepted source `3d116423` is newer than both. It has no final all-link build
   or M-61 result yet. No current margin may be inferred from the older ELF.

The canonical Resolution package is `dclutch-resolution-proof-sbf`, with ELF
stem `dclutch_resolution_proof_sbf` and checked-gate role path
`elf/resolution.so`. The old `dclutch_sbf.so` is a banished generation-one
monolith. It is neither a permanent role nor one of the checked links. Any
"Resolution CU" obtained by substituting that orphan ELF is an invalid hostile
control, not protocol evidence.

The last complete checked gate enumerated thirteen links: the seven permanent
roles, General and Dealer accelerators, Series Shadow, plus three frame-only
program links for Dealer, Direct AOT, and Product Runtime V2. Its deepest-frame
diagnostics identify the following current rewrite targets, but remain bound to
the old source:

| Link | Old deepest frame | Spare bytes before 4,096 | Highest-risk old function |
| --- | ---: | ---: | --- |
| Trading | 4,032 | 64 | `direct_replay_setup_v1::invoke_replay_child_v1` |
| Core | 3,968 | 128 | `generic_founding_v1::authenticate_claims_and_custody` |
| Custody | 3,968 | 128 | projected `advance_source_state` / `realize_and_close` |
| Resolution | 3,968 | 128 | `core_effect::process_close` |
| Claims | 3,904 | 192 | protocol Position close |
| Series Shadow | 3,264 | 832 | link-wide old maximum |

The accepted Aggregate checkpoint campaign subsequently remeasured changed
Core at 3,968, Claims at 3,904, and Rent at 1,344 bytes with zero emitted-stack
overwrite diagnostics. Those are exact diagnostics for that focused source,
not a replacement for the final all-link gate.

## Prioritized change matrix

| Priority | Route | Actual wall or evidence | Concrete architecture change | Atomic section that must remain | Done when |
| --- | --- | --- | --- | --- | --- |
| P0 | Direct Hot | true compute pressure; last exact old ELF M-61 was 20/20 with 1,359,277-CU mean | consolidate child invocation authority where envelopes remain role- and request-bound; decode/hash each authenticated object once; reuse owner-stored canonical bumps | order, Position, Claims, Custody, token, and replay effects commit in one rollback domain | exact final Trading ELF passes frames and M-61; report 20-seed pass count and mean |
| P0 | Controller funding/Open | old combined controller stage exhausted 1.4M; accepted caller is now `DCLTCFQ1 -> DCLTPCB2 -> DCLTGMF2` at 51/60/60 compiled keys | retain `Prepared -> CustodyStaged -> Open-or-abort`; complete durable journals and hostile resume for every step | each step is atomic; Open's liability creation remains one transaction and closes checkpoint last | caller-backed lifecycle, both cleanup orders, exact packet/key census, final frames/M-61 |
| P0 | Resolution V7 | old Core-wrapped V6 lifecycle passed 0/3 at the 1.4M maximum | retain direct activation receipt, no-CPI Core Accept, direct provider execute, terminal Core Accept, direct close | each heavy mutation plus its immediate receipt is atomic; Core accepts only a live exact receipt | full caller, journal, crash/resume, real SBF, and final source-bound campaign |
| P0 | Aggregate retirement | old direct payload was 2,152 bytes before framing, so compute was not the first wall | use landed four-step `prepare -> close-vault -> close-replay -> finish`; add durable caller journals that resume from the onchain phase | each child close and receipt is atomic; finish closes checkpoint, Market, then RentCredit last | four exterior transactions execute from a clean lifecycle and survive every restart point |
| P0 | All shipped links | old Trading/Core/Custody/Resolution frames had only 64-128 bytes spare | replace large local arrays/CPI meta banks with bounded heap or borrowed account views; split authentication, planning, invoke, and receipt functions to shorten simultaneous locals | no semantic split is implied; this is local frame-lifetime work | fresh frame diagnostic parses every final link and finds no frame at or above 4,096 |
| P1 | Dealer Accepted | unsplit canonical frame has 121 instruction locks and 122 transaction locks with distinct payer | complete landed Trading checkpoint: create, six ordered authenticated page receipts, seal one best valid submitted candidate, then bounded final commit | Claims/Custody/obligation mutation stays atomic in final transaction; preparation moves no liability | real page producers, accelerator receipt, <=64-key final caller, cleanup, real SBF and M-61 |
| P1 | Series Shadow | the shipped Series SBF directly links software `sha2` and hashes the request plus embedded artifact slices | replace production `Sha256` calls with `dclutch_sha256_adapter::digest`/`digestv`; preserve exact concatenation and host vectors | unchanged digest is reauthenticated in the same Series transaction | SBF dependency census contains no software SHA; exact digest vectors and real-SBF caller pass |
| P1 | General Complete V2 | action semantics are incremental; old N=258 legacy packets, not compute, were the measured wall | finish fifteen-action artifacts and real caller; retain v0/ALT routing; settle/release orders incrementally from sealed candidate facts | each order's collateral/Claims mutation remains atomic; never claim optimal clearing without a checked certificate | real caller executes all fifteen actions with lock/packet/frame/M-61 evidence |
| P2 | Fractional twin | V3 contract has bounded 40-lock/700-byte maximum topology but no live Claims or Trading caller | add Fractional-specific Claims child and Trading dispatch; integrate adapter at every SBF-reachable digest; retire one coordinate at a time | Wrap/Unwrap/Terminal Claims + Token/Custody mutation remains atomic; coordinate retirement changes no liability | artifacts, producer root, one-coordinate closure, real ELF/caller, frames/M-61 |
| P2 | Relayed/Pyth observations | already incremental; no final source-bound proof of a remaining ceiling | add a single-use verified-observation checkpoint only if the final caller still approaches the ceiling | provider execution or relay Consume remains the sole state/economic commit | measurement first; checkpoint only with exact binding and cleanup semantics |

The priority is architectural, not a request to serialize work behind one large
release gate. Caller, stack, Direct, Dealer, Series, Fractional, and General
lanes can progress independently; the final evidence campaign should be batched
after their ELFs freeze.

## Direct Hot: reduce work without weakening atomicity

Direct's setup is already incremental: replay initialization, ALT
create/extend/freeze/activate, seal, token setup, then Hot. The Hot transaction
must not be divided into a prepare transaction that moves assets and a later
transaction that mints or updates liabilities. A crash between those steps
would create an unmatched asset/liability state.

The safe optimization surface is inside the atomic transaction:

1. **One authenticated invocation authority.** Several child calls currently
   pay for independent ephemeral PDA authentication. A consolidated authority
   is acceptable only if every child envelope binds release set, Market and
   generation, exact role and program, request digest, invocation/route index,
   batch identity, expiry, and replay domain. A generic signer PDA that a child
   can reinterpret is not acceptable.
2. **Decode and hash once.** Parse each immutable artifact and mutable prestate
   into a borrowed authenticated view, then pass references to planning and CPI
   receipt checks. Do not persist a second DTO as protocol truth.
3. **Reuse canonical bumps from their semantic owner.** Where Manifest,
   ProgramSet, Config, or a lifecycle record already owns a stable bump,
   validate it with `create_program_address` instead of searching again. Do not
   accept a caller-chosen alternative bump or introduce two replay authorities.
4. **Shorten live locals.** Build child metas and request bytes immediately
   before each CPI, consume the receipt, then let those buffers die before the
   next child. This attacks both CU and the 4,032-byte Trading frame.

Historical phase diagnostics found the largest lottery spread around lifecycle
close/authentication work, not in the exact-integer trade calculation. They
identify where to optimize; they are not a current margin.

## Checkpoint invariant template

Every new multi-transaction architecture in this matrix must preserve the same
minimum contract:

- the checkpoint is fixed-layout, versioned, canonical, and owned by one
  program;
- it binds the release set, Market and generation, exact request digest, exact
  mutable prestate digest, page/order coordinate, rolling receipt digest,
  expiry/revision, and immutable rent/refund beneficiary;
- phases are exhaustive, disjoint, ordered, and permit only one canonical next
  phase; the caller does not choose progress or page order;
- partial preparation either moves/mints no liability or has a deterministic,
  authenticated compensation path;
- the final liability or value mutation reauthenticates every live mutable
  prestate and stays in one transaction;
- the checkpoint advances last in intermediate transactions and closes last in
  the final mutation;
- external journals preserve signed packets and ambiguity, but onchain state is
  the authority for resume;
- permissionless cleanup is available only after terminal state or expiry and
  refunds only immutable destinations; and
- an Upgrade either refuses while live checkpoints exist or preserves the
  exact recovery ABI for them.

Each transaction must separately prove complete unique keys <=64, fully signed
wire bytes <=1,232, every SBF frame <4,096, and source-bound compute. An ALT can
discharge only the address-serialization part of the packet condition.

## Series SHA adapter requirement

`programs/dclutch-series-shadow-sbf/src/evaluator.rs` directly imports
`sha2::{Digest, Sha256}` in production code. It hashes the request and the
capability, account-profile, request-profile, transition, effect, strategy, and
lifecycle artifact slices. This is the same hidden software-SHA class already
removed elsewhere.

The tree already has the correct trust boundary:
`dclutch-sha256-adapter::digestv` uses Solana's `sol_sha256` syscall on SBF and
`sha2` in host tests, with identical SHA-256 output. Existing evidence measured
a 4,288-byte digest at 456,008 CU in software versus 2,234 CU through the
syscall. That observation motivates the change but does not establish Series'
final CU margin.

The Series conversion should:

1. express each existing streaming preimage as the exact ordered slice list;
2. combine adjacent scalar fields into a buffer that outlives `digestv` so the
   syscall does not pay a needless per-slice floor;
3. keep host cross-backend vectors proving the old and new digests byte-exact;
4. assert the final Series ELF's dependency/symbol census no longer contains a
   software SHA implementation; and
5. run the real Series caller, its frame diagnostic, and its own 20-seed result.

## Fractional integration requirement

Fractional V3 is a bounded contract/operator rung, not a live capability. Its
maximum candidate topology is already 40 locks and 700 signed v0 bytes, and its
retirement cursor makes `K` change transaction count instead of transaction
width. The missing work is semantic and exterior:

- a Fractional-specific Claims child that owns native Claims plus Token-2022
  and optional Custody mutation without importing Rational receipt semantics;
- the matching Trading dispatch and exact generated AccountProfile,
  RequestProfile, EffectProgram, and ExecutionStrategy;
- a producer-root version that authenticates the V3 cursor and lifecycle rent;
- one-coordinate Position/reserve and Mint closure, followed by fixed final
  Core/Rent closure; and
- a real caller-backed SBF campaign.

`dclutch-fractional-claim-contract` and
`dclutch-fractional-claims-kernel` still use `sha2`. They are not currently
linked into the permanent Claims ELF, so this is not a claim about today's
shipped Claims CU. Before either crate becomes SBF-reachable, every digest used
by the adapter layer must route through `dclutch-sha256-adapter`; the pure
kernel may remain runtime-agnostic only if the program adapter supplies and
reauthenticates the digest without creating a second semantic truth.

## Measurement order after the architecture freezes

1. Build every final shipped and frame-only SBF link from one clean source
   archive; record byte length and SHA-256.
2. Run the emitted-stack diagnostic over every link and parse the output even
   when the compiler exits successfully.
3. Run one exact caller-backed seed for each changed route and capture outer and
   child CU as diagnostics, along with complete locks and signed packet bytes.
4. Repair topology or CU failures in batches, then rerun the changed links.
5. Run M-61 only on the exact final links. Report each route as pass count out
   of 20 plus the arithmetic mean across those 20 seeds.
6. Only then bind the release manifest and compare the candidate ELF identities
   with the public devnet ProgramData capture.

This ordering avoids both failure modes seen in prior work: treating a
single-draw diagnostic as a margin, and measuring a large ELF whose package or
role was not the one the release actually dispatches.
