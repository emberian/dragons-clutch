import DClutchSemantics.AbiSchema
import DClutchSemantics.CapabilityExecutionAbi
import DClutchSemantics.TransitionVMV2

/-!
# Data-defined capability-program and Trading child-root ABIs

The descriptor is finalized, account-resident Registry material whose complete
content identity is the capability manifest entry's semantic `release_id`.
The Trading-owned root account has one immutable header embedding the exact
activation projection followed by a descriptor-sized mutable state tail. The
header is not a second semantic owner. The manifest child schema names the
tail, and hot actions authenticate the root rather than carrying the projection
again.
-/

namespace DClutch.CapabilityProgramAbi

open DClutch.AbiSchema

def descriptorMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x50, 0x52, 0x31]
def rootMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x43, 0x52, 0x54, 0x31]
def schemaVersion : Nat := 1
def descriptorArtifactProfile : Nat := 2
def rootArtifactProfile : Nat := 1

inductive DescriptorField where
  | magic | schemaVersion | artifactProfile | reserved
  | kind | configSchema | requestSchema | rootSchema | accountProfile
  | derivationPolicy | capacityProfile | effectSchema | rootStateBytes
  | bodyReserved
  deriving DecidableEq, Repr

def descriptorSchema : List (FieldSpec DescriptorField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.kind, .bytes 32⟩,
  ⟨.configSchema, .bytes 32⟩,
  ⟨.requestSchema, .bytes 32⟩,
  ⟨.rootSchema, .bytes 32⟩,
  ⟨.accountProfile, .bytes 32⟩,
  ⟨.derivationPolicy, .bytes 32⟩,
  ⟨.capacityProfile, .bytes 32⟩,
  ⟨.effectSchema, .bytes 32⟩,
  ⟨.rootStateBytes, .u32⟩,
  ⟨.bodyReserved, .reserved 4⟩
]

def descriptorLayout : List (PlacedField DescriptorField) := specialize descriptorSchema
def descriptorHeaderBytes : Nat := schemaWidth descriptorSchema

/-- Maximum bytes in one finalized, account-resident Registry record.

This bound is a **measured-profile coordinate with a named floor and no
physical ceiling**, not a derivation from a chain constant. Every constraint a
finalized record actually faces, measured 2026-08-31:

* **Account data — not the wall.** The record is one Registry raw-record PDA
  allocated at its exact content length by a single `create_account`
  (`programs/dclutch-registry-sbf/src/record_v1.rs:259-266,876-923`) and never
  grown, so the 10,240-byte per-instruction realloc cap does not apply. The
  system ceiling is 10 MiB (`SOLANA_MAX_PERMITTED_ACCOUNT_DATA_BYTES`,
  `crates/dclutch-release-tool/src/lib.rs:40`) — four orders of magnitude away.
* **Packet — not the wall.** Record content never rides in one transaction. It
  is staged `Begin → N × Append → Finalize` at `CANONICAL_RECORD_PAGE_BYTES_V1
  = 768` semantic bytes per Append (`crates/dclutch-record-contract/src/lib.rs:36`),
  an 808-byte instruction well inside the 1232-byte packet, and `page_count`
  is unbounded (`:380-392`). Record width costs Append *transactions*, not
  packet bytes.
* **Rent — a gradient, not a ceiling.** `(128 + bytes) × 6960` lamports, linear
  and unbudgeted; no rent ceiling on record width exists in the tree.
  `CAPABILITY_PROGRAM_MAX_RENT_LAMPORTS_V1 = 9966720` is a derived fact checked
  in a test, not a gate.
* **Already exceeded in-tree, on this same allocator.** `CapabilityProgramSetV2`
  (2336) and `CapabilityManifestV1` (8464) are the same Registry raw-record
  class published through the same path — 6.45× this bound.

**The floor is real and close.** The widest shipped consumer is the Direct
inline ordinary route: its RequestProfile V1 is 50 operations = 1232 bytes and
its signed V2 wrapper is 1272 (`crates/dclutch-direct-codec/src/ordinary_artifacts_v3.rs:52-60`,
asserted `:535-536`). That leaves **40 bytes** — under two 24-byte operations —
of headroom on the flagship route. The bound must not fall, and the next
operation added to that route exhausts it.

**Why it has not been raised.** Widening does not move the cliff this literal
is blamed for. The Structured/Rational coordinate ceiling `K = 3` does derive
from this bound (`29 + 8K` operations at 24 bytes), but a cluster cannot carry
the actions that spend it: `IssueStructured`/`UnwrapStructured` at `K = 3`
measure **1357 bytes** as a v0 message with the Address Lookup Table already
applied, against the 1232-byte packet limit, and the packet caps full-width
issuance at `K = 2` — one coordinate *below* this bound's ceiling
(`programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs:3471`,
which derives the 2 rather than asserting it). Raising this literal would admit
descriptors that can be published and denominated but never issued or
unwrapped. The Structured cliff is a packet problem; it wants the
commit-don't-inline treatment (`docs/design/CLIFF_DOCTRINE_V1.md` §4 rank 1) or
a staged issuance, not a wider record.

So the bound is PURCHASABLE in the doctrine's sense — its price is Append
transactions plus 6960 lamports per byte — and it stays at its current value
until a lift has a consumer whose route fits. Raise it when that consumer
exists, not to move a number that a second wall already binds lower. -/
def finalizedRecordMaxBytes : Nat := 1312
def transitionMaxInstructions : Nat :=
  (finalizedRecordMaxBytes - descriptorHeaderBytes - TransitionVMV2.Codec.headerBytes) /
    TransitionVMV2.Codec.instructionBytes
def transitionMaxBytes : Nat := TransitionVMV2.Codec.headerBytes +
  transitionMaxInstructions * TransitionVMV2.Codec.instructionBytes
def descriptorMaxBytes : Nat := descriptorHeaderBytes +
  transitionMaxBytes
def rootStateMaxBytes : Nat := 4096

namespace DescriptorField

def rustName : DescriptorField → String
  | .magic => "CAPABILITY_PROGRAM_MAGIC_OFFSET"
  | .schemaVersion => "CAPABILITY_PROGRAM_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "CAPABILITY_PROGRAM_PROFILE_OFFSET"
  | .reserved => "CAPABILITY_PROGRAM_RESERVED_OFFSET"
  | .kind => "CAPABILITY_PROGRAM_KIND_OFFSET"
  | .configSchema => "CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET"
  | .requestSchema => "CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET"
  | .rootSchema => "CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET"
  | .accountProfile => "CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET"
  | .derivationPolicy => "CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET"
  | .capacityProfile => "CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET"
  | .effectSchema => "CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET"
  | .rootStateBytes => "CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET"
  | .bodyReserved => "CAPABILITY_PROGRAM_BODY_RESERVED_OFFSET"

end DescriptorField

inductive RootField where
  | magic | schemaVersion | artifactProfile | reserved
  | releaseSet | market | generation | selection
  deriving DecidableEq, Repr

def rootHeaderSchema : List (FieldSpec RootField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.releaseSet, .bytes 32⟩,
  ⟨.market, .bytes 32⟩,
  ⟨.generation, .u64⟩,
  ⟨.selection, .bytes CapabilityExecutionAbi.bytes⟩
]

def rootHeaderLayout : List (PlacedField RootField) := specialize rootHeaderSchema
def rootHeaderBytes : Nat := schemaWidth rootHeaderSchema
def rootAccountMaxBytes : Nat := rootHeaderBytes + rootStateMaxBytes

namespace RootField

def rustName : RootField → String
  | .magic => "CAPABILITY_ROOT_MAGIC_OFFSET"
  | .schemaVersion => "CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET"
  | .artifactProfile => "CAPABILITY_ROOT_PROFILE_OFFSET"
  | .reserved => "CAPABILITY_ROOT_RESERVED_OFFSET"
  | .releaseSet => "CAPABILITY_ROOT_RELEASE_SET_OFFSET"
  | .market => "CAPABILITY_ROOT_MARKET_OFFSET"
  | .generation => "CAPABILITY_ROOT_GENERATION_OFFSET"
  | .selection => "CAPABILITY_ROOT_SELECTION_OFFSET"

end RootField

theorem descriptor_header_width_is_exact : descriptorHeaderBytes = 280 := by native_decide
theorem transition_max_instructions_is_exact : transitionMaxInstructions = 42 := by native_decide
theorem transition_max_width_is_exact : transitionMaxBytes = 1024 := by native_decide
theorem descriptor_max_width_is_exact : descriptorMaxBytes = 1304 := by native_decide
theorem descriptor_fits_finalized_record_bound :
    descriptorMaxBytes ≤ finalizedRecordMaxBytes := by native_decide
theorem descriptor_names_are_unique :
    (descriptorSchema.map fun field => field.name).Nodup := by native_decide
theorem descriptor_fields_are_disjoint :
    descriptorLayout.Pairwise Before := specializeFrom_pairwise 0 descriptorSchema
theorem root_header_width_is_exact : rootHeaderBytes = 232 := by native_decide
theorem root_account_max_width_is_exact : rootAccountMaxBytes = 4328 := by native_decide
theorem root_names_are_unique :
    (rootHeaderSchema.map fun field => field.name).Nodup := by native_decide
theorem root_fields_are_disjoint :
    rootHeaderLayout.Pairwise Before := specializeFrom_pairwise 0 rootHeaderSchema

end DClutch.CapabilityProgramAbi
