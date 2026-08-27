import DClutchSemantics.SeriesOccurrenceV3
import DClutchSemantics.Codec

/-!
# Series occurrence V3 physical ABI

The fixed records carry only content identities, schedule coordinates, and
four exact funding compartments.  Merkle proof bytes are a borrowed physical
argument whose canonical length is derived from `occurrenceCount`; they are not
persisted in any of these records.
-/

namespace DClutch.SeriesOccurrenceV3.Abi

open DClutch
open DClutch.SeriesOccurrenceV3

def schemaVersion : Nat := 3
def profile : Nat := 1

def templateMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x53, 0x54, 0x56, 0x33] -- DCLTSTV3
def occurrenceMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x53, 0x4f, 0x56, 0x33] -- DCLTSOV3
def ticketMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x53, 0x4b, 0x56, 0x33] -- DCLTSKV3

def templateBytes : Nat := 400
def occurrenceBytes : Nat := 288
def ticketBytes : Nat := 256
def maximumMerkleHeight : Nat := 32

def templateSchemaReleasePreimage : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x63, 0x68, 0x65,
   0x6d, 0x61, 0x2f, 0x73, 0x65, 0x72, 0x69, 0x65, 0x73, 0x2d, 0x74, 0x65,
   0x6d, 0x70, 0x6c, 0x61, 0x74, 0x65, 0x2d, 0x76, 0x33]
def templateSchemaReleaseId : List UInt8 :=
  [0x60, 0xc8, 0xa3, 0x8f, 0x6f, 0xbf, 0x45, 0xf0, 0x15, 0xdd, 0xaf, 0x06,
   0x13, 0xdb, 0xaa, 0x06, 0x9b, 0x8f, 0xaa, 0x1c, 0x61, 0x73, 0x13, 0x11,
   0xe6, 0x0d, 0xe7, 0x71, 0x32, 0xc1, 0x1e, 0x10]
def occurrenceSchemaReleasePreimage : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x63, 0x68, 0x65,
   0x6d, 0x61, 0x2f, 0x73, 0x65, 0x72, 0x69, 0x65, 0x73, 0x2d, 0x6f, 0x63,
   0x63, 0x75, 0x72, 0x72, 0x65, 0x6e, 0x63, 0x65, 0x2d, 0x76, 0x33]
def occurrenceSchemaReleaseId : List UInt8 :=
  [0x15, 0x93, 0x36, 0x28, 0x4b, 0x80, 0xd0, 0x3b, 0xa3, 0x50, 0xe7, 0xe1,
   0xba, 0x92, 0xab, 0xbb, 0xdb, 0xc1, 0x22, 0x56, 0x73, 0x14, 0x50, 0x5c,
   0xc9, 0x41, 0x62, 0x96, 0xec, 0x35, 0xcb, 0xae]
def ticketSchemaReleasePreimage : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x63, 0x68, 0x65,
   0x6d, 0x61, 0x2f, 0x73, 0x65, 0x72, 0x69, 0x65, 0x73, 0x2d, 0x74, 0x69,
   0x63, 0x6b, 0x65, 0x74, 0x2d, 0x76, 0x33]
def ticketSchemaReleaseId : List UInt8 :=
  [0x18, 0xd6, 0x37, 0xb5, 0x0f, 0xb9, 0x9c, 0xdd, 0x2b, 0x22, 0x29, 0x45,
   0xcd, 0xa6, 0x3d, 0xe1, 0xbb, 0x71, 0x3b, 0x6d, 0xf4, 0x73, 0xcc, 0x9c,
   0xb9, 0x98, 0xc7, 0xc9, 0xc9, 0xc3, 0xfa, 0x0c]

def templateContentDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x74, 0x65, 0x6d, 0x70, 0x6c, 0x61, 0x74, 0x65, 0x2d,
   0x76, 0x33]
def occurrenceContentDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x6f, 0x63, 0x63, 0x75, 0x72, 0x72, 0x65, 0x6e, 0x63,
   0x65, 0x2d, 0x76, 0x33]
def ticketContentDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x74, 0x69, 0x63, 0x6b, 0x65, 0x74, 0x2d, 0x76, 0x33]
def projectionNodeDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x70, 0x72, 0x6f, 0x6a, 0x65, 0x63, 0x74, 0x69, 0x6f,
   0x6e, 0x2d, 0x6e, 0x6f, 0x64, 0x65, 0x2d, 0x76, 0x33]
def fundingListDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x66, 0x75, 0x6e, 0x64, 0x69, 0x6e, 0x67, 0x2d, 0x6c,
   0x69, 0x73, 0x74, 0x2d, 0x76, 0x33]

namespace TemplateOffset
def occurrenceCount : Nat := 12
def firstSlot : Nat := 16
def periodSlots : Nat := 24
def retryWindow : Nat := 32
def closeRent : Nat := 40
def realm : Nat := 48
def releaseSet : Nat := 80
def productGenerator : Nat := 112
def occurrenceGenerator : Nat := 144
def capabilityTemplate : Nat := 176
def productDerivation : Nat := 208
def occurrenceDerivation : Nat := 240
def capabilityDerivation : Nat := 272
def fundingDerivation : Nat := 304
def projectionRoot : Nat := 336
def refundOwner : Nat := 368
end TemplateOffset

namespace OccurrenceOffset
def occurrence : Nat := 12
def scheduledSlot : Nat := 16
def productRecord : Nat := 24
def resolutionPolicy : Nat := 56
def liabilityBasis : Nat := 88
def rationalRepresentation : Nat := 120
def capabilityManifest : Nat := 152
def fundingList : Nat := 184
def market : Nat := 216
def hoardPrincipal : Nat := 248
def marketRent : Nat := 256
def capabilityNative : Nat := 264
def foundingWork : Nat := 272
def reserved : Nat := 280
def reservedBytes : Nat := 8
end OccurrenceOffset

namespace TicketOffset
def occurrence : Nat := 12
def template : Nat := 16
def occurrenceId : Nat := 48
def market : Nat := 80
def fundingList : Nat := 112
def founder : Nat := 144
def refundOwner : Nat := 176
def hoardPrincipal : Nat := 208
def marketRent : Nat := 216
def capabilityNative : Nat := 224
def foundingWork : Nat := 232
def reserved : Nat := 240
def reservedBytes : Nat := 16
end TicketOffset

def encodeU16 (value : Nat) : List UInt8 := Codec.encodeLE 2 value
def encodeU32 (value : Nat) : List UInt8 := Codec.encodeLE 4 value
def encodeU64 (value : Nat) : List UInt8 := Codec.encodeLE 8 value
def encodeIdentity (value : Identity) : List UInt8 := Codec.encodeLE 32 value

def encodeTemplate (value : Template) : List UInt8 :=
  templateMagic ++ encodeU16 schemaVersion ++ encodeU16 profile ++
  encodeU32 value.occurrenceCount ++ encodeU64 value.firstOccurrenceSlot ++
  encodeU64 value.periodSlots ++ encodeU64 value.retryWindowSlots ++
  encodeU64 value.seriesCloseRentLamports ++ encodeIdentity value.realmId ++
  encodeIdentity value.releaseSetId ++ encodeIdentity value.productGeneratorId ++
  encodeIdentity value.occurrenceGeneratorId ++
  encodeIdentity value.capabilityTemplateId ++
  encodeIdentity value.productDerivationPolicyId ++
  encodeIdentity value.occurrenceDerivationPolicyId ++
  encodeIdentity value.capabilityDerivationPolicyId ++
  encodeIdentity value.fundingDerivationPolicyId ++
  encodeIdentity value.occurrenceProjectionRoot ++
  encodeIdentity value.seriesRefundOwner

def encodeFunds (funds : FoundingFunds) : List UInt8 :=
  encodeU64 funds.hoardPrincipal ++ encodeU64 funds.marketRentLamports ++
  encodeU64 funds.capabilityNativeLamports ++ encodeU64 funds.foundingWorkLamports

def encodeOccurrence (value : Occurrence) : List UInt8 :=
  occurrenceMagic ++ encodeU16 schemaVersion ++ encodeU16 profile ++
  encodeU32 value.occurrence ++ encodeU64 value.scheduledSlot ++
  encodeIdentity value.productRecordId ++
  encodeIdentity value.resolutionPolicyId ++
  encodeIdentity value.liabilityBasisId ++
  encodeIdentity value.rationalRepresentationId ++
  encodeIdentity value.capabilityManifestId ++ encodeIdentity value.fundingListId ++
  encodeIdentity value.marketId ++ encodeFunds value.funds ++ List.replicate 8 0

def encodeTicket (value : TicketCommitment) : List UInt8 :=
  ticketMagic ++ encodeU16 schemaVersion ++ encodeU16 profile ++
  encodeU32 value.occurrence ++ encodeIdentity value.templateId ++
  encodeIdentity value.occurrenceId ++ encodeIdentity value.marketId ++
  encodeIdentity value.fundingListId ++ encodeIdentity value.founder ++
  encodeIdentity value.refundOwner ++ encodeFunds value.funds ++ List.replicate 16 0

def exampleFunds : FoundingFunds := {
  hoardPrincipal := 10
  marketRentLamports := 20
  capabilityNativeLamports := 30
  foundingWorkLamports := 40
}

def exampleTemplate : Template := {
  realmId := 1
  releaseSetId := 2
  productGeneratorId := 3
  occurrenceGeneratorId := 4
  capabilityTemplateId := 5
  productDerivationPolicyId := 6
  occurrenceDerivationPolicyId := 7
  capabilityDerivationPolicyId := 8
  fundingDerivationPolicyId := 9
  occurrenceProjectionRoot := 10
  seriesRefundOwner := 11
  occurrenceCount := 3
  firstOccurrenceSlot := 100
  periodSlots := 10
  retryWindowSlots := 5
  seriesCloseRentLamports := 7
}

def exampleOccurrence : Occurrence := {
  occurrence := 1
  scheduledSlot := 110
  productRecordId := 13
  resolutionPolicyId := 15
  liabilityBasisId := 16
  rationalRepresentationId := 17
  capabilityManifestId := 18
  fundingListId := 19
  marketId := 20
  funds := exampleFunds
}

def exampleTicket : TicketCommitment := {
  templateId := 12
  occurrenceId := 21
  marketId := 20
  fundingListId := 19
  founder := 22
  refundOwner := 23
  occurrence := 1
  funds := exampleFunds
}

theorem template_layout_is_exact :
    (encodeTemplate exampleTemplate).length = templateBytes := by native_decide

theorem occurrence_layout_is_exact :
    (encodeOccurrence exampleOccurrence).length = occurrenceBytes := by native_decide

theorem ticket_layout_is_exact :
    (encodeTicket exampleTicket).length = ticketBytes := by native_decide

end DClutch.SeriesOccurrenceV3.Abi
