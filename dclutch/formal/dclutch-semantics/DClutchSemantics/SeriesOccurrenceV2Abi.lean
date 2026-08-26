import DClutchSemantics.SeriesOccurrenceV2
import DClutchSemantics.Codec

/-!
# Series occurrence V2 physical ABI

The fixed records carry only content identities, schedule coordinates, and
four exact funding compartments.  Merkle proof bytes are a borrowed physical
argument whose canonical length is derived from `occurrenceCount`; they are not
persisted in any of these records.
-/

namespace DClutch.SeriesOccurrenceV2.Abi

open DClutch
open DClutch.SeriesOccurrenceV2

def schemaVersion : Nat := 2
def profile : Nat := 1

def templateMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x53, 0x54, 0x56, 0x32] -- DCLTSTV2
def occurrenceMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x53, 0x4f, 0x56, 0x32] -- DCLTSOV2
def ticketMagic : List UInt8 :=
  [0x44, 0x43, 0x4c, 0x54, 0x53, 0x4b, 0x56, 0x32] -- DCLTSKV2

def templateBytes : Nat := 400
def occurrenceBytes : Nat := 320
def ticketBytes : Nat := 256
def maximumMerkleHeight : Nat := 32

def templateContentDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x74, 0x65, 0x6d, 0x70, 0x6c, 0x61, 0x74, 0x65, 0x2d,
   0x76, 0x32]
def occurrenceContentDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x6f, 0x63, 0x63, 0x75, 0x72, 0x72, 0x65, 0x6e, 0x63,
   0x65, 0x2d, 0x76, 0x32]
def ticketContentDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x74, 0x69, 0x63, 0x6b, 0x65, 0x74, 0x2d, 0x76, 0x32]
def projectionNodeDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x70, 0x72, 0x6f, 0x6a, 0x65, 0x63, 0x74, 0x69, 0x6f,
   0x6e, 0x2d, 0x6e, 0x6f, 0x64, 0x65, 0x2d, 0x76, 0x32]
def fundingListDomain : List UInt8 :=
  [0x64, 0x63, 0x6c, 0x75, 0x74, 0x63, 0x68, 0x2f, 0x73, 0x65, 0x72, 0x69,
   0x65, 0x73, 0x2d, 0x66, 0x75, 0x6e, 0x64, 0x69, 0x6e, 0x67, 0x2d, 0x6c,
   0x69, 0x73, 0x74, 0x2d, 0x76, 0x32]

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
def product : Nat := 24
def resultDomain : Nat := 56
def resolutionPolicy : Nat := 88
def liabilityBasis : Nat := 120
def rationalRepresentation : Nat := 152
def capabilityManifest : Nat := 184
def fundingList : Nat := 216
def market : Nat := 248
def hoardPrincipal : Nat := 280
def marketRent : Nat := 288
def capabilityNative : Nat := 296
def foundingWork : Nat := 304
def reserved : Nat := 312
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
  encodeIdentity value.productId ++ encodeIdentity value.resultDomainId ++
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
  productId := 13
  resultDomainId := 14
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

end DClutch.SeriesOccurrenceV2.Abi
