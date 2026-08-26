import DClutchSemantics.SeriesExamples
import DClutchSemantics.Codec

/-!
# Fixed Series ABI

Lean owns the exact persisted Template, Series cursor, prepaid Ticket, normalized
Registry receipt, and transition-request layouts.  The generated Rust module is
a bounded refinement over 32-byte identities and `u64` physical quantities.
-/

namespace DClutch.Series.Abi

open DClutch
open DClutch.Series

def version : Nat := 1

def templateMagic : List UInt8 := [0x44, 0x43, 0x54, 0x53] -- DCTS
def seriesMagic : List UInt8 := [0x44, 0x43, 0x53, 0x53] -- DCSS
def ticketMagic : List UInt8 := [0x44, 0x43, 0x54, 0x4b] -- DCTK
def receiptMagic : List UInt8 := [0x44, 0x43, 0x52, 0x52] -- DCRR
def requestMagic : List UInt8 := [0x44, 0x43, 0x53, 0x52] -- DCSR

def templateBytes : Nat := 240
def seriesBytes : Nat := 96
def ticketBytes : Nat := 216
def receiptBytes : Nat := 168
def requestBytes : Nat := 64

def headerVersionOffset : Nat := 4
def headerTagOffset : Nat := 6
def headerReservedOffset : Nat := 7

namespace TemplateOffset
def templateId : Nat := 8
def realmId : Nat := 40
def productId : Nat := 72
def releaseSetId : Nat := 104
def seriesRefundOwner : Nat := 136
def outcomeCount : Nat := 168
def occurrenceCount : Nat := 172
def firstOccurrenceSlot : Nat := 176
def periodSlots : Nat := 184
def retryWindowSlots : Nat := 192
def seedQuantity : Nat := 200
def marketRent : Nat := 208
def capabilityRent : Nat := 216
def foundingWork : Nat := 224
def seriesCloseRent : Nat := 232
end TemplateOffset

namespace SeriesOffset
def seriesId : Nat := 8
def templateId : Nat := 40
def nextOccurrence : Nat := 72
def reservedBody : Nat := 76
def revision : Nat := 80
def closeRent : Nat := 88
end SeriesOffset

namespace TicketOffset
def ticketId : Nat := 8
def templateId : Nat := 40
def founder : Nat := 72
def refundOwner : Nat := 104
def committedMarketId : Nat := 136
def occurrence : Nat := 168
def reservedBody : Nat := 172
def revision : Nat := 176
def hoardPrincipal : Nat := 184
def marketRent : Nat := 192
def capabilityRent : Nat := 200
def foundingWork : Nat := 208
end TicketOffset

namespace ReceiptOffset
def registryProgram : Nat := 8
def releaseSetId : Nat := 40
def observedProgram : Nat := 72
def artifactRelease : Nat := 104
def semanticRelease : Nat := 136
end ReceiptOffset

namespace RequestOffset
def nowSlot : Nat := 8
def expectedSeriesRevision : Nat := 16
def expectedTicketRevision : Nat := 24
def workRecipient : Nat := 32
end RequestOffset

def coreRoleTag : Nat := 0
def receiptAuthenticatedFlags : Nat := 3
def actionConsume : Nat := 0
def actionExpire : Nat := 1
def actionClose : Nat := 2
def phaseActive : Nat := 0
def phaseTerminal : Nat := 1
def phaseClosed : Nat := 2
def ticketReady : Nat := 0
def ticketConsumed : Nat := 1
def ticketExpired : Nat := 2

def encodeIdentity (identity : Identity) : List UInt8 := Codec.encodeLE 32 identity
def encodeU16 (value : Nat) : List UInt8 := Codec.encodeLE 2 value
def encodeU32 (value : Nat) : List UInt8 := Codec.encodeLE 4 value
def encodeU64 (value : Nat) : List UInt8 := Codec.encodeLE 8 value

def phaseTag : Phase -> UInt8
  | .active => UInt8.ofNat phaseActive
  | .terminal => UInt8.ofNat phaseTerminal
  | .closed => UInt8.ofNat phaseClosed

def ticketPhaseTag : TicketPhase -> UInt8
  | .ready => UInt8.ofNat ticketReady
  | .consumed => UInt8.ofNat ticketConsumed
  | .expired => UInt8.ofNat ticketExpired

def encodeTemplate (value : Template) : List UInt8 :=
  templateMagic ++ encodeU16 version ++ [0, 0] ++
  encodeIdentity value.templateId ++ encodeIdentity value.realmId ++
  encodeIdentity value.productId ++ encodeIdentity value.releaseSetId ++
  encodeIdentity value.seriesRefundOwner ++
  encodeU32 value.outcomeCount ++ encodeU32 value.occurrenceCount ++
  encodeU64 value.firstOccurrenceSlot ++ encodeU64 value.periodSlots ++
  encodeU64 value.retryWindowSlots ++ encodeU64 value.seedQuantity ++
  encodeU64 value.marketRentLamports ++ encodeU64 value.capabilityRentLamports ++
  encodeU64 value.foundingWorkLamports ++ encodeU64 value.seriesCloseRentLamports

def encodeSeries (value : State) : List UInt8 :=
  seriesMagic ++ encodeU16 version ++ [phaseTag value.phase, 0] ++
  encodeIdentity value.seriesId ++ encodeIdentity value.templateId ++
  encodeU32 value.nextOccurrence ++ [0, 0, 0, 0] ++
  encodeU64 value.revision ++ encodeU64 value.closeRentLamports

def encodeTicket (value : Ticket) : List UInt8 :=
  ticketMagic ++ encodeU16 version ++ [ticketPhaseTag value.phase, 0] ++
  encodeIdentity value.ticketId ++ encodeIdentity value.templateId ++
  encodeIdentity value.founder ++ encodeIdentity value.refundOwner ++
  encodeIdentity value.committedMarketId ++ encodeU32 value.occurrence ++
  [0, 0, 0, 0] ++ encodeU64 value.revision ++
  encodeU64 value.funds.hoardPrincipal ++ encodeU64 value.funds.marketRent ++
  encodeU64 value.funds.capabilityRent ++ encodeU64 value.funds.foundingWork

def encodeReceipt (value : ExecutionRelease.Receipt) : List UInt8 :=
  receiptMagic ++ encodeU16 version ++ [UInt8.ofNat coreRoleTag,
    UInt8.ofNat receiptAuthenticatedFlags] ++
  encodeIdentity value.registryProgram ++ encodeIdentity value.releaseSetId ++
  encodeIdentity value.observed.program ++ encodeIdentity value.observed.artifactRelease ++
  encodeIdentity value.observed.semanticRelease

def encodeRequest (frame : Frame) : List UInt8 :=
  let action := match frame.command with
    | .consume _ => actionConsume | .expire => actionExpire | .close => actionClose
  let workRecipient := match frame.command with
    | .consume recipient => recipient | .expire | .close => 0
  requestMagic ++ encodeU16 version ++ [UInt8.ofNat action, 0] ++
  encodeU64 frame.nowSlot ++ encodeU64 frame.expectedSeriesRevision ++
  encodeU64 frame.expectedTicketRevision ++ encodeIdentity workRecipient

theorem template_layout_is_exact :
    (encodeTemplate Examples.template).length = templateBytes := by native_decide

theorem series_layout_is_exact :
    (encodeSeries Examples.series0).length = seriesBytes := by native_decide

theorem ticket_layout_is_exact :
    (encodeTicket Examples.ticket0).length = ticketBytes := by native_decide

theorem receipt_layout_is_exact :
    (encodeReceipt Examples.releaseAdmission.receipt).length = receiptBytes := by native_decide

theorem request_layout_is_exact :
    (encodeRequest Examples.consume0).length = requestBytes := by native_decide

end DClutch.Series.Abi
