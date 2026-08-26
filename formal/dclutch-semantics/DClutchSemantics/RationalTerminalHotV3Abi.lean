import DClutchSemantics.RationalRepresentationV2PhysicalAbi

/-!
# Rational terminal Hot V3 family ABI

This is the wallet-facing, ephemeral Hot request for one terminal rational
redemption.  It deliberately reuses the exact 648-byte terminal child shape,
but its parent-context coordinate is canonically zero.  The authenticated Hot
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
def requestBytes : Nat := requestHeaderBytes + fixedAssetCount * assetBytes

def childMagicOffset : Nat := RequestField.offset .magic
def childVersionOffset : Nat := RequestField.offset .version
def childActionOffset : Nat := RequestField.offset .action
def childCallerRoleOffset : Nat := RequestField.offset .callerRole
def parentContextOffset : Nat := RequestField.offset .parentContext

/-!
The Hot RequestProfile consumes the terminal family as one fixed 648-byte
request.  These aliases remain projections of the sole physical ABI owner;
they do not restate numeric coordinates.  Asset coordinates are made absolute
because the RequestProfile instruction space is the complete family request.
-/
def requestOffset (field : RequestField) : Nat := RequestField.offset field
def assetStartOffset : Nat := requestHeaderBytes
def absoluteAssetOffset (field : AssetField) : Nat :=
  assetStartOffset + AssetField.offset field

theorem request_bytes_exact : requestBytes = 648 := by decide
theorem parent_context_is_digest_coordinate : parentContextOffset = 144 := by decide
theorem specialization_preserves_width :
    requestBytes = requestHeaderBytes + assetBytes := by decide
theorem request_profile_coordinates_exact :
    requestOffset .releaseSet = 16 ∧
    requestOffset .expectedRepresentationRevision = 400 ∧
    requestOffset .outcomeCount = 472 ∧
    absoluteAssetOffset .shardMint = 488 ∧
    absoluteAssetOffset .expectedStructuredShards = 640 := by decide

end DClutch.RationalTerminalHotV3Abi
