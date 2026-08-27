import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec

/-!
# Capability Program Set V2

V2 selects one exact descriptor schema and one exact descriptor content identity
for every request action.  The schema coordinate is explicit authority: adapters
must authenticate the selected finalized record under that schema before choosing
an implemented decoder.  Raw magic, caller hints, and V1 program-only entries are
not production descriptor authority.
-/

namespace DClutch.CapabilityProgramSetV2

open DClutch.AbiSchema

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x50, 0x53, 0x32]
def schemaVersion : Nat := 2
def artifactProfile : Nat := 2
def canonicalEndian : Nat := 0
def entryBytes : Nat := 72
def maxBytes : Nat := 2336
def schemaReleasePreimage : List UInt8 := [
  0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x63, 0x68, 0x65,
  0x6d, 0x61, 0x2f, 0x63, 0x61, 0x70, 0x61, 0x62, 0x69, 0x6c, 0x69, 0x74,
  0x79, 0x2d, 0x70, 0x72, 0x6f, 0x67, 0x72, 0x61, 0x6d, 0x2d, 0x73, 0x65,
  0x74, 0x2d, 0x76, 0x32
]
def schemaReleaseId : List UInt8 := [
  0x37, 0xdf, 0x09, 0xe7, 0xde, 0xeb, 0xdd, 0x0a, 0xd0, 0xd1, 0x25, 0x13, 0xa7, 0x8d, 0xd4, 0x4c,
  0x97, 0x24, 0x30, 0x37, 0x99, 0xb7, 0x54, 0x4d, 0xc9, 0x1b, 0x3b, 0x6a, 0x2e, 0x6d, 0x62, 0x96
]

inductive HeaderField where
  | magic | schemaVersion | artifactProfile | selectorOffset | selectorWidth
  | selectorEndian | entryCount | reserved
  deriving DecidableEq, Repr

def headerSchema : List (FieldSpec HeaderField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.selectorOffset, .u32⟩,
  ⟨.selectorWidth, .u8⟩,
  ⟨.selectorEndian, .u8⟩,
  ⟨.entryCount, .u16⟩,
  ⟨.reserved, .reserved 12⟩
]

def headerLayout : List (PlacedField HeaderField) := specialize headerSchema
def headerBytes : Nat := schemaWidth headerSchema
def maxEntries : Nat := (maxBytes - headerBytes) / entryBytes

namespace HeaderField

def rustName : HeaderField → String
  | .magic => "CAPABILITY_PROGRAM_SET_MAGIC_OFFSET_V2"
  | .schemaVersion => "CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_OFFSET_V2"
  | .selectorOffset => "CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V2"
  | .selectorWidth => "CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V2"
  | .selectorEndian => "CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V2"
  | .entryCount => "CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2"
  | .reserved => "CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V2"

end HeaderField

inductive EntryField where
  | selector | descriptorSchema | descriptorProgram | reserved
  deriving DecidableEq, Repr

def entrySchema : List (FieldSpec EntryField) := [
  ⟨.selector, .u32⟩,
  ⟨.descriptorSchema, .bytes 32⟩,
  ⟨.descriptorProgram, .bytes 32⟩,
  ⟨.reserved, .reserved 4⟩
]

def entryLayout : List (PlacedField EntryField) := specialize entrySchema

namespace EntryField

def rustName : EntryField → String
  | .selector => "CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2"
  | .descriptorSchema => "CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2"
  | .descriptorProgram => "CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2"
  | .reserved => "CAPABILITY_PROGRAM_SET_ENTRY_RESERVED_OFFSET_V2"

end EntryField

structure SelectorSpec where
  offset : Nat
  width : Nat
  deriving DecidableEq, Repr

structure Entry where
  selector : Nat
  descriptorSchema : List UInt8
  descriptorProgram : List UInt8
  deriving DecidableEq, Repr

structure ProgramSet where
  selector : SelectorSpec
  entries : List Entry
  deriving DecidableEq, Repr

def canonicalWidth (width : Nat) : Bool := width = 1 || width = 2 || width = 4

def identityNonzero (identity : List UInt8) : Bool :=
  identity.length = 32 && identity.any (· != 0)

def selectorsStrict (entries : List Entry) : Bool :=
  (entries.map (·.selector)).Pairwise (· < ·)

def selectorFits (width value : Nat) : Bool := value < 256 ^ width

def valid (set : ProgramSet) : Bool :=
  canonicalWidth set.selector.width &&
    set.entries.length > 0 && set.entries.length ≤ maxEntries &&
    selectorsStrict set.entries &&
    set.entries.all fun entry =>
      selectorFits set.selector.width entry.selector &&
        identityNonzero entry.descriptorSchema &&
        identityNonzero entry.descriptorProgram

def encodeEntry (entry : Entry) : List UInt8 :=
  DClutch.Codec.encodeLE 4 entry.selector ++ entry.descriptorSchema ++
    entry.descriptorProgram ++ List.replicate 4 0

def encode (set : ProgramSet) : List UInt8 :=
  magic ++
    DClutch.Codec.encodeLE 2 schemaVersion ++
    DClutch.Codec.encodeLE 2 artifactProfile ++
    DClutch.Codec.encodeLE 4 set.selector.offset ++
    DClutch.Codec.encodeLE 1 set.selector.width ++
    DClutch.Codec.encodeLE 1 canonicalEndian ++
    DClutch.Codec.encodeLE 2 set.entries.length ++
    List.replicate 12 0 ++
    set.entries.flatMap encodeEntry

def decodeEntry (bytes : List UInt8) : Option Entry := do
  if bytes.length != entryBytes then none else
  let selector := DClutch.Codec.decodeLE (bytes.take 4)
  let descriptorSchema := (bytes.drop 4).take 32
  let descriptorProgram := (bytes.drop 36).take 32
  if !identityNonzero descriptorSchema || !identityNonzero descriptorProgram ||
      !(bytes.drop 68).all (· = 0) then none else
  some { selector, descriptorSchema, descriptorProgram }

def decodeEntries : Nat → List UInt8 → Option (List Entry)
  | 0, bytes => if bytes.isEmpty then some [] else none
  | count + 1, bytes => do
      let entry ← decodeEntry (bytes.take entryBytes)
      let rest ← decodeEntries count (bytes.drop entryBytes)
      some (entry :: rest)

def decode (bytes : List UInt8) : Option ProgramSet := do
  if bytes.length < headerBytes || bytes.length > maxBytes then none else
  if bytes.take 8 != magic then none else
  if DClutch.Codec.decodeLE ((bytes.drop 8).take 2) != schemaVersion then none else
  if DClutch.Codec.decodeLE ((bytes.drop 10).take 2) != artifactProfile then none else
  let selectorOffset := DClutch.Codec.decodeLE ((bytes.drop 12).take 4)
  let selectorWidth := DClutch.Codec.decodeLE ((bytes.drop 16).take 1)
  let selectorEndian := DClutch.Codec.decodeLE ((bytes.drop 17).take 1)
  let entryCount := DClutch.Codec.decodeLE ((bytes.drop 18).take 2)
  if selectorEndian != canonicalEndian || !(bytes.drop 20 |>.take 12 |>.all (· = 0)) then none else
  if entryCount = 0 || entryCount > maxEntries then none else
  if bytes.length != headerBytes + entryCount * entryBytes then none else
  let entries ← decodeEntries entryCount (bytes.drop headerBytes)
  let set := { selector := { offset := selectorOffset, width := selectorWidth }, entries }
  if valid set then some set else none

def readSelector (spec : SelectorSpec) (request : List UInt8) : Option Nat := do
  if !canonicalWidth spec.width then none else
  if spec.offset + spec.width > request.length then none else
  some (DClutch.Codec.decodeLE ((request.drop spec.offset).take spec.width))

def select (set : ProgramSet) (request : List UInt8) : Option Entry := do
  if !valid set then none else
  let selector ← readSelector set.selector request
  set.entries.find? (·.selector = selector)

def id (byte : UInt8) : List UInt8 := List.replicate 32 byte

def exampleSet : ProgramSet := {
  selector := { offset := 10, width := 1 }
  entries := [
    { selector := 1, descriptorSchema := id 0x41, descriptorProgram := id 0x11 },
    { selector := 3, descriptorSchema := id 0x42, descriptorProgram := id 0x22 },
    { selector := 7, descriptorSchema := id 0x43, descriptorProgram := id 0x33 }
  ]
}

def canonicalExample : List UInt8 := encode exampleSet

def zeroSchemaSet : ProgramSet := { exampleSet with entries := [
  { selector := 1, descriptorSchema := id 0x41, descriptorProgram := id 0x11 },
  { selector := 3, descriptorSchema := List.replicate 32 0, descriptorProgram := id 0x22 },
  { selector := 7, descriptorSchema := id 0x43, descriptorProgram := id 0x33 }
] }

def zeroProgramSet : ProgramSet := { exampleSet with entries := [
  { selector := 1, descriptorSchema := id 0x41, descriptorProgram := id 0x11 },
  { selector := 3, descriptorSchema := id 0x42, descriptorProgram := List.replicate 32 0 },
  { selector := 7, descriptorSchema := id 0x43, descriptorProgram := id 0x33 }
] }

def hostileCorpus : List (List UInt8) := [
  List.set canonicalExample 0 0xff,
  List.set canonicalExample 8 1,
  List.set canonicalExample 10 1,
  List.set canonicalExample 16 3,
  List.set canonicalExample 17 1,
  List.set canonicalExample 18 0,
  List.set canonicalExample 20 1,
  List.set canonicalExample (headerBytes + 68) 1,
  List.set canonicalExample (headerBytes + entryBytes) 1,
  List.set canonicalExample (headerBytes + 2 * entryBytes) 2,
  encode zeroSchemaSet,
  encode zeroProgramSet
]

theorem header_width_is_exact : headerBytes = 32 := by native_decide
theorem entry_width_is_exact : schemaWidth entrySchema = entryBytes := by native_decide
theorem max_entries_is_exact : maxEntries = 32 := by native_decide
theorem schema_identity_widths :
    schemaReleasePreimage.length = 40 ∧ schemaReleaseId.length = 32 := by native_decide
theorem header_fields_disjoint : headerLayout.Pairwise Before :=
  specializeFrom_pairwise 0 headerSchema
theorem entry_fields_disjoint : entryLayout.Pairwise Before :=
  specializeFrom_pairwise 0 entrySchema
theorem canonical_accepts : decode canonicalExample = some exampleSet := by native_decide
theorem hostile_refuse : hostileCorpus.all (decode · |>.isNone) := by native_decide
theorem selector_example : select exampleSet (List.replicate 10 0 ++ [3]) = exampleSet.entries[1]? := by
  native_decide
theorem selector_short_refuses : select exampleSet (List.replicate 10 0) = none := by native_decide
theorem selector_absent_refuses : select exampleSet (List.replicate 10 0 ++ [2]) = none := by
  native_decide

end DClutch.CapabilityProgramSetV2
