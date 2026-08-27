import DClutchSemantics.AbiSchema

/-!
# Execution Strategy V2 ABI

The strategy graph is deliberately acyclic:

`CapabilityProgramV3 -> Strategy -> {TransitionVM, Certificate, Admission}`,
`Admission -> Certificate`, and `Certificate -> ArtifactRelease`.

The Strategy also selects the underlying TransitionVM program.  A Certificate
contains only the semantic equivalence tuple that the adapter joins to the
authenticated descriptor and Strategy; it never points back to either parent.
Admission is the minimal Registry-owned authorization of one exact Certificate
for admitted-AOT execution.
-/

namespace DClutch.ExecutionStrategyV2Abi

open DClutch.AbiSchema

def strategyMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x53, 0x54, 0x47, 0x32]
def certificateMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x45, 0x53, 0x43, 0x32]
def admissionMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x45, 0x53, 0x41, 0x32]
def requestMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x41, 0x49, 0x52, 0x32]
def ackMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x41, 0x41, 0x4b, 0x32]
def scratchMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x53, 0x50, 0x47, 0x32]

def schemaVersion : Nat := 2
def artifactProfile : Nat := 2
def returnDataBytes : Nat := 1024
def chunkAckHeaderBytes : Nat := 144
def chunkPayloadBytes : Nat := returnDataBytes - chunkAckHeaderBytes

inductive StrategyField where
  | magic | schemaVersion | artifactProfile | disposition
  | certificatePresent | admissionPresent | headerReserved
  | transitionSchema | transitionProgram
  | certificateSchema | certificateProgram
  | admissionSchema | admissionProgram
  | requestSchema | ackSchema
  deriving DecidableEq, Repr

def strategySchema : List (FieldSpec StrategyField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.disposition, .u8⟩,
  ⟨.certificatePresent, .u8⟩,
  ⟨.admissionPresent, .u8⟩,
  ⟨.headerReserved, .reserved 1⟩,
  ⟨.transitionSchema, .bytes 32⟩,
  ⟨.transitionProgram, .bytes 32⟩,
  ⟨.certificateSchema, .bytes 32⟩,
  ⟨.certificateProgram, .bytes 32⟩,
  ⟨.admissionSchema, .bytes 32⟩,
  ⟨.admissionProgram, .bytes 32⟩,
  ⟨.requestSchema, .bytes 32⟩,
  ⟨.ackSchema, .bytes 32⟩
]

def strategyLayout : List (PlacedField StrategyField) := specialize strategySchema
def strategyBytes : Nat := schemaWidth strategySchema

namespace StrategyField

def rustName : StrategyField → String
  | .magic => "STRATEGY_MAGIC_OFFSET_V2"
  | .schemaVersion => "STRATEGY_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "STRATEGY_ARTIFACT_PROFILE_OFFSET_V2"
  | .disposition => "STRATEGY_DISPOSITION_OFFSET_V2"
  | .certificatePresent => "STRATEGY_CERTIFICATE_PRESENT_OFFSET_V2"
  | .admissionPresent => "STRATEGY_ADMISSION_PRESENT_OFFSET_V2"
  | .headerReserved => "STRATEGY_HEADER_RESERVED_OFFSET_V2"
  | .transitionSchema => "STRATEGY_TRANSITION_SCHEMA_OFFSET_V2"
  | .transitionProgram => "STRATEGY_TRANSITION_PROGRAM_OFFSET_V2"
  | .certificateSchema => "STRATEGY_CERTIFICATE_SCHEMA_OFFSET_V2"
  | .certificateProgram => "STRATEGY_CERTIFICATE_PROGRAM_OFFSET_V2"
  | .admissionSchema => "STRATEGY_ADMISSION_SCHEMA_OFFSET_V2"
  | .admissionProgram => "STRATEGY_ADMISSION_PROGRAM_OFFSET_V2"
  | .requestSchema => "STRATEGY_REQUEST_SCHEMA_OFFSET_V2"
  | .ackSchema => "STRATEGY_ACK_SCHEMA_OFFSET_V2"

end StrategyField

inductive CertificateField where
  | magic | schemaVersion | artifactProfile | reserved
  | accountProfileProgram
  | requestProfileSchema | requestProfileProgram
  | transitionSchema | transitionProgram | effectProgram
  | artifactRelease | compilerRelease | toolchain | translationValidation
  deriving DecidableEq, Repr

def certificateSchema : List (FieldSpec CertificateField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.reserved, .reserved 4⟩,
  ⟨.accountProfileProgram, .bytes 32⟩,
  ⟨.requestProfileSchema, .bytes 32⟩,
  ⟨.requestProfileProgram, .bytes 32⟩,
  ⟨.transitionSchema, .bytes 32⟩,
  ⟨.transitionProgram, .bytes 32⟩,
  ⟨.effectProgram, .bytes 32⟩,
  ⟨.artifactRelease, .bytes 32⟩,
  ⟨.compilerRelease, .bytes 32⟩,
  ⟨.toolchain, .bytes 32⟩,
  ⟨.translationValidation, .bytes 32⟩
]

def certificateLayout : List (PlacedField CertificateField) := specialize certificateSchema
def certificateBytes : Nat := schemaWidth certificateSchema

namespace CertificateField

def rustName : CertificateField → String
  | .magic => "CERTIFICATE_MAGIC_OFFSET_V2"
  | .schemaVersion => "CERTIFICATE_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "CERTIFICATE_ARTIFACT_PROFILE_OFFSET_V2"
  | .reserved => "CERTIFICATE_RESERVED_OFFSET_V2"
  | .accountProfileProgram => "CERTIFICATE_ACCOUNT_PROFILE_PROGRAM_OFFSET_V2"
  | .requestProfileSchema => "CERTIFICATE_REQUEST_PROFILE_SCHEMA_OFFSET_V2"
  | .requestProfileProgram => "CERTIFICATE_REQUEST_PROFILE_PROGRAM_OFFSET_V2"
  | .transitionSchema => "CERTIFICATE_TRANSITION_SCHEMA_OFFSET_V2"
  | .transitionProgram => "CERTIFICATE_TRANSITION_PROGRAM_OFFSET_V2"
  | .effectProgram => "CERTIFICATE_EFFECT_PROGRAM_OFFSET_V2"
  | .artifactRelease => "CERTIFICATE_ARTIFACT_RELEASE_OFFSET_V2"
  | .compilerRelease => "CERTIFICATE_COMPILER_RELEASE_OFFSET_V2"
  | .toolchain => "CERTIFICATE_TOOLCHAIN_OFFSET_V2"
  | .translationValidation => "CERTIFICATE_TRANSLATION_VALIDATION_OFFSET_V2"

end CertificateField

inductive AdmissionField where
  | magic | schemaVersion | artifactProfile | disposition | reserved
  | certificateProgram
  deriving DecidableEq, Repr

def admissionSchema : List (FieldSpec AdmissionField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.disposition, .u8⟩,
  ⟨.reserved, .reserved 3⟩,
  ⟨.certificateProgram, .bytes 32⟩
]

def admissionLayout : List (PlacedField AdmissionField) := specialize admissionSchema
def admissionBytes : Nat := schemaWidth admissionSchema

namespace AdmissionField

def rustName : AdmissionField → String
  | .magic => "ADMISSION_MAGIC_OFFSET_V2"
  | .schemaVersion => "ADMISSION_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "ADMISSION_ARTIFACT_PROFILE_OFFSET_V2"
  | .disposition => "ADMISSION_DISPOSITION_OFFSET_V2"
  | .reserved => "ADMISSION_RESERVED_OFFSET_V2"
  | .certificateProgram => "ADMISSION_CERTIFICATE_PROGRAM_OFFSET_V2"

end AdmissionField

inductive RequestField where
  | magic | schemaVersion | artifactProfile | transport | headerReserved
  | strategyProgram | certificateProgram | capabilityProgram
  | invocationContext | inputBankDigest
  | tailCount | scalarCount | identityCount
  | chunkIndex | chunkCount | chunkOffset | totalBankBytes | tailReserved
  deriving DecidableEq, Repr

def requestSchema : List (FieldSpec RequestField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.transport, .u8⟩,
  ⟨.headerReserved, .reserved 3⟩,
  ⟨.strategyProgram, .bytes 32⟩,
  ⟨.certificateProgram, .bytes 32⟩,
  ⟨.capabilityProgram, .bytes 32⟩,
  ⟨.invocationContext, .bytes 32⟩,
  ⟨.inputBankDigest, .bytes 32⟩,
  ⟨.tailCount, .u32⟩,
  ⟨.scalarCount, .u32⟩,
  ⟨.identityCount, .u32⟩,
  ⟨.chunkIndex, .u32⟩,
  ⟨.chunkCount, .u32⟩,
  ⟨.chunkOffset, .u64⟩,
  ⟨.totalBankBytes, .u64⟩,
  ⟨.tailReserved, .reserved 12⟩
]

def requestLayout : List (PlacedField RequestField) := specialize requestSchema
def requestHeaderBytes : Nat := schemaWidth requestSchema

namespace RequestField

def rustName : RequestField → String
  | .magic => "REQUEST_MAGIC_OFFSET_V2"
  | .schemaVersion => "REQUEST_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "REQUEST_ARTIFACT_PROFILE_OFFSET_V2"
  | .transport => "REQUEST_TRANSPORT_OFFSET_V2"
  | .headerReserved => "REQUEST_HEADER_RESERVED_OFFSET_V2"
  | .strategyProgram => "REQUEST_STRATEGY_PROGRAM_OFFSET_V2"
  | .certificateProgram => "REQUEST_CERTIFICATE_PROGRAM_OFFSET_V2"
  | .capabilityProgram => "REQUEST_CAPABILITY_PROGRAM_OFFSET_V2"
  | .invocationContext => "REQUEST_INVOCATION_CONTEXT_OFFSET_V2"
  | .inputBankDigest => "REQUEST_INPUT_BANK_DIGEST_OFFSET_V2"
  | .tailCount => "REQUEST_TAIL_COUNT_OFFSET_V2"
  | .scalarCount => "REQUEST_SCALAR_COUNT_OFFSET_V2"
  | .identityCount => "REQUEST_IDENTITY_COUNT_OFFSET_V2"
  | .chunkIndex => "REQUEST_CHUNK_INDEX_OFFSET_V2"
  | .chunkCount => "REQUEST_CHUNK_COUNT_OFFSET_V2"
  | .chunkOffset => "REQUEST_CHUNK_OFFSET_OFFSET_V2"
  | .totalBankBytes => "REQUEST_TOTAL_BANK_BYTES_OFFSET_V2"
  | .tailReserved => "REQUEST_TAIL_RESERVED_OFFSET_V2"

end RequestField

inductive AckField where
  | magic | schemaVersion | artifactProfile | disposition | headerReserved
  | requestDigest | invocationContext | totalBankDigest
  | totalBankBytes | chunkIndex | chunkCount | chunkOffset
  | payloadBytes | tailReserved
  deriving DecidableEq, Repr

def ackSchema : List (FieldSpec AckField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.disposition, .u8⟩,
  ⟨.headerReserved, .reserved 3⟩,
  ⟨.requestDigest, .bytes 32⟩,
  ⟨.invocationContext, .bytes 32⟩,
  ⟨.totalBankDigest, .bytes 32⟩,
  ⟨.totalBankBytes, .u64⟩,
  ⟨.chunkIndex, .u32⟩,
  ⟨.chunkCount, .u32⟩,
  ⟨.chunkOffset, .u64⟩,
  ⟨.payloadBytes, .u16⟩,
  ⟨.tailReserved, .reserved 6⟩
]

def ackLayout : List (PlacedField AckField) := specialize ackSchema
def ackHeaderBytes : Nat := schemaWidth ackSchema

namespace AckField

def rustName : AckField → String
  | .magic => "ACK_MAGIC_OFFSET_V2"
  | .schemaVersion => "ACK_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "ACK_ARTIFACT_PROFILE_OFFSET_V2"
  | .disposition => "ACK_DISPOSITION_OFFSET_V2"
  | .headerReserved => "ACK_HEADER_RESERVED_OFFSET_V2"
  | .requestDigest => "ACK_REQUEST_DIGEST_OFFSET_V2"
  | .invocationContext => "ACK_INVOCATION_CONTEXT_OFFSET_V2"
  | .totalBankDigest => "ACK_TOTAL_BANK_DIGEST_OFFSET_V2"
  | .totalBankBytes => "ACK_TOTAL_BANK_BYTES_OFFSET_V2"
  | .chunkIndex => "ACK_CHUNK_INDEX_OFFSET_V2"
  | .chunkCount => "ACK_CHUNK_COUNT_OFFSET_V2"
  | .chunkOffset => "ACK_CHUNK_OFFSET_OFFSET_V2"
  | .payloadBytes => "ACK_PAYLOAD_BYTES_OFFSET_V2"
  | .tailReserved => "ACK_TAIL_RESERVED_OFFSET_V2"

end AckField

inductive ScratchField where
  | magic | schemaVersion | artifactProfile | kind | headerReserved
  | tradingProgram | strategyProgram | invocationContext | totalBankDigest
  | tailCount | scalarCount | identityCount
  | chunkIndex | chunkCount | chunkOffset | totalBankBytes
  | payloadBytes | tailReserved
  deriving DecidableEq, Repr

def scratchSchema : List (FieldSpec ScratchField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.schemaVersion, .u16⟩,
  ⟨.artifactProfile, .u16⟩,
  ⟨.kind, .u8⟩,
  ⟨.headerReserved, .reserved 3⟩,
  ⟨.tradingProgram, .bytes 32⟩,
  ⟨.strategyProgram, .bytes 32⟩,
  ⟨.invocationContext, .bytes 32⟩,
  ⟨.totalBankDigest, .bytes 32⟩,
  ⟨.tailCount, .u32⟩,
  ⟨.scalarCount, .u32⟩,
  ⟨.identityCount, .u32⟩,
  ⟨.chunkIndex, .u32⟩,
  ⟨.chunkCount, .u32⟩,
  ⟨.chunkOffset, .u64⟩,
  ⟨.totalBankBytes, .u64⟩,
  ⟨.payloadBytes, .u16⟩,
  ⟨.tailReserved, .reserved 10⟩
]

def scratchLayout : List (PlacedField ScratchField) := specialize scratchSchema
def scratchHeaderBytes : Nat := schemaWidth scratchSchema

namespace ScratchField

def rustName : ScratchField → String
  | .magic => "SCRATCH_MAGIC_OFFSET_V2"
  | .schemaVersion => "SCRATCH_SCHEMA_VERSION_OFFSET_V2"
  | .artifactProfile => "SCRATCH_ARTIFACT_PROFILE_OFFSET_V2"
  | .kind => "SCRATCH_KIND_OFFSET_V2"
  | .headerReserved => "SCRATCH_HEADER_RESERVED_OFFSET_V2"
  | .tradingProgram => "SCRATCH_TRADING_PROGRAM_OFFSET_V2"
  | .strategyProgram => "SCRATCH_STRATEGY_PROGRAM_OFFSET_V2"
  | .invocationContext => "SCRATCH_INVOCATION_CONTEXT_OFFSET_V2"
  | .totalBankDigest => "SCRATCH_TOTAL_BANK_DIGEST_OFFSET_V2"
  | .tailCount => "SCRATCH_TAIL_COUNT_OFFSET_V2"
  | .scalarCount => "SCRATCH_SCALAR_COUNT_OFFSET_V2"
  | .identityCount => "SCRATCH_IDENTITY_COUNT_OFFSET_V2"
  | .chunkIndex => "SCRATCH_CHUNK_INDEX_OFFSET_V2"
  | .chunkCount => "SCRATCH_CHUNK_COUNT_OFFSET_V2"
  | .chunkOffset => "SCRATCH_CHUNK_OFFSET_OFFSET_V2"
  | .totalBankBytes => "SCRATCH_TOTAL_BANK_BYTES_OFFSET_V2"
  | .payloadBytes => "SCRATCH_PAYLOAD_BYTES_OFFSET_V2"
  | .tailReserved => "SCRATCH_TAIL_RESERVED_OFFSET_V2"

end ScratchField

theorem strategy_width_is_exact : strategyBytes = 272 := by native_decide
theorem certificate_width_is_exact : certificateBytes = 336 := by native_decide
theorem admission_width_is_exact : admissionBytes = 48 := by native_decide
theorem request_width_is_exact : requestHeaderBytes = 224 := by native_decide
theorem ack_width_is_exact : ackHeaderBytes = 144 := by native_decide
theorem scratch_width_is_exact : scratchHeaderBytes = 192 := by native_decide
theorem chunk_payload_is_exact : chunkPayloadBytes = 880 := by native_decide

theorem strategy_fields_disjoint : strategyLayout.Pairwise Before :=
  specializeFrom_pairwise 0 strategySchema
theorem certificate_fields_disjoint : certificateLayout.Pairwise Before :=
  specializeFrom_pairwise 0 certificateSchema
theorem admission_fields_disjoint : admissionLayout.Pairwise Before :=
  specializeFrom_pairwise 0 admissionSchema
theorem request_fields_disjoint : requestLayout.Pairwise Before :=
  specializeFrom_pairwise 0 requestSchema
theorem ack_fields_disjoint : ackLayout.Pairwise Before :=
  specializeFrom_pairwise 0 ackSchema
theorem scratch_fields_disjoint : scratchLayout.Pairwise Before :=
  specializeFrom_pairwise 0 scratchSchema

end DClutch.ExecutionStrategyV2Abi
