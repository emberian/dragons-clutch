import DClutchSemantics.RationalTerminalHotV3Abi
import DClutchSemantics.TsEmit

/-! Emit the Rational terminal Hot V3 wallet ABI as TypeScript.

The browser used to obtain these coordinates by running regular expressions
over TWO Lean-emitted Rust modules at once — the Claims-child request layout in
`dclutch-rational-representation-v2-request-contract` and the Hot
specialization in `dclutch-rational-representation-v2-contract`. Both are
printed from the schemas imported here, so the scrape was a mirror of a mirror
that also had to re-spell all fifty-three constant NAMES to find them, and it
carried a third author besides: the writer path pinned the 648-byte request
width as a literal of its own, so the one number the two Lean modules exist to
agree about was restated a third time in JavaScript.

Nothing about the layout is stated here. The request and asset offsets are
`TsEmit.offsets` over the physical schema; the Hot coordinates are the
projections `RationalTerminalHotV3Abi` already defines; the two magics and the
action/caller tags are the schema's own data. -/

open DClutch.AbiSchema
open DClutch.TsEmit
open DClutch.RationalRepresentationV2PhysicalAbi
  (RequestField AssetField Action CallerRole requestLayout assetLayout
   requestHeaderBytes assetBytes requestMagic)
open DClutch.RationalTerminalHotV3Abi
  (familyMagic requestBytes fixedAssetCount childMagicOffset childVersionOffset
   childActionOffset childCallerRoleOffset parentContextOffset)

def main : IO Unit :=
  emit (header "EmitRationalTerminalHotV3Ts.lean" "abi:rational-terminal-v3") [
    [ nat "PHYSICAL_ABI_VERSION_V2" DClutch.RationalRepresentationV2PhysicalAbi.version,
      nat "REQUEST_HEADER_BYTES_V2" requestHeaderBytes,
      nat "ASSET_BYTES_V2" assetBytes ]
      ++ offsets RequestField.rustName requestLayout
      ++ offsets AssetField.rustName assetLayout
      ++ [ nat "ACTION_REDEEM_TERMINAL" (Action.tag .redeemTerminal),
           nat "CALLER_ROLE_TRADING" (CallerRole.tag .trading),
           nat "RATIONAL_TERMINAL_HOT_VERSION_V3"
             DClutch.RationalTerminalHotV3Abi.version,
           nat "RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3" requestBytes,
           nat "RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3" fixedAssetCount,
           nat "RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3" childMagicOffset,
           nat "RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3" childVersionOffset,
           nat "RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3" childActionOffset,
           nat "RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3" childCallerRoleOffset,
           nat "RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3" parentContextOffset ],
    [ bytes "REQUEST_MAGIC_V2" requestMagic,
      bytes "RATIONAL_TERMINAL_HOT_MAGIC_V3" familyMagic ]
  ]
