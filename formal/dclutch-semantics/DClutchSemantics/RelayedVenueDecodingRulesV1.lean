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

## The one venue in v1

Meteora Dynamic Bonding Curve (`dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN`),
graduation state only.  *verified-from-source* against
`MeteoraAg/dynamic-bonding-curve @ 3b540e94`,
`programs/dynamic-bonding-curve/src/state/virtual_pool.rs`, and confirmed against
live mainnet bytes; recorded in `MAINNET_STATE_RELAY.md` §10.1.
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
  deriving DecidableEq, Repr

namespace Observable

/-- The `RelayedAdapterConfigV1.observable_selector` value naming this row. -/
def selector : Observable → Nat
  | .dbcMigrationProgress => 0

/-- The declared base-ten scale of the atom this row produces.  A discrete
graduation state is a count of states, so its scale is zero. -/
def rawExponent : Observable → Int
  | .dbcMigrationProgress => 0

/-- Hostile selection.  An unknown selector has no row and therefore no
interpretation; it must not fall through to row zero. -/
def ofSelector (value : Nat) : Option Observable :=
  if value = 0 then some .dbcMigrationProgress else none

theorem selector_round_trips (row : Observable) : ofSelector row.selector = some row := by
  cases row <;> rfl

theorem an_unknown_selector_refuses (value : Nat) (h : 1 ≤ value) : ofSelector value = none := by
  have nonzero : value ≠ 0 := by omega
  simp [ofSelector, nonzero]

end Observable

/-! ## Roles inside the pinned ordered account set

The set itself is pinned by `account_set_id`, which already binds each
position's key, expected owning program, and inline width.  What the transport
cannot know is what each position *is for*; that is a decoding-rules fact and it
lives here.
-/

/-- Cardinality of the DBC graduation set. -/
def venueSetCardinality : Nat := 4
/-- The venue program's `Program` account, for `DeploymentObservationV1`. -/
def programPosition : Nat := 0
/-- The venue program's `ProgramData` account; its tail digest *is* the ELF digest. -/
def programDataPosition : Nat := 1
/-- The observed venue state account. -/
def venuePosition : Nat := 2
/-- The observed cluster's `Clock` sysvar: the only source of foreign time. -/
def clockPosition : Nat := 3

theorem the_position_roles_are_distinct :
    [programPosition, programDataPosition, venuePosition, clockPosition].eraseDups.length
      = venueSetCardinality := by
  native_decide

theorem every_position_role_is_in_range :
    [programPosition, programDataPosition, venuePosition, clockPosition].all
      (fun index => index < venueSetCardinality) = true := by
  native_decide

theorem the_set_fits_the_release_account_ceiling :
    venueSetCardinality ≤ RelayedMainnetStateV1Abi.maxAccounts := by
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

end DClutch.RelayedVenueDecodingRulesV1
