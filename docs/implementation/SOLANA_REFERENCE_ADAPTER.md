# Offline Solana reference adapter

Status: implemented offline reference semantics; not an SVM program, deployment
artifact, token adapter, or chain-readiness claim.

The `programs/solana-reference` crate is the narrow executable seam between the
hostile-byte-facing layouts in `programs/solana-layout` and the pure transition
semantics in `crates/clutch-kernel`, with the typed window evidence plane of
`crates/clutch-accumulator`. It is `no_std`, safe Rust, allocator-free, and
dependency-limited to those three local crates. It has no Solana SDK,
entrypoint, syscall, `AccountInfo`, CPI, Token-2022 implementation, RPC, key,
signing, deployment, or network behavior.

## Implemented reference subset

- Market-init validation decodes the Realm, Profile, Market, Hoard, Position,
  supply-ledger, reference kernel, external-balance, and replay accounts;
  refuses a Realm Profile whose collateral policy is not frozen; decodes the frozen
  `CreateMarket` intent; checks the canonical market and outcome IDs; checks
  account linkage, versions, bumps, empty initial supplies, and profile limits;
  and runs the kernel invariant checker. It does not authorize or execute
  creation. `CreateMarket` through the transition function fails closed.
- `Split` debits position cash, checks the immutable collateral cap, credits
  hoard collateral, and invokes the complete-set kernel split.
- `Merge` is the exact inverse of `Split`, and it is the transition
  PROJECT.md's central promise names: "the complete set can always be
  recombined into its collateral before resolution". It invokes the
  complete-set kernel merge — which debits every internal balance and the
  Hoard collateral, refusing `InsufficientCollateral` before
  `InsufficientBalance` — and then credits position cash by the released
  collateral with a checked add. It carries `Split`'s phase discipline
  verbatim (`lifecycle == 0` and `close_state == 0`), and two of its
  differences from `Split` are decisions rather than sign flips:
  - **No collateral-cap check.** A merge lowers `hoard.collateral_atoms`, so
    the post-state cannot cross a ceiling the pre-state was under. Checking
    the cap here would be worse than redundant: a market already above its cap
    would be unable to unwind, and unwinding is the one direction that must
    always stay open.
  - **The cash credit follows the kernel step**, where `Split`'s debit
    precedes it. A debit is the precondition of a mint; a credit is the
    consequence of a burn. That is the order `RedeemInternal` already credits
    a payout in, so every cash credit in this adapter lands after the kernel
    step that justified it.
- `Materialize` and `Dematerialize` invoke the kernel and move quantities
  between internal balances and an explicit reference-only external shadow.
  They do not mint or burn an SPL token.
- `Resolve` is **evidence-gated**, not authorized. `apply` supplies no evidence
  plane and therefore still refuses `ResolutionEvidenceUnavailable`;
  `apply_with_evidence` runs the full gate below. A signer is still not evidence
  of anything: resolution is permissionless, and no key can substitute for any
  element of the gate.
- `RedeemInternal` is gated on a resolution record that is bound to the market's
  immutable terms, selected a payout inside that frozen set, and agrees exactly
  with the resolved kernel state. Caller-supplied bytes that merely claim the
  market is resolved still refuse.
- Every transition consumes the exact next sequence in a replay account bound
  to the position generation. State-account aliases, wrong program owners,
  wrong keys, non-writable accounts, unsigned actors, and wrong bumps refuse.

The frozen layout did not yet contain payout vectors, external balances, or
replay sequences. This lab therefore defines three fixed reference-only
accounts. They make the missing state explicit without pretending to freeze a
deployment ABI. Aggregate supply is no longer one of them: the adapter now
consumes the frozen `SupplyLedgerAccount` as the two-term aggregate, and the
reference kernel account's payout set is bound to the immutable
`TermsAccount`. The Market lifecycle, Hoard collateral, Position internal
balances, and reference accounts each retain one semantic owner; the adapter
reconstructs a kernel state only on the stack.

This reference is a **multi-position adapter under the CLO-DELTA-V1
delta-accounting invariant** (design and refusal-set analysis in
[`MULTI_POSITION_CLOSURE.md`](MULTI_POSITION_CLOSURE.md)). The supply ledger
is the only counted truth. Per transition, for every active outcome, the
adapter checks:

```text
C0  a newly initialized position triple is provably zero
C1  supply.internal[o] + supply.external[o] == kernel.total_supply[o]  (pre and post)
C2  position.internal[o] <= supply.internal[o]  and
    external.balance[o]  <= supply.external[o]  (presented triple, pre and post)
C3  supply' == supply - position_pre + position_post   (checked; moved, never overwritten)
```

C2 is deliberately one-sided: over-counting in the ledger is the conservative,
locked-collateral direction, while a position exceeding the ledger term — the
counterfeit-claim direction — refuses. C3 with C1-post forces the kernel's
aggregate effect to equal the presented position's effect, making the
represented-balances property an inductive invariant of histories rather than
a scan. The residue a purely local check cannot catch — position bytes
fabricated outside any history that nonetheless satisfy C1/C2 — is named in
the design doc and belongs to the PDA/authentication obligations (1-3, 9),
not to this invariant. Initialization additionally requires zero position
cash and reserved cash, an open position, and a zero replay sequence; ledger
generation is the market accounting era (proposed SVM rule: ledger PDA with
no close path).


## The resolution evidence gate

`Action::Resolve` and `Action::RedeemInternal` used to refuse unconditionally.
They now refuse unless a caller supplies a `ResolutionEvidence` whose every
element checks out. **Evidence-gated is not authenticated.** The feed identity
bytes a window is bound to are opaque 32-byte values; no crate here can relate
them to a real adapter, program, deployment, subject, quote, or orientation.
Nothing authenticates the observations that were folded. What the gate
establishes is narrower and exact: the payout index is the one a sealed,
complete, mature, correctly-domained fold of the *supplied* observations selects
under terms the market's own digest already commits to.

The fail-closed default is a missing code path, not a flag. `apply` takes no
evidence argument, so both actions return `ResolutionEvidenceUnavailable`
whatever the account bytes claim, and a regression asserts exactly that against
the pre-evidence fixture.

### What is bound, and what V1 pins instead

`MarketAccount.terms` is the domain-separated digest of the `TermsAccount` body,
so the payout set, feed, observation grid, expected bucket range, maturity
horizon, and the coverage/repair/failure policy triple are all committed facts.
Everything the derivation reads must come from there or be canonical; nothing
may be caller-supplied, because an unbound boundary table would let a caller
choose the payout.

> **Superseded 2026-08-18:** the `TermsAccount` v3 revision landed and every
> pin in the table below became a stored, digest-committed field (see the
> addendum at the end of this document). The table is retained as the record
> of what V1 pinned and why.

`RESOLUTION_EVIDENCE_PLAN.md` §2.1 proposed a separate 432-byte artifact
digested into `MarketAccount.terms`. The landed `TermsAccount` took that digest
slot with a different body, so §2.1's artifact cannot also be that digest. V1
therefore **pinned** the fields the frozen terms did not carry, and each pin
was an obligation on a later `TermsAccount` revision rather than a default:

| Field | V1 value | Consequence |
| --- | --- | --- |
| statistic | `STAT-TERMINAL-01` | TWAP/sampled-extrema markets are not expressible yet |
| ambiguity policy | `AMBIG-REFUSE-01` | uncertainty never becomes a definite claim |
| partition | ordinal cells `C_i = [i, i+1)`, closed at the top, `n = outcome_count` | a threshold market (the plan's `boundaries = [50]` example) cannot resolve here |
| payout map | identity | requires `outcome_count <= payout_count`, else `R-09` |
| source/evaluator version | `1` | any other version is `MismatchedFeed` |
| source-adapter/feed-spec id | both `TermsAccount.feed` | any other identity is `MismatchedFeed` |
| repair generation | `0` under `GEN-EXACT-01` | any repaired window is `MismatchedGeneration` |
| coverage policy | only `COMPLETE_REQUIRED` | `BOUNDED_GAPS` refuses: the terms carry no gap bound to read |

`FAIL-UNIFORM-REFUND-01` and `FAIL-EXTENDED-WINDOW-02` are recorded but never
executed: the first inherits §P1-A and the second needs a successor domain the
terms cannot name. A refused derivation refuses the whole transition.
`STAT-RELATIVE-TERMINAL-TWAP-05` stays inadmissible on the `u128` headroom
argument of plan §2.3. `AMBIG-COMPATIBLE-SET-02` and
`GEN-FINAL-AT-MATURITY-02` refuse as unimplemented.

### The gate, in order

1. state and evidence account metadata: owner program, expected key, aliasing
   against every other role and the actor, and mutability — the immutable terms
   artifact may never be presented writable, and the resolution record must be
   writable exactly when the action writes it;
2. a signature from *some* actor, because a transaction has a fee payer; no key
   is privileged and resolution is otherwise permissionless;
3. the market must be `Active` and the kernel phase must agree;
4. `TermsAccount` decodes, its stored bump matches, and `binds_market` holds —
   the artifact is self-certifying, so editing any field changes its digest;
5. the reference kernel account's payout set must equal the frozen one;
6. `ResolutionAccount` decodes, binds the market and the terms, and is still
   unresolved;
7. `ResolutionTerms::from_market_terms` must derive an admissible V1 terms;
8. the observation records fold through the accumulator's own
   `Open -> Mature -> Sealed` machine — there is no "sealed" flag on the wire;
9. `WindowResult::check_domain` against the terms-derived domain;
10. `derive_payout` selects exactly one payout; and
11. the request must be asking for exactly that index.

Redemption re-runs 1, 4, 5, 6 with the record *resolved*, requires the record's
window identity and payout index to match, requires the resolved kernel state to
agree with the record, and refuses a window blob outright: re-deriving a payout
at redemption time would create a second place a payout can be decided.

### Refusal taxonomy

Adapter-level refusals, each reachable and each distinct:

| Refusal | Raised when |
| --- | --- |
| `ResolutionEvidenceUnavailable` | no evidence plane was supplied at all |
| `UnexpectedEvidence` | evidence supplied for a layout intent, or a window blob supplied for a redemption |
| `ImmutableAccountWritable` | the terms artifact, or a redemption's resolution record, was presented writable |
| `NotWritable` | a resolve was asked to write a read-only resolution record |
| `WindowIdentityUnavailable` | the trusted `WindowId` binding was zero |
| `TermsBindingMismatch` | the terms artifact is not the one `MarketAccount.terms` binds |
| `PayoutSetMismatch` | the reference kernel payout set is not the frozen terms set |
| `ResolutionBindingMismatch` | the record is not bound to this market, these terms, or this window |
| `ResolutionAlreadyRecorded` | a payout was already selected for this market |
| `ResolutionNotRecorded` | redemption without a record that selected a payout |
| `PayoutIndexMismatch` | the request asks for a payout the evidence does not derive |
| `MismatchedState` | resolved kernel state disagrees with the record |
| `Window(_)` | the accumulator refused; see below |
| `Resolution(_)` | the derivation refused; see below |

`Window(_)` carries the accumulator's own named reason, so these stay
distinguishable: `IncompleteDomain` (a truncated prefix cannot seal),
`NotMature` (covered is not the same fact as mature), `CoverageRefused` (an
explicit gap under `COMPLETE_REQUIRED`), `NonContiguous` (reordered, duplicated,
or skipped buckets), `NonMonotoneCursor`, `ZeroIdentity`,
`UnknownCoveragePolicy`, `InvalidMaturity`, `InvalidRange`, and the six
`check_domain` field reasons.

`Resolution(_)` carries the `R-01..R-11` registry of plan §2.5, mapped by
`ResolutionRefusal::class` so a taxonomy code never depends on enum
discriminants: `R-01` terms digest mismatch, `R-02` terms malformed or naming an
unimplemented policy, `R-03` partition malformed, `R-04` window domain mismatch
(carrying the field), `R-05` statistic unsupported or inadmissible, `R-06`
ambiguous interval, `R-07` no accepted coverage, `R-08` ambiguous denominator,
`R-09` payout index outside the frozen set, `R-10` market not active, `R-11`
checked comparison overflow.

`NotSealed`, `NotMature`, and `IncompleteDomain` can never reach the derivation:
a `WindowResult` cannot exist in those states, so they surface in the fold.

## Exact byte evidence

The tests build full pre-state byte arrays for all seven state accounts and
compare the full post-state arrays. The split vector independently changes only
the named little-endian fields:

- Hoard collateral at bytes `98..106`;
- Position outcomes 0 and 1 at `74..82` and `82..90`;
- Position cash at `202..210`;
- Kernel aggregate supplies 0 and 1 at `38..46` and `46..54`;
- Supply ledger internal terms 0 and 1 at `75..83` and `83..91`; and
- Replay sequence at `74..82`.

The merge vector is read against the split-11 post-state rather than the
fixture, so every number in it is the inverse of the split vector at the same
offsets: Hoard collateral `11 -> 7`, both position outcomes `11 -> 7`, position
cash `89 -> 93` (credited, not debited), both kernel aggregates `11 -> 7`, both
ledger internal terms `11 -> 7`, sequence `1 -> 2`, and the Market and the
external shadow untouched. A round-trip test then runs `split q -> merge q` and
requires every one of the seven accounts back at its pre-split bytes **except**
the replay account, whose sequence must read 2 — the residue a state machine
that forgot two consumed transitions would not have. It repeats over quantities
1, 7, and 93; 93 rather than 100 because `PositionAccount::validate` refuses
`reserved_cash_atoms > cash_atoms`, which makes the fixture's 7 reserved atoms a
floor under the cash a split may spend rather than an annotation.

The full lifecycle vector runs `create -> split 20 -> observe/seal -> resolve ->
redeem_internal 20` and compares every account array at every step. Resolve
changes exactly the Market lifecycle byte at `131`, the kernel phase and
resolved-payout bytes at `34` and `35`, the replay sequence, and the resolution
record's window identity at `98..130`, sealed feed cursor at `130..138`, sealed
end bucket at `138..146`, repair generation at `146..154`, recorded slot at
`154..162`, and payout index at `162`. Redemption then zeroes the Hoard, the
winning outcome's position, kernel, and ledger entries, credits position cash
back to 100, and returns the record byte-identical: redemption never edits its
own authority. The same vector also checks the exact 144-byte canonical window
preimage the terms name, field by field, because this crate owns no hash
primitive and publishes the preimage rather than a digest.

The materialize/dematerialize test checks aggregate-supply neutrality across
both ledger terms. An exact signer-bypass regression proves that an arbitrary
signed actor cannot resolve without evidence; it then forges internally coherent
“resolved” bytes and proves redemption still refuses. Adversarial tests cover
account aliases, layout versions, runtime program ownership, stored bumps, stale
replay, replay overflow, unsupported intents, absent resolution evidence, absent
signatures, an unfrozen collateral policy, every window-evidence codec refusal,
truncated/immature/gapped/reordered folds, eight wrong-domain fields, payout-set
substitution on both sides of the terms digest, and the `R-01..R-11` classes of
the pure derivation.
One regression freezes the exact counterfeit-claim counterexample: internal
outcome balance one, aggregate supply zero, Hoard zero, then `Materialize(0, 1)`.
It must return `AggregateClosureMismatch` without producing post-state. A
bounded trace test checks the equality after splits and materializations across
quantities 1 through 16.

`Merge` carries the same counterfeit regression in the direction that matters
more: a split mints against cash the position must actually hold, but a merge
*pays cash out*, so a forged complete set would be a counterfeit converted into
collateral. Both C2 (position above its ledger term) and C1 (ledger terms no
longer summing to the kernel aggregate) refuse it before any write. `Merge`'s
own refusal battery pins `InsufficientCollateral` for a single-position market
over-merging, `InsufficientBalance` for a position merging against a *second*
position's claims — the fault the kernel's collateral-before-balance order makes
reachable only in a multi-position market — a closing position, and a resolved
market. `Merge` is correspondingly no longer on the unsupported-intent list;
what remains outside the reference subset is the feed and order families
(`FeedAdvance`, `PlaceOrder`, `CancelOrder`, `SettlePage`), and the test now
sweeps three of them.

Run the evidence gates independently because this crate is intentionally not in
a root workspace:

```sh
cargo test --manifest-path programs/solana-reference/Cargo.toml --offline --locked
cargo clippy --manifest-path programs/solana-reference/Cargo.toml --offline --locked --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path programs/solana-reference/Cargo.toml --offline --locked --no-deps
cargo fmt --manifest-path programs/solana-reference/Cargo.toml -- --check
```

## Obligations before an SVM adapter or token CPI

An eventual SVM adapter must establish all of the following before a token CPI
is added. The reference crate does not establish them.

1. Derive every Realm, Profile, Market, Hoard, Position, outcome-mint,
   external-token, replay, feed, and authority PDA from frozen domain-separated
   seeds and compare both address and canonical bump. Caller-supplied expected
   keys are not sufficient.
2. Authenticate each `AccountInfo` owner, executable bit, signer bit, writable
   bit, data length, discriminator, version, rent/lifecycle state, and uniqueness
   by role before borrowing mutable data.
3. Prove that no writable alias, duplicate account, remaining-account shuffle,
   owner substitution, close/reopen generation reuse, or instruction replay can
   cause one logical debit or credit to be applied twice.
4. Freeze the transaction-atomic ordering: validate and compute first, perform
   token effects, then commit program state, with every CPI failure rolling back
   all effects under actual SVM semantics.
5. Select and pin the exact SPL Token or Token-2022 program and extensions.
   Validate mint authority, freeze authority, decimals, transfer fees, hooks,
   delegates, withheld fees, interest-bearing/confidential extensions, account
   owner, and token-account mint. Unsupported extensions must refuse.
6. Prove one-to-one conservation between internal-to-external materialization
   and actual mint/burn or escrow movements. The external shadow in this lab is
   not token evidence and must not survive as a second balance truth.
7. Prove collateral conservation across user token accounts and the Hoard,
   including checked decimals, transfer-fee behavior, exact observed balance
   deltas, collateral caps, and the rule that principal is never a fee, bounty,
   rent source, reserve, or treasury source.
8. Authenticate the resolution path. The typed, non-discretionary half now
   exists offline: checked maturity, a sealed `WindowResult` produced by driving
   the accumulator's own state machine, feed/source/generation identity,
   market-terms binding, and payout-set membership, with no signer or
   caller-supplied binding able to substitute for any of it. What remains is the
   *authenticated* half — every observation is still caller-supplied and every
   feed identity is still opaque. Items 15 through 19 below are the specific
   pieces.
9. Freeze replay semantics across Solana transaction replay, durable nonces,
   instruction duplication, batch retries, position close/reopen, and program
   upgrades. The local sequence account is only a model.
10. Bound stack, heap, compute units, account count, serialization cost, CPI
    count, and transaction size on the pinned SBF toolchain. The large offline
    post-state witness is not an onchain mutation strategy.
11. Reconcile concurrent multi-position aggregate supply. Each owner position
    must change the single market aggregate exactly once; closure must prove
    that all internal and external balances are represented without scanning an
    unbounded set. The offline adapter now implements the CLO-DELTA-V1 inductive
    closure (see MULTI_POSITION_CLOSURE.md); the SVM design still owes the
    PDA/authentication half that makes fabricated-history bytes unreachable.
12. Establish upgrade authority, program-data identity, initialization race
    handling, account closing destinations, migration/version rules, emergency
    posture, and immutable-profile behavior without introducing discretionary
    seizure or undercollateralization.
13. Add differential fixtures connecting pure-kernel results, host adapter
    results, SBF execution, and token-program post-balances. Simulations and
    fixtures remain evidence of those exact cases, not mainnet correctness.
14. Name the exact verification claims and trust boundary. Any Verus/Rocq proof
    must pin source digest and toolchain and must state that SVM runtime,
    serialization, PDA derivation, CPI, token programs, and deployment remain
    outside the pure-kernel theorem unless separately proved.
15. Authenticate observations and feeds. `FeedIdentity`'s two 32-byte values are
    opaque; the source-adapter admission dossier of `ACCUMULATOR_PLAN.md` §9 has
    no entries. Until it does, a `WindowResult` is honest evidence about a fold
    and never evidence about a source, and the whole gate rests on whoever
    assembled the observation records.
16. Persist and authenticate window evidence. Today the only constructor of a
    `WindowResult` is an in-process `WindowAccumulator`, so the evidence must be
    produced by the same call that consumes it. A persisted, authenticated
    window-result account and its decoder do not exist, and the offline blob
    codec here is not one.
17. Give the window identity one owner. `ResolutionAccount.window` is written
    from `EvidenceBindings.window_id`, a trusted binding beside the PDA keys and
    bumps, because neither this crate nor `clutch-accumulator` owns a hash
    primitive. A real adapter must derive it as
    `HASH(WINDOW_DOMAIN_TAG || WindowDomain::encode_canonical())`;
    `expected_window_preimage` publishes the exact 144 preimage bytes so an
    independent recomputation cannot disagree about what is hashed.
18. Carry the derivation's remaining inputs in immutable terms. A `TermsAccount`
    revision must add a statistic id, an ambiguity policy id, a coverage-policy
    parameter, a repair generation, source/evaluator versions, a source-adapter
    identity, and a boundary table with its payout map, all inside the digest.
    Until then V1 pins them (see the table above) and refuses everything else,
    which means a threshold market simply cannot resolve here.
19. Bind the collateral policy, not just its freeze discipline. This adapter
    refuses an unfrozen Realm Profile and requires the digest to be nonzero
    exactly when `PROFILE_FLAG_POLICY_FROZEN` is set. It does **not** recompute
    the child digest `D_col` from an actual decoded 266-byte collateral policy,
    which is `RESOLUTION_EVIDENCE_PLAN.md` §3.4 obligation 3: a well-formed
    frozen Profile can still commit to another Realm's collateral policy and
    nothing here would notice. Obligations 2 and 4 of that section — the 64-byte
    parent encoder/decoder and the Rust golden vectors — are also unwritten;
    only the exact-length requirement on `canonical_profile_hash` landed.

Until those obligations have checked artifacts, the correct description is
“offline reference transition adapter with an evidence-gated, unauthenticated
resolution path.”

---

## Addendum 2026-08-18: obligation 18 discharged, obligation 19 wired

The unified `TermsAccount` v3 revision landed (design:
`DISTRIBUTIONAL_CLAIMS_DESIGN.md` §6 and its implementation addendum; codec:
`programs/solana-layout`, `account_version::TERMS = 3`, 1,656 bytes, digest
domain `dragons-clutch/terms/v2`).

**Obligation 18 is discharged.** The immutable terms now carry — inside the
self-certifying digest — the statistic id, ambiguity policy id, edge policy
id, coverage-policy parameter, repair generation, source/evaluator versions,
source-adapter identity, the knot vector (whose degree-0 prefix is the
boundary table), the payout map, and the basis degree.
`ResolutionTerms::from_market_terms` decodes every one of them; no V1 pin
remains. A threshold market resolves here, end-to-end through
`apply_with_evidence`, with the plan §2.6 worked example landed as a test.
`BOUNDED_GAPS` coverage is expressible (the gap bound is a stored field; a
zero bound still refuses). Degree-1 (hat-basis) markets additionally resolve
whenever the derived, validated weight vector is a member of the frozen
preset set — the kernel still resolves by index, and the missing
`resolve_with_vector` transition is the one named residue, refusing
`R-16 DerivedVectorUnrepresentable` fail-closed for every non-member vector.
Degrees 2-3 refuse as unimplemented variants.

**Obligation 19 is wired on the reference side.** `validate_market_init`
takes the 266 collateral-policy bytes and the terms artifact as evidence
inputs: it recomputes the child digest via
`collateral::verify_collateral_binding` (a foreign well-formed policy, a
bit-flipped stored digest, and hostile policy bytes each refuse with their
own class — the §3.4 adversarial battery exists), requires the founding
`MarketAccount.collateral_cap` to equal the terms' digest-committed
`collateral_cap` (nonzero by codec, so a market with no cap decision cannot
be founded), and refuses a cap above `check_market_cap`'s mint ceiling.
The on-chain `CreateMarket` writes the terms' cap and re-checks the equality,
but still checks only freeze discipline against the Profile: no account in
its frozen twelve-account plane carries the policy bytes, so the on-chain
binding half of obligation 19 remains open and named there.

The evidence-gate refusal taxonomy is unchanged in classes; its numeric
projection in `clutch-sbf` is no longer lossy (the `0x0050-0x005f` block in
`error.rs`). The compute-cost defect this crate's five-decode gate shape
caused on-chain was closed by `TermsAccount::decode_unchecked`/`decode_into`
in the layout crate; this adapter's own semantics did not change — resolve
and redeem still bind terms, payout set, and record at the same points in the
same order.

What did **not** move: no refusal was relaxed; evidence is still
unauthenticated (obligations 15-17 stand); resolution still consumes one
sealed `WindowResult` through the same gate; and the kernel API residue above
is the only derived-mode gap.
