import DClutchSemantics.RationalTerminalHotV3Abi
import DClutchSemantics.TsEmit

/-! Emit the Rational terminal Hot V3 wallet ABI as TypeScript.

The browser used to obtain these coordinates by running regular expressions
over TWO Lean-emitted Rust modules at once — the Claims-child request layout in
`dclutch-rational-representation-v2-request-contract` and the Hot
specialization in `dclutch-rational-representation-v2-contract`. Both are
printed from the schemas imported here, so the scrape was a mirror of a mirror
that also had to re-spell all fifty-three constant NAMES to find them, and it
carried a third author besides: the writer path pinned the request width as a
literal of its own, so the one number the two Lean modules exist to agree about
was restated a third time in JavaScript.

Nothing about the layout is stated here. Physical ABI v3 made the request
header ACTION-CONDITIONAL, so there is no longer one request layout to print:
the common prefix is placed identically in all three classes and printed once
under its class-free names, and each class's tail is printed under that class's
prefix. A browser that wants a coordinate must now say which action it is
decoding, which is the point — the v2 names could be read against the wrong
action and produce a plausible number.

The terminal family's own coordinates are printed in full from `emittedFields`,
the same list the Rust emitter iterates, so the wallet builder never has to
pick a class at all: every `RATIONAL_TERMINAL_HOT_*` name is already the
terminal one. -/

open DClutch.AbiSchema
open DClutch.TsEmit
open DClutch.RationalRepresentationV2PhysicalAbi
  (RequestField AssetField RequestClass Action CallerRole commonSchema classLayout
   classHeaderBytes commonPrefixBytes assetLayout assetBytes requestMagic)
open DClutch.RationalTerminalHotV3Abi
  (familyMagic familyName familyAssetName emittedFields requestOffset requestBytes
   fixedAssetCount assetStartOffset absoluteAssetOffset)

/-- A class tail constant, named exactly as the Rust twin names it. -/
def classTailOffsets (kind : RequestClass) : List String :=
  ((classLayout kind).drop commonSchema.length).map fun placed =>
    nat s!"{RequestClass.rustName kind}_{RequestField.rustName placed.spec.name}_V3" placed.offset

def main : IO Unit :=
  emit (header "EmitRationalTerminalHotV3Ts.lean" "abi:rational-terminal-v3") [
    [ nat "PHYSICAL_ABI_VERSION_V3" DClutch.RationalRepresentationV2PhysicalAbi.version,
      nat "REQUEST_COMMON_PREFIX_BYTES_V3" commonPrefixBytes,
      nat "ASSET_BYTES_V3" assetBytes ]
      ++ (RequestClass.all.map fun kind =>
            nat s!"REQUEST_{RequestClass.rustName kind}_HEADER_BYTES_V3" (classHeaderBytes kind)),
    (specialize commonSchema).map fun placed =>
      nat s!"{RequestField.rustName placed.spec.name}_V3" placed.offset,
    (RequestClass.all.map classTailOffsets).flatten,
    assetLayout.map fun placed =>
      nat s!"{AssetField.rustName placed.spec.name}_V3" placed.offset,
    [ nat "ACTION_DENOMINATE" (Action.tag .denominate),
      nat "ACTION_RECONSTITUTE" (Action.tag .reconstitute),
      nat "ACTION_ISSUE_STRUCTURED" (Action.tag .issueStructured),
      nat "ACTION_UNWRAP_STRUCTURED" (Action.tag .unwrapStructured),
      nat "ACTION_REDEEM_TERMINAL" (Action.tag .redeemTerminal),
      nat "CALLER_ROLE_CORE" (CallerRole.tag .core),
      nat "CALLER_ROLE_TRADING" (CallerRole.tag .trading),
      nat "RATIONAL_TERMINAL_HOT_VERSION_V3"
        DClutch.RationalTerminalHotV3Abi.version,
      nat "RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3" requestBytes,
      nat "RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3" fixedAssetCount ]
      ++ (emittedFields.map fun field => nat (familyName field) (requestOffset field))
      ++ [ nat "RATIONAL_TERMINAL_HOT_ASSET_OFFSET_V3" assetStartOffset ]
      ++ (AssetField.all.map fun field =>
            nat (familyAssetName field) (absoluteAssetOffset field)),
    [ bytes "REQUEST_MAGIC_V2" requestMagic,
      bytes "RATIONAL_TERMINAL_HOT_MAGIC_V3" familyMagic ]
  ]
