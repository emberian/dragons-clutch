# SBF bring-up: the instruction set, executed against the reference oracle

Status: **host-differential evidence for eight instruction families**
(Split, Merge, Materialize, Dematerialize, CreateMarket, FeedAdvance,
evidence-gated Resolve, RedeemInternal), each mirroring the offline
reference adapter's semantics with byte-level differential tests, plus
SVM execution evidence for Split only (the recorded run predates the
ten-account CLO-DELTA plane; the harness fixtures are stale and the SVM
leg for every family is owed to the regeneration wave). Order placement,
cancellation, and batch settlement remain honest stubs pending the
orders_batch lane. Not a complete program, not audited, not a deployment
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

This lane answers that for one instruction. It produces:

1. a real deployable SBF ELF with a `entrypoint` symbol, built reproducibly by
   the pinned `cargo-build-sbf`;
2. a program that validates hostile `AccountInfo` metadata, derives and checks
   every program address, decodes through `clutch-solana-layout`, transitions
   through `clutch-kernel`, and writes back into runtime account data;
3. a **differential result**: for one fixture, the six state accounts after a
   real SVM execution are byte-identical to the offline reference adapter's
   post-state; and
4. three adversarial refusals that refuse in the SVM with the same meaning the
   offline adapter gives them.

It does **not** establish correctness of the protocol, of the kernel, of the
layout, or of any economic claim. It establishes that this one instruction
survives the account-facing boundary on the pinned toolchain.

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
| `instructions/market_init.rs` | `Intent::CreateMarket` | one lane, stub today |
| `instructions/observe_resolve.rs` | `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` | one lane, stub today |
| `instructions/orders_batch.rs` | `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SettlePage` | one lane, **blocked**, see below |

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
in which case this program mirrors that reason exactly:

| action | refusal | why this one |
| --- | --- | --- |
| `CreateMarket` | `AuthorizationUnavailable` `0x000f` | no authority model exists; code is not the missing part |
| `Resolve`, `RedeemInternal` | `ResolutionEvidenceUnavailable` `0x0010` | the typed evidence plane has no on-chain counterpart, and the fail-closed default must stay a missing code path |
| `Merge`, `Materialize`, `Dematerialize` | — **implemented**, no blanket refusal | see below |
| `FeedAdvance` | `NotYetImplemented` `0x0017` | nothing structural is missing |
| `PlaceOrder`, `CancelOrder`, `SettlePage` | `NotYetImplemented` `0x0017` | no adapter, offline or on-chain, joins these layouts to a transition |

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

The other instruction families (`market_init`, `observe_resolve`,
`orders_batch`) own their own account planes; this section describes the seam
plane only.

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
   (`NotYetImplemented`); all three now run through the seam plane and are
   covered by the host differential. `Merge` closed on *both* sides at once,
   because the gap was mis-stated: the offline adapter did not implement it
   either (see the correction under
   [Refusal discipline](#refusal-discipline-for-the-stubs)). What is still open
   is the **SVM leg** for the whole seam family — `harness/` emits a
   nine-account `Split` transaction, so no emitted transaction exercises the
   ten-account plane at all; see item 13.
2. `validate_market_init` has no on-chain counterpart: there is no
   initialization instruction, so every account in the fixture is preloaded at
   genesis instead of being created and validated by the program.
3. `CreateMarket` refuses with `AuthorizationUnavailable` and
   `Resolve`/`RedeemInternal` refuse with `ResolutionEvidenceUnavailable`, which
   matches the offline adapter — these are intended refusals, not gaps, but they
   mean the program cannot bring a market into existence at all.

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
10. Multi-position aggregate closure remains refused by representation, exactly
    as in the offline adapter (obligation 11).
11. The resource envelope is one compute measurement for one fixture. Heap,
    account count, transaction size, and worst-case outcome counts are unmeasured
    (obligation 10).
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
13. **Partly closed, and the remainder moved.** This item used to read "the
    supply ledger is loaded but not used, and this is now a divergence": the
    ledger had a seed, a canonical address, and genesis bytes, but `Split` did
    not take it, so this program still checked the retired single-position
    equality `internal + external == total_supply` while the offline adapter had
    moved to CLO-DELTA-V1 (commit `9c43863`,
    [`MULTI_POSITION_CLOSURE.md`](MULTI_POSITION_CLOSURE.md)).

    **The host side of that is discharged.** The seam plane now takes the ledger
    as a tenth account and carries the three obligations through the shared
    primitives — `accounts::require_two_term_closure` (C1),
    `accounts::require_representation_bound` (C2), and two
    `accounts::apply_ledger_delta` calls (C3) — for all four seam intents,
    `Merge` included. A market holding a second position is representable and
    differentially tested against the reference on both `Split` and `Merge`, so
    the concrete symptom the old text named ("it refuses every market holding a
    second position") is gone.

    **The SVM side is not, and it regressed while the host side advanced.**
    `harness/` is frozen and still emits a *nine*-account `Split` transaction,
    so the transactions under `tx/` no longer match the instruction's account
    count: the SVM leg of the differential is stale rather than merely
    incomplete, and `simulate.py` still does not compare `expected/supply.hex`.
    No emitted transaction exercises `Merge`, `Materialize`, or `Dematerialize`
    at all. Regenerating the fixtures for the ten-account plane, and adding
    transactions for the other three seam intents, is a named harness-lane wave
    and is the whole of what item 13 still owes.

14. **An order page cannot be decoded on-chain at all.** See
    [Stack findings](#stack-findings) — this blocks the whole
    `orders_batch` family and the fix belongs in `clutch-solana-layout`.

## How the transition is executed

The harness never signs. Every identity it uses — program id, fee payer, actor,
a stranger, and an imposter address — is a System-program PDA of a fixed literal
seed, so the fixture is reproducible and this lane holds no key material of any
kind.

`scripts/run_bringup.sh` starts a `solana-test-validator` bound to loopback with
the ELF at the chosen program id and nineteen accounts loaded at genesis, then
`scripts/simulate.py` sends each transaction to `simulateTransaction` with
`sigVerify: false`, `replaceRecentBlockhash: true`, and an `accounts` request for
the six state accounts, and compares the returned data against the reference
post-state.

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

Command: `programs/clutch-sbf/scripts/run_bringup.sh`, recorded against commit
`ad1e330` on `aarch64-apple-darwin`.

The ELF is a function of the `clutch-kernel`, `clutch-solana-layout`, and
`clutch-solana-reference` sources, all three of which changed while this lane
ran. A later commit to any of them changes the hash; that is correct behaviour,
not a reproducibility failure. The reproducibility claim is that two builds of
one source tree agree.

### Reproducible ELF

Built twice into fresh target directories:

```text
pass 1  sha256=42d553132b0a22ebffd374c85d12a444e4ca8c3e99aa211322c5b8a947467cdd  bytes=102568
pass 2  sha256=42d553132b0a22ebffd374c85d12a444e4ca8c3e99aa211322c5b8a947467cdd  bytes=102568
sbf_reproducibility=PASS
```

Dynamic symbols: exports `entrypoint` and `custom_panic`; imports the syscalls
`sol_try_find_program_address`, `sol_log_`, `sol_panic_`, `sol_memset_`,
`sol_memcpy_`, `sol_memcmp_`, and `abort`. There is no CPI syscall and no token
program reference, because there is no CPI and no token code.

### Differential against the offline reference adapter

One `Split` of quantity 5 on a two-outcome market, from a fixture that mirrors
the offline adapter's own `Split` test: position generation 2, replay sequence 0,
position cash 100, reserved cash 7, collateral cap 1000, Hoard collateral 0, all
balances zero.

The offline adapter computes the expected post-state; the SVM executes the ELF;
the six returned accounts are compared byte for byte.

```text
accept: executed, unitsConsumed=72869
differential market    MATCH (unchanged by Split)
differential hoard     MATCH (changed by Split)
differential position  MATCH (changed by Split)
differential kernel    MATCH (changed by Split)
differential external  MATCH (unchanged by Split)
differential replay    MATCH (changed by Split)
```

`market` and `external` are unchanged by `Split` and match the reference's
re-encoded post-state; `hoard`, `position`, `kernel`, and `replay` change and
match exactly.

Diffing the pre-state and post-state bytes shows that the SVM execution changed
exactly the fields the offline adapter's own byte-evidence section names, at the
same offsets:

| account | offset | before | after |
| --- | --- | --- | --- |
| Hoard | collateral at `98..106` | 0 | 5 |
| Position | outcome 0 at `74..82` | 0 | 5 |
| Position | outcome 1 at `82..90` | 0 | 5 |
| Position | cash at `202..210` | 100 | 95 |
| kernel aggregate | supply 0 at `38..46` | 0 | 5 |
| kernel aggregate | supply 1 at `46..54` | 0 | 5 |
| replay | sequence at `74..82` | 0 | 1 |

No other byte of any of the six accounts changed.

### Refusals

Each adversarial case is run in the SVM and cross-checked against the offline
adapter's refusal for the same situation.

```text
refusal refuse-unsigned  Custom(0x0002) (offline reference: MissingSignature)
refusal refuse-stranger  Custom(0x0011) (offline reference: UnauthorizedActor)
refusal refuse-imposter  Custom(0x0009) (offline reference: WrongAccountKey)
```

- `refuse-unsigned` presents the position owner as a read-only, non-signing
  account.
- `refuse-stranger` presents a different authenticated signer.
- `refuse-imposter` presents byte-identical replay-account state at an address
  that is not the canonical replay PDA. Every decode and every linkage check
  passes on it, so only address derivation can refuse it. The offline adapter
  refuses the same situation as `WrongAccountKey` because it is handed a trusted
  binding; the program derives the address instead, which is the stronger check
  obligation 1 asks for.

### The wave-3 plane is loaded but not transacted against

Genesis carries the nine accounts the `Split` instruction takes, the imposter
replay account, and nine more that no transaction in this plan touches: the
supply ledger, the immutable terms artifact and its price grid, the resolution
record, the feed head, and one epoch with its frozen order page, selected
candidate, final pot, and settlement receipt. They are loaded so that an
instruction lane inherits a real, bound, canonically addressed plane instead of
inventing one, and so that `manifest.txt` already lists the addresses the seed
schema produces.

What that fixture claims is narrow and checked. Every account decodes through
its frozen codec, and every *identity* binding the layout crate can decide is
asserted while the harness builds it — terms to market, supply ledger to market,
grid to terms, epoch to terms and grid, epoch to its frozen page set, candidate
to epoch, pot and receipt to candidate, resolution to terms. A fixture that
drifted apart fails `cargo test` rather than shipping as a genesis nobody
checked. `MarketAccount::terms` in particular is no longer a free byte pattern:
it is the digest of the terms artifact loaded beside it, exactly as in the
offline adapter's own fixture.

What it does **not** claim is any economic coherence. Whether this candidate is
the best valid submitted candidate for this book, whether the pot balances
against the receipts, and whether the prices clear anything are questions for a
batch relation that no adapter runs yet. The fixture is a shape, bound at every
seam a codec owns and at none that it does not.

The window-policy numbers in the terms artifact are copied from the offline
reference adapter's own resolution fixture, so that a future resolution
differential is a disagreement between two adapters rather than between two
scenarios. The batch-auction numbers have no reference counterpart to copy —
no adapter implements that family — so they are the smallest shape the frozen
codecs accept.

### Re-run after the module split

The gate was re-run after the restructure, on the same host and toolchain:

```text
sbf_reproducibility=PASS
accept: executed, unitsConsumed=73273
differential market    MATCH (unchanged by Split)
differential hoard     MATCH (changed by Split)
differential position  MATCH (changed by Split)
differential kernel    MATCH (changed by Split)
differential external  MATCH (unchanged by Split)
differential replay    MATCH (changed by Split)
refusal refuse-unsigned  Custom(0x0002) (offline reference: MissingSignature)
refusal refuse-stranger  Custom(0x0011) (offline reference: UnauthorizedActor)
refusal refuse-imposter  Custom(0x0009) (offline reference: WrongAccountKey)
```

Six of six accounts byte-identical and three of three refusal codes unchanged,
which is what pins "the module split moved code and did not change behaviour".

Two honest qualifications. The ELF digest from this run is deliberately **not**
recorded here: it was measured on a working tree with other lanes live, and a
digest that names no commit is not a reproducibility record. Re-record it from a
clean tree. And the compute measurement moved from 72 869 to 73 273 units, which
is a re-measurement on changed dependencies and a new routing match, not a
before/after comparison of one variable.

### The differential is falsifiable

A comparison that cannot go red is not evidence. Mutating the reference
expectation for one field — Hoard collateral `5` to `6` at byte 98 of
`plan/expected/hoard.hex`, leaving the program and the SVM run untouched — turns
the check red on exactly that account and leaves the other five green:

```text
FAIL
  differential hoard: on-chain bytes != reference bytes
```

Reproduce by running the gate once, editing that byte in the work directory, and
re-running `scripts/simulate.py --plan <work>/plan` against the still-running
validator.

### Resource envelope

`unitsConsumed=72 869` for the accepting `Split`, against a 200 000
compute-unit default. Address derivation dominates: eight
`sol_try_find_program_address` calls. This is one measurement of one fixture with
two outcomes; it is not an envelope.

### Stack findings

The SBF backend reports a function whose frame exceeds 4 KiB as an `Error:` line
that does **not** fail the build. An SBF program that overflows its frame is
undefined behaviour at execution time, so these lines are the only warning there
is, and `scripts/run_bringup.sh` greps them out of the build log on every run.

As of the run below, every reported function belongs to `clutch-solana-layout`
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

Reference-crate functions are dead-code-eliminated from this ELF and are never
called by a program. The finding still stands on its own as obligation-10
evidence about the *offline adapter's shape*: its by-value, whole-state calling
convention does not fit an SBF frame.

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
`ORDER_SLOT_BYTES` scratch slot instead of buffering a page. Until it exists, no
on-chain instruction can read an order page, and the `orders_batch` family
cannot be started. `verify_page_set` has the same problem one level up: it takes
pages by value as a slice.

## Reproducing

```sh
programs/clutch-sbf/scripts/run_bringup.sh          # full gate, ~1 minute
cargo test   --manifest-path programs/clutch-sbf/Cargo.toml
cargo clippy --manifest-path programs/clutch-sbf/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path programs/clutch-sbf/Cargo.toml --no-deps
cargo fmt --manifest-path programs/clutch-sbf/Cargo.toml --all -- --check
```

`cargo test` needs the pinned `solana` CLI on `PATH` or in `SOLANA_BIN`: address
derivation is not compiled into these crates, so both the harness fixture and
its decoder test derive out of process.

The gate needs `solana-test-validator`, `python3`, and `curl`. It binds
`127.0.0.1:18899` and `127.0.0.1:19900` by default (`CLUTCH_RPC_PORT`,
`CLUTCH_FAUCET_PORT` override) and contacts nothing else.

## Correct description

"A bring-up SBF program whose instruction set is routed but of which one
instruction is implemented, whose account validation, address derivation, and
post-state bytes agree with the offline reference adapter on one single-position
fixture under a local simulated bank." It is not a complete program, not
verified, not audited, and not authorization to deploy anywhere. Its closure
check is currently stricter than the reference adapter's and refuses any market
with a second position.
