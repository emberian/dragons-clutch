import DClutchSemantics.Codec
import DClutchSemantics.RelayedMainnetStateV1Abi
import Std.Tactic

/-!
# Venue decoding rules for `RelayedMainnetStateV1`

`DClutchSemantics.RelayedMainnetStateV1Abi` owns the *transport*: what a relayer
signs and how a record persists it.  It deliberately names no venue, because the
relayer attests observations and never interpretations.

This module owns the other half — the **interpretation** — and it is the object
`docs/research/CHAIN_STATE_SOURCES_2026_08.md` §6.3 calls the decoding rules: the
owning program, the account discriminator, the admitted data-length set, the
field offsets, the sentinel semantics, and the derived observation.  Nothing here
is signed by anybody.  It is applied on the observing cluster, by the adapter, to
bytes a quorum already certified, which is what makes "swapping trust roots never
moves semantics" a property rather than a slogan.

## Where these rules live at runtime, and why

`RelayedAdapterConfigV1.observable_selector` selects one row of the table below,
and the config record's content identity is `ProviderReleaseV1.decoding_rules_id`.
The table itself is carried by `ProviderReleaseV1.adapter_release_id` — it is
code, pinned by an immutable adapter release, in exactly the way `PythAdapterConfigV1`'s
`feed_id`/`exponent` select a row of a Pyth codec the adapter also carries.

This amends `docs/design/MAINNET_STATE_RELAY.md` §4.10, which says
`decoding_rules_id` carries "every layout fact, offset, sentinel, scale, and
rounding boundary".  The tripwire §4.10 states is unchanged and still executable:
`decoding_rules_id` is byte-identical across every trust-root row, because the
adapter configuration is byte-identical.  What moved is where the *layout* half
is stored — the config selects a row rather than transcribing one — and the
reason is that widening the 80-byte configuration to inline a venue grammar would
change `decoding_rules_id` for every existing row and defeat the tripwire it was
built to arm.

## The venues

**Row 0 — Meteora Dynamic Bonding Curve**
(`dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN`), graduation state only.
*verified-from-source* against `MeteoraAg/dynamic-bonding-curve @ 3b540e94`,
`programs/dynamic-bonding-curve/src/state/virtual_pool.rs`, and confirmed against
live mainnet bytes; recorded in `MAINNET_STATE_RELAY.md` §10.1.

**Row 1 — SPL Token-2022 mint authority renunciation.**  *verified-from-source*
against `spl-token-interface 3.0.0` `src/state.rs` (`impl Pack for Mint`,
`LEN = 82`, the `array_refs![src, 36, 8, 1, 1, 36]` split, and
`unpack_coption_key`'s two admitted four-byte tags) and
`spl-token-2022 4.0.0` `src/processor.rs:722`, whose own comment is the whole
proposition:

> *"Once a mint's supply is fixed, it cannot be undone by setting a new
> mint_authority"* — `mint.base.mint_authority.ok_or(TokenError::FixedSupply)?`

That is terminality of exactly the kind row 0 has, and from the same place: the
observed program refuses the transition out of the state, so an observation of
the state is a proof about every later slot as well as this one.  It is why this
row can be a **terminal-window proposition** rather than a snapshot.

Row 1 is deliberately NOT a price.  The relayed family carries an attested
account snapshot and this row reads a four-byte `COption` tag out of it; the
atom is a two-state discriminant, at scale zero, exactly as row 0's is.  A
second row that WAS a price would have been the first evidence that the payload
is price-shaped after all.
-/

namespace DClutch.RelayedVenueDecodingRulesV1

open DClutch

/-! ## The observable table

One row per `(venue, proposition)` pair.  `observable_selector` is a `u32` in
the adapter configuration, so an unknown selector is a refusal rather than a
default.
-/

/-- Which observable of this release's decoding-rules table a Source produces. -/
inductive Observable where
  /-- Meteora DBC `VirtualPool.migration_progress`, as a graduation proposition. -/
  | dbcMigrationProgress
  /-- SPL Token-2022 `Mint.mint_authority`, as a renunciation proposition. -/
  | token2022MintAuthorityRenounced
  /-- A Feature program account's `activated_at`, as "activated by slot `S`":
  the decision parent of the flagship conditional market (decision 0029, tenth
  item).  Both branches are OBSERVED: `Some(a)` yields `a`, `None` yields a
  sentinel above every slot, and the Product's cut at `S + 1` separates them. -/
  | featureGateActivation
  /-- Mainnet's mean slot duration since the current epoch began, in
  milliseconds, from the `Clock` and `EpochSchedule` sysvars alone: the metric
  parent of the flagship.  One attested record, one atom, no venue program. -/
  | clockMeanSlotTimeSinceEpochStart
  deriving DecidableEq, Repr

namespace Observable

/-- Every row of this release's table, for the properties that quantify over
all of them.  Exhaustiveness is carried by the `cases` in the theorems below:
a row added to `Observable` and not to this list fails `all_rows_is_complete`. -/
def allRows : List Observable :=
  [.dbcMigrationProgress, .token2022MintAuthorityRenounced, .featureGateActivation,
    .clockMeanSlotTimeSinceEpochStart]

/-- The `RelayedAdapterConfigV1.observable_selector` value naming this row. -/
def selector : Observable → Nat
  | .dbcMigrationProgress => 0
  | .token2022MintAuthorityRenounced => 1
  | .featureGateActivation => 2
  | .clockMeanSlotTimeSinceEpochStart => 3

/-- The declared base-ten scale of the atom this row produces.  A discrete
state is a count of states, so every row's scale is zero. -/
def rawExponent : Observable → Int
  | .dbcMigrationProgress => 0
  | .token2022MintAuthorityRenounced => 0
  -- a slot number is a count of slots
  | .featureGateActivation => 0
  -- milliseconds: the atom times ten to this power is seconds
  | .clockMeanSlotTimeSinceEpochStart => -3

/-- Hostile selection.  An unknown selector has no row and therefore no
interpretation; it must not fall through to row zero. -/
def ofSelector (value : Nat) : Option Observable :=
  if value = 0 then some .dbcMigrationProgress
  else if value = 1 then some .token2022MintAuthorityRenounced
  else if value = 2 then some .featureGateActivation
  else if value = 3 then some .clockMeanSlotTimeSinceEpochStart
  else none

theorem all_rows_is_complete (row : Observable) : row ∈ allRows := by
  cases row <;> simp [allRows]

theorem selector_round_trips (row : Observable) : ofSelector row.selector = some row := by
  cases row <;> rfl

theorem the_selectors_are_distinct :
    (allRows.map selector).eraseDups.length = allRows.length := by
  native_decide

theorem an_unknown_selector_refuses (value : Nat) (h : 4 ≤ value) : ofSelector value = none := by
  have h0 : value ≠ 0 := by omega
  have h1 : value ≠ 1 := by omega
  have h2 : value ≠ 2 := by omega
  have h3 : value ≠ 3 := by omega
  simp [ofSelector, h0, h1, h2, h3]

end Observable

/-! ## Roles inside the pinned ordered account set

The set itself is pinned by `account_set_id`, which already binds each
position's key, expected owning program, and inline width.  What the transport
cannot know is what each position *is for*; that is a decoding-rules fact and it
lives here.
-/

/-- What owns the state account, and therefore how the cross-cluster
deployment defense (`MAINNET_STATE_RELAY.md` §4.6) is discharged.

* `loaderV3`: the state is owned by an upgradeable program.  The set carries
  the program's `Program` and `ProgramData` bodies and the adapter rebuilds a
  `DeploymentObservationV1` and authenticates it against the pinned release —
  the Loopscale defense in full, because the venue can be upgraded under a
  market.
* `native`: the state is owned by a program the validator itself implements
  (the Feature program, the sysvar owner).  There is no `ProgramData`, no ELF
  digest and no upgrade authority: what the defense pins is the owner's
  address, which the pinned account-set entry already binds byte for byte and
  which no transaction on the observed cluster can move.  So the two program
  positions are ABSENT from the set, and `require_pinned_venue` is replaced by
  the entry's owner pin plus the address check below. -/
inductive VenueKind where
  | loaderV3
  | native
  deriving DecidableEq, Repr

def Observable.venueKind : Observable → VenueKind
  | .dbcMigrationProgress => .loaderV3
  | .token2022MintAuthorityRenounced => .loaderV3
  | .featureGateActivation => .native
  | .clockMeanSlotTimeSinceEpochStart => .native

/-- Cardinality of one row's ordered account set. -/
def Observable.setCardinality : Observable → Nat
  | .dbcMigrationProgress => 4
  | .token2022MintAuthorityRenounced => 4
  | .featureGateActivation => 2
  | .clockMeanSlotTimeSinceEpochStart => 2

/-- The venue program's `Program` account, for `DeploymentObservationV1`.
Meaningless for a `native` row, whose set has no such position; the emitter
prints it for Loader V3 rows only and `positions` omits it. -/
def Observable.programPosition : Observable → Nat
  | .dbcMigrationProgress => 0
  | .token2022MintAuthorityRenounced => 0
  | .featureGateActivation => 0
  | .clockMeanSlotTimeSinceEpochStart => 0

/-- The venue program's `ProgramData` account; its tail digest *is* the ELF digest. -/
def Observable.programDataPosition : Observable → Nat
  | .dbcMigrationProgress => 1
  | .token2022MintAuthorityRenounced => 1
  | .featureGateActivation => 1
  | .clockMeanSlotTimeSinceEpochStart => 1

/-- The observed account whose bytes carry this row's own state. -/
def Observable.statePosition : Observable → Nat
  | .dbcMigrationProgress => 2
  | .token2022MintAuthorityRenounced => 2
  | .featureGateActivation => 0
  | .clockMeanSlotTimeSinceEpochStart => 0

/-- The observed cluster's `Clock` sysvar: the only source of foreign time. -/
def Observable.clockPosition : Observable → Nat
  | .dbcMigrationProgress => 3
  | .token2022MintAuthorityRenounced => 3
  | .featureGateActivation => 1
  | .clockMeanSlotTimeSinceEpochStart => 1

/-- Every role of one row, in set order.  A `native` row has no program roles. -/
def Observable.positions (row : Observable) : List Nat :=
  match row.venueKind with
  | .loaderV3 => [row.programPosition, row.programDataPosition, row.statePosition, row.clockPosition]
  | .native => [row.statePosition, row.clockPosition]

/-- No row may put two roles on one position: two roles reading one body is
the defect the Rust interpreter's `require_well_formed` refuses, and this is
the reason it can never fire for a shipped row. -/
theorem the_position_roles_are_distinct_in_every_row :
    Observable.allRows.all (fun row =>
      row.positions.eraseDups.length == row.setCardinality) = true := by
  native_decide

theorem every_position_role_is_in_range_in_every_row :
    Observable.allRows.all (fun row =>
      row.positions.all (fun index => index < row.setCardinality)) = true := by
  native_decide

theorem every_set_fits_the_release_account_ceiling :
    Observable.allRows.all (fun row =>
      row.setCardinality ≤ RelayedMainnetStateV1Abi.maxAccounts) = true := by
  native_decide

/-! ## The observed cluster's `Clock` sysvar

`require_observation_freshness` needs foreign time, and the only honest source
of it is the foreign `Clock`, decoded here under the same rules as any other
observed account.  `MAINNET_STATE_RELAY.md` §10.6 records why this cannot be an
append-time check: filling only moves bytes the signer committed to.
-/

/-- `SysvarC1ock11111111111111111111111111111111`, as read on the observed
cluster.  The founding-time account set already pins whatever key sits at the
clock position; pinning the sysvar address here as well stops a founder from
nominating some other forty-byte account as the source of foreign time. -/
def clockSysvarKey : List UInt8 := [
  0x06, 0xa7, 0xd5, 0x17, 0x18, 0xc7, 0x74, 0xc9, 0x28, 0x56, 0x63, 0x98, 0x69, 0x1d, 0x5e, 0xb6,
  0x8b, 0x5e, 0xb8, 0xa3, 0x9b, 0x4b, 0x6d, 0x5c, 0x73, 0x55, 0x5b, 0x21, 0x00, 0x00, 0x00, 0x00]

/-- `Sysvar1111111111111111111111111111111111111`, the owner every sysvar
reports. -/
def sysvarOwner : List UInt8 := [
  0x06, 0xa7, 0xd5, 0x17, 0x18, 0x75, 0xf7, 0x29, 0xc7, 0x3d, 0x93, 0x40, 0x8f, 0x21, 0x61, 0x20,
  0x06, 0x7e, 0xd8, 0x8c, 0x76, 0xe0, 0x8c, 0x28, 0x7f, 0xc1, 0x94, 0x60, 0x00, 0x00, 0x00, 0x00]

theorem the_clock_is_not_its_own_owner : clockSysvarKey ≠ sysvarOwner := by
  native_decide

theorem the_pinned_sysvar_addresses_are_addresses :
    clockSysvarKey.length = 32 ∧ sysvarOwner.length = 32 := by
  native_decide

/-- `Clock.slot`. -/
def clockSlotOffset : Nat := 0
/-- `Clock.unix_timestamp`, after `slot`, `epoch_start_timestamp`, `epoch` and
`leader_schedule_epoch`. -/
def clockUnixTimestampOffset : Nat := 32
/-- Both fields are `u64`/`i64`. -/
def clockFieldBytes : Nat := 8

theorem the_clock_reads_lie_inside_the_sysvar :
    clockSlotOffset + clockFieldBytes ≤ RelayedMainnetStateV1Abi.clockSysvarBytes
      ∧ clockUnixTimestampOffset + clockFieldBytes
          ≤ RelayedMainnetStateV1Abi.clockSysvarBytes := by
  native_decide

theorem the_clock_is_carried_whole :
    RelayedMainnetStateV1Abi.clockSysvarBytes = 40 := by
  native_decide

/-! ## The DBC `VirtualPool` grammar

*chain-derived.*  The account type is `#[account(zero_copy)] VirtualPool {
pool_state: PoolState }`, allocated `space = 8 + VirtualPool::INIT_SPACE`, so the
on-chain data length is 424 rather than the 416 of the inner `PoolState` body.
-/

/-- `sha256("account:VirtualPool")[..8]`, agreeing with the deployed on-chain IDL
and a live mainnet pool account. -/
def virtualPoolDiscriminator : List UInt8 :=
  [0xd5, 0xe0, 0x05, 0xd1, 0x62, 0x45, 0x77, 0x5c]

/-- `sha256("account:TransferHookPool")[..8]`.  After the 0.2.0 upgrade this
second discriminator shares the identical 424-byte body — the dual of the
dossier's §1.3 warning.  It is named here, and deliberately **not** admitted, so
that a transfer-hook pool refuses on the discriminator rather than decoding
silently under rules minted for a different account type. -/
def transferHookPoolDiscriminator : List UInt8 :=
  [0xed, 0xdb, 0xb8, 0x17, 0x2a, 0xbd, 0xa9, 0x23]

theorem the_two_pool_discriminators_are_distinguishable :
    virtualPoolDiscriminator ≠ transferHookPoolDiscriminator := by
  native_decide

/-- The admitted data-length set.  The program contains no `realloc`, so unlike
pump.fun's `{49, 81, 115}` this is a singleton, and any other observed length is
a different account. -/
def admittedDataLengths : List Nat := [424]

def discriminatorOffset : Nat := 0
def discriminatorBytes : Nat := 8
/-- `is_migrated: u8`. -/
def isMigratedOffset : Nat := 305
/-- `migration_progress: u8`. -/
def migrationProgressOffset : Nat := 308
/-- `finish_curve_timestamp: u64`. -/
def finishCurveTimestampOffset : Nat := 344
def finishCurveTimestampBytes : Nat := 8

/-- The pinned inline width for the venue position.  §10.1 notes a release *may*
pin the 352-byte graduation prefix and let the remainder ride in the tail digest;
v1 carries the account whole, because the whole account still fits the release's
inline ceiling and a prefix pin is a second thing to get wrong. -/
def venueInlineBytes : Nat := 424

theorem every_read_span_lies_inside_every_admitted_length :
    admittedDataLengths.all (fun length =>
      discriminatorOffset + discriminatorBytes ≤ length
        && isMigratedOffset + 1 ≤ length
        && migrationProgressOffset + 1 ≤ length
        && finishCurveTimestampOffset + finishCurveTimestampBytes ≤ length) = true := by
  native_decide

theorem the_venue_position_is_carried_whole :
    admittedDataLengths.all (fun length => venueInlineBytes == length) = true := by
  native_decide

theorem the_venue_body_fits_the_release_inline_ceiling :
    venueInlineBytes ≤ RelayedMainnetStateV1Abi.maxInlineBytes := by
  native_decide

theorem the_graduation_fields_are_prefix_contiguous :
    finishCurveTimestampOffset + finishCurveTimestampBytes = 352 := by
  native_decide

/-! ## `MigrationProgress`, and the graduation proposition -/

/-- The explicit four-state graduation enum, with the transition flows documented
in a source comment.  *verified-from-source-code.* -/
inductive MigrationProgress where
  | preBondingCurve
  | postBondingCurve
  | lockedVesting
  | createdPool
  deriving DecidableEq, Repr

namespace MigrationProgress

def byte : MigrationProgress → Nat
  | .preBondingCurve => 0
  | .postBondingCurve => 1
  | .lockedVesting => 2
  | .createdPool => 3

/-- Hostile decode of the observed byte.  A fifth value is not a state. -/
def ofByte (value : Nat) : Option MigrationProgress :=
  if value = 0 then some .preBondingCurve
  else if value = 1 then some .postBondingCurve
  else if value = 2 then some .lockedVesting
  else if value = 3 then some .createdPool
  else none

theorem byte_round_trips (state : MigrationProgress) : ofByte state.byte = some state := by
  cases state <;> rfl

theorem an_unenumerated_byte_refuses (value : Nat) (h : 4 ≤ value) : ofByte value = none := by
  have h0 : value ≠ 0 := by omega
  have h1 : value ≠ 1 := by omega
  have h2 : value ≠ 2 := by omega
  have h3 : value ≠ 3 := by omega
  simp [ofByte, h0, h1, h2, h3]

/-- Terminal for a `WindowKind.Terminal` graduation proposition.

Only `CreatedPool` is terminal, and the flow is **not monotone per step**:
without locked vesting it jumps `0 → 2 → 3`.  An adapter that treated
`migration_progress` as a counter to compare across observations would be wrong,
which is why terminality below is equality with one state and never an ordering. -/
def terminal : MigrationProgress → Bool
  | .createdPool => true
  | _ => false

theorem terminality_is_equality_not_an_ordering (state : MigrationProgress) :
    state.terminal = decide (state = .createdPool) := by
  cases state <;> rfl

end MigrationProgress

/-- The two coherence rules the observed body must satisfy before its
`migration_progress` byte means anything.  *chain-derived*, from §10.1:
`is_migrated` is written only at `CreatedPool`, and `finish_curve_timestamp == 0`
is the pre-completion sentinel.

The second rule is deliberately one-directional.  "The curve finished" is a
strictly weaker fact than "the pool migrated", and a rule claiming the converse
would be an inference this repository has not verified from source. -/
def coherent (state : MigrationProgress) (isMigrated finishCurveTimestamp : Nat) : Bool :=
  (isMigrated == 1) == state.terminal
    && (isMigrated == 0 || isMigrated == 1)
    && (!state.terminal || finishCurveTimestamp != 0)

/-- The graduation observable, exactly as the on-chain adapter computes it.

`atoms` is the `MigrationProgress` discriminant itself at `rawExponent = 0`.  The
table does **not** decide which outcome a discriminant selects: the Product's own
`ResultDomainV2` cuts do, which is what keeps one venue's rules reusable across
Products that carve the same observable differently.

`none` is a refusal, and there are exactly three sources of one:

* an unenumerated `migration_progress` byte,
* an incoherent body — a pool claiming `CreatedPool` while `is_migrated` is zero,
  or claiming migration with no `finish_curve_timestamp`,
* a **pre-terminal** state.

The third is the load-bearing one and it is not a defect.  A terminal-window
graduation proposition can only ever be *proved* by graduation; "it did not
graduate" is proved by the deadline passing, which is the funded permissionless
failure walk of `MAINNET_STATE_RELAY.md` §4.8 and lands on the Product's own
pre-disclosed failure selector.  A pre-terminal observation is therefore not a
negative answer, it is no answer, and the honest response is to refuse rather
than to resolve a market early on a state that is still moving. -/
def graduationAtoms (progressByte isMigrated finishCurveTimestamp : Nat) : Option Int :=
  match MigrationProgress.ofByte progressByte with
  | none => none
  | some state =>
      if coherent state isMigrated finishCurveTimestamp && state.terminal then
        some (Int.ofNat state.byte)
      else
        none

/-- Over the whole byte range, and with the companions a graduated pool would
carry, exactly one `migration_progress` value resolves. -/
theorem only_the_terminal_state_resolves :
    (List.range 256).all
        (fun value => (graduationAtoms value 1 1_756_000_500).isSome == (value == 3)) = true := by
  native_decide

/-- A pre-terminal state refuses; it does not quietly become a zero atom. -/
theorem a_pre_terminal_state_is_a_refusal_not_a_zero :
    graduationAtoms 0 0 0 = none
      ∧ graduationAtoms 1 0 1_756_000_500 = none
      ∧ graduationAtoms 2 0 1_756_000_500 = none := by
  native_decide

/-- A body that claims migration while `is_migrated` is zero, or claims it with
no finish timestamp, is a signed statement no resolution accepts. -/
theorem an_incoherent_body_refuses :
    graduationAtoms 3 0 1_756_000_500 = none
      ∧ graduationAtoms 3 1 0 = none
      ∧ graduationAtoms 0 1 0 = none
      ∧ graduationAtoms 3 2 1_756_000_500 = none := by
  native_decide

/-- The terminal state carries its own discriminant into the Product's cuts. -/
theorem the_terminal_state_carries_its_discriminant :
    graduationAtoms 3 1 1_756_000_500 = some 3 := by
  native_decide

/-! ## The acceptance table, emitted as an oracle

Four rows, one per state, each with the companions that state would coherently
carry.  The Rust adapter is tested against exactly these bytes, so a divergence
between the two implementations is a test failure rather than a silent
disagreement about what a graduation is.
-/

/-- `(migration_progress, is_migrated, finish_curve_timestamp, accepted, atoms)`. -/
def acceptanceTable : List (Nat × Nat × Nat × Bool × Int) :=
  [ (0, 0, 0, false, 0)
  , (1, 0, 1_756_000_500, false, 0)
  , (2, 0, 1_756_000_500, false, 0)
  , (3, 1, 1_756_000_500, true, 3)
  , (4, 0, 0, false, 0)
  , (255, 1, 1_756_000_500, false, 0)
  , (3, 0, 1_756_000_500, false, 0)
  , (3, 1, 0, false, 0)
  , (0, 1, 0, false, 0)
  ]

theorem the_acceptance_table_agrees_with_the_proposition :
    acceptanceTable.all (fun row =>
      match row with
      | (progress, isMigrated, finish, accepted, atoms) =>
          match graduationAtoms progress isMigrated finish with
          | none => accepted == false
          | some produced => accepted == true && produced == atoms) = true := by
  native_decide

/-! ## The SPL Token-2022 `Mint` grammar

*verified-from-source* against `spl-token-interface 3.0.0` `src/state.rs`.
`impl Pack for Mint` is `LEN = 82` over `array_refs![src, 36, 8, 1, 1, 36]`:
a 36-byte `COption<Pubkey>` mint authority, a `u64` supply, a `u8` decimals, a
one-byte `is_initialized` admitting only `[0]` or `[1]`, and a second 36-byte
`COption<Pubkey>` freeze authority.  `unpack_coption_key` admits exactly two
four-byte tags, `[0,0,0,0]` and `[1,0,0,0]`, and refuses every other.

SPL Token carries **no discriminator**.  What plays the discriminator's role
here is the admitted length together with the two `COption` tags: `Account` is
165 bytes and `Multisig` is 355 (`state.rs:132,218`), so no other account this
program owns can present as 82, and an 82-byte body whose tags are not one of
the two admitted words is not a `Mint` that this program wrote.
-/

/-- The admitted data-length set: the BASE mint only.

A Token-2022 mint carrying TLV extensions is longer and puts an `AccountType`
byte at `BASE_ACCOUNT_LENGTH - Mint::LEN = 83`.  Admitting only 82 is the
narrow, honest v1: an extended mint refuses on its length rather than decoding
under rules minted for a base one.  Lifting plan: admit the extended shape once
the `AccountType` byte and the TLV walk are themselves verified from source. -/
def mintAdmittedDataLengths : List Nat := [82]

/-- `mint_authority: COption<Pubkey>` — the four-byte tag. -/
def mintAuthorityTagOffset : Nat := 0
/-- Every `COption<Pubkey>` tag in this layout is four bytes. -/
def coptionTagBytes : Nat := 4
/-- `supply: u64`. -/
def mintSupplyOffset : Nat := 36
/-- `decimals: u8`. -/
def mintDecimalsOffset : Nat := 44
/-- `is_initialized: bool`, admitting only `[0]` or `[1]`. -/
def mintIsInitializedOffset : Nat := 45
/-- `freeze_authority: COption<Pubkey>` — the four-byte tag. -/
def mintFreezeAuthorityTagOffset : Nat := 46
/-- The pinned inline width for the mint position: the account whole. -/
def mintInlineBytes : Nat := 82

/-- `COption::None`, little-endian. -/
def coptionNoneTag : Nat := 0
/-- `COption::Some`, little-endian. -/
def coptionSomeTag : Nat := 1

theorem every_mint_read_span_lies_inside_every_admitted_length :
    mintAdmittedDataLengths.all (fun length =>
      mintAuthorityTagOffset + coptionTagBytes ≤ length
        && mintSupplyOffset + 8 ≤ length
        && mintDecimalsOffset + 1 ≤ length
        && mintIsInitializedOffset + 1 ≤ length
        && mintFreezeAuthorityTagOffset + coptionTagBytes ≤ length) = true := by
  native_decide

theorem the_mint_position_is_carried_whole :
    mintAdmittedDataLengths.all (fun length => mintInlineBytes == length) = true := by
  native_decide

theorem the_mint_body_fits_the_release_inline_ceiling :
    mintInlineBytes ≤ RelayedMainnetStateV1Abi.maxInlineBytes := by
  native_decide

/-- The two `COption` fields do not overlap each other or the scalars between
them, and the whole 82 bytes is exactly `36 + 8 + 1 + 1 + 36`. -/
theorem the_mint_fields_tile_the_account :
    mintAuthorityTagOffset + 36 = mintSupplyOffset
      ∧ mintSupplyOffset + 8 = mintDecimalsOffset
      ∧ mintDecimalsOffset + 1 = mintIsInitializedOffset
      ∧ mintIsInitializedOffset + 1 = mintFreezeAuthorityTagOffset
      ∧ mintFreezeAuthorityTagOffset + 36 = mintInlineBytes := by
  native_decide

/-- A `Mint` cannot be confused with the other two accounts this program owns:
`Account::LEN = 165` and `Multisig::LEN = 355`. -/
theorem a_mint_length_is_not_a_token_account_or_multisig_length :
    mintAdmittedDataLengths.all (fun length => length != 165 && length != 355) = true := by
  native_decide

/-! ## `MintAuthorityState`, and the renunciation proposition -/

/-- Whether this mint can still mint.  Two states, and the transition between
them runs one way only: `processor.rs:722` refuses `SetAuthority` for
`MintTokens` when the current authority is `None`. -/
inductive MintAuthorityState where
  | held
  | renounced
  deriving DecidableEq, Repr

namespace MintAuthorityState

def byte : MintAuthorityState → Nat
  | .held => 0
  | .renounced => 1

/-- Hostile decode of the observed four-byte `COption` tag.  `unpack_coption_key`
admits exactly `[0,0,0,0]` and `[1,0,0,0]`; every other word is not a tag this
program ever wrote, and is a refusal rather than a third state. -/
def ofAuthorityTag (tag : Nat) : Option MintAuthorityState :=
  if tag = coptionNoneTag then some .renounced
  else if tag = coptionSomeTag then some .held
  else none

theorem the_none_tag_is_the_renounced_state :
    ofAuthorityTag coptionNoneTag = some .renounced := by native_decide

theorem the_some_tag_is_the_held_state :
    ofAuthorityTag coptionSomeTag = some .held := by native_decide

theorem an_unadmitted_tag_refuses (tag : Nat) (h : 2 ≤ tag) : ofAuthorityTag tag = none := by
  have h0 : tag ≠ coptionNoneTag := by simp [coptionNoneTag]; omega
  have h1 : tag ≠ coptionSomeTag := by simp [coptionSomeTag]; omega
  simp [ofAuthorityTag, h0, h1]

/-- Terminal for a `WindowKind.Terminal` renunciation proposition.

`renounced` is terminal and the observed program itself enforces it: once
`mint_authority` is `None`, `SetAuthority(MintTokens)` returns `FixedSupply`
without writing.  Terminality here is equality with one state, exactly as it is
for row 0, and for the same reason: it must never be read as an ordering. -/
def terminal : MintAuthorityState → Bool
  | .renounced => true
  | .held => false

theorem terminality_is_equality_not_an_ordering (state : MintAuthorityState) :
    state.terminal = decide (state = .renounced) := by
  cases state <;> rfl

end MintAuthorityState

/-- The coherence rules an observed `Mint` body must satisfy before its
authority tag means anything.  *verified-from-source*, from `impl Pack for Mint`:

* `is_initialized` admits only `0` or `1` — `unpack_from_slice` returns
  `InvalidAccountData` for anything else;
* an **uninitialized** mint is not a mint that renounced anything.  Its zeroed
  authority tag would read as `None` and therefore as a graduation of the
  proposition, which is the exact silent-decode this rule exists to stop;
* the freeze-authority tag must itself be one of the two admitted words, because
  a body that is not a well-formed `Mint` in its second `COption` is not a
  well-formed `Mint` in its first either.  With no discriminator to lean on,
  this is what stops an 82-byte foreign body decoding under these rules. -/
def mintCoherent (isInitialized freezeAuthorityTag : Nat) : Bool :=
  isInitialized == 1
    && (freezeAuthorityTag == coptionNoneTag || freezeAuthorityTag == coptionSomeTag)

/-- The renunciation observable, exactly as the on-chain adapter computes it.

`atoms` is the `MintAuthorityState` discriminant at `rawExponent = 0`.  As with
row 0, the table does not decide which outcome a discriminant selects; the
Product's `ResultDomainV2` cuts do.

`none` is a refusal, and there are exactly three sources of one:

* an unadmitted `COption` tag on the mint authority,
* an incoherent body — uninitialized, or a freeze-authority tag that is not a
  tag,
* a **held** authority, which is a pre-terminal state.

The third is the load-bearing one and it is not a defect.  A terminal-window
renunciation proposition can only ever be *proved* by renunciation; "it did not
renounce" is proved by the deadline passing, on the same funded permissionless
failure walk row 0 uses.  A held authority is not a negative answer, it is no
answer. -/
def mintAuthorityAtoms (authorityTag isInitialized freezeAuthorityTag : Nat) : Option Int :=
  match MintAuthorityState.ofAuthorityTag authorityTag with
  | none => none
  | some state =>
      if mintCoherent isInitialized freezeAuthorityTag && state.terminal then
        some (Int.ofNat state.byte)
      else
        none

/-- Over the whole admitted tag range and beyond it, exactly one tag resolves. -/
theorem only_the_renounced_state_resolves :
    (List.range 256).all
        (fun tag => (mintAuthorityAtoms tag 1 coptionSomeTag).isSome == (tag == 0)) = true := by
  native_decide

/-- A held authority refuses; it does not quietly become a zero atom. -/
theorem a_held_authority_is_a_refusal_not_a_zero :
    mintAuthorityAtoms coptionSomeTag 1 coptionSomeTag = none
      ∧ mintAuthorityAtoms coptionSomeTag 1 coptionNoneTag = none := by
  native_decide

/-- AN UNINITIALIZED MINT IS ALL ZEROES, and all zeroes reads as `None`.  If
coherence did not refuse it, a freshly allocated 82-byte account would prove the
proposition the instant it existed.  This is the row's sharpest refusal. -/
theorem a_zeroed_account_does_not_prove_a_renunciation :
    mintAuthorityAtoms coptionNoneTag 0 coptionNoneTag = none := by
  native_decide

/-- A body whose freeze-authority tag is not a tag is not a `Mint`. -/
theorem an_incoherent_mint_body_refuses :
    mintAuthorityAtoms coptionNoneTag 1 2 = none
      ∧ mintAuthorityAtoms coptionNoneTag 2 coptionSomeTag = none
      ∧ mintAuthorityAtoms 2 1 coptionSomeTag = none := by
  native_decide

/-- The terminal state carries its own discriminant into the Product's cuts. -/
theorem the_renounced_state_carries_its_discriminant :
    mintAuthorityAtoms coptionNoneTag 1 coptionSomeTag = some 1 := by
  native_decide

/-- `(mint_authority_tag, is_initialized, freeze_authority_tag, accepted, atoms)`. -/
def mintAcceptanceTable : List (Nat × Nat × Nat × Bool × Int) :=
  [ (0, 1, 1, true, 1)
  , (0, 1, 0, true, 1)
  , (1, 1, 1, false, 0)
  , (1, 1, 0, false, 0)
  , (0, 0, 0, false, 0)
  , (0, 2, 1, false, 0)
  , (0, 1, 2, false, 0)
  , (2, 1, 1, false, 0)
  , (255, 1, 1, false, 0)
  ]

theorem the_mint_acceptance_table_agrees_with_the_proposition :
    mintAcceptanceTable.all (fun row =>
      match row with
      | (authorityTag, isInitialized, freezeTag, accepted, atoms) =>
          match mintAuthorityAtoms authorityTag isInitialized freezeTag with
          | none => accepted == false
          | some produced => accepted == true && produced == atoms) = true := by
  native_decide

/-- The two rows produce atoms that are not the same number, so a Product
carving one observable cannot silently be resolved by the other.  Row 0's
terminal atom is 3 and row 1's is 1. -/
theorem the_two_rows_terminal_atoms_differ :
    graduationAtoms 3 1 1_756_000_500 ≠ mintAuthorityAtoms 0 1 1 := by
  native_decide

/-! ## Native venues: the Feature program and the sysvars

Two rows whose state is owned by a program the validator implements.  Their
grammars are *chain-derived* from `solana-feature-gate-interface` (a `Feature`
is `bincode(Option<u64>)`, nine bytes: a tag byte then `activated_at`) and
`solana-epoch-schedule` (`bincode(EpochSchedule)`, thirty-three bytes:
`slots_per_epoch u64`, `leader_schedule_slot_offset u64`, `warmup bool`,
`first_normal_epoch u64`, `first_normal_slot u64`).  Both are stable ABI of the
runtime rather than of any deployable program, which is the property that
makes a `native` venue kind honest.
-/

/-- `Feature111111111111111111111111111111111111`, the owner every feature-gate
account reports. -/
def featureProgramId : List UInt8 := [
  0x03, 0xc0, 0xa0, 0xcd, 0xcb, 0x06, 0xd2, 0xda, 0xef, 0xae, 0x82, 0xd1, 0x6f, 0xee, 0x7a, 0xcf,
  0x61, 0xec, 0x73, 0x7b, 0x23, 0x48, 0x1b, 0x21, 0x94, 0x6a, 0x76, 0x70, 0x00, 0x00, 0x00, 0x00]

/-- `SysvarEpochSchedu1e111111111111111111111111`, as read on the observed
cluster; owned by `sysvarOwner` like the clock. -/
def epochScheduleSysvarKey : List UInt8 := [
  0x06, 0xa7, 0xd5, 0x17, 0x18, 0xdc, 0x3f, 0xee, 0x02, 0xd3, 0xe4, 0x7f, 0x01, 0x00, 0xf8, 0xb0,
  0x54, 0xf7, 0x94, 0x2e, 0x60, 0x59, 0x1e, 0x3f, 0x50, 0x87, 0x19, 0xa8, 0x05, 0x00, 0x00, 0x00]

theorem the_native_addresses_are_addresses_and_distinct :
    featureProgramId.length = 32 ∧ epochScheduleSysvarKey.length = 32 ∧
    featureProgramId ≠ sysvarOwner ∧ epochScheduleSysvarKey ≠ clockSysvarKey ∧
    epochScheduleSysvarKey ≠ sysvarOwner := by
  native_decide

/-- `bincode(Option<u64>)`: the tag byte then the slot. -/
def featureTagOffset : Nat := 0
def featureActivatedAtOffset : Nat := 1
def featureInlineBytes : Nat := 9
def featureAdmittedDataLengths : List Nat := [9]
def featureNoneTag : Nat := 0
def featureSomeTag : Nat := 1

/-- The atom a not-yet-activated feature yields: above every slot a
`u64` can name, so a cut at `S + 1` puts it in the "not activated by `S`"
cell for every `S`.  Nine bytes cannot say WHEN it was observed not to be
activated; the market's own window (which admits an attested mainnet
`unix_timestamp` only inside `[start, end]`) is what makes "not activated
by `S`" a proposition about a period after `S`.  The founder sets the window's
start at or after mainnet's calendar for `S`; that residual is the founder's,
and `BUILD_PRODUCT-SHAPES.md` §3 records it. -/
def featureNotActivatedSentinel : Nat := 2 ^ 64 - 1

theorem the_sentinel_is_above_every_slot (slot : Nat) (h : slot < 2 ^ 64 - 1) :
    slot < featureNotActivatedSentinel := h

/-- The feature-gate observable.  `none` is a refusal: a tag that is neither
`0` nor `1` is not a `Feature` this program wrote, and a `None` with a
nonzero payload is not canonical bincode. -/
def featureActivationAtoms (tag activatedAt : Nat) : Option Int :=
  if tag = featureSomeTag then some (Int.ofNat activatedAt)
  else if tag = featureNoneTag then
    if activatedAt = 0 then some (Int.ofNat featureNotActivatedSentinel) else none
  else none

theorem an_activated_feature_carries_its_slot (slot : Nat) :
    featureActivationAtoms featureSomeTag slot = some (Int.ofNat slot) := by
  simp [featureActivationAtoms, featureSomeTag]

theorem a_dormant_feature_carries_the_sentinel :
    featureActivationAtoms featureNoneTag 0 = some (Int.ofNat featureNotActivatedSentinel) := by
  native_decide

theorem a_noncanonical_none_refuses : featureActivationAtoms featureNoneTag 7 = none := by
  native_decide

theorem an_unenumerated_tag_refuses (tag : Nat) (h : 2 ≤ tag) (slot : Nat) :
    featureActivationAtoms tag slot = none := by
  have h0 : tag ≠ 0 := by omega
  have h1 : tag ≠ 1 := by omega
  simp [featureActivationAtoms, featureSomeTag, featureNoneTag, h0, h1]

/-- With cuts `[S + 1]` over denominator one, the two cells are exactly
"activated at or before `S`" and "not": a `Some(a)` lands in cell 0 iff
`a ≤ S`, and the sentinel always lands in cell 1.  Stated over the atoms so the
Rust interpreter's test can walk the same table. -/
def activatedBy (atoms : Int) (deadlineSlot : Nat) : Bool := atoms ≤ Int.ofNat deadlineSlot

theorem the_cut_separates_the_branches (a deadline : Nat) :
    activatedBy (Int.ofNat a) deadline = decide (a ≤ deadline) ∧
    activatedBy (Int.ofNat featureNotActivatedSentinel) deadline = decide (deadline ≥ 2 ^ 64 - 1) := by
  constructor
  · simp [activatedBy]
  · -- `simp` normalises the sentinel to `2 ^ 64 - 1` on the left and leaves the
    -- `Int.ofNat` coercion standing on the right; `omega` is what discharges a
    -- goal that is an inequality across that cast and nothing else.
    simp [activatedBy, featureNotActivatedSentinel]
    omega

/-- `bincode(EpochSchedule)`. -/
def epochScheduleSlotsPerEpochOffset : Nat := 0
def epochScheduleLeaderScheduleSlotOffsetOffset : Nat := 8
def epochScheduleWarmupOffset : Nat := 16
def epochScheduleFirstNormalEpochOffset : Nat := 17
def epochScheduleFirstNormalSlotOffset : Nat := 25
def epochScheduleInlineBytes : Nat := 33
def epochScheduleAdmittedDataLengths : List Nat := [33]

theorem every_epoch_schedule_read_lies_inside_the_sysvar :
    epochScheduleSlotsPerEpochOffset + 8 ≤ epochScheduleInlineBytes ∧
    epochScheduleWarmupOffset + 1 ≤ epochScheduleInlineBytes ∧
    epochScheduleFirstNormalEpochOffset + 8 ≤ epochScheduleInlineBytes ∧
    epochScheduleFirstNormalSlotOffset + 8 ≤ epochScheduleInlineBytes := by native_decide

/-- `Clock.epoch_start_timestamp`, between `slot` and `epoch`. -/
def clockEpochStartTimestampOffset : Nat := 8
/-- `Clock.epoch`. -/
def clockEpochOffset : Nat := 16

theorem the_epoch_reads_lie_inside_the_clock :
    clockEpochStartTimestampOffset + clockFieldBytes ≤ RelayedMainnetStateV1Abi.clockSysvarBytes ∧
    clockEpochOffset + clockFieldBytes ≤ RelayedMainnetStateV1Abi.clockSysvarBytes := by
  native_decide

/-- The first slot of `epoch` under a schedule past its warmup.  An epoch
inside the warmup is refused: mainnet's `first_normal_epoch` is zero and the
arithmetic of a warmup epoch is a different function this row does not carry. -/
def epochStartSlot (slotsPerEpoch firstNormalEpoch firstNormalSlot epoch : Nat) : Option Nat :=
  if epoch < firstNormalEpoch then none
  else some (firstNormalSlot + (epoch - firstNormalEpoch) * slotsPerEpoch)

/-- Mean slot duration since the epoch began, in milliseconds, floored.
`none` refuses a zero `slots_per_epoch`, a warmup epoch, an observation at
the epoch's first slot (no elapsed slots, no mean), and a clock whose
`unix_timestamp` precedes its own `epoch_start_timestamp`. -/
def meanSlotTimeMillis (slotsPerEpoch firstNormalEpoch firstNormalSlot slot epoch
    epochStartTimestamp unixTimestamp : Nat) : Option Int :=
  if slotsPerEpoch = 0 then none
  else match epochStartSlot slotsPerEpoch firstNormalEpoch firstNormalSlot epoch with
    | none => none
    | some start =>
        if slot ≤ start ∨ unixTimestamp < epochStartTimestamp then none
        else some (Int.ofNat ((unixTimestamp - epochStartTimestamp) * 1000 / (slot - start)))

/-- Mainnet's schedule (432,000 slots, no warmup), epoch 800, 100,000 slots
into the epoch, 40,000 seconds elapsed: 400 ms. -/
theorem a_worked_mean : meanSlotTimeMillis 432000 0 0 345700000 800 1_700_000_000 1_700_040_000
    = some 400 := by native_decide

theorem the_epoch_boundary_refuses :
    meanSlotTimeMillis 432000 0 0 345600000 800 1_700_000_000 1_700_040_000 = none := by
  native_decide

theorem a_warmup_epoch_refuses :
    meanSlotTimeMillis 432000 5 100 3000 2 1_700_000_000 1_700_040_000 = none := by
  native_decide

theorem a_clock_before_its_epoch_refuses :
    meanSlotTimeMillis 432000 0 0 345700000 800 1_700_040_000 1_700_000_000 = none := by
  native_decide

theorem a_zero_slot_rate_refuses :
    meanSlotTimeMillis 0 0 0 345700000 800 1_700_000_000 1_700_040_000 = none := by
  native_decide

/-- The mean is exact-floored and nonnegative wherever it is defined, so a
Product cut in milliseconds compares against it without a rounding boundary of
its own. -/
theorem the_mean_is_nonnegative (a b c d e f g : Nat) (v : Int)
    (h : meanSlotTimeMillis a b c d e f g = some v) : 0 ≤ v := by
  unfold meanSlotTimeMillis at h
  split at h
  · exact absurd h (by simp)
  · split at h
    · exact absurd h (by simp)
    · split at h
      · exact absurd h (by simp)
      · simp only [Option.some.injEq] at h
        subst h
        exact Int.natCast_nonneg _

/-- The flagship's parent B cuts, from the design note's example: `[390, 410]`
milliseconds give three cells — faster, on-cadence, slower. -/
def flagshipSlotTimeCutsMillis : List Nat := [390, 410]

theorem the_flagship_cuts_are_strictly_increasing :
    flagshipSlotTimeCutsMillis = [390, 410] ∧ 390 < 410 := by native_decide

/-- The native rows read the roles the interpreter needs and nothing more:
their sets are exactly `[state, clock]`. -/
theorem native_rows_carry_state_and_clock_only :
    Observable.featureGateActivation.positions = [0, 1] ∧
    Observable.clockMeanSlotTimeSinceEpochStart.positions = [0, 1] ∧
    Observable.featureGateActivation.setCardinality = 2 ∧
    Observable.clockMeanSlotTimeSinceEpochStart.setCardinality = 2 := by native_decide

end DClutch.RelayedVenueDecodingRulesV1
