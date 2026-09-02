import DClutchSemantics.AbiCoverage

/-!
# Product V2 admission: three records and the coordinate they are made of

An admission names three finalized records -- the Product, its result domain,
its portfolio -- and proves they are the ones a Market was founded against.
Three wire records carry that: a Product record, an admission request, and a
reference-only receipt holding one 128-byte coordinate per finalized record.

`crates/dclutch-product-runtime-v2-admission/src/lib.rs` was the author of all
four layouts and it wrote them as seventeen bare constants, six of which were
not constants at all but `offset + 32`, `offset + 64` and `offset + 96` spelled
twice inside `decode_coordinate` and `encode_coordinate`.

Two facts the crate implemented and could not state:

* The Product record and the admission request are the SAME SHAPE.  Their three
  identity coordinates were declared twice under two names --
  `PRODUCT_DOMAIN_DIGEST_OFFSET` beside `REQUEST_DOMAIN_DIGEST_OFFSET`, both
  `48` -- so "they agree" was a coincidence maintained by hand across six
  declarations.  `the_record_and_the_request_are_one_shape` is the statement.
* The receipt spends one byte of the span the other two leave reserved.  All
  three begin with a magic and a version; then the Product record and the
  request reserve six bytes, and the receipt puts its record count in the FIRST
  of those six and reserves the remaining five.  That is why `require_zero`
  reads `(10, 6)` in two places and `(11, 5)` in the third, which looks like
  three unrelated numbers and is one.

The four schema identities are held here as their preimage and their SHA-256,
which is the pattern `SourceMaterialV2Abi` established: Lean does not hash, so
the digest is data and the byte-compare guard is what holds it to the label.
The crate keeps its own hashing test, which is the independent check.
-/

namespace DClutch.ProductAdmissionV2Abi

open DClutch.AbiSchema

/-- Shared admission wire version. -/
def version : Nat := 2

/-- Number of finalized records in one complete admission. -/
def recordCount : Nat := 3

/-- `DCLTPRM2`, `DCLTPRQ2`, `DCLTPRA2`. -/
def productRecordMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x50, 0x52, 0x4d, 0x32]
def requestMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x50, 0x52, 0x51, 0x32]
def receiptMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x50, 0x52, 0x41, 0x32]

/-- Admission-program PDA domain for one exact reference-only receipt. -/
def receiptPdaDomain : String := "dclutch/product-v2/admission"

/-! ## The four schema identities -/

def productRecordSchemaPreimage : String :=
  "dclutch/schema/product-runtime-v2-product-record"
def productRecordSchemaId : List UInt8 := [
  0xd9, 0xc3, 0x9f, 0xb6, 0x0c, 0x7d, 0xb7, 0x79,
  0xa7, 0x84, 0x4d, 0xe7, 0x85, 0x05, 0x73, 0x8a,
  0x58, 0x99, 0x26, 0x4f, 0x86, 0x83, 0xdb, 0x4c,
  0x6a, 0xe6, 0x1c, 0x9e, 0xf0, 0xe3, 0xcf, 0xf8
]

def resultDomainSchemaPreimage : String :=
  "dclutch/schema/product-runtime-v2-result-domain"
def resultDomainSchemaId : List UInt8 := [
  0x39, 0x9c, 0xc5, 0x74, 0x0f, 0x62, 0x1e, 0xa5,
  0xc3, 0x0f, 0x96, 0x0a, 0x14, 0xaf, 0x83, 0x9b,
  0x0b, 0x5c, 0xfd, 0x58, 0xa9, 0x30, 0x5d, 0xcc,
  0x09, 0xc6, 0x1f, 0xd1, 0x67, 0x81, 0xb7, 0xc2
]

def portfolioSchemaPreimage : String :=
  "dclutch/schema/product-runtime-v2-portfolio"
def portfolioSchemaId : List UInt8 := [
  0x76, 0x70, 0x6d, 0xdf, 0x08, 0x91, 0x7b, 0xb3,
  0xdf, 0x08, 0x6b, 0x8c, 0x65, 0x04, 0x92, 0x83,
  0xbb, 0xab, 0x69, 0x75, 0x9c, 0x5b, 0x24, 0xb0,
  0x75, 0x29, 0x7c, 0x47, 0x0f, 0xe3, 0xd6, 0x65
]

def receiptSchemaPreimage : String :=
  "dclutch/schema/product-runtime-v2-admission-receipt"
def receiptSchemaId : List UInt8 := [
  0xb7, 0x24, 0x54, 0x93, 0x39, 0x06, 0xb8, 0xb7,
  0x7f, 0x0d, 0x48, 0xa8, 0xf3, 0x63, 0xf9, 0xd9,
  0x2b, 0xa1, 0xa2, 0x75, 0x34, 0x07, 0xff, 0xed,
  0x39, 0x7e, 0x42, 0x00, 0x14, 0xae, 0xa4, 0x7b
]

/-! ## One finalized record coordinate -/

inductive CoordinateField where
  | schemaId | contentDigest | rawAccount | stagingAccount
  deriving DecidableEq, Repr

def coordinateSchema : List (FieldSpec CoordinateField) := [
  ⟨.schemaId, .bytes 32⟩, ⟨.contentDigest, .bytes 32⟩,
  ⟨.rawAccount, .bytes 32⟩, ⟨.stagingAccount, .bytes 32⟩
]

def coordinateLayout : List (PlacedField CoordinateField) :=
  specialize coordinateSchema
def coordinateBytes : Nat := schemaWidth coordinateSchema

namespace CoordinateField

def all : List CoordinateField :=
  [.schemaId, .contentDigest, .rawAccount, .stagingAccount]

def rustName : CoordinateField → String
  | .schemaId => "RECORD_COORDINATE_SCHEMA_ID_OFFSET_V2"
  | .contentDigest => "RECORD_COORDINATE_CONTENT_DIGEST_OFFSET_V2"
  | .rawAccount => "RECORD_COORDINATE_RAW_ACCOUNT_OFFSET_V2"
  | .stagingAccount => "RECORD_COORDINATE_STAGING_ACCOUNT_OFFSET_V2"

def doc : CoordinateField → String
  | .schemaId => "Schema identity the finalized record must declare."
  | .contentDigest => "Exact content digest of the finalized record."
  | .rawAccount => "Account holding the finalized raw bytes."
  | .stagingAccount => "Account holding the record's staging state."

def offset (field : CoordinateField) : Nat :=
  ((coordinate? field coordinateLayout).getD (0, 0)).1

end CoordinateField

/-! ## The Product record and the admission request -/

inductive BodyField where
  | magic | version | reserved | first | resultDomainDigest | portfolioDigest
  deriving DecidableEq, Repr

/-- One shape, used twice.  The Product record calls its first identity the
Product id and the request calls it the Product digest; both are 32 bytes at
the same coordinate, which is the thing six separate declarations obscured. -/
def bodySchema : List (FieldSpec BodyField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.reserved, .reserved 6⟩,
  ⟨.first, .bytes 32⟩, ⟨.resultDomainDigest, .bytes 32⟩,
  ⟨.portfolioDigest, .bytes 32⟩
]

def bodyLayout : List (PlacedField BodyField) := specialize bodySchema
def bodyBytes : Nat := schemaWidth bodySchema

namespace BodyField

def all : List BodyField :=
  [.magic, .version, .reserved, .first, .resultDomainDigest, .portfolioDigest]

def offset (field : BodyField) : Nat :=
  ((coordinate? field bodyLayout).getD (0, 0)).1
def width (field : BodyField) : Nat :=
  ((coordinate? field bodyLayout).getD (0, 0)).2

end BodyField

/-! ## The reference-only receipt -/

inductive ReceiptField where
  | magic | version | recordCount | reserved
  | product | resultDomain | portfolio
  deriving DecidableEq, Repr

def receiptSchema : List (FieldSpec ReceiptField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.recordCount, .u8⟩,
  ⟨.reserved, .reserved 5⟩,
  ⟨.product, .nested coordinateBytes⟩,
  ⟨.resultDomain, .nested coordinateBytes⟩,
  ⟨.portfolio, .nested coordinateBytes⟩
]

def receiptLayout : List (PlacedField ReceiptField) := specialize receiptSchema
def receiptBytes : Nat := schemaWidth receiptSchema

namespace ReceiptField

def all : List ReceiptField :=
  [.magic, .version, .recordCount, .reserved, .product, .resultDomain,
   .portfolio]

def rustName : ReceiptField → String
  | .magic => "ADMISSION_RECEIPT_MAGIC_OFFSET_V2"
  | .version => "ADMISSION_RECEIPT_VERSION_OFFSET_V2"
  | .recordCount => "ADMISSION_RECEIPT_COUNT_OFFSET_V2"
  | .reserved => "ADMISSION_RECEIPT_RESERVED_OFFSET_V2"
  | .product => "ADMISSION_RECEIPT_RECORDS_OFFSET_V2"
  | .resultDomain => "ADMISSION_RECEIPT_RESULT_DOMAIN_OFFSET_V2"
  | .portfolio => "ADMISSION_RECEIPT_PORTFOLIO_OFFSET_V2"

def doc : ReceiptField → String
  | .magic => "Canonical reference-only receipt magic."
  | .version => "Shared admission wire version, at this record's coordinate."
  | .recordCount => "Exact finalized-record count: the first reserved byte."
  | .reserved => "Canonical-zero remainder of the shared reserved span."
  | .product => "First record coordinate: the Product."
  | .resultDomain => "Second record coordinate: the result domain."
  | .portfolio => "Third record coordinate: the portfolio."

def offset (field : ReceiptField) : Nat :=
  ((coordinate? field receiptLayout).getD (0, 0)).1
def width (field : ReceiptField) : Nat :=
  ((coordinate? field receiptLayout).getD (0, 0)).2

end ReceiptField

/-! ## What the layouts say -/

theorem schemas_well_formed :
    WellFormed coordinateSchema ∧ WellFormed bodySchema ∧
      WellFormed receiptSchema := by
  refine ⟨⟨by native_decide, by native_decide⟩,
    ⟨by native_decide, by native_decide⟩,
    ⟨by native_decide, by native_decide⟩⟩

theorem layouts_disjoint :
    coordinateLayout.Pairwise Before ∧ bodyLayout.Pairwise Before ∧
      receiptLayout.Pairwise Before :=
  ⟨specializeFrom_pairwise 0 coordinateSchema,
   specializeFrom_pairwise 0 bodySchema,
   specializeFrom_pairwise 0 receiptSchema⟩

/-- Each record's fields cover the width its readers allocate. -/
theorem layouts_cover_their_declared_widths :
    (coordinateBytes = 128 ∧ tiles 0 coordinateLayout 128 = true) ∧
      (bodyBytes = 112 ∧ tiles 0 bodyLayout 112 = true) ∧
      (receiptBytes = 400 ∧ tiles 0 receiptLayout 400 = true) := by
  native_decide

theorem coordinate_coordinates_are_canonical :
    coordinates coordinateLayout = [
      (.schemaId, 0, 32), (.contentDigest, 32, 32),
      (.rawAccount, 64, 32), (.stagingAccount, 96, 32)
    ] := by
  native_decide

theorem body_coordinates_are_canonical : coordinates bodyLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.reserved, 10, 6),
    (.first, 16, 32), (.resultDomainDigest, 48, 32),
    (.portfolioDigest, 80, 32)
  ] := by
  native_decide

theorem receipt_coordinates_are_canonical : coordinates receiptLayout = [
    (.magic, 0, 8), (.version, 8, 2), (.recordCount, 10, 1),
    (.reserved, 11, 5), (.product, 16, 128), (.resultDomain, 144, 128),
    (.portfolio, 272, 128)
  ] := by
  native_decide

/-- The Product record and the admission request are one shape.  The Rust
declared their three identity coordinates twice, under `PRODUCT_*` and
`REQUEST_*` names, and nothing related the two sets. -/
theorem the_record_and_the_request_are_one_shape :
    BodyField.offset .first = 16 ∧
      BodyField.offset .resultDomainDigest = 48 ∧
      BodyField.offset .portfolioDigest = 80 ∧
      bodyBytes = 112 := by
  native_decide

/-- The receipt's count byte is the FIRST byte of the span the other two records
leave reserved, and the two reserved widths differ by exactly that byte.  This
is why `require_zero` reads `(10, 6)` twice and `(11, 5)` once. -/
theorem the_count_is_the_first_reserved_byte :
    ReceiptField.offset .recordCount = BodyField.offset .reserved ∧
      ReceiptField.offset .reserved =
        BodyField.offset .reserved + ReceiptField.width .recordCount ∧
      ReceiptField.width .recordCount + ReceiptField.width .reserved =
        BodyField.width .reserved := by
  native_decide

/-- The three coordinates tile the receipt's tail at one stride, so the `+ 128`
and `+ 2 * 128` the Rust computed at four call sites are placements. -/
theorem the_three_coordinates_are_one_stride :
    ReceiptField.offset .resultDomain =
        ReceiptField.offset .product + coordinateBytes ∧
      ReceiptField.offset .portfolio =
        ReceiptField.offset .product + 2 * coordinateBytes ∧
      ReceiptField.offset .product + recordCount * coordinateBytes =
        receiptBytes := by
  native_decide

/-- The three magics differ, which is the only thing separating three records
that share a prologue and a version. -/
theorem record_magics_are_pairwise_distinct :
    productRecordMagic ≠ requestMagic ∧ productRecordMagic ≠ receiptMagic ∧
      requestMagic ≠ receiptMagic := by
  native_decide

theorem magics_are_eight_bytes :
    productRecordMagic.length = 8 ∧ requestMagic.length = 8 ∧
      receiptMagic.length = 8 := by native_decide

/-- The four schema identities are distinct 32-byte digests.  A receipt admits a
finalized record by comparing against exactly one of them, so two that collided
would let one record stand in for another. -/
theorem schema_ids_are_distinct_and_full_width :
    [productRecordSchemaId, resultDomainSchemaId, portfolioSchemaId,
      receiptSchemaId].Nodup ∧
    [productRecordSchemaId, resultDomainSchemaId, portfolioSchemaId,
      receiptSchemaId].all (fun id => id.length == 32) = true := by
  native_decide

theorem schema_preimages_are_distinct :
    [productRecordSchemaPreimage, resultDomainSchemaPreimage,
      portfolioSchemaPreimage, receiptSchemaPreimage].Nodup := by
  native_decide

theorem rust_names_are_distinct :
    (CoordinateField.all.map CoordinateField.rustName ++
      ReceiptField.all.map ReceiptField.rustName).Nodup := by
  native_decide

end DClutch.ProductAdmissionV2Abi
