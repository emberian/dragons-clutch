import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# `RelayedMainnetStateV1` attestation ABI

Fixed-layout wire for the disclosed proof-of-authority relayer family specified
in `docs/design/MAINNET_STATE_RELAY.md` §4.  The relayer signs *observations* of
mainnet account bytes; it never signs an interpretation.  Every layout fact for
a venue account lives in the separately content-addressed decoding-rules record
and is applied by the on-devnet adapter, so nothing here names a venue.

Five signed or persisted shapes are specialized:

* `observation` — the per-account observation body head (112 bytes) followed by
  exactly `inlineLen` inline bytes.  A fully inline account sets
  `inlineLen = dataLen` and `tailDigest` to the SHA-256 of the empty string;
  there is no variant tag and no special case.
* `attestation` — one signer, one account (`DCLTRMA1`).
* `observationSet` — one signer, the whole ordered set (`DCLTRMS1`).
* `seal` — one signer, sealing a completed set (`DCLTRSS1`), exactly 156 bytes.
* `keySet` — the immutable relayer key set (`DCLTRKS1`), whose content identity
  *is* `ProviderReleaseV1.provider_deployment_release_id`.
* `adapterConfig` — the founding-time adapter pin (`DCLTRAC1`).
* `record` — the persisted `RelayedObservationRecordV1` header (`DCLTROB1`).

One deliberate deviation from the design document is recorded here rather than
discovered later: §4.3 draws `RelayedAdapterConfigV1` as 64 bytes beginning
directly with `account_set_id`.  Every other content-addressed record in this
repository begins with an 8-byte magic and a schema version, and that header is
what stops a hostile or merely mistaken 64-byte raw record of a *different*
family from decoding in the same slot.  The record is therefore 80 bytes with
the house header; the semantic field set is unchanged.
-/

namespace DClutch.RelayedMainnetStateV1Abi

open DClutch
open DClutch.AbiSchema

/-! ## Family, transport and schema release identities -/

/-- `ProviderReleaseV1.provider_family_id` for this family. -/
def familyReleasePreimage : List UInt8 :=
  "dclutch/relayed-mainnet-state-family/v1".toUTF8.toList
def familyReleaseId : List UInt8 := [
  0x09, 0x5d, 0x90, 0xfc, 0xe8, 0xc9, 0xae, 0x83,
  0xf0, 0x8c, 0x4e, 0x37, 0x9f, 0xfd, 0x73, 0xa8,
  0xef, 0x76, 0x8e, 0x30, 0x30, 0xd2, 0xf9, 0x08,
  0x0c, 0x83, 0x5b, 0xcb, 0xb7, 0xe6, 0xf5, 0x0f
]

/-- `ProviderReleaseV1.transport_profile_id` for the family-general record
profile: append per account, seal per signer, then resolve. -/
def recordTransportProfilePreimage : List UInt8 :=
  "dclutch/relayed-transport-observation-record/v1".toUTF8.toList
def recordTransportProfileId : List UInt8 := [
  0xc3, 0xd1, 0x90, 0x54, 0xb5, 0xde, 0x53, 0x30,
  0x72, 0x52, 0xbb, 0xaf, 0xa3, 0x5f, 0x42, 0xd5,
  0x43, 0x3d, 0x41, 0x93, 0x57, 0xd4, 0x9e, 0x59,
  0xce, 0xe2, 0x91, 0x7f, 0x88, 0x55, 0xed, 0x81
]

/-- `ProviderReleaseV1.transport_profile_id` for the one-transaction profile,
admitted only where the packet geometry allows it and only at threshold one. -/
def oneTransactionTransportProfilePreimage : List UInt8 :=
  "dclutch/relayed-transport-one-transaction/v1".toUTF8.toList
def oneTransactionTransportProfileId : List UInt8 := [
  0xae, 0x70, 0xb6, 0x6d, 0xe0, 0x09, 0xae, 0x3d,
  0x52, 0x25, 0xd4, 0xf3, 0x4e, 0xe2, 0x5a, 0x2b,
  0x4a, 0xd0, 0x38, 0x96, 0x48, 0xb8, 0xa1, 0xd4,
  0xf5, 0x02, 0x8d, 0x5d, 0xdc, 0x0e, 0xfc, 0x61
]

def keySetSchemaReleasePreimage : List UInt8 :=
  "dclutch/relayer-key-set-schema/v1".toUTF8.toList
def keySetSchemaReleaseId : List UInt8 := [
  0x01, 0x74, 0x8b, 0x41, 0xa3, 0x41, 0xbd, 0x03,
  0x30, 0xc3, 0x09, 0x91, 0x8c, 0x6d, 0x6d, 0x8f,
  0xba, 0xc6, 0x05, 0x6a, 0x39, 0x8a, 0x8d, 0xaa,
  0x8c, 0xbf, 0x29, 0xf8, 0xce, 0x0b, 0x2b, 0xe5
]

def adapterConfigSchemaReleasePreimage : List UInt8 :=
  "dclutch/relayed-adapter-config-schema/v1".toUTF8.toList
def adapterConfigSchemaReleaseId : List UInt8 := [
  0xf5, 0x90, 0x9f, 0x8a, 0xed, 0xda, 0xbd, 0xfe,
  0x75, 0x10, 0x8b, 0x85, 0x20, 0x13, 0xa3, 0x9d,
  0x69, 0x4c, 0x9a, 0x40, 0x16, 0x1d, 0xb4, 0x08,
  0x99, 0x60, 0xb0, 0x13, 0xbb, 0xb8, 0x5b, 0xa7
]

/-- The decoding-rules record's schema release.  §4.10's swap tripwire is the
assertion that `decoding_rules_id` is byte-identical across trust roots; this
is the schema those records are minted under, not any one record. -/
def decodingRulesSchemaReleasePreimage : List UInt8 :=
  "dclutch/relayed-decoding-rules-schema/v1".toUTF8.toList
def decodingRulesSchemaReleaseId : List UInt8 := [
  0x9c, 0x68, 0xc8, 0xb9, 0x43, 0x61, 0x6b, 0xba,
  0x40, 0xad, 0x61, 0x90, 0xcb, 0xa6, 0xbe, 0x11,
  0xa8, 0x03, 0xaf, 0xe7, 0x5e, 0xa3, 0xbc, 0x1a,
  0x8b, 0x1a, 0x8e, 0x11, 0xe3, 0x29, 0xc2, 0x6b
]

def recordSchemaReleasePreimage : List UInt8 :=
  "dclutch/relayed-observation-record-schema/v1".toUTF8.toList
def recordSchemaReleaseId : List UInt8 := [
  0x30, 0x9e, 0xe5, 0xc4, 0x93, 0x1e, 0x56, 0x36,
  0xc7, 0xf9, 0xb5, 0x71, 0x67, 0x56, 0x94, 0x30,
  0x9c, 0xf5, 0xc1, 0x8c, 0xb2, 0x12, 0xbd, 0xb8,
  0x47, 0x64, 0xa9, 0x41, 0x99, 0x0b, 0x1b, 0xfc
]

def recordDerivationReleasePreimage : List UInt8 :=
  "dclutch/relayed-observation-record-derivation/v1".toUTF8.toList
def recordDerivationReleaseId : List UInt8 := [
  0x7d, 0xab, 0xc8, 0xf4, 0xfb, 0xd0, 0x30, 0xa8,
  0x16, 0xc0, 0x5f, 0xed, 0xdf, 0xe3, 0x4a, 0x82,
  0x42, 0xe7, 0x07, 0x9c, 0xac, 0x1f, 0xae, 0x46,
  0xdf, 0x93, 0x24, 0x8d, 0xd9, 0x7b, 0xde, 0x9e
]

def attestationWireReleasePreimage : List UInt8 :=
  "dclutch/relayed-attestation-wire/v1".toUTF8.toList
def attestationWireReleaseId : List UInt8 := [
  0xaf, 0x2c, 0xaa, 0x56, 0xd8, 0x2f, 0x55, 0x41,
  0xff, 0x38, 0x5a, 0xc2, 0x2e, 0xaa, 0x27, 0xb0,
  0xdc, 0x29, 0xd0, 0x66, 0xaa, 0x16, 0x8b, 0xba,
  0x89, 0x53, 0x4e, 0x39, 0x0b, 0x34, 0x0e, 0xa9
]

/-! ## Unhashed domain separators

These are seed and digest domains, consumed as bytes rather than as a content
identity, so they are pinned as preimages only. -/

/-- PDA seed domain of one persisted observation record. -/
def recordPdaDomain : List UInt8 := "dclutch/relayed-obs/v1".toUTF8.toList
/-- Domain separator of the founding-time ordered account-set identity. -/
def accountSetDomain : List UInt8 := "dclutch/relayed-account-set/v1".toUTF8.toList
/-- Domain separator of the running set-digest fold. -/
def setDigestDomain : List UInt8 := "dclutch/relayed-set/v1".toUTF8.toList

/-! ## Cluster identities

`observed_cluster_id` is a *signed* field precisely because a venue `Program`
account can be byte-identical on two clusters (§4.6).  Both genesis hashes are
pinned so the negative test — the devnet twin must refuse, and refuse on the
cluster identity — is expressible against constants rather than prose. -/

/-- Solana mainnet-beta genesis hash `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`. -/
def mainnetGenesisHash : List UInt8 := [
  0x45, 0x29, 0x69, 0x98, 0xa6, 0xf8, 0xe2, 0xa7,
  0x84, 0xdb, 0x5d, 0x9f, 0x95, 0xe1, 0x8f, 0xc2,
  0x3f, 0x70, 0x44, 0x1a, 0x10, 0x39, 0x44, 0x68,
  0x01, 0x08, 0x98, 0x79, 0xb0, 0x8c, 0x7e, 0xf0
]

/-- Solana devnet genesis hash `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`. -/
def devnetGenesisHash : List UInt8 := [
  0xce, 0x59, 0xdb, 0x50, 0x80, 0xfc, 0x2c, 0x6d,
  0x3b, 0xcf, 0x7c, 0xa9, 0x07, 0x12, 0xd3, 0xc2,
  0xe5, 0xe6, 0xc2, 0x8f, 0x27, 0xf0, 0xdf, 0xbb,
  0x99, 0x53, 0xbd, 0xb0, 0x89, 0x4c, 0x03, 0xab
]

theorem clusters_are_distinguishable : mainnetGenesisHash ≠ devnetGenesisHash := by
  native_decide

/-- SHA-256 of the empty string.  A fully inline body must carry exactly this
as its `tailDigest`; the adapter recomputes rather than special-cases. -/
def emptyTailDigest : List UInt8 := [
  0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14,
  0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
  0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
  0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55
]

/-! ## Profile bounds -/

/-- Chain-derived ceiling from the 1,232-byte packet with provisional frame
arithmetic (§4.4).  It must be re-derived from a measured frame before release. -/
def maxInlineBytes : Nat := 448
/-- Provisional; chosen to cover the widest set in the chain-state dossier
(pool, two vaults, program, programdata, clock) with headroom. -/
def maxAccounts : Nat := 8
/-- The Loader V3 `ProgramData` metadata prefix.  A body inlining exactly this
many bytes carries `elf_digest` in `tailDigest` by construction. -/
def loaderV3ProgramDataMetadataBytes : Nat := 45
/-- Loader V3 `Program` account width; the whole body rides inline. -/
def loaderV3ProgramBytes : Nat := 36
/-- Mainnet `Clock` sysvar width; the whole body rides inline. -/
def clockSysvarBytes : Nat := 40

def schemaVersion : Nat := 1

/-- Zero a whole field span.  A corpus entry that flips one byte of a
multi-byte field can leave the field still admissible; zeroing the span states
the intended corruption exactly. -/
def zeroSpan (input : List UInt8) (offset width : Nat) : List UInt8 :=
  input.take offset ++ List.replicate width 0 ++ input.drop (offset + width)

theorem zeroSpan_preserves_length (input : List UInt8) (offset width : Nat)
    (fits : offset + width ≤ input.length) :
    (zeroSpan input offset width).length = input.length := by
  simp [zeroSpan]
  omega

/-! ## `RelayedAccountObservationV1` — the observation body -/

namespace Observation

def magicless : Unit := ()

inductive Field where
  | key | owner | lamports | dataLen | inlineLen | executable | reserved | tailDigest
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.key, .bytes 32⟩,
  ⟨.owner, .bytes 32⟩,
  ⟨.lamports, .u64⟩,
  ⟨.dataLen, .u32⟩,
  ⟨.inlineLen, .u16⟩,
  ⟨.executable, .u8⟩,
  ⟨.reserved, .reserved 1⟩,
  ⟨.tailDigest, .bytes 32⟩
]

def layout : List (PlacedField Field) := specialize schema
def headBytes : Nat := schemaWidth schema

theorem head_width : headBytes = 112 := by native_decide

theorem coordinates_pinned : coordinates layout = [
    (.key, 0, 32),
    (.owner, 32, 32),
    (.lamports, 64, 8),
    (.dataLen, 72, 4),
    (.inlineLen, 76, 2),
    (.executable, 78, 1),
    (.reserved, 79, 1),
    (.tailDigest, 80, 32)
  ] := by native_decide

theorem well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

namespace Field
def rustName : Field → String
  | .key => "RELAYED_OBSERVATION_KEY_OFFSET"
  | .owner => "RELAYED_OBSERVATION_OWNER_OFFSET"
  | .lamports => "RELAYED_OBSERVATION_LAMPORTS_OFFSET"
  | .dataLen => "RELAYED_OBSERVATION_DATA_LEN_OFFSET"
  | .inlineLen => "RELAYED_OBSERVATION_INLINE_LEN_OFFSET"
  | .executable => "RELAYED_OBSERVATION_EXECUTABLE_OFFSET"
  | .reserved => "RELAYED_OBSERVATION_RESERVED_OFFSET"
  | .tailDigest => "RELAYED_OBSERVATION_TAIL_DIGEST_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0
end Field

/-- One observation body as semantics rather than bytes. -/
structure Body where
  key : Nat
  owner : Nat
  lamports : Nat
  dataLen : Nat
  inline : List UInt8
  executable : Bool
  tailDigest : Nat
  deriving DecidableEq, Repr

def fitsId (value : Nat) : Bool := value < 256 ^ 32

/-- Admissibility of one body, independent of any venue.  The two rules that
matter are `inlineLen ≤ min dataLen maxInlineBytes` — the relayer may carry a
prefix but never invent one — and a nonzero owner and key, which the adapter
then compares against the founding-time pin. -/
def Body.valid (value : Body) : Bool :=
  value.key != 0 && fitsId value.key &&
  value.owner != 0 && fitsId value.owner &&
  value.lamports < 256 ^ 8 &&
  value.dataLen < 256 ^ 4 &&
  value.inline.length ≤ value.dataLen &&
  value.inline.length ≤ maxInlineBytes &&
  fitsId value.tailDigest

def encode (value : Body) : List UInt8 :=
  Codec.encodeLE 32 value.key ++
  Codec.encodeLE 32 value.owner ++
  Codec.encodeLE 8 value.lamports ++
  Codec.encodeLE 4 value.dataLen ++
  Codec.encodeLE 2 value.inline.length ++
  [if value.executable then 1 else 0] ++ [0] ++
  Codec.encodeLE 32 value.tailDigest ++
  value.inline

theorem encoding_length (value : Body) :
    (encode value).length = headBytes + value.inline.length := by
  simp [encode, headBytes, schema, schemaWidth, Codec.encodeLE_length,
    FieldKind.byteWidth]
  omega

/-- A fully inline body is not a variant: it is the case `inlineLen = dataLen`,
and its tail digest is forced to the empty-string digest by the same recompute
every other body faces. -/
def fullyInline (value : Body) : Bool := value.inline.length = value.dataLen

end Observation

/-! ## `RelayedMainnetAccountAttestationV1` -/

namespace Attestation

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x4d, 0x41, 0x31] -- `DCLTRMA1`

inductive Field where
  | magic | version | reserved | messageLen | observedClusterId | relayFamilyId
  | decodingRulesId | accountSetId | observedSlot | setIndex | setCount
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reserved, .reserved 2⟩,
  ⟨.messageLen, .u32⟩,
  ⟨.observedClusterId, .bytes 32⟩,
  ⟨.relayFamilyId, .bytes 32⟩,
  ⟨.decodingRulesId, .bytes 32⟩,
  ⟨.accountSetId, .bytes 32⟩,
  ⟨.observedSlot, .u64⟩,
  ⟨.setIndex, .u16⟩,
  ⟨.setCount, .u16⟩
]

def layout : List (PlacedField Field) := specialize schema
def headBytes : Nat := schemaWidth schema

theorem head_width : headBytes = 156 := by native_decide

theorem coordinates_pinned : coordinates layout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.reserved, 10, 2),
    (.messageLen, 12, 4),
    (.observedClusterId, 16, 32),
    (.relayFamilyId, 48, 32),
    (.decodingRulesId, 80, 32),
    (.accountSetId, 112, 32),
    (.observedSlot, 144, 8),
    (.setIndex, 152, 2),
    (.setCount, 154, 2)
  ] := by native_decide

theorem well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

/-- Fixed cost of one single-account attestation, before inline bytes. -/
theorem fixed_cost : headBytes + Observation.headBytes = 268 := by native_decide

namespace Field
def rustName : Field → String
  | .magic => "RELAYED_ATTESTATION_MAGIC_OFFSET"
  | .version => "RELAYED_ATTESTATION_VERSION_OFFSET"
  | .reserved => "RELAYED_ATTESTATION_RESERVED_OFFSET"
  | .messageLen => "RELAYED_ATTESTATION_MESSAGE_LEN_OFFSET"
  | .observedClusterId => "RELAYED_ATTESTATION_OBSERVED_CLUSTER_ID_OFFSET"
  | .relayFamilyId => "RELAYED_ATTESTATION_RELAY_FAMILY_ID_OFFSET"
  | .decodingRulesId => "RELAYED_ATTESTATION_DECODING_RULES_ID_OFFSET"
  | .accountSetId => "RELAYED_ATTESTATION_ACCOUNT_SET_ID_OFFSET"
  | .observedSlot => "RELAYED_ATTESTATION_OBSERVED_SLOT_OFFSET"
  | .setIndex => "RELAYED_ATTESTATION_SET_INDEX_OFFSET"
  | .setCount => "RELAYED_ATTESTATION_SET_COUNT_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0
end Field

structure Message where
  observedClusterId : Nat
  relayFamilyId : Nat
  decodingRulesId : Nat
  accountSetId : Nat
  observedSlot : Nat
  setIndex : Nat
  setCount : Nat
  body : Observation.Body
  deriving DecidableEq, Repr

def Message.valid (value : Message) : Bool :=
  value.observedClusterId != 0 && Observation.fitsId value.observedClusterId &&
  value.relayFamilyId != 0 && Observation.fitsId value.relayFamilyId &&
  value.decodingRulesId != 0 && Observation.fitsId value.decodingRulesId &&
  value.accountSetId != 0 && Observation.fitsId value.accountSetId &&
  value.observedSlot < 256 ^ 8 &&
  0 < value.setCount && value.setCount ≤ maxAccounts &&
  value.setIndex < value.setCount &&
  value.body.valid

def encode (value : Message) : List UInt8 :=
  let body := Observation.encode value.body
  magic ++ Codec.encodeLE 2 schemaVersion ++ List.replicate 2 0 ++
  Codec.encodeLE 4 (headBytes + body.length) ++
  Codec.encodeLE 32 value.observedClusterId ++
  Codec.encodeLE 32 value.relayFamilyId ++
  Codec.encodeLE 32 value.decodingRulesId ++
  Codec.encodeLE 32 value.accountSetId ++
  Codec.encodeLE 8 value.observedSlot ++
  Codec.encodeLE 2 value.setIndex ++
  Codec.encodeLE 2 value.setCount ++
  body

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

/-- The hostile decoder, as a predicate over bytes.  `messageLen` is required
to equal the actual verified message length: a signature over a longer or
shorter slice than the field declares is refused before any field is read. -/
def validBytes (input : List UInt8) : Bool :=
  Observation.headBytes + headBytes ≤ input.length &&
  input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reserved.offset).take 2 = List.replicate 2 0 &&
  sliceNat input Field.messageLen.offset 4 = input.length &&
  sliceNat input Field.observedClusterId.offset 32 != 0 &&
  sliceNat input Field.relayFamilyId.offset 32 != 0 &&
  sliceNat input Field.decodingRulesId.offset 32 != 0 &&
  sliceNat input Field.accountSetId.offset 32 != 0 &&
  sliceNat input (headBytes + Observation.Field.key.offset) 32 != 0 &&
  sliceNat input (headBytes + Observation.Field.owner.offset) 32 != 0 &&
  (input.drop (headBytes + Observation.Field.reserved.offset)).take 1 = [0] &&
  sliceNat input (headBytes + Observation.Field.executable.offset) 1 ≤ 1 &&
  sliceNat input (headBytes + Observation.Field.inlineLen.offset) 2 ≤ maxInlineBytes &&
  sliceNat input (headBytes + Observation.Field.inlineLen.offset) 2 ≤
    sliceNat input (headBytes + Observation.Field.dataLen.offset) 4 &&
  input.length =
    headBytes + Observation.headBytes +
      sliceNat input (headBytes + Observation.Field.inlineLen.offset) 2 &&
  sliceNat input Field.setCount.offset 2 != 0 &&
  sliceNat input Field.setCount.offset 2 ≤ maxAccounts &&
  sliceNat input Field.setIndex.offset 2 < sliceNat input Field.setCount.offset 2

def exampleBody : Observation.Body := {
  key := 7
  owner := 9
  lamports := 1_000_000
  dataLen := 4
  inline := [0xaa, 0xbb, 0xcc, 0xdd]
  executable := false
  tailDigest := 11
}

def exampleMessage : Message := {
  observedClusterId := 3
  relayFamilyId := 4
  decodingRulesId := 5
  accountSetId := 6
  observedSlot := 423941138
  setIndex := 2
  setCount := 4
  body := exampleBody
}

theorem example_valid : exampleMessage.valid = true := by native_decide
theorem example_bytes_accepted : validBytes (encode exampleMessage) = true := by native_decide
theorem example_length : (encode exampleMessage).length = 272 := by native_decide

/-- Every entry corrupts exactly one field of the accepted example.  The
adversarial names are deliberate: they are the wire half of §6.3's hostile
corpus, and each must be refused by `validBytes` alone. -/
def refusalCorpus : List (List UInt8) := [
  (encode exampleMessage).set 0 0,                                     -- wrong magic
  (encode exampleMessage).set Field.version.offset 2,                  -- wrong abi version
  (encode exampleMessage).set Field.reserved.offset 1,                 -- nonzero reserved
  (encode exampleMessage).set Field.messageLen.offset 0,               -- declared length lie
  (encode exampleMessage).set Field.observedClusterId.offset 0,        -- zero cluster identity
  (encode exampleMessage).set Field.relayFamilyId.offset 0,            -- zero family
  (encode exampleMessage).set Field.decodingRulesId.offset 0,          -- zero decoding rules
  (encode exampleMessage).set Field.accountSetId.offset 0,             -- zero account set
  (encode exampleMessage).set (headBytes + Observation.Field.key.offset) 0,
  (encode exampleMessage).set (headBytes + Observation.Field.owner.offset) 0,
  (encode exampleMessage).set (headBytes + Observation.Field.reserved.offset) 1,
  (encode exampleMessage).set (headBytes + Observation.Field.executable.offset) 2,
  (encode exampleMessage).set (headBytes + Observation.Field.inlineLen.offset) 5,
  (encode exampleMessage).set Field.setCount.offset 0,
  (encode exampleMessage).set Field.setIndex.offset 4,
  (encode exampleMessage).take 271,
  (encode exampleMessage) ++ [0]
]

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

/-- Truncation at every prefix is refused.  This is the property a hostile
decoder most often fails, and it is checked exhaustively rather than sampled. -/
theorem every_truncation_refuses :
    ((List.range (encode exampleMessage).length).map
      fun width => (encode exampleMessage).take width).all
        fun candidate => !validBytes candidate := by native_decide

end Attestation

/-! ## `RelayedObservationSetSealV1` -/

namespace Seal

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x53, 0x53, 0x31] -- `DCLTRSS1`

inductive Field where
  | magic | version | reserved | messageLen | observedClusterId | relayFamilyId
  | accountSetId | observedSlot | setCount | reservedTail | setDigest
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reserved, .reserved 2⟩,
  ⟨.messageLen, .u32⟩,
  ⟨.observedClusterId, .bytes 32⟩,
  ⟨.relayFamilyId, .bytes 32⟩,
  ⟨.accountSetId, .bytes 32⟩,
  ⟨.observedSlot, .u64⟩,
  ⟨.setCount, .u16⟩,
  ⟨.reservedTail, .reserved 2⟩,
  ⟨.setDigest, .bytes 32⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

theorem exact_width : bytes = 156 := by native_decide

theorem coordinates_pinned : coordinates layout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.reserved, 10, 2),
    (.messageLen, 12, 4),
    (.observedClusterId, 16, 32),
    (.relayFamilyId, 48, 32),
    (.accountSetId, 80, 32),
    (.observedSlot, 112, 8),
    (.setCount, 120, 2),
    (.reservedTail, 122, 2),
    (.setDigest, 124, 32)
  ] := by native_decide

theorem well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

namespace Field
def rustName : Field → String
  | .magic => "RELAYED_SEAL_MAGIC_OFFSET"
  | .version => "RELAYED_SEAL_VERSION_OFFSET"
  | .reserved => "RELAYED_SEAL_RESERVED_OFFSET"
  | .messageLen => "RELAYED_SEAL_MESSAGE_LEN_OFFSET"
  | .observedClusterId => "RELAYED_SEAL_OBSERVED_CLUSTER_ID_OFFSET"
  | .relayFamilyId => "RELAYED_SEAL_RELAY_FAMILY_ID_OFFSET"
  | .accountSetId => "RELAYED_SEAL_ACCOUNT_SET_ID_OFFSET"
  | .observedSlot => "RELAYED_SEAL_OBSERVED_SLOT_OFFSET"
  | .setCount => "RELAYED_SEAL_SET_COUNT_OFFSET"
  | .reservedTail => "RELAYED_SEAL_RESERVED_TAIL_OFFSET"
  | .setDigest => "RELAYED_SEAL_SET_DIGEST_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0
end Field

structure Message where
  observedClusterId : Nat
  relayFamilyId : Nat
  accountSetId : Nat
  observedSlot : Nat
  setCount : Nat
  setDigest : Nat
  deriving DecidableEq, Repr

def Message.valid (value : Message) : Bool :=
  value.observedClusterId != 0 && Observation.fitsId value.observedClusterId &&
  value.relayFamilyId != 0 && Observation.fitsId value.relayFamilyId &&
  value.accountSetId != 0 && Observation.fitsId value.accountSetId &&
  value.observedSlot < 256 ^ 8 &&
  0 < value.setCount && value.setCount ≤ maxAccounts &&
  value.setDigest != 0 && Observation.fitsId value.setDigest

def encode (value : Message) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++ List.replicate 2 0 ++
  Codec.encodeLE 4 bytes ++
  Codec.encodeLE 32 value.observedClusterId ++
  Codec.encodeLE 32 value.relayFamilyId ++
  Codec.encodeLE 32 value.accountSetId ++
  Codec.encodeLE 8 value.observedSlot ++
  Codec.encodeLE 2 value.setCount ++
  List.replicate 2 0 ++
  Codec.encodeLE 32 value.setDigest

theorem encoding_length (value : Message) : (encode value).length = bytes := by
  simp [encode, bytes, schema, schemaWidth, magic, Codec.encodeLE_length,
    FieldKind.byteWidth]

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def validBytes (input : List UInt8) : Bool :=
  input.length = bytes &&
  input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reserved.offset).take 2 = List.replicate 2 0 &&
  sliceNat input Field.messageLen.offset 4 = bytes &&
  sliceNat input Field.observedClusterId.offset 32 != 0 &&
  sliceNat input Field.relayFamilyId.offset 32 != 0 &&
  sliceNat input Field.accountSetId.offset 32 != 0 &&
  sliceNat input Field.setCount.offset 2 != 0 &&
  sliceNat input Field.setCount.offset 2 ≤ maxAccounts &&
  (input.drop Field.reservedTail.offset).take 2 = List.replicate 2 0 &&
  sliceNat input Field.setDigest.offset 32 != 0

def exampleMessage : Message := {
  observedClusterId := 3
  relayFamilyId := 4
  accountSetId := 6
  observedSlot := 423941138
  setCount := 4
  setDigest := 12345
}

theorem example_valid : exampleMessage.valid = true := by native_decide
theorem example_bytes_accepted : validBytes (encode exampleMessage) = true := by native_decide

def refusalCorpus : List (List UInt8) := [
  (encode exampleMessage).set 0 0,
  (encode exampleMessage).set Field.version.offset 2,
  (encode exampleMessage).set Field.reserved.offset 1,
  (encode exampleMessage).set Field.messageLen.offset 0,
  (encode exampleMessage).set Field.observedClusterId.offset 0,
  (encode exampleMessage).set Field.relayFamilyId.offset 0,
  (encode exampleMessage).set Field.accountSetId.offset 0,
  (encode exampleMessage).set Field.setCount.offset 0,
  (encode exampleMessage).set Field.setCount.offset 9,
  (encode exampleMessage).set Field.reservedTail.offset 1,
  zeroSpan (encode exampleMessage) Field.setDigest.offset 32,
  (encode exampleMessage).take 155,
  (encode exampleMessage) ++ [0]
]

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

theorem every_truncation_refuses :
    ((List.range bytes).map fun width => (encode exampleMessage).take width).all
      fun candidate => !validBytes candidate := by native_decide

end Seal

/-! ## `RelayerKeySetV1`

The record whose content identity *is*
`ProviderReleaseV1.provider_deployment_release_id`.  Keys ascend strictly as
byte strings, which makes the set canonical and duplicates structurally
impossible rather than merely checked. -/

namespace KeySet

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x4b, 0x53, 0x31] -- `DCLTRKS1`

def maxKeys : Nat := 5

inductive Field where
  | magic | version | keyCount | sealThreshold | reserved | keys
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.keyCount, .u8⟩,
  ⟨.sealThreshold, .u8⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.keys, .bytes 160⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

theorem exact_width : bytes = 176 := by native_decide

theorem coordinates_pinned : coordinates layout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.keyCount, 10, 1),
    (.sealThreshold, 11, 1),
    (.reserved, 12, 4),
    (.keys, 16, 160)
  ] := by native_decide

theorem well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

/-- The key set is exactly as wide as `ProviderReleaseV1`, which is the record
it stands in for. -/
theorem matches_provider_release_width : bytes = 176 := by native_decide

namespace Field
def rustName : Field → String
  | .magic => "RELAYER_KEY_SET_MAGIC_OFFSET"
  | .version => "RELAYER_KEY_SET_VERSION_OFFSET"
  | .keyCount => "RELAYER_KEY_SET_KEY_COUNT_OFFSET"
  | .sealThreshold => "RELAYER_KEY_SET_SEAL_THRESHOLD_OFFSET"
  | .reserved => "RELAYER_KEY_SET_RESERVED_OFFSET"
  | .keys => "RELAYER_KEY_SET_KEYS_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0
end Field

structure Set where
  keys : List Nat
  sealThreshold : Nat
  deriving DecidableEq, Repr

/-- Strictly ascending big-endian key material.  Keys are compared as byte
strings on the wire; `Nat` ordering of the big-endian decode agrees. -/
def strictlyAscending : List Nat → Bool
  | [] => true
  | [_] => true
  | first :: second :: rest => first < second && strictlyAscending (second :: rest)

def Set.valid (value : Set) : Bool :=
  0 < value.keys.length && value.keys.length ≤ maxKeys &&
  value.keys.all (fun key => key != 0 && Observation.fitsId key) &&
  strictlyAscending value.keys &&
  0 < value.sealThreshold && value.sealThreshold ≤ value.keys.length

def encodeKeys (keys : List Nat) : List UInt8 :=
  (keys.flatMap (fun key => Codec.encodeLE 32 key)) ++
    List.replicate ((maxKeys - keys.length) * 32) 0

def encode (value : Set) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++
  [UInt8.ofNat value.keys.length] ++ [UInt8.ofNat value.sealThreshold] ++
  List.replicate 4 0 ++ encodeKeys value.keys

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def keyAt (input : List UInt8) (index : Nat) : Nat :=
  sliceNat input (Field.keys.offset + index * 32) 32

def validBytes (input : List UInt8) : Bool :=
  input.length = bytes &&
  input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reserved.offset).take 4 = List.replicate 4 0 &&
  0 < sliceNat input Field.keyCount.offset 1 &&
  sliceNat input Field.keyCount.offset 1 ≤ maxKeys &&
  0 < sliceNat input Field.sealThreshold.offset 1 &&
  sliceNat input Field.sealThreshold.offset 1 ≤ sliceNat input Field.keyCount.offset 1 &&
  strictlyAscending
    ((List.range (sliceNat input Field.keyCount.offset 1)).map (keyAt input)) &&
  ((List.range (sliceNat input Field.keyCount.offset 1)).all
    fun index => keyAt input index != 0) &&
  ((List.range' (sliceNat input Field.keyCount.offset 1)
      (maxKeys - sliceNat input Field.keyCount.offset 1)).all
    fun index => keyAt input index = 0)

def exampleSet : Set := { keys := [11, 22, 33], sealThreshold := 2 }
def singletonSet : Set := { keys := [11], sealThreshold := 1 }

theorem example_valid : exampleSet.valid = true := by native_decide
theorem example_bytes_accepted : validBytes (encode exampleSet) = true := by native_decide
theorem singleton_bytes_accepted : validBytes (encode singletonSet) = true := by native_decide

/-- Duplicate members cannot be encoded at all: the ascending rule refuses
them structurally rather than by a separate uniqueness pass. -/
theorem duplicate_keys_refuse :
    Set.valid { keys := [11, 11], sealThreshold := 1 } = false := by native_decide

theorem descending_keys_refuse :
    Set.valid { keys := [22, 11], sealThreshold := 1 } = false := by native_decide

theorem threshold_above_cardinality_refuses :
    Set.valid { keys := [11, 22], sealThreshold := 3 } = false := by native_decide

def refusalCorpus : List (List UInt8) := [
  (encode exampleSet).set 0 0,
  (encode exampleSet).set Field.version.offset 2,
  (encode exampleSet).set Field.reserved.offset 1,
  (encode exampleSet).set Field.keyCount.offset 0,
  (encode exampleSet).set Field.keyCount.offset 6,
  (encode exampleSet).set Field.sealThreshold.offset 0,
  (encode exampleSet).set Field.sealThreshold.offset 4,
  (encode exampleSet).set Field.keys.offset 0,
  (encode exampleSet).set (Field.keys.offset + 32) 11,
  (encode exampleSet).set (Field.keys.offset + 96) 44,
  (encode exampleSet).take 175,
  (encode exampleSet) ++ [0]
]

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

end KeySet

/-! ## `RelayedAdapterConfigV1` -/

namespace AdapterConfig

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x41, 0x43, 0x31] -- `DCLTRAC1`

inductive Field where
  | magic | version | reserved | accountSetId | observableSelector | rawExponent
  | maxObservationAgeSeconds | maxClusterSkewSeconds | reservedTail
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reserved, .reserved 6⟩,
  ⟨.accountSetId, .bytes 32⟩,
  ⟨.observableSelector, .u32⟩,
  ⟨.rawExponent, .u32⟩,
  ⟨.maxObservationAgeSeconds, .u64⟩,
  ⟨.maxClusterSkewSeconds, .u64⟩,
  ⟨.reservedTail, .reserved 8⟩
]

def layout : List (PlacedField Field) := specialize schema
def bytes : Nat := schemaWidth schema

theorem exact_width : bytes = 80 := by native_decide

theorem coordinates_pinned : coordinates layout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.reserved, 10, 6),
    (.accountSetId, 16, 32),
    (.observableSelector, 48, 4),
    (.rawExponent, 52, 4),
    (.maxObservationAgeSeconds, 56, 8),
    (.maxClusterSkewSeconds, 64, 8),
    (.reservedTail, 72, 8)
  ] := by native_decide

theorem well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

namespace Field
def rustName : Field → String
  | .magic => "RELAYED_ADAPTER_CONFIG_MAGIC_OFFSET"
  | .version => "RELAYED_ADAPTER_CONFIG_VERSION_OFFSET"
  | .reserved => "RELAYED_ADAPTER_CONFIG_RESERVED_OFFSET"
  | .accountSetId => "RELAYED_ADAPTER_CONFIG_ACCOUNT_SET_ID_OFFSET"
  | .observableSelector => "RELAYED_ADAPTER_CONFIG_OBSERVABLE_SELECTOR_OFFSET"
  | .rawExponent => "RELAYED_ADAPTER_CONFIG_RAW_EXPONENT_OFFSET"
  | .maxObservationAgeSeconds => "RELAYED_ADAPTER_CONFIG_MAX_OBSERVATION_AGE_SECONDS_OFFSET"
  | .maxClusterSkewSeconds => "RELAYED_ADAPTER_CONFIG_MAX_CLUSTER_SKEW_SECONDS_OFFSET"
  | .reservedTail => "RELAYED_ADAPTER_CONFIG_RESERVED_TAIL_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0
end Field

structure Config where
  accountSetId : Nat
  observableSelector : Nat
  rawExponent : Int
  maxObservationAgeSeconds : Nat
  maxClusterSkewSeconds : Nat
  deriving DecidableEq, Repr

/-- The one founding-time predicate that keeps two-cluster skew from ever being
the thing that walks a market to failure: the window's own liveness grace must
cover the declared skew allowance.  §4.7 states it; here it is checkable. -/
def admitsWindow (value : Config) (windowMaxAgeSeconds : Nat) : Bool :=
  value.maxClusterSkewSeconds ≤ windowMaxAgeSeconds

def Config.valid (value : Config) : Bool :=
  value.accountSetId != 0 && Observation.fitsId value.accountSetId &&
  value.observableSelector < 256 ^ 4 &&
  -2147483648 ≤ value.rawExponent && value.rawExponent ≤ 2147483647 &&
  0 < value.maxObservationAgeSeconds && value.maxObservationAgeSeconds < 256 ^ 8 &&
  value.maxClusterSkewSeconds < value.maxObservationAgeSeconds

def encodeExponent (value : Int) : List UInt8 :=
  Codec.encodeLE 4 (if value < 0 then (4294967296 + value).toNat else value.toNat)

def encode (value : Config) : List UInt8 :=
  magic ++ Codec.encodeLE 2 schemaVersion ++ List.replicate 6 0 ++
  Codec.encodeLE 32 value.accountSetId ++
  Codec.encodeLE 4 value.observableSelector ++
  encodeExponent value.rawExponent ++
  Codec.encodeLE 8 value.maxObservationAgeSeconds ++
  Codec.encodeLE 8 value.maxClusterSkewSeconds ++
  List.replicate 8 0

def sliceNat (input : List UInt8) (offset width : Nat) : Nat :=
  Codec.decodeLE ((input.drop offset).take width)

def validBytes (input : List UInt8) : Bool :=
  input.length = bytes &&
  input.take 8 = magic &&
  sliceNat input Field.version.offset 2 = schemaVersion &&
  (input.drop Field.reserved.offset).take 6 = List.replicate 6 0 &&
  (input.drop Field.reservedTail.offset).take 8 = List.replicate 8 0 &&
  sliceNat input Field.accountSetId.offset 32 != 0 &&
  0 < sliceNat input Field.maxObservationAgeSeconds.offset 8 &&
  sliceNat input Field.maxClusterSkewSeconds.offset 8 <
    sliceNat input Field.maxObservationAgeSeconds.offset 8

def exampleConfig : Config := {
  accountSetId := 6
  observableSelector := 0
  rawExponent := -8
  maxObservationAgeSeconds := 5400
  maxClusterSkewSeconds := 120
}

theorem example_valid : exampleConfig.valid = true := by native_decide
theorem example_bytes_accepted : validBytes (encode exampleConfig) = true := by native_decide

/-- A negative declared scale survives the round trip; the exponent is a signed
field and the reserved spans are not a place to hide its sign. -/
theorem negative_exponent_round_trips :
    sliceNat (encode exampleConfig) Field.rawExponent.offset 4 = 4294967288 := by native_decide

/-- Skew alone can never expire a window whose grace covers it. -/
theorem skew_allowance_admitted : admitsWindow exampleConfig 5400 = true := by native_decide

theorem skew_allowance_refused_when_window_is_tighter :
    admitsWindow exampleConfig 60 = false := by native_decide

def refusalCorpus : List (List UInt8) := [
  (encode exampleConfig).set 0 0,
  (encode exampleConfig).set Field.version.offset 2,
  (encode exampleConfig).set Field.reserved.offset 1,
  (encode exampleConfig).set Field.reservedTail.offset 1,
  (encode exampleConfig).set Field.accountSetId.offset 0,
  zeroSpan (encode exampleConfig) Field.maxObservationAgeSeconds.offset 8,
  (encode exampleConfig).set (Field.maxClusterSkewSeconds.offset + 1) 0xff,
  (encode exampleConfig).take 79,
  (encode exampleConfig) ++ [0]
]

theorem generated_refusal_corpus_refuses :
    refusalCorpus.all fun candidate => !validBytes candidate := by native_decide

end AdapterConfig

/-! ## `RelayedObservationRecordV1` — the persisted record header

The record is a direct Market child seeded by `observed_slot`, which is what
makes equivocation structurally bounded: at most one record exists per set per
slot, so a second contradictory signature cannot overwrite the first — it can
only become a permanent, publicly checkable proof of equivocation. -/

namespace Record

def magic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x52, 0x4f, 0x42, 0x31] -- `DCLTROB1`

/-- Fill is 1-of-n authenticated; sealing is m-of-n.  A malicious member can
waste a rent deposit and publish a signed lie; it cannot deny service, because
honest members build a record at a different slot. -/
inductive Phase where
  | collecting | sealed | consumed | retired
  deriving DecidableEq, Repr

def Phase.byte : Phase → Nat
  | .collecting => 1
  | .sealed => 2
  | .consumed => 3
  | .retired => 4

inductive Field where
  | magic | version | reserved | market | generation | sourceMaterialId
  | accountSetId | providerReleaseId | relayerKeySetId | observedClusterId
  | observedSlot | setDigest | rentCreditBeneficiary | createdUnixSeconds
  | sealedUnixSeconds | setCount | filledCount | sealThreshold | sealCount
  | sealedByBitmap | phase | reservedTail
  deriving DecidableEq, Repr

def schema : List (FieldSpec Field) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reserved, .reserved 2⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.sourceMaterialId, .bytes 32⟩,
  ⟨.accountSetId, .bytes 32⟩,
  ⟨.providerReleaseId, .bytes 32⟩,
  ⟨.relayerKeySetId, .bytes 32⟩,
  ⟨.observedClusterId, .bytes 32⟩,
  ⟨.observedSlot, .u64⟩,
  ⟨.setDigest, .bytes 32⟩,
  ⟨.rentCreditBeneficiary, .bytes 32⟩,
  ⟨.createdUnixSeconds, .u64⟩,
  ⟨.sealedUnixSeconds, .u64⟩,
  ⟨.setCount, .u16⟩,
  ⟨.filledCount, .u16⟩,
  ⟨.sealThreshold, .u8⟩,
  ⟨.sealCount, .u8⟩,
  ⟨.sealedByBitmap, .u8⟩,
  ⟨.phase, .u8⟩,
  ⟨.reservedTail, .reserved 4⟩
]

def layout : List (PlacedField Field) := specialize schema
def headerBytes : Nat := schemaWidth schema

theorem header_width : headerBytes = 312 := by native_decide

theorem coordinates_pinned : coordinates layout = [
    (.magic, 0, 8),
    (.version, 8, 2),
    (.reserved, 10, 2),
    (.market, 12, 32),
    (.generation, 44, 8),
    (.sourceMaterialId, 52, 32),
    (.accountSetId, 84, 32),
    (.providerReleaseId, 116, 32),
    (.relayerKeySetId, 148, 32),
    (.observedClusterId, 180, 32),
    (.observedSlot, 212, 8),
    (.setDigest, 220, 32),
    (.rentCreditBeneficiary, 252, 32),
    (.createdUnixSeconds, 284, 8),
    (.sealedUnixSeconds, 292, 8),
    (.setCount, 300, 2),
    (.filledCount, 302, 2),
    (.sealThreshold, 304, 1),
    (.sealCount, 305, 1),
    (.sealedByBitmap, 306, 1),
    (.phase, 307, 1),
    (.reservedTail, 308, 4)
  ] := by native_decide

theorem well_formed : WellFormed schema := by
  constructor
  · native_decide
  · intro field member
    simp [schema] at member
    rcases member with rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl |
      rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl | rfl <;> decide

theorem disjoint : layout.Pairwise Before := specializeFrom_pairwise 0 schema

/-- One fixed slot: the observation body head plus the release-pinned inline
region, zero-padded beyond `inlineLen`. -/
def slotBytes : Nat := Observation.headBytes + maxInlineBytes

theorem slot_stride : slotBytes = 560 := by native_decide

/-- The record is runtime-width in `setCount`, following the runtime-width
Source resolution state rather than the fixed-width shared-observation child.
An exact-length decoder is strictly more hostile than a padded fixed one, and
a four-account set stops paying for four unused slots. -/
def recordBytes (setCount : Nat) : Nat := headerBytes + setCount * slotBytes

theorem widest_record : recordBytes maxAccounts = 4792 := by native_decide
theorem dbc_record : recordBytes 4 = 2552 := by native_decide

namespace Field
def rustName : Field → String
  | .magic => "RELAYED_RECORD_MAGIC_OFFSET"
  | .version => "RELAYED_RECORD_VERSION_OFFSET"
  | .reserved => "RELAYED_RECORD_RESERVED_OFFSET"
  | .market => "RELAYED_RECORD_MARKET_OFFSET"
  | .generation => "RELAYED_RECORD_GENERATION_OFFSET"
  | .sourceMaterialId => "RELAYED_RECORD_SOURCE_MATERIAL_ID_OFFSET"
  | .accountSetId => "RELAYED_RECORD_ACCOUNT_SET_ID_OFFSET"
  | .providerReleaseId => "RELAYED_RECORD_PROVIDER_RELEASE_ID_OFFSET"
  | .relayerKeySetId => "RELAYED_RECORD_RELAYER_KEY_SET_ID_OFFSET"
  | .observedClusterId => "RELAYED_RECORD_OBSERVED_CLUSTER_ID_OFFSET"
  | .observedSlot => "RELAYED_RECORD_OBSERVED_SLOT_OFFSET"
  | .setDigest => "RELAYED_RECORD_SET_DIGEST_OFFSET"
  | .rentCreditBeneficiary => "RELAYED_RECORD_RENT_CREDIT_BENEFICIARY_OFFSET"
  | .createdUnixSeconds => "RELAYED_RECORD_CREATED_UNIX_SECONDS_OFFSET"
  | .sealedUnixSeconds => "RELAYED_RECORD_SEALED_UNIX_SECONDS_OFFSET"
  | .setCount => "RELAYED_RECORD_SET_COUNT_OFFSET"
  | .filledCount => "RELAYED_RECORD_FILLED_COUNT_OFFSET"
  | .sealThreshold => "RELAYED_RECORD_SEAL_THRESHOLD_OFFSET"
  | .sealCount => "RELAYED_RECORD_SEAL_COUNT_OFFSET"
  | .sealedByBitmap => "RELAYED_RECORD_SEALED_BY_BITMAP_OFFSET"
  | .phase => "RELAYED_RECORD_PHASE_OFFSET"
  | .reservedTail => "RELAYED_RECORD_RESERVED_TAIL_OFFSET"

def offset (field : Field) : Nat :=
  (coordinate? field layout).map (fun value => value.1) |>.getD 0
end Field

/-- Admitted lifecycle transitions.  Appends fill strictly increasing indices,
so a repeat is a replay refusal rather than an overwrite; only the transition
into `sealed` may stamp `sealedUnixSeconds`; only a `sealed` record may be
consumed; retirement may close a record from any live phase. -/
def admitsTransition : Phase → Phase → Bool
  | .collecting, .collecting => true
  | .collecting, .sealed => true
  | .collecting, .retired => true
  | .sealed, .sealed => true
  | .sealed, .consumed => true
  | .sealed, .retired => true
  | .consumed, .retired => true
  | _, _ => false

/-- A consumed or retired record is terminal for observation purposes: nothing
reopens it, and in particular no append may follow a seal. -/
theorem no_append_after_seal : admitsTransition .sealed .collecting = false := by native_decide
theorem retirement_is_terminal :
    ([Phase.collecting, .sealed, .consumed, .retired].all
      fun target => !admitsTransition .retired target) := by native_decide
theorem consumption_requires_a_seal :
    admitsTransition .collecting .consumed = false := by native_decide

end Record

/-! ## Ed25519 transport geometry

The signature primitive is the native Ed25519 precompile, verified against the
transaction's top-level instruction list before any program runs.  These are the
numbers the packet arithmetic of §4.4 rests on; they are pinned here so the
Rust side compares against a generated constant rather than a literal. -/

/-- `SIGNATURE_OFFSETS_SERIALIZED_SIZE`, `solana-ed25519-program-3.0.0`. -/
def ed25519DescriptorBytes : Nat := 14
/-- `SIGNATURE_OFFSETS_START`. -/
def ed25519DescriptorStart : Nat := 2
def ed25519PublicKeyBytes : Nat := 32
def ed25519SignatureBytes : Nat := 64
/-- `SOLANA_PACKET_DATA_SIZE_3_0`, already pinned in `dclutch-direct-contract`. -/
def packetDataSize : Nat := 1232

/-- An m-signature precompile instruction is exactly `2 + 110m` data bytes. -/
def ed25519InstructionBytes (signatures : Nat) : Nat :=
  ed25519DescriptorStart +
    signatures * (ed25519DescriptorBytes + ed25519PublicKeyBytes + ed25519SignatureBytes)

theorem one_signature_instruction : ed25519InstructionBytes 1 = 112 := by native_decide

/-- §4.4's measured-frame budget: 425 bytes of fixed transaction overhead for a
v0 transaction over an address lookup table, leaving 807 for instruction data,
of which the Source-owned 64-byte prefix takes its share.  *provisional frame
arithmetic* — the campaign must re-derive it from a real frame. -/
def provisionalFixedFrameBytes : Nat := 425
def sourceAcceptPrefixBytes : Nat := 64

def provisionalMessageBudget : Nat :=
  packetDataSize - provisionalFixedFrameBytes - sourceAcceptPrefixBytes

theorem provisional_message_budget : provisionalMessageBudget = 743 := by native_decide

/-- The inline ceiling is what is left after the attestation's fixed cost, and
`maxInlineBytes` is set below it with headroom for frame variation. -/
theorem inline_ceiling_has_headroom :
    maxInlineBytes + Attestation.headBytes + Observation.headBytes + 27
      = provisionalMessageBudget := by native_decide

/-- The one-transaction profile is admitted only where the whole set fits.  The
Meteora DBC set does not: 156 + 148 + 157 + 528 + 152 = 1,141 > 743. -/
def oneTransactionSetBytes (inlineLens : List Nat) : Nat :=
  Attestation.headBytes +
    (inlineLens.map (fun value => Observation.headBytes + value)).sum

/-- The Meteora DBC venue account's on-chain data length.  *chain-derived,
verified-from-source and confirmed against live mainnet bytes*: the account type
is `VirtualPool`, whose 416-byte `PoolState` body rides behind an 8-byte Anchor
discriminator, so the admitted length set is the singleton `{424}` — the program
contains no `realloc`, so there is exactly one live width.  The design document
and the chain-state dossier both quote 416, which is `INIT_SPACE` and not the
account length; carrying 416 would truncate the record by eight bytes. -/
def dbcVirtualPoolAccountBytes : Nat := 424

/-- The observable fields — `is_migrated` at body offset 297, `migration_progress`
at 300, `finish_curve_timestamp` at 336 — are prefix-contiguous and end at body
offset 344, i.e. account offset 352.  A release may therefore pin a partial
inline window and let the remainder ride in the tail digest; the relayer still
commits to the complete account either way. -/
def dbcGraduationPrefixBytes : Nat := 352

theorem dbc_graduation_prefix_is_a_prefix :
    dbcGraduationPrefixBytes ≤ dbcVirtualPoolAccountBytes := by native_decide

theorem dbc_graduation_prefix_is_carriable :
    dbcGraduationPrefixBytes ≤ maxInlineBytes := by native_decide

def dbcInlineLens : List Nat := [
  loaderV3ProgramBytes,
  loaderV3ProgramDataMetadataBytes,
  dbcVirtualPoolAccountBytes,
  clockSysvarBytes
]

theorem dbc_set_bytes : oneTransactionSetBytes dbcInlineLens = 1149 := by native_decide

theorem dbc_needs_the_record_profile :
    ¬ (oneTransactionSetBytes dbcInlineLens ≤ provisionalMessageBudget) := by native_decide

/-- Even the tightest honest carriage of the same set — the graduation prefix
instead of the whole account — does not fit one transaction.  The record profile
is not an artefact of carrying redundant bytes. -/
theorem dbc_needs_the_record_profile_even_at_the_prefix :
    ¬ (oneTransactionSetBytes
        [loaderV3ProgramBytes, loaderV3ProgramDataMetadataBytes,
         dbcGraduationPrefixBytes, clockSysvarBytes]
      ≤ provisionalMessageBudget) := by native_decide

/-- A single-account transport gate — the mainnet `Clock` sysvar alone — does
fit the one-transaction profile.  §6.2 orders it first precisely because it
exercises the whole transport with zero venue decoding. -/
theorem clock_only_gate_fits :
    oneTransactionSetBytes [clockSysvarBytes] ≤ provisionalMessageBudget := by native_decide

end DClutch.RelayedMainnetStateV1Abi
