import DClutchSemantics.ScoringRuleV1
import DClutchSemantics.ScoringRuleAbiV1
import DClutchSemantics.TsEmit

/-! The browser and SDK half of `EmitScoringRuleV1Rust.lean`: the same magics,
widths, offsets, PDA domains, frames and the root-chain table, printed as the
TypeScript module `packages/dclutch-sdk/lib/generated/scoringRuleV1.ts`.

    node scripts/lean-emit.mjs DClutchSemantics.ScoringRuleAbiV1 EmitScoringRuleV1Ts.lean lib/generated/scoringRuleV1.ts [--check]

The table is printed as `bigint` literals: a price is a `u64` at scale `2^62`
and the exponentials are `u128`, neither of which a JS `number` holds. -/

open DClutch.ScoringRuleV1 DClutch.ScoringRuleAbiV1 DClutch.TsEmit DClutch.AbiSchema

def bigint (name : String) (value : Nat) : String :=
  s!"export const {name} = {value}n;"

def bigintList (name : String) (values : List Nat) : List String :=
  let rows := (List.range ((values.length + 3) / 4)).map fun row =>
    "  " ++ String.intercalate ", " (((values.drop (row * 4)).take 4).map fun v => s!"{v}n") ++ ","
  (s!"export const {name}: readonly bigint[] = [" :: rows) ++ ["] as const;"]

def frameSection (prefix_ : String) (frame : List FrameSlot) : List String :=
  let count := nat s!"{prefix_}_ACCOUNT_COUNT" frame.length
  let slots := (frame.zipIdx).map fun (slot, index) =>
    nat (slotConstantName prefix_ slot) index
  let bools (pick : FrameSlot → Bool) : String :=
    String.intercalate ", " (frame.map fun slot => if pick slot then "true" else "false")
  count :: slots ++ [
    s!"export const {prefix_}_WRITABLE: readonly boolean[] = [{bools (·.writable)}];",
    s!"export const {prefix_}_SIGNER: readonly boolean[] = [{bools (·.signer)}];"
  ]

def main : IO Unit :=
  emit (header "EmitScoringRuleV1Ts.lean" "abi:scoring-rule") [
    [ nat "FRACTION_BITS" fractionBits,
      bigint "ONE_Q62" one,
      bigint "LOG_SLACK" logSlack,
      nat "MAX_OUTCOMES" maxOutcomes,
      nat "MIN_OUTCOMES" 2,
      nat "VECTOR_BYTES" vectorBytes,
      nat "WIRE_VERSION" wireVersion,
      nat "RULE_VERSION" ruleVersion,
      nat "REFUSAL_SUB_BAND_OFFSET" refusalSubBandOffset,
      bigint "MAX_LIQUIDITY" (2 ^ 40) ],
    bigintList "EXP2_NEG_TABLE_Q62" tableList,
    [ ascii "RULE_MAGIC" ruleMagic,
      ascii "FUND_MAGIC" fundMagic,
      ascii "QUOTE_MAGIC" quoteMagic,
      ascii "FOUND_REQUEST_MAGIC" foundRequestMagic,
      ascii "QUOTE_REQUEST_MAGIC" quoteRequestMagic,
      ascii "FILL_REQUEST_MAGIC" fillRequestMagic,
      ascii "WITHDRAW_REQUEST_MAGIC" withdrawRequestMagic,
      ascii "RECEIPT_MAGIC" receiptMagic,
      ascii "FILL_WITNESS_MAGIC" fillWitnessMagic ],
    [ domain "RULE_PDA_DOMAIN" rulePdaDomain,
      domain "FUND_PDA_DOMAIN" fundPdaDomain,
      domain "QUOTE_PDA_DOMAIN" quotePdaDomain ],
    [ nat "RULE_BYTES" ruleBytes,
      nat "FUND_BYTES" fundBytes,
      nat "QUOTE_BYTES" quoteBytes,
      nat "FOUND_REQUEST_BYTES" foundRequestBytes,
      nat "QUOTE_REQUEST_BYTES" quoteRequestBytes,
      nat "FILL_REQUEST_BYTES" fillRequestBytes,
      nat "WITHDRAW_REQUEST_BYTES" withdrawRequestBytes,
      nat "RECEIPT_BYTES" receiptBytes,
      nat "FILL_WITNESS_BYTES" fillWitnessBytes ],
    offsets RuleFieldName (specialize ruleSchema),
    offsets FundField.constantName fundLayout,
    offsets QuoteField.constantName quoteLayout,
    offsets FoundRequestField.constantName foundRequestLayout,
    offsets QuoteRequestField.constantName quoteRequestLayout,
    offsets FillRequestField.constantName fillRequestLayout,
    offsets WithdrawRequestField.constantName withdrawRequestLayout,
    offsets ReceiptField.constantName receiptLayout,
    offsets FillWitnessField.constantName fillWitnessLayout,
    [ nat "FUND_PHASE_OPEN" fundPhaseOpen.toNat,
      nat "FUND_PHASE_RETIRED" fundPhaseRetired.toNat,
      nat "ROUTE_FOUND" routeFound.toNat,
      nat "ROUTE_QUOTE" routeQuote.toNat,
      nat "ROUTE_FILL" routeFill.toNat,
      nat "ROUTE_WITHDRAW" routeWithdraw.toNat ],
    frameSection "FOUND" foundFrame,
    frameSection "QUOTE" quoteFrame,
    frameSection "FILL" fillFrame,
    frameSection "WITHDRAW" withdrawFrame
  ]
