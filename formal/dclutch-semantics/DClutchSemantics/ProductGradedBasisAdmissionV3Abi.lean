import DClutchSemantics.AbiSchema
import DClutchSemantics.Codec
import Std.Tactic

/-!
# Product graded-basis Registry admission V3 ABI

The runtime basis and projection certificate are independently finalized raw
records.  The admission record joins their exact raw digests to the Product
Runtime V2 result domain and to the compiler/toolchain evidence.  It does not
reinterpret the semantic basis as an LBV2 or EconomicSlice identity.
-/

namespace DClutch.ProductGradedBasisAdmissionV3Abi

open DClutch
open DClutch.AbiSchema

def schemaVersion : Nat := 3

def basisSchemaPreimage : List UInt8 :=
  "dclutch/schema/product-runtime-graded-basis-v3".toUTF8.toList
def basisSchemaId : List UInt8 := [
  0x0b, 0x68, 0xd1, 0xa8, 0x31, 0xe9, 0xee, 0x4b,
  0x7c, 0x7f, 0x04, 0x7f, 0x69, 0x82, 0xea, 0xde,
  0x18, 0x01, 0xc7, 0x59, 0x68, 0xcd, 0x2e, 0x49,
  0x8b, 0x76, 0x62, 0xe7, 0xee, 0x62, 0xd0, 0xe8
]
def certificateSchemaPreimage : List UInt8 :=
  "dclutch/schema/product-runtime-graded-projection-certificate-v3".toUTF8.toList
def certificateSchemaId : List UInt8 := [
  0x9e, 0x01, 0x00, 0x7b, 0xa4, 0x3c, 0x60, 0x1d,
  0x4c, 0xc8, 0x38, 0xe0, 0x60, 0x69, 0x4a, 0x64,
  0xdd, 0x6b, 0x6d, 0xda, 0x9c, 0x47, 0x1e, 0xab,
  0xd6, 0x60, 0x13, 0x4f, 0xa0, 0xd6, 0x34, 0xb3
]
def admissionSchemaPreimage : List UInt8 :=
  "dclutch/schema/product-runtime-graded-basis-admission-v3".toUTF8.toList
def admissionSchemaId : List UInt8 := [
  0x03, 0xcd, 0x32, 0xf6, 0xee, 0x40, 0x50, 0xa6,
  0x9d, 0x6b, 0x0a, 0x46, 0x16, 0xcc, 0x2a, 0x67,
  0x23, 0x0e, 0x09, 0x0e, 0x38, 0x5b, 0x25, 0xca,
  0x86, 0xb5, 0xe0, 0xea, 0xbb, 0x21, 0xe4, 0xe0
]

def certificateMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x41, 0x50, 0x58, 0x33]
def admissionMagic : List UInt8 := [0x44, 0x43, 0x4c, 0x54, 0x47, 0x41, 0x44, 0x33]

inductive CertificateField where
  | magic | version | boundary | headerReserved | basisWidth | outcomeCount
  | payoutScale | maxComponentError | product | resultDomain | semanticBasis
  | linkedBasis | evaluatorRelease | projectionDigest | tailReserved
  deriving DecidableEq, Repr

def certificateSchema : List (FieldSpec CertificateField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.boundary, .u8⟩,
  ⟨.headerReserved, .reserved 5⟩,
  ⟨.basisWidth, .u32⟩,
  ⟨.outcomeCount, .u32⟩,
  ⟨.payoutScale, .u64⟩,
  ⟨.maxComponentError, .u64⟩,
  ⟨.product, .bytes 32⟩,
  ⟨.resultDomain, .bytes 32⟩,
  ⟨.semanticBasis, .bytes 32⟩,
  ⟨.linkedBasis, .bytes 32⟩,
  ⟨.evaluatorRelease, .bytes 32⟩,
  ⟨.projectionDigest, .bytes 32⟩,
  ⟨.tailReserved, .reserved 24⟩
]

def certificateLayout : List (PlacedField CertificateField) := specialize certificateSchema
def certificateBytes : Nat := schemaWidth certificateSchema

namespace CertificateField

def rustName : CertificateField → String
  | .magic => "APPROXIMATION_CERTIFICATE_MAGIC_OFFSET_V3"
  | .version => "APPROXIMATION_CERTIFICATE_VERSION_OFFSET_V3"
  | .boundary => "APPROXIMATION_CERTIFICATE_BOUNDARY_OFFSET_V3"
  | .headerReserved => "APPROXIMATION_CERTIFICATE_HEADER_RESERVED_OFFSET_V3"
  | .basisWidth => "APPROXIMATION_CERTIFICATE_BASIS_WIDTH_OFFSET_V3"
  | .outcomeCount => "APPROXIMATION_CERTIFICATE_OUTCOME_COUNT_OFFSET_V3"
  | .payoutScale => "APPROXIMATION_CERTIFICATE_PAYOUT_SCALE_OFFSET_V3"
  | .maxComponentError => "APPROXIMATION_CERTIFICATE_MAX_ERROR_OFFSET_V3"
  | .product => "APPROXIMATION_CERTIFICATE_PRODUCT_OFFSET_V3"
  | .resultDomain => "APPROXIMATION_CERTIFICATE_RESULT_DOMAIN_OFFSET_V3"
  | .semanticBasis => "APPROXIMATION_CERTIFICATE_SEMANTIC_BASIS_OFFSET_V3"
  | .linkedBasis => "APPROXIMATION_CERTIFICATE_LINKED_BASIS_OFFSET_V3"
  | .evaluatorRelease => "APPROXIMATION_CERTIFICATE_EVALUATOR_RELEASE_OFFSET_V3"
  | .projectionDigest => "APPROXIMATION_CERTIFICATE_PROJECTION_DIGEST_OFFSET_V3"
  | .tailReserved => "APPROXIMATION_CERTIFICATE_TAIL_RESERVED_OFFSET_V3"

end CertificateField

inductive AdmissionField where
  | magic | version | reserved | resultDomain | product | coordinateDomain
  | resultUnit | semanticBasis | linkedBasis | compilerRelease | toolchain
  | certificateDigest
  deriving DecidableEq, Repr

def admissionSchema : List (FieldSpec AdmissionField) := [
  ⟨.magic, .bytes 8⟩,
  ⟨.version, .u16⟩,
  ⟨.reserved, .reserved 6⟩,
  ⟨.resultDomain, .bytes 32⟩,
  ⟨.product, .bytes 32⟩,
  ⟨.coordinateDomain, .bytes 32⟩,
  ⟨.resultUnit, .bytes 32⟩,
  ⟨.semanticBasis, .bytes 32⟩,
  ⟨.linkedBasis, .bytes 32⟩,
  ⟨.compilerRelease, .bytes 32⟩,
  ⟨.toolchain, .bytes 32⟩,
  ⟨.certificateDigest, .bytes 32⟩
]

def admissionLayout : List (PlacedField AdmissionField) := specialize admissionSchema
def admissionBytes : Nat := schemaWidth admissionSchema

namespace AdmissionField

def rustName : AdmissionField → String
  | .magic => "GRADED_BASIS_ADMISSION_MAGIC_OFFSET_V3"
  | .version => "GRADED_BASIS_ADMISSION_VERSION_OFFSET_V3"
  | .reserved => "GRADED_BASIS_ADMISSION_RESERVED_OFFSET_V3"
  | .resultDomain => "GRADED_BASIS_ADMISSION_RESULT_DOMAIN_OFFSET_V3"
  | .product => "GRADED_BASIS_ADMISSION_PRODUCT_OFFSET_V3"
  | .coordinateDomain => "GRADED_BASIS_ADMISSION_COORDINATE_DOMAIN_OFFSET_V3"
  | .resultUnit => "GRADED_BASIS_ADMISSION_RESULT_UNIT_OFFSET_V3"
  | .semanticBasis => "GRADED_BASIS_ADMISSION_SEMANTIC_BASIS_OFFSET_V3"
  | .linkedBasis => "GRADED_BASIS_ADMISSION_LINKED_BASIS_OFFSET_V3"
  | .compilerRelease => "GRADED_BASIS_ADMISSION_COMPILER_RELEASE_OFFSET_V3"
  | .toolchain => "GRADED_BASIS_ADMISSION_TOOLCHAIN_OFFSET_V3"
  | .certificateDigest => "GRADED_BASIS_ADMISSION_CERTIFICATE_DIGEST_OFFSET_V3"

end AdmissionField

theorem certificate_width_is_exact : certificateBytes = 256 := by native_decide
theorem admission_width_is_exact : admissionBytes = 304 := by native_decide
theorem certificate_layout_is_byte_disjoint : certificateLayout.Pairwise Before :=
  specializeFrom_pairwise 0 certificateSchema
theorem admission_layout_is_byte_disjoint : admissionLayout.Pairwise Before :=
  specializeFrom_pairwise 0 admissionSchema

structure Admission where
  resultDomain : Nat
  product : Nat
  coordinateDomain : Nat
  resultUnit : Nat
  semanticBasis : Nat
  linkedBasis : Nat
  compilerRelease : Nat
  toolchain : Nat
  certificateDigest : Nat
  deriving DecidableEq, Repr

structure AuthenticatedFacts where
  resultDomain : Nat
  product : Nat
  coordinateDomain : Nat
  resultUnit : Nat
  semanticBasis : Nat
  linkedBasis : Nat
  compilerRelease : Nat
  toolchain : Nat
  certificateDigest : Nat
  deriving DecidableEq, Repr

def Admission.admits (record : Admission) (facts : AuthenticatedFacts) : Bool :=
  record.resultDomain != 0 && record.resultDomain = facts.resultDomain &&
  record.product != 0 && record.product = facts.product &&
  record.coordinateDomain != 0 && record.coordinateDomain = facts.coordinateDomain &&
  record.resultUnit != 0 && record.resultUnit = facts.resultUnit &&
  record.semanticBasis != 0 && record.semanticBasis = facts.semanticBasis &&
  record.linkedBasis != 0 && record.linkedBasis = facts.linkedBasis &&
  record.compilerRelease != 0 && record.compilerRelease = facts.compilerRelease &&
  record.toolchain != 0 && record.toolchain = facts.toolchain &&
  record.certificateDigest != 0 && record.certificateDigest = facts.certificateDigest

theorem substituted_result_domain_refuses
    (record : Admission) (facts : AuthenticatedFacts)
    (substituted : record.resultDomain ≠ facts.resultDomain) :
    record.admits facts = false := by
  simp [Admission.admits, substituted]

theorem substituted_raw_basis_refuses
    (record : Admission) (facts : AuthenticatedFacts)
    (substituted : record.linkedBasis ≠ facts.linkedBasis) :
    record.admits facts = false := by
  simp [Admission.admits, substituted]

theorem substituted_compiler_refuses
    (record : Admission) (facts : AuthenticatedFacts)
    (substituted : record.compilerRelease ≠ facts.compilerRelease) :
    record.admits facts = false := by
  simp [Admission.admits, substituted]

theorem substituted_certificate_refuses
    (record : Admission) (facts : AuthenticatedFacts)
    (substituted : record.certificateDigest ≠ facts.certificateDigest) :
    record.admits facts = false := by
  simp [Admission.admits, substituted]

end DClutch.ProductGradedBasisAdmissionV3Abi
