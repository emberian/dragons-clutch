# Observing finalization without locking 24 vacant PDAs

*2026-09-02. Written by the Dealer lane against the 70-unique-lock wall on the
equity Add (selector 1), measured at `83f9e6e6`.*

## The wall, in one measurement

The Dealer equity Add's frame carries **70 unique locks against the runtime's
64-lock ceiling**. It fits a packet through an address table (3,084 legacy bytes
become 964) and is still unsubmittable, because a looked-up address is locked
exactly like an inline one. Classified at `83f9e6e6`:

| class | count |
| --- | --- |
| System-owned, zero-length (`*_staging` cursors) | **24** |
| executable programs | 7 |
| loader programdata | 4 (two of them zero-length placeholders) |
| real state accounts | 35 |
| **total** | **70** |

Six locks close the gap. **Twenty-four of them are accounts that hold nothing**,
and are in the frame to be observed ABSENT.

## What those 24 accounts prove today

`hot_v3::borrow_record_against` is the reader for every Registry record on every
hot route. Its finalization conjunct is four lines:

```rust
|| staging.key != &expected_staging
|| staging.owner != &system_program::ID
|| staging.data_len() != 0
|| staging.is_signer || staging.is_writable || staging.executable
```

"Finalized" means *the staging-cursor PDA at
`[STAGING_CURSOR_PDA_SEED_V1, schema, digest, bump]` does not exist.* That is an
observation of NON-EXISTENCE, and a program cannot make one without the account
in its frame. There is no cheaper instrument for the fact as currently sited;
the cost is intrinsic to WHERE the fact lives, not to how it is read.

## The premise, verified: finalization is terminal

The design below is only sound if a finalized record can never become
un-finalized. **It cannot.** The record lifecycle in
`programs/dclutch-registry-sbf/src/record_v1.rs` has exactly four verbs --
`Begin` (5), `Append` (2), `Finalize` (3), `Abort` (4), `dispatch` at :51 -- and
after `process_finalize` closes the cursor (:497, asserting `is_vacant` at :505)
the two routes that could reopen it are both refused, by construction:

- **`Begin` cannot re-run for that digest.** `authenticate_begin` opens with
  `require_prefunded_vacant(frame.raw)` (:344), and vacancy is
  `owner == system_program && data.is_empty()` (:990). A finalized raw record is
  program-owned and full-length, so a re-publication under the same digest --
  and the recreate half of any close-and-recreate -- refuses there.
- **`Abort` cannot run to close the raw record.** It is the only route that
  closes it (`close_pda_to_zero(program_id, frame.raw)`, :600), and it opens
  with `require_live_record_accounts` (:794), which demands
  `cursor.owner == program_id`, `cursor.data_len() == STAGING_CURSOR_BYTES_V1`
  and `cursor.lamports() != 0` (:799-805). `Finalize` has already made all three
  false.

Both PDAs are derived under the Registry program id and owned by it, so no other
program can close or recreate either. **Finalization is monotone and one-way: a
record that has been finalized once is finalized for the life of the chain.**
That is what makes the fact safe to record once, elsewhere, instead of
re-observing it on every route.

## The change: the fact moves to an account the frame already holds

**Not into the raw record.** The raw record's identity IS
`hash(data) == digest` -- `borrow_record_against` :479 checks exactly that -- so
adding a finalization field changes the digest, hence the record PDA, hence every
stored `ContentId`, every artifact that names one, and every emitted corpus in
the tree. That is the maximal ABI change available and it buys nothing the
alternative does not.

**Into the activation cache.** `frame.activation_cache` is already in every hot
frame, already Registry-owned, already authenticated as the role receipt, and it
**already carries `record_bumps` for exactly these coordinates** --
`family_context.record_bumps().manifest_raw()` / `.manifest_staging()` are what
`borrow_finalized_record_at` uses to derive the two PDAs. The account that knows
where the cursor would be is the natural place to record that it is gone.

Shape: at activation, for each record coordinate the receipt already names, the
Registry observes the cursor vacant once -- it holds both PDAs in that frame
already -- and records `finalized: true` alongside the bump it already stores.
The hot reader then drops the four staging lines and checks the receipt's flag
for that coordinate instead. The raw record's own conjuncts are untouched: PDA
derivation, program ownership, `hash(data) == digest`, rent exemption.

### ABI cost

- **One record format**: the activation receipt gains one bit per record
  coordinate beside a bump it already stores. Its emitter, its decoder, and its
  Lean ABI pin if it has one move together; nothing else does.
- **No change** to the raw record, to any `ContentId`, to any artifact, or to any
  emitted corpus.
- **Frame**: the 24 `*_staging` coordinates leave every hot route's account list.
  Selector 1 goes 70 -> 46 locks, eighteen under the ceiling, and every other
  Dealer row loses the same class.
- **Migration**: a receipt written before the flag exists has no answer for it.
  The reader must refuse rather than assume, so live activations are
  re-authenticated -- `Reauthenticate` already exists as a Registry verb
  (`lib.rs` :340) -- or the flag's absence is a distinct refusal until they are.
  That is the whole of the deployment cost and it should be named in the commit
  that lands it.

### What still needs deciding before it is written

The activation receipt is a per-ROOT artifact and the record set it names is
per-selection. Whether one receipt can carry the flag for every coordinate a
family's routes will read -- or whether the descriptor-selected records
(descriptor, config, and the six artifact records, which are chosen per ACTION,
not per root) need their own site -- is the open question. The 24 split between
those two groups, and only the per-root half is unambiguously the receipt's.

## The open question, answered: the per-action half is the SEAL's, and it is free

*Appended by the Dealer lane, 2026-09-02, from `96c6a083` + this session's
accelerator work.*

The document above left one thing undecided — whether the per-ACTION records
(descriptor, and the six artifact records selected per action rather than per
root) need a site of their own. They do, and **it already exists and already
holds the observation**: the decision-0005 validated-artifact seal.

`process_capability_seal_v1` (`hot_v3/seal.rs`) materializes one write-once,
Trading-owned seal per `(descriptor schema, descriptor digest, action, Trading
semantic release, Market-selected Registry)`. To mint it, it calls
`borrow_finalized_record` on exactly six records — `seal.rs` :581 descriptor,
:594 lifecycle, :615 account profile, :645 request profile, :655 transition,
:669 effect — and `seal_row_v1` (:742) says what it does with the result in its
own first line: *"Record one row from the accounts `borrow_finalized_record`
just authenticated."* Each `SealedRecordRowV1` persists both coordinates, raw
and staging.

`borrow_finalized_record` reaches `borrow_record_against`, whose finalization
conjunct is the same four lines this document quotes. **So the seal is already a
durable, on-chain, write-once record that those six staging cursors were
observed vacant** — taken once, at materialization, under the same Registry, for
the same digests.

Compose that with this document's own verified premise — finalization is
monotone and one-way — and the seal's observation can never be invalidated. Its
seed set joins on the Trading semantic release, so an interpreter upgrade moves
every seal to a fresh address that must be minted afresh; there is no older body
to inherit a stale answer from.

### What follows

- **Per-action (six coordinates): no new field, no ABI change, no migration.**
  The seal carries the fact today. `require_sealed_record_coordinates_v1`
  (`seal.rs` :925) drops its live-staging conjunct; the row's
  `staging_account() != raw_record_account()` check, which is the one that
  matters for the alias shape, is untouched at :910. Every raw-record conjunct —
  PDA equality against the row, Registry ownership, read-only privileges, rent
  exemption, exact width, `hash(bytes) == digest` — is untouched.
- **Per-root: the activation-cache flag this document proposes**, for the
  coordinates `record_bumps` already names — manifest, program set, config — and
  for the product graph.
- **Per-strategy** is a third group the 24 also contains (strategy, certificate,
  admission, artifact release) and it belongs with neither; it is named here so
  the next reader does not have to rediscover that the split is three ways, not
  two.

The frame change itself is still the expensive part and is unchanged by this: the
six coordinates leave `HOT_FIXED_ACCOUNT_COUNT_V3`, which moves every hot
route's account layout, the bundle builders, `admitted_composition_v3`'s
accelerator coordinates, and `apps/dclutch-web`. What this answers is only that
the per-action half needs no new persisted fact and no re-activation to become
sound — which was the half the document could not price.

## Superseded: the frame move is not how these six locks come off

*Appended by the Dealer lane, 2026-09-02, from a measurement.*

Everything above about the per-ACTION six is right and its premise holds. What
it got wrong is the price. This document treats "the six coordinates leave
`HOT_FIXED_ACCOUNT_COUNT_V3`" as the necessary cost -- 209 references over
60-odd files, every family's account layout, both TypeScript trees. **That
frame move is not required to remove the locks, and it should not be done for
this reason.** The mechanism that removes them already exists, is already
authenticated, and is already shipping.

`hot_v3.rs` carries `SEALED_EXECUTION_FIXED_ALIASES_V3`,
`validate_hot_fixed_alias_shape_v3`, and
`HotFixedFrameV3::uses_sealed_execution_aliases`, and `hot_v3/seal.rs`'s
`require_sealed_record_coordinates_v1` takes a `direct_alias_shape` flag. In
that shape each of the six staging coordinates carries **its own raw record
again** instead of the vacant cursor, so the transaction locks one account per
record rather than two -- and the conjunct it stops re-observing is precisely
the one this document proves is already durably recorded by the seal. The
coordinate stays in the frame; only the lock goes away. Direct ordinary
execution has been submitting exactly this shape.

Nothing scopes it to Direct except one exact-equality family gate in
`authenticate_and_execute_hot_v3`, which compares
`frame.uses_sealed_execution_aliases()` against
`kind == DIRECT_SUCCESSOR_KIND_ID_V3 && action == InlineOrdinary`. Widening
that predicate to the Dealer kind is the whole of the on-chain intent.

### The measurement

Real ELFs, `dealer-accelerator-sbf/program-test/tests/accepted.rs`, the frame
built with the six per-action staging coordinates aliased onto their raw
records:

| row | before | after |
| --- | --- | --- |
| LP-hot | 54 unique locks | **48**, measured |
| equity Add (selector 1) | 70 unique locks | **64**, by the same six |

Minus six exactly, which is what the arithmetic requires: the frame is proven
duplicate-free by `validate_hot_fixed_alias_shape_v3`, so each of the six
staging PDAs contributes exactly one distinct key and aliasing removes exactly
six. 64 is the devnet ceiling
(`dclutch_operator::dealer_scenario_hot_v4::SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1`),
so selector 1 reaches it with no headroom, and the per-ROOT and per-STRATEGY
cursors this document also names are where any further margin has to come from.

### Two blockers, both measured, neither previously known

The alias shape is not free for the Dealer family, because the Dealer family
reaches its accelerator and the Direct family does not.

1. **`HotFrameV3::parse_accelerator_readonly` carried its own bare pairwise
   distinctness loop** instead of calling `validate_hot_fixed_alias_shape_v3`.
   That copy is the strictly older rule -- it refuses the alias shape the
   shared authority exists to admit -- and since every AdmittedAot family
   reaches its accelerator through that function, the duplicate silently
   confined the alias shape to families that never take the path. Replacing the
   loop with the call is both the fix and one fewer parallel authority.

2. **`admitted_composition_v3::require_record_pair` refuses the aliased
   descriptor pair**, because it derives the staging-cursor PDA and compares.
   Convicted, not read: with the gate widened and blocker 1 fixed, LP Open
   refuses `Custom(16407)` = `0x4017 AdmittedFrame`. The shape bit has to reach
   that validator -- through `AdmittedCpiFrameV3`, whose accelerator-side parse
   must carry it too -- so the descriptor pair admits `staging == raw` exactly
   when the family does. Note that
   `execution_strategy_v2::authenticate_common_frame_with_sealed_capability_pair`
   ALREADY handles both shapes (`let capability_is_aliased = raw.key ==
   staging.key`), which is the evidence that the mechanism was built general
   and only this one validator was left behind.

### What is still owed

The on-chain half above; `DEALER_KIND_ID_V2` as a pinned literal beside
`DEALER_KIND_PREIMAGE_V2`; the producers writing the raw key into the six
staging slots (the campaign's is one loop in the bundle builder, and the frame
SHAPE has to come from one declaration both the executor and the builders
read); eleven hard-coded lock-count assertions in the operator crate re-pinned
from runs rather than edited to taste; the TypeScript projector in both trees,
which today would render an aliased frame as one address printed twice; and the
existing sealed-execution-alias hostile extended to the Dealer kind. Two ELFs,
both owing frame rows. It is one green series or it is nothing: the gate
compares with `!=`, so a producer and an executor that disagree about the shape
refuse in either direction.

A partial implementation is saved at
`unit2-sealed-alias-partial.patch` in the lane scratchpad; it is deliberately
unlanded, because a half-landed frame shape breaks the family it half-lands in.

## Both blockers are retired, and the price of the row is three producers

*Appended by the Dealer lane, 2026-09-02, at `aa72e3a09`.*

The two blockers above are gone, and the shape now has ONE declaration instead
of three. `SEALED_EXECUTION_FIXED_ALIASES_V3` (what the shape is -- the six
`(raw, staging)` pairs, formerly spelled privately in the executor AND again in
`dclutch-operator`'s Direct projector) and `SEALED_EXECUTION_ALIAS_FAMILIES_V3`
(who submits it, formerly one inline `kind == … && action == …`) live beside
each other in `dclutch-capability-program-contract::hot_v3`, and the executor,
the operator and every builder read both from there.
`parse_accelerator_readonly` calls `validate_hot_fixed_alias_shape_v3` instead
of its own older loop, and `require_record_pair` takes the shape -- for the
DESCRIPTOR only, because the other four records it checks are per-STRATEGY and
the seal never witnessed their cursors.

**The Dealer row is still not there, and its price is not on-chain work.** With
the row added and the campaign's bundle builder writing the raw key into the
six staging slots, measured on real ELFs at `1f41f40a`: LP-hot **54 -> 48**,
the equity Add **70 -> 64**, the post-trade partial Remove **71 -> 65**,
campaign unchanged at 30/1. What the row costs is three producers, and every
one of them is outside the programs:

- `apps/dclutch-web/lib/dealerEquityChain.ts:145` THROWS on a staging
  coordinate that is not the derived cursor, so the first aliased Dealer frame
  breaks the browser. `packages/dclutch-sdk/lib/dealerEquityV3.ts` is the twin.
- `dclutch-operator`'s Dealer route has no projector for the shape and no guard
  that refuses the wrong one before a transaction is signed. The executor
  compares with `!=`, so a producer that builds the distinct frame is refused
  `TradingSbfError::Content` on chain -- one of 2,124 sites -- rather than
  told.
- `registry_hot_continuation`'s `SealedExecutionAliasHostile` covers the Direct
  kind only.

And a fourth thing the next lane must not mistake for its own regression:
`dclutch-operator`'s `final_commit_topology_reports_dense_selector_nine_lock_wall`
and `unsplit_topology_derives_spans_and_proves_devnet_refusal` are RED at
`3e7f06cc`, before any of this. They are `efca6966`'s cross-frame alias
partition re-measured -- 107 against a pinned 117, 117 against a pinned 122 --
whose profile pins were re-pinned from runs and whose operator pins were not.

## The row's price, re-surveyed: five copies were seven, and the Remove is no longer lock-bound

*Appended by the Dealer lane, 2026-09-02, at `9c133b27c`. The row is STILL not
landed, and this section is why the next lane's series is shorter than the one
above described.*

### Two more copies of the six pairs existed, and they are gone

`aa72e3a09` said three authorities became one. There were five. The two it did
not reach are both in `programs/dclutch-trading-sbf/program-test/direct-hot`,
and neither spells the constant's name, which is why a grep for it found
neither:

* `fixture.rs::alias_sealed_execution_metas` -- the pass that BUILDS the
  aliased frame every Direct-hot test submits;
* `waist.rs::has_canonical_sealed_execution_aliases` -- the predicate that
  CHECKS one.

Both now read `SEALED_EXECUTION_FIXED_ALIASES_V3`, retired at `9c133b27c` at
zero behaviour change. **`fixture.rs::alias_sealed_execution_metas` is also the
exact model for the Dealer producer the row needs**: it takes the built metas,
copies each raw meta over its staging coordinate, and refuses if either is a
signer or writable. The Dealer campaign's builder needs the same pass, keyed on
`hot_frame_uses_sealed_execution_aliases_v3(kind, action)` rather than applied
unconditionally.

### The browser's refusal, named at its conjunct

`apps/dclutch-web/lib/dealerEquityChain.ts:145` is
`derived.staging !== stagingAddress`, inside the per-record finalized-content
check. Under the alias shape the staging COORDINATE carries the raw record's
address, so the derived-PDA equality is the wrong question for an aliased
family and the vacancy check on the line below it has no account to make. The
repair is not to delete either: it is to ask
`hot_frame_uses_sealed_execution_aliases_v3` first and require the coordinate
to equal `rawAddress` exactly when it says so -- which is a STRICTER check than
the one it replaces, and the same `!=`-shaped agreement the executor makes.
`packages/dclutch-sdk/lib/dealerEquityV3.ts` is the twin.

### The operator's gap, named at its conjunct

`crates/dclutch-operator/src/dealer_equity_hot_v3.rs::validate_fixed_frame`
walks all thirty-nine fixed accounts requiring `!is_signer` and
`is_writable == (index == HOT_ROOT_ACCOUNT_V3)`. It never asks whether two
coordinates hold one key, so it neither requires the alias nor forbids it: it
signs whichever frame it was handed. `direct_inline_route_v3.rs`'s
`project_direct_inline_sealed_execution_physical_v3` is the shape to mirror,
and its `has_duplicate` guard on the pre-projection frame is the half that
makes the projection safe.

### The Remove is no longer lock-bound, which changes what the row buys

`c3e14e096` closed the borrowed-witness wall, and the post-trade partial equity
Remove now runs to its Claims child and exhausts the **1,400,000-CU transaction
ceiling** -- see `docs/design/BORROWED_WITNESS_TWO_SPELLINGS_2026_09_02.md` for
the per-phase decomposition. Its 71 locks were never the binding constraint on
that action and 65 will not be either.

So the row's measured value is now:

| route | locks today | with the row | binds on |
| --- | ---: | ---: | --- |
| LP-hot | 54 | 48 | nothing; comfortable |
| equity Add | 70 | **64** | exactly `SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1` |
| partial Remove | 71 | 65 | COMPUTE, at 1.4M, long before 64 |

The Add is the case the row is for: it reaches the devnet limit with zero
headroom, and it is an action that completes. The Remove's sixty-fifth lock is
a question for after its compute wall, not before it.
