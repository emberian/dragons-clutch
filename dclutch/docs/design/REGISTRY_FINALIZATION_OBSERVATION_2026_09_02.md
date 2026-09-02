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
