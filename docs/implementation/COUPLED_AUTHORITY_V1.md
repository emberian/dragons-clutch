# Narrow Candidate Submission V1

Status: **IMPLEMENTED NARROW SUBMISSION; EXECUTABLE SELECTION-AUTHORITY STOP;
RECEIPT FREEZE AND GENERAL CLEARING REMAIN STOP**

`Intent::SubmitDirectPage` closes one dependency immediately before the live
coupled settlement slice. It permissionlessly constructs a canonical
`CandidateRecord` in `SUBMITTED` status and its canonical `CandidateFeed` from
authenticated, frozen, fully funded state. It does not claim that the candidate
is valid, best, selected, or entitled to settle.

## Exact admitted shape

The constructor accepts only all of the following:

- one frozen Epoch with exactly one page, two populated orders, two owners, and
  two outcomes;
- one frozen page at index zero, no tombstones, whose page digest is recomputed
  from every slot and whose one-page `order_set` is recomputed and equals the
  Epoch commitment;
- two single-Egg orders at live indices zero and one;
- opposite sides, distinct owners, one outcome, equal nonzero quantity, one
  equal interior limit, zero flags, and zero minimum fill;
- both the common price and its simplex complement are exact members of the
  frozen price grid;
- one exact, untouched, zero-fee `ACTIVE` reservation per order, including
  market, Epoch, owner, order identity/generation, page, Terms, grid, policy,
  outcome width, family, side, and complete initial/remaining envelope; and
- request sequence zero and an authenticated Clock sysvar for
  `submitted_slot`.

The resulting price vector sets the traded outcome to the common limit and the
other outcome to `price_scale - limit`. Both fills equal both order quantities.
The feed declares exactly one direct slice, with buy/sell indices derived from
the two stored sides. A caller chooses none of these facts.

This deliberately excludes partial fills, AON/minimum-fill policy, portfolios,
virtual split/merge legs, nonzero fees, tombstones, multiple pages, and more
than two outcomes.

## Authority and account binding

The account list is fixed at eleven roles:

1. writable signer funding rent, with no economic or selection privilege;
2. read-only canonical Epoch PDA;
3. read-only canonical frozen PriceGrid PDA;
4. read-only canonical page PDA;
5. read-only canonical reservation PDA for live order zero;
6. read-only canonical reservation PDA for live order one;
7. writable, creatable Candidate PDA;
8. writable, creatable CandidateFeed PDA;
9. System program;
10. Rent sysvar; and
11. Clock sysvar.

Every existing state account must be program-owned, non-executable, exact
length, and read-only. Every address is rederived from its decoded semantic
owner. The Candidate address is

```text
PDA("dragons-clutch:candidate:v1", epoch, candidate_identity)
```

and the feed address is

```text
PDA("dragons-clutch:cand-feed:v1", epoch, candidate_identity)
```

where `candidate_identity` is the layout-owned SHA-256 identity over the Epoch,
market, order length, outcome width, full price vector, virtual pair, and AON
mask. The CandidateFeed repeats and is checked against those coordinates and
the frozen order-set identity.

The same content therefore has one Candidate account and one feed account.
Both targets must still be zero-data System accounts. A prior SOL transfer to a
predictable target is a donation, not initialization: construction funds only
the rent shortfall, then PDA-signs System `Allocate` and `Assign`. An existing
program-owned account makes replay refuse before any CPI.

## What `SUBMITTED` means here

The Candidate and feed carry zero for every claimed score component and for the
relation's 128-bit claimed digest. Those are visibly unverified claims, not an
approximation.

The Epoch authenticates a 32-byte policy identity but not the exact
`FrozenPolicyV1` preimage the relation consumes. The relation also uses five
`u64` domain identities where the account plane owns `Hash32`, and no injective
mapping is specified. Computing a supposedly canonical relation digest here
would therefore invent both a policy and an identity projection. The
constructor refuses to do that.

Accordingly:

- Candidate status remains `SUBMITTED`;
- Epoch phase remains `FROZEN`;
- no SettlementReceipt or FinalPot is created;
- `SettlePage` still refuses this output because it requires `CLEARED`,
  `SELECTED`, and a pre-frozen receipt; and
- once-only account creation is not candidate-window closure or selection.

## Atomicity and measured execution

All state, content, address, sequence, target, System, Rent, and Clock checks run
before the first CPI. Candidate creation precedes CandidateFeed creation only
because the System program creates one account per CPI. Any later CPI, borrow,
or codec failure rejects the whole Solana transaction.

Real-SBF tests in
`programs/clutch-sbf/svm-tests/tests/coupled_authority.rs` prove:

- one-lamport prefunding of both predictable PDAs succeeds and retains exact
  rent funding;
- the resulting Candidate is `SUBMITTED`, score/digest claims are zero, and
  the feed contains the exact two fills and one direct slice;
- the Epoch, page, grid, and both reservations are byte-identical before and
  after success;
- replay refuses and leaves both created accounts byte-identical;
- reservation substitution and cross-outcome books refuse before creation;
- a funder able to create Candidate but unable to fund CandidateFeed causes a
  real nested System-program failure, and the bank rolls Candidate creation,
  funder lamports, and all frozen inputs back exactly; and
- the successful prefunded transaction consumed **1,249,403 transaction compute
  units** under a 1,400,000-unit budget in the recorded local run.

The exact local ELF for that run was 807,656 bytes with SHA-256
`0804455f27a773cb874c8d4686408900d60b5ebfb101616d4e1df70b3df54321`.
It names the joined working tree, not a release or deployment artifact.

The SBF build emits no frame diagnostic for `submit_direct_page`,
`load_direct_submission_plan`, or `prepare_direct_submission`. Historical
diagnostics in unused buffered/reference routines remain separate debt.

## Exact V2 dependency map

The next transition may set `VERIFIED`/`REFUSED`, select a candidate, freeze
receipts, and move the Epoch to `CLEARED` only after all of these have one
semantic owner:

1. **Policy preimage account.** A versioned, fixed-layout encoding of every
   `FrozenPolicyV1` choice and parameter, with a canonical digest that must equal
   `Epoch.policy`. There may be no default-by-omission constructor.
2. **Relation-domain identity bridge.** Either the relation consumes the
   account plane's full `Hash32` identities, or a specified injective encoding
   binds every Hash32 to its relation-domain value. Truncation is forbidden.
3. **Stable verifier checkpoint.** `ClearWorkV1` needs a stable no-alloc account
   codec or another resumable verifier whose persisted body is not a cast of
   `repr(Rust)` memory.
4. **Claim completion and verification.** The exact policy/domain must
   recompute fills, score components, the 128-bit relation digest, and the
   explicit pairing witness. V2 must specify whether zero submission claims are
   completed once or replaced by a distinct verified record; it may not silently
   relabel them as matched.
5. **Candidate-window closure.** A frozen deadline/Clock rule, complete
   candidate-set commitment, and once-only state transition must prove no later
   candidate can enter before the program says “best valid submitted
   candidate.” A single content PDA is not a closed set.
6. **Reservation-set closure.** The complete funded reservation set must bind
   the complete frozen live book, including a future exact live cardinality for
   tombstone-bearing sets.
7. **Complete entitlement freeze.** Every selected fill must become an exact
   receipt/pot entitlement before `CLEARED`; no order page or later resolution
   may create transfer authority. Fees require the authenticated policy
   preimage and named recipient.
8. **Terminal state machine.** Partial/multi-slice cumulative consumption,
   virtual pots, lapse/refund, and a proof that every reservation, receipt, and
   pot closes exactly once remain separate required transitions.

Until that map lands, general relation verification, partials, portfolios,
fees, lapse, receipt construction, Epoch `CLEARED`, and terminal closure are
explicit STOPs.

The follow-up authority audit is executable and records two concrete policy
counterexamples plus the minimum immutable policy/domain/window design in
[DIRECT_SELECTION_AUTHORITY_V1.md](DIRECT_SELECTION_AUTHORITY_V1.md). Its key
distinction is that this exact one-page/two-order constructor already closes
the live order-to-reservation set locally; the earlier remaining blockers are
the frozen policy preimage, full-width relation domain, and closed candidate
window. No live ABI was added merely to return a guaranteed refusal.
