import DClutchSemantics.UpkeepVaultV1
import DClutchSemantics.TsEmit

/-! The SDK's view of the upkeep vault: magics, the seed domain, the record
offsets and the source-class tags, printed from the same Lean object the Rust
reads. -/

open DClutch DClutch.UpkeepVaultV1 DClutch.TsEmit

def recordName : RecordField -> String
  | .magic => "UPKEEP_VAULT_RECORD_MAGIC_OFFSET_V1"
  | .version => "UPKEEP_VAULT_RECORD_VERSION_OFFSET_V1"
  | .kind => "UPKEEP_VAULT_RECORD_KIND_OFFSET_V1"
  | .bump => "UPKEEP_VAULT_RECORD_BUMP_OFFSET_V1"
  | .reservedHeader => "UPKEEP_VAULT_RECORD_RESERVED_HEADER_OFFSET_V1"
  | .inflowTotal => "UPKEEP_VAULT_RECORD_INFLOW_TOTAL_OFFSET_V1"
  | .outflowTotal => "UPKEEP_VAULT_RECORD_OUTFLOW_TOTAL_OFFSET_V1"
  | .inflowResidue => "UPKEEP_VAULT_RECORD_INFLOW_RESIDUE_OFFSET_V1"
  | .inflowDonation => "UPKEEP_VAULT_RECORD_INFLOW_DONATION_OFFSET_V1"
  | .inflowSeatRent => "UPKEEP_VAULT_RECORD_INFLOW_SEAT_RENT_OFFSET_V1"
  | .inflowDeposit => "UPKEEP_VAULT_RECORD_INFLOW_DEPOSIT_OFFSET_V1"
  | .creditCount => "UPKEEP_VAULT_RECORD_CREDIT_COUNT_OFFSET_V1"
  | .lastCreditDigest => "UPKEEP_VAULT_RECORD_LAST_CREDIT_DIGEST_OFFSET_V1"
  | .reservedTail => "UPKEEP_VAULT_RECORD_RESERVED_TAIL_OFFSET_V1"

def main : IO Unit :=
  emit (header "EmitUpkeepVaultV1Ts.lean" "abi:upkeep-vault") [
    [ nat "UPKEEP_VAULT_ABI_VERSION_V1" abiVersion,
      nat "UPKEEP_VAULT_RECORD_BYTES_V1" recordBytes,
      nat "UPKEEP_VAULT_REQUEST_BYTES_V1" requestBytes,
      nat "UPKEEP_VAULT_RECEIPT_BYTES_V1" receiptBytes,
      ascii "UPKEEP_VAULT_RECORD_MAGIC_V1" "DCLCUPK1",
      ascii "UPKEEP_VAULT_REQUEST_MAGIC_V1" "DCLCUPQ1",
      ascii "UPKEEP_VAULT_RECEIPT_MAGIC_V1" "DCLCUPC1",
      domain "CUSTODY_UPKEEP_VAULT_PDA_DOMAIN_V1" pdaDomain ],
    SourceClass.all.map fun c => nat (SourceClass.rustName c) (SourceClass.tag c).toNat,
    [ nat "UPKEEP_OPERATION_FOUND_TAG_V1" (Operation.tag .found).toNat,
      nat "UPKEEP_OPERATION_CREDIT_TAG_V1" (Operation.tag .credit).toNat,
      nat "UPKEEP_OPERATION_DEBIT_TAG_RESERVED_V1" debitTag.toNat ],
    offsets recordName recordLayout
  ]
