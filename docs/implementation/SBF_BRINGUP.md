# SBF bring-up: the instruction set, executed against the reference oracle

Status: **host-differential evidence for eight instruction families**
(Split, Merge, Materialize, Dematerialize, CreateMarket, FeedAdvance,
evidence-gated Resolve, RedeemInternal), each mirroring the offline
reference adapter's semantics with byte-level differential tests, **plus
SVM execution evidence for seven of the eight**: every one of those
families except `Resolve` now executes inside a local Agave bank against
regenerated fixtures, and every writable account it touches comes back
byte-identical to the oracle's post-state.

`Resolve` is the exception and it is a **measurement, not an omission**:
on the pinned runtime it consumes the entire 1 400 000-unit
per-transaction compute ceiling and is aborted, so it cannot be executed
on-chain today at all. `RedeemInternal` executes but costs 1 356 878
units — 97% of the same ceiling. Both numbers are recorded in
[Results](#results) and the cause is one named defect in
`clutch-solana-layout`.

`PlaceOrder` landed on-chain after these fixtures were regenerated and
has **no SVM leg**; cancellation and batch settlement remain honest
stubs. Not a complete program, not audited, not a deployment
authorization, and not mainnet, devnet, or testnet evidence. A stub
reads no account, writes no byte, and reports no success.

This document records what the `programs/clutch-sbf` lane built, what actually
ran, what failed, and what is deferred. Numbers below are from the run recorded
in [Results](#results); re-running `programs/clutch-sbf/scripts/run_bringup.sh`
reproduces them.

## What this lane converts

Before this lane, `programs/solana-reference` was an *offline reference
transition adapter*: a pure function from caller-asserted account metadata and
account bytes to post-state bytes, with no entrypoint, no runtime, no address
derivation, and no write-back. The open question was whether that shape survives
contact with an actual SVM.

This lane answers that for an instruction set. It produces:

1. a real deployable SBF ELF with a `entrypoint` symbol, built reproducibly by
   the pinned `cargo-build-sbf`;
2. a program that validates hostile `AccountInfo` metadata, derives and checks
   every program address, decodes through `clutch-solana-layout`, transitions
   through `clutch-kernel`, and writes back into runtime account data;
3. a **differential result per family**: for eight accepting transactions
   across seven families, every writable account after a real SVM execution is
   byte-identical to the oracle's post-state;
4. a **round trip through the bank**: `Split` and `Merge` as two instructions of
   one transaction, after which every account except the replay sequence is
   byte-identical to its pre-state, sequenced by the bank rather than by the
   harness;
5. sixteen adversarial refusals executed in the SVM, whose numeric
   `ProgramError::Custom` codes are compared against the projection the offline
   adapter's refusal class maps to; and
6. one **negative** result recorded as evidence: `Resolve` does not fit a Solana
   transaction on this runtime.

It does **not** establish correctness of the protocol, of the kernel, of the
layout, or of any economic claim. It establishes that these instructions survive
the account-facing boundary on the pinned toolchain, and names the one that does
not.

## Layering, and where the logic is not

`programs/clutch-sbf/program` contains no semantic or economic logic:

- balances, supplies, collateral, and invariants come from `clutch-kernel`;
- byte ownership comes from `clutch-solana-layout` and from the reference-only
  account codecs in `clutch-solana-reference`;
- what the program adds is only what those crates cannot have: account
  authentication, program-address derivation, and write-back.

`clutch-kernel`, `clutch-solana-layout`, and `clutch-solana-reference` are
unmodified path dependencies. The program depends on `clutch-solana-reference`
**only** for the reference-only `KernelAccount`, `ExternalAccount`,
`ReplayAccount`, and `Request` codecs, so those byte layouts keep exactly one
semantic owner. It never calls `clutch_solana_reference::apply`; the transition
composition is written independently, which is what makes the differential a
comparison of two adapters over one kernel rather than a comparison of a
function with itself.

That the offline `apply` is genuinely absent from the artifact is checked, not
assumed. A linker map of the built ELF lists exactly these symbols from the
reference crate, and nothing else:

```text
clutch_solana_reference::KernelAccount::encode
clutch_solana_reference::KernelAccount::decode
clutch_solana_reference::KernelAccount::validate_shape
clutch_solana_reference::ExternalAccount::encode
clutch_solana_reference::ExternalAccount::decode
clutch_solana_reference::ReplayAccount::encode
clutch_solana_reference::ReplayAccount::decode
clutch_solana_reference::Request::decode
clutch_solana_reference::resolution::derive_payout
clutch_solana_reference::resolution::ResolutionTerms::from_market_terms
clutch_solana_reference::resolution::ResolutionTerms::validate
clutch_solana_reference::resolution::ResolutionTerms::cell_of
clutch_solana_reference::resolution::ResolutionTerms::cell_of_ratio
```

`ExternalAccount::encode` joined the list when the seam plane grew
`Materialize`/`Dematerialize` and began writing the shadow rather than only
reading it. The five `resolution::` symbols are the *pure* terms-to-payout
derivation, which the observation and resolution plane calls directly; that
module owns no account bytes and no evidence plane, so calling it is not the
offline `apply` leaking in. `apply`, `apply_with_evidence`, `apply_inner`,
`validate_market_init`, `validate_position_init`, `resolve_from_evidence`,
`redeem_from_evidence`, and `DecodedState::decode` are all absent.

**A caveat about that check.** `cargo-build-sbf` emits platform-tools frame
diagnostics for eight functions that are *not* in the map:
`clutch_solana_reference::{apply_inner, DecodedState::decode,
validate_market_init, validate_position_init, resolve_from_evidence,
redeem_from_evidence}` and
`clutch_solana_layout::OrderPageAccount::{decode, decode_on_grid}`. The stack
analyser runs over the objects before `--gc-sections` drops them, so these are
diagnostics about dead code that never reaches the image — the map is the
authority on what is in the artifact, not the diagnostic list. They are still
worth reading as a resource signal for the day any of those functions *does*
need an on-chain counterpart, and the `OrderPageAccount` pair is the same stack
finding that blocks the `orders_batch` family. **No `clutch_sbf` function
produces a frame diagnostic.**

`clutch-accumulator` entered the graph late, as a new dependency of
`clutch-solana-reference`. It used to reach the ELF with **no** symbols; it now
reaches it with thirteen (`WindowAccumulator::{observe, absorb, result}`,
`WindowDomain::{new, check_against}`, `WindowResult::check_domain`,
`FeedIdentity::new`, `Summary::{append, combine, validate}`, `combine_extrema`,
and two byte helpers), because the observation and resolution plane drives the
accumulator's `Open -> Mature -> Sealed` state machine on-chain rather than
re-deriving its algebra. That is a deliberate dependency, not a leak, and it is
why `clutch-accumulator` is now named directly in the program's `Cargo.toml`.

Reproduce with
`RUSTFLAGS="-C link-arg=-Map=<file>" cargo-build-sbf --manifest-path programs/clutch-sbf/program/Cargo.toml`.

## Program module map and lane ownership

The program is split so that per-instruction lanes can work in parallel without
two lanes editing one file. Every path below is under
`programs/clutch-sbf/program/src/`.

| file | owns | shared? |
| --- | --- | --- |
| `lib.rs` | crate docs, module list, the `entrypoint!` expansion | shared — coordinator |
| `error.rs` | the stable numeric refusal codes | shared — append only, never renumber |
| `seeds.rs` | the proposed PDA seed schema for all 15 protocol accounts and the 3 reference-only ones | shared — append only |
| `accounts.rs` | hostile-metadata authentication, PDA comparison, and every account decoder | shared — foundation lane |
| `dispatch.rs` | request decoding and routing on the action tag | shared — one arm per family |
| `instructions/split.rs` | `Intent::Split` | **implemented** |
| `instructions/merge_materialize.rs` | `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize` | **implemented** — all three, through the `split.rs` seam plane |
| `instructions/market_init.rs` | `Intent::CreateMarket` | **implemented** — validated initialization-write over pre-created accounts |
| `instructions/observe_resolve.rs` | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` | **implemented** — `Resolve` does not fit a transaction, see deferred check 15 |
| `instructions/orders_batch.rs` | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SettlePage` | `PlaceOrder` **implemented**; the other two **blocked**, see below |

A lane owns its `instructions/*.rs` file outright. It touches the shared files
only to *append*: a new refusal code at the end of `error.rs`, a new seed
function in `seeds.rs`, a new reader in `accounts.rs`, and its own arm in
`dispatch.rs`. Renumbering an existing refusal code or changing an existing seed
is an ABI decision and is not a lane's call.

Each module's own doc comment carries the decisions its lane has to make before
writing any code; `orders_batch.rs` in particular records a hard blocker that
has to be cleared in `clutch-solana-layout` first.

### The request is decoded before any account is validated

The single-instruction program authenticated its fixed nine-account list before
looking at instruction data at all. A program with an instruction *set* cannot:
how many accounts an instruction takes, which are writable, and what each one
must be is a function of which instruction it is, and that lives in the data. So
`dispatch::process` decodes the reference request envelope first and routes on
the action tag, and the family module then runs its own account plane.

The consequence is named rather than hidden. For any request that decodes,
nothing changed — the same checks run in the same order and produce the same
refusal codes, which is what the differential below re-confirms. For a request
that does *not* decode and is **also** presented with bad accounts, the codec
refusal now wins where an account refusal used to. Both are refusals, and no
state is read or written in either case.

### Refusal discipline for the stubs

A family module refuses with `NotYetImplemented` (`0x0017`) unless the offline
reference adapter refuses the same action for a *stronger, structural* reason,
in which case this program mirrors that reason exactly. The table below is the
history of that discipline and where each family stands now:

| action | blanket refusal it used to return | now |
| --- | --- | --- |
| `CreateMarket` | `AuthorizationUnavailable` `0x000f` — no authority model existed | **implemented**, permissionless, over pre-created accounts |
| `Resolve`, `RedeemInternal` | `ResolutionEvidenceUnavailable` `0x0010` — the typed evidence plane had no on-chain counterpart | **implemented** behind the PROPOSED `0x47` evidence buffer |
| `Merge`, `Materialize`, `Dematerialize` | `UnsupportedInstruction`, then `NotYetImplemented` | **implemented**, see the correction below |
| `FeedAdvance` | `NotYetImplemented` `0x0017` | **implemented** behind the PROPOSED `0x48` feed page |
| `PlaceOrder` | `NotYetImplemented` `0x0017` | **implemented**; no SVM evidence yet (deferred check 16) |
| `CancelOrder`, `SettlePage` | `NotYetImplemented` `0x0017` | still refused: no adapter, offline or on-chain, joins these layouts to a transition |

`Merge`, `Materialize`, and `Dematerialize` previously refused
`UnsupportedInstruction` (`0x000e`), then `NotYetImplemented` (`0x0017`). All
three now run through the ten-account seam plane of `instructions/split.rs` and
refuse only what the transition itself refuses.

**Correction.** This table used to justify that row with "the offline adapter
implements all three". For `Merge` that sentence was **wrong** and had been
wrong since it was written: `clutch_solana_reference::apply_inner` had no
`Intent::Merge` arm at all, so the intent fell through to
`Err(Error::UnsupportedIntent)` — this despite `clutch_kernel::MarketState::merge`
existing and despite PROJECT.md's central promise that a complete set can always
be recombined into its collateral before resolution. The program mirrored the
refusal rather than accepting a transition its own oracle refused, and left an
alarm test (`merge_is_refused_by_both_adapters`) designed to fail the day the
reference grew the arm. The reference has since grown it — with the cash
direction named, which was the missing semantics — the alarm fired, and both
sides now implement `Merge` as the exact inverse of `Split`. The two decisions
that are not sign flips (no collateral-cap check on the way down; the cash
credit lands *after* the kernel step) are recorded in
[`SOLANA_REFERENCE_ADAPTER.md`](SOLANA_REFERENCE_ADAPTER.md) and in the
`merge_materialize.rs` module docs.

## Proposed PDA seed schema

`programs/clutch-sbf/program/src/seeds.rs` is a **proposal**, not a frozen ABI.
Changing any byte changes every account address, so freezing it is an ABI
decision for a later gate.

| role | seeds |
| --- | --- |
| Realm | `"dragons-clutch:realm:v1"`, `realm_hash` |
| Profile | `"dragons-clutch:profile:v1"`, `realm_hash`, `profile_hash` |
| Market | `"dragons-clutch:market:v1"`, `realm_hash`, `market_id` |
| Hoard | `"dragons-clutch:hoard:v1"`, `market_id` |
| Position | `"dragons-clutch:position:v1"`, `market_id`, `owner` |
| kernel aggregate | `"dragons-clutch:kernel:v1"`, `market_id` |
| external shadow | `"dragons-clutch:external:v1"`, `market_id`, `owner`, `generation` (u64 LE) |
| replay sequence | `"dragons-clutch:replay:v1"`, `market_id`, `owner`, `generation` (u64 LE) |
| supply ledger | `"dragons-clutch:supply:v1"`, `market_id` |
| feed head | `"dragons-clutch:feed:v1"`, `feed_id` |
| immutable terms | `"dragons-clutch:terms:v1"`, `realm_hash`, `terms_digest` |
| price grid | `"dragons-clutch:grid:v1"`, `realm_hash`, `grid_digest` |
| resolution record | `"dragons-clutch:resolution:v1"`, `market_id` |
| epoch | `"dragons-clutch:epoch:v1"`, `market_id`, `epoch_index` (u64 LE) |
| order page | `"dragons-clutch:page:v1"`, `epoch_id`, `page_index` (u16 LE) |
| candidate | `"dragons-clutch:candidate:v1"`, `epoch_id`, `candidate_digest` |
| final pot | `"dragons-clutch:pot:v1"`, `epoch_id` |
| settlement receipt | `"dragons-clutch:receipt:v1"`, `epoch_id`, `candidate_digest`, `slice_index` (u16 LE) |

Four of those choices are load-bearing enough to state out loud, and all four
are proposals:

- **Terms and the price grid are content-addressed and Realm-namespaced.** Their
  own digest is a seed, so one terms artifact can be shared by many markets and
  can never be silently re-authored at the same address. The stored bump is
  outside both digests, which is what lets an account derived *from* a digest
  still carry the bump that derivation produced.
- **The epoch is seeded on its index, not its identity**, because
  `canonical_epoch_id` already derives the identity from exactly
  `(market, index)`. Seeding on the index keeps the address derivable by a
  caller that has not yet fetched the account.
- **Order pages, candidates, pots, and receipts are seeded under the epoch
  identity, which already binds the market**, so the market is not repeated.
- **One resolution record per market, and one pot per epoch.** A second address
  for either would be a second place a payout or a pot could be decided.

Two further proposals ride along and are equally unfrozen:

- the 32-byte `PositionAccount::owner` identity is interpreted as the raw bytes
  of the owning wallet address, which is what lets an authenticated signer be
  bound to a stored position; and
- `generation` is part of the external-shadow and replay seeds, so a
  close/reopen produces different addresses rather than reusing a sequence.

Every address is recomputed with `find_program_address` and compared against the
supplied account key, and every stored bump is compared against the canonical
bump. Caller-supplied expected keys are never accepted. Two accounts carry no
bump field in their frozen layout — Profile and the reference-only kernel
aggregate — so those two are checked by address only; that gap is listed under
[Deferred checks](#deferred-checks).

## Instruction and account set

The **seam plane** carries all four Hoard/Position intents — `Split`, `Merge`,
`Materialize`, `Dematerialize` — in the reference adapter's `Request` envelope
(`0xd1`, version, u64 sequence, layout action, `u16` length, frozen `Intent`
bytes). One account list serves all four, because the offline reference adapter
routes all four through one `TransitionMetadata` / `StateBytes` /
`ExpectedBindings` triple. The list, the check order, and the write-back live in
`instructions/split.rs`, and `instructions/merge_materialize.rs` calls into
them; a second copy of the list would be a second place for the seam's writable
set to drift. The list is fixed at exactly ten, in this order, and a different
count refuses:

| # | role | signer | writable |
| --- | --- | --- | --- |
| 0 | actor (must be the position owner) | yes | — |
| 1 | Realm | no | no |
| 2 | Profile | no | no |
| 3 | Market | no | yes |
| 4 | Hoard | no | yes |
| 5 | Position | no | yes |
| 6 | kernel aggregate | no | yes |
| 7 | external shadow | no | yes |
| 8 | replay sequence | no | yes |
| 9 | supply ledger | no | yes |

The external-balance shadow is **not** omitted. It could have been — neither
`Split` nor `Merge` changes it — but the CLO-DELTA-V1 obligations are stated
over *both* ledger terms, so C1's two-term closure and C2's representation bound
need it present, and keeping it makes the differential an exact seven-account
byte comparison.

The supply ledger at index 9 is the tenth account, appended after the replay
account exactly as `ExpectedBindings::supply` is the last state binding of the
reference adapter. It arrived with the CLO-DELTA-V1 port; the retired
single-position equality `internal + external == total_supply` that preceded it
made a market holding a second position unrepresentable.

The other instruction families own their own account planes. They are listed
here because the SVM differential now drives all of them, so a reader
reproducing the run needs to know what each transaction carries:

| family | accounts | shape |
| --- | --- | --- |
| seam (`Split`, `Merge`, `Materialize`, `Dematerialize`) | 10 | the table above |
| `CreateMarket` | 12 | creator, Realm, Profile, terms (read-only); Market, Hoard, Position, kernel, external, replay, ledger, resolution record (writable, and required to arrive **all-zero**) |
| `Resolve` / `RedeemInternal` | 12 | actor; Market, Hoard, Position, kernel, external, replay, ledger (writable); terms (read-only); resolution record (writable for a resolve, read-only for a redemption); feed head (read-only); caller-supplied evidence buffer (read-only) |
| `FeedAdvance` | 3 | actor; feed head (writable); caller-supplied observation page (read-only) |

The two caller-supplied buffers are the only accounts in the program with no
canonical address, deliberately: their bytes are the claim rather than the
state, so binding their identity would suggest the bytes are trusted. They are
still required to be program-owned, non-executable, and read-only. Their formats
are the **PROPOSED** `0x47` evidence buffer and `0x48` feed page defined in
`instructions/observe_resolve.rs`.

### Checks the program performs

Metadata, before any borrow: exact account count; actor signature; pairwise key
distinctness across all ten roles (including actor-versus-state aliasing);
program ownership of all nine state accounts; non-executable bit; declared
writability per role, including refusing a *writable* Realm or Profile; exact
data length per role.

Derivation: canonical address and canonical bump for Realm, Market, Hoard,
Position, external shadow, replay, and the supply ledger; canonical address for
Profile and the kernel aggregate; and `MarketAccount::hoard_bump` against the
derived Hoard bump.

Decoding: every account through its frozen codec, which re-checks length,
discriminator, version, enums, identities, and canonical padding.

Linkage, mirroring `validate_links` in the offline adapter and adding the
Realm/Profile edges that the offline adapter only checks at market
initialization: Realm/Profile/Market identity agreement, profile version
agreement, `realm.max_outcomes == MAX_OUTCOMES`, `outcome_count <=
max_outcomes`, Market/Hoard/Position/kernel/external/replay/ledger identity
agreement, Realm and outcome-count agreement with the ledger,
owner and generation agreement across Position, external shadow, and replay,
lifecycle-versus-phase agreement, `lifecycle <= 1`, payout outcome count against
market outcome count, and outcome count within the kernel bound.

State: zero padding beyond the active outcome count in every balance vector;
the CLO-DELTA-V1 obligations C1 (two-term closure against the kernel aggregate)
and C2 (representation bound on the presented triple) before the transition and
again after it, with C3 (the ledger moved by exactly the position delta) in
between; exact replay sequence and a checked increment.

Transition: signer identity equal to `position.owner`; intent market and owner
bound to the stored accounts. For `Split`: `lifecycle == 0` and
`close_state == 0`; checked collateral cap; checked position-cash debit; then
`clutch_kernel`'s `MarketState::split`. For `Merge`: the same phase discipline;
**no** cap check, because a merge lowers the hoard and cannot cross a ceiling
the pre-state was under; then `MarketState::merge`; then the checked
position-cash *credit*, which follows the kernel step because it is the
consequence of a burn rather than the precondition of a mint. For `Materialize`
and `Dematerialize`: the caller-named destination or source must equal the
already-derived external-shadow address, then the matching kernel transition.
Every `MarketState` transition runs its own invariant check over the prospective
state before its first write.

Write-back: Hoard, Position, kernel aggregate, supply ledger, external shadow,
and replay are re-encoded through their codecs. Market is left untouched because
no seam transition changes it; the differential still compares all seven state
accounts against the reference adapter's re-encoded post-state, so a codec that
failed to round-trip would fail the comparison rather than be hidden by a
rewrite.

Every refusal maps to a stable `ProgramError::Custom(code)`; the table is in
`programs/clutch-sbf/program/src/error.rs`.

### Deferred checks

Each item below is a real gap, not a formality. Two have moved since they were
first written and say so in place; the rest are untouched.

Relative to what `programs/solana-reference` already does:

1. **Closed.** `Merge`, `Materialize`, and `Dematerialize` were refused
   (`NotYetImplemented`); all three now run through the seam plane, are covered
   by the host differential, and — since the fixture regeneration — each has its
   own accepting SVM transaction on the ten-account plane, plus a `Split`/`Merge`
   round trip sequenced by the bank. `Merge` closed on *both* sides at once,
   because the gap was mis-stated: the offline adapter did not implement it
   either (see the correction under
   [Refusal discipline](#refusal-discipline-for-the-stubs)).
2. **Closed as stated, and replaced by a narrower gap.** `validate_market_init`
   now has an on-chain counterpart: `CreateMarket` founds a market and its
   post-state is compared byte for byte against an independent re-encode that
   the reference's own `validate_market_init` accepts. What it still does **not**
   do is *create* accounts: all twelve arrive already created, rent-funded,
   correctly sized, and — for the eight it writes — all-zero. The
   `system_instruction::create_account` CPI, the rent-exemption computation, and
   the `invoke_signed` seed plumbing are unwritten and untested, and the
   all-zero precondition is what keeps that missing half detectable rather than
   assumed. Genesis pre-creates the eight zeroed accounts.
3. **Closed.** `CreateMarket` no longer refuses `AuthorizationUnavailable` and
   `Resolve`/`RedeemInternal` no longer refuse `ResolutionEvidenceUnavailable`;
   all three are implemented and all three are driven in the SVM. What replaced
   this item is item 15: `Resolve` is implemented and *cannot be executed*
   because of its compute cost.

Relative to obligations 1-4 of `SOLANA_REFERENCE_ADAPTER.md`:

4. Rent-exemption and account lifecycle state are not checked (obligation 2).
   The fixture funds every account well above the rent-exempt minimum, which
   hides the question rather than answering it.
5. Profile and kernel-aggregate accounts have no stored bump to compare, so
   their derivation check is address-only (obligation 1).
6. Account creation, closing, close/reopen generation reuse, and the
   destination of closed-account lamports are untested (obligations 3 and 12).
7. Transaction-level replay is untested. The program consumes the local replay
   sequence, but no committed transaction is ever sent, so Solana transaction
   replay, durable nonces, instruction duplication within one transaction, and
   batch retries are all outside this evidence (obligations 3 and 9).
8. Transaction-atomic ordering is asserted, not demonstrated: the program
   validates and computes before its first write, and relies on SVM rollback to
   discard partial writes on a later failure. No test forces a failure after the
   first write (obligation 4).
9. No token program, CPI, mint, or escrow behaviour exists (obligations 5-7).
10. **Closed on both sides.** Multi-position aggregate closure used to be
    refused by representation. It is now carried by the CLO-DELTA-V1 two-term
    ledger in both adapters, and the ledger is compared on-chain on every
    accepting transaction of every family (obligation 11, and item 13).
11. **Partly closed.** The resource envelope is now eight compute measurements
    across seven families plus one measured ceiling exhaustion, and every
    transaction's serialized size is recorded; see
    [Resource envelope](#resource-envelope). Heap, worst-case outcome counts,
    worst-case page sizes, and any envelope over more than the two-outcome
    fixture remain unmeasured (obligation 10).
12. **Partly closed.** The account plane now has 20 host-side unit tests
    covering metadata authentication in both directions (foreign owner,
    executable bit, read-only-arrived-writable, writable-arrived-read-only,
    wrong length, aliasing, missing signature, wrong account count), the
    key-and-bump comparison, a short/long/mistagged/misversioned battery against
    every account decoder, and the three CLO-DELTA-V1 closure primitives. What
    is still untested on the host is the *derivation*, and only that: off-chain
    address derivation is not compiled into the crate (see
    [Toolchain and offline constraints](#toolchain-and-offline-constraints)), so
    `split::seam` takes an already-derived `Bindings` value as a parameter and
    the host differential supplies the same trusted bindings the offline
    adapter takes as `ExpectedBindings`. The *transition* is therefore covered
    on the host for all four seam intents — request decoding, metadata
    authentication, every linkage and closure check, the kernel step, and the
    write-back — and the SVM differential remains the only test of the one
    thing that gap names: that the derived address is the canonical one.
13. **Closed.** This item tracked the supply ledger being loaded but unused,
    then the host side closing while the SVM side regressed: `harness/` emitted
    a *nine*-account `Split` transaction after the instruction had grown to ten,
    so no emitted transaction exercised the CLO-DELTA plane at all and
    `simulate.py` never compared the ledger.

    The fixtures were regenerated. The seam plane's SVM transactions now carry
    ten accounts, the supply ledger is compared byte for byte on every accepting
    transaction of every family, and `Merge`, `Materialize`, and `Dematerialize`
    each have their own transaction. The three CLO-DELTA obligations are
    therefore exercised on-chain and not only on the host: C1 and C2 run over the
    pre-state and again over the post-state, and C3 is visible as the ledger
    delta in the compared bytes.

14. **An order page cannot be decoded on-chain at all.** See
    [Stack findings](#stack-findings) — this blocks the whole
    `orders_batch` family and the fix belongs in `clutch-solana-layout`.

15. **`Resolve` does not fit a Solana transaction.** MEASURED on the pinned
    runtime: with `SetComputeUnitLimit` at the 1 400 000-unit per-transaction
    ceiling, the program consumes every unit granted and the runtime aborts it
    with `ProgramFailedToComplete`. No post-state and no gate refusal past the
    signature check is observable on-chain. `RedeemInternal` executes at
    1 356 878 units, 97% of the same ceiling.

    The cause is named in `observe_resolve.rs`'s own module docs and is not a
    surprise: the resolve path decodes the immutable terms artifact **five**
    times and `TermsAccount::decode` recomputes a SHA-256 over the 1.2 KiB terms
    body on every call; a redemption decodes it four times, which is the
    corroborating measurement rather than a guess. The fix is a facts or
    `decode_unchecked` API in `clutch-solana-layout` — the same crate that owns
    deferred check 14 — and it is not correct to fix it by skipping a binding.

    The gate asserts the exhaustion rather than skipping the case, so an
    instruction that became cheap enough to finish turns the check red and this
    section has to be re-measured instead of being left stale.

16. **`PlaceOrder` has no SVM leg.** It landed on-chain in commit `5cb4ad1`,
    after these fixtures were regenerated, and the offline reference adapter has
    no `PlaceOrder` transition to be an oracle for one. Driving it needs a
    writable epoch and order-page plane in genesis and an oracle decision that
    is not a harness lane's to make.

## How the transition is executed

The harness never signs. Every identity it uses — program id, fee payer, actor,
a stranger, and an imposter address — is a System-program PDA of a fixed literal
seed, so the fixture is reproducible and this lane holds no key material of any
kind.

`scripts/run_bringup.sh` starts one `solana-test-validator` bound to loopback
with the ELF at the chosen program id and **55 accounts loaded at genesis**,
then `scripts/simulate.py` sends every transaction in `plan.json` to
`simulateTransaction` with `sigVerify: false`, `replaceRecentBlockhash: true`,
and an `accounts` request for exactly that transaction's writable accounts, and
compares the returned data against the oracle post-state the harness wrote to
`expected/`.

### Why the plan carries several markets

`simulateTransaction` never commits, so **every transaction runs against
genesis**. A family whose precondition is another family's post-state therefore
needs its own genesis, and each such genesis is produced by running the offline
reference adapter forward from an empty market rather than by hand-writing the
intermediate bytes — a pre-state a lane typed out would be a state neither
implementation produced. Five market planes exist, all in one Realm, all bound
to one Profile, one price grid, and one immutable terms artifact, because none
of those four codecs binds a market identity:

| plane | nonce | genesis pre-state | drives |
| --- | --- | --- | --- |
| `seam` | 9 | empty, active, replay sequence 0 | `Split`, the `Split`/`Merge` round trip, the three `Split` refusals, and the `CreateMarket` re-initialization refusal |
| `held` | 10 | `apply(Split 20)` | `Merge`, `Materialize`, `Resolve` |
| `shadow` | 11 | `apply(Split 20)` then `apply(Materialize 0, 3)` | `Dematerialize` |
| `redeem` | 13 | `apply(Split 20)` then `apply_with_evidence(Resolve 1)` | `RedeemInternal` |
| `create` | 14 | eight all-zero accounts at their canonical addresses | `CreateMarket` |

Two feed identities exist for the same reason: `Resolve` needs a **read-only**
feed head whose cursor is already past the window's maturity bound, and
`FeedAdvance` needs a **writable** one sitting at the start bucket. One account
cannot be both.

The one place a real sequence executes is the `roundtrip` transaction, which
carries `Split` and `Merge` as two instructions of one transaction so that the
**bank** sequences them and the second instruction reads the first's writes.
That is what makes the round-trip claim a bank fact rather than a harness fact.

### Three families need a raised compute limit

`Resolve`, `RedeemInternal`, and `CreateMarket` do not fit the runtime's default
200 000-unit budget, so their transactions carry a `SetComputeUnitLimit`
instruction at the 1 400 000-unit per-transaction ceiling ahead of the program
instruction, exactly as a real caller would. The raised number is itself the
measurement; see [Resource envelope](#resource-envelope).

What that does establish: the ELF is loaded and executed by an Agave bank; the
runtime serializes real account data into the VM and writes the program's
mutations back; the `is_signer`, `is_writable`, `owner`, and `executable` bits
the program reads are the runtime's, taken from the transaction message header
and the loaded accounts; program-address derivation runs as the real
`sol_try_find_program_address` syscall; and compute is metered.

What it does **not** establish: no Ed25519 signature is verified, so "the actor
signed" is a message-header fact rather than a cryptographic one; nothing is
committed to a ledger, so no fee is paid, no state persists, and no replay or
durable-nonce behaviour is exercised; and a simulated bank is not a cluster.

## Toolchain and offline constraints

Verified on this host, `aarch64-apple-darwin`:

```text
solana-cli 4.0.2 (src:549805f3; feat:6ff76655, client:Agave)
cargo-build-sbf 4.0.0
platform-tools v1.53
rustc 1.89.0
```

### The panamax mirror does not exist on this host

The lane brief named a full panamax mirror at `~/crates.io/full`, to be used
through a registry source replacement in `programs/clutch-sbf/.cargo/config.toml`.
It is not there:

```text
ls: /Users/ember/crates.io/full/: No such file or directory
```

`lcrio --source panamax` likewise resolves nothing. The only offline crate
source available is the ordinary Cargo download cache under `~/.cargo/registry`,
so `.cargo/config.toml` sets `[net] offline = true` instead of a source
replacement: a resolution that would need a fetch fails loudly rather than
silently reaching for crates.io. **Reproducing this build on a machine without
that cache requires network access or a real mirror**; `Cargo.lock` pins exactly
what to fetch.

### Dependency selection was forced by what the cache holds

`cargo-build-sbf` runs `cargo metadata`, which resolves for *every* platform and
requires a downloaded `.crate` archive for every package in the graph — even
packages that this platform never builds. The cache holds 2058 archives but 4147
unpacked sources, so several otherwise-reasonable dependency sets are
unbuildable offline. Two blockers, verbatim:

```text
error: failed to download `curve25519-dalek-derive v0.1.1`

Caused by:
  attempting to make an HTTP request, but --offline was specified
```

```text
error: failed to download `solana-define-syscall v5.2.0`

Caused by:
  attempting to make an HTTP request, but --offline was specified
```

The first is why the host-side `curve25519` backend of `solana-pubkey` is not
enabled anywhere in this workspace: it is needed only off-chain, it is never
built on `aarch64`, and enabling it makes `cargo metadata` fail. The consequence
is that `seeds::find` is a syscall under `target_os = "solana"` and
`unimplemented!()` off-chain, and the harness derives addresses out of process
with the pinned `solana find-program-derived-address` command instead. Seed
prefixes still come from `clutch_sbf::seeds`, so the seed bytes keep one source
of truth.

The second was resolved by pinning `solana-define-syscall` to `=5.1.0`, the
version whose source this host actually has. Its `.crate` archive is missing, so
the workspace patches that one dependency to
`programs/clutch-sbf/vendor/solana-define-syscall-5.1.0`, a verbatim copy of the
published crate with provenance and the crates.io checksum recorded in
`vendor/PROVENANCE.md`. This is build plumbing for an offline host, not a fork:
deleting the directory and the `[patch.crates-io]` entry restores an ordinary
registry dependency.

### Pins

| crate | version | source |
| --- | --- | --- |
| `clutch-accumulator` | 0.1.0 | local path (transitive, via `clutch-solana-reference`) |
| `clutch-kernel` | 0.1.0 | local path |
| `clutch-solana-layout` | 0.1.0 | local path |
| `clutch-solana-reference` | 0.1.0 | local path |
| `clutch-sbf` | 0.1.0 | local path |
| `clutch-sbf-harness` | 0.1.0 | local path |
| `five8` | 1.0.0 | registry |
| `five8_const` | 1.0.0 | registry |
| `five8_core` | 1.0.0 | registry |
| `solana-account-info` | 3.1.1 | registry |
| `solana-address` | 2.6.1 | registry |
| `solana-define-syscall` | 4.0.1 | registry |
| `solana-define-syscall` | 5.1.0 | vendored path (see above) |
| `solana-program-entrypoint` | 3.1.1 | registry |
| `solana-program-error` | 3.0.1 | registry |
| `solana-program-memory` | 3.1.0 | registry |
| `solana-pubkey` | 4.2.0 | registry |
| `solana-sanitize` | 3.0.1 | registry |

## Harness ladder: what was tried, in order

**(a) `solana-program-test` in-process bank — unavailable.** The crate is absent
from this host's offline cache in every version, along with `litesvm` and
`mollusk`. `cargo-test-sbf` is installed but has nothing to run.

**(a′) `agave-ledger-tool program run` — blocked by an upstream defect.** This
subcommand executes an SBF ELF against a mocked runtime from a JSON account
description and would have been the cleanest in-process harness. In the pinned
release it fails before reading the input, for every invocation:

```text
$ agave-ledger-tool program run -l <ledger> --input <file>.json --output json <program>.so
[... INFO agave_ledger_tool] agave-ledger-tool 4.0.2 (src:549805f3; feat:6ff76655, client:Agave)
error: The argument 'accounts_index_limit' wasn't found
```

The subcommand reads an argument it never registers, so no ledger, flag, or
input fixes it; `--accounts-index-limit` is rejected as an unexpected argument.
Building an in-process harness from `solana-program-runtime`'s
`mock_process_instruction` was also considered and rejected: the 4.2.1 sources
are unpacked on this host but not one of the ~40 crates in that graph has a
`.crate` archive.

**(b) `solana-test-validator` on loopback — reached, and this is the recorded
result.** See [How the transition is executed](#how-the-transition-is-executed).

**(c) ELF plus a manual plan — not needed.**

## Results

Command: `programs/clutch-sbf/scripts/run_bringup.sh`, one validator session,
recorded on `aarch64-apple-darwin` with `HEAD` at `9ac1d27` and a working tree
dirty in exactly four files.

**The tree-state caveat, stated precisely.** The gate prints `git rev-parse HEAD`
and `git status --porcelain` before it builds, and this run reported:

```text
== source pin ==
9ac1d273b9e7a1c4382e4228309b13c0f1149f0b
tree=DIRTY (the ELF digest below names this working tree, not a commit)
   M docs/implementation/SBF_BRINGUP.md
   M programs/clutch-sbf/harness/src/main.rs
   M programs/clutch-sbf/scripts/run_bringup.sh
   M programs/clutch-sbf/scripts/simulate.py
```

The four dirty files are this document, the harness, and the two scripts.
**None of them is an input to the ELF**: `cargo-build-sbf` is invoked on
`program/Cargo.toml`, and the program crate does not depend on the harness, on
anything under `scripts/`, or on documentation. The last commit touching any ELF
input — `programs/clutch-sbf/program`, `programs/solana-layout`,
`programs/solana-reference`, `crates/clutch-kernel`, `crates/clutch-accumulator`
— is **`5cb4ad1`**, so the digest below is pinned to `5cb4ad1`'s sources even
though the tree was not clean.

What cannot be claimed from a dirty tree is a clean-tree reproduction of the
whole *run*: re-running the gate from `5cb4ad1` alone would use the pre-
regeneration harness and emit the old nine-account plan. Once the harness and
scripts land, re-run the gate on a clean tree and re-record both the digest and
the commit here.

Earlier in this same session the digest moved between runs — `7a02ca02…`,
`3ad336c5…`, `921b514c…` — because a concurrent lane was editing
`program/src/accounts.rs` and `program/src/instructions/orders_batch.rs` while
the gate ran. That is the ELF correctly tracking its inputs, not a
reproducibility failure, and it is why the source-pin block exists at all.

### Reproducible ELF

Built twice into fresh target directories:

```text
pass 1  sha256=921b514c3ee5eba104e5063c684e4ce3672a09be73e969eeada2d5c0426f838f  bytes=332368
pass 2  sha256=921b514c3ee5eba104e5063c684e4ce3672a09be73e969eeada2d5c0426f838f  bytes=332368
sbf_reproducibility=PASS
```

The same digest was reproduced a third time from two further independent target
directories outside the gate, so the agreement is not an artifact of the gate's
own directory layout. The previously recorded digest
(`42d553132b0a22ebffd374c85d12a444e4ca8c3e99aa211322c5b8a947467cdd`, 102 568
bytes) predates the whole instruction set and is retired; the artifact is now
3.2× larger because seven more instruction families are in it.

Dynamic symbols, re-read from this ELF with the pinned `llvm-readelf`: ten
`.dynsym` entries, exporting `entrypoint` and `custom_panic` and importing the
syscalls `sol_try_find_program_address`, `sol_log_`, `sol_panic_`, `abort`,
`sol_memset_`, `sol_memcpy_`, and `sol_memcmp_`. There is no CPI syscall and no
token program reference, because there is no CPI and no token code — and that
is unchanged by seven more instruction families landing.

### Genesis

55 accounts, all program-owned, loaded at genesis by one validator invocation:
the Realm-wide plane (Realm, Profile, price grid, immutable terms, two feed
heads), five market planes of eight accounts each, the batch-auction plane no
implemented instruction touches (epoch, order page, candidate, final pot,
settlement receipt), three caller-supplied buffers, and the imposter replay
account. The Profile is **frozen** — the policy flag is set and the digest is
nonzero — because `CreateMarket` refuses a Realm that has not frozen its
collateral policy; that is a shape claim and this harness owns no 266-byte
collateral policy to back it.

### Differential against the oracle, per family

Every accepting transaction's writable accounts are compared byte for byte. A
role marked *unchanged* is additionally required to come back equal to its
pre-state, and a role marked *changed* is required to differ from it: an
expectation that happens to equal the pre-state cannot pass by accident.

| transaction | family | oracle | accounts compared | result |
| --- | --- | --- | --- | --- |
| `split` | `Split` | `reference::apply` | 7 | 7/7 MATCH |
| `roundtrip` | `Split`+`Merge` | `reference::apply` ∘ `apply` | 7 | 7/7 MATCH |
| `merge` | `Merge` | `reference::apply` | 7 | 7/7 MATCH |
| `materialize` | `Materialize` | `reference::apply` | 7 | 7/7 MATCH |
| `dematerialize` | `Dematerialize` | `reference::apply` | 7 | 7/7 MATCH |
| `resolve` | `Resolve` | `reference::apply_with_evidence` | 8 written, 0 compared | **not executable** (compute ceiling) |
| `redeem` | `RedeemInternal` | `reference::apply_with_evidence` | 8 | 8/8 MATCH |
| `feed-advance` | `FeedAdvance` | accumulator fold + `FeedAccount` codec | 1 | 1/1 MATCH |
| `create-market` | `CreateMarket` | layout re-encode + `reference::validate_market_init` | 8 | 8/8 MATCH |

52 account comparisons, 52 matches, 0 mismatches. The eight `resolve`
expectations are written to disk on every run but cannot be compared, because
the transaction never completes.

**The oracles are not all equally strong, and the plan says so per case.** For
the seam plane and for `RedeemInternal` the oracle is a second, independently
written implementation of the same transition, so a match is two adapters
agreeing. For `CreateMarket` there is no reference transition: the expectation is
an independent re-encode through the frozen `clutch-solana-layout` codecs from
the intent and the terms artifact alone, following the PROPOSED initial values
`market_init.rs` documents (zero collateral cap, zero created slot, generation
zero, the Hoard PDA as its own authority, the terms artifact's payout set, an
unresolved record), and that re-encode is then required to satisfy the
reference's own `validate_market_init`. For `FeedAdvance` there is no reference
adapter at all: the expected cursor is what `clutch_accumulator`'s `Summary`
fold lands on for the same records, re-encoded through the frozen `FeedAccount`
codec.

### The round trip through the bank

`roundtrip` is one transaction carrying two instructions — `Split` 5 at sequence
0 and `Merge` 5 at sequence 1 — so the **bank**, not the harness, sequences them
and the `Merge` reads the `Split`'s writes:

```text
accept roundtrip  program_units=155800 per-instruction [77924, 77876] bytes=750
  differential seam.market    MATCH (unchanged)
  differential seam.hoard     MATCH (unchanged)
  differential seam.position  MATCH (unchanged)
  differential seam.kernel    MATCH (unchanged)
  differential seam.external  MATCH (unchanged)
  differential seam.replay    MATCH (changed)
  differential seam.supply    MATCH (unchanged)
```

Six of the seven accounts are byte-identical to the pre-split genesis. The
seventh is the replay account, whose sequence advanced 0 → 2. The harness asserts
that shape at build time as well, so a round trip that stopped closing would fail
`cargo test` before it ever reached a validator.

### Refusals

Every refusal is executed in the SVM and its numeric `ProgramError::Custom` code
compared against an expected value, with the offline adapter's own refusal class
for the same situation recorded beside it. Sixteen executed cases, at least two
per family, plus one that cannot be executed:

| transaction | code | offline reference |
| --- | --- | --- |
| `split-unsigned` | `0x0002` `MissingSignature` | `MissingSignature` |
| `split-stranger` | `0x0011` `UnauthorizedActor` | `UnauthorizedActor` |
| `split-imposter` | `0x0009` `WrongPda` | `WrongAccountKey` |
| `merge-unsigned` | `0x0002` | `MissingSignature` |
| `merge-overdraw` | `0x2009` kernel `InsufficientCollateral` | `Kernel(InsufficientCollateral)` |
| `materialize-unsigned` | `0x0002` | `MissingSignature` |
| `materialize-wrong-destination` | `0x0009` `WrongPda` | `WrongAccountKey` |
| `dematerialize-unsigned` | `0x0002` | `MissingSignature` |
| `dematerialize-overdraw` | `0x2008` kernel `InsufficientBalance` | `Kernel(InsufficientBalance)` |
| `resolve-unsigned` | `0x3009` | `MissingSignature` |
| `resolve-wrong-payout` | — | `PayoutIndexMismatch` (**not executable**, see below) |
| `redeem-unsigned` | `0x3009` | `MissingSignature` |
| `redeem-stranger` | `0x300a` | `UnauthorizedActor` |
| `feed-advance-unsigned` | `0x0002` | n/a — the offline adapter has no `FeedAdvance` |
| `feed-advance-replay` | `0x3011` | n/a — the offline adapter has no `FeedAdvance` |
| `create-unsigned` | `0x0002` | n/a — `validate_market_init` models no signer |
| `create-already-initialized` | `0x3010` `NonEmptyInitialization` | `NonEmptyInitialization` |

Three things in that table are deliberate and are not defects:

- **`0x0002` versus `0x3009` for the same fault.** The seam plane and
  `FeedAdvance` hoist the signature check to the account plane and report
  `ClutchError::MissingSignature`; the evidence gate checks it at the reference's
  point in the gate order and reports `Error::MissingSignature` projected to
  `0x3009`. Hoisting it would be a cheaper refusal and a *different order*, and
  order is what the differential is checking.
- **`0x0009` versus `WrongAccountKey`.** The program *derives* the address and
  the reference is handed a trusted binding, so the on-chain check is strictly
  stronger and gets its own adapter-level code. `split-imposter` presents
  byte-identical replay state at a non-canonical address: every decode and every
  linkage check passes on it, so only derivation can refuse it.
- **The kernel codes are not typed out by the harness.** `0x2008` and `0x2009`
  are `error.rs`'s projection of whatever class the oracle actually returned,
  computed at fixture-build time, because both implementations raise the pure
  kernel's own refusal there and neither re-vocabularizes it. Every
  adapter-vocabulary code above is an explicit constant, because there the two
  implementations deliberately differ.

**The KNOWN lossy `0x3fff` collapse.** `error::reference_code` predates the
evidence plane and maps eleven distinct gate refusal classes onto one catch-all
`0x3fff`. `resolve-wrong-payout` is such a case — the request names payout 0
while the sealed window selects payout 1, which is `Error::PayoutIndexMismatch`
— and the plan expects `0x3fff` and nothing finer. That is asserted as the
documented collapse rather than dressed up as precision: on-chain those eleven
classes are indistinguishable, and they stay exactly distinguishable only in the
host differential, which compares typed values.
`observe_resolve.rs`'s `the_numeric_projection_of_the_gate_is_lossy` pins the
collapse and carries the proposed `0x0050-0x005f` allocation that would fix it;
widening `reference_code` is an `error.rs` decision.

### `Resolve` does not fit a transaction — measured

This is the headline negative result of the regeneration.

```text
== Resolve ==
  UNDRIVABLE resolve              consumed 1399850 of 1399850 granted and was aborted
  refuse    resolve-unsigned      Custom(0x3009)  offline reference: MissingSignature
  UNDRIVABLE resolve-wrong-payout consumed 1399850 of 1399850 granted and was aborted
```

With `SetComputeUnitLimit` at 1 400 000 — the per-transaction ceiling, of which
150 units are the budget instruction itself — the program consumes every unit
granted and the runtime aborts it with `ProgramFailedToComplete`. There is
therefore **no observable post-state and no observable gate refusal past the
signature check** for `Resolve` on this runtime. The oracle's expectation is
still written to `plan/expected/` on every run, so the comparison is ready for
the day the instruction fits.

`resolve-unsigned` *is* observed, because the gate checks the signature before
the first terms decode.

The cause is not mysterious and `observe_resolve.rs` predicted it in prose: the
resolve path decodes the immutable terms artifact **five** times — address
derivation, market binding, payout-set binding, record binding, payout
derivation — and `TermsAccount::decode` recomputes a SHA-256 over the 1.2 KiB
terms body on every call. `RedeemInternal` decodes it four times and measures
1 356 878 units, which is the corroborating measurement. The fix is a facts or
`decode_unchecked` API in `clutch-solana-layout`; skipping a binding to save the
hash would not be correct.

The gate **asserts** the exhaustion — the exact `consumed N of N` shape and the
`ProgramFailedToComplete` class — rather than skipping the case. An instruction
that becomes cheap enough to finish turns that check red, which is the signal to
re-measure and rewrite this section rather than leave it stale.

### Resource envelope

Program compute, from the run above. `program_units` counts only the units the
program under test consumed, filtered by program id so the `SetComputeUnitLimit`
instruction is not miscounted as a second measurement.

| transaction | program units | % of 200 000 default | tx bytes | needs a raised limit |
| --- | --- | --- | --- | --- |
| `feed-advance` | 7 673 | 4% | 419 | no |
| `split` | 77 924 | 39% | 650 | no |
| `merge` | 79 376 | 40% | 650 | no |
| `materialize` | 79 320 | 40% | 683 | no |
| `dematerialize` | 80 821 | 40% | 683 | no |
| `roundtrip` | 155 800 (77 924 + 77 876) | 78% | 750 | no |
| `create-market` | 988 153 | 494% | 822 | yes — 71% of the 1 400 000 ceiling |
| `redeem` | 1 356 878 | 678% | 689 | yes — **97%** of the ceiling |
| `resolve` | > 1 399 850 | — | 681 | **exceeds the ceiling; aborted** |

Read those last three rows as a resource finding, not a footnote. `Split` costs
what it did before (72 869 → 73 273 → 77 924 across three recorded runs on
changing dependencies); the evidence gate and the initializer cost one to two
orders of magnitude more, and one of them does not fit at all. Address
derivation dominates the cheap instructions — eight to nine
`sol_try_find_program_address` syscalls each — and terms decoding dominates the
expensive ones.

This is still one fixture with two outcomes and one page of three observations.
It is not an envelope: worst-case outcome counts, worst-case payout sets, and
larger evidence buffers are unmeasured, and every number above would grow.

### The differential is falsifiable

A comparison that cannot go red is not evidence, so the gate falsifies itself
inside the same validator session. After the differential passes, one byte of one
oracle expectation is flipped — the Hoard collateral a `Split` moved — and the
split case is re-run against the same still-running validator and the same
unmodified ELF:

```text
== falsifiability self-check (same validator session) ==
one byte of the Hoard collateral expectation was flipped; the differential went red:
    split / seam.hoard: on-chain bytes != oracle bytes
```

The expectation is restored afterwards. A gate whose self-check *passes* fails
the run.

### The batch-auction plane is loaded but not transacted against

Genesis still carries one epoch with its frozen order page, selected candidate,
final pot, and settlement receipt, bound to the `seam` market. No transaction in
this plan touches them: `PlaceOrder` landed after the fixtures were regenerated
and has no oracle, and cancellation and settlement are stubs. They are loaded so
that an instruction lane inherits a real, bound, canonically addressed plane
instead of inventing one.

What that fixture claims is narrow and checked. Every account decodes through its
frozen codec, and every *identity* binding the layout crate can decide is
asserted while the harness builds it — terms to market, supply ledger to market,
grid to terms, epoch to terms and grid, epoch to its frozen page set, candidate
to epoch, pot and receipt to candidate, resolution to terms — for every one of
the five market planes. A fixture that drifted apart fails `cargo test` rather
than shipping as a genesis nobody checked.

What it does **not** claim is any economic coherence. Whether this candidate is
the best valid submitted candidate for this book, whether the pot balances
against the receipts, and whether the prices clear anything are questions for a
batch relation that no adapter runs yet. The fixture is a shape, bound at every
seam a codec owns and at none that it does not.

The window-policy numbers in the terms artifact are exactly the offline reference
adapter's own resolution fixture and the `observe_resolve` lifecycle vector —
buckets 100 to 103, maturity horizon 4, feed cursor 104, grid family 7 version 1
at 60 seconds, complete-coverage policy, `GEN-EXACT-01`, `FAIL-UNIFORM-REFUND-01`
— so the resolution differential compares two adapters over one scenario rather
than over two, and the offline oracle accepts the same evidence bytes the SVM is
handed.

### Stack findings

The SBF backend reports a function whose frame exceeds 4 KiB as an `Error:` line
that does **not** fail the build. An SBF program that overflows its frame is
undefined behaviour at execution time, so these lines are the only warning there
is, and `scripts/run_bringup.sh` greps them out of the build log on every run.

As of the run above, every reported function belongs to `clutch-solana-layout`
or `clutch-solana-reference`. **None belongs to `clutch-sbf`.**

| function | estimated frame |
| --- | --- |
| `clutch_solana_reference::validate_market_init` | 10496 |
| `clutch_solana_reference::apply_inner` | 9792 |
| `clutch_solana_layout::OrderPageAccount::decode` | 8640 |
| `clutch_solana_layout::OrderPageAccount::decode_on_grid` | 8320 |
| `clutch_solana_reference::validate_position_init` | 8512 |
| `clutch_solana_reference::DecodedState::decode` | 7296 |
| `clutch_solana_reference::resolve_from_evidence` | 6592 |
| `clutch_solana_reference::redeem_from_evidence` | 4544 |

The backend additionally reports that a call inside
`clutch_solana_reference::resolve_from_evidence` *overwrites values in the
frame*. Reference-crate functions are dead-code-eliminated from this ELF and are
never called by a program, so none of these reaches the artifact; the finding
stands on its own as obligation-10 evidence about the *offline adapter's shape*:
its by-value, whole-state calling convention does not fit an SBF frame.

Staying inside 4 KiB is not automatic, and it is why every account decoder in
`accounts.rs` is an `#[inline(never)]` reader that keeps the large decoded value
in its own frame and returns only a small facts structure. `MarketAccount`,
`KernelAccount`, and `clutch_kernel::MarketState` are together well over 4 KiB;
holding two or three whole accounts by value at once does not fit.

#### The order page does not fit, and that blocks a whole instruction family

`OrderPageAccount` became version 3 in commit `da2fbf7`, when portfolio orders
gained a persisted encoding: a page is now `MAX_ORDERS_PER_PAGE` tag-
discriminated slots of `ORDER_SLOT_BYTES` each, 3883 bytes in total. Its
`decode` builds that whole value on the stack before returning it, which is the
8640-byte frame in the table above.

A wrapper cannot fix this from `clutch-sbf`. Writing the obvious reader —
decode, keep the page-set commitment fields, drop the slots — produced its own
overflow, measured:

```text
Error: Function clutch_sbf::accounts::read_order_page overflows the maximum
allowed frame space by accessing an offset 4096 bytes greater than the maximum
of 4096. Estimated function frame size: 8192 bytes.
```

So `accounts::read_order_page` is compiled **off-chain only**
(`#[cfg(not(target_os = "solana"))]`). An instruction lane that reaches for it
gets a compile error naming the problem instead of a frame overflow the loader
will happily run; the host-side hostile-header test still covers it. With that
gate in place the diagnostic is gone from the ELF build, which is how the table
above ends up with nothing from this crate in it.

The fix belongs in `clutch-solana-layout`: a streaming header-and-commitment
decoder that never materializes the slot array, in the same shape as that
crate's own `recomputed_page_digest`, which already streams one
`ORDER_SLOT_BYTES` scratch slot instead of buffering a page. `verify_page_set`
has the same problem one level up: it takes pages by value as a slice.

### What the plan looks like on disk

`clutch-sbf-harness <out-dir>` writes a self-describing plan, so re-running one
case against a live validator needs no shell archaeology:

```text
plan/plan.json              every case: family, oracle, tx file, expected files, expected refusal code
plan/genesis.txt            role, address, and file of all 55 genesis accounts
plan/accounts/<role>.json   one validator --account dump per genesis account
plan/tx/<case>.b64          one serialized unsigned transaction per case
plan/expected/<case>.<plane>.<role>.hex       oracle post-state
plan/expected/<case>.<plane>.<role>.pre.hex   genesis pre-state
```

`scripts/simulate.py --plan <dir> --only <case>` re-runs a single case.

## Reproducing

```sh
programs/clutch-sbf/scripts/run_bringup.sh          # full gate, ~2 minutes
cargo test   --manifest-path programs/clutch-sbf/Cargo.toml
cargo clippy --manifest-path programs/clutch-sbf/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path programs/clutch-sbf/Cargo.toml --no-deps
cargo fmt --manifest-path programs/clutch-sbf/Cargo.toml --all -- --check
```

`cargo test` needs the pinned `solana` CLI on `PATH` or in `SOLANA_BIN`: address
derivation is not compiled into these crates, so the harness derives ~70
program addresses out of process while it builds the plan. That is also why the
plan takes a few seconds to write.

The gate needs `solana-test-validator`, `python3`, and `curl`. It binds
`127.0.0.1:18899` and `127.0.0.1:19900` by default (`CLUTCH_RPC_PORT`,
`CLUTCH_FAUCET_PORT` override) and contacts nothing else. It uses **one**
validator session for all 26 transactions, including the falsifiability
self-check. If a previous run was interrupted before its `trap` fired, an
orphaned validator will still hold the RPC port; `pkill -f solana-test-validator`
before re-running.

## Correct description

"A bring-up SBF program implementing eight instruction families, seven of which
execute inside a local simulated bank on regenerated fixtures with account
validation, address derivation, and post-state bytes agreeing byte for byte with
the offline reference adapter — or, where no reference transition exists, with an
independent re-encode that the reference's own validator accepts — plus one
family, `Resolve`, that is implemented and measured not to fit a Solana
transaction at all."

It is not a complete program, not verified, not audited, and not authorization
to deploy anywhere. Every transaction is simulated with signature verification
off and nothing is committed, so "the actor signed" remains a message-header
fact. `PlaceOrder` has no SVM evidence; cancellation and settlement are stubs.
`CreateMarket` founds markets over pre-created accounts and cannot create an
account. The window identity and the feed summary digest are recorded and never
verified, because the program owns no hash primitive.
