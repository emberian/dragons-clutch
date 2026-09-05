/-!
# Rust backend for the Lean ABI emitters

The Rust twin of `TsEmit`.  Every `Emit*.lean` used to carry its own copy of
these printers -- seventy-five `rustByte`s, forty-odd `emitBytes`es in ten
formatting variants -- and the variants were not choices, they were the drift
of one idea copied by hand.  Under the guards' `rustfmt --edition 2024`
normalisation only two things about a printed array are meaning rather than
whitespace: its visibility, and whether it is wrapped in `#[rustfmt::skip]`
(which pins a sixteen-per-line layout so a diff names the row that moved).
The printers here are named by those two facts.

An emitter that needs a shape not here keeps a local definition; a shape
that a second emitter needs moves here.

This module imports nothing and `Codec` imports it.  That direction is what
makes the printers reachable from every guard: a guard builds exactly the ABI
module it names and then runs the emitter, and every ABI module reaches
`Codec`, so the printers are built wherever an emitter runs -- including the
guards that materialise a clean archive and build one module.  The hexadecimal
rendering of a byte lives here for the same reason, and `Codec.byteHex` is
this one.
-/

namespace DClutch.RustEmit

/-- Lowercase hexadecimal digit. -/
def hexDigit (value : Nat) : Char :=
  match value with
  | 0 => '0' | 1 => '1' | 2 => '2' | 3 => '3'
  | 4 => '4' | 5 => '5' | 6 => '6' | 7 => '7'
  | 8 => '8' | 9 => '9' | 10 => 'a' | 11 => 'b'
  | 12 => 'c' | 13 => 'd' | 14 => 'e' | _ => 'f'

/-- Two lowercase hexadecimal digits. -/
def byteHex (byte : UInt8) : String :=
  let value := byte.toNat
  String.ofList [hexDigit (value / 16), hexDigit (value % 16)]

/-- One byte as a Rust hexadecimal literal. -/
def rustByte (byte : UInt8) : String := s!"0x{byteHex byte}"

/-- `rustByte` under the name the schema-identity emitters gave it. -/
def schemaByte (byte : UInt8) : String := rustByte byte

/-- A whole byte string as one Rust array literal, on one line. -/
def rustBytes (bytes : List UInt8) : String :=
  s!"[{String.intercalate ", " (bytes.map rustByte)}]"

/-- `pub const NAME: [u8; N] = [..];` -- rustfmt decides the line breaks. -/
def emitBytes (name : String) (value : List UInt8) : IO Unit := do
  IO.println s!"pub const {name}: [u8; {value.length}] = ["
  IO.println s!"    {String.intercalate ", " (value.map rustByte)},"
  IO.println "];"

/-- `pub const NAME: [u8; N]` behind `#[rustfmt::skip]`, sixteen bytes to a
line, so a record fixture's diff names the row that moved. -/
def emitBytesSkip (name : String) (value : List UInt8) : IO Unit := do
  IO.println "#[rustfmt::skip]"
  IO.println s!"pub const {name}: [u8; {value.length}] = ["
  for line in List.range ((value.length + 15) / 16) do
    let chunk := (value.drop (line * 16)).take 16
    IO.println s!"    {String.intercalate ", " (chunk.map rustByte)},"
  IO.println "];"

/-- `emitBytesSkip` with the visibility spelled by the caller. -/
def emitBytesRows (visibility name : String) (value : List UInt8) : IO Unit := do
  IO.println "#[rustfmt::skip]"
  IO.println s!"{visibility} const {name}: [u8; {value.length}] = ["
  for line in List.range ((value.length + 15) / 16) do
    let chunk := (value.drop (line * 16)).take 16
    IO.println s!"    {String.intercalate ", " (chunk.map rustByte)},"
  IO.println "];"

/-- Sixteen bytes to a line with the caller's visibility and no `skip`: the
layout is a courtesy to the reader of the raw emission, and rustfmt owns the
committed one. -/
def emitRustBytes (visibility name : String) (bytes : List UInt8) : IO Unit := do
  IO.println s!"{visibility} const {name}: [u8; {bytes.length}] = ["
  for line in List.range ((bytes.length + 15) / 16) do
    let chunk := (bytes.drop (line * 16)).take 16
    IO.println s!"    {String.intercalate ", " (chunk.map rustByte)},"
  IO.println "];"

/-- `pub const NAME: &[u8] = &[..];` -/
def emitSlice (name : String) (value : List UInt8) : IO Unit := do
  IO.println s!"pub const {name}: &[u8] = &["
  IO.println s!"    {String.intercalate ", " (value.map rustByte)},"
  IO.println "];"

/-- `emitSlice` behind `#[rustfmt::skip]`. -/
def emitSliceSkip (name : String) (value : List UInt8) : IO Unit := do
  IO.println "#[rustfmt::skip]"
  IO.println s!"pub const {name}: &[u8] = &["
  IO.println s!"    {String.intercalate ", " (value.map rustByte)},"
  IO.println "];"

/-- A documented scalar constant. -/
def emitConst (name kind value doc : String) : IO Unit := do
  IO.println s!"/// {doc}"
  IO.println s!"pub const {name}: {kind} = {value};"

end DClutch.RustEmit
