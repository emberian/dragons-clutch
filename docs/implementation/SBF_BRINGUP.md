# SBF bring-up: the instruction set, executed against the reference oracle

> **Historical evidence ledger.** This long-form document preserves the
> bring-up chronology and the measurements available at each checkpoint; it is
> not the present-tense capability index. Since the passages below were
> written, real-SBF evidence has landed for funded `PlaceOrder`/`CancelOrder`,
> prefund-safe blank-bank market construction, free-cash withdrawal, a narrow
> coupled `SettlePage`, and native degree-1--3 point resolution/internal
> redemption. Consult [CURRENT_TRUTH.md](../../CURRENT_TRUTH.md) and the newer
> focused implementation reports before relying on any sentence below that
> says “stub,” “host only,” “unbacked,” or “no SVM leg.” Source-provider/archive
> authentication, general settlement, and a complete operatorless lifecycle
> remain STOPs unless a newer evidence record explicitly closes them.

Status: **host-differential evidence for eight instruction families**
(Split, Merge, Materialize, Dematerialize, CreateMarket, FeedAdvance,
evidence-gated Resolve, RedeemInternal), each mirroring the offline
reference adapter's semantics with byte-level differential tests, **plus
SVM execution evidence for those eight and for `Endow`**: every one of
those families executes inside a local Agave bank against regenerated
fixtures, and every writable account it touches — the Token-2022 mints
and token accounts included — comes back byte-identical to the oracle's
post-state.

**Regenerated 2026-08-19 for the mandatory token plane.** Commits
`472b7fe` and `50c6e35` made the Token-2022 legs mandatory, gave
`CreateMarket` seven creation CPIs, and changed every account count; the
gate exited 1 on every family until the harness was rewritten to emit the
new planes. Every number in this document is from the post-regeneration
run and the pre-token value is kept beside it wherever it moved. The
short version: the outcome leg costs about **15 000** compute units, the
collateral leg about **135 000** — dominated not by the CPI but by two
software SHA-256 digests inside `collateral::verify_profile_identity` —
and eight of the nine families now need a raised compute limit where
three did before.

`Resolve` used to be an exception — on this runtime it consumed the
entire 1 400 000-unit per-transaction compute ceiling and was aborted —
until the named defect was fixed at its named owner: the terms-revision
wave landed `TermsAccount::decode_unchecked`/`decode_into` in
`clutch-solana-layout` and the gate now pays the terms digest SHA-256
once per transaction. Re-measured after the token plane: `Resolve`
executes at **554 929** program units and `RedeemInternal` at **555 739**
— 40% of the ceiling each. The numbers are recorded in
[Results](#results).

`PlaceOrder` and `CancelOrder` landed on-chain after these fixtures were
regenerated and have **no SVM leg** and no offline oracle (the reference
adapter models no order family); batch settlement (`SettlePage`) remains
the one honest stub.  *(Retired 2026-08-20, T2-8: `SettlePage` now consumes
entitled receipts — single-Egg direct slices and portfolio full pairs —
created by the entitlement freeze, tags 58-59; unentitled and general
shapes keep refusing `0x0017`.)* Not a complete program, not audited, not a deployment
authorization, and not mainnet, devnet, or testnet evidence. A stub
reads no account, writes no byte, and reports no success.

This document records what the `programs/clutch-sbf` lane built, what actually
ran, what failed, and what is deferred. Numbers below are from the run recorded
in [Results](#results); re-running `programs/clutch-sbf/scripts/run_bringup.sh`
reproduces them.

**Per-family evidence is not the PROJECT.md section-10 walk.** Ten families
that each pass in isolation say nothing about whether the eleventh state is
reachable from the first. The same gate script now runs one market end to end
as one ordered, all-or-nothing walk in the same validator session, closing with
the section-10 item-10 accounting identity read out of the on-chain bytes; that
walk, its measured table, and its honest skip list are in
[`LIFECYCLE_WALK.md`](LIFECYCLE_WALK.md).

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
| `instructions/genesis.rs` | `Intent::InitRealm`, `Intent::InitProfile`, `Intent::InitPriceGrid`, `Intent::InitTerms`, `Intent::InitOrderPage`, `Intent::Endow` | **implemented** — the only module that creates accounts; see the genesis addendum |
| `instructions/split.rs` | `Intent::Split` | **implemented** |
| `instructions/merge_materialize.rs` | `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize` | **implemented** — all three, through the `split.rs` seam plane |
| `instructions/market_init.rs` | `Intent::CreateMarket` | **implemented** — validated initialization-write over pre-created state accounts, **plus** the creation of every outcome mint and the Hoard token account by CPI |
| `instructions/observe_resolve.rs` | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` | **implemented** — `Resolve` fits a transaction again (554 929 CU) since the terms revision; deferred check 15 is closed; `RedeemInternal` carries the mandatory collateral leg |
| `instructions/orders_batch.rs` | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SettlePage` | `PlaceOrder` and `CancelOrder` **implemented**; `SettlePage` **implemented for entitled receipts** since T2-8 (2026-08-20); general shapes still refuse |

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
| `CancelOrder` | `NotYetImplemented` `0x0017` | **implemented** (v4 tombstone retirement, landed with the orders-on-v4 integration); like `PlaceOrder`, no offline adapter and no SVM evidence |
| `SettlePage` | `NotYetImplemented` `0x0017` | **retired for entitled receipts** (T2-8, 2026-08-20): the streaming walk verified the relation on-chain, the entitlement freeze (tags 58-59) creates the receipts, and `SettlePage` consumes them; partial fills, virtual legs, mixed pairs, and inexact conversions keep refusing `0x0017` honestly |

`Merge`, `Materialize`, and `Dematerialize` previously refused
`UnsupportedInstruction` (`0x000e`), then `NotYetImplemented` (`0x0017`). All
three now run through the seam plane of `instructions/split.rs` — ten shared
state accounts plus the intent's mandatory token leg, sixteen accounts for
`Merge` and thirteen for the other two — and refuse only what the transition
itself refuses.

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
set to drift.

The **first ten accounts** are shared by all four intents, in this order:

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

**Every seam intent then appends exactly one mandatory token leg, and which
leg it gets is a function of the intent rather than of the caller.** That is
the ABI break the Token-2022 completion landed (`50c6e35`) and it is why this
whole document was regenerated: `split::select_token_leg` is a total function
of the intent, a ten-account seam transaction is now `ClutchError::AccountCount`
(`0x0001`), and every emitter had to be rewritten. `Split` and `Merge` take the
**collateral leg** and total sixteen:

| # | role | signer | writable |
| --- | --- | --- | --- |
| 10 | Token-2022 program (executable) | no | no |
| 11 | the Realm's 266 collateral-policy bytes | no | no |
| 12 | the collateral mint the policy names | no | no |
| 13 | the actor's own collateral token account | no | yes |
| 14 | the Hoard's signing authority (holds no data) | no | no |
| 15 | the Hoard's collateral token account | no | yes |

`Materialize` and `Dematerialize` take the **outcome leg** and total thirteen:
the token program at index 10, the derived outcome mint for the named outcome
at 11 (writable), and the holder's token account for that mint at 12
(writable).

The 266 policy bytes are in the collateral list because the collateral mint's
*identity* lives nowhere else: without them a caller could present a worthless
mint and its own accounts and buy complete sets with it. They are
**content**-authenticated rather than address-authenticated —
`collateral::verify_profile_identity` recomputes the child digest from the
bytes, compares it against the Profile's frozen digest, *and* recomputes the
parent Profile identity from that digest and compares it against the stored
Profile ID — so the account they arrive from is arbitrary and this plan
presents them from a fixed literal address that nothing derives.

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
| `Split`, `Merge` | 16 | the ten above, then the collateral leg |
| `Materialize`, `Dematerialize` | 13 | the ten above, then the outcome leg |
| `CreateMarket` | 19 + `outcome_count` (**21** here) | creator (signer **and writable**: it is the rent payer), Realm, Profile, terms (read-only); Market, Hoard, Position, kernel, external, replay, ledger, resolution record (writable, and required to arrive **all-zero**); the policy bytes, the Token-2022 program, the collateral mint, the System program, the Rent sysvar, the Hoard authority (all read-only); the Hoard token account and one outcome mint per outcome (writable, and required to arrive **uncreated** — zero lamports and zero data) |
| `Resolve` | 12 | actor; Market, Hoard, Position, kernel, external, replay, ledger (writable); terms (read-only); resolution record (writable for a resolve, read-only for a redemption); feed head (read-only); caller-supplied evidence buffer (read-only) |
| `RedeemInternal` | 19 | the twelve above, then the Profile, the token program, the policy, the collateral mint, the redeemer's own collateral account, the Hoard authority, and the Hoard token account |
| `FeedAdvance` | 3 | actor; feed head (writable); caller-supplied observation page (read-only) |
| `Endow` | 4 | actor (must be the position owner); Market (read-only); Position, replay sequence (writable). No System program: an endowment allocates nothing |

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
   own accepting SVM transaction on the seam plane — sixteen accounts with the
   collateral leg, thirteen with the outcome leg — plus a `Split`/`Merge`
   round trip sequenced by the bank in which real Token-2022 collateral leaves
   the actor's account and comes back. `Merge` closed on *both* sides at once,
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

   **Update 2026-08-19 — the creation half now exists, for a different set of
   accounts.** `instructions/genesis.rs` writes the
   `system_instruction::create_account` CPI, reads the rent-exemption parameters
   off the rent sysvar, and drives `invoke_signed` with PDA seeds, for five
   accounts: Realm, Profile, price grid, terms, and order page. `CreateMarket`
   is **unchanged** and still creates nothing — its twelve accounts still arrive
   pre-created — so the item stays open *for the market plane*. What it is no
   longer true of is the program: the three named sub-gaps are written and host-
   tested, and what remains for `market_init` is adopting them, which is that
   lane's edit and not this one's.
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
9. **Half closed (2026-08-19).** When written: no token program, CPI, mint, or
   escrow behaviour existed. Since `5c88505`, the Token-2022 CPI leg is live
   for outcome mints — Materialize mints and Dematerialize burns via
   `invoke_signed`, with exact post-delta checks and svm-tests evidence on a
   real bank (`programs/clutch-sbf/svm-tests`). Still true: the collateral
   escrow leg (`TransferChecked` into/out of a Hoard token account) is
   constructed and unit-tested but wired into no instruction, and no mint or
   token account is ever created by the program (obligations 5-7 remain open
   on that half; see `TOKEN2022_PLAN.md` §status).
10. **Closed on both sides.** Multi-position aggregate closure used to be
    refused by representation. It is now carried by the CLO-DELTA-V1 two-term
    ledger in both adapters, and the ledger is compared on-chain on every
    accepting transaction of every family (obligation 11, and item 13).
11. **Partly closed.** The resource envelope is now nine compute measurements
    across eight families — the former ceiling exhaustion is a measurement
    again — and every transaction's serialized size is recorded; see
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

15. **Closed: `Resolve` fits a Solana transaction again.** The exhaustion was
    MEASURED here first — with `SetComputeUnitLimit` at the 1 400 000-unit
    ceiling the program consumed every unit granted and was aborted, because
    the resolve path decoded the immutable terms artifact **five** times and
    `TermsAccount::decode` recomputed a SHA-256 over the terms body on every
    call. The fix landed at the owner this item named: the
    `decode_unchecked`/`decode_into` API in `clutch-solana-layout`, consumed
    by `observe_resolve` so the digest is recomputed exactly once per
    transaction (in the account plane's full `read_terms`) while every
    structural check still runs at every gate step and no binding was
    skipped. The exhaustion gate asserted the exhaustion, went red when the
    instruction finished — exactly as designed — and this section was
    re-measured rather than left stale: `Resolve` 536 123 program units,
    `RedeemInternal` 408 294, `CreateMarket` 701 548, on the larger v3 terms
    account (1 656 bytes). Those three numbers are themselves now historical —
    the token plane moved them to 554 929, 555 739 and 857 343. See
    [Resource envelope](#resource-envelope).

15b. **Closed the same way, 2026-08-19: the three cases the ceiling kept
    undrivable are driven.** `resolve-repeat-idempotent` (two identical
    Resolves in one transaction; the second consumes no Position or Replay
    and leaves the resolution fact unchanged),
    `resolve-late-conflict-rolls-back` (payout one then payout zero in one
    transaction; the late `0x0057` refusal rolls the whole transaction
    back), and the committed walk's step 19
    (`committed-19-external-exit-rollback`: a successful bearer exit
    followed by a duplicate exit refuses `TOKEN_DELTA_MISMATCH` atomically)
    were each authored with full oracle expectations and then demoted to
    `exhausted` because two such instructions could not fit one
    1,400,000-unit transaction. The syscall-hash rework (`6c25df4`)
    dissolved that stop; the demotions are removed and the authored
    semantics execute. The committed walk therefore reports
    `committed_expected_refusals=2` / `committed_compute_exhaustions=0`,
    and the bringup campaign reports no undrivable case.

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
with the ELF at the chosen program id and **195 accounts loaded at genesis**,
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
| `seam` | 9 | empty, active, replay sequence 0 | `Split`, the `Split`/`Merge` round trip, the four `Split` refusals, all four `Endow` cases, and the `CreateMarket` re-initialization refusal |
| `held` | 10 | `apply(Split 20)` | `Merge`, `Materialize`, `Resolve` |
| `shadow` | 11 | `apply(Split 20)` then `apply(Materialize 0, 3)` | `Dematerialize` |
| `redeem` | 13 | `apply(Split 20)` then `apply_with_evidence(Resolve 1)` | `RedeemInternal` |
| `create` | 14 | eight all-zero accounts at their canonical addresses, **and no token accounts at all** | `CreateMarket` |

**Every founded plane now carries a Token-2022 plane too**, and none of it is
chosen: the Hoard's token account holds exactly the plane's own
`HoardAccount::collateral_atoms` (the mirror `token::require_hoard_mirror`
re-checks over the pre-state before anything moves), and outcome mint *i* has
exactly the plane's own supply-ledger external term for outcome *i* (the
reconciliation `require_shadow_reconciles` re-checks after every mint or
burn). A genesis that disagreed with its own state would be refused on chain
rather than silently accepted.

Those images are bytes this harness wrote rather than bytes Token-2022 wrote,
because a validator loaded from a genesis dump cannot run an instruction before
its first slot. That claim is defended twice. The **real** Token-2022 program is
what executes `MintTo`, `Burn` and `TransferChecked` against them inside the
SVM, and it refuses anything it did not consider a mint or an account; and the
harness re-runs this program's own `token::check_mint` and
`token::check_token_account` admission over every image before emitting it, so
a byte this program would refuse is a build-time panic in `cargo test` rather
than a mysterious on-chain refusal later.

The `create` plane is the one that installs **nothing**: `CreateMarket` creates
the Hoard token account and every outcome mint itself, by System-program CPI,
and `market_init::require_uncreated` demands that those addresses hold zero
lamports and zero data. Their absence from the genesis dump is the
precondition under test.

Two feed identities exist for the same reason: `Resolve` needs a **read-only**
feed head whose cursor is already past the window's maturity bound, and
`FeedAdvance` needs a **writable** one sitting at the start bucket. One account
cannot be both.

The one place a real sequence executes is the `roundtrip` transaction, which
carries `Split` and `Merge` as two instructions of one transaction so that the
**bank** sequences them and the second instruction reads the first's writes.
That is what makes the round-trip claim a bank fact rather than a harness fact.

### Every family but `FeedAdvance` needs a raised compute limit

This used to say *three* families. The mandatory token plane moved it to
**eight of nine**: a `Split` that moves real collateral recomputes the
266-byte policy digest and the parent Profile hash, admits a mint and two
token accounts, and performs a `TransferChecked` CPI, and none of that fits
the runtime's default 200 000-unit budget. Every transaction in the plan
except `FeedAdvance` therefore carries a `SetComputeUnitLimit` instruction at
the 1 400 000-unit per-transaction ceiling ahead of the program instruction,
exactly as a real caller would. `FeedAdvance` is deliberately left without one
so that the difference stays visible in the recorded numbers. The raised
number is itself the measurement; see
[Resource envelope](#resource-envelope).

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
recorded on `aarch64-apple-darwin` with `HEAD` at `7ce4c09`.

**Regenerated 2026-08-19 for the mandatory token plane.** Every number below
this line was re-measured after commits `472b7fe` (the genesis plane) and
`50c6e35` (token completion) made the token legs mandatory and changed every
account count. The previous recording — `split` at 77 924 units on a
ten-account plane, `create-market` at 701 548 on twelve accounts, 122 genesis
accounts, no token account anywhere in the plan — is retired, not merely
superseded: those transactions are now refused `AccountCount` (`0x0001`)
because the planes they emit no longer exist. A one-line historical note is
kept at each moved number so a reader can see which direction it moved and
why.

**The tree-state caveat, stated precisely.** The gate prints `git rev-parse HEAD`
and, since this regeneration, the working-tree status of **exactly the paths
`cargo-build-sbf` reads** separately from the status of everything else. That
distinction is the whole point of printing tree state: a tree dirty only in the
harness, the scripts, or this document still produces a digest that names a
commit. This run reported:

```text
== source pin ==
7ce4c091a6caaecbbee1a3960884d06022529ebb
elf_inputs=DIRTY (the ELF digest below names this working tree, not a commit)
   M programs/clutch-sbf/Cargo.lock
```

`Cargo.lock` is an ELF input and it is dirty, so the gate says so rather than
guessing. The change is confined to the `clutch-sbf-harness` package's own
dependency list (the harness gained `solana-pubkey`, at the pin the program
already uses, so that it can re-run this program's Token-2022 admission over
every fixture image it emits); no version in the program's build graph moved.
That is an argument, not evidence, so it was **checked**: the same ELF was
built from a clean extraction of `HEAD` (`git archive HEAD | tar -x` into a
fresh directory, fresh `CARGO_TARGET_DIR`) and produced the identical digest.
The digest below therefore names commit `7ce4c09`.

### Reproducible ELF

Built twice into fresh target directories:

```text
pass 1  sha256=59c48c482831626ae9d7cb908f4de0e3f93b1572cdd82105c61f2f87bdaad25f  bytes=505960
pass 2  sha256=59c48c482831626ae9d7cb908f4de0e3f93b1572cdd82105c61f2f87bdaad25f  bytes=505960
sbf_reproducibility=PASS
```

Reproduced a third time from the clean `HEAD` extraction described above, with
its own target directory, so the agreement is not an artifact of the gate's own
directory layout.

The artifact is 505 960 bytes. Historical: the pre-token ELF was 402 192 bytes
(`d8a9267c…`) and the pre-instruction-set one 102 568 bytes; the growth is the
genesis plane's System-program CPIs and the Token-2022 admission, CPI
construction, and TLV walker.

Dynamic symbols, re-read from this ELF with the pinned `llvm-readelf`: eleven
`.dynsym` entries, exporting `entrypoint` and `custom_panic` and importing the
syscalls `sol_try_find_program_address`, `sol_log_`, `sol_panic_`, `abort`,
**`sol_invoke_signed_rust`**, `sol_memset_`, `sol_memcpy_`, and `sol_memcmp_`.
The CPI syscall is the one addition since the pre-token ELF, and it is now
load-bearing on every family but `FeedAdvance` and `Endow`. There is still **no
hash syscall**: `sol_sha256` is absent because `clutch-solana-layout` carries
its own software SHA-256, which is why the collateral leg costs what the
[Resource envelope](#resource-envelope) says it costs. The Token-2022 program
id remains data, not a dynamic symbol.

### Genesis

**195 accounts**, loaded by one validator invocation, in three ownerships:

| owner | count | what |
| --- | ---: | --- |
| this program | 131 | six Realm-wide accounts (Realm, Profile, price grid, immutable terms, two feed heads), the 266-byte collateral policy, **fourteen** market planes of eight accounts each (112), the five-account batch-auction plane no implemented instruction touches, six caller-supplied buffers (three evidence/page buffers plus the walk's three observation pages), and the imposter replay account |
| Token-2022 | 63 | the collateral mint, the actor's and the stranger's collateral accounts, and — for each of the twelve **founded** planes — a Hoard token account, two outcome mints, and two holder token accounts |
| System | 1 | the creator's lamports: `CreateMarket` founds real accounts through a System-program CPI and the creator is the rent payer, so a signer with no account could not fund a creation |

Historical: 55 accounts before the lifecycle walk, 122 after it and before the
token plane. The Profile is **frozen** — the policy flag is set and the digest
is nonzero — and is no longer a bare shape claim: the digest is the recomputed
child digest of a real, decodable 266-byte policy that names a mint this
genesis actually installs, and the Profile *identity* is the canonical parent
hash over that same digest, because `collateral::verify_profile_identity`
refuses any other pairing. That is the single change that moved every address
in this plan.

### Differential against the oracle, per family

Every accepting transaction's writable accounts are compared byte for byte. A
role marked *unchanged* is additionally required to come back equal to its
pre-state, and a role marked *changed* is required to differ from it: an
expectation that happens to equal the pre-state cannot pass by accident.

| transaction | family | oracle | accounts compared | result |
| --- | --- | --- | --- | --- |
| `split` | `Split` | `reference::apply` | 9 (7 state + 2 token) | 9/9 MATCH |
| `roundtrip` | `Split`+`Merge` | `reference::apply` ∘ `apply` | 9 | 9/9 MATCH |
| `merge` | `Merge` | `reference::apply` | 9 | 9/9 MATCH |
| `materialize` | `Materialize` | `reference::apply` | 9 (7 state + mint + holder) | 9/9 MATCH |
| `dematerialize` | `Dematerialize` | `reference::apply` | 9 | 9/9 MATCH |
| `resolve` | `Resolve` | `reference::apply_with_evidence` | 8 | 8/8 MATCH |
| `redeem` | `RedeemInternal` | `reference::apply_with_evidence` | 10 (8 + 2 token) | 10/10 MATCH |
| `endow` | `Endow` | layout re-encode (**no reference oracle**) | 2 | 2/2 MATCH |
| `feed-advance` | `FeedAdvance` | accumulator fold + `FeedAccount` codec | 1 | 1/1 MATCH |
| `create-market` | `CreateMarket` | layout re-encode + `reference::validate_market_init` | 8 | 8/8 MATCH |

74 account comparisons, 74 matches, 0 mismatches. `resolve` is an ordinary
compared case again and has been since the decode-once rework; the "not
executable" row it used to occupy is retired.

**The token accounts are compared, and their expectations are the oracle's
numbers.** They are not a second description of what a transfer, a mint or a
burn does:

- the Hoard's token account must end holding exactly the reference adapter's
  `HoardAccount::collateral_atoms` — which is the mirror
  `token::require_hoard_mirror` re-checks on chain;
- the actor's collateral account must end holding exactly what it started with,
  less whatever the adapter says the Hoard gained (or plus whatever it says the
  Hoard lost); and
- an outcome mint's supply and its holder's balance must end at the adapter's
  supply-ledger external term for that outcome — the reconciliation
  `require_shadow_reconciles` re-checks on chain.

So each is the claim the program enforces, restated over bytes the bank
returned, against a number a second implementation computed.

**The oracles are not all equally strong, and the plan says so per case.** For
the seam plane and for `RedeemInternal` the oracle is a second, independently
written implementation of the same transition, so a match is two adapters
agreeing. For `CreateMarket` there is no reference transition: the expectation is
an independent re-encode through the frozen `clutch-solana-layout` codecs from
the intent and the terms artifact alone, following the PROPOSED initial values
`market_init.rs` documents (the terms' collateral cap, zero created slot,
generation zero, the Hoard PDA as its own authority, the terms artifact's
payout set, an unresolved record), and that re-encode is then required to
satisfy the reference's own `validate_market_init`. For `FeedAdvance` there is
no reference adapter at all: the expected cursor is what `clutch_accumulator`'s
`Summary` fold lands on for the same records, re-encoded through the frozen
`FeedAccount` codec. **`Endow` is the weakest of all and the plan labels it
so**: `reference::apply` refuses `Intent::Endow` with `UnsupportedIntent`, so
the expectation is this harness moving the two fields the transition moves
through the frozen `PositionAccount` and `ReplayAccount` codecs. What that
still catches is every *other* byte of both accounts, which must come back
unchanged.

### The round trip through the bank

`roundtrip` is one transaction carrying two instructions — `Split` 5 at sequence
0 and `Merge` 5 at sequence 1 — so the **bank**, not the harness, sequences them
and the `Merge` reads the `Split`'s writes:

```text
accept roundtrip  program_units=424656 per-instruction [212310, 212346] bytes=994
  differential seam.market            MATCH (unchanged)
  differential seam.hoard             MATCH (unchanged)
  differential seam.position          MATCH (unchanged)
  differential seam.kernel            MATCH (unchanged)
  differential seam.external          MATCH (unchanged)
  differential seam.replay            MATCH (changed)
  differential seam.supply            MATCH (unchanged)
  differential seam.hoard-token       MATCH (unchanged)
  differential seam.actor-collateral  MATCH (unchanged)
```

Eight of the nine accounts are byte-identical to the pre-split genesis. The
ninth is the replay account, whose sequence advanced 0 → 2. **Two of those
eight are now Token-2022 accounts**, which is what makes this a round trip
through the token program and not only through the ledger: real collateral left
the actor's account for the Hoard's and came back, and the bank's own bytes say
both are exactly where they started. The harness asserts that shape at build
time as well, so a round trip that stopped closing would fail `cargo test`
before it ever reached a validator.

(Historical: 155 800 units across 750 bytes on the pre-token ten-account plane.
The transaction is 2.7× more expensive and 244 bytes longer because each half
now moves collateral.)

### Refusals

Every refusal is executed in the SVM and its numeric `ProgramError::Custom` code
compared against an expected value, with the offline adapter's own refusal class
for the same situation recorded beside it. **Twenty-two executed cases**, at
least two per family, none undrivable:

| transaction | code | offline reference |
| --- | --- | --- |
| `split-unsigned` | `0x0002` `MissingSignature` | `MissingSignature` |
| `split-stranger` | `0x0011` `UnauthorizedActor` | `UnauthorizedActor` |
| `split-imposter` | `0x0009` `WrongPda` | `WrongAccountKey` |
| `split-foreign-collateral` | `0x001b` `TokenAccountNotAdmitted` | n/a — the offline adapter has no collateral leg |
| `merge-unsigned` | `0x0002` | `MissingSignature` |
| `merge-overdraw` | `0x2009` kernel `InsufficientCollateral` | `Kernel(InsufficientCollateral)` |
| `materialize-unsigned` | `0x0002` | `MissingSignature` |
| `materialize-wrong-destination` | `0x0009` `WrongPda` | `WrongAccountKey` |
| `dematerialize-unsigned` | `0x0002` | `MissingSignature` |
| `dematerialize-overdraw` | `0x2008` kernel `InsufficientBalance` | `Kernel(InsufficientBalance)` |
| `resolve-unsigned` | `0x3009` | `MissingSignature` |
| `resolve-wrong-payout` | `0x0057` `PayoutIndexMismatch` | `PayoutIndexMismatch` |
| `redeem-unsigned` | `0x3009` | `MissingSignature` |
| `redeem-stranger` | `0x300a` | `UnauthorizedActor` |
| `redeem-foreign-collateral` | `0x001b` `TokenAccountNotAdmitted` | n/a — the offline adapter has no collateral leg |
| `endow-unsigned` | `0x0002` | n/a — the offline adapter has no `Endow` |
| `endow-stranger` | `0x0011` `UnauthorizedActor` | n/a — the offline adapter has no `Endow` |
| `endow-over-cap` | `0x0012` `CollateralCap` | n/a — the offline adapter has no `Endow` |
| `feed-advance-unsigned` | `0x0002` | n/a — the offline adapter has no `FeedAdvance` |
| `feed-advance-replay` | `0x3011` | n/a — the offline adapter has no `FeedAdvance` |
| `create-unsigned` | `0x0002` | n/a — `validate_market_init` models no signer |
| `create-already-initialized` | `0x0040` `AlreadyInitialized` | `NonEmptyInitialization` |

**The two `0x001b` rows are new with the mandatory collateral leg and they are
the reason it names the actor at all.** `split-foreign-collateral` is the
position owner signing a `Split` funded out of an account it does not own;
`redeem-foreign-collateral` is the claim owner directing a payout into one.
Both are refused because `TokenAccountPolicy::collateral_holder` binds the
presented account's owner authority to the *authenticated actor*. Without that
check a caller who may name the asset could buy complete sets out of anyone's
wallet.

**One refusal moved class, and it is an order-of-checks fact worth stating.**
`redeem-stranger` reaches `0x300a UnauthorizedActor` only because the stranger
presents *its own* collateral account: `RedeemInternal` authenticates the
collateral leg **before** the evidence gate authorizes the actor, so a stranger
presenting the owner's account is refused `0x001b` for owning the wrong token
account — a true refusal about a different question. This plan therefore
installs a collateral account for the stranger too, so that the authorization
refusal stays a refusal about authorization, and asks the other question in its
own case.

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

**The lossy `0x3fff` collapse is CLOSED.** `error::reference_code` predated
the evidence plane and mapped eleven distinct gate refusal classes onto one
catch-all `0x3fff`; the terms-revision wave widened it with exactly the
`0x0050-0x005f` allocation `observe_resolve.rs` proposed, so
`resolve-wrong-payout` — the request names payout 0 while the sealed window
selects payout 1 — now lands on `0x0057 PayoutIndexMismatch` and the plan
expects that code and nothing coarser. The sub-reasons inside `Window(_)` and
`Resolution(_)` stay one number per class on-chain, exactly distinguishable
in the host differential, which compares typed values.
`observe_resolve.rs`'s `the_numeric_projection_of_the_gate_is_allocated` pins
the table.

### `Resolve` fits a transaction again — re-measured

The headline negative result of the previous regeneration is closed, by the
mechanism that regeneration prescribed. The record of the exhaustion stands:
with `SetComputeUnitLimit` at 1 400 000 the program consumed every unit
granted and was aborted with `ProgramFailedToComplete`, because the resolve
path decoded the immutable terms artifact **five** times — address
derivation, market binding, payout-set binding, record binding, payout
derivation — and `TermsAccount::decode` recomputed a SHA-256 over the terms
body on every call (`RedeemInternal`, at four decodes and 1 356 878 units,
was the corroborating measurement).

The fix landed where this section said it belonged — a `decode_unchecked` /
`decode_into` API in `clutch-solana-layout`, not a skipped binding: the
account plane's `read_terms` pays the one full decode (digest included), and
every later gate read runs the full structural validation minus the digest
recomputation, sound because the terms account is presented read-only within
the transaction that already proved the digest over the same bytes. The
exhaustion gate asserted the `consumed N of N` shape, went red the moment the
instruction finished — exactly the trip-wire it was designed to be — and the
re-measured run, on the *larger* v3 terms account (1 656 bytes, up from
1 304), reads:

```text
== Resolve ==
  accept resolve                      program_units=554929 tx_units=555079 limit=1400000
  refuse resolve-unsigned             Custom(0x3009)  offline reference: MissingSignature
  refuse resolve-wrong-payout         Custom(0x0057)  offline reference: PayoutIndexMismatch
```

(The re-measurement at the time of the fix read 536 123; the token-plane
regeneration moved it to 554 929 with no change to the evidence gate itself.
`Resolve` carries no token leg — `TOKEN2022_PLAN.md` §3.2 gives it no CPI — and
its plane is still the unchanged twelve accounts.)

Every gate refusal is now observable on-chain, and `resolve-wrong-payout`
lands on its allocated projection (`0x0057 PayoutIndexMismatch`, from the
`0x0050-0x005f` block `error.rs` carries since the terms-revision wave)
rather than the former `0x3fff` collapse.

### Resource envelope

Program compute, from the run above. `program_units` counts only the units the
program under test consumed, filtered by program id so the `SetComputeUnitLimit`
instruction is not miscounted as a second measurement. The **was** column is the
pre-token recording; it is kept in place so the direction and size of every move
is visible rather than quietly replaced.

| transaction | program units | was (pre-token) | % of 200 000 default | tx bytes | needs a raised limit |
| --- | ---: | ---: | ---: | ---: | --- |
| `feed-advance` | 7 689 | 7 673 | 4% | 419 | no |
| `endow` | 81 675 | — (new) | 41% | 492 | no, but carries one |
| `dematerialize` | 94 125 | 80 821 | 47% | 822 | yes |
| `materialize` | 95 647 | 79 320 | 48% | 822 | yes |
| `split` | 212 310 | 77 924 | 106% | 888 | yes — 15% of the 1 400 000 ceiling |
| `merge` | 218 346 | 79 376 | 109% | 888 | yes — 16% |
| `roundtrip` | 424 656 (212 310 + 212 346) | 155 800 | 212% | 994 | yes — 30% |
| `resolve` | 554 929 | 536 123 | 277% | 681 | yes — 40% |
| `redeem` | 555 739 | 408 294 | 278% | 920 | yes — 40% |
| `create-market` | 857 343 | 701 548 | 429% | 1 119 | yes — 61% |

**What moved, and why.**

- **The outcome leg costs about 15 000 units.** `materialize` +16 327 and
  `dematerialize` +13 304: one mint admission, one token-account admission, one
  TLV walk each, and one `MintTo` or `Burn` CPI.
- **The collateral leg costs about 135 000.** `split` +134 386 and `merge`
  +138 970 are the clean measurements: those planes gained the leg and nothing
  else. `redeem` +147 445 includes the same ~19 000-unit baseline shift
  `resolve` shows below, so the leg's own share there is about 128 600. The CPI
  is not the expensive part. The expensive
  part is `collateral::verify_profile_identity`, which runs **two software
  SHA-256 digests** — the 266-byte policy and the 88-byte parent-Profile
  preimage — inside the VM, because `clutch-solana-layout` carries its own
  SHA-256 and this program imports no `sol_sha256` syscall. That is the single
  largest line item in the token plane and it is a named, addressable one: a
  syscall-backed digest behind the same API would return most of it.
- **`CreateMarket` +155 795**, to 857 343. It now performs **seven CPIs** —
  three `CreateAccount`s, `InitializeMint2` twice, `InitializeImmutableOwner`,
  `InitializeAccount3` — reads the rent sysvar, and re-admits every account the
  token program just wrote through the same policies every later instruction
  will apply to them. At 61% of the per-transaction ceiling with two outcomes,
  and one more mint per additional outcome, this is the number that bounds
  `MAX_OUTCOMES` in practice long before the account list does.
- **`resolve` +18 806** with no token leg at all. The evidence gate did not
  change; the program around it did (the genesis plane, the market-init
  rewrite), and the SBF backend's inlining and layout decisions moved with it.
  It is recorded rather than explained away.
- **Eight of nine families now need a raised limit**, where three did before.
  `split` and `merge` crossed the 200 000-unit default in this wave and did not
  before.

Read those numbers as a resource finding, not a victory lap. This is still one
fixture with two outcomes, one page of three observations, and one position. It
is not an envelope: worst-case outcome counts, worst-case payout sets, larger
evidence buffers, and any market with more than one position are unmeasured,
and every number above would grow.

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
or `clutch-solana-reference`. **None belongs to `clutch-sbf`**, and that is
still true after the token plane and the genesis plane landed — the same
`#[inline(never)]` reader discipline covers `token::observe_mint`,
`token::check_token_account`, `split::validate_collateral_leg` and
`market_init::create_token_plane`.

| function | estimated frame |
| --- | ---: |
| `clutch_solana_reference::resolve_from_evidence` | 13 568 |
| `clutch_solana_reference::validate_market_init` | 13 440 |
| `clutch_solana_reference::redeem_from_evidence` | 11 776 |
| `clutch_solana_reference::apply_inner` | 9 792 |
| `clutch_solana_layout::OrderPageAccount::decode` | 8 896 |
| `clutch_solana_reference::validate_position_init` | 8 768 |
| `clutch_solana_layout::OrderPageAccount::decode_on_grid` | 8 576 |
| `clutch_solana_reference::DecodedState::decode` | 7 296 |

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

## The genesis plane (addendum, 2026-08-19)

> **Driven status, added at the token-plane regeneration.** Of this module's
> six intents, **`Endow` is now driven in the SVM** — one accepting case and
> three refusals in the per-family plan above, and step 2 of the lifecycle
> walk. The other five (`InitRealm`, `InitProfile`, `InitPriceGrid`,
> `InitTerms`, `InitOrderPage`) are **drivable but not driven this round**, and
> the distinction matters: nothing about the fixture model blocks them. Each
> needs its own fresh identity plane whose target address is absent from the
> genesis dump — a Realm nonce, a policy, a grid body and a terms body whose
> digests are not the ones already installed — and none has a reference oracle,
> so a differential would compare this program against a re-encode of its own
> intent. Their `system_instruction::create_account` CPI has therefore **never
> run on a bank**; only `CreateMarket`'s has, seven times per founding
> transaction. Deferred check 2 is half closed, not closed.

`instructions/genesis.rs` is the first family in this program that **creates**
accounts. Every other family writes over accounts that arrived already created,
program-owned, rent-funded and correctly sized; that arrangement is deferred
check 2 above, and this module is the missing half of it — the
`system_instruction::create_account` CPI, the rent-exemption computation, and
the `invoke_signed` seed plumbing, written and host-tested.

### Instruction coverage

| intent | accounts | creates | oracle |
| --- | ---: | --- | --- |
| `InitRealm` | 5 | `RealmAccount`, 70 B | layout codec |
| `InitProfileV2` | 8 | `ProfileAccount`, 100 B, policy/release frozen after exact immutable token ProgramData authentication | layout codec + `verify_profile_identity` |
| `InitPriceGrid` | 6 | `PriceGridAccount`, 589 B | layout codec, verbatim buffer copy |
| `InitTerms` | 7 | `TermsAccount`, 1,656 B | layout codec, verbatim buffer copy |
| `InitOrderPage` | 6 | one order page, 4,012 B | `stream::init_page` |
| `Endow` | 4 | nothing — credits position cash | layout codecs on the post-state |

Account lists all start `[payer, target, …]` and the creating five end
`[…, system program, rent sysvar]`. The payer signs and funds; nothing else is
privileged, which is the same **PROPOSED** permissionless model `market_init`
argues for and for the same reason — the frozen layout carries no authority
field to gate against, so a gate here would be an invented ABI. What bounds the
plane instead is that every address is content- or nonce-derived: a caller names
evidence and the address follows.

`Endow` is not permissionless: its signer must be the position's own owner.

### The creation step, and the two checks that make it falsifiable

`create_pda_account` refuses a target that is not an empty, system-owned,
writable slot (`AlreadyInitialized`, **before** any CPI), refuses a space above
the runtime's 10,240-byte per-instruction growth ceiling, computes the
rent-exempt minimum, invokes the system program with the target's seeds, and
then checks that an account actually appeared — `target.data_len() == space &&
target.owner == program_id`.

That last check is not ceremony. `solana_cpi::invoke_signed` compiles to
`Ok(())` off `target_os = "solana"`, exactly as the token lane measured, so
without it every host path would report a creation that did not happen. It is
the same silent-no-op detector `token.rs` answers with its exact-delta
comparisons, and it means the honest statement about this module is precise:
**the account-creation path is host-tested for its refusals and its written
bytes, and its CPI has not yet been executed on a bank.** Driving it needs a
harness plane that does *not* pre-create the target — which is the harness
lane's edit, not this one's.

### Rent is read from the chain

The `CreateAccount` instruction takes a lamport figure and something has to
decide it. This module reads the **rent sysvar account** — 17 bytes: rate
(`u64`), exemption threshold (`f64`), burn percent (`u8`) — and computes
`(((128 + space) * rate) as f64 * threshold) as u64`, which is
`solana_rent::Rent::minimum_balance` transcribed under a citation. Pinning the
mainnet defaults as constants would have been shorter and would have made this
program's idea of rent a parallel truth to the runtime's. The sysvar is
authenticated like any other evidence account: right key, right length, not
writable, and a threshold that is a finite non-negative number.

At the default parameters (`3,480` lamports/byte-year, threshold `2.0`) the
figures are: Realm 1,378,080; Profile 1,586,880; price grid 4,990,320; terms
12,416,640; order page **28,814,400** lamports (≈ 0.0288 SOL), the widest
account this module creates.

No dependency was added to reach the formula. `solana-rent` would have brought
one transitive crate for one three-line function; restating it under a citation,
with a test that pins the result against the published defaults, is the smaller
thing to audit. That is a judgement call and it is recorded as one.

### The evidence-buffer initializers copy verbatim

`InitPriceGrid` and `InitTerms` take a read-only buffer account holding exactly
the artifact's encoded bytes. Both artifacts are self-certifying, so decoding
the buffer already proves its digest is its own; the intent's declared digest is
compared against that, and the Realm, Profile and grid bindings are compared
against accounts the plane authenticated. Then the buffer is copied **verbatim**
— not decoded into a struct and re-encoded — for two reasons: a terms value plus
its encode buffer is 3.3 KiB on one 4 KiB frame, and a verbatim copy makes "what
did the account become" a byte comparison rather than a trusted round trip.

The consequence is stated rather than hidden: the caller must present the buffer
already carrying the derived PDA bump, and a buffer carrying any other bump is
`WrongBump`. Patching the bump at a byte offset here would be a second
transcription of a layout that `clutch-solana-layout` owns.

### Refusal codes

`error.rs` gains four appends in the reserved `0x0070-0x007f` block; `0x0060`
onward was previously unallocated and `0x0074-0x007f` stay free.

| code | name | fires when |
| --- | --- | --- |
| `0x0070` | `WrongSystemProgram` | the system-program role is not the all-zero, executable system program |
| `0x0071` | `WrongRentSysvar` | wrong key, wrong length, writable, or a non-finite exemption threshold |
| `0x0072` | `AccountCreationFailed` | the `CreateAccount` CPI refused, or returned without creating anything |
| `0x0073` | `EvidenceBufferMismatch` | a policy or artifact buffer is not the artifact the intent names |

Reused, unchanged: `AlreadyInitialized` (`0x0040`) for a target that already
exists, `WrongBump` (`0x000a`) for a buffer carrying the wrong bump,
`CollateralCap` (`0x0012`) for an endowment past the market's immutable cap,
`Replay` (`0x000d`), `UnauthorizedActor` (`0x0011`), `NotActive` (`0x0016`), and
`Arithmetic` (`0x0013`).

### `Endow`: the internal-ledger half of a deposit

`Endow` credits `PositionAccount::cash_atoms` and advances the replay sequence.
It moves no collateral, touches no Hoard, and is backed by nothing — the value
leg is `token::transfer_checked` into the Hoard token account, constructed and
wired by nothing (`TOKEN2022_PLAN.md`: "constructed, not wired").

This is the harness's existing conjuring, promoted to an instruction — and as
of this regeneration, promoted in the harness too. The bring-up fixture *used
to* write an opening `cash_atoms` into a position account before any
transaction ran, and `LIFECYCLE_WALK.md` §item 2 named that as the sharpest gap
in the walk. It no longer does: step 2 of the walk is an `Endow`, measured at
81 675 units, and the walk's opening state is now byte-for-byte exactly what
`CreateMarket` writes with nothing added. A number in a fixture has no signer,
no sequence, no log line and no ceiling. This one now has all four, and is
still unbacked, which is the residue the walk's item-2 entry states.

One ceiling is real and is stated as what it is: the resulting `cash_atoms` must
not exceed `MarketAccount::collateral_cap`. That is **necessary and not
sufficient** — the market's immutable cap bounds all collateral it may ever
hold, so a fortiori it bounds any one position's claim on that collateral, but
the sufficient check needs a market-wide cash aggregate no account in the frozen
layout carries. Named, not closed.

The cap is deliberately **not** checked against `HoardAccount::collateral_atoms`:
the Hoard's number is escrowed collateral backing complete sets, which only
`Split` raises, and cash is the un-escrowed balance beside it. Adding the two
would double-count the same atoms across the seam they are defined by. A test
asserts the Hoard is unmoved by an endowment.

### No reference oracle — the differential must not be pointed here

`clutch_solana_reference::apply` refuses every intent in this module with
`UnsupportedIntent`, exactly as it refuses `PlaceOrder` and `CancelOrder`
(deferred check 16). So a differential run over this family would be this
program agreeing with a refusal, which is worth nothing. The host tests use the
**layout codecs** as the oracle instead: every expected account is produced by
`clutch-solana-layout`'s own encoder and compared byte for byte, and no expected
byte is typed by hand. The SVM `endow` case inherits exactly that weakness and
declares it in the plan (`oracle = "layout re-encode (the offline reference
refuses Endow: UnsupportedIntent)"`); what it adds over the host test is that a
real bank executed it, and that every byte of the position and replay accounts
*other than* the two the transition moves came back untouched. Closing this properly means an order-and-genesis
transition family in `programs/solana-reference`, which is the same open row the
order family has.

### What this module does not do

* **No `InitEpoch`.** `InitOrderPage` requires an epoch account to already
  exist, and on a fresh chain nothing creates one; the harness loads the epoch
  plane at genesis. The epoch is a ten-identity account whose freeze semantics
  belong with the order-set commitment work, and fixing a wire format for a
  freeze this program cannot perform would be the wrong order. Gap-ledger row 14
  is therefore **half** closed: page creation is representable and driveable,
  epoch creation and epoch freeze are not.
* **No clearing accounts.** The two accounts of `clutch_solana_layout::clearing`
  landed as codecs and are consumed by nothing. The checkpoint is also the one
  account in the inventory a single CPI cannot allocate — 48,750 bytes against
  the 10,240-byte growth ceiling — so its creation is a five-instruction realloc
  sequence or a client-signed top-level `CreateAccount`; both are analyzed in
  `SOLANA_LAYOUT.md`, neither is implemented, and `create_pda_account` refuses
  the space outright rather than emitting a CPI that would fail deeper.
* **No SVM leg.** Nothing in this family has an accepting transaction on a bank
  yet, for the reason above: the harness pre-creates every account, so it cannot
  present the empty target these instructions require.

### Frames

`cargo-build-sbf` on the pinned toolchain emits **zero** frame diagnostics for
any `clutch_sbf` function, this module included. The diagnostic set is
byte-identical to the pre-genesis build: thirteen diagnostics, all in
`clutch_solana_reference` and in the two host-only `OrderPageAccount` decoders
of `clutch_solana_layout`, none of which the entrypoint reaches. The new layout
module (`clearing`) adds none of its own, which is the measurement its
streaming-only shape was chosen for.

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

The gate needs `python3`, `curl`, and the pinned patched validator built by
`tools/agave-loopback-validator/build.sh`. It refuses stock or copied
`solana-test-validator` binaries after checking the cached build record against
tracked source/patch/toolchain pins, then retains exact-PID listener proofs
before and after protocol traffic. It binds
`127.0.0.1:18899` and `127.0.0.1:19900` by default (`CLUTCH_RPC_PORT`,
`CLUTCH_FAUCET_PORT` override) and contacts nothing else. It uses **one**
validator session for all 26 per-family transactions, the falsifiability
self-check, the ten transactions of the
[lifecycle walk](LIFECYCLE_WALK.md), and the walk's own two self-checks. If a previous run was interrupted before its `trap` fired, an
orphaned validator will still hold the RPC port; `pkill -f solana-test-validator`
before re-running.

## Correct description

"A bring-up SBF program implementing sixteen instruction families, eight of
which execute inside a local simulated bank on regenerated fixtures with account
validation, address derivation, and post-state bytes agreeing byte for byte with
the offline reference adapter — or, where no reference transition exists, with an
independent re-encode that the reference's own validator accepts. The
Token-2022 legs are **mandatory and driven**: a `Split` moves real collateral
between the actor's token account and the Hoard's, a `Materialize` mints a real
outcome token, and a `CreateMarket` creates the mints and the Hoard token
account itself through seven CPIs. `Endow` is driven with no reference oracle.
`PlaceOrder`, `CancelOrder`, and the five account-creating genesis initializers
are implemented with host tests only: no reference oracle and no SVM leg.
`SettlePage` is the one remaining stub."

It is not a complete program, not verified, not audited, and not authorization
to deploy anywhere. Every transaction is simulated with signature verification
off and nothing is committed, so "the actor signed" remains a message-header
fact. `PlaceOrder`, `CancelOrder`, and the five creating genesis initializers
have no SVM evidence; settlement is a stub. The account-creation CPI has run on
a bank only through `CreateMarket`; `genesis.rs`'s five initializers have never
executed. `Endow` credits internal cash that no collateral backs, by design and
in writing. The window identity and the feed summary digest are recorded and
never verified, because the program owns no hash **syscall** — it does own a
software SHA-256 in `clutch-solana-layout`, which the collateral leg runs twice
per transaction and pays about 135 000 compute units for.


> **Token-plane regeneration record (2026-08-19).** The staleness notice that
> stood here is discharged. Commits `472b7fe` (genesis plane) and `50c6e35`
> (token completion) made the token legs mandatory and changed every account
> count; the gate exited 1 with `AccountCount` (`0x0001`) refusals on every
> family until the harness was regenerated. It now emits the 16/13/19/21-account
> planes, installs a real collateral mint and the per-market Token-2022 plane at
> genesis, and derives the Profile identity as
> `ParentProfile::from_policy(policy).identity()` — which moved every PDA in
> every fixture. Full gate at `HEAD = 7ce4c09`: **bring-up PASS** (10 accepting
> transactions, 22 refusals, 0 undrivable, 195 genesis accounts), **lifecycle
> PASS** (11 steps), all three falsifiability self-checks fired, exit 0. ELF
> sha256 `59c48c482831626ae9d7cb908f4de0e3f93b1572cdd82105c61f2f87bdaad25f`,
> 505 960 bytes, reproduced from a clean `HEAD` extraction. Every pre-token
> number in this document that moved carries its old value beside the new one;
> the pre-token digest `d8a9267c…` and the pre-token CU table are retired
> because the transactions that produced them no longer have valid account
> planes.
