import DClutchSemantics.RationalRepresentationV2PhysicalAbi

/-!
# Rational terminal Hot V3 family ABI

This is the wallet-facing, ephemeral Hot request for one terminal rational
redemption.  It deliberately reuses the exact 648-byte terminal child shape,
but its parent-context coordinate is canonically zero.  Physical ABI version
three made the request header action-conditional, so this family is the
TERMINAL class of that one schema: 444 header bytes and one 64-byte asset row,
508 in all, down from 648.  The authenticated Hot
adapter hashes these bytes, writes that digest into the child coordinate, and
changes only the magic/version pair before invoking Claims.  Keeping the two
messages distinct avoids the impossible fixed point in which a child contains
the digest of itself.

No Product, Claims, Token, or Custody fact is persisted by this family ABI.
-/

namespace DClutch.RationalTerminalHotV3Abi

open DClutch.RationalRepresentationV2PhysicalAbi

def familyMagic : List UInt8 :=
  [0x44, 0x43, 0x52, 0x52, 0x48, 0x54, 0x56, 0x33] -- `DCRRHTV3`

def version : Nat := 3
def fixedAssetCount : Nat := 1
def requestBytes : Nat := classHeaderBytes .terminal + fixedAssetCount * assetBytes
def requestSchemaPreimage : List UInt8 :=
  "dclutch/schema/rational-terminal-hot-request-v3".toUTF8.toList
def requestSchemaId : List UInt8 := [
  0x8b, 0xab, 0xcd, 0x90, 0x65, 0xc6, 0x52, 0x25,
  0x32, 0xe2, 0x6c, 0x60, 0x63, 0x61, 0x56, 0x72,
  0x4f, 0x7f, 0x6c, 0xfc, 0xfb, 0xa2, 0x60, 0x82,
  0x75, 0x86, 0x27, 0xc4, 0x9c, 0xdc, 0x80, 0x3b
]

/-- A coordinate in the TERMINAL class layout.  A field this class does not
carry has no coordinate here, and the `getD` fallback would hand it `magic`'s
zero.  Nothing is allowed to reach that fallback: `emittedFields` is the only
list a generator may iterate, and `emitted_fields_are_carried` proves every
member of it is `some`. -/
def requestOffset (field : RequestField) : Nat :=
  (RequestField.offsetIn? .terminal field).getD 0

/-- THE fields this family publishes a coordinate for -- the one list, iterated
by both generators, quantified over by both theorems below.  A generator that
spells its own list is how a dropped field keeps a name: the Rust emitter and
the theorem each carried a hand-written copy, they disagreed by exactly one
entry, and `expectedActorPositionRevision` -- which the terminal class stopped
carrying when the header became action-conditional -- was emitted as offset
zero, aliasing `magic`.  Two consumers then wrote a `u64` over the magic. -/
def emittedFields : List RequestField := [
  .magic, .version, .action, .callerRole, .parentContext,
  .reservedHeader, .releaseSet, .market, .graphId, .descriptorId,
  .actor, .receiptMint, .representationAuthority, .tokenProgram,
  .realm, .collateralRecipient,
  .expectedRepresentationRevision, .expectedClaimsMarketRevision,
  .expectedCustodyPositionRevision, .expectedCustodyReplayRevision,
  .generation, .quantity, .denominator, .expectedReceiptSupply,
  .outcomeCount, .selectedOutcome, .reservedTail
]

/-- The family constant name for a request field, derived from the physical
ABI's own name rather than respelled.  `emitted_names_are_request_names` is
what makes the `drop` sound for every field a generator actually passes. -/
def familyName (field : RequestField) : String :=
  "RATIONAL_TERMINAL_HOT_" ++ (RequestField.rustName field).drop "REQUEST_".length ++ "_V3"
def childMagicOffset : Nat := requestOffset .magic
def childVersionOffset : Nat := requestOffset .version
def childActionOffset : Nat := requestOffset .action
def childCallerRoleOffset : Nat := requestOffset .callerRole
def parentContextOffset : Nat := requestOffset .parentContext

/-!
The Hot RequestProfile consumes the terminal family as one fixed 648-byte
request.  These aliases remain projections of the sole physical ABI owner;
they do not restate numeric coordinates.  Asset coordinates are made absolute
because the RequestProfile instruction space is the complete family request.
-/
def assetStartOffset : Nat := classHeaderBytes .terminal
def absoluteAssetOffset (field : AssetField) : Nat :=
  assetStartOffset + AssetField.offset field

/-- The family constant name for an asset field.  Asset rows are not
action-conditional, so `AssetField.all` is already the exact emitted set. -/
def familyAssetName (field : AssetField) : String :=
  "RATIONAL_TERMINAL_HOT_" ++ AssetField.rustName field ++ "_V3"

theorem request_bytes_exact : requestBytes = 508 := by native_decide
theorem parent_context_is_digest_coordinate : parentContextOffset = 144 := by native_decide
theorem specialization_preserves_width :
    requestBytes = classHeaderBytes .terminal + assetBytes := by native_decide

/-- Soundness: every field this family emits a coordinate for is one the
terminal class actually carries, so `requestOffset` never reaches its `getD`
fallback and no coordinate can alias `magic` at zero. -/
theorem emitted_fields_are_carried :
    ∀ field ∈ emittedFields, (RequestField.offsetIn? .terminal field).isSome := by
  native_decide

/-- Completeness, the direction the old hand-written list could not state:
every field the terminal class carries is emitted.  Together with soundness
this pins `emittedFields` to the class exactly, so a field added to or removed
from `classTail .terminal` breaks this file rather than the wire. -/
theorem carried_fields_are_emitted :
    ∀ placed ∈ classLayout .terminal, placed.spec.name ∈ emittedFields := by
  native_decide

/-- The name derivation only drops a prefix every request field really has. -/
theorem emitted_names_are_request_names :
    ∀ field ∈ emittedFields, "REQUEST_".isPrefixOf (RequestField.rustName field) := by
  native_decide

/-- And the three fields of the vocabulary the terminal class does NOT carry:
the Structured receipt Account, the derived asset count, and the actor-Position
revision, which is `ABSENT_REVISION` for every terminal redemption and so is
computed by the decoder rather than sent. -/
theorem terminal_family_omits_exactly :
    (RequestField.offsetIn? .terminal .receiptAccount).isNone ∧
      (RequestField.offsetIn? .terminal .assetCount).isNone ∧
      (RequestField.offsetIn? .terminal .expectedActorPositionRevision).isNone := by
  native_decide

theorem request_schema_coordinates_are_exact :
    requestSchemaPreimage.length = 47 ∧ requestSchemaId.length = 32 := by native_decide
theorem request_profile_coordinates_exact :
    requestOffset .releaseSet = 16 ∧
    requestOffset .expectedRepresentationRevision = 304 ∧
    requestOffset .outcomeCount = 344 ∧
    requestOffset .realm = 348 ∧
    absoluteAssetOffset .actorShardAccount = 444 ∧
    absoluteAssetOffset .expectedStructuredShards = 500 := by native_decide

end DClutch.RationalTerminalHotV3Abi
