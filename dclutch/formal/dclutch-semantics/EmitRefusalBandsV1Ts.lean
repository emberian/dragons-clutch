import DClutchSemantics.RefusalBandsV1
import DClutchSemantics.TsEmit

/-! Emit decision 0007's refusal band allocation as TypeScript.

This module retires a text scrape. The browser and the SDK obtained this table
by running a regular expression over `crates/dclutch-refusal-registry`, which
is now itself generated -- so the scrape had become a generated file read to
rebuild what the generator already knew. -/

open DClutch.RefusalBands
open DClutch.TsEmit

def tsHexDigit (value : Nat) : String :=
  ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F"].getD
    value "0"

def tsHexAux : Nat → Nat → String
  | 0, _ => ""
  | fuel + 1, value =>
      if value < 16 then tsHexDigit value
      else tsHexAux fuel (value / 16) ++ tsHexDigit (value % 16)

def tsHex (value : Nat) : String := "0x" ++ tsHexAux 16 value

def tsTier (tier : BandTier) : String :=
  match tier with
  | .program => "program"
  | .testCaller => "test-caller"

def bandRow (band : Band) : String :=
  s!"  \{ label: '{band.label}', package: '{band.package}', base: {tsHex band.base}, tier: '{tsTier band.tier}' },"

def main : IO Unit :=
  emit (header "EmitRefusalBandsV1Ts.lean" "abi:refusal-bands") [
    [ nat "REFUSAL_BAND_SHIFT" bandShift,
      nat "REFUSAL_BAND_SPAN" bandSpan,
      nat "REFUSAL_BAND_COUNT" bands.length,
      nat "REFUSAL_FIRST_PROGRAM_BAND" firstProgramBand,
      nat "REFUSAL_FIRST_TEST_BAND" firstTestBand ],
    [ "export interface RefusalBandV1 {",
      "  readonly label: string;",
      "  readonly package: string;",
      "  readonly base: number;",
      "  readonly tier: 'program' | 'test-caller';",
      "}" ],
    ["export const REFUSAL_BANDS_V1: ReadonlyArray<RefusalBandV1> = ["]
      ++ bands.map bandRow ++ ["];"]
  ]
