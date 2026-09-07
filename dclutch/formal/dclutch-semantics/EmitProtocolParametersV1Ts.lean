import DClutchSemantics.ProtocolParametersV1
import DClutchSemantics.TsEmit

/-! The SDK's view of the governed record: the constitution's constants, the
genesis values, the seed domains and the record offsets, printed from the same
Lean object the Rust reads.  The browser's terms surfaces read the crank cap
and the fee band from a decoded record, and fall back to the genesis constants
printed here only when the cluster has no record, saying so. -/

open DClutch DClutch.ProtocolParametersV1 DClutch.TsEmit

def recordName : RecordField -> String
  | .magic => "PROTOCOL_PARAMETERS_RECORD_MAGIC_OFFSET_V1"
  | .version => "PROTOCOL_PARAMETERS_RECORD_VERSION_OFFSET_V1"
  | .kind => "PROTOCOL_PARAMETERS_RECORD_KIND_OFFSET_V1"
  | .bump => "PROTOCOL_PARAMETERS_RECORD_BUMP_OFFSET_V1"
  | .reservedHeader => "PROTOCOL_PARAMETERS_RECORD_RESERVED_HEADER_OFFSET_V1"
  | .governanceAuthority => "PROTOCOL_PARAMETERS_RECORD_GOVERNANCE_AUTHORITY_OFFSET_V1"
  | .protocolBeneficiary => "PROTOCOL_PARAMETERS_RECORD_PROTOCOL_BENEFICIARY_OFFSET_V1"
  | .pendingDigest => "PROTOCOL_PARAMETERS_RECORD_PENDING_DIGEST_OFFSET_V1"
  | .generation => "PROTOCOL_PARAMETERS_RECORD_GENERATION_OFFSET_V1"
  | .activationSlot => "PROTOCOL_PARAMETERS_RECORD_ACTIVATION_SLOT_OFFSET_V1"
  | .changeDelaySlots => "PROTOCOL_PARAMETERS_RECORD_CHANGE_DELAY_SLOTS_OFFSET_V1"
  | .pendingEarliestApplySlot => "PROTOCOL_PARAMETERS_RECORD_PENDING_EARLIEST_APPLY_SLOT_OFFSET_V1"
  | .closerRewardCapLamports => "PROTOCOL_PARAMETERS_RECORD_CLOSER_REWARD_CAP_OFFSET_V1"
  | .crankRewardCapLamports => "PROTOCOL_PARAMETERS_RECORD_CRANK_REWARD_CAP_OFFSET_V1"
  | .maxFeeBasisPoints => "PROTOCOL_PARAMETERS_RECORD_MAX_FEE_BASIS_POINTS_OFFSET_V1"
  | .protocolTakeBasisPoints => "PROTOCOL_PARAMETERS_RECORD_TAKE_BASIS_POINTS_OFFSET_V1"
  | .closerCarveBasisPoints => "PROTOCOL_PARAMETERS_RECORD_CLOSER_CARVE_BASIS_POINTS_OFFSET_V1"
  | .reservedTail => "PROTOCOL_PARAMETERS_RECORD_RESERVED_TAIL_OFFSET_V1"

def receiptName : ReceiptField -> String
  | .magic => "PROTOCOL_PARAMETERS_RECEIPT_MAGIC_OFFSET_V1"
  | .version => "PROTOCOL_PARAMETERS_RECEIPT_VERSION_OFFSET_V1"
  | .reservedHeader => "PROTOCOL_PARAMETERS_RECEIPT_RESERVED_HEADER_OFFSET_V1"
  | .previousDigest => "PROTOCOL_PARAMETERS_RECEIPT_PREVIOUS_DIGEST_OFFSET_V1"
  | .newDigest => "PROTOCOL_PARAMETERS_RECEIPT_NEW_DIGEST_OFFSET_V1"
  | .generation => "PROTOCOL_PARAMETERS_RECEIPT_GENERATION_OFFSET_V1"
  | .proposedAtSlot => "PROTOCOL_PARAMETERS_RECEIPT_PROPOSED_AT_SLOT_OFFSET_V1"
  | .activationSlot => "PROTOCOL_PARAMETERS_RECEIPT_ACTIVATION_SLOT_OFFSET_V1"
  | .delaySlots => "PROTOCOL_PARAMETERS_RECEIPT_DELAY_SLOTS_OFFSET_V1"

def main : IO Unit :=
  emit (header "EmitProtocolParametersV1Ts.lean" "abi:protocol-parameters") [
    [ nat "PROTOCOL_PARAMETERS_ABI_VERSION_V1" abiVersion,
      nat "PROTOCOL_PARAMETERS_RECORD_BYTES_V1" recordBytes,
      nat "PROTOCOL_PARAMETERS_RECEIPT_BYTES_V1" receiptBytes,
      nat "PROTOCOL_PARAMETERS_REQUEST_BYTES_V1" requestBytes,
      ascii "PROTOCOL_PARAMETERS_RECORD_MAGIC_V1" "DCLTPRM1",
      ascii "PROTOCOL_PARAMETERS_RECEIPT_MAGIC_V1" "DCLTPRC1",
      ascii "PROTOCOL_PARAMETERS_REQUEST_MAGIC_V1" "DCLTPRQ1",
      domain "PROTOCOL_PARAMETERS_PDA_DOMAIN_V1" recordPdaDomain,
      domain "PROTOCOL_PARAMETERS_RECEIPT_PDA_DOMAIN_V1" receiptPdaDomain ],
    [ nat "PROTOCOL_BASIS_POINT_DENOMINATOR_V1" basisPointDenominator,
      nat "PROTOCOL_ABSOLUTE_FEE_CEILING_BASIS_POINTS_V1" absoluteFeeCeilingBasisPoints,
      nat "PROTOCOL_SLOTS_PER_NOMINAL_DAY_V1" slotsPerNominalDay,
      nat "PROTOCOL_MINIMUM_CHANGE_DELAY_SLOTS_V1" minimumChangeDelaySlots,
      s!"export const PROTOCOL_TAKE_ADMITTED_THIS_RELEASE_V1 = {protocolTakeAdmittedThisRelease} as const;" ],
    [ nat "PROTOCOL_GENESIS_MAX_FEE_BASIS_POINTS_V1" genesis.maxFeeBasisPoints,
      nat "PROTOCOL_GENESIS_TAKE_BASIS_POINTS_V1" genesis.protocolTakeBasisPoints,
      nat "PROTOCOL_GENESIS_CLOSER_CARVE_BASIS_POINTS_V1" genesis.closerCarveBasisPoints,
      nat "PROTOCOL_GENESIS_CLOSER_REWARD_CAP_LAMPORTS_V1" genesis.closerRewardCapLamports,
      nat "PROTOCOL_GENESIS_CRANK_REWARD_CAP_LAMPORTS_V1" genesis.crankRewardCapLamports ],
    offsets recordName recordLayout,
    offsets receiptName receiptLayout
  ]
